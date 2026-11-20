//! Revisioned bounded immutable surface patches, shared by all
//! vertical sections. Terrain noise, water levels and older revisions are untouched.
use super::{
    SurfaceColumn, TerraGenerator,
    seed::layer_seed,
    structures::Site,
    vegetation::{Tree, trees_in_column},
};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{Arc, Mutex},
};
use terra_voxel::MaterialId;

mod cactus;
mod fantasy;
mod flora;
mod mist;
mod monumental;
mod regional;
#[cfg(test)]
mod regional_tests;
mod rocks;
#[cfg(test)]
mod tests;

pub const TILE_SIZE: i64 = 32;
const CACHE_LIMIT: usize = 128;
type Tile = [i64; 2];
type Cell = [i64; 3];
type CachedPatches = (HashMap<Tile, Arc<NaturalPatch>>, VecDeque<Tile>);

/// Real generated voxel formation, useful for native/debug inspections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RockFormation {
    pub center: Cell,
    pub bounds: [Cell; 2],
    pub material: u16,
    pub voxel_count: usize,
}

/// A connected desert plant made from real, independently mineable voxels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CactusPlant {
    pub origin: Cell,
    pub height: u8,
    pub bounds: [Cell; 2],
    pub voxel_count: usize,
}

#[derive(Default)]
pub(super) struct NaturalCache {
    entries: Mutex<CachedPatches>,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub(super) struct NaturalPatch {
    cells: Vec<(Cell, u16)>,
    rocks: Vec<RockFormation>,
    cacti: Vec<CactusPlant>,
}
impl NaturalPatch {
    pub(super) fn block_at(&self, cell: Cell) -> Option<u16> {
        self.cells
            .binary_search_by_key(&cell, |(p, _)| *p)
            .ok()
            .map(|i| self.cells[i].1)
    }
    pub(super) fn stamp(&self, start: Cell, size: usize, blocks: &mut [u16]) {
        for &(position, material) in &self.cells {
            let local = std::array::from_fn::<_, 3, _>(|axis| position[axis] - start[axis]);
            if local.iter().all(|v| (0..size as i64).contains(v)) {
                let [x, y, z] = local.map(|v| v as usize);
                blocks[x + size * (y + size * z)] = material;
            }
        }
    }
    pub(super) fn maximum(&self) -> Option<i64> {
        self.cells.iter().map(|(p, _)| p[1]).max()
    }
}

impl TerraGenerator {
    /// `tile` uses global 32m coordinates, including Euclidean negative tiles.
    pub fn surface_rock_formations(&self, tile: Tile) -> Vec<RockFormation> {
        self.natural_patch(tile).rocks.clone()
    }
    /// Natural cactus metadata for a global 32m tile, not a 16m section.
    pub fn surface_cacti(&self, tile: Tile) -> Vec<CactusPlant> {
        self.natural_patch(tile).cacti.clone()
    }
    /// Mineable fog sources in a global 32m tile; most lie beneath a grass cap.
    pub fn spirit_mist_sources(&self, tile: Tile) -> Vec<Cell> {
        self.natural_patch(tile)
            .cells
            .iter()
            .filter_map(|(cell, material)| {
                (*material == MaterialId::FogBlock as u16).then_some(*cell)
            })
            .collect()
    }
    /// Actual rooted flora cells, useful for inspecting regional distributions.
    pub fn surface_flora(&self, tile: Tile) -> Vec<(Cell, u16)> {
        self.natural_patch(tile)
            .cells
            .iter()
            .copied()
            .filter(
                |(_, material)| matches!(material, 316..=318 | 322..=323 | 412..=421 | 423..=429),
            )
            .collect()
    }
    pub(super) fn natural_material_at(&self, cell: Cell) -> Option<u16> {
        if self.config().ecology_revision < 5 {
            return None;
        }
        self.natural_patch([cell[0].div_euclid(TILE_SIZE), cell[2].div_euclid(TILE_SIZE)])
            .block_at(cell)
    }
    pub(super) fn natural_patches(&self, start: Cell, size: usize) -> Vec<Arc<NaturalPatch>> {
        if self.config().ecology_revision < 5 {
            return Vec::new();
        }
        let mut patches = Vec::with_capacity(4);
        for z in start[2].div_euclid(TILE_SIZE)..=(start[2] + size as i64 - 1).div_euclid(TILE_SIZE)
        {
            for x in
                start[0].div_euclid(TILE_SIZE)..=(start[0] + size as i64 - 1).div_euclid(TILE_SIZE)
            {
                let patch = self.natural_patch([x, z]);
                if !patch.cells.is_empty() {
                    patches.push(patch);
                }
            }
        }
        patches
    }
    fn natural_patch(&self, tile: Tile) -> Arc<NaturalPatch> {
        if self.config().ecology_revision < 5 || tile.iter().any(|v| v.unsigned_abs() > 31_250_000)
        {
            return Arc::default();
        }
        if let Some(patch) = self
            .natural_features
            .entries
            .lock()
            .ok()
            .and_then(|entries| entries.0.get(&tile).cloned())
        {
            return patch;
        }
        // All planning occurs outside the mutex; concurrent duplicate work is
        // harmless and cannot make generation depend on arrival order.
        let patch = Arc::new(build(self, tile));
        if let Ok(mut entries) = self.natural_features.entries.lock() {
            if let Some(existing) = entries.0.get(&tile) {
                return existing.clone();
            }
            if entries.0.len() >= CACHE_LIMIT
                && let Some(oldest) = entries.1.pop_front()
            {
                entries.0.remove(&oldest);
            }
            entries.0.insert(tile, patch.clone());
            entries.1.push_back(tile);
        }
        patch
    }
}

fn ticket(seed: u64, coordinate: Tile, salt: u64) -> u32 {
    layer_seed(
        seed ^ (coordinate[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
            ^ (coordinate[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
        salt,
    )
}

struct Habitat<'a> {
    terrain: &'a TerraGenerator,
    trees: Vec<Tree>,
    sites: Vec<Arc<Site>>,
    columns: HashMap<Tile, SurfaceColumn>,
}
impl Habitat<'_> {
    fn column(&mut self, point: Tile) -> SurfaceColumn {
        *self.columns.entry(point).or_insert_with(|| {
            self.terrain
                .surface_column(point[0] as f64 + 0.5, point[1] as f64 + 0.5)
        })
    }
    fn ground(&mut self, point: Tile) -> Option<(i64, SurfaceColumn)> {
        let column = self.column(point);
        let ground = (column.height - 0.5).floor() as i64;
        if !column.surface_material.is_natural_ground()
            || column.slope > 2.0
            || column
                .water_level
                .is_some_and(|level| level > ground as f64 + 0.75)
            || column.cave_cavity.is_some()
            || column.river_cavity.is_some()
            || column.cave.open_to_sky()
        {
            return None;
        }
        for depth in [0.0, 1.0] {
            if self
                .terrain
                .cave_density_with_column(
                    [
                        point[0] as f64 + 0.5,
                        ground as f64 + 0.5 - depth,
                        point[1] as f64 + 0.5,
                    ],
                    column,
                )
                .0
                > 0.0
            {
                return None;
            }
        }
        Some((ground, column))
    }
    fn reserved(&self, point: Tile, margin: i64) -> bool {
        self.sites.iter().any(|site| {
            [0, 2].into_iter().enumerate().all(|(i, axis)| {
                point[i] >= site.summary.bounds[0][axis] - margin
                    && point[i] <= site.summary.bounds[1][axis] + margin
            })
        })
    }
    fn root_distance_squared(&self, point: Tile) -> i64 {
        self.trees
            .iter()
            .map(|tree| (tree.origin[0] - point[0]).pow(2) + (tree.origin[2] - point[1]).pow(2))
            .min()
            .unwrap_or(i64::MAX)
    }
    fn tree_occupied(&self, cell: Cell) -> bool {
        self.trees
            .iter()
            .any(|tree| tree.wood_at(cell).is_some() || tree.leaves_at(cell).is_some())
    }
}

fn build(terrain: &TerraGenerator, tile: Tile) -> NaturalPatch {
    if !terrain.config().biomes {
        return NaturalPatch::default();
    }
    let start = [tile[0] * TILE_SIZE, 0, tile[1] * TILE_SIZE];
    let mut habitat = Habitat {
        terrain,
        trees: trees_in_column(terrain, start, TILE_SIZE as usize),
        sites: terrain.structures_for_column([tile[0] * 2, tile[1] * 2], TILE_SIZE as usize),
        columns: HashMap::new(),
    };
    let mut cells = BTreeMap::new();
    let rocks = if terrain.config().ecology_revision >= 9 {
        fantasy::build(&mut habitat, tile, &mut cells)
            .or_else(|| rocks::build(&mut habitat, tile, &mut cells))
    } else {
        rocks::build(&mut habitat, tile, &mut cells)
    }
    .into_iter()
    .collect();
    let cacti = if terrain.config().ecology_revision >= 6 {
        let cacti = cactus::build(&mut habitat, tile, &mut cells);
        regional::build(&mut habitat, tile, &mut cells);
        cacti
    } else {
        Vec::new()
    };
    flora::build(&mut habitat, tile, &mut cells);
    mist::build(&mut habitat, tile, &mut cells);
    NaturalPatch {
        cells: cells.into_iter().collect(),
        rocks,
        cacti,
    }
}
