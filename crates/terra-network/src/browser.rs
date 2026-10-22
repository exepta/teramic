//! Bounded, versioned server status and automatic LAN-profile connection setup.
use crate::protocol::session::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Write},
    net::{IpAddr, SocketAddr, TcpStream},
    time::Duration,
};
use terra_core::{Config, PlayerProfile};

pub const DISCOVERY_PORT: u16 = 54545;
pub const DISCOVERY_GROUP: std::net::Ipv4Addr = std::net::Ipv4Addr::new(239, 255, 84, 77);
pub(crate) const MAGIC: &str = "TERAMIC-LAN-1";
#[cfg(feature = "server")]
pub(crate) const MAX_REQUEST: usize = 2048;
const MAX_RESPONSE: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerStatus {
    pub instance: terra_core::PlayerUuid,
    pub protocol: u32,
    pub title: String,
    pub description: String,
    pub players: u16,
    pub max_players: u16,
    pub port: u16,
    pub identity_mode: String,
}
impl ServerStatus {
    pub fn validate(&self) -> Result<(), String> {
        if self.instance.is_nil()
            || self.title.trim().is_empty()
            || self.title.chars().count() > 64
            || self.description.chars().count() > 160
            || self.players > self.max_players
            || !(1..=128).contains(&self.max_players)
            || self.port == 0
            || self
                .title
                .chars()
                .chain(self.description.chars())
                .any(char::is_control)
            || !matches!(
                self.identity_mode.as_str(),
                "local_profile" | "verified_account"
            )
        {
            return Err("Invalid server status response".into());
        }
        Ok(())
    }
    pub fn compatible(&self) -> bool {
        self.protocol == PROTOCOL_VERSION
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Request {
    Status,
    Join {
        version: u32,
        profile: PlayerProfile,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Response {
    Status {
        status: ServerStatus,
    },
    Join {
        status: ServerStatus,
        token: Vec<u8>,
    },
    Error {
        message: String,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Probe {
    pub magic: String,
    pub nonce: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Announcement {
    pub magic: String,
    pub nonce: u64,
    pub status: ServerStatus,
}

pub fn private_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|ip| private_address(ip.into()))
        }
    }
}

fn exchange(address: SocketAddr, request: &Request) -> Result<Response, String> {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .map_err(|e| format!("Cannot reach {address}: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let mut bytes = serde_json::to_vec(request).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    stream.write_all(&bytes).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut response = Vec::new();
    // Stop before allocating arbitrary data from an untrusted server.
    loop {
        let available = reader.fill_buf().map_err(|e| e.to_string())?;
        if available.is_empty() {
            return Err("Server closed the status connection".into());
        }
        let count = available
            .iter()
            .position(|&b| b == b'\n')
            .map_or(available.len(), |p| p + 1);
        if response.len() + count > MAX_RESPONSE {
            return Err("Server response exceeds limit".into());
        }
        response.extend_from_slice(&available[..count]);
        reader.consume(count);
        if response.last() == Some(&b'\n') {
            break;
        }
    }
    serde_json::from_slice(&response).map_err(|e| format!("Invalid TERAMIC server response: {e}"))
}
pub fn query_status(address: SocketAddr) -> Result<ServerStatus, String> {
    match exchange(address, &Request::Status)? {
        Response::Status { status } => {
            status.validate()?;
            Ok(status)
        }
        Response::Error { message } => Err(message),
        _ => Err("Expected a status response".into()),
    }
}
pub fn join(address: SocketAddr, profile: &PlayerProfile) -> Result<Vec<u8>, String> {
    profile.validate().map_err(|e| e.to_string())?;
    match exchange(
        address,
        &Request::Join {
            version: PROTOCOL_VERSION,
            profile: profile.clone(),
        },
    )? {
        Response::Join { status, token } => {
            status.validate()?;
            if !status.compatible() || token.len() != 2048 {
                return Err("Incompatible server handshake".into());
            }
            #[cfg(feature = "client")]
            if crate::client::token_server_address(&token)? != address {
                return Err("Server tried to redirect the connection to another endpoint".into());
            }
            Ok(token)
        }
        Response::Error { message } => Err(message),
        _ => Err("Expected a join response".into()),
    }
}
