use axum::{http::StatusCode, response::IntoResponse};

use crate::{error::Error, receive::ReceiveError, send::SendError};

impl Error {
    fn status_code(&self) -> StatusCode {
        match self {
            Error::Receive(e) => e.into(),
            Error::Send(e) => e.into(),
            _ => StatusCode::INTERNAL_SERVER_ERROR, // 500
        }
    }
}

impl From<&ReceiveError> for StatusCode {
    fn from(value: &ReceiveError) -> Self {
        match value {
            ReceiveError::Cancelled => StatusCode::OK,           // 200
            ReceiveError::EmptyFiles => StatusCode::BAD_REQUEST, // 400
            ReceiveError::InvalidIp(_) => StatusCode::FORBIDDEN, // 403
            ReceiveError::InvalidParameters => StatusCode::BAD_REQUEST, // 400
            ReceiveError::InvalidRecipient => StatusCode::CONFLICT, // 409
            ReceiveError::InvalidServerState => StatusCode::INTERNAL_SERVER_ERROR, // 500
            ReceiveError::InvalidSessionId => StatusCode::FORBIDDEN, // 403
            ReceiveError::InvalidToken => StatusCode::FORBIDDEN, // 403
            ReceiveError::NothingSelected => StatusCode::NO_CONTENT, // 204
            ReceiveError::SaveFileFailed => StatusCode::INTERNAL_SERVER_ERROR, // 500
            ReceiveError::SessionBlocked => StatusCode::CONFLICT, // 409
            ReceiveError::SessionDeclined => StatusCode::FORBIDDEN, // 403
            ReceiveError::SessionNotExists => StatusCode::CONFLICT, // 409
        }
    }
}

impl From<&SendError> for StatusCode {
    fn from(value: &SendError) -> Self {
        match value {
            SendError::NoPermission => StatusCode::FORBIDDEN, // 403
            _ => StatusCode::INTERNAL_SERVER_ERROR,           // 500
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let status_code = self.status_code();
        let message = if let StatusCode::INTERNAL_SERVER_ERROR = status_code {
            "Internal server error".to_owned()
        } else {
            self.to_string()
        };
        (status_code, message).into_response()
    }
}
