use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
};

use axum::{routing::post, Router};
use localsend_proto::{dto::FileDto, ApiRoute};
use tokio::{
    net::TcpListener,
    sync::{
        mpsc::{Receiver, Sender},
        Mutex,
    },
};

use crate::send::{SendSession, UploadProgress};
use crate::{receive::ReceiveSession, Settings};

use self::controller::*;

mod controller;
mod error;

pub type MutexServerState = Arc<Mutex<ServerState>>;

#[derive(Clone, Debug)]
pub enum ClientMessage {
    FilesSelected(Sender<UploadProgress>, Vec<FileDto>),
    Declined,
}

#[derive(Clone, Debug)]
pub enum ServerMessage {
    SelectedFiles(Vec<FileDto>),
}

pub struct ServerState {
    pub settings: Settings,
    pub server_tx: Sender<ServerMessage>,
    pub client_rx: Receiver<ClientMessage>,
    pub receive_session: Option<ReceiveSession>,
    pub send_session: Option<SendSession>,
}

impl ServerState {
    pub fn new(server_tx: Sender<ServerMessage>, client_rx: Receiver<ClientMessage>) -> Self {
        Self {
            settings: Settings::default(),
            server_tx,
            client_rx,
            receive_session: None,
            send_session: None,
        }
    }
}

pub async fn start_api_server(port: u16, state: MutexServerState) -> std::io::Result<()> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)).await?;
    axum::serve(
        listener,
        Router::new()
            .route(&ApiRoute::PrepareUpload.v1(), post(prepare_upload_v1))
            .route(&ApiRoute::PrepareUpload.v2(), post(prepare_upload_v2))
            .route(&ApiRoute::Upload.v1(), post(upload_v1))
            .route(&ApiRoute::Upload.v2(), post(upload_v2))
            .route(&ApiRoute::Cancel.v1(), post(cancel_v1))
            .route(&ApiRoute::Cancel.v2(), post(cancel_v2))
            .with_state(state)
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Failed to start api server");
    Ok(())
}
