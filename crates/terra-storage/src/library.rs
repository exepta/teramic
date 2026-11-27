use crate::{SAVE_FORMAT_VERSION, WorldManifest};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use terra_core::{Config, GameMode, GameplayState, WorldConfig, WorldId, WorldSeed, save_toml};
use terra_world::edits::{EditJournal, SectionDelta};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldEntry {
    pub title: String,
    pub game_version: String,
    pub mode: GameMode,
    pub last_played: Option<u64>,
    pub manifest: WorldManifest,
}
impl WorldEntry {
    pub fn compatible(&self) -> Result<(), String> {
        if !matches!(self.manifest.save_format_version, 1 | SAVE_FORMAT_VERSION) {
            return Err("Unsupported world save format".into());
        }
        if self.game_version != env!("CARGO_PKG_VERSION") {
            return Err(format!("World requires game {}", self.game_version));
        }
        self.manifest.world.validate().map_err(|e| e.to_string())
    }
}
pub fn world_directory() -> PathBuf {
    std::env::var_os("TERRA_WORLD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| terra_core::installation_root().join("worlds"))
}
#[derive(Clone, Debug)]
pub struct WorldLibrary {
    pub root: PathBuf,
}
impl Default for WorldLibrary {
    fn default() -> Self {
        Self {
            root: world_directory(),
        }
    }
}
impl WorldLibrary {
    pub fn path(&self, id: WorldId) -> PathBuf {
        self.resolve(id)
            .unwrap_or_else(|_| self.root.join(format!("{:016x}", id.0)))
    }
    fn resolve(&self, id: WorldId) -> Result<PathBuf, String> {
        let mut found = None;
        for item in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let path = item.map_err(|e| e.to_string())?.path();
            if !path.join("world.toml").is_file() {
                continue;
            }
            if let Ok(entry) = read_entry(&path)
                && entry.manifest.world.id == id
                && found.replace(path).is_some()
            {
                return Err("Duplicate world ID; refusing ambiguous world access".into());
            }
        }
        found.ok_or_else(|| "World not found".into())
    }
    pub fn list(&self) -> Result<(Vec<WorldEntry>, Vec<String>), String> {
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let mut worlds = Vec::new();
        let mut errors = Vec::new();
        for item in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            let Some(name) = item.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !item.path().join("world.toml").is_file() {
                continue;
            }
            match read_entry(&item.path()) {
                Ok(world) => worlds.push(world),
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        worlds.sort_by(|a, b| {
            b.last_played
                .cmp(&a.last_played)
                .then_with(|| a.title.cmp(&b.title))
        });
        Ok((worlds, errors))
    }
    pub fn load(&self, id: WorldId) -> Result<WorldEntry, String> {
        read_entry(&self.resolve(id)?)
    }
    pub fn create(
        &self,
        title: &str,
        seed: Option<u64>,
        mode: GameMode,
        template: &WorldConfig,
    ) -> Result<WorldEntry, String> {
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 64 || title.chars().any(char::is_control) {
            return Err("World name must contain 1..64 printable characters".into());
        }
        let biome_definitions = template.biome_definitions.clone().map_or_else(
            || {
                terra_core::BiomeDefinitions::load_directory(
                    terra_core::asset_directory().join("biome"),
                )
                .map_err(|error| error.to_string())
            },
            Ok,
        )?;
        biome_definitions
            .validate()
            .map_err(|error| error.to_string())?;
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        for sequence in 0..32_u64 {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            (SystemTime::now(), std::process::id(), title, sequence).hash(&mut hash);
            let id = WorldId(hash.finish());
            let path = self.available_name(title, sequence);
            match fs::create_dir(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
            let mut world = template.clone();
            world.id = id;
            world.seed = WorldSeed(seed.unwrap_or_else(|| id.0.rotate_left(17)));
            world.geology_revision = 4;
            world.ecology_revision = WorldConfig::CURRENT_ECOLOGY_REVISION;
            world.spirit_blossom_revision = 2;
            world.blood_forest_revision = 1;
            world.landscape_revision = WorldConfig::CURRENT_LANDSCAPE_REVISION;
            world.biome_definitions = Some(biome_definitions.clone());
            world.ocean_revision = 1;
            world.cave_structure_revision = WorldConfig::CURRENT_CAVE_STRUCTURE_REVISION;
            world.structures_revision = terra_core::WorldConfig::CURRENT_STRUCTURES_REVISION;
            world.structure_border_gap = template.kingdom_rules.realm_min_border_gap;
            world.ravine_revision = WorldConfig::CURRENT_RAVINE_REVISION;
            world.river_terrain_revision = WorldConfig::CURRENT_RIVER_TERRAIN_REVISION;
            if world.ores.revision == 0 {
                world.ores.enabled = true;
            }
            world.ores.revision = 1;
            let entry = WorldEntry {
                title: title.into(),
                game_version: env!("CARGO_PKG_VERSION").into(),
                mode,
                last_played: None,
                manifest: WorldManifest::new(world),
            };
            if let Err(error) =
                save_toml(&entry, path.join("world.toml")).map_err(|e| e.to_string())
            {
                let _ = fs::remove_dir(&path);
                return Err(error);
            }
            return Ok(entry);
        }
        Err("Cannot allocate a unique world directory".into())
    }
    pub fn save(&self, entry: &WorldEntry) -> Result<(), String> {
        entry.compatible()?;
        let path = self.resolve(entry.manifest.world.id)?;
        crate::manifest::protect_metadata(&path, entry.manifest.world.id)?;
        let mut entry = entry.clone();
        entry.manifest.save_format_version = SAVE_FORMAT_VERSION;
        save_toml(
            &entry,
            self.resolve(entry.manifest.world.id)?.join("world.toml"),
        )
        .map_err(|e| e.to_string())
    }
    pub fn mark_played(&self, id: WorldId) -> Result<WorldEntry, String> {
        let mut entry = self.load(id)?;
        entry.compatible()?;
        let path = self.resolve(id)?;
        let _lock = crate::lock_world(&path)?;
        if path
            .file_name()
            .is_some_and(|n| n == format!("{:016x}", id.0).as_str())
        {
            for sequence in 0..32 {
                let named = self.available_name(&entry.title, sequence);
                if !named.exists() {
                    fs::rename(&path, named).map_err(|e| e.to_string())?;
                    break;
                }
            }
        }
        entry.last_played = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_secs(),
        );
        entry.manifest.save_format_version = SAVE_FORMAT_VERSION;
        self.save(&entry)?;
        Ok(entry)
    }
    pub fn delete(&self, id: WorldId) -> Result<(), String> {
        self.load(id)?;
        let path = self.resolve(id)?;
        let _lock = crate::lock_world(&path)?;
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    }
    fn available_name(&self, title: &str, sequence: u64) -> PathBuf {
        let name: String = title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let name = name.trim();
        let name = if name.is_empty() { "World" } else { name };
        let escaped = if name.len() == 16 && name.chars().all(|c| c.is_ascii_hexdigit()) {
            format!("World-{name}")
        } else {
            name.into()
        };
        let name = escaped.as_str();
        self.root.join(if sequence == 0 {
            name.into()
        } else {
            format!("{name}-{}", sequence + 1)
        })
    }
}
fn read_entry(path: &Path) -> Result<WorldEntry, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("World directory must not be a symbolic link".into());
    }
    let manifest = path.join("world.toml");
    if fs::symlink_metadata(&manifest)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("World metadata must not be a symbolic link".into());
    }
    let text = fs::read_to_string(manifest).map_err(|e| e.to_string())?;
    let entry: WorldEntry = toml::from_str(&text).map_err(|e| e.to_string())?;
    if let Some(name) = path.file_name().and_then(|n| n.to_str())
        && name.len() == 16
        && name.chars().all(|c| c.is_ascii_hexdigit())
        && u64::from_str_radix(name, 16).ok() != Some(entry.manifest.world.id.0)
    {
        return Err("Legacy world directory and manifest ID disagree".into());
    }
    Ok(entry)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EditSnapshot {
    save_format_version: u32,
    world: WorldId,
    sections: Vec<SectionDelta>,
    #[serde(default)]
    gameplay: GameplayState,
}
pub(crate) fn load_legacy(path: &Path, world: WorldId) -> Result<EditJournal, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(EditJournal::default()),
        Err(e) => return Err(e.to_string()),
    };
    let snapshot: EditSnapshot = toml::from_str(&text).map_err(|e| e.to_string())?;
    if snapshot.save_format_version != 1 || snapshot.world != world {
        return Err("Edit snapshot belongs to an incompatible world".into());
    }
    let mut journal = EditJournal::default();
    journal.import_gameplay(snapshot.gameplay)?;
    let mut owners = std::collections::HashSet::new();
    for section in snapshot.sections {
        if !owners.insert(section.position) {
            return Err("Duplicate section in edit snapshot".into());
        }
        journal.import_delta(section)?;
    }
    Ok(journal)
}
#[cfg(test)]
fn save_legacy(path: &Path, world: WorldId, journal: &EditJournal) -> Result<(), String> {
    let mut positions: Vec<_> = journal.changed_sections().collect();
    positions.sort_by_key(|p| (p.x, p.y, p.z));
    let snapshot = EditSnapshot {
        save_format_version: 1,
        world,
        gameplay: journal.gameplay().clone(),
        sections: positions
            .into_iter()
            .map(|p| journal.section_delta(p))
            .collect(),
    };
    save_toml(&snapshot, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{load_edits, save_edits};
    use terra_core::BiomeDefinitions;
    fn library() -> WorldLibrary {
        let root = std::env::temp_dir().join(format!(
            "teramic-worlds-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        WorldLibrary { root }
    }
    #[test]
    fn world_create_reload_selection_and_delete_are_real() {
        let library = library();
        let entry = library
            .create(
                "Green Vale",
                Some(u64::MAX),
                GameMode::Creative,
                &WorldConfig::from_toml("").unwrap(),
            )
            .unwrap();
        assert_eq!(entry.manifest.world.landscape_revision, 9);
        assert_eq!(entry.manifest.world.sea_level, 8.0);
        assert_eq!(
            entry.manifest.world.biome_definitions,
            Some(BiomeDefinitions::default())
        );
        assert!(!entry.manifest.world.rivers);
        assert_eq!(
            entry.manifest.world.structures_revision,
            WorldConfig::CURRENT_STRUCTURES_REVISION
        );
        assert_eq!(
            entry.manifest.world.ecology_revision,
            WorldConfig::CURRENT_ECOLOGY_REVISION
        );
        assert_eq!(entry.manifest.world.river_terrain_revision, 7);
        assert_eq!(library.list().unwrap().0.len(), 1);
        assert_eq!(
            library
                .load(entry.manifest.world.id)
                .unwrap()
                .manifest
                .world
                .seed
                .0,
            u64::MAX
        );
        assert!(
            library
                .mark_played(entry.manifest.world.id)
                .unwrap()
                .last_played
                .is_some()
        );
        library.delete(entry.manifest.world.id).unwrap();
        assert!(library.list().unwrap().0.is_empty());
        fs::remove_dir_all(library.root).unwrap();
    }

    #[test]
    fn new_singleplayer_world_snapshots_creation_gap_once_and_keeps_custom_live_rules() {
        let library = library();
        let template = WorldConfig {
            structures_revision: 2,
            structure_border_gap: 99,
            kingdom_rules: terra_core::kingdom::KingdomRules {
                realm_min_border_gap: 6,
                kingdom_build_base: 47,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut entry = library
            .create("Custom policy", Some(20202), GameMode::Survival, &template)
            .unwrap();
        assert_eq!(
            entry.manifest.world.structures_revision,
            WorldConfig::CURRENT_STRUCTURES_REVISION
        );
        assert_eq!(entry.manifest.world.structure_border_gap, 6);
        assert_eq!(entry.manifest.world.kingdom_rules, template.kingdom_rules);
        entry.manifest.world.kingdom_rules.realm_min_border_gap = 80;
        entry
            .manifest
            .world
            .kingdom_rules
            .settlements_follow_kingdom_level = true;
        library.save(&entry).unwrap();
        let loaded = library.mark_played(entry.manifest.world.id).unwrap();
        assert_eq!(loaded.manifest.world.structure_border_gap, 6);
        assert_eq!(
            loaded.manifest.world.kingdom_rules,
            entry.manifest.world.kingdom_rules
        );
        assert!(loaded.manifest.world.same_terrain(&entry.manifest.world));
        let mut journal = EditJournal::default();
        let mut rules = terra_core::WorldRuleSettings::from_world(&loaded.manifest.world);
        rules.kingdom_rules.realm_min_border_gap = 92;
        journal.world_rules = Some(rules);
        crate::save_edits(
            &library.path(entry.manifest.world.id),
            entry.manifest.world.id,
            &journal,
        )
        .unwrap();
        let changed = library.load(entry.manifest.world.id).unwrap();
        assert_eq!(changed.manifest.world.kingdom_rules, rules.kingdom_rules);
        assert_eq!(changed.manifest.world.structure_border_gap, 6);
        assert!(changed.manifest.world.same_terrain(&entry.manifest.world));
        fs::remove_dir_all(library.root).unwrap();
    }

    #[test]
    fn saved_landscapes_keep_their_revision_through_load_play_and_save() {
        let library = library();
        for revision in 0..=5 {
            let mut entry = library
                .create(
                    &format!("Saved Landscape {revision}"),
                    Some(77),
                    GameMode::Survival,
                    &WorldConfig::default(),
                )
                .unwrap();
            entry.manifest.world.landscape_revision = revision;
            entry.manifest.world.rivers = revision % 2 == 1;
            entry.manifest.world.sea_level = if revision < 4 {
                8.0 + f64::from(revision)
            } else {
                22.0
            };
            if revision < 4 && revision % 2 == 0 {
                entry.manifest.world.biome_definitions = None;
            } else {
                let definitions = entry
                    .manifest
                    .world
                    .biome_definitions
                    .insert(BiomeDefinitions::default());
                definitions.entries[terra_core::BiomeKind::Plains as usize]
                    .terrain
                    .max_height -= 1.0 + f64::from(revision);
                definitions.landscapes.warp_strength += 1.0 + f64::from(revision);
                definitions.landscapes.profiles
                    [terra_core::config::TerrainKind::Forest as usize]
                    .max_height -= 1.0 + f64::from(revision);
            }
            let id = entry.manifest.world.id;
            let path = library.path(id);
            save_toml(&entry, path.join("world.toml")).unwrap();
            let saved: WorldEntry =
                toml::from_str(&fs::read_to_string(path.join("world.toml")).unwrap()).unwrap();
            assert_eq!(saved.manifest.world, entry.manifest.world);
            assert_eq!(saved.manifest.world.id, id);
            let mut journal = EditJournal::default();
            journal.record([-17, 12, -1], 16).unwrap();
            save_edits(&path, id, &journal).unwrap();
            assert!(
                library
                    .load(id)
                    .unwrap()
                    .manifest
                    .world
                    .same_terrain(&entry.manifest.world)
            );
            let mut reloaded = library.mark_played(id).unwrap();
            assert_eq!(reloaded.manifest.world.landscape_revision, revision);
            reloaded.manifest.world.time.paused = true;
            library.save(&reloaded).unwrap();
            let restored = library.load(id).unwrap();
            assert_eq!(restored.manifest.world, reloaded.manifest.world);
            assert_eq!(restored.manifest.world.id, id);
            assert_eq!(restored.manifest.world.landscape_revision, revision);
            assert_eq!(
                load_edits(&path, id).unwrap().block([-17, 12, -1]),
                Some(16)
            );
        }
        fs::remove_dir_all(library.root).unwrap();
    }

    #[test]
    fn new_worlds_use_main_terrain_and_preserve_explicit_sea_levels_and_rivers() {
        let library = library();
        for (revision, sea_level, expected) in [
            (0, 8.0, 8.0),
            (3, 8.0, 8.0),
            (3, 13.0, 13.0),
            (4, 8.0, 8.0),
            (4, 22.0, 22.0),
            (5, 8.0, 8.0),
            (5, 22.0, 22.0),
        ] {
            let template = WorldConfig {
                landscape_revision: revision,
                sea_level,
                rivers: revision % 2 == 1,
                biome_definitions: Some(BiomeDefinitions::default()),
                ..Default::default()
            };
            let entry = library
                .create("Sea Level", Some(77), GameMode::Survival, &template)
                .unwrap();
            assert_eq!(entry.manifest.world.landscape_revision, 9);
            assert_eq!(entry.manifest.world.sea_level, expected);
            assert_eq!(entry.manifest.world.rivers, template.rivers);
            assert_eq!(
                entry.manifest.world.biome_definitions,
                template.biome_definitions
            );
            let reloaded = library.load(entry.manifest.world.id).unwrap();
            assert_eq!(reloaded.manifest.world, entry.manifest.world);
            assert_eq!(template.sea_level, sea_level);
        }
        fs::remove_dir_all(library.root).unwrap();
    }

    #[test]
    fn named_directories_do_not_escape_root_and_legacy_hex_directories_move_without_changing_ids() {
        let library = library();
        let entry = library
            .create(
                "../River/Valley",
                Some(77),
                GameMode::Creative,
                &WorldConfig::default(),
            )
            .unwrap();
        let id = entry.manifest.world.id;
        let named = library.path(id);
        assert_eq!(named.parent(), Some(library.root.as_path()));
        assert!(!named.file_name().unwrap().to_string_lossy().contains('/'));
        let legacy = library.root.join(format!("{:016x}", id.0));
        fs::rename(&named, &legacy).unwrap();
        let mut journal = EditJournal::default();
        journal.record([-1, 0, -1], 3).unwrap();
        save_edits(&legacy, id, &journal).unwrap();
        assert_eq!(library.mark_played(id).unwrap().manifest.world.id, id);
        assert_eq!(library.path(id), named);
        assert!(!legacy.exists());
        assert_eq!(load_edits(&named, id).unwrap().block([-1, 0, -1]), Some(3));
        fs::remove_dir_all(library.root).unwrap();
    }
    #[test]
    fn existing_journal_roundtrips_without_changing_ids_or_revisions() {
        let library = library();
        let entry = library
            .create("Save", Some(7), GameMode::Survival, &WorldConfig::default())
            .unwrap();
        let id = entry.manifest.world.id;
        let path = library.path(id).join("edits.toml");
        let mut journal = EditJournal::default();
        let legacy: terra_world::environment::SurfaceCondition =
            toml::from_str("wetness = 0.2\ncharred = 0.3\n").unwrap();
        assert_eq!(legacy.snow, 0.0);
        assert_eq!(legacy.fire, 0.0);
        journal
            .set_surface_condition(
                [-2, 0, 15],
                terra_world::environment::SurfaceCondition {
                    snow: 1.0,
                    fire: 0.5,
                    ..Default::default()
                },
            )
            .unwrap();
        journal.record([-1, 0, 15], 0).unwrap();
        let fluid_position = [-2, 1, 15];
        let fluid = terra_world::water::FluidCell {
            amount: 5,
            flow: [-1, 0],
            source: false,
        };
        journal.set_fluid(fluid_position, fluid).unwrap();
        let owner = terra_world::edits::SectionDelta::default().position;
        journal
            .remove_prop(owner, terra_world::props::PropId(u64::MAX))
            .unwrap();
        let planted = terra_world::props::WorldProp {
            id: terra_world::props::PropId(33),
            kind: terra_world::props::PropKind::Flower,
            position: [2.5, 1.0, 2.5],
            rotation: 0.3,
            scale: 1.0,
            variant: 2,
            section: owner,
            removed: false,
        };
        assert!(journal.place_prop(planted).unwrap());
        let uuid = terra_core::PlayerUuid::new_v4();
        let bag = journal.gameplay_mut().inventories.entry(uuid).or_default();
        bag.insert(terra_core::ItemKind::Block(3), 128);
        assert!(bag.move_slot(0, 26, 128));
        bag.creative_cursor(terra_core::ItemStack {
            item: terra_core::ItemKind::Block(19),
            count: 7,
        });
        assert!(
            journal
                .gameplay_mut()
                .drop_one([3.5, 4.0, 2.5], terra_core::ItemKind::Block(4))
        );
        journal.gameplay_mut().drops[0].velocity = [0.1, -2.0, 0.0];
        journal.gameplay_mut().drops[0].age_seconds = 42.0;
        journal.gameplay_mut().drops[0].rotation = [0.0, 0.6, 0.0, 0.8];
        journal.gameplay_mut().drops[0].angular_velocity = [1.0, 2.0, 3.0];
        journal.gameplay_mut().drops[0].pickup_delay_seconds = 0.25;
        save_legacy(&path, id, &journal).unwrap();
        let loaded = load_edits(&path, id).unwrap();
        assert_eq!(loaded.edited_blocks(), 1);
        assert_eq!(loaded.surface_condition([-2, 0, 15]).snow, 1.0);
        assert_eq!(loaded.surface_condition([-2, 0, 15]).fire, 0.5);
        assert_eq!(loaded.fluid(fluid_position), Some(fluid));
        for section in journal.changed_sections() {
            assert_eq!(loaded.fluid_epoch(section), journal.fluid_epoch(section));
        }
        assert!(loaded.prop_removed(owner, terra_world::props::PropId(u64::MAX)));
        assert_eq!(loaded.section_delta(owner).placed_props, [planted]);
        assert_eq!(loaded.gameplay(), journal.gameplay());
        assert!(loaded.gameplay().inventories[&uuid].slot(0).is_none());
        assert_eq!(
            loaded.gameplay().inventories[&uuid].slot(26).unwrap().count,
            128
        );
        assert!(load_edits(&path, WorldId(id.0 + 1)).is_err());
        fs::remove_dir_all(library.root).unwrap();
    }

    #[test]
    fn legacy_edits_without_fluid_fields_load_and_invalid_amounts_are_rejected() {
        let library = library();
        let entry = library
            .create(
                "Legacy",
                Some(7),
                GameMode::Survival,
                &WorldConfig::default(),
            )
            .unwrap();
        let path = library.root.join("legacy-fluid/edits.toml");
        assert!(entry.compatible().is_ok());
        // TOML's generic Value uses signed integers; production serde supports u64 IDs.
        let id = WorldId(7);
        let mut journal = EditJournal::default();
        journal.record([-1, 0, 15], 0).unwrap();
        let section = journal.changed_sections().next().unwrap();
        let mut snapshot = toml::Value::try_from(EditSnapshot {
            save_format_version: 1,
            world: id,
            sections: vec![journal.section_delta(section)],
            gameplay: GameplayState::default(),
        })
        .unwrap();
        let saved_section = snapshot
            .get_mut("sections")
            .unwrap()
            .as_array_mut()
            .unwrap()[0]
            .as_table_mut()
            .unwrap();
        saved_section.remove("fluid_epoch");
        saved_section.remove("fluids");
        save_toml(&snapshot, &path).unwrap();
        let loaded = load_edits(&path, id).unwrap();
        assert_eq!(loaded.edited_blocks(), 1);
        assert_eq!(loaded.fluid_epoch(section), 0);
        assert_eq!(loaded.fluid_cells().count(), 0);

        let invalid_path = library.root.join("invalid-legacy/edits.toml");
        let mut invalid = journal.section_delta(section);
        invalid.fluid_epoch = 1;
        invalid.fluids.push((
            [0, 1, 0],
            terra_world::water::FluidCell {
                amount: 9,
                flow: [0, 0],
                source: false,
            },
        ));
        save_toml(
            &EditSnapshot {
                save_format_version: 1,
                world: id,
                sections: vec![invalid],
                gameplay: GameplayState::default(),
            },
            &invalid_path,
        )
        .unwrap();
        assert!(load_edits(&invalid_path, id).is_err());
        fs::remove_dir_all(library.root).unwrap();
    }

    #[test]
    fn incompatible_worlds_stay_listed_and_manifest_id_mismatch_is_rejected() {
        let library = library();
        let mut entry = library
            .create(
                "Old Valley",
                None,
                GameMode::Survival,
                &WorldConfig::default(),
            )
            .unwrap();
        let id = entry.manifest.world.id;
        entry.game_version = "incompatible".into();
        save_toml(&entry, library.path(id).join("world.toml")).unwrap();
        let (entries, errors) = library.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(errors.is_empty());
        assert!(entries[0].compatible().is_err());
        assert!(library.mark_played(id).is_err());
        entry.manifest.world.id = WorldId(id.0.wrapping_add(1));
        save_toml(&entry, library.path(id).join("world.toml")).unwrap();
        assert!(library.load(id).is_err());
        assert!(library.delete(id).is_err());
        fs::remove_dir_all(library.root).unwrap();
    }

    #[test]
    fn inventories_and_loose_drops_share_the_edit_save_and_legacy_saves_still_load() {
        let library = library();
        let entry = library
            .create(
                "Resources",
                Some(7),
                GameMode::Survival,
                &WorldConfig::default(),
            )
            .unwrap();
        let path = library.root.join("resource-data/edits.toml");
        assert!(entry.compatible().is_ok());
        let id = WorldId(7);
        let uuid = terra_core::PlayerUuid::new_v4();
        let mut journal = EditJournal::default();
        journal.record([0, 0, 0], 0).unwrap();
        journal
            .gameplay_mut()
            .drop_one([0.5, 1.5, 0.5], terra_core::ItemKind::Block(3));
        assert!(journal.gameplay_mut().pickup(uuid, 1));
        journal.gameplay_mut().drop_one(
            [-1.5, 1.5, 0.5],
            terra_core::ItemKind::Prop(terra_core::ResourcePropKind::Flower),
        );
        save_edits(&path, id, &journal).unwrap();
        let loaded = load_edits(&path, id).unwrap();
        assert_eq!(loaded.gameplay(), journal.gameplay());
        assert_eq!(loaded.edited_blocks(), 1);
        let legacy = library.root.join("legacy-inventory/edits.toml");
        save_legacy(&legacy, id, &journal).unwrap();
        let mut snapshot: toml::Value =
            toml::from_str(&fs::read_to_string(&legacy).unwrap()).unwrap();
        snapshot.as_table_mut().unwrap().remove("gameplay");
        save_toml(&snapshot, &legacy).unwrap();
        assert_eq!(
            load_edits(&legacy, id).unwrap().gameplay(),
            &GameplayState::default()
        );
        let mut invalid = journal.gameplay().clone();
        invalid.drops[0].stack.count = terra_core::STACK_LIMIT + 1;
        assert!(journal.import_gameplay(invalid).is_err());
        fs::remove_dir_all(library.root).unwrap();
    }

    #[test]
    fn legacy_metadata_is_backed_up_and_rejects_old_readers_before_region_migration() {
        let library = library();
        let mut entry = library
            .create(
                "Upgrade",
                Some(77),
                GameMode::Creative,
                &WorldConfig::default(),
            )
            .unwrap();
        let id = entry.manifest.world.id;
        let path = library.path(id);
        entry.manifest.save_format_version = 1;
        save_toml(&entry, path.join("world.toml")).unwrap();
        let mut journal = EditJournal::default();
        journal.record([-1, 20, -1], 27).unwrap();
        save_legacy(&path.join("edits.toml"), id, &journal).unwrap();
        let loaded = load_edits(&path, id).unwrap();
        assert_eq!(loaded.block([-1, 20, -1]), Some(27));
        assert_eq!(library.load(id).unwrap().manifest.save_format_version, 2);
        let old: WorldEntry =
            toml::from_str(&fs::read_to_string(path.join("world.toml.legacy-backup")).unwrap())
                .unwrap();
        assert_eq!(old.manifest.save_format_version, 1);
        assert_eq!(old.manifest.world, entry.manifest.world);
        assert!(path.join("edits.toml.legacy-backup").is_file());
        fs::remove_dir_all(library.root).unwrap();
    }
}
