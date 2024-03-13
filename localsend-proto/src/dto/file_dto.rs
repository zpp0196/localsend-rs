use std::str::FromStr;

use mime_guess::Mime;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Image,
    Video,
    Pdf,
    Text,
    Apk,
    Other,
}

impl Default for FileType {
    fn default() -> Self {
        FileType::Other
    }
}

impl From<Mime> for FileType {
    fn from(mime: Mime) -> Self {
        use mime_guess::mime::*;

        match (mime.type_(), mime.subtype()) {
            (IMAGE, _) => FileType::Image,
            (VIDEO, _) => FileType::Video,
            (APPLICATION, PDF) => FileType::Pdf,
            (TEXT, _) => FileType::Text,
            (APPLICATION, name) if name.as_str() == "vnd.android.package-archive" => FileType::Apk,
            _ => FileType::Other,
        }
    }
}

impl<'de> Deserialize<'de> for FileType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let file_type = mime_guess::Mime::from_str(&String::deserialize(deserializer)?)
            .map(Self::from)
            .unwrap_or_default();
        Ok(file_type)
    }
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
