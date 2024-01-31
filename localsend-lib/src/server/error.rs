use axum::{http::StatusCode, response::IntoResponse};

use crate::{error::Error, send::SendError};

impl Error {
    fn status_code(&self) -> StatusCode {
        match self {
            Error::Send(e) => e.status_code(),
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl SendError {
    fn status_code(&self) -> StatusCode {
        match self {
            SendError::NoPermission => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::Send(e) => e.into_response(),
            _ => (self.status_code(), "Internal server error").into_response(),
        }
    }
}

impl IntoResponse for SendError {
    fn into_response(self) -> axum::response::Response {
        (self.status_code(), self.to_string()).into_response()
    }
}
