use std::collections::HashMap;

use axum::extract::{Query, State};

use super::MutexServerState;

use crate::{send::SendError, Result};

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
