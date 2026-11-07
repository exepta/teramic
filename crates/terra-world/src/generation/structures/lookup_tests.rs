use super::*;
use std::{hint::black_box, time::Instant};
use terra_core::WorldConfig;

fn previous_lookup(terrain: &TerraGenerator, global: [i64; 3]) -> Option<u16> {
    terrain
        .structures_for_column([global[0].div_euclid(16), global[2].div_euclid(16)], 0)
        .iter()
        .find_map(|site| site.block_at(global))
}

#[test]
fn point_queries_match_column_plans_including_negative_region_edges() {
    for revision in [1, 2] {
        let terrain = TerraGenerator::new(WorldConfig {
            structures_revision: revision,
            landscape_revision: 4,
            sea_level: 22.0,
            rivers: true,
            biome_definitions: Some(terra_core::BiomeDefinitions::default()),
            ..Default::default()
        })
        .unwrap();
        let mut checked_sites = 0;
        for region in [[-1, -1], [0, -2], [1, -1], [0, 1]] {
            let Some(site) = terrain.planned_site(region) else {
                continue;
            };
            checked_sites += 1;
            for (axis, horizontal) in [0, 2].into_iter().enumerate() {
                assert!(site.summary.bounds[0][horizontal] >= region[axis] * REGION_SIZE + 96);
                assert!(site.summary.bounds[1][horizontal] <= region[axis] * REGION_SIZE + 288);
            }
            for (owner, patch) in &site.sections {
                for &(index, material) in patch.cells.iter().step_by(31) {
                    let i = usize::from(index);
                    let p = owner.global_block([i % 16, i / 16 % 16, i / 256]).unwrap();
                    assert_eq!(terrain.structure_block(p), Some(material));
                    assert_eq!(terrain.structure_block(p), previous_lookup(&terrain, p));
                }
            }
            for x in [95, 96, 97, 287, 288, 289] {
                for z in [95, 96, 97, 287, 288, 289] {
                    let p = [
                        region[0] * REGION_SIZE + x,
                        site.summary.center[1],
                        region[1] * REGION_SIZE + z,
                    ];
                    assert_eq!(terrain.structure_block(p), previous_lookup(&terrain, p));
                }
            }
        }
        assert!(
            checked_sites > 0,
            "revision {revision} must exercise real plans"
        );
    }
}

#[test]
#[ignore = "opt-in wall-clock comparison; not a machine-dependent CI threshold"]
fn benchmark_point_lookup_without_region_vector_allocation() {
    let terrain = TerraGenerator::new(WorldConfig {
        structures_revision: 1,
        landscape_revision: 4,
        sea_level: 22.0,
        rivers: true,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        ..Default::default()
    })
    .unwrap();
    let site = terrain
        .planned_site([0, -2])
        .expect("stable legacy fixture");
    let positions: Vec<_> = site
        .sections
        .iter()
        .flat_map(|(owner, patch)| {
            patch.cells.iter().step_by(127).map(|&(i, _)| {
                let i = usize::from(i);
                owner.global_block([i % 16, i / 16 % 16, i / 256]).unwrap()
            })
        })
        .collect();
    for pass in 0..3 {
        let start = Instant::now();
        for i in 0..200_000 {
            black_box(previous_lookup(
                &terrain,
                black_box(positions[i % positions.len()]),
            ));
        }
        let before = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        for i in 0..200_000 {
            black_box(terrain.structure_block(black_box(positions[i % positions.len()])));
        }
        let after = start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "structure_point_lookup pass={pass} queries=200000 previous_ms={before:.3} direct_ms={after:.3}"
        );
    }
}
