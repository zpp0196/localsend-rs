use serde::{Deserialize, Serialize};

use crate::{Device, DeviceType, FALLBACK_PROTOCOL_VERSION};

use super::ProtocolType;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterDto {
    pub alias: String,
    pub version: Option<String>, // v2, format: major.minor
    pub device_model: Option<String>,
    pub device_type: Option<DeviceType>,
    pub fingerprint: String,
    pub port: Option<u16>,              // v2
    pub protocol: Option<ProtocolType>, // v2
    pub download: Option<bool>,         // v2
}

impl From<Device> for RegisterDto {
    fn from(value: Device) -> Self {
        Self {
            alias: value.alias,
            version: Some(value.version),
            device_model: value.device_model,
            device_type: Some(value.device_type),
            fingerprint: value.fingerprint,
            port: Some(value.port),
            protocol: Some(if value.https {
                ProtocolType::Https
            } else {
                ProtocolType::Http
            }),
            download: Some(value.download),
        }
    }
}

impl RegisterDto {
    pub fn to_device(self, ip: impl ToString, own_port: u16, own_https: bool) -> Device {
        Device {
            ip: ip.to_string(),
            version: self.version.unwrap_or(FALLBACK_PROTOCOL_VERSION.to_owned()),
            port: self.port.unwrap_or(own_port),
            https: self
                .protocol
                .map(|p| p == ProtocolType::Https)
                .unwrap_or(own_https),
            fingerprint: self.fingerprint,
            alias: self.alias,
            device_model: self.device_model,
            device_type: self.device_type.unwrap_or_default(),
            download: self.download.unwrap_or(false),
        }
    }
}
