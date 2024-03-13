use std::{net::Ipv4Addr, path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use itertools::Itertools;
use localsend_lib::{
    scanner::MulticastDeviceScanner,
    send::{SendError, SendSession, SendingFiles, UploadProgress},
    server::{start_api_server, ClientMessage, ServerMessage, ServerState},
    util::device,
    Result, Settings,
};
use localsend_proto::{
    Device, DEFAULT_HTTP_PORT, DEFAULT_MULTICAST, DEFAULT_PORT, PROTOCOL_VERSION_2,
};
use simple_logger::SimpleLogger;

use crate::ui::{FileProgressBar, InteractiveUI, PromptUI};

mod ui;

#[derive(Parser)]
struct Args {
    /// Alias of localsend, use hostname by default
    #[arg(long, env = "LOCALSEND_ALIAS")]
    alias: Option<String>,

    /// Multicast address of localsend
    #[arg(long, env = "LOCALSEND_MULTIADDR", default_value = DEFAULT_MULTICAST)]
    multiaddr: Ipv4Addr,

    /// Port of localsend
    #[arg(long, env = "LOCALSEND_PORT", default_value_t = DEFAULT_PORT)]
    port: u16,

    /// Port of localsend http server
    #[arg(long, env = "LOCALSEND_HTTP_PORT", default_value_t = DEFAULT_HTTP_PORT)]
    http_port: u16,

    /// Do not use nerd fonts
    #[arg(long)]
    no_nerd: bool,

    #[clap(subcommand)]
    cmd: SubCommand,
}

impl Args {
    fn is_receive_mode(&self) -> bool {
        if let SubCommand::Receive(_) = self.cmd {
            true
        } else {
            false
        }
    }
}

#[derive(clap::Subcommand)]
enum SubCommand {
    /// Run as receive server
    Receive(ReceiveArgs),
    /// Run as send client
    Send(SendArgs),
}

#[derive(Parser)]
struct ReceiveArgs {
    /// File save destination path
    #[arg(long = "dest", env = "LOCALSEND_DESTINATION", default_value = ".")]
    destination: PathBuf,

    /// Quickly save all files without asking
    #[arg(long = "quick-save")]
    quick_save: bool,
}

#[derive(Parser)]
struct SendArgs {
    /// Text or file path to be sent
    #[arg(required = true)]
    input: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .env()
        .init()
        .expect("Failed to init logger");

    let args: Args = Args::parse();

    let local_addr = device::local_addr()?;
    log::debug!("local_addr: {:?}", local_addr);

    let device = Device {
        ip: local_addr.ip().to_string(),
        alias: args.alias.clone().unwrap_or(device::alias()),
        fingerprint: device::fingerprint(),
        version: PROTOCOL_VERSION_2.to_string(),
        device_model: Some(device::device_model()),
        device_type: localsend_proto::DeviceType::Headless,
        download: false,
        https: false,
        port: args.http_port,
    };

    let (server_tx, mut server_rx) = tokio::sync::mpsc::channel(1);
    let (client_tx, client_rx) = tokio::sync::mpsc::channel(1);
    let mut state = ServerState::new(server_tx, client_rx);
    {
        let mut settings = Settings::default();
        if let SubCommand::Receive(args) = &args.cmd {
            settings.destination = args.destination.clone();
            settings.quick_save = args.quick_save;
        };
        state.settings = settings;
    }
    let shared_state = Arc::new(tokio::sync::Mutex::new(state));
    let server_state = shared_state.clone();
    tokio::spawn(async move {
        start_api_server(args.http_port, server_state)
            .await
            .expect("Failed to start api server")
    });

    let mut send_files = SendingFiles::default();

    if let SubCommand::Send(args) = &args.cmd {
        for text in args.input.iter().unique().collect_vec() {
            if let Ok(path) = std::fs::canonicalize(text) {
                if path.is_file() {
                    send_files.add_file(path, None)?;
                    continue;
                } else if path.is_dir() {
                    send_files.add_dir(path)?;
                    continue;
                }
            }
            send_files.add_text(text, text.len() < 1024);
        }
    }

    let (running_tx, mut running_rx) = tokio::sync::mpsc::channel(1);
    if let Ok(_) = ctrlc::set_handler(move || running_tx.blocking_send(false).unwrap()) {
        let state = shared_state.clone();
        tokio::spawn(async move {
            running_rx.recv().await;

            let mut state = state.lock().await;
            if let Some(session) = state.send_session.take() {
                session
                    .cancel_by_sender()
                    .await
                    .expect("Failed to cancel task");
            }
            std::process::exit(0)
        });
    }

    let scanner =
        MulticastDeviceScanner::new(&device, args.multiaddr, args.port, args.http_port).await?;
    let scanner = Arc::new(scanner);
    let mut ui = PromptUI::default();
    ui.use_nerd_fonts = !args.no_nerd;

    if args.is_receive_mode() {
        let scanner = scanner.clone();
        tokio::spawn(async move {
            loop {
                for ms in vec![100, 500, 2000] {
                    scanner.send_announcement().await;
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                }
            }
        });

        if let SubCommand::Receive(args) = args.cmd {
            if args.quick_save {
                std::future::pending::<()>().await
            }
        }

        let message = ui
            .show_loading("Waiting".to_string(), async move { server_rx.recv().await })
            .await;
        match message {
            Some(ServerMessage::SelectedFiles(files)) => {
                let (progress_tx, mut progress_rx) =
                    tokio::sync::mpsc::channel::<UploadProgress>(100);

                let files = match ui.select_files(files) {
                    Some(files) => files,
                    None => {
                        client_tx.send(ClientMessage::Declined).await.unwrap();
                        return Ok(());
                    }
                };
                let pb_files = files
                    .iter()
                    .map(|file| (file.id.clone(), file.clone()))
                    .collect();

                client_tx
                    .send(ClientMessage::FilesSelected(progress_tx, files))
                    .await
                    .unwrap();

                let mut pb = FileProgressBar::new(pb_files, !args.no_nerd);
                while let Some(progress) = progress_rx.recv().await {
                    pb.update(progress);
                }
            }
            _ => return Ok(()),
        }

        std::process::exit(0)
    }

    let run = || async {
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<UploadProgress>(100);
        let mut pb = FileProgressBar::new(send_files.to_dto_map(), !args.no_nerd);
        tokio::spawn(async move {
            while let Some(progress) = progress_rx.recv().await {
                pb.update(progress);
            }
        });

        ui.print_files(&send_files);

        let target = ui.select_device(&scanner).await?;
        let session = SendSession::new(&device, target, &send_files);

        session
            .upload(shared_state.clone(), progress_tx.clone())
            .await?;
        localsend_lib::Result::<()>::Ok(())
    };

    loop {
        match run().await {
            Ok(_) => {}
            Err(localsend_lib::Error::Send(SendError::NothingSelected)) => {}
            Err(e) => {
                ui.print_error(&e);
            }
        }
        println!();
        if !ui.ask_continue() {
            break;
        }
    }

    Ok(())
}
