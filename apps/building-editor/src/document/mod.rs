//! Exact editable voxel document. The workspace support platform is never authored data.
mod export;
mod files;
#[cfg(test)]
mod tests;

use bevy::prelude::Resource;
pub use files::{list_directory, save_template};
use std::{collections::BTreeMap, path::PathBuf};
use terra_core::building_jobs::template::{BuildingTemplate, TemplateCategory};
use terra_world::edits::BlockObjectState;

pub const MINIMUM: [i64; 3] = [-4, -2, -6];
pub const MAXIMUM: [i64; 3] = [4, 9, 4];
pub use crate::workspace::PLATFORM_Y;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredCell {
    pub material: u16,
    pub state: BlockObjectState,
}
impl AuthoredCell {
    /// Ordinary cubes have no object-state entry in the real edit journal.
    /// Their implicit identity pose is valid without inventing an object state.
    pub fn valid_at(&self, position: [i64; 3]) -> bool {
        self.state.valid_for(position, self.material)
            || self.state == BlockObjectState::default()
                && (!terra_world::placement::furniture(self.material)
                    || terra_core::textiles::is_carpet(self.material))
                && !terra_core::locomotive_parts::is_part(self.material)
                && !terra_core::lantern::is_part(self.material)
    }
}

/// Schema 1 stores native material variants, without a separate wood override.
pub fn native_wood(material: u16) -> terra_core::WoodSpecies {
    if terra_core::ItemKind::supports_wood(material) {
        terra_core::ItemKind::Block(material)
            .wood_species()
            .unwrap_or_default()
    } else {
        terra_core::WoodSpecies::default()
    }
}

#[derive(Resource, Clone)]
pub struct Document {
    pub id: String,
    pub name: String,
    pub category: TemplateCategory,
    pub anchor: [i64; 3],
    pub cells: BTreeMap<[i64; 3], AuthoredCell>,
    pub path: Option<PathBuf>,
    pub revision: u64,
    pub saved_revision: u64,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            id: "new-building".into(),
            name: "New building".into(),
            category: TemplateCategory::Residential,
            anchor: [0; 3],
            cells: BTreeMap::new(),
            path: None,
            revision: 0,
            saved_revision: 0,
        }
    }
}
impl Document {
    pub fn contains(&self, position: [i64; 3]) -> bool {
        (0..3).all(|axis| {
            (MINIMUM[axis] + self.anchor[axis]..=MAXIMUM[axis] + self.anchor[axis])
                .contains(&position[axis])
        })
    }
    pub fn dirty(&self) -> bool {
        self.revision != self.saved_revision
    }
    pub fn from_template(
        template: BuildingTemplate,
        path: Option<PathBuf>,
    ) -> Result<Self, String> {
        template.validate()?;
        let mut cells = BTreeMap::new();
        for cell in template.steps.iter().flat_map(|step| &step.cells) {
            let state = BlockObjectState {
                facing: cell.facing,
                peer: cell.peer,
                secondary: cell.secondary,
                wood: native_wood(cell.material),
                ..Default::default()
            };
            let authored = AuthoredCell {
                material: cell.material,
                state,
            };
            if !authored.valid_at(cell.position) {
                return Err(format!(
                    "Unsupported saved object state at {:?}",
                    cell.position
                ));
            }
            cells.insert(cell.position, authored);
        }
        Ok(Self {
            id: template.id,
            name: template.display_name,
            category: template.category,
            anchor: template.anchor,
            cells,
            path,
            revision: 0,
            saved_revision: 0,
        })
    }
    pub fn template(&self) -> Result<BuildingTemplate, String> {
        export::template(self)
    }
}
