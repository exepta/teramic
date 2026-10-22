//! Small per-player map annotations, separate from world state and exploration tiles.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, io::Read, path::Path};
use terra_core::{Config, ConfigError};

const MAX_WAYPOINTS: usize = 256;
const MAX_FILE_BYTES: u64 = 256 * 1024;

/// Stable, renderer-independent symbols stored with personal map annotations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum WaypointIcon {
    #[default]
    Pin,
    Home,
    Cave,
    Camp,
    Treasure,
    Danger,
    Catalog(String),
}

impl WaypointIcon {
    pub fn key(&self) -> &str {
        match self {
            Self::Pin => "pin",
            Self::Home => "home",
            Self::Cave => "cave",
            Self::Camp => "camp",
            Self::Treasure => "treasure",
            Self::Danger => "danger",
            Self::Catalog(name) => name,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pin" => Some(Self::Pin),
            "home" => Some(Self::Home),
            "cave" => Some(Self::Cave),
            "camp" => Some(Self::Camp),
            "treasure" => Some(Self::Treasure),
            "danger" => Some(Self::Danger),
            _ if !value.is_empty()
                && value.len() <= 48
                && value
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-') =>
            {
                Some(Self::Catalog(value.into()))
            }
            _ => None,
        }
    }

    pub fn glyph(&self) -> &str {
        match self {
            Self::Pin => "map-pin",
            Self::Cave => "mountain",
            Self::Camp => "tent",
            Self::Treasure => "diamond",
            Self::Danger => "warning",
            _ => self.key(),
        }
    }
}
impl TryFrom<String> for WaypointIcon {
    type Error = String;
    fn try_from(value: String) -> Result<Self, String> {
        Self::parse(&value).ok_or_else(|| "Invalid waypoint icon name".into())
    }
}
impl From<WaypointIcon> for String {
    fn from(value: WaypointIcon) -> Self {
        value.key().into()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Waypoint {
    pub id: u64,
    pub name: String,
    pub position: [f64; 3],
    pub cave_level: Option<i16>,
    pub color: [u8; 3],
    #[serde(default)]
    pub icon: WaypointIcon,
    #[serde(default)]
    pub show_in_world: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WaypointBook {
    pub entries: Vec<Waypoint>,
    next_id: u64,
}

impl Default for WaypointBook {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
        }
    }
}

fn valid_point(point: &Waypoint) -> bool {
    let limit = terra_world::MAX_SAMPLE_COORDINATE as f64;
    point.id != 0
        && !point.name.trim().is_empty()
        && point.name.trim() == point.name
        && point.name.chars().count() <= 48
        && !point.name.chars().any(char::is_control)
        && point
            .position
            .iter()
            .all(|v| v.is_finite() && v.abs() <= limit)
        && point
            .cave_level
            .is_none_or(|level| (-128..=356).contains(&level) && level % 4 == 0)
        && WaypointIcon::parse(point.icon.key()).is_some()
}

impl Config for WaypointBook {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.entries.len() > MAX_WAYPOINTS
            || self.next_id == 0
            || self
                .entries
                .iter()
                .any(|point| !valid_point(point) || point.id >= self.next_id)
            || self
                .entries
                .iter()
                .map(|point| point.id)
                .collect::<BTreeSet<_>>()
                .len()
                != self.entries.len()
        {
            return Err(ConfigError::Validation("Invalid map waypoints".into()));
        }
        Ok(())
    }
}

impl WaypointBook {
    pub fn create(
        &mut self,
        name: &str,
        position: [f64; 3],
        cave_level: Option<i16>,
        color: [u8; 3],
    ) -> Result<u64, String> {
        if self.entries.len() >= MAX_WAYPOINTS {
            return Err("A map can contain at most 256 waypoints".into());
        }
        let next = self
            .next_id
            .checked_add(1)
            .ok_or("Waypoint IDs exhausted")?;
        let point = Waypoint {
            id: self.next_id,
            name: name.trim().into(),
            position,
            cave_level,
            color,
            icon: WaypointIcon::default(),
            show_in_world: true,
        };
        if !valid_point(&point) {
            return Err("Enter a waypoint name of 1-48 characters and finite coordinates".into());
        }
        let id = point.id;
        self.entries.push(point);
        self.next_id = next;
        Ok(id)
    }

    pub fn update(&mut self, point: Waypoint) -> Result<(), String> {
        if !valid_point(&point) {
            return Err("Invalid waypoint name or coordinates".into());
        }
        let existing = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == point.id)
            .ok_or("Waypoint no longer exists")?;
        *existing = point;
        Ok(())
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let previous = self.entries.len();
        self.entries.retain(|point| point.id != id);
        previous != self.entries.len()
    }

    pub fn load_directory(root: &Path) -> Result<Self, String> {
        let file = match fs::File::open(root.join("waypoints.toml")) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error.to_string()),
        };
        let mut text = String::new();
        file.take(MAX_FILE_BYTES + 1)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() as u64 > MAX_FILE_BYTES {
            return Err("Waypoint file exceeds its size limit".into());
        }
        Self::from_toml(&text).map_err(|e| e.to_string())
    }

    pub fn save_directory(&self, root: &Path) -> Result<(), String> {
        self.save(root.join("waypoints.toml"))
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maps::MapIdentity;

    #[test]
    fn waypoint_crud_roundtrips_and_worlds_and_players_are_isolated() {
        let temporary = std::env::temp_dir().join(format!(
            "terra-waypoints-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let identity = MapIdentity {
            world: terra_core::WorldId(8),
            seed: terra_core::WorldSeed(12),
            generator: 5,
            player: terra_core::PlayerUuid::new_v4(),
        };
        let root = identity.directory(&temporary);
        let mut book = WaypointBook::load_directory(&root).unwrap();
        let id = book
            .create(" Cave ", [-300.5, -65.0, 40.0], Some(-64), [176, 122, 231])
            .unwrap();
        book.save_directory(&root).unwrap();
        assert_eq!(WaypointBook::load_directory(&root).unwrap(), book);
        let mut point = book.entries[0].clone();
        point.name = "Home".into();
        point.position = [42.0, 82.0, -18.0];
        point.cave_level = None;
        book.update(point).unwrap();
        book.save_directory(&root).unwrap();
        assert_eq!(
            WaypointBook::load_directory(&root).unwrap().entries[0].name,
            "Home"
        );
        let other = MapIdentity {
            world: terra_core::WorldId(9),
            ..identity.clone()
        };
        assert!(
            WaypointBook::load_directory(&other.directory(&temporary))
                .unwrap()
                .entries
                .is_empty()
        );
        let other = MapIdentity {
            player: terra_core::PlayerUuid::new_v4(),
            ..identity
        };
        assert!(
            WaypointBook::load_directory(&other.directory(&temporary))
                .unwrap()
                .entries
                .is_empty()
        );
        assert!(book.remove(id));
        assert!(!book.remove(id));
        book.save_directory(&root).unwrap();
        assert!(
            WaypointBook::load_directory(&root)
                .unwrap()
                .entries
                .is_empty()
        );
        fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn invalid_waypoints_and_failed_saves_preserve_the_previous_data() {
        let temporary = std::env::temp_dir().join(format!(
            "terra-waypoints-invalid-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let mut book = WaypointBook::default();
        assert!(book.create("", [0.0; 3], None, [0; 3]).is_err());
        assert!(
            book.create("Invalid\nName", [0.0; 3], None, [0; 3])
                .is_err()
        );
        assert!(
            book.create("NaN", [f64::NAN, 2.0, 3.0], None, [0; 3])
                .is_err()
        );
        assert!(book.entries.is_empty());
        book.create("Valid", [-1.0, 82.0, 12.0], None, [204, 169, 92])
            .unwrap();
        book.save_directory(&temporary).unwrap();
        let valid = fs::read(temporary.join("waypoints.toml")).unwrap();
        book.entries[0].position[0] = f64::INFINITY;
        assert!(book.save_directory(&temporary).is_err());
        assert_eq!(fs::read(temporary.join("waypoints.toml")).unwrap(), valid);
        assert!(WaypointBook::from_toml("next_id = 0").is_err());
        fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn legacy_waypoints_load_without_rewriting_and_new_options_roundtrip() {
        let legacy = "next_id = 2\n[[entries]]\nid = 1\nname = 'Home'\nposition = [1.0, 80.0, -3.0]\ncolor = [231, 190, 102]\n";
        let mut book = WaypointBook::from_toml(legacy).unwrap();
        assert_eq!(book.entries[0].icon, WaypointIcon::Pin);
        assert!(!book.entries[0].show_in_world);
        book.entries[0].icon = WaypointIcon::Treasure;
        book.entries[0].show_in_world = true;
        assert_eq!(
            WaypointBook::from_toml(&toml::to_string(&book).unwrap()).unwrap(),
            book
        );
        for key in ["pin", "home", "cave", "camp", "treasure", "danger"] {
            assert_eq!(WaypointIcon::parse(key).unwrap().key(), key);
        }
        assert_eq!(WaypointIcon::parse("../arbitrary-file"), None);
        let catalog = WaypointIcon::parse("flower").unwrap();
        assert_eq!(catalog.key(), "flower");
        book.entries[0].icon = catalog;
        assert_eq!(
            WaypointBook::from_toml(&toml::to_string(&book).unwrap()).unwrap(),
            book
        );
        let mut created = WaypointBook::default();
        created
            .create("New", [1.25, 80.0, -3.0625], None, [255; 3])
            .unwrap();
        assert!(created.entries[0].show_in_world);
    }
}
