use std::{
    net::{Ipv4Addr, SocketAddr},
    time::{Duration, Instant},
};

use localsend_proto::{
    dto::{MulticastDto, RegisterDto},
    Device, DeviceType,
};
use tokio::net::UdpSocket;

#[derive(Debug)]
pub struct MulticastDeviceScanner {
    socket: UdpSocket,
    device: MulticastDto,
    addr: SocketAddr,
    announce_msg: String,
}

impl MulticastDeviceScanner {
    pub async fn new(device: &Device, multiaddr: Ipv4Addr, port: u16, http_port: u16) -> std::io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).await?;
        socket.join_multicast_v4(multiaddr, Ipv4Addr::UNSPECIFIED)?;

        let device = MulticastDto::v2(
            device.alias.clone(),
            device.device_model.clone(),
            DeviceType::Headless,
            device.fingerprint.clone(),
            http_port,
            true,
        );
        let announce_msg = serde_json::to_string(&device)?;

        Ok(Self {
            socket,
            device,
            addr: (multiaddr, port).into(),
            announce_msg,
        })
    }
}

impl MulticastDeviceScanner {
    pub async fn send_announcement(&self) {
        let size = self
            .socket
            .send_to(self.announce_msg.as_bytes(), self.addr)
            .await
            .ok();
        assert!(size == Some(self.announce_msg.len()));
    }

    pub async fn scan(&self) -> std::io::Result<Vec<Device>> {
        let mut devices = vec![];
        let mut buf = [0u8; 2048 as usize];

        self.send_announcement().await;

        let instant = Instant::now();
        while instant.elapsed() < Duration::from_secs(2) || devices.is_empty() {
            if let Ok((size, addr)) = self.socket.try_recv_from(&mut buf) {
                let register_dto: RegisterDto = serde_json::from_slice(&buf[..size])?;
                if register_dto.fingerprint == self.device.fingerprint {
                    continue;
                }

                let device = register_dto.to_device(addr.ip().to_string(), addr.port(), false);
                if !devices.contains(&device) {
                    log::trace!("found device: {:?}", device);
                    devices.push(device);
                }
            } else {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        return Ok(devices);
    }
}
