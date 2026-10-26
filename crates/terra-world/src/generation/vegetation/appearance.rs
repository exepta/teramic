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
}

impl TreeAppearance {
    pub(in crate::generation) fn of(tree: &Tree) -> Self {
        match tree.species {
            TreeSpecies::Green => Self::Oak,
            TreeSpecies::Autumn => Self::Autumn,
            TreeSpecies::Birch => Self::Birch,
            TreeSpecies::Spruce => Self::Spruce,
            TreeSpecies::Palm => Self::Palm,
            TreeSpecies::Mushroom(_) => Self::Mushroom,
            TreeSpecies::Cherry(_) => Self::Cherry,
        }
    }
}

pub struct TreeAppearanceSnapshot {
    trees: TreeColumn,
    origin: [i64; 3],
    cells: Box<[AtomicU8]>,
}

fn cell_memo() -> Box<[AtomicU8]> {
    std::iter::repeat_with(|| AtomicU8::new(0))
        .take(5 * 4096)
        .collect()
}

impl TerraGenerator {
    /// Prepare once per asynchronous section composition, never once per vertex.
    pub fn tree_appearance(&self, origin: [i64; 3]) -> Arc<TreeAppearanceSnapshot> {
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
    pub fn at(&self, global: [i64; 3], material: u16) -> Option<TreeAppearance> {
        if matches!(material, 132 | 133) {
            return Some(TreeAppearance::Cherry);
        }
        if material == 36 {
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
                6 | 10 | 36 | 134 => tree.wood_at(global),
                7..=9 | 132 | 133 => tree.leaves_at(global),
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
                            tree.leaves_at(global).map(|_| global)
                        })
                    })
                })
                .expect("generated crown");
            let column = crate::PreparedColumn {
                structures: Vec::new(),
                config: Default::default(),
                coordinate: [-1, -1],
                padding: 0,
                size: 16,
                columns: Vec::new(),
                trees: vec![tree],
                tree_props: Vec::new(),
                maximum: 150.0,
                surface_sections: None,
            };
            assert_eq!(column.tree_appearance_at(leaf, material), Some(expected));
            assert_eq!(column.tree_appearance_at(leaf, 3), None);
            assert_eq!(column.tree_appearance_at([1000, 100, 1000], material), None);
        }
    }

    #[test]
    fn appearance_uses_actual_species_and_never_changes_material_ids() {
        for (species, expected) in [
            (TreeSpecies::Green, TreeAppearance::Oak),
            (TreeSpecies::Birch, TreeAppearance::Birch),
            (TreeSpecies::Spruce, TreeAppearance::Spruce),
        ] {
            let tree = Tree::with_species([-16, 100, -16], 42, [0.0; 4], species);
            let snapshot = TreeAppearanceSnapshot {
                trees: Arc::new(vec![tree]),
                origin: [-16, 96, -16],
                cells: cell_memo(),
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
