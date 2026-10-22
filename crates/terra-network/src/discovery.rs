//! One ephemeral socket per browser, so multiple clients can discover concurrently.
use crate::browser::{
    Announcement, DISCOVERY_GROUP, DISCOVERY_PORT, MAGIC, Probe, ServerStatus, private_address,
};
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct LanServer {
    pub address: SocketAddr,
    pub status: ServerStatus,
    pub seen: Instant,
    pub ping_ms: u32,
}
pub struct LanScanner {
    socket: UdpSocket,
    nonce: u64,
    last_probe: Option<Instant>,
    pub servers: HashMap<SocketAddr, LanServer>,
}
impl LanScanner {
    pub fn new() -> Result<Self, String> {
        let socket =
            UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0)).map_err(|e| e.to_string())?;
        socket.set_nonblocking(true).map_err(|e| e.to_string())?;
        socket
            .set_multicast_loop_v4(true)
            .map_err(|e| e.to_string())?;
        socket.set_multicast_ttl_v4(1).map_err(|e| e.to_string())?;
        Ok(Self {
            socket,
            nonce: terra_core::PlayerUuid::new_v4().as_u128() as u64,
            last_probe: None,
            servers: HashMap::new(),
        })
    }
    pub fn poll(&mut self) -> Result<(), String> {
        if self
            .last_probe
            .is_none_or(|time| time.elapsed() >= Duration::from_secs(2))
        {
            self.nonce = self.nonce.wrapping_add(1);
            let probe = serde_json::to_vec(&Probe {
                magic: MAGIC.into(),
                nonce: self.nonce,
            })
            .map_err(|e| e.to_string())?;
            // Loopback detects servers intentionally bound only to this machine.
            self.socket
                .send_to(&probe, (std::net::Ipv4Addr::LOCALHOST, DISCOVERY_PORT))
                .map_err(|e| e.to_string())?;
            self.last_probe = Some(Instant::now());
            let interface = socket2::SockRef::from(&self.socket);
            if interface
                .set_multicast_if_v4(&std::net::Ipv4Addr::LOCALHOST)
                .is_ok()
            {
                let _ = self
                    .socket
                    .send_to(&probe, (DISCOVERY_GROUP, DISCOVERY_PORT));
            }
            let _ = interface.set_multicast_if_v4(&std::net::Ipv4Addr::UNSPECIFIED);
            let _ = self
                .socket
                .send_to(&probe, (DISCOVERY_GROUP, DISCOVERY_PORT));
        }
        let mut buffer = [0_u8; 2048];
        for _ in 0..128 {
            match self.socket.recv_from(&mut buffer) {
                Ok((size, source)) if private_address(source.ip()) => {
                    let Ok(reply) = serde_json::from_slice::<Announcement>(&buffer[..size]) else {
                        continue;
                    };
                    if reply.magic != MAGIC
                        || reply.nonce != self.nonce
                        || reply.status.validate().is_err()
                    {
                        continue;
                    }
                    let address = SocketAddr::new(source.ip(), reply.status.port);
                    let old = self.servers.iter().find_map(|(&address, server)| {
                        (server.status.instance == reply.status.instance).then_some(address)
                    });
                    if let Some(old) = old {
                        if old.ip().is_loopback() && !address.ip().is_loopback() {
                            continue;
                        }
                        if old != address {
                            self.servers.remove(&old);
                        }
                    }
                    if self.servers.len() < 64 || self.servers.contains_key(&address) {
                        self.servers.insert(
                            address,
                            LanServer {
                                address,
                                status: reply.status,
                                seen: Instant::now(),
                                ping_ms: self.last_probe.map_or(0, |time| {
                                    time.elapsed().as_millis().min(u128::from(u32::MAX)) as u32
                                }),
                            },
                        );
                    }
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.to_string()),
            }
        }
        self.servers
            .retain(|_, server| server.seen.elapsed() < Duration::from_secs(7));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanner_rejects_foreign_and_malformed_announcements_and_expires_stale_rows() {
        let mut scanner = LanScanner::new().unwrap();
        scanner.last_probe = Some(Instant::now());
        let destination =
            SocketAddr::from(([127, 0, 0, 1], scanner.socket.local_addr().unwrap().port()));
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let status = ServerStatus {
            instance: terra_core::PlayerUuid::new_v4(),
            protocol: crate::protocol::session::PROTOCOL_VERSION,
            title: "Local test".into(),
            description: String::new(),
            players: 0,
            max_players: 8,
            port: 5000,
            identity_mode: "local_profile".into(),
        };
        sender.send_to(b"not json", destination).unwrap();
        sender
            .send_to(
                &serde_json::to_vec(&Announcement {
                    magic: MAGIC.into(),
                    nonce: scanner.nonce.wrapping_add(1),
                    status: status.clone(),
                })
                .unwrap(),
                destination,
            )
            .unwrap();
        scanner.poll().unwrap();
        assert!(scanner.servers.is_empty());
        sender
            .send_to(
                &serde_json::to_vec(&Announcement {
                    magic: MAGIC.into(),
                    nonce: scanner.nonce,
                    status,
                })
                .unwrap(),
                destination,
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while scanner.servers.is_empty() {
            scanner.poll().unwrap();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        scanner
            .servers
            .values_mut()
            .for_each(|row| row.seen = Instant::now() - Duration::from_secs(8));
        scanner.poll().unwrap();
        assert!(scanner.servers.is_empty());
    }
}
