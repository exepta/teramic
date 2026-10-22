use super::{Config, ConfigError, ensure};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerEntry {
    pub id: u64,
    pub name: String,
    pub address: String,
    pub port: u16,
    // Read old lists without requiring a migration; new saves omit obsolete credentials.
    #[serde(default, skip_serializing)]
    pub token_file: PathBuf,
}
impl ServerEntry {
    pub fn validate(&self) -> Result<(), ConfigError> {
        ensure(
            !self.name.trim().is_empty()
                && self.name.chars().count() <= 64
                && !self.name.chars().any(char::is_control),
            "Server name must contain 1..64 printable characters",
        )?;
        ensure(self.port != 0, "Port must be 1..65535")?;
        let host = self.address.trim();
        let valid = host.parse::<std::net::IpAddr>().is_ok()
            || (host.len() <= 253
                && host.split('.').all(|label| {
                    !label.is_empty()
                        && label.len() <= 63
                        && !label.starts_with('-')
                        && !label.ends_with('-')
                        && label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                }));
        ensure(
            valid,
            "Enter an IP address or hostname without a scheme or port",
        )
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerList {
    pub servers: Vec<ServerEntry>,
}
impl Config for ServerList {
    fn validate(&self) -> Result<(), ConfigError> {
        ensure(self.servers.len() <= 256, "At most 256 saved servers")?;
        let mut ids = std::collections::HashSet::new();
        for server in &self.servers {
            server.validate()?;
            ensure(ids.insert(server.id), "Duplicate saved server ID")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_join_file_is_accepted_on_read_but_never_required_or_saved() {
        let text = "[[servers]]\nid = 42\nname = 'Workshop'\naddress = 'localhost'\nport = 5000\ntoken_file = '/nonexistent/old.token'";
        let list: ServerList = toml::from_str(text).unwrap();
        list.validate().unwrap();
        let saved = toml::to_string(&list).unwrap();
        assert!(!saved.contains("token_file"));
        let reloaded: ServerList = toml::from_str(&saved).unwrap();
        assert_eq!(reloaded.servers[0].id, 42);
        assert_eq!(reloaded.servers[0].name, "Workshop");
        assert_eq!(reloaded.servers[0].address, "localhost");
        assert!(reloaded.servers[0].token_file.as_os_str().is_empty());
    }
}
