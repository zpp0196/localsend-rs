use std::path::PathBuf;

#[derive(Debug)]
pub struct Settings {
    pub destination: PathBuf,
    pub quick_save: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            destination: PathBuf::from("."),
            quick_save: false,
        }
    }
}
