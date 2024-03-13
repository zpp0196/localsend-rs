use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Receive(#[from] crate::receive::ReceiveError),
    #[error(transparent)]
    Send(#[from] crate::send::SendError),
    #[error(transparent)]
    WalkDir(#[from] walkdir::Error),
}
