use crate::{Device, PROTOCOL_VERSION_1, PROTOCOL_VERSION_2};

pub enum ApiRoute {
    PrepareUpload,
    Upload,
    Cancel,
}

impl ApiRoute {
    pub fn v1(&self) -> String {
        self.route(PROTOCOL_VERSION_1)
    }

    pub fn v2(&self) -> String {
        self.route(PROTOCOL_VERSION_2)
    }

    fn _v1(&self) -> &'static str {
        match self {
            ApiRoute::PrepareUpload => "send-request",
            ApiRoute::Upload => "send",
            ApiRoute::Cancel => "cancel",
        }
    }

    fn _v2(&self) -> &'static str {
        match self {
            ApiRoute::PrepareUpload => "prepare-upload",
            ApiRoute::Upload => "upload",
            _ => self._v1(),
        }
    }

    fn route(&self, version: impl AsRef<str>) -> String {
        let path = if version.as_ref() == PROTOCOL_VERSION_1 {
            format!("/v1/{}", self._v1())
        } else {
            format!("/v2/{}", self._v2())
        };
        format!("/api/localsend{}", path)
    }

    pub fn target(&self, device: &Device) -> String {
        self.target_raw(&device.ip, device.port, device.https, &device.version)
    }

    pub fn target_raw(
        &self,
        ip: impl AsRef<str>,
        port: u16,
        https: bool,
        version: impl AsRef<str>,
    ) -> String {
        let protocol = if https { "https" } else { "http" };
        let route = self.route(&version);
        format!("{}://{}:{}{}", protocol, ip.as_ref(), port, route)
    }
}
