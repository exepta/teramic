//! Stable species and exact ownership of player-grown trees, shared by saves and replicas.
use crate::{PlayerUuid, ResourcePropKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub const MAX_TREES: usize = 8192;
pub const MAX_TREE_WOOD: usize = 4096;
pub const MAX_TREE_LEAVES: usize = 16384;
pub const MAX_TREE_CELLS: usize = MAX_TREE_WOOD + MAX_TREE_LEAVES;

/// Placement only checks the miniature plant; light and crown space govern later growth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaplingPlacementError {
    Soil,
    Unloaded,
    Occupied,
    Water,
    Protected,
    Limit,
    MissingItem,
    Changed,
}
impl SaplingPlacementError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::Soil => "Setzlinge brauchen Gras, Erde oder Ackerboden.",
            Self::Unloaded => "Der Pflanzplatz wird noch geladen. Bitte kurz warten.",
            Self::Occupied => "Am Pflanzplatz steht bereits ein Block oder eine gesetzte Pflanze.",
            Self::Water => "Setzlinge können nicht im Wasser gepflanzt werden.",
            Self::Protected => "Du darfst hier nicht pflanzen oder das vorhandene Gras entfernen.",
            Self::Limit => "Hier können derzeit keine weiteren Pflanzen gespeichert werden.",
            Self::MissingItem => "Du hast diesen Setzling nicht im Inventar.",
            Self::Changed => "Der Pflanzplatz hat sich geändert. Bitte erneut versuchen.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeSpecies {
    Oak,
    Spruce,
    Birch,
    Cherry,
    Apple,
}
impl TreeSpecies {
    pub const ALL: [Self; 5] = [
        Self::Oak,
        Self::Spruce,
        Self::Birch,
        Self::Cherry,
        Self::Apple,
    ];
    pub const fn sapling(self) -> ResourcePropKind {
        match self {
            Self::Oak => ResourcePropKind::OakSapling,
            Self::Spruce => ResourcePropKind::SpruceSapling,
            Self::Birch => ResourcePropKind::BirchSapling,
            Self::Cherry => ResourcePropKind::CherrySapling,
            Self::Apple => ResourcePropKind::AppleSapling,
        }
    }
    pub const fn from_sapling(kind: ResourcePropKind) -> Option<Self> {
        Some(match kind {
            ResourcePropKind::OakSapling => Self::Oak,
            ResourcePropKind::SpruceSapling => Self::Spruce,
            ResourcePropKind::BirchSapling => Self::Birch,
            ResourcePropKind::CherrySapling => Self::Cherry,
            ResourcePropKind::AppleSapling => Self::Apple,
            _ => return None,
        })
    }
    pub const fn wood(self) -> u16 {
        match self {
            Self::Spruce => 36,
            Self::Birch => 10,
            Self::Cherry => 134,
            _ => 6,
        }
    }
    pub const fn leaves(self) -> u16 {
        match self {
            Self::Birch => 9,
            Self::Cherry => 132,
            Self::Apple => 195,
            _ => 7,
        }
    }
    /// Active daylight growth seconds, deliberately longer for conifers.
    pub const fn growth_seconds(self) -> f32 {
        match self {
            Self::Oak => 900.,
            Self::Spruce => 1500.,
            Self::Birch => 1100.,
            Self::Cherry => 1200.,
            Self::Apple => 1400.,
        }
    }
    /// Suggested mature-tree spacing, never a restriction on planting a sapling.
    pub const fn spacing(self) -> i64 {
        match self {
            Self::Spruce => 12,
            Self::Birch | Self::Oak | Self::Cherry => 16,
            Self::Apple => 12,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeStage {
    Sapling,
    Mature,
    Felled,
    /// Appended: full generator form; legacy Mature retains its historic style.
    Generated,
    GeneratedFelled,
}
impl TreeStage {
    pub const fn is_mature(self) -> bool {
        matches!(self, Self::Mature | Self::Generated)
    }
    pub const fn is_generated(self) -> bool {
        matches!(self, Self::Generated | Self::GeneratedFelled)
    }
    pub const fn retired(self) -> Self {
        if self.is_generated() {
            Self::GeneratedFelled
        } else {
            Self::Felled
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeGeometryKey {
    pub root: [i64; 3],
    pub species: TreeSpecies,
    pub seed: u64,
    pub stage: TreeStage,
    pub bounds: Option<[[i8; 3]; 2]>,
    pub wood: usize,
    pub leaves: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FruitPhase {
    Dormant,
    Blossom,
    Unripe,
    Ripe,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FruitState {
    pub phase: FruitPhase,
    pub remaining: u8,
    pub picked: u8,
    pub cycle_day: u64,
    pub last_harvest_day: Option<u64>,
    pub revision: u64,
}
impl FruitState {
    pub fn new(day: u64) -> Self {
        Self {
            phase: FruitPhase::Dormant,
            remaining: 0,
            picked: 0,
            cycle_day: day,
            last_harvest_day: None,
            revision: 0,
        }
    }
    pub fn valid(&self) -> bool {
        self.remaining <= 8
            && if self.phase == FruitPhase::Ripe {
                self.remaining == 8 - self.picked.count_ones() as u8
            } else {
                self.remaining == 0 && self.picked == 0
            }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeRecord {
    pub id: u64,
    /// Root is the first trunk cell, one above the planting support.
    pub root: [i64; 3],
    pub species: TreeSpecies,
    pub seed: u64,
    /// None denotes a natural fruit tree registered by deterministic world generation.
    pub planted_by: Option<PlayerUuid>,
    pub stage: TreeStage,
    pub growth_seconds: f32,
    /// Exact surviving generated cells; player replacement relinquishes attribution.
    /// Exact local bounds of surviving attributed cells; no global per-cell cache.
    pub bounds: Option<[[i8; 3]; 2]>,
    pub wood: Vec<[i8; 3]>,
    pub leaves: Vec<[i8; 3]>,
    pub fruit: Option<FruitState>,
    pub revision: u64,
}
impl TreeRecord {
    pub fn valid(&self) -> bool {
        self.id != 0
            && self.root.iter().all(|v| v.unsigned_abs() <= 1_000_000)
            && self.planted_by.is_none_or(|p| !p.is_nil())
            && self.growth_seconds.is_finite()
            && (0.0..=86_400.0).contains(&self.growth_seconds)
            && self.wood.len() <= MAX_TREE_WOOD
            && self.leaves.len() <= MAX_TREE_LEAVES
            && self.wood.len() + self.leaves.len() <= MAX_TREE_CELLS
            && self
                .fruit
                .is_none_or(|f| self.species == TreeSpecies::Apple && f.valid())
            && (self.stage.is_mature() || self.fruit.is_none())
            && (self.stage != TreeStage::Sapling
                || self.planted_by.is_some() && self.wood.is_empty() && self.leaves.is_empty())
            && (!self.stage.is_mature() || self.planted_by.is_none() || !self.wood.is_empty())
            && (self.wood.is_empty() && self.leaves.is_empty() || self.bounds.is_some())
            && self
                .bounds
                .is_none_or(|b| (0..3).all(|a| b[0][a] <= b[1][a]))
            && {
                let mut cells = HashSet::new();
                let mut actual: Option<[[i8; 3]; 2]> = None;
                let valid = self.wood.iter().chain(&self.leaves).all(|p| {
                    if !cells.insert(*p) || p.iter().any(|v| v.unsigned_abs() > 64) {
                        return false;
                    }
                    let bounds = actual.get_or_insert([*p; 2]);
                    for axis in 0..3 {
                        bounds[0][axis] = bounds[0][axis].min(p[axis]);
                        bounds[1][axis] = bounds[1][axis].max(p[axis]);
                    }
                    true
                });
                valid && actual == self.bounds
            }
    }
    /// Refresh only the affected tree after growth/removal. Other trees need no voxel scan.
    pub fn refresh_bounds(&mut self) {
        self.bounds = self.wood.first().or(self.leaves.first()).map(|first| {
            let mut b = [*first; 2];
            for p in self.wood.iter().chain(&self.leaves) {
                for a in 0..3 {
                    b[0][a] = b[0][a].min(p[a]);
                    b[1][a] = b[1][a].max(p[a]);
                }
            }
            b
        });
    }
    pub fn with_bounds(mut self) -> Self {
        self.refresh_bounds();
        self
    }
    pub fn could_own(&self, cell: [i64; 3]) -> bool {
        self.bounds.is_some_and(|b| {
            (0..3).all(|a| {
                let Some(relative) = cell[a].checked_sub(self.root[a]) else {
                    return false;
                };
                (i64::from(b[0][a])..=i64::from(b[1][a])).contains(&relative)
            })
        })
    }
    pub fn geometry_key(&self) -> TreeGeometryKey {
        TreeGeometryKey {
            root: self.root,
            species: self.species,
            seed: self.seed,
            stage: self.stage,
            bounds: self.bounds,
            wood: self.wood.len(),
            leaves: self.leaves.len(),
        }
    }
    pub fn global(&self, offset: [i8; 3]) -> [i64; 3] {
        std::array::from_fn(|i| self.root[i] + i64::from(offset[i]))
    }
    pub fn offset(&self, cell: [i64; 3]) -> Option<[i8; 3]> {
        Some([
            i8::try_from(cell[0].checked_sub(self.root[0])?).ok()?,
            i8::try_from(cell[1].checked_sub(self.root[1])?).ok()?,
            i8::try_from(cell[2].checked_sub(self.root[2])?).ok()?,
        ])
    }
    pub fn owns(&self, cell: [i64; 3], material: u16) -> bool {
        self.could_own(cell)
            && self.offset(cell).is_some_and(|p| {
                (material == self.species.wood() && self.wood.contains(&p))
                    || (material == self.species.leaves() && self.leaves.contains(&p))
            })
    }
}
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeRegistry {
    pub revision: u64,
    pub trees: BTreeMap<u64, TreeRecord>,
}
impl TreeRegistry {
    pub fn valid(&self) -> bool {
        self.trees.len() <= MAX_TREES && self.trees.iter().all(|(id, t)| *id == t.id && t.valid())
    }
    pub fn get(&self, id: u64) -> Option<&TreeRecord> {
        self.trees.get(&id)
    }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut TreeRecord> {
        self.trees.get_mut(&id)
    }
    pub fn insert(&mut self, tree: TreeRecord) -> bool {
        if !tree.valid()
            || self.trees.contains_key(&tree.id)
            || self.trees.len() >= MAX_TREES
            || self.revision == u64::MAX
        {
            return false;
        }
        self.trees.insert(tree.id, tree);
        self.changed();
        true
    }
    pub fn changed(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }
    pub fn owner(&self, cell: [i64; 3], material: u16) -> Option<&TreeRecord> {
        self.trees.values().find(|t| t.owns(cell, material))
    }
    /// Any ordinary replacement, including equal-looking player wood, stops being tree-owned.
    pub fn relinquish(&mut self, cell: [i64; 3]) -> usize {
        let mut changed = false;
        let mut candidates = 0;
        for tree in self.trees.values_mut() {
            // Severing any fruit tree root retires fruit immediately, including when
            // branches survive and equal-looking player wood is placed before a tick.
            if tree.species == TreeSpecies::Apple && tree.root == cell && tree.stage.is_mature() {
                tree.stage = tree.stage.retired();
                tree.fruit = None;
                tree.revision = tree.revision.saturating_add(1);
                changed = true;
            }
            if !tree.could_own(cell) {
                continue;
            }
            candidates += 1;
            let Some(offset) = tree.offset(cell) else {
                continue;
            };
            let before = tree.wood.len() + tree.leaves.len();
            tree.wood.retain(|v| *v != offset);
            tree.leaves.retain(|v| *v != offset);
            if before != tree.wood.len() + tree.leaves.len() {
                if tree.wood.is_empty() && tree.stage.is_mature() {
                    tree.stage = tree.stage.retired();
                    tree.fruit = None;
                }
                tree.refresh_bounds();
                tree.revision = tree.revision.saturating_add(1);
                changed = true;
            }
        }
        if changed {
            self.changed();
        }
        candidates
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn species_and_exact_replacement_are_stable() {
        for s in TreeSpecies::ALL {
            assert_eq!(TreeSpecies::from_sapling(s.sapling()), Some(s));
        }
        let mut r = TreeRegistry::default();
        assert!(
            r.insert(
                TreeRecord {
                    id: 1,
                    root: [-1, 15, 16],
                    species: TreeSpecies::Oak,
                    seed: 7,
                    planted_by: Some(PlayerUuid::from_u128(1)),
                    stage: TreeStage::Mature,
                    growth_seconds: 900.,
                    bounds: None,
                    wood: vec![[0, 0, 0], [0, 1, 0]],
                    leaves: vec![[1, 2, 0]],
                    fruit: None,
                    revision: 1
                }
                .with_bounds()
            )
        );
        assert_eq!(r.owner([-1, 15, 16], 6).unwrap().id, 1);
        assert!(r.owner([-1, 15, 16], 36).is_none());
        r.relinquish([-1, 15, 16]);
        assert!(r.owner([-1, 15, 16], 6).is_none());
        assert!(r.valid());
        r.relinquish([-1, 16, 16]);
        assert_eq!(r.get(1).unwrap().stage, TreeStage::Felled);
    }
}

#[cfg(test)]
mod spatial_tests {
    use super::*;
    fn grown(id: u64, root: [i64; 3]) -> TreeRecord {
        TreeRecord {
            id,
            root,
            species: TreeSpecies::Oak,
            seed: id,
            planted_by: Some(PlayerUuid::from_u128(1)),
            stage: TreeStage::Mature,
            growth_seconds: 900.,
            bounds: None,
            wood: vec![[0, 0, 0], [0, 1, 0]],
            leaves: vec![[1, 2, 0]],
            fruit: None,
            revision: 0,
        }
        .with_bounds()
    }
    #[test]
    fn ordinary_edit_uses_only_local_bounds_among_thousands_of_distant_trees() {
        let mut registry = TreeRegistry::default();
        for id in 1..=4096 {
            assert!(registry.insert(grown(id, [id as i64 * 16, 16, -32])));
        }
        assert!(registry.insert(grown(5000, [0, 16, 0])));
        assert_eq!(
            registry.relinquish([0, 16, 0]),
            1,
            "only the geometrically local footprint can be retained/scanned"
        );
        assert!(
            registry.owner([0, 16, 0], 6).is_none(),
            "an identical-looking player log relinquishes tree ownership"
        );
        assert!(registry.owner([0, 17, 0], 6).is_some());
        assert_eq!(registry.relinquish([0, 18, 14]), 0);
        for id in 1..=4096 {
            assert_eq!(registry.get(id).unwrap().wood.len(), 2);
        }
        assert!(registry.valid());
    }
    #[test]
    fn invalid_bounds_cannot_hide_cells_from_replacement_attribution() {
        let mut tree = grown(1, [0, 16, 0]);
        tree.bounds = Some([[0, 0, 0], [0, 1, 0]]);
        assert!(!tree.valid());
        tree.refresh_bounds();
        assert!(tree.valid());
    }
}

#[cfg(test)]
mod natural_root_tests {
    use super::*;
    #[test]
    fn natural_apple_root_edit_retires_fruit_even_when_same_wood_is_restored() {
        let mut r = TreeRegistry::default();
        assert!(r.insert(TreeRecord {
            id: 77,
            root: [8, 16, 8],
            species: TreeSpecies::Apple,
            seed: 77,
            planted_by: None,
            stage: TreeStage::Mature,
            growth_seconds: 0.,
            bounds: None,
            wood: vec![],
            leaves: vec![],
            fruit: Some(FruitState::new(2)),
            revision: 0
        }));
        let revision = r.revision;
        assert_eq!(
            r.relinquish([8, 16, 8]),
            0,
            "empty natural metadata needs no footprint scan"
        );
        assert_eq!(r.get(77).unwrap().stage, TreeStage::Felled);
        assert!(r.get(77).unwrap().fruit.is_none());
        assert_eq!(r.revision, revision + 1);
        r.relinquish([8, 16, 8]);
        assert_eq!(r.get(77).unwrap().stage, TreeStage::Felled);
        assert!(r.valid());
    }
}
