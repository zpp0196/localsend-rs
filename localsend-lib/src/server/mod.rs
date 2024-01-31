use std::{
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
};

use axum::{routing::post, Router};
use localsend_proto::ApiRoute;
use tokio::{net::TcpListener, sync::Mutex};

use crate::send::SendSession;

use self::controller::*;

mod controller;
mod error;

pub type MutexServerState = Arc<Mutex<ServerState>>;

#[derive(Default)]
pub struct ServerState {
    pub send_session: Option<SendSession>,
}

pub async fn start_api_server(port: u16, state: MutexServerState) -> std::io::Result<()> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)).await?;
    axum::serve(
        listener,
        Router::new()
            .route(&ApiRoute::Cancel.v1(), post(cancel_v1))
            .route(&ApiRoute::Cancel.v2(), post(cancel_v2))
            .with_state(state),
    )
    .await
    .expect("Failed to start api server");
    Ok(())
}
