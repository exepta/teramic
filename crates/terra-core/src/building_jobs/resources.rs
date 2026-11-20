//! Authoritative town inventory totals and bounded server-authored previews.
mod stock_items;
use super::*;
use crate::ItemKind;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_TOWN_STOCK_ITEMS: usize = 1024;
pub const MAX_TOWN_CHESTS: usize = 4096;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TownStock {
    pub configured: bool,
    /// False when some selected storage is unloaded or the bounded scan is incomplete.
    pub complete: bool,
    /// Logical containers: a valid double chest contributes one, both inventories count.
    pub chests: u16,
    #[serde(with = "stock_items")]
    pub items: BTreeMap<ItemKind, u32>,
    pub wood: u32,
    pub stone: u32,
    pub iron: u32,
    pub coal: u32,
}
impl TownStock {
    pub fn count(&self, item: ItemKind) -> u32 {
        self.items.get(&item).copied().unwrap_or(0)
    }
    pub fn valid(&self) -> bool {
        let total: u64 = self.items.values().map(|n| u64::from(*n)).sum();
        self.items.len() <= MAX_TOWN_STOCK_ITEMS
            && usize::from(self.chests) <= MAX_TOWN_CHESTS
            && self.items.iter().all(|(item, count)| {
                item.valid() && *count > 0 && *count <= 2 * MAX_TOWN_CHESTS as u32 * 18 * 128
            })
            && total <= 2 * MAX_TOWN_CHESTS as u64 * 18 * 128
            && (!self.complete || self.configured)
            && [self.wood, self.stone, self.iron, self.coal]
                .into_iter()
                .all(|n| u64::from(n) <= total)
    }
    pub fn summarize(&mut self) {
        self.wood = 0;
        self.stone = 0;
        self.iron = 0;
        self.coal = 0;
        for (item, count) in &self.items {
            match item.block_material() {
                Some(6 | 10 | 16..=18 | 36 | 134 | 135) => self.wood += count,
                Some(3 | 11..=13 | 43 | 57 | 58) => self.stone += count,
                Some(31) => self.iron += count,
                Some(32) => self.coal += count,
                _ => {}
            }
            match item {
                ItemKind::Mineral(crate::MineralItem::RawIron | crate::MineralItem::IronIngot) => {
                    self.iron += count
                }
                ItemKind::Mineral(crate::MineralItem::Coal) => self.coal += count,
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplatePreviewCell {
    pub position: [i8; 3],
    pub material: u16,
    pub facing: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplatePreview {
    pub id: String,
    pub cells: Vec<TemplatePreviewCell>,
}
impl TemplatePreview {
    pub fn from_template(template: &template::BuildingTemplate) -> Result<Self, String> {
        template.validate()?;
        let cells = template
            .steps
            .iter()
            .flat_map(|s| &s.cells)
            .map(|c| TemplatePreviewCell {
                position: std::array::from_fn(|i| (c.position[i] - template.anchor[i]) as i8),
                material: c.material,
                facing: c.facing,
            })
            .collect();
        Ok(Self {
            id: template.id.clone(),
            cells,
        })
    }
    pub fn valid(&self) -> bool {
        let mut seen = BTreeSet::new();
        template::valid_template_id(&self.id)
            && !self.cells.is_empty()
            && self.cells.len() <= 2 * MAX_BUILD_STEPS
            && self.cells.iter().all(|c| {
                (-4..=4).contains(&c.position[0])
                    && (-2..=9).contains(&c.position[1])
                    && (-6..=4).contains(&c.position[2])
                    && c.material != 0
                    && terra_voxel::MaterialId::from_raw(c.material).is_some()
                    && template::rotated_facing(
                        &template::TemplateCell {
                            position: c.position.map(i64::from),
                            material: c.material,
                            facing: c.facing,
                            rotate_facing: false,
                            peer: None,
                            secondary: false,
                        },
                        0,
                    )
                    .is_ok()
                    && seen.insert(c.position)
            })
    }
}
