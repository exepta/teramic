//! Status/LAN bootstrap runs off the simulation thread. No credential files are used.
use crate::{
    browser::*,
    protocol::session::{PROTOCOL_ID, PROTOCOL_VERSION},
};
use bevy::prelude::*;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use terra_core::{Config, PlayerProfile, ServerConfig};

struct Ticket {
    profile: PlayerProfile,
    until: Instant,
}
struct Shared {
    status: ServerStatus,
    tickets: HashMap<u64, Ticket>,
    next: u64,
    key: [u8; 32],
}
#[derive(Resource)]
pub(crate) struct Gateway {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Gateway {
    pub fn start(config: &ServerConfig, tcp: TcpListener, key: [u8; 32]) -> Result<Self, String> {
        if config.require_verified_accounts {
            return Err("Online account authentication requires an account verifier; none is configured. Refusing to downgrade to local profiles.".into());
        }
        let bound = tcp.local_addr().map_err(|e| e.to_string())?;
        tcp.set_nonblocking(true).map_err(|e| e.to_string())?;
        let lan = if config.lan_discovery && bound.is_ipv4() {
            match discovery_socket(bound.ip()) {
                Ok(socket) => Some(socket),
                Err(error) => {
                    tracing::warn!(%error, "LAN discovery unavailable; direct address joins remain available");
                    None
                }
            }
        } else {
            None
        };
        let shared = Arc::new(Mutex::new(Shared {
            status: ServerStatus {
                instance: terra_core::PlayerUuid::new_v4(),
                protocol: PROTOCOL_VERSION,
                title: config.title.clone(),
                description: config.description.clone(),
                players: 0,
                max_players: config.max_players,
                port: bound.port(),
                identity_mode: "local_profile".into(),
            },
            tickets: HashMap::new(),
            next: 1 << 48,
            key,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let state = shared.clone();
        let handle = thread::Builder::new()
            .name("terra-status-lan".into())
            .spawn(move || serve(tcp, lan, bound, state, signal))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            shared,
            stop,
            thread: Some(handle),
        })
    }
    pub fn update_players(&self, players: usize) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.status.players = (players as u16).min(shared.status.max_players);
        }
    }
    pub fn validate_claim(&self, transport_id: u64, profile: &PlayerProfile) -> Result<(), String> {
        profile.validate().map_err(|e| e.to_string())?;
        let shared = self
            .shared
            .lock()
            .map_err(|_| "Identity registry unavailable")?;
        if transport_id >= (1 << 48) && !shared.tickets.contains_key(&transport_id) {
            return Err("Connection handshake expired; reconnect by address".into());
        }
        if let Some(ticket) = shared.tickets.get(&transport_id)
            && (ticket.until < Instant::now() || ticket.profile != *profile)
        {
            return Err("Player UUID does not match the connection handshake".into());
        }
        Ok(())
    }
}
impl Drop for Gateway {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn discovery_socket(bind: std::net::IpAddr) -> Result<(UdpSocket, UdpSocket), String> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .map_err(|e| e.to_string())?;
    socket.set_reuse_address(true).map_err(|e| e.to_string())?;
    socket
        .bind(&SocketAddr::from(([0, 0, 0, 0], DISCOVERY_PORT)).into())
        .map_err(|e| e.to_string())?;
    let socket: UdpSocket = socket.into();
    socket.set_nonblocking(true).map_err(|e| e.to_string())?;
    socket
        .join_multicast_v4(&DISCOVERY_GROUP, &std::net::Ipv4Addr::UNSPECIFIED)
        .map_err(|e| e.to_string())?;
    let _ = socket.join_multicast_v4(&DISCOVERY_GROUP, &std::net::Ipv4Addr::LOCALHOST);
    // A specifically bound server must advertise that interface, not the route
    // selected by the shared wildcard multicast receiver (especially loopback).
    let replies = UdpSocket::bind(SocketAddr::new(bind, 0)).map_err(|e| e.to_string())?;
    replies.set_nonblocking(true).map_err(|e| e.to_string())?;
    Ok((socket, replies))
}
struct Pending {
    stream: TcpStream,
    peer: SocketAddr,
    input: Vec<u8>,
    output: Option<Vec<u8>>,
    written: usize,
    deadline: Instant,
}
fn serve(
    tcp: TcpListener,
    lan: Option<(UdpSocket, UdpSocket)>,
    bound: SocketAddr,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
) {
    let mut pending = Vec::<Pending>::new();
    let mut rates = HashMap::<std::net::IpAddr, (Instant, u16)>::new();
    while !stop.load(Ordering::Acquire) {
        for _ in 0..16 {
            match tcp.accept() {
                Ok((stream, peer)) if pending.len() < 32 => {
                    if stream.set_nonblocking(true).is_ok() {
                        pending.push(Pending {
                            stream,
                            peer,
                            input: Vec::new(),
                            output: None,
                            written: 0,
                            deadline: Instant::now() + Duration::from_secs(2),
                        });
                    }
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        pending.retain_mut(|connection| service(connection, &shared, &mut rates));
        if let Some((socket, replies)) = &lan {
            let mut buffer = [0_u8; 512];
            for _ in 0..64 {
                let (size, source) = match socket.recv_from(&mut buffer) {
                    Ok(value) => value,
                    Err(_) => break,
                };
                if !private_address(source.ip())
                    || (bound.ip().is_loopback() && !source.ip().is_loopback())
                {
                    continue;
                }
                if !allow_request(&mut rates, source.ip(), 40) {
                    continue;
                }
                let Ok(probe) = serde_json::from_slice::<Probe>(&buffer[..size]) else {
                    continue;
                };
                if probe.magic != MAGIC {
                    continue;
                }
                if let Ok(state) = shared.lock()
                    && let Ok(bytes) = serde_json::to_vec(&Announcement {
                        magic: MAGIC.into(),
                        nonce: probe.nonce,
                        status: state.status.clone(),
                    })
                {
                    let _ = replies.send_to(&bytes, source);
                }
            }
        }
        rates.retain(|_, (time, _)| time.elapsed() < Duration::from_secs(10));
        thread::sleep(Duration::from_millis(10));
    }
}
fn allow_request(
    rates: &mut HashMap<std::net::IpAddr, (Instant, u16)>,
    ip: std::net::IpAddr,
    limit: u16,
) -> bool {
    if !rates.contains_key(&ip) && rates.len() >= 256 {
        return false;
    }
    let entry = rates.entry(ip).or_insert((Instant::now(), 0));
    if entry.0.elapsed() >= Duration::from_secs(2) {
        *entry = (Instant::now(), 0);
    }
    entry.1 = entry.1.saturating_add(1);
    entry.1 <= limit
}
fn service(
    connection: &mut Pending,
    shared: &Mutex<Shared>,
    rates: &mut HashMap<std::net::IpAddr, (Instant, u16)>,
) -> bool {
    if Instant::now() >= connection.deadline {
        return false;
    }
    if connection.output.is_none() {
        let mut buffer = [0_u8; 512];
        for _ in 0..5 {
            match connection.stream.read(&mut buffer) {
                Ok(0) => return false,
                Ok(size) => {
                    if connection.input.len() + size > MAX_REQUEST {
                        return false;
                    }
                    connection.input.extend_from_slice(&buffer[..size]);
                    if connection.input.contains(&b'\n') {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => return false,
            }
        }
        if !connection.input.contains(&b'\n') {
            return true;
        }
        let response = if !allow_request(rates, connection.peer.ip(), 30) {
            Response::Error {
                message: "Server query rate exceeded".into(),
            }
        } else {
            respond(
                &connection.input,
                connection.peer,
                connection.stream.local_addr().ok(),
                shared,
            )
            .unwrap_or_else(|message| Response::Error { message })
        };
        let Ok(mut bytes) = serde_json::to_vec(&response) else {
            return false;
        };
        bytes.push(b'\n');
        connection.output = Some(bytes);
    }
    if let Some(bytes) = &connection.output {
        match connection.stream.write(&bytes[connection.written..]) {
            Ok(0) => return false,
            Ok(size) => connection.written += size,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return false,
        }
        connection.written < bytes.len()
    } else {
        true
    }
}
fn respond(
    bytes: &[u8],
    peer: SocketAddr,
    local: Option<SocketAddr>,
    shared: &Mutex<Shared>,
) -> Result<Response, String> {
    let request: Request =
        serde_json::from_slice(bytes).map_err(|_| "Invalid status/join request")?;
    let mut state = shared.lock().map_err(|_| "Status service unavailable")?;
    match request {
        Request::Status => Ok(Response::Status {
            status: state.status.clone(),
        }),
        Request::Join { version, profile } => {
            if version != PROTOCOL_VERSION {
                return Err("Incompatible TERAMIC protocol".into());
            }
            if !private_address(peer.ip()) {
                return Err("This server accepts unverified profiles only over trusted LAN/loopback. Online account verification is not configured.".into());
            }
            profile.validate().map_err(|e| e.to_string())?;
            if state.status.players >= state.status.max_players {
                return Err("Server is full".into());
            }
            state
                .tickets
                .retain(|_, ticket| ticket.until > Instant::now());
            if state.tickets.len() >= 256 {
                return Err("Too many pending joins; try again shortly".into());
            }
            let id = state.next;
            state.next = state.next.checked_add(1).ok_or("Connection ID exhausted")?;
            let address = local.ok_or("Missing local server address")?;
            let token =
                lightyear::netcode::ConnectToken::build(address, PROTOCOL_ID, id, state.key)
                    .expire_seconds(120)
                    .timeout_seconds(10)
                    .generate()
                    .map_err(|e| e.to_string())?
                    .try_into_bytes()
                    .map_err(|e| e.to_string())?
                    .to_vec();
            state.tickets.insert(
                id,
                Ticket {
                    profile,
                    until: Instant::now() + Duration::from_secs(120),
                },
            );
            Ok(Response::Join {
                status: state.status.clone(),
                token,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_bootstrap_is_bounded_and_does_not_claim_account_verification() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let gateway = Gateway::start(
            &ServerConfig::default(),
            listener,
            lightyear::netcode::generate_key(),
        )
        .unwrap();
        let status = query_status(address).unwrap();
        assert_eq!(status.identity_mode, "local_profile");
        let profile = PlayerProfile::default();
        let response = |peer: &str, version, profile| {
            respond(
                &serde_json::to_vec(&Request::Join { version, profile }).unwrap(),
                peer.parse().unwrap(),
                Some(address),
                &gateway.shared,
            )
        };
        assert!(response("8.8.8.8:1234", PROTOCOL_VERSION, profile.clone()).is_err());
        assert!(response("127.0.0.1:1234", PROTOCOL_VERSION + 1, profile.clone()).is_err());
        let Response::Join { token, .. } =
            response("127.0.0.1:1234", PROTOCOL_VERSION, profile.clone()).unwrap()
        else {
            panic!("missing bootstrap");
        };
        assert_eq!(token.len(), 2048);
        let id = 1 << 48;
        assert!(gateway.validate_claim(id, &profile).is_ok());
        assert!(
            gateway
                .validate_claim(id, &PlayerProfile::default())
                .is_err()
        );
        gateway
            .shared
            .lock()
            .unwrap()
            .tickets
            .get_mut(&id)
            .unwrap()
            .until = Instant::now() - Duration::from_secs(1);
        assert!(gateway.validate_claim(id, &profile).is_err());
        gateway.shared.lock().unwrap().tickets.clear();
        assert!(gateway.validate_claim(id, &profile).is_err());
        let mut oversized = TcpStream::connect(address).unwrap();
        oversized
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        oversized.write_all(&vec![b'a'; MAX_REQUEST + 1]).unwrap();
        let mut byte = [0];
        assert!(matches!(oversized.read(&mut byte), Ok(0) | Err(_)));
        assert!(
            query_status(address).is_ok(),
            "bad requests must not kill the listener"
        );
    }

    #[test]
    fn verified_accounts_fail_closed_without_an_account_service() {
        let config = ServerConfig {
            require_verified_accounts: true,
            ..default()
        };
        let result = Gateway::start(&config, TcpListener::bind("127.0.0.1:0").unwrap(), [0; 32]);
        assert!(result.err().unwrap().contains("account verifier"));
    }
}
