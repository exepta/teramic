//! Root-side windfall, shade plants and flower drifts along forest trails.
use super::{Cell, Habitat, TILE_SIZE, Tile, ticket};
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

pub(super) fn build(h: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    let origin = tile.map(|v| v * TILE_SIZE);
    if h.terrain
        .biome_at((origin[0] + 16) as f64, (origin[1] + 16) as f64)
        .weights
        .forest
        < 0.35
    {
        return;
    }
    for z in 0..8 {
        for x in 0..8 {
            let grid = [tile[0] * 8 + x, tile[1] * 8 + z];
            let hash = ticket(h.terrain.config().seed.0, grid, 3345);
            if hash % 5 >= 2 {
                continue;
            }
            let p = [
                origin[0] + x * 4 + i64::from(hash & 3),
                origin[1] + z * 4 + i64::from((hash >> 4) & 3),
            ];
            let c = h.column(p);
            if c.biome.weights.forest < 0.5
                || !matches!(
                    c.surface_material,
                    M::Grass | M::WoodlandLitter | M::MossyStone
                )
                || h.reserved(p, 1)
            {
                continue;
            }
            let Some((floor, _)) = h.ground(p) else {
                continue;
            };
            let cell = [p[0], floor + 1, p[1]];
            if cells.contains_key(&[p[0], floor, p[1]]) || h.tree_occupied(cell) {
                continue;
            }
            let material = match (hash >> 9) % 24 {
                0 if c.slope < 0.35 => M::ButtressStump,
                1 if c.slope < 0.35 => M::MossyNurseLog,
                2..=5 => M::WindfallTwigs,
                6..=8 => M::WoodlandStones,
                9..=11 => M::ForestBramble,
                12..=15 if h.root_distance_squared(p) < 100 => M::TawnyMilkcaps,
                12..=15 => M::WoodlandToadstools,
                16..=19 if c.surface_material == M::Grass => M::MeadowDaisies,
                16..=19 => M::ForestFern,
                _ => M::CushionMoss,
            };
            if !h.terrain.in_prop_patch(p, material as u16) {
                continue;
            }
            cells.entry(cell).or_insert(material as u16);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, TerraGenerator};
    use std::collections::BTreeSet;
    use terra_core::{BiomeDefinitions, WorldConfig, WorldSeed};
    use terra_voxel::global_to_section;

    fn world(revision: u8) -> TerraGenerator {
        TerraGenerator::new(WorldConfig {
            seed: WorldSeed(42042),
            ecology_revision: revision,
            biome_definitions: Some(
                BiomeDefinitions::load_directory(terra_core::asset_directory().join("biome"))
                    .unwrap(),
            ),
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn forest_infill_increases_root_count_and_reduces_neighbor_distance() {
        use crate::generation::vegetation::trees_in_column;
        let roots = |revision| {
            trees_in_column(&world(revision), [960, 0, -5152], 192)
                .into_iter()
                .map(|t| [t.origin[0], t.origin[2]])
                .filter(|[x, z]| (960..1152).contains(x) && (-5152..-4960).contains(z))
                .collect::<BTreeSet<_>>()
        };
        let old = roots(14);
        let new = roots(15);
        let mean_neighbor_distance = |points: &BTreeSet<[i64; 2]>| {
            points
                .iter()
                .map(|a| {
                    points
                        .iter()
                        .filter(|b| *b != a)
                        .map(|b| (a[0] - b[0]).pow(2) + (a[1] - b[1]).pow(2))
                        .min()
                        .unwrap() as f64
                })
                .map(f64::sqrt)
                .sum::<f64>()
                / points.len() as f64
        };
        let before = mean_neighbor_distance(&old);
        let after = mean_neighbor_distance(&new);
        eprintln!(
            "forest roots {} -> {}, mean nearest trunk {before:.1} -> {after:.1}",
            old.len(),
            new.len()
        );
        assert!(new.len() > old.len() * 3 / 2);
        assert!(after < before * 0.85);
    }

    #[test]
    fn forest_trails_and_understory_survive_section_generation() {
        let new = world(14);
        let old = world(13);
        let sections = SectionGenerator::new(new.clone());
        let mut seen = BTreeSet::new();
        for tile in [
            [31, -157],
            [32, -157],
            [33, -157],
            [34, -157],
            [35, -157],
            [36, -157],
        ] {
            for (p, m) in new.surface_flora(tile) {
                if !(521..=526).contains(&m) || !seen.insert(m) {
                    continue;
                }
                let (owner, local) = global_to_section(p);
                assert_eq!(sections.generate(owner).unwrap().block(local).unwrap(), m);
                let floor = sections.block_at([p[0], p[1] - 1, p[2]]).unwrap();
                assert!(M::from_raw(floor).unwrap().is_natural_ground());
                assert_ne!(floor, M::WoodlandTrail as u16);
            }
            assert!(
                old.surface_flora(tile)
                    .iter()
                    .all(|(_, m)| !(521..=526).contains(m))
            );
        }
        assert_eq!(seen.len(), 6);
        let (x, z) = (1082, -5096);
        let c = new.surface_column(x as f64 + 0.5, z as f64 + 0.5);
        let top = (c.height - 0.5).floor() as i64;
        assert_eq!(
            sections.block_at([x, top, z]).unwrap(),
            M::WoodlandTrail as u16
        );
        assert_eq!(sections.block_at([x, top - 1, z]).unwrap(), M::Dirt as u16);
    }
}
