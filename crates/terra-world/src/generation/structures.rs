//! Bounded immutable regional plans, stamped by the existing section generator.
use super::{TerraGenerator, seed::layer_seed};
use crate::edits::BlockObjectState;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};
use terra_core::Inventory;
use terra_voxel::{SectionPos, global_to_section};

mod build;
#[cfg(test)]
mod lookup_tests;
mod loot;
mod mine;
mod settlement;
pub use settlement::{ResidentSite, VillageBed, VillageResidence, VillageSettlement, VillageSize};
#[cfg(test)]
mod tests;

pub const REGION_SIZE: i64 = 384;
const CACHE_LIMIT: usize = 64;
pub const MAX_SITE_SECTIONS: usize = 256;
pub const MAX_SITE_CELLS: usize = 300_000;
const BUILDING_TREE_CLEARANCE: i64 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureKind {
    Village,
    Ruin,
    Mine,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VillageStyle {
    Temperate,
    Desert,
    Highland,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingRole {
    House,
    Church,
    Smith,
    Watch,
    Medic,
    Farm,
    Bridge,
    Ruin,
    Mine,
    Police,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingSummary {
    pub role: BuildingRole,
    pub entrance: [i64; 3],
    pub bounds: [[i64; 3]; 2],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteSummary {
    pub id: u64,
    pub kind: StructureKind,
    pub style: VillageStyle,
    pub center: [i64; 3],
    pub bounds: [[i64; 3]; 2],
    pub buildings: Vec<BuildingSummary>,
    pub containers: Vec<[i64; 3]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureObject {
    pub position: [i64; 3],
    pub material: u16,
    pub state: Option<BlockObjectState>,
    pub loot: Option<Inventory>,
    pub crop: Option<terra_core::CropKind>,
}

#[derive(Debug)]
pub(crate) struct StructurePatch {
    pub cells: Vec<(u16, u16)>,
    pub objects: Vec<StructureObject>,
}
#[derive(Debug)]
pub(crate) struct Site {
    pub summary: SiteSummary,
    pub sections: BTreeMap<SectionPos, StructurePatch>,
    pub settlement: Option<Arc<VillageSettlement>>,
}
impl Site {
    pub fn block_at(&self, global: [i64; 3]) -> Option<u16> {
        let (owner, local) = global_to_section(global);
        let index = (local[0] + 16 * (local[1] + 16 * local[2])) as u16;
        let cells = &self.sections.get(&owner)?.cells;
        cells
            .binary_search_by_key(&index, |&(i, _)| i)
            .ok()
            .map(|i| cells[i].1)
    }
    pub fn stamp(&self, start: [i64; 3], size: usize, padded: &mut [u16]) {
        let low = global_to_section(start).0;
        let high = global_to_section(start.map(|p| p + size as i64 - 1)).0;
        for z in low.z..=high.z {
            for y in low.y..=high.y {
                for x in low.x..=high.x {
                    let owner = SectionPos::new(x, y, z);
                    let Some(patch) = self.sections.get(&owner) else {
                        continue;
                    };
                    let Ok(origin) = owner.origin() else { continue };
                    for &(index, material) in &patch.cells {
                        let i = i64::from(index);
                        let local = [
                            origin[0] + i % 16 - start[0],
                            origin[1] + i / 16 % 16 - start[1],
                            origin[2] + i / 256 - start[2],
                        ];
                        if local.iter().all(|v| (0..size as i64).contains(v)) {
                            padded[local[0] as usize
                                + size * (local[1] as usize + size * local[2] as usize)] = material;
                        }
                    }
                }
            }
        }
    }
    pub fn clears_tree(&self, root: [i64; 3]) -> bool {
        self.block_at(root).is_some()
            || self.summary.buildings.iter().any(|b| {
                b.role != BuildingRole::Mine
                    && root[0] >= b.bounds[0][0] - BUILDING_TREE_CLEARANCE
                    && root[0] <= b.bounds[1][0] + BUILDING_TREE_CLEARANCE
                    && root[2] >= b.bounds[0][2] - BUILDING_TREE_CLEARANCE
                    && root[2] <= b.bounds[1][2] + BUILDING_TREE_CLEARANCE
            })
    }
}

type Slot = Arc<OnceLock<Option<Arc<Site>>>>;
#[derive(Default)]
struct Entries {
    slots: HashMap<[i64; 2], Slot>,
    order: VecDeque<[i64; 2]>,
}
#[derive(Default)]
pub(super) struct StructureCache(Mutex<Entries>);
impl TerraGenerator {
    fn planned_site(&self, region: [i64; 2]) -> Option<Arc<Site>> {
        if self.config().structures_revision == 0 || region.iter().any(|v| v.unsigned_abs() > 2500)
        {
            return None;
        }
        let slot = {
            let mut cache = self.structures.0.lock().ok()?;
            if let Some(slot) = cache.slots.get(&region) {
                slot.clone()
            } else {
                while cache.slots.len() >= CACHE_LIMIT {
                    if let Some(old) = cache.order.pop_front() {
                        cache.slots.remove(&old);
                    } else {
                        break;
                    }
                }
                let slot = Slot::default();
                cache.order.push_back(region);
                cache.slots.insert(region, slot.clone());
                slot
            }
        };
        slot.get_or_init(|| {
            let _scope = terra_core::profiling::ScopeTimer::new("structures.plan");
            let seed = layer_seed(
                self.config().seed.0
                    ^ (region[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                    ^ (region[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
                0x573731,
            );
            build::plan(self, region, seed).map(Arc::new)
        })
        .clone()
    }
    /// Exact immutable plan used for generation, suitable for native/debug views.
    pub fn structure_site(&self, region: [i64; 2]) -> Option<SiteSummary> {
        self.planned_site(region).map(|s| s.summary.clone())
    }
    /// Explicit regional lookup, cached by the same immutable structure plan.
    /// Runtime NPC updates should use PreparedColumn's loaded-only iterators.
    pub fn village_settlement(&self, region: [i64; 2]) -> Option<Arc<VillageSettlement>> {
        self.planned_site(region)?.settlement.clone()
    }
    pub fn village_residents(&self, region: [i64; 2]) -> Vec<ResidentSite> {
        self.village_settlement(region)
            .map_or_else(Vec::new, |v| v.residents.clone())
    }
    pub(crate) fn structures_for_column(
        &self,
        coordinate: [i64; 2],
        padding: usize,
    ) -> Vec<Arc<Site>> {
        if self.config().structures_revision == 0 {
            return Vec::new();
        }
        let low = coordinate.map(|v| v * 16 - padding as i64);
        let high = coordinate.map(|v| v * 16 + 15 + padding as i64);
        let region = low.map(|v| v.div_euclid(REGION_SIZE));
        let mut sites = Vec::new();
        for z in region[1] - 1..=region[1] + 1 {
            for x in region[0] - 1..=region[0] + 1 {
                // Sites occupy the interior of a region. Avoid building nine
                // plans just to discover that an ordinary column misses them.
                if [x, z]
                    .into_iter()
                    .enumerate()
                    .any(|(i, r)| high[i] < r * REGION_SIZE + 96 || low[i] > r * REGION_SIZE + 288)
                {
                    continue;
                }
                if let Some(site) = self.planned_site([x, z])
                    && [0, 2].into_iter().enumerate().all(|(i, axis)| {
                        site.summary.bounds[0][axis] <= high[i]
                            && site.summary.bounds[1][axis] >= low[i]
                    })
                {
                    sites.push(site);
                }
            }
        }
        sites
    }
    pub(crate) fn structure_block(&self, global: [i64; 3]) -> Option<u16> {
        if self.config().structures_revision == 0 {
            return None;
        }
        // A point can belong to only one interior site envelope. Unlike column
        // preparation, this hot path needs neither nine region probes nor a Vec.
        let horizontal = [global[0], global[2]];
        if horizontal
            .iter()
            .any(|v| !(96..=288).contains(&v.rem_euclid(REGION_SIZE)))
        {
            return None;
        }
        self.planned_site(horizontal.map(|v| v.div_euclid(REGION_SIZE)))?
            .block_at(global)
    }
    pub(crate) fn structure_clears_tree(&self, root: [i64; 3]) -> bool {
        self.structures_for_column(
            [root[0].div_euclid(16), root[2].div_euclid(16)],
            BUILDING_TREE_CLEARANCE as usize,
        )
        .iter()
        .any(|site| site.clears_tree(root))
    }
}
