use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use linked_hash_map::LinkedHashMap;
use localsend_proto::dto::{FileDto, FileType};
use uuid::Uuid;

use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    Queue,
    Skipped,
    Sending,
    Failed,
    Finished,
}

#[derive(Debug, Clone)]
pub struct SendingFile {
    pub index: usize,
    pub file: FileDto,
    pub status: FileStatus,
    pub path: Option<PathBuf>,
    pub token: Option<String>,
}

impl SendingFile {
    pub fn new(index: usize, file: FileDto, path: Option<PathBuf>) -> Self {
        Self {
            index,
            file,
            status: FileStatus::Queue,
            path,
            token: None,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SendingFiles {
    pub files: LinkedHashMap<String, SendingFile>,
}

impl SendingFiles {
    pub fn get(&self, file_id: &String) -> Option<&SendingFile> {
        self.files.get(file_id)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn add_text(&mut self, text: impl ToString, preview: bool) {
        let text = text.to_string();
        let id = Uuid::new_v4().to_string();
        let text_hash = format!("{:x}", md5::compute(&text));
        let file = FileDto {
            id: id.clone(),
            file_name: format!("{}.txt", text_hash),
            size: text.len() as u64,
            file_type: localsend_proto::dto::FileType::Text,
            hash: Some(text_hash),
            preview: if preview { Some(text) } else { None },
        };
        self.files
            .insert(id.clone(), SendingFile::new(self.files.len(), file, None));
    }

    pub fn add_dir(&mut self, path: impl AsRef<Path>) -> Result<()> {
        use super::SendError;

        let base = path.as_ref().parent().ok_or(SendError::NoPermission)?;

        for entry in walkdir::WalkDir::new(&path) {
            let entry = entry?;
            let entry_path = entry.path();
            if !entry_path.is_file() {
                continue;
            }

            let diff_path =
                pathdiff::diff_paths(entry_path, &base).ok_or(SendError::NoPermission)?;
            let file_name = match diff_path.to_str() {
                Some(name) => name.replace("\\", "/"),
                None => {
                    log::error!("ignore file: {:?}", entry_path);
                    continue;
                }
            };

            log::debug!("add file {}", file_name);
            self.add_file(entry_path, Some(file_name))?;
        }

        Ok(())
    }

    pub fn add_file(&mut self, path: impl AsRef<Path>, file_name: Option<String>) -> Result<()> {
        fn get_file_name(path: &Path) -> Option<String> {
            Some(path.file_name()?.to_str()?.to_string())
        }

        fn file_type(file_name: &String) -> FileType {
            use mime_guess::mime::*;

            let mime = mime_guess::from_path(file_name).first_or_octet_stream();
            match (mime.type_(), mime.subtype()) {
                (IMAGE, _) => FileType::Image,
                (VIDEO, _) => FileType::Video,
                (APPLICATION, PDF) => FileType::Pdf,
                (TEXT, _) => FileType::Text,
                (APPLICATION, name) if name.as_str() == "vnd.android.package-archive" => {
                    FileType::Apk
                }
                _ => FileType::Other,
            }
        }

        let path = path.as_ref();

        let id = Uuid::new_v4().to_string();
        let size = std::fs::metadata(path)?.len();
        let file_name = file_name.unwrap_or(get_file_name(path).unwrap_or(id.clone()));
        let file_type = file_type(&file_name);

        let file = FileDto {
            id: id.clone(),
            file_name,
            size,
            file_type,
            hash: None,
            preview: None,
        };
        self.files.insert(
            id.clone(),
            SendingFile::new(self.files.len(), file, Some(path.to_path_buf())),
        );
        Ok(())
    }

    pub fn update_token(&mut self, token: HashMap<String, String>) {
        for (file_id, file) in &mut self.files {
            match token.get(file_id) {
                Some(token) => {
                    file.status = FileStatus::Sending;
                    file.token = Some(token.clone());
                }
                None => {
                    file.status = FileStatus::Skipped;
                }
            }
        }
    }

    pub fn to_finish_status(&mut self, file_id: String, success: bool) {
        self.files.get_mut(&file_id).map(|file| {
            if success {
                file.status = FileStatus::Finished;
            } else {
                file.status = FileStatus::Failed;
            }
        });
    }

    pub fn to_dto_map(&self) -> HashMap<String, FileDto> {
        self.files
            .iter()
            .map(|(id, file)| (id.clone(), file.file.clone()))
            .collect()
    }
}
