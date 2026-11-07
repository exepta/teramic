use super::*;
use crate::{
    TerraGenerator,
    edits::{BlockObjectState, EditJournal},
    water::{FluidCell, space::FluidSpace},
};
use std::collections::BTreeMap;
use terra_core::{WorldConfig, block_properties::BlockPropertiesCatalog, construction, plumbing};

type Block = ([i64; 3], u16, u8);

fn scene(
    blocks: &[Block],
    natural: Option<f64>,
    bottom: i64,
    top: i64,
) -> (WaterPatch, FluidOccluders) {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: -20.,
        sea_level: natural.unwrap_or(-10.),
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        biomes: false,
        caves: false,
        trees: false,
        rivers: false,
        landscape_revision: 0,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap();
    let columns = vec![terrain.surface_column(0., 0.); 18 * 18];
    let materials: BTreeMap<_, _> = blocks
        .iter()
        .map(|&(p, material, _)| (p, material))
        .collect();
    let block_at = |p| Some(materials.get(&p).copied().unwrap_or(0));
    let mut journal = EditJournal::default();
    let mut padded = vec![0; 18 * 18 * 18];
    for &(p, material, facing) in blocks {
        journal.record(p, material).unwrap();
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        padded[(p[0] + 1) as usize + 18 * ((p[1] + 1) as usize + 18 * (p[2] + 1) as usize)] =
            material;
    }
    let mut occluders = FluidOccluders::new();
    for &(p, material, facing) in blocks {
        let mut shapes = Vec::new();
        assert!(crate::interaction_shapes::visit_boxes_with(
            &journal,
            &BlockPropertiesCatalog::default(),
            p,
            block_at,
            |b| shapes.push(b)
        ));
        occluders.insert(p, shapes);
        if plumbing::is_pipe(material) {
            occluders.pipes.insert(
                p,
                crate::plumbing::connection_mask(&journal, p, material, facing, block_at),
            );
        } else if matches!(material, plumbing::VALVE..=plumbing::PUMP) {
            occluders.fixtures.insert(p, (material, facing));
        }
    }
    let mut fluids = FluidSnapshot::new();
    let mut precise = HashMap::new();
    for z in 7..=9 {
        for y in bottom..=top {
            for x in 7..=9 {
                let p = [x, y, z];
                let level = natural.map_or(y as f64 + 1., |level| level.min(y as f64 + 1.));
                let amount = ((level - y as f64).clamp(0., 1.) * 8.).ceil() as u8;
                let mut cell = FluidCell {
                    amount,
                    ..Default::default()
                };
                if materials.contains_key(&p) {
                    cell = FluidSpace::at(&journal, p, block_at).clamp(cell);
                }
                fluids.insert(p, cell);
                if natural.is_some() {
                    precise.insert(p, level);
                }
            }
        }
    }
    let retained = fluids.clone();
    let mut patch = None;
    append_fluid_patch(
        &mut patch,
        SectionPos::default(),
        &columns,
        &padded,
        &fluids,
        (false, &occluders, &precise),
    );
    assert_eq!(fluids, retained, "meshing may not change stored contents");
    (patch.unwrap(), occluders)
}

fn has_top(patch: &WaterPatch, point: [f64; 3]) -> bool {
    patch.indices.chunks_exact(3).any(|triangle| {
        if patch.normals[triangle[0] as usize][1] < 0.99 {
            return false;
        }
        let positions = [triangle[0], triangle[1], triangle[2]]
            .map(|i| patch.positions[i as usize].map(f64::from));
        if positions.iter().any(|p| (p[1] - point[1]).abs() > 1e-5) {
            return false;
        }
        if (0..3).any(|axis| {
            point[axis]
                < positions
                    .iter()
                    .map(|p| p[axis])
                    .fold(f64::INFINITY, f64::min)
                    - 1e-8
                || point[axis]
                    > positions
                        .iter()
                        .map(|p| p[axis])
                        .fold(f64::NEG_INFINITY, f64::max)
                        + 1e-8
        }) {
            return false;
        }
        let p = bevy_math::DVec2::new(point[0], point[2]);
        let v = positions.map(|p| bevy_math::DVec2::new(p[0], p[2]));
        let signed: [f64; 3] = std::array::from_fn(|i| {
            let edge = v[(i + 1) % 3] - v[i];
            edge.perp_dot(p - v[i]) / edge.length()
        });
        signed.iter().all(|d| *d >= -1e-8) || signed.iter().all(|d| *d <= 1e-8)
    })
}

fn solid_at(shapes: &FluidOccluders, point: [f64; 3]) -> bool {
    shapes.values().flatten().any(|b| {
        (0..3).all(|axis| {
            point[axis] >= b.minimum[axis] - 1e-8 && point[axis] <= b.maximum[axis] + 1e-8
        })
    })
}

#[test]
fn covered_slab_and_stair_pockets_retain_the_connected_head_at_every_exposed_neighbor_corner() {
    for (material, facings) in [(82, 9), (74, 24)] {
        for facing in 0..facings {
            for natural in [None, Some(8.)] {
                let (patch, _) = scene(&[([8, 7, 8], material, facing)], natural, 7, 7);
                let mut tops = 0;
                for triangle in patch.indices.chunks_exact(3) {
                    if patch.normals[triangle[0] as usize][1] < 0.99 {
                        continue;
                    }
                    tops += 1;
                    for &i in triangle {
                        assert!(
                            (patch.positions[i as usize][1] - 8.).abs() < 1e-5,
                            "covered pocket lowered its natural reservoir: material={material} facing={facing} vertex={:?}",
                            patch.positions[i as usize]
                        );
                    }
                }
                assert!(tops > 0);
            }
        }
    }
}

#[test]
fn a_covered_lower_pocket_never_invents_water_in_a_separate_dry_upper_gap() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 20.,
        sea_level: -10.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        biomes: false,
        caves: false,
        trees: false,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    let columns = vec![terrain.surface_column(0., 0.); 18 * 18];
    let p = [8, 7, 8];
    let occluders = FluidOccluders::from([(
        p,
        vec![ShapeBox {
            minimum: [8., 7.375, 8.],
            maximum: [9., 7.625, 9.],
            hinged: false,
        }],
    )]);
    let mut padded = vec![0; 18 * 18 * 18];
    padded[9 + 18 * (8 + 18 * 9)] = 82;
    let mut fluids = FluidSnapshot::new();
    for z in 7..=9 {
        for x in 7..=9 {
            fluids.insert(
                [x, 7, z],
                FluidCell {
                    amount: if [x, 7, z] == p { 3 } else { 8 },
                    ..Default::default()
                },
            );
        }
    }
    let before = fluids.clone();
    let mut patch = None;
    append_fluid_patch(
        &mut patch,
        SectionPos::default(),
        &columns,
        &padded,
        &fluids,
        (false, &occluders),
    );
    assert!(
        !has_top(&patch.unwrap(), [8.5, 8., 8.5]),
        "a neighboring full cell cannot make the model's dry upper gap instantly wet"
    );
    assert_eq!(fluids, before);
}

#[test]
fn a_sealed_lower_pocket_cannot_depress_the_unconnected_water_surface_outside_its_walls() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 20.,
        sea_level: -10.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        biomes: false,
        caves: false,
        trees: false,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    let columns = vec![terrain.surface_column(0., 0.); 18 * 18];
    let p = [8, 7, 8];
    let cube = |minimum, maximum| ShapeBox {
        minimum,
        maximum,
        hinged: false,
    };
    let occluders = FluidOccluders::from([(
        p,
        vec![
            cube([8., 7.5, 8.], [9., 8., 9.]),
            cube([8., 7., 8.], [8.125, 7.5, 9.]),
            cube([8.875, 7., 8.], [9., 7.5, 9.]),
            cube([8., 7., 8.], [9., 7.5, 8.125]),
            cube([8., 7., 8.875], [9., 7.5, 9.]),
        ],
    )]);
    let mut padded = vec![0; 18 * 18 * 18];
    padded[9 + 18 * (8 + 18 * 9)] = 82;
    let mut fluids = FluidSnapshot::new();
    for z in 7..=9 {
        for x in 7..=9 {
            fluids.insert(
                [x, 7, z],
                FluidCell {
                    amount: if [x, 7, z] == p { 4 } else { 8 },
                    ..Default::default()
                },
            );
        }
    }
    let before = fluids.clone();
    let mut patch = None;
    append_fluid_patch(
        &mut patch,
        SectionPos::default(),
        &columns,
        &padded,
        &fluids,
        (false, &occluders),
    );
    let patch = patch.unwrap();
    assert!(has_top(&patch, [7.5, 8., 7.5]));
    for triangle in patch.indices.chunks_exact(3) {
        if patch.normals[triangle[0] as usize][1] < 0.99 {
            continue;
        }
        assert!(
            triangle
                .iter()
                .all(|i| (patch.positions[*i as usize][1] - 8.).abs() < 1e-5)
        );
    }
    assert_eq!(fluids, before);
}

#[test]
fn inner_and_outer_stair_corners_fill_each_actual_free_part_at_the_neighbors_water_level() {
    for facing in 0..4 {
        for turn in [-1, 1] {
            let p = [8, 7, 8];
            let forward = construction::stair_basis(facing)[2];
            let q = std::array::from_fn(|axis| p[axis] + i64::from(forward[axis]) * turn);
            let (patch, solids) = scene(
                &[(p, 74, facing), (q, 74, (facing + 1) % 4)],
                Some(8.),
                7,
                7,
            );
            for at in [p, q] {
                for x in [0.117, 0.491, 0.883] {
                    for z in [0.173, 0.547, 0.937] {
                        let point = [at[0] as f64 + x, 8., at[2] as f64 + z];
                        assert_eq!(
                            has_top(&patch, point),
                            !solid_at(&solids, point),
                            "stair-corner gap/solid overlap facing={facing} turn={turn} point={point:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn fence_water_fills_between_posts_and_only_the_rails_connected_in_that_pose() {
    for material in [90, 93, 97, 138] {
        for facing in 0..4 {
            for mask in [0, 1, 15] {
                let p = [8, 7, 8];
                let mut blocks = vec![(p, material, facing)];
                for (side, delta) in [[-1, 0], [1, 0], [0, -1], [0, 1]].into_iter().enumerate() {
                    if mask & (1 << side) != 0 {
                        blocks.push(([8 + delta[0], 7, 8 + delta[1]], material, 0));
                    }
                }
                let (patch, solids) = scene(&blocks, Some(7.375), 7, 7);
                for [x, z] in [
                    [0.12, 0.5],
                    [0.88, 0.5],
                    [0.5, 0.12],
                    [0.5, 0.88],
                    [0.12, 0.12],
                    [0.5, 0.5],
                ] {
                    let point = [8. + x, 7.375, 8. + z];
                    assert_eq!(
                        has_top(&patch, point),
                        !solid_at(&solids, point),
                        "fence water inherited a voxel/rail bounding box: material={material} facing={facing} mask={mask} point={point:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn fully_submerged_partial_models_never_draw_internal_vertical_voxel_walls() {
    for material in [
        74,
        82,
        90,
        93,
        97,
        138,
        plumbing::WOOD_PIPE,
        plumbing::STONE_PIPE,
        plumbing::IRON_PIPE,
        plumbing::GLASS_PIPE,
        plumbing::VALVE,
        plumbing::FAUCET,
        plumbing::PUMP,
    ] {
        for facing in 0..if material == plumbing::VALVE || plumbing::is_pipe(material) {
            6
        } else {
            4
        } {
            let (patch, _) = scene(&[([8, 7, 8], material, facing)], None, 6, 8);
            for triangle in patch.indices.chunks_exact(3) {
                let normal = patch.normals[triangle[0] as usize];
                if normal[1].abs() > 0.01 {
                    continue;
                }
                let center: [f32; 3] = std::array::from_fn(|axis| {
                    triangle
                        .iter()
                        .map(|i| patch.positions[*i as usize][axis])
                        .sum::<f32>()
                        / 3.
                });
                assert!(
                    (center[0] - 7.).abs() < 1e-5
                        || (center[0] - 10.).abs() < 1e-5
                        || (center[2] - 7.).abs() < 1e-5
                        || (center[2] - 10.).abs() < 1e-5,
                    "submerged model emitted a fake inner water/air wall: material={material} facing={facing} center={center:?} normal={normal:?}"
                );
            }
        }
    }
}
