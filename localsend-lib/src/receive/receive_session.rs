use std::{collections::HashMap, path::PathBuf};

use localsend_proto::Device;
use thiserror::Error;
use tokio::sync::mpsc::Sender;

use crate::send::UploadProgress;

use super::ReceivingFile;

#[derive(Error, Debug)]
pub enum ReceiveError {
    #[error("Request must contain at least one file")]
    EmptyFiles,
    #[error("Invalid IP address: {0}")]
    InvalidIp(String),
    #[error("Missing parameters")]
    InvalidParameters,
    #[error("Recipient is in wrong state")]
    InvalidRecipient,
    #[error("Invalid session id")]
    InvalidSessionId,
    #[error("Server is in invalid state")]
    InvalidServerState,
    #[error("Invalid token")]
    InvalidToken,
    #[error("Nothing selected")]
    NothingSelected,
    #[error("Could not save file")]
    SaveFileFailed,
    #[error("Blocked by another session")]
    SessionBlocked,
    #[error("File request declined by recipient")]
    SessionDeclined,
    #[error("No session")]
    SessionNotExists,
    #[error("Cancelled")]
    Cancelled,
}

#[derive(Debug)]
pub struct ReceiveSession {
    pub session_id: String,
    pub status: ReceiveSessionStatus,
    pub sender: Device,
    pub files: HashMap<String, ReceivingFile>,
    pub destination_directory: PathBuf,
    pub progress_tx: Option<Sender<UploadProgress>>,
}

#[derive(Debug, PartialEq)]
pub enum ReceiveSessionStatus {
    Waiting,
    Sending,
}
