use std::{net::Ipv4Addr, sync::Arc};

use clap::Parser;
use itertools::Itertools;
use localsend_lib::{
    scanner::MulticastDeviceScanner,
    send::{SendError, SendSession, SendingFiles, UploadProgress},
    server::{start_api_server, ServerState},
    util::device,
    Result,
};
use localsend_proto::{Device, DEFAULT_MULTICAST, DEFAULT_PORT, PROTOCOL_VERSION_2};
use simple_logger::SimpleLogger;

use crate::ui::{InteractiveUI, PromptUI, UploadFileProgressBar};

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

    /// Text or file path to be sent
    #[clap(required = true)]
    input: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .env()
        .init()
        .expect("Failed to init logger");

    let args = Args::parse();

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
        port: local_addr.port(),
    };

    let shared_state = Arc::new(tokio::sync::Mutex::new(ServerState::default()));
    let server_state = shared_state.clone();
    tokio::spawn(async move {
        start_api_server(local_addr.port(), server_state)
            .await
            .expect("Failed to start api server")
    });

    let mut send_files = SendingFiles::default();

    for text in &args.input.iter().unique().collect_vec() {
        if let Ok(path) = std::fs::canonicalize(text) {
            if path.is_file() {
                send_files.add_file(path)?;
                continue;
            }
        }
        send_files.add_text(text, text.len() < 1024);
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

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<UploadProgress>(100);
    let mut pb = UploadFileProgressBar::new(&send_files);
    tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            pb.update(progress).await;
        }
    });

    let scanner = MulticastDeviceScanner::new(&device, args.multiaddr, args.port).await?;
    let ui = PromptUI::default();

    let run = || async {
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
