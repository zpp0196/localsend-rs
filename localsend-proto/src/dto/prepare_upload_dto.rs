use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{FileDto, RegisterDto};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareUploadRequestDto {
    pub info: RegisterDto,
    pub files: HashMap<String, FileDto>,
}

/// v2
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareUploadResponseDto {
    pub session_id: String,
    pub files: HashMap<String, String>,
}
