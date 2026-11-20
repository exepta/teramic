use crate::{
    SectionGenerator, TerraGenerator,
    generation::rivers::{DrainageFrame, POINTS, RiverPath},
};
use std::collections::HashMap;
use terra_core::{WorldConfig, WorldSeed};

fn config(seed: u64) -> WorldConfig {
    WorldConfig {
        seed: WorldSeed(seed),
        river_terrain_revision: 5,
        rivers: true,
        trees: false,
        caves: false,
        structures_revision: 0,
        ocean_revision: 0,
        ..Default::default()
    }
}

/// Intersect a metre-scaled normal line with the *voxel* footprint. Repeated
/// samples inside a diagonal voxel describe physical width, not a misleading
/// count of grid cells touched by the diagonal.
fn span(mut wet: impl FnMut(f64) -> bool) -> f64 {
    assert!(wet(0.0), "course centre must contain water");
    let mut edges = [0.0; 2];
    for (index, sign) in [-1.0, 1.0].into_iter().enumerate() {
        for n in 1..=256 {
            let at = f64::from(n) * 0.125;
            if !wet(sign * at) {
                edges[index] = at - 0.0625;
                break;
            }
        }
        assert!(edges[index] > 0.0, "finite individual channel bank");
    }
    edges[0] + edges[1]
}

#[test]
fn voxelized_width_limits_include_shallow_diagonal_negative_and_fractional_channels() {
    let terrain = TerraGenerator::new(config(42042)).unwrap();
    let mut measured = [f64::INFINITY, 0.0_f64];
    for brook in [false, true] {
        for slope in [-1.4_f64, -0.7, 0.0, 0.7, 1.4] {
            for level in [20.0, 20.01, 20.49, 20.99] {
                for path_width in [1.0, 4.0, 8.0, 14.0] {
                    let path = RiverPath {
                        points: std::array::from_fn(|i| {
                            [slope * i as f64 * 8.0, level, i as f64 * 8.0]
                        }),
                        ground: [level + 3.0; POINTS],
                        extensions: [0.0; 2],
                        width: path_width,
                        phase: 0.3,
                        branch: None,
                        brook,
                        step: 8.0,
                        frame: DrainageFrame {
                            origin: [-533.17, -211.31],
                            downstream: [0.0, 1.0],
                        },
                        lake: None,
                    };
                    let center = path.frame.world(slope * 192.0, 192.0);
                    let normal = [1.0 / slope.hypot(1.0), -slope / slope.hypot(1.0)];
                    let width = span(|offset| {
                        let point = [
                            center[0] + normal[0] * offset,
                            center[1] + normal[1] * offset,
                        ]
                        .map(|v| v.floor() + 0.5);
                        let sample = terrain
                            .sample_drainage(&path, point[0], point[1], None)
                            .unwrap();
                        let bed = (sample.bed_height() - 0.5).floor() + 1.0;
                        sample.distance <= sample.width && bed < sample.level
                    });
                    assert!(
                        (6.0..=22.0).contains(&width),
                        "brook={brook} slope={slope} level={level} source={path_width}: {width}"
                    );
                    measured[0] = measured[0].min(width);
                    measured[1] = measured[1].max(width);
                }
            }
        }
    }
    assert!(
        measured[1] - measured[0] > 7.0,
        "width variation: {measured:?}"
    );
}

#[test]
fn real_regional_channels_have_six_to_twenty_two_blocks_of_water_and_real_empty_water_voxels() {
    let mut checked = 0;
    let mut negative = false;
    let mut measured = [f64::INFINITY, 0.0_f64];
    for seed in [42042, 7, 99173] {
        let terrain = TerraGenerator::new(config(seed)).unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        for region in (-2..=2).flat_map(|z| (-2..=2).map(move |x| [x, z])) {
            let Some(site) = terrain.hydrology_region_at(
                f64::from(region[0]) * 2048.0 + 1024.0,
                f64::from(region[1]) * 2048.0 + 1024.0,
            ) else {
                continue;
            };
            for index in (8..40).step_by(4) {
                let p = site.course[index];
                // Inspect an individual open reach, not a lake, confluence or
                // waterfall where several reservoirs/levels meet by design.
                if p[1] <= terrain.config().sea_level + 1.0
                    || site.course[index..=index + 1]
                        .windows(2)
                        .any(|pair| (pair[0][1] - pair[1][1]).abs() >= 1.0)
                    || site.lake.is_some_and(|lake| {
                        (p[0] - lake.center[0]).hypot(p[2] - lake.center[1]) < lake.radius + 40.0
                    })
                    || site
                        .tributaries
                        .iter()
                        .any(|t| (p[0] - t.mouth[0]).hypot(p[2] - t.mouth[2]) < 64.0)
                    || site
                        .distributaries
                        .iter()
                        .any(|t| (p[0] - t.source[0]).hypot(p[2] - t.source[2]) < 64.0)
                {
                    continue;
                }
                let sample = terrain.river_at(p[0], p[2]).unwrap();
                let normal = [f64::from(sample.flow[1]), -f64::from(sample.flow[0])];
                // Reservoir outlines are anisotropic and can exceed their
                // nominal radius. Exclude their actual footprints from the
                // individual-channel contract, never arbitrary wide rivers.
                if (-8..=8).any(|step| {
                    let x = p[0] + normal[0] * f64::from(step) * 4.0;
                    let z = p[2] + normal[1] * f64::from(step) * 4.0;
                    terrain
                        .oasis_at(x, z)
                        .or_else(|| terrain.lake_at(x, z))
                        .is_some_and(|lake| lake.distance <= lake.radius)
                }) {
                    continue;
                }
                let mut columns = HashMap::new();
                let width = span(|offset| {
                    let xz = [
                        (p[0] + normal[0] * offset).floor() as i64,
                        (p[2] + normal[1] * offset).floor() as i64,
                    ];
                    *columns.entry(xz).or_insert_with(|| {
                        let column = terrain.surface_column(xz[0] as f64 + 0.5, xz[1] as f64 + 0.5);
                        let wet = column.water_level.is_some_and(|level| {
                            (level - p[1]).abs() < 1.0 && column.water_bed() < level
                        });
                        if wet {
                            let y = (column.water_level.unwrap() - 0.000001).floor() as i64;
                            assert_eq!(
                                generator.block_at([xz[0], y, xz[1]]).unwrap(),
                                0,
                                "water must occupy actual empty voxels"
                            );
                        }
                        wet
                    })
                });
                assert!(
                    (6.0..=22.0).contains(&width),
                    "seed={seed} region={region:?} index={index} at={p:?}: {width}"
                );
                measured[0] = measured[0].min(width);
                measured[1] = measured[1].max(width);
                checked += 1;
                negative |= p[0] < 0.0 || p[2] < 0.0;
            }
        }
    }
    assert!(
        checked >= 12 && negative,
        "checked={checked} negative={negative}"
    );
    assert!(
        measured[1] - measured[0] > 3.0,
        "natural widths must vary: {measured:?}"
    );
    println!("river_revision5 sampled_reaches={checked} measured_voxel_width={measured:?}");
}

#[test]
fn new_river_sections_share_negative_boundaries_and_are_generation_order_independent() {
    let terrain = TerraGenerator::new(config(42042)).unwrap();
    let site = [[-1, -1], [-2, 0], [0, -1]]
        .into_iter()
        .find_map(|r| {
            terrain.hydrology_region_at(
                f64::from(r[0]) * 2048.0 + 1024.0,
                f64::from(r[1]) * 2048.0 + 1024.0,
            )
        })
        .unwrap();
    let p = site.course[24].map(|v| v.floor() as i64);
    let owner = terra_voxel::global_to_section(p).0;
    let positions = [owner, owner.offset([1, 0, 0]).unwrap()];
    let a = SectionGenerator::new(terrain.clone());
    let b = SectionGenerator::new(TerraGenerator::new(terrain.config().clone()).unwrap());
    let first = a.generate(positions[0]).unwrap();
    let second = a.generate(positions[1]).unwrap();
    assert_eq!(second.blocks(), b.generate(positions[1]).unwrap().blocks());
    assert_eq!(first.blocks(), b.generate(positions[0]).unwrap().blocks());
    for (position, section) in positions.into_iter().zip([first, second]) {
        let origin = position.origin().unwrap();
        for x in [0, 15] {
            for y in 0..16 {
                for z in 0..16 {
                    let global = [
                        origin[0] + x as i64,
                        origin[1] + y as i64,
                        origin[2] + z as i64,
                    ];
                    assert_eq!(
                        section.block([x, y, z]).unwrap(),
                        a.block_at(global).unwrap()
                    );
                }
            }
        }
    }
}
