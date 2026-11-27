use super::{Tree, mix, species::TreeSpecies};
use crate::props::{PropId, PropKind, WorldProp};
use terra_voxel::global_to_section;

/// Fruit hangs just below the crown. Only exposed faces of the last trunk segment
/// bear nuts, so each one can be picked individually without hitting a leaf.
pub(crate) fn palm_coconuts(tree: &Tree, world_seed: u64) -> Vec<WorldProp> {
    if tree.species != TreeSpecies::Palm {
        return Vec::new();
    }
    let seed = crate::fruit::natural_id(world_seed, tree.origin);
    let Some(trunk) = tree.wood.last() else {
        return Vec::new();
    };
    let bounds = trunk.bounds;
    let y = tree.origin[1] + i64::from(bounds.max[1]) - 4;
    let mut fruit = Vec::with_capacity(3);
    for index in 0..(2 + (seed & 1)) {
        let face = ((seed >> 8) + index) as usize % 4;
        let [dx, dz] = [[0, 1], [1, 0], [0, -1], [-1, 0]][face];
        let candidate = (bounds.min[0]..bounds.max[0])
            .flat_map(|x| (bounds.min[2]..bounds.max[2]).map(move |z| [x, z]))
            .map(|[x, z]| {
                [
                    tree.origin[0] + i64::from(x),
                    y,
                    tree.origin[2] + i64::from(z),
                ]
            })
            .find(|&anchor| {
                let outside = [anchor[0] + dx, y, anchor[2] + dz];
                tree.wood_at(anchor).is_some()
                    && tree.wood_at(outside).is_none()
                    && tree.leaves_at(outside).is_none()
            });
        if let Some(anchor) = candidate {
            fruit.push(WorldProp {
                id: PropId(mix(seed ^ 0x434f_434f_4e55_5400 ^ index)),
                kind: PropKind::Coconut,
                position: [
                    anchor[0] as f64 + 0.5 + dx as f64 * 0.82,
                    y as f64 + 0.12,
                    anchor[2] as f64 + 0.5 + dz as f64 * 0.82,
                ],
                rotation: face as f32 * std::f32::consts::FRAC_PI_2,
                scale: 1.15,
                variant: 128,
                section: global_to_section(anchor).0,
                removed: false,
            });
        }
    }
    fruit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::placement;
    use terra_voxel::MaterialId;

    #[test]
    fn palms_bear_individually_pickable_coconuts_below_voxel_crowns() {
        for seed in 0..32 {
            let palm = Tree::oasis_palm([-1, 12, -16], seed, true);
            assert_eq!(palm.leaves_material, MaterialId::Leaves);
            assert_eq!(
                Tree::oasis_palm(palm.origin, seed, false).leaves_material,
                MaterialId::PalmFrondEast
            );
            let nuts = palm_coconuts(&palm, seed);
            assert!(!nuts.is_empty(), "seed {seed}");
            assert_eq!(nuts, palm_coconuts(&palm, seed));
            for nut in nuts {
                let support = placement::support(&nut);
                assert!(nut.valid() && nut.wall_attached());
                assert_eq!(palm.wood_at(support), Some(MaterialId::Wood));
                assert!(support[1] >= palm.origin[1] + 5);
                let cell = nut.position.map(|v| v.floor() as i64);
                assert!(palm.leaves_at(cell).is_none() && palm.wood_at(cell).is_none());
            }
        }
    }
    #[test]
    fn tropical_coast_groves_are_revision_gated_and_leave_cold_shores_clear() {
        use crate::{TerraGenerator, generation::TreeAppearance};
        use terra_core::{BiomeDefinitions, WorldConfig};
        let modern = WorldConfig {
            ecology_revision: 15,
            biome_definitions: Some(BiomeDefinitions::builtin().clone()),
            ..Default::default()
        };
        let legacy = TerraGenerator::new(WorldConfig {
            ecology_revision: 14,
            ..modern.clone()
        })
        .unwrap();
        let current = TerraGenerator::new(modern).unwrap();
        let palms = |terrain: &TerraGenerator, section: [i64; 3]| {
            terrain
                .tree_appearance(section)
                .tree_roots()
                .filter(|(_, tree)| *tree == TreeAppearance::Palm)
                .count()
        };
        // Seed 42042. The warm beach touches a coral reef; the second site is
        // a snow-covered sea stack in cold marine water.
        let beach = [-1488, 16, 1712];
        assert!(palms(&current, beach) >= 2);
        assert_eq!(palms(&legacy, beach), 0);
        assert_eq!(palms(&current, [1168, 16, -2288]), 0);
    }
}
