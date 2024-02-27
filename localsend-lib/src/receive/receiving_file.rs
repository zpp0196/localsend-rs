use localsend_proto::dto::FileDto;

use crate::send::FileStatus;

#[derive(Debug, Clone)]
pub struct ReceivingFile {
    pub file: FileDto,
    pub status: FileStatus,
    pub token: Option<String>,
}
