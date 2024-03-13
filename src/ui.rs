use std::{collections::HashMap, fmt::Write, future::Future, sync::Arc, time::Duration};

use async_trait::async_trait;
use colored::Colorize;
use comfy_table::Table;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use localsend_lib::{
    scanner::MulticastDeviceScanner,
    send::{SendingFiles, UploadProgress},
    Error, Result,
};
use localsend_proto::{
    dto::{FileDto, FileType},
    Device,
};

const PROGRESS_BAR_NO_NERD_TICK_CHARS: &'static str = "+x*";

pub struct FileProgressBar {
    style: ProgressStyle,
    pbs: HashMap<String, ProgressBar>,
    files: HashMap<String, FileDto>,
}

impl FileProgressBar {
    pub fn new(files: HashMap<String, FileDto>, use_nerd_fonts: bool) -> Self {
        let mut style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} [{elapsed_precise}] [{msg}] [{bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
            .progress_chars("#>-");
        if !use_nerd_fonts {
            style = style.tick_chars(PROGRESS_BAR_NO_NERD_TICK_CHARS);
        }
        Self {
            style,
            pbs: HashMap::new(),
            files,
        }
    }

    pub fn update(&mut self, progress: UploadProgress) {
        if let Some(pb) = self.pbs.get(&progress.file_id) {
            pb.set_position(progress.position);
            if progress.finish {
                pb.finish();
            }
            return;
        }

        let file = self.files.get(&progress.file_id).unwrap();
        let index = self.files.values().position(|f| f.id == file.id).unwrap();

        let pb = indicatif::ProgressBar::new(file.size)
            .with_prefix(format!("[{}/{}]", index + 1, self.files.len()))
            .with_style(self.style.clone())
            .with_message(file.file_name.clone())
            .with_position(progress.position);

        if progress.finish {
            pb.finish();
        }
        self.pbs.insert(progress.file_id, pb);
    }
}

#[async_trait]
pub trait InteractiveUI {
    async fn select_device(&self, scanner: &Arc<MulticastDeviceScanner>) -> Result<Device>;

    async fn show_loading<T>(&self, message: String, task: T) -> T::Output
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static;

    fn select_files(&self, files: Vec<FileDto>) -> Option<Vec<FileDto>>;

    fn print_files(&self, files: &SendingFiles);

    fn print_error(&self, error: &Error);

    fn ask_continue(&self) -> bool;
}

#[derive(Clone)]
pub struct PromptUI {
    pub use_nerd_fonts: bool,
}

impl Default for PromptUI {
    fn default() -> Self {
        Self {
            use_nerd_fonts: true,
        }
    }
}

#[async_trait]
impl InteractiveUI for PromptUI {
    async fn select_device(&self, scanner: &Arc<MulticastDeviceScanner>) -> Result<Device> {
        loop {
            let devices = {
                let scanner = scanner.clone();
                self.show_loading("Scanning".to_owned(), async move { scanner.scan().await })
                    .await?
            };

            use colored::Colorize;
            fn format_device_alias(device: &Device) -> String {
                let (r, g, b) = match device.device_type {
                    localsend_proto::DeviceType::Mobile => (95, 175, 0),
                    localsend_proto::DeviceType::Desktop => (95, 175, 255),
                    localsend_proto::DeviceType::Web => (0, 128, 128),
                    localsend_proto::DeviceType::Headless => (95, 0, 175),
                    localsend_proto::DeviceType::Server => (128, 0, 128),
                };
                let alias = device.alias.truecolor(r, g, b);
                if let Some(model) = &device.device_model {
                    format!("{} {}", model.truecolor(r, g, b), alias)
                } else {
                    format!("{}", alias)
                }
            }

            enum SelectItem<'a> {
                Refresh,
                Device(&'a Device),
            }

            impl<'a> std::fmt::Display for SelectItem<'a> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match self {
                        SelectItem::Refresh => {
                            f.write_str("Refresh devices".bold().to_string().as_str())
                        }
                        SelectItem::Device(device) => {
                            f.write_str(format_device_alias(device).as_str())
                        }
                    }
                }
            }

            let mut items: Vec<SelectItem> = devices
                .iter()
                .map(|device| SelectItem::Device(device))
                .collect();
            items.insert(0, SelectItem::Refresh);

            let selection = inquire::Select::new("Select the device you want to send to", items)
                .with_help_message("↑↓ to move, enter to select, type to filter, esc to exit")
                .with_vim_mode(true)
                .prompt_skippable();
            match selection {
                Ok(Some(SelectItem::Refresh)) => {
                    continue;
                }
                Ok(Some(SelectItem::Device(device))) => {
                    return Ok(device.clone());
                }
                _ => std::process::exit(0),
            }
        }
    }

    async fn show_loading<T>(&self, message: String, task: T) -> T::Output
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let mut style = ProgressStyle::default_spinner();
        if !self.use_nerd_fonts {
            style = style.tick_chars(PROGRESS_BAR_NO_NERD_TICK_CHARS);
        }
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_message(message);
        pb.set_style(style);
        let l = pb.clone();
        let timer = tokio::spawn(async move {
            loop {
                l.inc(1);
                tokio::time::sleep(Duration::from_millis(64)).await;
            }
        });
        let output = task.await;
        pb.finish_and_clear();
        timer.abort();
        output
    }

    fn select_files(&self, files: Vec<FileDto>) -> Option<Vec<FileDto>> {
        struct SelectItem<'a>(&'a PromptUI, &'a FileDto);

        impl<'a> std::fmt::Display for SelectItem<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(
                    format!("{} {}", self.0.file_name(self.1), self.0.file_size(self.1)).as_str(),
                )
            }
        }

        let items: Vec<SelectItem> = files.iter().map(|file| SelectItem(self, &file)).collect();
        let defaults: Vec<usize> = items.iter().enumerate().map(|(index, _)| index).collect();
        let selection = inquire::MultiSelect::new("Select the files you want to receive", items)
            .with_default(&defaults)
            .with_help_message(
                "↑↓ to move, space to select one, → to all, ← to none, type to filter, esc to cancel",
            )
            .with_vim_mode(true)
            .prompt_skippable();
        match selection {
            Ok(Some(files)) => Some(files.into_iter().map(|f| f.1.to_owned()).collect()),
            _ => None,
        }
    }

    fn print_files(&self, files: &SendingFiles) {
        let mut table = Table::new();
        table.set_header(vec!["No.", "Name", "Size"]);
        for file in files.files.values() {
            table.add_row(vec![
                &format!("{}", file.index + 1),
                &self.file_name(&file.file),
                &self.file_size(&file.file),
            ]);
        }
        println!("{}", table);
    }

    fn print_error(&self, error: &Error) {
        println!("{}", error.to_string().bold().red());
    }

    fn ask_continue(&self) -> bool {
        inquire::Confirm::new("Do you want to continue sending to other device?")
            .with_default(true)
            .with_help_message("enter to continue, other to exit")
            .with_parser(&|s| Ok(s == "y" || s == "Y"))
            .prompt_skippable()
            .is_ok_and(|r| r == Some(true))
    }
}

impl PromptUI {
    fn file_name(&self, file: &FileDto) -> String {
        format!("{} {}", self.file_icon(&file.file_type), file.file_name)
    }

    fn file_icon(&self, file_type: &FileType) -> &'static str {
        if !self.use_nerd_fonts {
            return "";
        }
        match file_type {
            FileType::Image => "󰈟",
            FileType::Video => "󰈫",
            FileType::Pdf => "󰈧",
            FileType::Text => "󰈙",
            FileType::Apk => "󰀲",
            FileType::Other => "󰈔",
        }
    }

    fn file_size(&self, file: &FileDto) -> String {
        humansize::format_size(file.size, humansize::DECIMAL)
    }
}
