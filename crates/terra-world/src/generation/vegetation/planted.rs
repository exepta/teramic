//! Player-grown trees share the authored generator forms, above their planting plane.
use super::{Tree, species::TreeSpecies};
type TreeCells = (Vec<[i8; 3]>, Vec<[i8; 3]>);

/// Exact cells use the same capsules and crowns as procedural trees. Below-ground
/// roots are clipped so an existing soil platform is never replaced by growth.
#[cfg(feature = "bevy")]
pub fn planted_shape(species: terra_core::TreeSpecies, seed: u64) -> Option<TreeCells> {
    planted_shape_with_ecology(species, seed, 4)
}

/// New birches use the saved world ecology; existing exact-cell records never
/// regenerate. Other species retain their established player-grown silhouettes.
#[cfg(feature = "bevy")]
pub fn planted_shape_with_ecology(
    species: terra_core::TreeSpecies,
    seed: u64,
    ecology_revision: u8,
) -> Option<TreeCells> {
    let revision = if species == terra_core::TreeSpecies::Birch && ecology_revision >= 7 {
        7
    } else {
        4
    };
    let species = match species {
        terra_core::TreeSpecies::Oak => TreeSpecies::Green,
        terra_core::TreeSpecies::Spruce => TreeSpecies::Spruce,
        terra_core::TreeSpecies::Birch => TreeSpecies::Birch,
        terra_core::TreeSpecies::Cherry => TreeSpecies::Cherry(false),
        terra_core::TreeSpecies::Apple => TreeSpecies::Apple,
    };
    let tree = Tree::with_species_revision([0; 3], seed, [0.; 4], species, revision);
    let shape = tree.felling_shape()?;
    let offsets = |cells: Vec<[i64; 3]>| -> Option<Vec<[i8; 3]>> {
        cells
            .into_iter()
            .filter(|p| p[1] >= 0)
            .map(|p| {
                Some([
                    i8::try_from(p[0]).ok()?,
                    i8::try_from(p[1]).ok()?,
                    i8::try_from(p[2]).ok()?,
                ])
            })
            .collect()
    };
    let wood = offsets(shape.wood)?;
    let leaves = offsets(shape.leaves)?;
    (!wood.is_empty() && wood.len() + leaves.len() <= terra_core::trees::MAX_TREE_CELLS)
        .then_some((wood, leaves))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn revised_apple_branch_capsules_are_connected_without_retroactively_changing_old_trees() {
        for seed in 0..64 {
            let old = Tree::with_species([0; 3], seed, [0.; 4], TreeSpecies::Apple);
            assert_eq!(
                Tree::with_species_revision([0; 3], seed, [0.; 4], TreeSpecies::Apple, 3),
                old
            );
            let revised = Tree::with_species_revision([0; 3], seed, [0.; 4], TreeSpecies::Apple, 4);
            assert_eq!(
                revised.crowns, old.crowns,
                "the existing broad apple crown stays authored"
            );
            assert_eq!(revised.wood.len(), old.wood.len());
            assert_ne!(
                revised.wood, old.wood,
                "branches gain radius rather than bridge blocks"
            );
            let (wood, _) = planted_shape(terra_core::TreeSpecies::Apple, seed).unwrap();
            let cells: BTreeSet<_> = wood.iter().copied().collect();
            let mut reached = BTreeSet::from([[0_i8; 3]]);
            let mut todo = vec![[0_i8; 3]];
            while let Some(p) = todo.pop() {
                for delta in [
                    [-1, 0, 0],
                    [1, 0, 0],
                    [0, -1, 0],
                    [0, 1, 0],
                    [0, 0, -1],
                    [0, 0, 1],
                ] {
                    let next = std::array::from_fn(|a| p[a] + delta[a]);
                    if cells.contains(&next) && reached.insert(next) {
                        todo.push(next);
                    }
                }
            }
            assert_eq!(reached.len(), wood.len(), "apple seed {seed}");
        }
    }
    #[test]
    fn every_planted_species_uses_the_same_generated_capsules_and_crowns_without_replacing_soil() {
        for (core, natural) in [
            (terra_core::TreeSpecies::Oak, TreeSpecies::Green),
            (terra_core::TreeSpecies::Spruce, TreeSpecies::Spruce),
            (terra_core::TreeSpecies::Birch, TreeSpecies::Birch),
            (terra_core::TreeSpecies::Cherry, TreeSpecies::Cherry(false)),
            (terra_core::TreeSpecies::Apple, TreeSpecies::Apple),
        ] {
            for seed in [0, 1, 42, 1201] {
                let expected = Tree::with_species_revision([0; 3], seed, [0.; 4], natural, 4)
                    .felling_shape()
                    .unwrap();
                let (wood, leaves) = planted_shape(core, seed).unwrap();
                let to_set = |v: Vec<[i64; 3]>| {
                    v.into_iter()
                        .filter(|p| p[1] >= 0)
                        .map(|p| p.map(|v| v as i8))
                        .collect::<BTreeSet<_>>()
                };
                assert_eq!(
                    wood.iter().copied().collect::<BTreeSet<_>>(),
                    to_set(expected.wood)
                );
                assert_eq!(
                    leaves.iter().copied().collect::<BTreeSet<_>>(),
                    to_set(expected.leaves)
                );
                assert!(wood.iter().chain(&leaves).all(|p| p[1] >= 0));
                assert!(leaves.len() > 100);
                assert!(wood.len() + leaves.len() <= terra_core::trees::MAX_TREE_CELLS);
            }
        }
    }
}
