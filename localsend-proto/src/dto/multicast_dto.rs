use serde::{Deserialize, Serialize};

use crate::{Device, DeviceType, FALLBACK_PROTOCOL_VERSION, PROTOCOL_VERSION_2};

use super::ProtocolType;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MulticastDto {
    pub alias: String,
    pub version: Option<String>, // v2, format: major.minor
    pub device_model: Option<String>,
    pub device_type: Option<DeviceType>, // nullable since v2
    pub fingerprint: String,
    pub port: Option<u16>,              // v2
    pub protocol: Option<ProtocolType>, // v2
    pub download: Option<bool>,         // v2
    pub announcement: Option<bool>,     // v1
    pub announce: Option<bool>,         // v2
}

impl MulticastDto {
    pub fn v1(
        alias: impl ToString,
        device_model: Option<String>,
        device_type: DeviceType,
        fingerprint: impl ToString,
        announcement: bool,
    ) -> Self {
        Self {
            alias: alias.to_string(),
            version: None,
            device_model,
            device_type: Some(device_type),
            fingerprint: fingerprint.to_string(),
            port: None,
            protocol: None,
            download: None,
            announcement: Some(announcement),
            announce: None,
        }
    }

    pub fn v2(
        alias: impl ToString,
        device_model: Option<String>,
        device_type: DeviceType,
        fingerprint: impl ToString,
        port: u16,
        announcement: bool,
    ) -> Self {
        Self {
            alias: alias.to_string(),
            version: Some(PROTOCOL_VERSION_2.to_string()),
            device_model,
            device_type: Some(device_type),
            fingerprint: fingerprint.to_string(),
            port: Some(port),
            protocol: Some(ProtocolType::Http),
            download: None,
            announcement: Some(announcement),
            announce: None,
        }
    }

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

#[cfg(test)]
mod tests {

    use crate::DeviceType;

    use super::MulticastDto;

    #[test]
    pub fn test_serde_json() {
        let dto = MulticastDto::v1(
            "Nice Orange",
            Some("Samsung".to_owned()),
            DeviceType::Mobile,
            "random string",
            true,
        );
        let dto_str = r#"{"alias":"Nice Orange","version":null,"deviceModel":"Samsung","deviceType":"mobile","fingerprint":"random string","port":null,"protocol":null,"download":null,"announcement":true,"announce":null}"#;
        assert_eq!(dto_str, serde_json::to_string(&dto).unwrap());
        let new_dto: MulticastDto = serde_json::from_str(dto_str).unwrap();
        assert_eq!(dto.alias, new_dto.alias);
        assert_eq!(dto.fingerprint, new_dto.fingerprint);
    }
}
