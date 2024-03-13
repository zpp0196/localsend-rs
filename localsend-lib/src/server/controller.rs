use std::{collections::HashMap, io, net::SocketAddr};

use axum::{
    body::Body,
    extract::{ConnectInfo, Query, State},
    Json,
};
use futures_util::{pin_mut, TryStreamExt};
use localsend_proto::{
    dto::{PrepareUploadRequestDto, PrepareUploadResponseDto},
    DEFAULT_PORT,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufWriter},
};
use tokio_util::io::StreamReader;

use super::MutexServerState;

use crate::{
    receive::{ReceiveError, ReceiveSession, ReceiveSessionStatus, ReceivingFile},
    send::{FileStatus, SendError, UploadProgress},
    server::ClientMessage,
    Result,
};

pub async fn cancel_v1(State(state): State<MutexServerState>) -> Result<()> {
    let mut state = state.lock().await;
    let session = state.send_session.take().ok_or(SendError::NoPermission)?;
    session.cancel_by_receiver().await?;
    Ok(())
}

pub async fn cancel_v2(
    Query(query): Query<HashMap<String, String>>,
    State(state): State<MutexServerState>,
) -> Result<()> {
    let remote_session_id = query.get("sessionId").ok_or(SendError::NoPermission)?;
    log::debug!("remote sessionId: {}", remote_session_id);
    let mut state = state.lock().await;
    if let Some(session) = &state.send_session {
        if session.remote_session_id.as_ref() != Some(remote_session_id) {
            return Err(SendError::NoPermission)?;
        }
    }
    let session = state.send_session.take().ok_or(SendError::NoPermission)?;
    session.cancel_by_receiver().await?;

    Ok(())
}

pub async fn prepare_upload_v1(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<MutexServerState>,
    Json(dto): Json<PrepareUploadRequestDto>,
) -> Result<Json<HashMap<String, String>>> {
    let dto = prepare_upload(addr, state, dto).await?;
    Ok(dto.files.into())
}

pub async fn prepare_upload_v2(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<MutexServerState>,
    Json(dto): Json<PrepareUploadRequestDto>,
) -> Result<Json<PrepareUploadResponseDto>> {
    let dto = prepare_upload(addr, state, dto).await?;
    Ok(dto.into())
}

async fn prepare_upload(
    addr: SocketAddr,
    state: MutexServerState,
    dto: PrepareUploadRequestDto,
) -> Result<PrepareUploadResponseDto> {
    log::info!("Client Addr: {}", addr);

    let mut _state = state.try_lock().map_err(|_| ReceiveError::SessionBlocked)?;
    if _state.receive_session.is_some() {
        return Err(ReceiveError::SessionBlocked)?;
    }

    if dto.files.is_empty() {
        return Err(ReceiveError::EmptyFiles)?;
    }

    let settings = &_state.settings;
    let destination = &settings.destination;
    let quick_save = settings.quick_save;
    let session_id = uuid::Uuid::new_v4().to_string();

    log::info!("Session Id: {}", session_id);
    log::info!(
        "Destination Directory: {:?}, Quick Save: {}",
        destination,
        quick_save
    );

    let receive_session = ReceiveSession {
        session_id: session_id.clone(),
        status: ReceiveSessionStatus::Waiting,
        sender: dto
            .info
            .to_device(addr.ip().to_string(), DEFAULT_PORT, false),
        files: HashMap::new(),
        destination_directory: settings.destination.clone(),
        progress_tx: None,
    };
    _state.receive_session = Some(receive_session);

    struct Guard(MutexServerState);

    impl Drop for Guard {
        fn drop(&mut self) {
            let state = self.0.clone();
            tokio::task::spawn_blocking(move || {
                let mut state = state.blocking_lock();
                if let Some(session) = &state.receive_session {
                    if session.status == ReceiveSessionStatus::Waiting {
                        state.receive_session = None;
                    }
                }
            });
        }
    }

    let _guard = Guard(state.clone());

    let files = dto.files.into_values().collect();

    let (progress_tx, selection) = if quick_save {
        (None, Some(files))
    } else {
        let tx = _state.server_tx.clone();
        tx.send(crate::server::ServerMessage::SelectedFiles(files))
            .await
            .unwrap();

        match _state.client_rx.recv().await {
            None => return Err(ReceiveError::NothingSelected)?,
            Some(ClientMessage::FilesSelected(progress_tx, files)) => {
                (Some(progress_tx), Some(files))
            }
            Some(ClientMessage::Declined) => (None, None),
        }
    };

    let receive_session = _state
        .receive_session
        .as_mut()
        .ok_or(ReceiveError::InvalidServerState)?;
    receive_session.progress_tx = progress_tx;

    let selection = match selection {
        Some(selection) => selection,
        None => {
            _state.receive_session = None;
            return Err(ReceiveError::SessionDeclined)?;
        }
    };

    if selection.is_empty() {
        _state.receive_session = None;
        return Err(ReceiveError::NothingSelected)?;
    }

    receive_session.status = ReceiveSessionStatus::Sending;
    receive_session.files = selection
        .into_iter()
        .map(|file| {
            let token = uuid::Uuid::new_v4().to_string();
            (
                file.id.clone(),
                ReceivingFile {
                    file: file.clone(),
                    status: FileStatus::Queue,
                    token: Some(token),
                },
            )
        })
        .collect();

    let session_id = receive_session.session_id.clone();
    let files = receive_session
        .files
        .iter_mut()
        .map(|(id, file)| (id.clone(), file.token.clone().unwrap()))
        .collect();
    let dto = PrepareUploadResponseDto { session_id, files };

    Ok(dto)
}

pub async fn upload_v1(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<MutexServerState>,
    body: Body,
) -> Result<()> {
    upload(addr, query, body, state, false).await?;
    Ok(())
}

pub async fn upload_v2(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<MutexServerState>,
    body: Body,
) -> Result<()> {
    upload(addr, query, body, state, true).await?;
    Ok(())
}

async fn upload(
    addr: SocketAddr,
    query: HashMap<String, String>,
    body: Body,
    state: MutexServerState,
    v2: bool,
) -> Result<()> {
    let mut _state = state.lock().await;
    let receive_session = _state
        .receive_session
        .as_mut()
        .ok_or(ReceiveError::SessionNotExists)?;

    if addr.ip().to_string() != receive_session.sender.ip {
        log::warn!(
            "Invalid ip address: {} (expected: {})",
            addr.ip(),
            receive_session.sender.ip
        );
        return Err(ReceiveError::InvalidIp(addr.ip().to_string()))?;
    }

    if receive_session.status != ReceiveSessionStatus::Sending {
        log::warn!(
            "Wrong state: {:?} (expected: {:?})",
            receive_session.status,
            ReceiveSessionStatus::Sending,
        );
        return Err(ReceiveError::InvalidRecipient)?;
    }

    let file_id = query.get("fileId").ok_or(ReceiveError::InvalidParameters)?;
    let token = query.get("token").ok_or(ReceiveError::InvalidParameters)?;

    if v2 {
        let session_id = query
            .get("sessionId")
            .ok_or(ReceiveError::InvalidParameters)?;
        if session_id != &receive_session.session_id {
            return Err(ReceiveError::InvalidSessionId)?;
        }
    }

    let receiving_file = receive_session
        .files
        .get_mut(file_id)
        .ok_or(ReceiveError::InvalidToken)?;

    let receiving_file_token = receiving_file
        .token
        .as_ref()
        .ok_or(ReceiveError::InvalidToken)?;
    if token != receiving_file_token {
        log::warn!(
            "Wrong file token: {} (expected: {})",
            token,
            receiving_file_token
        );
        return Err(ReceiveError::InvalidToken)?;
    }

    receiving_file.status = FileStatus::Sending;
    receiving_file.token = None; // remove token to reject further uploads of the same file

    let receiving_file = receiving_file.clone();
    let destination = &receive_session.destination_directory.clone();
    log::info!(
        "Saving {} to {:?}",
        receiving_file.file.file_name,
        destination
    );

    let progress_tx = receive_session.progress_tx.clone();

    // release state lock
    drop(_state);

    let save_file = || async {
        let stream = body.into_data_stream();
        let stream = stream.map_err(|e| io::Error::new(io::ErrorKind::Other, e));
        let reader = StreamReader::new(stream);
        pin_mut!(reader);

        const BUF_SIZE: usize = 1024 * 8;
        let path = std::path::Path::new(destination).join(&receiving_file.file.file_name);
        if let Some(path) = path.parent() {
            if !path.exists() {
                tokio::fs::create_dir_all(path).await?;
            }
        }

        let file = File::create(&path).await?;
        let mut file_buf = BufWriter::with_capacity(BUF_SIZE, file);

        let mut buf = [0u8; BUF_SIZE];
        let mut position: u64 = 0;

        loop {
            match reader.read(&mut buf[..]).await {
                Ok(0) => break,
                Ok(len) => {
                    position += len as u64;
                    file_buf.write(&buf[0..len]).await.unwrap();
                    if let Some(ref progress_tx) = progress_tx {
                        progress_tx
                            .send(UploadProgress {
                                file_id: receiving_file.file.id.clone(),
                                position,
                                finish: position >= receiving_file.file.size,
                            })
                            .await
                            .ok();
                    }
                }
                Err(e) => {
                    log::warn!("Error: {:?}", e);
                    tokio::fs::remove_file(path).await.ok();
                    return Err(ReceiveError::Cancelled)?;
                }
            }
        }

        file_buf.flush().await?;

        Result::Ok(())
    };

    let save_result = save_file().await;

    let mut _state = state.lock().await;
    let receive_session = _state
        .receive_session
        .as_mut()
        .ok_or(ReceiveError::Cancelled)?;
    let receiving_file = receive_session
        .files
        .get_mut(file_id)
        .ok_or(ReceiveError::InvalidToken)?;

    let result = match save_result {
        Ok(_) => {
            log::info!("File {:?} has been saved", receiving_file.file.file_name);
            receiving_file.status = FileStatus::Finished;
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to save file: {:?}", e);
            receiving_file.status = FileStatus::Failed;
            Err(ReceiveError::SaveFileFailed.into())
        }
    };

    let finish = receive_session
        .files
        .iter()
        .all(|f| f.1.status == FileStatus::Finished || f.1.status == FileStatus::Failed);
    if finish {
        _state.receive_session = None;
    }

    result
}
