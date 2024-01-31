use std::{ffi::OsString, net::SocketAddr};

use uuid::Uuid;

use crate::Result;

pub fn alias() -> String {
    if let Ok(Ok(name)) = hostname::get().map(OsString::into_string) {
        if !name.is_empty() {
            return name;
        }
    }
    "Desktop CLI".to_string()
}

#[cfg(target_os = "windows")]
pub fn device_model() -> String {
    "Windows".to_string()
}

#[cfg(target_os = "linux")]
pub fn device_model() -> String {
    "Linux".to_string()
}

#[cfg(target_os = "macos")]
pub fn device_model() -> String {
    "macOS".to_string()
}

pub fn fingerprint() -> String {
    if let Some(uid) = std::option_env!("LOCALSEND_FINGERPRINT") {
        if !uid.is_empty() {
            return uid.to_string();
        }
    }
    Uuid::new_v4().to_string()
}

pub fn local_addr() -> Result<SocketAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;
    Ok(socket.local_addr()?)
}
