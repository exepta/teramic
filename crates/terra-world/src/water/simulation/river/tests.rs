use super::*;
use crate::streaming::LoadedSection;
use terra_core::WorldConfig;
use terra_voxel::{BlockSection, SectionPos};

fn fixture(
    base: [i64; 2],
    player: bool,
) -> (
    TerraGenerator,
    ChunkStore,
    EditJournal,
    HashMap<[i64; 2], crate::SurfaceColumn>,
) {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: -20.0,
        sea_level: -30.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        trees: false,
        biomes: false,
        rivers: true,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for sx in (base[0] - 8).div_euclid(16)..=(base[0] + 8).div_euclid(16) {
        for sz in (base[1] - 2).div_euclid(16)..=(base[1] + 12).div_euclid(16) {
            for sy in 0..=1 {
                let p = SectionPos::new(sx, sy, sz);
                chunks.loaded.insert(
                    p,
                    LoadedSection {
                        data: BlockSection::empty(p).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: vec![],
                    },
                );
            }
        }
    }
    let mut columns = HashMap::new();
    let mut column = terrain.surface_column(0.5, 0.5);
    column.cave.water = None;
    column.river_cavity = None;
    column.water_level = None;
    for z in base[1] - 2..=base[1] + 12 {
        let floor = if z <= base[1] {
            20
        } else if z <= base[1] + 3 {
            14
        } else {
            8
        };
        for x in base[0] - 8..=base[0] + 8 {
            for y in 0..=floor {
                let (section, local) = global_to_section([x, y, z]);
                chunks
                    .loaded
                    .get_mut(&section)
                    .unwrap()
                    .data
                    .set_block(local, if (x + z) % 2 == 0 { 1 } else { 4 })
                    .unwrap();
            }
            let mut c = column;
            c.height = floor as f64;
            c.water_flow = if x == base[0] { [0.0, 1.0] } else { [0.0, 0.0] };
            if x == base[0] && z == base[1] {
                c.water_level = Some(23.0);
            }
            columns.insert([x, z], c);
        }
    }
    let source = [base[0], 22, base[1]];
    let mut journal = EditJournal::default();
    if player {
        let (owner, local) = global_to_section(source);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, 27)
            .unwrap();
        journal.record(source, 27).unwrap();
        journal
            .set_fluid(
                source,
                FluidCell {
                    amount: 8,
                    source: true,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    journal.wake_fluid_neighbors(source);
    (terrain, chunks, journal, columns)
}
fn step(
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    j: &mut EditJournal,
    columns: &HashMap<[i64; 2], crate::SurfaceColumn>,
) -> FluidStep {
    simulate_sampled_step(
        Sampler {
            terrain,
            chunks,
            columns: columns
                .iter()
                .map(|(p, c)| (*p, c.water_columns()))
                .collect(),
            spaces: HashMap::new(),
            river_columns: columns.clone(),
            river_origins: HashMap::new(),
        },
        j,
        256,
    )
    .unwrap()
}
#[test]
fn river_can_feed_a_connected_passage_under_its_bank() {
    let base = [15, 15];
    let (terrain, chunks, mut journal, mut columns) = fixture(base, false);
    // The dry surface terraces in fixture are untouched. A narrow gallery
    // under the higher nominal bank provides the alternative drainage route.
    for dx in 1..=4 {
        columns
            .get_mut(&[base[0] + dx, base[1] + 1])
            .unwrap()
            .height = 26.;
    }
    for _ in 0..160 {
        step(&terrain, &chunks, &mut journal, &columns);
    }
    assert!(
        journal.fluid_cells().any(|(p, c)| {
            p[0] >= base[0] + 2 && p[2] == base[1] + 1 && p[1] < 20 && c.amount > 0
        })
    );
    assert!(
        !journal
            .fluid_cells()
            .any(|(p, c)| { p[0] != base[0] && p[2] > base[1] + 2 && c.amount > 0 })
    );
}

#[test]
fn natural_falls_cross_section_seams_without_flooding_grass_and_sand_terraces_but_player_water_spreads()
 {
    for base in [[15, 15], [-17, -17]] {
        for player in [false, true] {
            let (terrain, chunks, mut j, columns) = fixture(base, player);
            for _ in 0..160 {
                assert!(step(&terrain, &chunks, &mut j, &columns).visited <= 256);
            }
            assert!(
                j.fluid([base[0], 16, base[1] + 1])
                    .is_some_and(|c| c.amount > 0 && !c.source),
                "real vertical fall must remain"
            );
            assert!(
                j.fluid_cells().any(|(p, c)| p[0] == base[0]
                    && p[2] >= base[1] + 4
                    && p[1] <= 14
                    && c.amount > 0),
                "the actual channel must reach the lower terrace"
            );
            let escaped = j
                .fluid_cells()
                .any(|(p, c)| p[0] != base[0] && p[2] > base[1] && c.amount > 0);
            assert_eq!(
                escaped, player,
                "natural river confines its landing, explicit bucket source retains ordinary spread"
            );
            assert!(
                j.fluid_cells()
                    .filter(|(_, c)| c.amount > 0 && !c.source)
                    .all(|(_, c)| c.amount <= 8)
            );
            if !player {
                // The actual fluid-cell mesh contains no lateral bank triangles.
                let fluids = j.fluid_cells().collect::<HashMap<_, _>>();
                let mut total = 0;
                for (&owner, loaded) in &chunks.loaded {
                    let origin = owner.origin().unwrap();
                    let mut padded = vec![3; 18 * 18 * 18];
                    for z in 0..18 {
                        for y in 0..18 {
                            for x in 0..18 {
                                let p = [
                                    origin[0] + x as i64 - 1,
                                    origin[1] + y as i64 - 1,
                                    origin[2] + z as i64 - 1,
                                ];
                                padded[x + 18 * (y + 18 * z)] = chunks.block_at(p).unwrap_or(3);
                            }
                        }
                    }
                    let flat = terrain.surface_column(0.5, 0.5);
                    let prepared = (0..18 * 18)
                        .map(|i| {
                            *columns
                                .get(&[
                                    origin[0] + (i % 18) as i64 - 1,
                                    origin[2] + (i / 18) as i64 - 1,
                                ])
                                .unwrap_or(&flat)
                        })
                        .collect::<Vec<_>>();
                    let mut patch = None;
                    crate::water::fluid_mesh::append_fluid_patch(
                        &mut patch, owner, &prepared, &padded, &fluids, true,
                    );
                    if let Some(patch) = patch {
                        total += patch.indices.len();
                        assert!(
                            patch.positions.iter().all(|p| {
                                let x = p[0] as f64 + origin[0] as f64;
                                x >= base[0] as f64 - 1e-6 && x <= (base[0] + 1) as f64 + 1e-6
                            }),
                            "mesh invented water across dry banks"
                        );
                    }
                    let _ = loaded;
                }
                assert!(total > 0);
            }
        }
    }
}
#[test]
fn river_proof_does_not_restrict_waterlogging_fixtures_or_explicit_terrain_channels() {
    let base = [15, 15];
    let (terrain, mut chunks, mut j, columns) = fixture(base, false);
    let target = [base[0] + 1, 15, base[1] + 1];
    let upstream = [base[0], 15, base[1] + 1];
    j.set_fluid(
        upstream,
        FluidCell {
            amount: 8,
            ..Default::default()
        },
    )
    .unwrap();
    for y in 16..=22 {
        j.set_fluid(
            [base[0], y, base[1] + 1],
            FluidCell {
                amount: 8,
                ..Default::default()
            },
        )
        .unwrap();
    }
    j.set_fluid(
        [base[0], 22, base[1] + 1],
        FluidCell {
            amount: 7,
            flow: [0, 1],
            source: false,
        },
    )
    .unwrap();
    let mut sample = Sampler {
        terrain: &terrain,
        chunks: &chunks,
        columns: columns
            .iter()
            .map(|(p, c)| (*p, c.water_columns()))
            .collect(),
        spaces: HashMap::new(),
        river_columns: columns.clone(),
        river_origins: HashMap::new(),
    };
    assert!(!lateral_allowed(&mut sample, &j, target, upstream));
    j.record([target[0], target[1] - 1, target[2]], 2).unwrap();
    assert!(lateral_allowed(&mut sample, &j, target, upstream));
    let (owner, local) = global_to_section(target);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, terra_core::plumbing::GLASS_PIPE)
        .unwrap();
    let mut sample = Sampler {
        terrain: &terrain,
        chunks: &chunks,
        columns: HashMap::new(),
        spaces: HashMap::new(),
        river_columns: columns,
        river_origins: HashMap::new(),
    };
    assert!(lateral_allowed(&mut sample, &j, target, upstream));
}
