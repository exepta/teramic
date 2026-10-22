use super::model::*;
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    collections::HashMap,
    net::{SocketAddr, ToSocketAddrs},
    time::{Duration, Instant},
};
use terra_network::{
    browser::{ServerStatus, query_status},
    discovery::{LanScanner, LanServer},
};

#[derive(Default)]
pub(super) struct StatusRecord {
    pub status: Option<ServerStatus>,
    pub endpoint: Option<SocketAddr>,
    pub ping_ms: u32,
    pub checked: Option<Instant>,
    pub error: String,
}
type StatusResult = Result<(SocketAddr, ServerStatus, u32), String>;
struct Pending {
    id: u64,
    address: String,
    port: u16,
    task: Task<StatusResult>,
}
#[derive(Resource, Default)]
pub(super) struct ServerBrowser {
    scanner: Option<LanScanner>,
    initialized: bool,
    pub lan: Vec<LanServer>,
    pub records: HashMap<u64, StatusRecord>,
    tasks: Vec<Pending>,
    pub error: String,
}
pub(super) fn update(
    mut browser: ResMut<ServerBrowser>,
    mut servers: ResMut<ServerMenu>,
    menu: Res<MenuState>,
) {
    if menu.page != MenuPage::Servers {
        return;
    }
    if !browser.initialized {
        browser.initialized = true;
        match LanScanner::new() {
            Ok(scanner) => browser.scanner = Some(scanner),
            Err(error) => browser.error = error,
        }
    }
    if let Some(scanner) = &mut browser.scanner {
        let result = scanner.poll();
        let entries = scanner.servers.values().cloned().collect::<Vec<_>>();
        browser.lan = entries;
        if let Err(error) = result {
            browser.error = error;
        }
    }
    browser.lan.sort_by(|a, b| {
        a.status
            .title
            .cmp(&b.status.title)
            .then(a.address.cmp(&b.address))
    });
    let mut finished = Vec::new();
    for pending in &mut browser.tasks {
        if let Some(result) = future::block_on(future::poll_once(&mut pending.task)) {
            finished.push((pending.id, pending.address.clone(), pending.port, result));
        }
    }
    for (id, address, port, result) in finished {
        browser.tasks.retain(|task| task.id != id);
        if !servers
            .list
            .servers
            .iter()
            .any(|server| server.id == id && server.address == address && server.port == port)
        {
            continue;
        }
        let mut record = StatusRecord {
            checked: Some(Instant::now()),
            ..default()
        };
        match result {
            Ok((endpoint, status, ping_ms)) => {
                record.status = Some(status);
                record.endpoint = Some(endpoint);
                record.ping_ms = ping_ms;
            }
            Err(error) => record.error = error,
        }
        browser.records.insert(id, record);
    }
    browser
        .records
        .retain(|id, _| servers.list.servers.iter().any(|s| s.id == *id));
    for server in &servers.list.servers {
        if browser.tasks.len() >= 4 {
            break;
        }
        if browser.tasks.iter().any(|task| task.id == server.id)
            || browser
                .records
                .get(&server.id)
                .and_then(|record| record.checked)
                .is_some_and(|time| time.elapsed() < Duration::from_secs(5))
        {
            continue;
        }
        let address = server.address.clone();
        let hostname = address.clone();
        let port = server.port;
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let addresses = (hostname.as_str(), port)
                .to_socket_addrs()
                .map_err(|e| e.to_string())?;
            let mut error = "Address has no usable endpoints".to_string();
            for address in addresses.take(2) {
                let started = Instant::now();
                match query_status(address) {
                    Ok(status) => {
                        return Ok((
                            address,
                            status,
                            started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32,
                        ));
                    }
                    Err(e) => error = e,
                }
            }
            Err(error)
        });
        browser.tasks.push(Pending {
            id: server.id,
            address,
            port,
            task,
        });
    }
    if let Some(address) = servers.lan_selected {
        if let Some(found) = browser.lan.iter().find(|s| s.address == address) {
            if let Some((&id, _)) = browser.records.iter().find(|(_, record)| {
                record
                    .status
                    .as_ref()
                    .is_some_and(|status| status.instance == found.status.instance)
            }) {
                servers.selected = Some(id);
                servers.lan_selected = None;
            }
        } else {
            servers.lan_selected = None;
        }
    }
}
