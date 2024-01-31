use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Mobile,
    Desktop,
    Web,
    Headless,
    Server,
}

impl Default for DeviceType {
    fn default() -> Self {
        Self::Desktop
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Device {
    pub ip: String,
    pub version: String,
    pub port: u16,
    pub https: bool,
    pub fingerprint: String,
    pub alias: String,
    pub device_model: Option<String>,
    pub device_type: DeviceType,
    pub download: bool,
}
