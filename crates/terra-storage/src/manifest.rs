//! World identity and an explicit storage-format upgrade guard.
use serde::{Deserialize, Serialize};
use terra_core::WorldConfig;

pub const SAVE_FORMAT_VERSION: u32 = 2;
/// Persist complete generator settings; storage upgrades never reseed a world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldManifest {
    pub save_format_version: u32,
    pub world: WorldConfig,
}
impl WorldManifest {
    pub fn new(world: WorldConfig) -> Self {
        Self {
            save_format_version: SAVE_FORMAT_VERSION,
            world,
        }
    }
}

pub(crate) fn save_rules(
    root: &std::path::Path,
    rules: terra_core::WorldRuleSettings,
) -> Result<(), String> {
    if !rules.valid() {
        return Err("Invalid saved gamerules".into());
    }
    let path = root.join("world.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if let Ok(mut entry) = toml::from_str::<crate::WorldEntry>(&text) {
        if terra_core::WorldRuleSettings::from_world(&entry.manifest.world) == rules {
            return Ok(());
        }
        rules.apply(&mut entry.manifest.world);
        terra_core::save_toml(&entry, path).map_err(|e| e.to_string())
    } else {
        let mut manifest: WorldManifest = toml::from_str(&text).map_err(|e| e.to_string())?;
        if terra_core::WorldRuleSettings::from_world(&manifest.world) == rules {
            return Ok(());
        }
        rules.apply(&mut manifest.world);
        terra_core::save_toml(&manifest, path).map_err(|e| e.to_string())
    }
}

pub(crate) fn protect_metadata(
    root: &std::path::Path,
    world: terra_core::WorldId,
) -> Result<(), String> {
    use terra_core::save_toml;
    let path = root.join("world.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    if std::fs::symlink_metadata(&path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("World metadata cannot be a symbolic link".into());
    }
    let backup = root.join("world.toml.legacy-backup");
    if let Ok(mut entry) = toml::from_str::<crate::WorldEntry>(&text) {
        if entry.manifest.world.id != world
            || !matches!(entry.manifest.save_format_version, 1 | SAVE_FORMAT_VERSION)
        {
            return Err("Incompatible world metadata".into());
        }
        if entry.manifest.save_format_version == 1 {
            if !backup.exists() {
                save_toml(&entry, &backup).map_err(|e| e.to_string())?;
            }
            entry.manifest.save_format_version = SAVE_FORMAT_VERSION;
            save_toml(&entry, path).map_err(|e| e.to_string())?;
        }
    } else {
        let mut manifest: WorldManifest = toml::from_str(&text).map_err(|e| e.to_string())?;
        if manifest.world.id != world
            || !matches!(manifest.save_format_version, 1 | SAVE_FORMAT_VERSION)
        {
            return Err("Incompatible world metadata".into());
        }
        if manifest.save_format_version == 1 {
            if !backup.exists() {
                save_toml(&manifest, &backup).map_err(|e| e.to_string())?;
            }
            manifest.save_format_version = SAVE_FORMAT_VERSION;
            save_toml(&manifest, path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
