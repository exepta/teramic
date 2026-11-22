#[derive(Default)]
pub(in crate::generation) struct PlantedAppearanceSource {
    records: Vec<std::sync::Arc<terra_core::TreeRecord>>,
    sections: std::sync::Mutex<
        std::collections::HashMap<[i64; 3], std::sync::Arc<TreeAppearanceSnapshot>>,
    >,
}
use super::{Tree, species::TreeSpecies, trees_in_column};
use crate::TerraGenerator;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
};

const MAX_COLUMNS: usize = 512;
type TreeColumn = Arc<Vec<Tree>>;
type PlantedCells = HashMap<([i64; 3], u16), TreeAppearance>;

#[derive(Default)]
pub(in crate::generation) struct TreeAppearanceCache(Mutex<Columns>, Mutex<Sections>);

#[derive(Default)]
struct Sections {
    entries: HashMap<[i64; 3], Arc<TreeAppearanceSnapshot>>,
    order: VecDeque<[i64; 3]>,
}

#[derive(Default)]
struct Columns {
    entries: HashMap<[i64; 2], TreeColumn>,
    order: VecDeque<[i64; 2]>,
}

/// Rendering metadata only: the logical wood and leaf material IDs remain unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeAppearance {
    Oak,
    Autumn,
    Birch,
    Spruce,
    Palm,
    Mushroom,
    Cherry,
    Fir,
    Apple,
}

impl TreeAppearance {
    pub(in crate::generation) fn of(tree: &Tree) -> Self {
        match tree.species {
            TreeSpecies::Green => Self::Oak,
            TreeSpecies::Autumn => Self::Autumn,
            TreeSpecies::Birch => Self::Birch,
            TreeSpecies::Spruce => Self::Spruce,
            TreeSpecies::Fir => Self::Fir,
            TreeSpecies::Apple => Self::Apple,
            TreeSpecies::Palm => Self::Palm,
            TreeSpecies::Mushroom(_) => Self::Mushroom,
            TreeSpecies::Cherry(_) | TreeSpecies::SpiritBlossom(_) | TreeSpecies::Bloodwood => {
                Self::Cherry
            }
        }
    }
}

pub struct TreeAppearanceSnapshot {
    trees: TreeColumn,
    origin: [i64; 3],
    cells: Arc<[AtomicU8]>,
    planted: Option<Arc<PlantedCells>>,
}

fn cell_memo() -> Arc<[AtomicU8]> {
    std::iter::repeat_with(|| AtomicU8::new(0))
        .take(5 * 4096)
        .collect::<Vec<_>>()
        .into()
}

impl TerraGenerator {
    /// Prepare once per asynchronous section composition, never once per vertex.
    pub fn with_planted_appearance(mut self, records: Vec<Arc<terra_core::TreeRecord>>) -> Self {
        self.planted_appearances = Arc::new(PlantedAppearanceSource {
            records,
            sections: Mutex::new(HashMap::new()),
        });
        self
    }
    pub fn tree_appearance(&self, origin: [i64; 3]) -> Arc<TreeAppearanceSnapshot> {
        let natural = self.natural_tree_appearance(origin);
        if self.planted_appearances.records.is_empty() {
            return natural;
        }
        if let Some(cached) = self
            .planted_appearances
            .sections
            .lock()
            .ok()
            .and_then(|m| m.get(&origin).cloned())
        {
            return cached;
        }
        let mut planted = HashMap::new();
        for tree in &self.planted_appearances.records {
            if !tree.stage.is_generated() {
                continue;
            }
            let kind = match tree.species {
                terra_core::TreeSpecies::Oak => TreeAppearance::Oak,
                terra_core::TreeSpecies::Spruce => TreeAppearance::Spruce,
                terra_core::TreeSpecies::Birch => TreeAppearance::Birch,
                terra_core::TreeSpecies::Cherry => TreeAppearance::Cherry,
                terra_core::TreeSpecies::Apple => TreeAppearance::Apple,
            };
            for (cells, material) in [
                (&tree.wood, tree.species.wood()),
                (&tree.leaves, tree.species.leaves()),
            ] {
                for offset in cells {
                    let p = tree.global(*offset);
                    if (0..3).all(|a| (origin[a] - 1..=origin[a] + 16).contains(&p[a])) {
                        planted.insert((p, material), kind);
                    }
                }
            }
        }
        let snapshot = Arc::new(TreeAppearanceSnapshot {
            trees: natural.trees.clone(),
            origin,
            cells: natural.cells.clone(),
            planted: Some(Arc::new(planted)),
        });
        if let Ok(mut cache) = self.planted_appearances.sections.lock() {
            cache.insert(origin, snapshot.clone());
        }
        snapshot
    }
    fn natural_tree_appearance(&self, origin: [i64; 3]) -> Arc<TreeAppearanceSnapshot> {
        if let Some(snapshot) = self
            .tree_appearances
            .1
            .lock()
            .ok()
            .and_then(|cache| cache.entries.get(&origin).cloned())
        {
            return snapshot;
        }
        let key = [origin[0], origin[2]];
        let cached = self
            .tree_appearances
            .0
            .lock()
            .ok()
            .and_then(|cache| cache.entries.get(&key).cloned());
        let trees = cached.unwrap_or_else(|| {
            // Generation is outside the lock; only small Arc/cache operations are serialized.
            let trees = Arc::new(trees_in_column(self, [origin[0], 0, origin[2]], 16));
            if let Ok(mut cache) = self.tree_appearances.0.lock() {
                if let Some(existing) = cache.entries.get(&key) {
                    return existing.clone();
                }
                if cache.entries.len() >= MAX_COLUMNS
                    && let Some(oldest) = cache.order.pop_front()
                {
                    cache.entries.remove(&oldest);
                }
                cache.order.push_back(key);
                cache.entries.insert(key, trees.clone());
            }
            trees
        });
        let snapshot = Arc::new(TreeAppearanceSnapshot {
            trees,
            origin,
            cells: cell_memo(),
            planted: None,
        });
        if let Ok(mut cache) = self.tree_appearances.1.lock() {
            if let Some(existing) = cache.entries.get(&origin) {
                return existing.clone();
            }
            if cache.entries.len() >= MAX_COLUMNS
                && let Some(oldest) = cache.order.pop_front()
            {
                cache.entries.remove(&oldest);
            }
            cache.order.push_back(origin);
            cache.entries.insert(origin, snapshot.clone());
        }
        snapshot
    }
}

impl TreeAppearanceSnapshot {
    /// Existing generated roots around this section, without resampling terrain.
    /// Neighboring snapshots can contain the same tree.
    pub fn tree_roots(&self) -> impl Iterator<Item = ([i64; 3], TreeAppearance)> + '_ {
        self.trees
            .iter()
            .map(|tree| (tree.origin, TreeAppearance::of(tree)))
    }

    pub fn is_planted(&self, global: [i64; 3], material: u16) -> bool {
        self.planted
            .as_ref()
            .is_some_and(|p| p.contains_key(&(global, material)))
    }
    pub fn at(&self, global: [i64; 3], material: u16) -> Option<TreeAppearance> {
        if let Some(kind) = self
            .planted
            .as_ref()
            .and_then(|p| p.get(&(global, material)))
        {
            return Some(*kind);
        }
        if matches!(material, 132 | 133) {
            return Some(TreeAppearance::Cherry);
        }
        if material == 195 {
            return Some(TreeAppearance::Apple);
        }
        if matches!(material, 36 | 134 | 310..=313 | 405..=406) {
            return self.uncached(global, material);
        }
        if !matches!(material, 6..=10) || self.trees.is_empty() {
            return None;
        }
        let local = std::array::from_fn::<_, 3, _>(|i| global[i].checked_sub(self.origin[i]));
        if let [Some(x), Some(y), Some(z)] = local
            && [x, y, z].iter().all(|v| (0..16).contains(v))
        {
            let index =
                x as usize + 16 * (y as usize + 16 * z as usize) + (material as usize - 6) * 4096;
            let cached = self.cells[index].load(Ordering::Relaxed);
            if cached != 0 {
                return match cached {
                    2 => Some(TreeAppearance::Oak),
                    3 => Some(TreeAppearance::Autumn),
                    4 => Some(TreeAppearance::Birch),
                    5 => Some(TreeAppearance::Spruce),
                    6 => Some(TreeAppearance::Palm),
                    7 => Some(TreeAppearance::Mushroom),
                    8 => Some(TreeAppearance::Cherry),
                    9 => Some(TreeAppearance::Fir),
                    10 => Some(TreeAppearance::Apple),
                    _ => None,
                };
            }
            let result = self.uncached(global, material);
            self.cells[index].store(result.map_or(1, |kind| kind as u8 + 2), Ordering::Relaxed);
            return result;
        }
        self.uncached(global, material)
    }

    fn uncached(&self, global: [i64; 3], material: u16) -> Option<TreeAppearance> {
        let tree = self.trees.iter().find(|tree| {
            let generated = match material {
                6 | 10 | 36 | 134 | 313 | 405 => tree.wood_at(global),
                7..=9 | 132 | 133 | 195 | 310..=312 | 406 => tree.leaves_at(global),
                _ => None,
            };
            generated.is_some_and(|generated| generated as u16 == material)
        })?;
        Some(TreeAppearance::of(tree))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_columns_classify_existing_crowns_without_climate_or_surface_samples() {
        for (species, expected) in [
            (TreeSpecies::Green, TreeAppearance::Oak),
            (TreeSpecies::Birch, TreeAppearance::Birch),
            (TreeSpecies::Spruce, TreeAppearance::Spruce),
            (TreeSpecies::Fir, TreeAppearance::Fir),
            (TreeSpecies::Apple, TreeAppearance::Apple),
            (TreeSpecies::SpiritBlossom(0), TreeAppearance::Cherry),
            (TreeSpecies::SpiritBlossom(1), TreeAppearance::Cherry),
            (TreeSpecies::SpiritBlossom(2), TreeAppearance::Cherry),
            (TreeSpecies::Bloodwood, TreeAppearance::Cherry),
        ] {
            let tree = Tree::with_species([-16, 100, -16], 42, [0.0; 4], species);
            let material = species.leaves() as u16;
            let leaf = (tree.bounds.min[2]..tree.bounds.max[2])
                .find_map(|z| {
                    (tree.bounds.min[1]..tree.bounds.max[1]).find_map(|y| {
                        (tree.bounds.min[0]..tree.bounds.max[0]).find_map(|x| {
                            let global = [
                                tree.origin[0] + i64::from(x),
                                tree.origin[1] + i64::from(y),
                                tree.origin[2] + i64::from(z),
                            ];
                            tree.leaves_at(global)
                                .filter(|id| *id as u16 == material)
                                .map(|_| global)
                        })
                    })
                })
                .expect("generated crown");
            let column = crate::PreparedColumn {
                structures: Vec::new(),
                natural_features: Vec::new(),
                config: Default::default(),
                coordinate: [-1, -1],
                padding: 0,
                size: 16,
                columns: Vec::new(),
                trees: vec![tree],
                tree_props: Vec::new(),
                maximum: 150.0,
                surface_sections: None,
                #[cfg(feature = "bevy")]
                cave_eye_bounds: None,
                #[cfg(feature = "bevy")]
                gallery_skyline: false,
            };
            assert_eq!(column.tree_appearance_at(leaf, material), Some(expected));
            assert_eq!(column.tree_appearance_at(leaf, 3), None);
            assert_eq!(column.tree_appearance_at([1000, 100, 1000], material), None);
            let snapshot = TreeAppearanceSnapshot {
                trees: Arc::new(column.trees),
                origin: [-16, 96, -16],
                cells: cell_memo(),
                planted: None,
            };
            assert_eq!(snapshot.at(leaf, material), Some(expected));
            if species == TreeSpecies::Bloodwood {
                assert_eq!(snapshot.at([1000, 100, 1000], material), None);
            }
        }
    }

    #[test]
    fn appearance_uses_actual_species_and_never_changes_material_ids() {
        for (species, expected) in [
            (TreeSpecies::Green, TreeAppearance::Oak),
            (TreeSpecies::Birch, TreeAppearance::Birch),
            (TreeSpecies::Spruce, TreeAppearance::Spruce),
            (TreeSpecies::Fir, TreeAppearance::Fir),
            (TreeSpecies::Bloodwood, TreeAppearance::Cherry),
        ] {
            let tree = Tree::with_species([-16, 100, -16], 42, [0.0; 4], species);
            let snapshot = TreeAppearanceSnapshot {
                trees: Arc::new(vec![tree]),
                origin: [-16, 96, -16],
                cells: cell_memo(),
                planted: None,
            };
            let material = species.wood() as u16;
            assert_eq!(snapshot.at([-16, 101, -16], material), Some(expected));
            assert_eq!(snapshot.at([-16, 101, -16], 2), None);
            assert_eq!(snapshot.at([1000, 101, 1000], material), None);
            assert_eq!(
                snapshot.at([-16, 101, -16], material),
                snapshot.uncached([-16, 101, -16], material)
            );
        }
    }

    #[test]
    fn columns_share_between_vertical_sections_and_cache_is_bounded() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            trees: false,
            ..Default::default()
        })
        .unwrap();
        let a = terrain.tree_appearance([-16, -32, -16]);
        let b = terrain.clone().tree_appearance([-16, 96, -16]);
        assert!(Arc::ptr_eq(&a.trees, &b.trees));
        assert!(Arc::ptr_eq(&a, &terrain.tree_appearance([-16, -32, -16])));
        for x in 0..MAX_COLUMNS as i64 + 8 {
            terrain.tree_appearance([x * 16, 0, 0]);
        }
        let cache = terrain.tree_appearances.0.lock().unwrap();
        assert_eq!(cache.entries.len(), MAX_COLUMNS);
        assert_eq!(cache.order.len(), MAX_COLUMNS);
        assert!(!cache.entries.contains_key(&[-16, -16]));
        let sections = terrain.tree_appearances.1.lock().unwrap();
        assert_eq!(sections.entries.len(), MAX_COLUMNS);
        assert_eq!(sections.order.len(), MAX_COLUMNS);
    }

    #[test]
    fn dense_memo_preserves_every_material_at_negative_section_coordinates() {
        let tree = Tree::with_species([-16, 100, -16], 42, [0.0; 4], TreeSpecies::Green);
        let snapshot = TreeAppearanceSnapshot {
            trees: Arc::new(vec![tree]),
            origin: [-16, 96, -16],
            cells: cell_memo(),
            planted: None,
        };
        for x in -16..0 {
            for y in 96..112 {
                for z in -16..0 {
                    for material in 6..=10 {
                        let p = [x, y, z];
                        let expected = snapshot.uncached(p, material);
                        assert_eq!(snapshot.at(p, material), expected);
                        assert_eq!(snapshot.at(p, material), expected);
                    }
                }
            }
        }
    }
}

#[cfg(all(test, feature = "bevy"))]
mod planted_tests {
    use super::*;
    use terra_core::{PlayerUuid, TreeRecord, TreeSpecies as CoreSpecies, TreeStage};
    #[test]
    fn exact_generated_ownership_selects_natural_style_and_ignores_player_replacements_and_legacy_mature()
     {
        let mut r = TreeRecord {
            id: 1,
            root: [0, 16, 0],
            species: CoreSpecies::Spruce,
            seed: 42,
            planted_by: Some(PlayerUuid::from_u128(1)),
            stage: TreeStage::Generated,
            growth_seconds: 1500.,
            bounds: None,
            wood: vec![[0, 0, 0]],
            leaves: vec![[0, 10, 0], [1, 10, 0]],
            fruit: None,
            revision: 1,
        }
        .with_bounds();
        let generator = TerraGenerator::new(terra_core::WorldConfig {
            trees: false,
            ..Default::default()
        })
        .unwrap();
        let snapshot = generator
            .clone()
            .with_planted_appearance(vec![Arc::new(r.clone())])
            .tree_appearance([0, 16, 0]);
        assert_eq!(snapshot.at([0, 26, 0], 7), Some(TreeAppearance::Spruce));
        assert!(snapshot.is_planted([0, 26, 0], 7));
        assert!(!snapshot.is_planted([0, 26, 0], 6));
        r.stage = TreeStage::GeneratedFelled;
        r.leaves.remove(0);
        r.refresh_bounds();
        let snapshot = generator
            .clone()
            .with_planted_appearance(vec![Arc::new(r.clone())])
            .tree_appearance([0, 16, 0]);
        assert!(!snapshot.is_planted([0, 26, 0], 7));
        assert_eq!(snapshot.at([1, 26, 0], 7), Some(TreeAppearance::Spruce));
        r.stage = TreeStage::Mature;
        let snapshot = generator
            .with_planted_appearance(vec![Arc::new(r)])
            .tree_appearance([0, 16, 0]);
        assert!(!snapshot.is_planted([1, 26, 0], 7));
        assert_eq!(snapshot.at([1, 26, 0], 7), None);
    }
}
