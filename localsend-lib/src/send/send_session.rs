use std::cmp::min;

use futures_util::StreamExt;
use localsend_proto::{
    dto::{FileType, PrepareUploadRequestDto, PrepareUploadResponseDto, RegisterDto},
    ApiRoute, Device, PROTOCOL_VERSION_1,
};
use once_cell::sync::Lazy;
use reqwest::{header, Body, Client, StatusCode};
use thiserror::Error;
use tokio::{
    fs::File,
    sync::mpsc::Sender,
    task::{AbortHandle, JoinError},
};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::{send::FileStatus, server::MutexServerState, Result};

use super::{SendingFile, SendingFiles};

static CLIENT: Lazy<Client> = Lazy::new(|| {
    reqwest::ClientBuilder::new()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("Failed to create  reqwest client")
});

#[derive(Error, Debug)]
pub enum SendError {
    #[error("Nothing selected")]
    NothingSelected,
    #[error("The recipient has rejected the request")]
    Rejected,
    #[error("The recipient is busy with another request")]
    Busy,
    #[error("Cancelled by receiver")]
    Cancelled,
    #[error("No permission")]
    NoPermission,
    #[error(transparent)]
    Aborted(JoinError),
    #[error("Unknown response status code: {0}")]
    Unknown(StatusCode),
}

#[derive(Debug)]
pub struct UploadProgress {
    pub file_id: String,
    pub position: u64,
    pub finish: bool,
}

#[derive(Debug)]
pub struct SendSession {
    pub session_id: String,
    info: RegisterDto,
    target: Device,
    files: SendingFiles,
    pub remote_session_id: Option<String>, // v1 nullable
    cancel_token: Option<AbortHandle>,
}

impl SendSession {
    pub fn new(device: &Device, target: Device, files: &SendingFiles) -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            info: device.clone().into(),
            target,
            files: files.clone(),
            remote_session_id: None,
            cancel_token: None,
        }
    }

    pub async fn upload(
        mut self,
        state: MutexServerState,
        progress_tx: Sender<UploadProgress>,
    ) -> Result<()> {
        let files = self.files.to_dto_map();
        let request_dto = PrepareUploadRequestDto {
            info: self.info.clone(),
            files,
        };
        let response = CLIENT
            .post(ApiRoute::PrepareUpload.target(&self.target))
            .json(&request_dto)
            .send()
            .await?;
        match response.status() {
            // 200
            StatusCode::OK => {}
            // 204
            StatusCode::NO_CONTENT => {
                return Err(SendError::NothingSelected.into());
            }
            // 403
            StatusCode::FORBIDDEN => {
                return Err(SendError::Rejected.into());
            }
            // 409
            StatusCode::CONFLICT => {
                return Err(SendError::Busy.into());
            }
            _ => {
                return Err(SendError::Unknown(response.status()).into());
            }
        }

        let file_token = if self.target.version == PROTOCOL_VERSION_1 {
            response.json().await?
        } else {
            let response_dto = response.json::<PrepareUploadResponseDto>().await?;
            self.remote_session_id = Some(response_dto.session_id);
            response_dto.files
        };
        if file_token.is_empty() {
            return Err(SendError::NothingSelected.into());
        }

        self.files.update_token(file_token);

        let join_handle = {
            let remote_session_id = self.remote_session_id.clone();
            let target = self.target.clone();
            let files = self.files.clone();
            let new_state = state.clone();

            let handle = tokio::spawn(async move {
                for (file_id, file) in files.files {
                    if file.status == FileStatus::Skipped {
                        continue;
                    }

                    let send_result =
                        Self::upload_file(&remote_session_id, &file, &target, progress_tx.clone())
                            .await;
                    if let Err(e) = &send_result {
                        log::error!("Failed to upload file {}: {}", file_id, e);
                    }

                    let mut state = new_state.lock().await;
                    if let Some(session) = &mut state.send_session {
                        session.files.to_finish_status(file_id, send_result.is_ok());
                    }
                }
            });
            self.cancel_token = Some(handle.abort_handle());

            let mut state = state.lock().await;
            state.send_session = Some(self);
            handle
        };

        let result = join_handle.await;
        {
            let mut state = state.lock().await;
            state.send_session.take();
        }
        if let Err(join_error) = result {
            if join_error.is_cancelled() {
                return Err(SendError::Cancelled.into());
            } else {
                return Err(SendError::Aborted(join_error).into());
            }
        }

        Ok(())
    }

    async fn upload_file(
        remote_session_id: &Option<String>,
        sending_file: &SendingFile,
        target: &Device,
        progress_tx: Sender<UploadProgress>,
    ) -> Result<()> {
        let file = &sending_file.file;
        let file_size = file.size;

        let body;
        match &sending_file.path {
            Some(path) => {
                let file_id = file.id.clone();
                let file = File::open(path).await?;
                let mut reader_stream = ReaderStream::new(file);
                let mut uploaded = 0;

                let async_stream = async_stream::stream! {
                    while let Some(chunk) = reader_stream.next().await {
                        if let Ok(chunk) = &chunk {
                            let pos = min(uploaded + (chunk.len() as u64), file_size);
                            uploaded = pos;
                            let progress = UploadProgress{
                                file_id: file_id.clone(),
                                position: pos,
                                finish: pos >= file_size,
                            };
                            progress_tx.send(progress).await.ok();
                        }
                        yield chunk;
                    }
                };
                body = Body::wrap_stream(async_stream);
            }
            None => {
                if file.file_type == FileType::Text && file.preview.is_some() {
                    let bytes = file.preview.as_ref().unwrap().as_bytes().to_vec();
                    body = Body::from(bytes);
                } else {
                    unimplemented!();
                }
            }
        }

        let content_type = mime_guess::from_path(&file.file_name)
            .first_or_octet_stream()
            .to_string();

        let v2_args = if let Some(session_id) = remote_session_id {
            format!("&sessionId={}", session_id,)
        } else {
            String::default()
        };
        let url = format!(
            "{}?fileId={}&token={}{}",
            ApiRoute::Upload.target(&target),
            file.id,
            sending_file.token.as_ref().expect("No file token"),
            v2_args,
        );
        let response = CLIENT
            .post(url)
            .header(header::CONTENT_LENGTH, file_size)
            .header(header::CONTENT_TYPE, content_type)
            .body(body)
            .send()
            .await?;
        match response.status() {
            StatusCode::OK => Ok(()),
            _ => Err(SendError::Unknown(response.status()).into()),
        }
    }

    pub async fn cancel_by_receiver(self) -> Result<()> {
        self.cancel(false).await
    }

    pub async fn cancel_by_sender(self) -> Result<()> {
        self.cancel(true).await
    }

    pub async fn cancel(self, from_sender: bool) -> Result<()> {
        let cancel_token = self.cancel_token.ok_or(SendError::NoPermission)?;
        let cancel_result = if from_sender {
            let v2_args = if let Some(session_id) = &self.remote_session_id {
                format!("?sessionId={}", session_id,)
            } else {
                String::default()
            };
            let url = format!("{}{}", ApiRoute::Cancel.target(&self.target), v2_args,);
            let status_code = CLIENT.post(url).send().await.map(|r| r.status());
            match status_code {
                // 200
                Ok(StatusCode::OK) => Ok(()),
                // 403
                Ok(StatusCode::FORBIDDEN) => Err(SendError::NoPermission),
                _ => Err(SendError::Unknown(status_code?)),
            }
        } else {
            Ok(())
        };
        cancel_token.abort();
        cancel_result?;
        log::info!(
            "{} cancelled, remote session_id: {:?}",
            self.session_id,
            self.remote_session_id
        );
        Ok(())
    }
}
