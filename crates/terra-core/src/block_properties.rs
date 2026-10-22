//! Renderer-independent mechanics read from the same per-block asset JSON.
use crate::{ItemKind, ToolKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolClass {
    Pickaxe,
    Axe,
    Shovel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatDisplay {
    Bar,
    Text,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HudStats {
    pub nectar: Option<StatDisplay>,
    pub honey: Option<StatDisplay>,
    pub occupants: Option<StatDisplay>,
    pub processing: Option<StatDisplay>,
}
pub fn prop_hud_stats() -> Result<BTreeMap<crate::ResourcePropKind, HudStats>, String> {
    #[derive(Deserialize)]
    struct Definition {
        kind: crate::ResourcePropKind,
        #[serde(default)]
        hud_stats: HudStats,
    }
    let mut stats = BTreeMap::new();
    for entry in std::fs::read_dir(crate::asset_directory().join("terra/props"))
        .map_err(|e| e.to_string())?
    {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_none_or(|s| s != "json") {
            continue;
        }
        let d: Definition =
            serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)
                .map_err(|e| format!("{}: {e}", path.display()))?;
        if stats.insert(d.kind, d.hud_stats).is_some() {
            return Err("Duplicated prop HUD definition".into());
        }
    }
    Ok(stats)
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BlockProperties {
    pub hardness: f32,
    #[serde(alias = "resistence")]
    pub resistance: f32,
    pub required_tool: Option<ToolClass>,
    pub mining_level: u8,
    /// Structural objects occupy the cell without smothering its ground surface.
    pub struct_block: bool,
    pub hud_stats: HudStats,
}
impl Default for BlockProperties {
    fn default() -> Self {
        Self {
            hardness: 0.55,
            resistance: 0.5,
            required_tool: None,
            mining_level: 0,
            struct_block: false,
            hud_stats: HudStats::default(),
        }
    }
}
impl BlockProperties {
    pub fn valid(self) -> bool {
        self.hardness.is_finite()
            && (0.05..=3600.0).contains(&self.hardness)
            && self.resistance.is_finite()
            && (0.0..=1_000_000.0).contains(&self.resistance)
            && self.mining_level <= 3
            && (self.mining_level == 0 || self.required_tool.is_some())
    }
    pub fn can_harvest(self, item: Option<ItemKind>) -> bool {
        let Some(required) = self.required_tool else {
            return true;
        };
        let Some(ItemKind::Tool(tool)) = item else {
            return false;
        };
        let (class, level) = match tool {
            ToolKind::WoodPickaxe => (ToolClass::Pickaxe, 1),
            ToolKind::StonePickaxe => (ToolClass::Pickaxe, 2),
            ToolKind::IronPickaxe => (ToolClass::Pickaxe, 3),
            ToolKind::StoneAxe => (ToolClass::Axe, 2),
            ToolKind::StoneShovel => (ToolClass::Shovel, 2),
            ToolKind::WoodAxe => (ToolClass::Axe, 1),
            ToolKind::WoodShovel => (ToolClass::Shovel, 1),
            ToolKind::WoodSword | ToolKind::StoneSword => return false,
        };
        class == required && level >= self.mining_level
    }
}
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct BlockPropertiesCatalog(
    pub BTreeMap<u16, BlockProperties>,
    pub BTreeMap<u16, std::sync::Arc<[crate::block_shapes::ShapeBox]>>,
);
impl BlockPropertiesCatalog {
    pub fn shape(&self, material: u16) -> &[crate::block_shapes::ShapeBox] {
        self.1.get(&material).map_or_else(
            || crate::block_shapes::builtin_boxes(material),
            |boxes| boxes.as_ref(),
        )
    }
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Definition {
            id: u16,
            #[serde(default)]
            collision_boxes: Vec<crate::block_shapes::ShapeBox>,
            #[serde(flatten)]
            properties: BlockProperties,
        }
        let mut result = Self::default();
        for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let definition: Definition =
                serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
            if !definition.properties.valid()
                || definition.collision_boxes.len() > 128
                || !definition
                    .collision_boxes
                    .iter()
                    .all(|shape| shape.valid_for(definition.id))
                || result
                    .0
                    .insert(definition.id, definition.properties)
                    .is_some()
            {
                return Err(format!("Invalid block mechanics: {}", path.display()));
            }
            if !definition.collision_boxes.is_empty() {
                result
                    .1
                    .insert(definition.id, definition.collision_boxes.into());
            }
        }
        if (1..=terra_voxel::MaterialId::LAST).any(|id| !result.0.contains_key(&id)) {
            return Err("Incomplete block mechanics".into());
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_stone_tier_is_not_satisfied_by_wood_or_wrong_tool() {
        let properties = BlockProperties {
            hardness: 2.5,
            resistance: 8.0,
            required_tool: Some(ToolClass::Pickaxe),
            mining_level: 2,
            struct_block: false,
            hud_stats: HudStats::default(),
        };
        assert!(properties.valid());
        assert!(!properties.can_harvest(None));
        assert!(!properties.can_harvest(Some(ItemKind::Tool(ToolKind::WoodPickaxe))));
        assert!(!properties.can_harvest(Some(ItemKind::Tool(ToolKind::StoneAxe))));
        assert!(properties.can_harvest(Some(ItemKind::Tool(ToolKind::StonePickaxe))));
        let crystals = BlockProperties {
            mining_level: 3,
            ..properties
        };
        assert!(crystals.valid());
        assert!(!crystals.can_harvest(Some(ItemKind::Tool(ToolKind::StonePickaxe))));
        assert!(crystals.can_harvest(Some(ItemKind::Tool(ToolKind::IronPickaxe))));
        assert!(
            !BlockProperties {
                hardness: f32::NAN,
                ..properties
            }
            .valid()
        );
        assert!(
            !BlockProperties {
                mining_level: 4,
                ..properties
            }
            .valid()
        );
    }
    #[test]
    fn all_mechanics_are_read_from_individual_shipped_block_files() {
        let catalog =
            BlockPropertiesCatalog::load(&crate::asset_directory().join("terra/blocks")).unwrap();
        assert_eq!(catalog.0.len(), usize::from(terra_voxel::MaterialId::LAST));
        assert!(catalog.0[&3].hardness > catalog.0[&7].hardness);
        assert!(catalog.0[&28].struct_block);
        assert!(catalog.0[&29].struct_block);
        assert!(catalog.0[&40].struct_block);
        assert!(!catalog.0[&3].struct_block);
    }
    #[test]
    fn legacy_definitions_default_to_terrain_covering_blocks() {
        let properties: BlockProperties = serde_json::from_str("{}").unwrap();
        assert!(!properties.struct_block);
        assert!(properties.valid());
    }
}
