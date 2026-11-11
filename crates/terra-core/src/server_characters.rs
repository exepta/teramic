//! Independent appearance overrides for each local identity and server endpoint.
use crate::{CharacterSelection, Config, ConfigError, PlayerProfile, PlayerUuid};
use serde::{Deserialize, Serialize};

const MAX_SELECTIONS: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerCharacter {
    uuid: PlayerUuid,
    host: String,
    port: u16,
    selection: CharacterSelection,
}

/// Visual overrides are keyed by address, port and UUID, never mutable list IDs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct ServerCharacterProfiles {
    characters: Vec<ServerCharacter>,
}

impl ServerCharacterProfiles {
    /// A missing entry marks the first visit and inherits the current local character.
    pub fn get(
        &self,
        uuid: PlayerUuid,
        host: &str,
        port: u16,
    ) -> Result<Option<CharacterSelection>, ConfigError> {
        let host = canonical_host(host, port)?;
        Ok(self
            .characters
            .iter()
            .find(|entry| entry.uuid == uuid && entry.host == host && entry.port == port)
            .map(|entry| entry.selection))
    }

    pub fn set(
        &mut self,
        uuid: PlayerUuid,
        host: &str,
        port: u16,
        selection: CharacterSelection,
    ) -> Result<(), ConfigError> {
        selection.validate()?;
        let host = canonical_host(host, port)?;
        if uuid.is_nil() {
            return Err(ConfigError::Validation(
                "A character requires a player UUID".into(),
            ));
        }
        if let Some(entry) = self
            .characters
            .iter_mut()
            .find(|entry| entry.uuid == uuid && entry.host == host && entry.port == port)
        {
            entry.selection = selection;
            return Ok(());
        }
        if self.characters.len() >= MAX_SELECTIONS {
            return Err(ConfigError::Validation(
                "Too many server character profiles".into(),
            ));
        }
        self.characters.push(ServerCharacter {
            uuid,
            host,
            port,
            selection,
        });
        Ok(())
    }

    /// Return the handshake profile without mutating the local default identity.
    pub fn apply(
        &self,
        profile: &PlayerProfile,
        host: &str,
        port: u16,
    ) -> Result<PlayerProfile, ConfigError> {
        let mut selected = profile.clone();
        if let Some(selection) = self.get(profile.uuid, host, port)? {
            selection.apply_to(&mut selected);
        }
        selected.validate()?;
        Ok(selected)
    }
}

impl Config for ServerCharacterProfiles {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.characters.len() > MAX_SELECTIONS {
            return Err(ConfigError::Validation(
                "Too many server character profiles".into(),
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for entry in &self.characters {
            entry.selection.validate()?;
            let host = canonical_host(&entry.host, entry.port)?;
            if entry.uuid.is_nil()
                || host != entry.host
                || !seen.insert((entry.uuid, host, entry.port))
            {
                return Err(ConfigError::Validation(
                    "Invalid or duplicate server character profile".into(),
                ));
            }
        }
        Ok(())
    }
}

fn canonical_host(host: &str, port: u16) -> Result<String, ConfigError> {
    let host = host.trim();
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    let host = host.parse::<std::net::IpAddr>().map_or_else(
        |_| host.trim_end_matches('.').to_ascii_lowercase(),
        |address| address.to_string(),
    );
    crate::config::servers::ServerEntry {
        id: 0,
        name: "Character".into(),
        address: host.clone(),
        port,
        token_file: Default::default(),
    }
    .validate()?;
    Ok(host)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CharacterAppearance, PlayerGender};

    #[test]
    fn server_override_survives_reload_without_changing_identity_or_other_servers() {
        let local = PlayerProfile::default();
        let selection = CharacterSelection {
            gender: PlayerGender::Female,
            appearance: CharacterAppearance {
                hairstyle: 9,
                hair_color: [24, 72, 98],
                shirt_color: [10, 20, 30],
                pants_color: [40, 50, 60],
                ..Default::default()
            },
        };
        let mut profiles = ServerCharacterProfiles::default();
        assert_eq!(
            profiles.get(local.uuid, "EXAMPLE.org.", 5000).unwrap(),
            None
        );
        profiles
            .set(local.uuid, "EXAMPLE.org.", 5000, selection)
            .unwrap();
        let encoded = toml::to_string(&profiles).unwrap();
        let profiles = ServerCharacterProfiles::from_toml(&encoded).unwrap();
        assert_eq!(
            profiles.get(local.uuid, "example.org", 5000).unwrap(),
            Some(selection)
        );
        let selected = profiles.apply(&local, "example.org", 5000).unwrap();
        assert_eq!(selected.uuid, local.uuid);
        assert_eq!(selected.name, local.name);
        assert_eq!(CharacterSelection::from(&selected), selection);
        assert_eq!(profiles.apply(&local, "example.org", 5001).unwrap(), local);
        assert_eq!(profiles.apply(&local, "another.org", 5000).unwrap(), local);
        let other = PlayerProfile::default();
        assert_eq!(profiles.apply(&other, "example.org", 5000).unwrap(), other);
        assert_eq!(local.appearance, CharacterAppearance::default());
    }

    #[test]
    fn endpoint_keys_are_canonical_and_invalid_updates_are_atomic() {
        let local = PlayerProfile::default();
        let selected = CharacterSelection::from(&local);
        let mut profiles = ServerCharacterProfiles::default();
        profiles
            .set(local.uuid, "[0:0:0:0:0:0:0:1]", 5000, selected)
            .unwrap();
        assert_eq!(
            profiles.get(local.uuid, "::1", 5000).unwrap(),
            Some(selected)
        );
        profiles.set(local.uuid, "[::1]", 5000, selected).unwrap();
        assert_eq!(profiles.characters.len(), 1);
        let before = profiles.clone();
        let invalid = CharacterSelection {
            appearance: CharacterAppearance {
                hairstyle: 255,
                ..Default::default()
            },
            ..selected
        };
        assert!(profiles.set(local.uuid, "::1", 5000, invalid).is_err());
        assert!(
            profiles
                .set(local.uuid, "https://example.org", 5000, selected)
                .is_err()
        );
        assert!(
            profiles
                .set(local.uuid, "example.org", 0, selected)
                .is_err()
        );
        assert_eq!(profiles, before);
        let duplicate = format!("{0}\n{0}", toml::to_string(&profiles).unwrap());
        assert!(ServerCharacterProfiles::from_toml(&duplicate).is_err());
    }
}
