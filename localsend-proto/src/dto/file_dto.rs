use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Image,
    Video,
    Pdf,
    Text,
    Apk,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDto {
    pub id: String, // unique inside session
    pub file_name: String,
    pub size: u64,
    pub file_type: FileType,
    pub hash: Option<String>,
    pub preview: Option<String>,
}
