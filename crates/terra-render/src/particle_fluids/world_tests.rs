use super::*;

fn saved_world() -> App {
    use terra_voxel::BlockSection;
    use terra_world::{TerraGenerator, streaming::LoadedSection};
    let mut app = App::new();
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        base_height: -64.,
        sea_level: -80.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        rivers: false,
        caves: false,
        trees: false,
        biomes: false,
        water: true,
        ..Default::default()
    })
    .unwrap();
    let mut data = BlockSection::empty(SectionPos::default()).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            data.set_block([x, 0, z], 3).unwrap();
        }
    }
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        SectionPos::default(),
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    app.insert_resource(chunks)
        .insert_resource(WorldTerrain(terrain))
        .insert_resource(ChunkFocus([8.; 3]))
        .init_resource::<EditJournal>()
        .init_resource::<BlockPropertiesCatalog>()
        .init_resource::<ParticleFluidInput>()
        .init_resource::<RestFluidGrid>()
        .init_resource::<Time>()
        .add_systems(Update, update);
    app
}

fn advance(app: &mut App) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs_f32(0.2));
    app.update();
}

#[test]
fn actual_saved_water_and_viscous_edits_drive_gpu_input_and_source_withdrawal() {
    use terra_world::{edits::BlockObjectState, water::FluidCell};
    let mut app = saved_world();
    {
        let world = app.world_mut();
        world
            .resource_mut::<EditJournal>()
            .set_fluid(
                [3, 2, 3],
                FluidCell {
                    amount: 8,
                    source: true,
                    flow: [1, 0],
                },
            )
            .unwrap();
        for (p, material) in [
            ([7, 2, 7], terra_core::bile::FLUID),
            ([11, 1, 11], terra_core::roads::WET_TAR),
        ] {
            world
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block(p.map(|v| v as usize), material)
                .unwrap();
            let mut journal = world.resource_mut::<EditJournal>();
            journal.record(p, material).unwrap();
            journal
                .set_object_state(
                    p,
                    Some(BlockObjectState {
                        facing: 7,
                        liquid: Some(terra_world::edits::LiquidState {
                            origin: p,
                            volume: 64,
                            age: if material == terra_core::bile::FLUID {
                                terra_core::bile::FINE
                            } else {
                                0
                            },
                            source: true,
                        }),
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
    }
    advance(&mut app);
    let input = app.world().resource::<ParticleFluidInput>();
    assert!(input.enabled);
    assert_eq!(input.target_mass, [1., 0., 1., 1. / 16.]);
    assert_eq!(
        (0..4)
            .map(|kind| input.particles.iter().filter(|p| p.kind == kind).count())
            .collect::<Vec<_>>(),
        [64, 0, 64, 64]
    );
    assert!(
        input
            .particles
            .iter()
            .filter(|p| p.kind == TAR)
            .all(|p| p.position[1] > 1.9375 && p.position[1] < 2.)
    );
    assert!(
        input
            .solids
            .iter()
            .any(|c| c.minimum == [0.; 3] && c.maximum == [16., 1., 16.])
    );
    assert_eq!(input.paths.len(), 1);
    let revision = input.revision;
    advance(&mut app);
    assert_eq!(
        app.world().resource::<ParticleFluidInput>().revision,
        revision,
        "A quiet loaded world must not rebuild or restart GPU particles"
    );
    {
        let world = app.world_mut();
        world
            .resource_mut::<EditJournal>()
            .set_fluid([3, 2, 3], FluidCell::default())
            .unwrap();
        for p in [[7, 2, 7], [11, 1, 11]] {
            world
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block(p.map(|v| v as usize), 0)
                .unwrap();
            world.resource_mut::<EditJournal>().record(p, 0).unwrap();
        }
    }
    advance(&mut app);
    let input = app.world().resource::<ParticleFluidInput>();
    assert!(!input.enabled);
    assert_eq!(input.target_mass, [0.; 4]);
    assert!(input.particles.iter().all(|p| p.kind == INACTIVE));
}

#[test]
fn actual_adapter_replaces_a_removed_pool_even_when_another_source_keeps_total_mass_identical() {
    use terra_world::water::FluidCell;
    let mut app = saved_world();
    let source = FluidCell {
        amount: 8,
        source: true,
        flow: [0; 2],
    };
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid([3, 2, 3], source)
        .unwrap();
    advance(&mut app);
    let previous = app
        .world()
        .resource::<ParticleFluidInput>()
        .particles
        .iter()
        .filter(|s| s.kind == WATER)
        .map(|s| s.generation)
        .max()
        .unwrap();
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.set_fluid([3, 2, 3], FluidCell::default()).unwrap();
        journal.set_fluid([12, 2, 3], source).unwrap();
    }
    advance(&mut app);
    let input = app.world().resource::<ParticleFluidInput>();
    assert_eq!(input.target_mass, [1., 0., 0., 0.]);
    let active: Vec<_> = input.particles.iter().filter(|s| s.kind == WATER).collect();
    assert_eq!(active.len(), 64);
    assert!(active.iter().all(|s| s.generation > previous));
    assert!(
        active
            .iter()
            .all(|s| s.position[0] > 12. && s.position[0] < 13.)
    );
}

#[test]
fn unloading_the_actual_source_section_leaves_no_ghost_mass_or_open_collision() {
    let mut app = saved_world();
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            [3, 2, 3],
            terra_world::water::FluidCell {
                amount: 8,
                source: true,
                flow: [0; 2],
            },
        )
        .unwrap();
    advance(&mut app);
    assert!(app.world().resource::<ParticleFluidInput>().enabled);
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    advance(&mut app);
    let input = app.world().resource::<ParticleFluidInput>();
    assert!(!input.enabled);
    assert_eq!(input.target_mass, [0.; 4]);
    assert!(input.particles.is_empty());
    assert!(input.solids.is_empty());
    assert!(input.paths.is_empty());
}

#[test]
fn full_liquid_on_a_vertical_section_border_reconstructs_in_the_invalidated_air_owner() {
    use terra_voxel::BlockSection;
    use terra_world::{streaming::LoadedSection, water::FluidCell};
    let mut app = saved_world();
    let upper = SectionPos::new(0, 1, 0);
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        upper,
        LoadedSection {
            data: BlockSection::empty(upper).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    app.init_resource::<FarFluidSections>()
        .add_systems(Update, update_far);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            [3, 15, 3],
            FluidCell {
                amount: 8,
                source: true,
                flow: [0; 2],
            },
        )
        .unwrap();
    {
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.fluid_epoch(upper), 0);
        assert_eq!(journal.mesh_epoch(upper), 0);
        assert!(journal.water_mesh_epoch(upper) > 0);
    }
    advance(&mut app);
    let output = app.world().resource::<FarFluidSections>();
    let field = &output.sections[&upper].fields[WATER as usize];
    assert!(field[3 + REST_SIDE * (REST_SIDE * 3)] > 0.);
    assert!(field[3 + REST_SIDE * (1 + REST_SIDE * 3)] < 0.);
    let revision = output.revision;
    advance(&mut app);
    assert_eq!(
        app.world().resource::<FarFluidSections>().revision,
        revision
    );
    // A loaded air section can lose all invalidation state when confirmed world
    // data is replaced. Revisit cached owners once to release their wet field.
    *app.world_mut().resource_mut::<EditJournal>() = EditJournal::default();
    advance(&mut app);
    assert!(
        !app.world()
            .resource::<FarFluidSections>()
            .sections
            .contains_key(&upper)
    );
    let revision = app.world().resource::<FarFluidSections>().revision;
    advance(&mut app);
    assert_eq!(
        app.world().resource::<FarFluidSections>().revision,
        revision
    );
}

#[test]
fn reconstructed_world_fields_share_borders_stay_cached_and_remove_empty_sections() {
    use terra_voxel::BlockSection;
    use terra_world::{streaming::LoadedSection, water::FluidCell};
    let mut app = saved_world();
    app.init_resource::<FarFluidSections>()
        .add_systems(Update, update_far);
    let neighbor = SectionPos::new(1, 0, 0);
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        neighbor,
        LoadedSection {
            data: BlockSection::empty(neighbor).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    for p in [[15, 2, 3], [16, 2, 3]] {
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_fluid(
                p,
                FluidCell {
                    amount: 8,
                    source: true,
                    flow: [0; 2],
                },
            )
            .unwrap();
    }
    advance(&mut app);
    let output = app.world().resource::<FarFluidSections>();
    assert_eq!(output.sections.len(), 2);
    let a = &output.sections[&SectionPos::default()].fields[WATER as usize];
    let b = &output.sections[&neighbor].fields[WATER as usize];
    assert!(a[15 + REST_SIDE * (2 + REST_SIDE * 3)] > 0.);
    for z in 0..17 {
        for y in 0..17 {
            assert_eq!(
                a[16 + 17 * (y + 17 * z)],
                b[17 * (y + 17 * z)],
                "Wet section seam must have one shared isosurface"
            );
        }
    }
    let revision = output.revision;
    advance(&mut app);
    assert_eq!(
        app.world().resource::<FarFluidSections>().revision,
        revision,
        "A quiet fluid surface must not rebuild meshes"
    );
    for p in [[15, 2, 3], [16, 2, 3]] {
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_fluid(p, FluidCell::default())
            .unwrap();
    }
    advance(&mut app);
    assert!(
        app.world()
            .resource::<FarFluidSections>()
            .sections
            .is_empty()
    );
}

#[test]
fn a_tiny_separate_puddle_keeps_the_world_surface_when_a_coarse_domain_cannot_sample_it() {
    use terra_world::water::FluidCell;
    let mut app = saved_world();
    for x in 0..16 {
        for y in 1..16 {
            for z in 0..16 {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_fluid(
                        [x, y, z],
                        FluidCell {
                            amount: 8,
                            source: x == 0 && y == 1 && z == 0,
                            flow: [0; 2],
                        },
                    )
                    .unwrap();
            }
        }
    }
    advance(&mut app);
    let input = app.world().resource::<ParticleFluidInput>();
    assert!(input.enabled);
    assert_eq!(input.target_mass[0], 3840.);
    assert_eq!(
        input.particles.iter().filter(|p| p.kind == WATER).count(),
        7680
    );
    // The water kind remains present, retaining its0.5m³ sample quantum, while
    // the deep domain becomes a shallower pool and one distant1/8m³ puddle.
    for x in 0..16 {
        for y in 1..16 {
            for z in 0..16 {
                let amount = if x < 8 && y < 7 && z < 8 {
                    8
                } else if [x, y, z] == [12, 1, 12] {
                    1
                } else {
                    0
                };
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_fluid(
                        [x, y, z],
                        FluidCell {
                            amount,
                            source: x == 0 && y == 1 && z == 0,
                            flow: [0; 2],
                        },
                    )
                    .unwrap();
            }
        }
    }
    advance(&mut app);
    let input = app.world().resource::<ParticleFluidInput>();
    assert_eq!(input.target_mass, [384.125, 0., 0., 0.]);
    assert!(
        !input.enabled,
        "Zero-quota water must retain the existing world surface"
    );
    assert!(input.particles.iter().all(|p| p.kind == INACTIVE));
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .fluid([12, 1, 12])
            .unwrap()
            .amount,
        1
    );
}

fn region(kind: u32) -> Region {
    Region {
        minimum: [2., 3., 4.],
        maximum: [3., 4., 5.],
        kind,
        source: false,
        velocity: [0.; 3],
        flow_origin: [2., 3., 4.],
        flow_radius: 0.,
    }
}

#[test]
fn shared_half_slab_excludes_its_real_solid_volume_and_never_seeds_inside_it() {
    let r = region(WATER);
    let solid = ShapeBox {
        minimum: [2., 3., 4.],
        maximum: [3., 3.5, 5.],
        hinged: false,
    };
    let free = subtract(r, solid);
    assert!((free.iter().map(|r| r.volume()).sum::<f32>() - 0.5).abs() < 0.00001);
    let mut slots = Vec::new();
    let mut generation = 0;
    reconcile(
        &mut slots,
        &free,
        [32, 0, 0, 0],
        [0.; 3],
        [0.; 3],
        &mut generation,
    );
    let particles: Vec<_> = slots.iter().filter(|p| p.kind == WATER).collect();
    assert_eq!(particles.len(), 32);
    assert!(
        particles
            .iter()
            .all(|p| p.position[1] > 3.5 && p.position[1] < 4.)
    );
}

#[test]
fn moving_volume_between_cells_keeps_live_particle_generations_and_source_mass() {
    let mut slots = Vec::new();
    let mut generation = 0;
    let mut a = region(WATER);
    a.source = true;
    reconcile(
        &mut slots,
        &[a],
        [64, 0, 0, 0],
        [0.; 3],
        [0.; 3],
        &mut generation,
    );
    let original = slots.clone();
    let mut downstream = a;
    downstream.minimum[0] += 1.;
    downstream.maximum[0] += 1.;
    downstream.source = false;
    // Same cubic metre moved owners. GPU positions must continue independently.
    reconcile(
        &mut slots,
        &[downstream],
        [64, 0, 0, 0],
        [0.; 3],
        [0.; 3],
        &mut generation,
    );
    for (old, new) in original.iter().zip(&slots).filter(|(_, p)| p.kind == WATER) {
        assert_eq!(old.generation, new.generation);
        assert!(
            new.position[0] >= downstream.minimum[0] && new.position[0] < downstream.maximum[0]
        );
    }
    reconcile(
        &mut slots,
        &[a, downstream],
        [72, 0, 0, 0],
        [0.; 3],
        [0.; 3],
        &mut generation,
    );
    let new: Vec<_> = slots
        .iter()
        .filter(|p| p.kind == WATER && p.generation > 64)
        .collect();
    assert_eq!(new.len(), 8);
    assert!(
        new.iter()
            .all(|p| p.position[0] >= a.minimum[0] && p.position[0] < a.maximum[0]),
        "New supplied mass must enter at its source, never pop into the receiving voxel"
    );
    reconcile(&mut slots, &[], [0; 4], [0.; 3], [0.; 3], &mut generation);
    assert!(slots.iter().all(|p| p.kind == INACTIVE));
}

#[test]
fn rebasing_a_negative_domain_preserves_overlapping_particle_generations_and_world_origins() {
    let mut slots = Vec::new();
    let mut generation = 0;
    let mut before = region(GALLE);
    before.minimum = [8.; 3];
    before.maximum = [9.; 3];
    before.flow_origin = [8.; 3];
    before.flow_radius = 4.;
    reconcile(
        &mut slots,
        &[before],
        [0, 0, 64, 0],
        [-16., -32., -48.],
        [-16., -32., -48.],
        &mut generation,
    );
    let old = slots.clone();
    let mut after = before;
    after.minimum = [4.; 3];
    after.maximum = [5.; 3];
    after.flow_origin = [4.; 3];
    reconcile(
        &mut slots,
        &[after],
        [0, 0, 64, 0],
        [-16., -32., -48.],
        [-12., -28., -44.],
        &mut generation,
    );
    for (a, b) in old.iter().zip(&slots).filter(|(_, p)| p.kind == GALLE) {
        assert_eq!(a.generation, b.generation);
        for axis in 0..3 {
            assert!(
                (a.position[axis] + [-16., -32., -48.][axis]
                    - b.position[axis]
                    - [-12., -28., -44.][axis])
                    .abs()
                    < 0.00001
            );
            assert!(
                (a.flow_origin[axis] + [-16., -32., -48.][axis]
                    - b.flow_origin[axis]
                    - [-12., -28., -44.][axis])
                    .abs()
                    < 0.00001
            );
        }
        assert_eq!(b.flow_radius, 4.);
    }
}

#[test]
fn a_dense_ocean_cannot_starve_the_other_three_liquids_or_exceed_the_fixed_pool() {
    assert_eq!(counts([1., 0., 0., 0.]), [64, 0, 0, 0]);
    assert_eq!(counts([0., 0., 0., 1. / 16.]), [0, 0, 0, 64]);
    let count = counts([4096., 1., 1., 1. / 16.]);
    assert!(count.iter().sum::<usize>() <= MAX_PARTICLES);
    assert!(count[1..].iter().all(|&n| n == 64));
    let mixed = counts([4096.; 4]);
    assert_eq!(mixed.iter().sum::<usize>(), MAX_PARTICLES);
    assert!(mixed.iter().all(|&n| n >= 256));
}

#[test]
fn collision_compaction_is_exact_and_does_not_close_a_gap_or_merge_kind_masks() {
    let mut boxes = Vec::new();
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..4 {
                boxes.push(FluidCollider {
                    minimum: [x as f32, y as f32, z as f32],
                    maximum: [x as f32 + 1., y as f32 + 1., z as f32 + 1.],
                    blocked_kinds: ALL_KINDS,
                });
            }
        }
    }
    merge_colliders(&mut boxes);
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].maximum, [16., 4., 16.]);
    boxes.push(FluidCollider {
        minimum: [0., 5., 0.],
        maximum: [16., 6., 16.],
        blocked_kinds: ALL_KINDS,
    });
    boxes.push(FluidCollider {
        minimum: [0., 4., 0.],
        maximum: [16., 5., 16.],
        blocked_kinds: 1 << TAR,
    });
    merge_colliders(&mut boxes);
    assert_eq!(boxes.len(), 3);
}

#[test]
fn disjoint_same_mass_pool_replacement_retires_old_generations_and_origins() {
    for kind in [WATER, GALLE, TAR] {
        let mut a = region(kind);
        a.source = true;
        a.flow_radius = if kind >= GALLE { 4. } else { 0. };
        let mut b = a;
        b.minimum[0] += 10.;
        b.maximum[0] += 10.;
        b.flow_origin[0] += 10.;
        let mut slots = Vec::new();
        let mut generation = 0;
        let mut wanted = [0; 4];
        wanted[kind as usize] = 64;
        reconcile(&mut slots, &[a], wanted, [0.; 3], [0.; 3], &mut generation);
        let previous = generation;
        reconcile(&mut slots, &[b], wanted, [0.; 3], [0.; 3], &mut generation);
        let active: Vec<_> = slots.iter().filter(|s| s.kind == kind).collect();
        assert_eq!(active.len(), 64);
        assert!(active.iter().all(|s| s.generation > previous));
        assert!(
            active
                .iter()
                .all(|s| s.position[0] >= 12. && s.position[0] < 13.)
        );
        assert!(active.iter().all(|s| s.flow_origin == b.flow_origin));
    }
}

#[test]
fn removing_one_of_two_separate_basins_preserves_the_other_basin() {
    let a = region(WATER);
    let mut b = a;
    b.minimum[0] += 10.;
    b.maximum[0] += 10.;
    let mut slots = Vec::new();
    let mut generation = 0;
    reconcile(
        &mut slots,
        &[a, b],
        [128, 0, 0, 0],
        [0.; 3],
        [0.; 3],
        &mut generation,
    );
    let basin_b: Vec<_> = slots
        .iter()
        .filter(|s| s.kind == WATER && s.position[0] >= 12.)
        .map(|s| s.generation)
        .collect();
    assert_eq!(basin_b.len(), 64);
    reconcile(
        &mut slots,
        &[b],
        [64, 0, 0, 0],
        [0.; 3],
        [0.; 3],
        &mut generation,
    );
    let kept: Vec<_> = slots
        .iter()
        .filter(|s| s.kind == WATER)
        .map(|s| s.generation)
        .collect();
    assert_eq!(kept, basin_b);
    assert_eq!(
        generation, 128,
        "Unrelated basin must neither refill nor restart"
    );
}

#[test]
fn splitting_a_connected_pool_assigns_each_new_basin_its_own_mass() {
    let mut regions = Vec::new();
    for x in 2..=12 {
        let mut r = region(WATER);
        r.minimum[0] = x as f32;
        r.maximum[0] = x as f32 + 1.;
        regions.push(r);
    }
    let mut slots = Vec::new();
    let mut generation = 0;
    reconcile(
        &mut slots,
        &regions,
        [704, 0, 0, 0],
        [0.; 3],
        [0.; 3],
        &mut generation,
    );
    reconcile(
        &mut slots,
        &[regions[0], regions[10]],
        [128, 0, 0, 0],
        [0.; 3],
        [0.; 3],
        &mut generation,
    );
    let active: Vec<_> = slots.iter().filter(|s| s.kind == WATER).collect();
    assert_eq!(active.len(), 128);
    assert_eq!(active.iter().filter(|s| s.position[0] < 3.).count(), 64);
    assert_eq!(active.iter().filter(|s| s.position[0] >= 12.).count(), 64);
}

#[test]
fn continuous_adjacent_transfers_advance_the_anchor_without_restarting_gpu_motion() {
    let mut r = region(WATER);
    let mut slots = Vec::new();
    let mut generation = 0;
    reconcile(
        &mut slots,
        &[r],
        [64, 0, 0, 0],
        [0.; 3],
        [0.; 3],
        &mut generation,
    );
    let original: Vec<_> = slots
        .iter()
        .filter(|s| s.kind == WATER)
        .map(|s| s.generation)
        .collect();
    for _ in 0..8 {
        r.minimum[0] += 1.;
        r.maximum[0] += 1.;
        reconcile(
            &mut slots,
            &[r],
            [64, 0, 0, 0],
            [0.; 3],
            [0.; 3],
            &mut generation,
        );
        assert_eq!(
            slots
                .iter()
                .filter(|s| s.kind == WATER)
                .map(|s| s.generation)
                .collect::<Vec<_>>(),
            original
        );
        assert!(
            slots
                .iter()
                .filter(|s| s.kind == WATER)
                .all(|s| s.position[0] >= r.minimum[0] && s.position[0] < r.maximum[0])
        );
    }
}

#[test]
fn moving_domain_keeps_overlap_and_never_concentrates_every_fallback_into_the_entering_slab() {
    let before: Vec<_> = (0..16)
        .map(|x| {
            let mut r = region(WATER);
            r.minimum[0] = x as f32;
            r.maximum[0] = x as f32 + 1.;
            r
        })
        .collect();
    let mut slots = Vec::new();
    let mut generation = 0;
    reconcile(
        &mut slots,
        &before,
        [1024, 0, 0, 0],
        [0.; 3],
        [0.; 3],
        &mut generation,
    );
    let previous = generation;
    reconcile(
        &mut slots,
        &before,
        [1024, 0, 0, 0],
        [0.; 3],
        [4., 0., 0.],
        &mut generation,
    );
    let retained: Vec<_> = slots
        .iter()
        .filter(|s| s.kind == WATER && s.generation <= previous)
        .collect();
    assert_eq!(retained.len(), 768);
    let edge = retained.iter().filter(|s| s.position[0] >= 12.).count();
    assert_eq!(
        edge, 192,
        "Fallbacks sample the whole pool rather than concentrating into the entering25%"
    );
    let incoming: Vec<_> = slots
        .iter()
        .filter(|s| s.kind == WATER && s.generation > previous)
        .collect();
    assert_eq!(incoming.len(), 256);
    assert!(
        incoming
            .iter()
            .all(|s| s.position[0] >= 12. && s.position[0] < 16.)
    );
}

#[test]
fn touching_acid_pools_from_different_sources_never_share_quota_or_provenance() {
    let mut a = region(GALLE);
    a.flow_radius = 4.;
    let mut b = a;
    b.minimum[0] += 1.;
    b.maximum[0] += 1.;
    b.flow_origin[0] += 1.;
    let old_domain = ShapeBox {
        minimum: [0.; 3],
        maximum: [16.; 3],
        hinged: false,
    };
    let pools = pools(&[a, b], [0, 0, 128, 0], old_domain);
    assert_eq!(pools.len(), 2);
    assert!(pools.iter().all(|p| p.wanted == 64));
}

#[test]
fn camera_mass_shrink_and_return_keep_particle_volume_stable_under_the_8192_slot_budget() {
    let mut resolution = Resolution::default();
    let (large, _) = resolution.counts([384., 0., 0., 0.]);
    let (small, changed) = resolution.counts([312., 0., 0., 0.]);
    let (back, changed_back) = resolution.counts([384., 0., 0., 0.]);
    assert_eq!(large, [6144, 0, 0, 0]);
    assert_eq!(small, [4992, 0, 0, 0]);
    assert_eq!(back, large);
    assert_eq!(changed, [false; 4]);
    assert_eq!(changed_back, [false; 4]);
    assert_eq!(384. / large[0] as f32, 312. / small[0] as f32);
    assert_eq!(resolution.quantum[0], 1. / 16.);
    assert!(large.iter().sum::<usize>() <= MAX_PARTICLES);
}

#[test]
fn mixed_fluids_keep_their_resolution_when_the_ocean_leaves_and_returns() {
    let mut resolution = Resolution::default();
    let (a, _) = resolution.counts([384., 1., 1., 1. / 16.]);
    let quantum = resolution.quantum;
    let (b, changed) = resolution.counts([312., 1., 1., 1. / 16.]);
    let (c, changed_back) = resolution.counts([384., 1., 1., 1. / 16.]);
    assert_eq!(a, c);
    assert_eq!(changed, [false; 4]);
    assert_eq!(changed_back, [false; 4]);
    assert_eq!(resolution.quantum, quantum);
    assert_eq!(b[1..], [64; 3]);
    assert_eq!(384. / a[0] as f32, 312. / b[0] as f32);
}

#[test]
fn rebase_mass_shrink_adds_only_actual_incoming_volume_and_has_no_edge_clump() {
    // Exact native regression:384m³ old window,288m³ overlap,24m³
    // entering shallows. The previous8192→8192 allocation overfilled that strip.
    fn channel(shallow: bool) -> Vec<Region> {
        let mut out = Vec::new();
        for x in 0..16 {
            for z in 0..16 {
                let mut r = region(WATER);
                r.minimum = [x as f32, 1., z as f32];
                r.maximum = [
                    x as f32 + 1.,
                    if shallow && x >= 12 { 1.375 } else { 2. },
                    z as f32 + 1.,
                ];
                out.push(r);
                if !shallow || x < 12 {
                    r.minimum[1] = 2.;
                    r.maximum[1] = 2.5;
                    out.push(r);
                }
            }
        }
        out
    }
    let mut resolution = Resolution::default();
    let mut slots = Vec::new();
    let mut anchors = Vec::new();
    let mut generation = 0;
    let large = resolution.counts([384., 0., 0., 0.]).0;
    reconcile_with_anchors(
        &mut slots,
        &channel(false),
        large,
        [0.; 3],
        [0.; 3],
        &mut generation,
        &mut anchors,
    );
    let first = generation;
    let mut physical: Vec<_> = slots
        .iter()
        .map(|s| (s.position, s.generation, s.kind))
        .collect();
    let small = resolution.counts([312., 0., 0., 0.]).0;
    reconcile_with_anchors(
        &mut slots,
        &channel(true),
        small,
        [0.; 3],
        [4., 0., 0.],
        &mut generation,
        &mut anchors,
    );
    let new: Vec<_> = slots
        .iter()
        .filter(|s| s.kind == WATER && s.generation > first)
        .collect();
    assert_eq!(new.len(), 384, "Exactly24m³ enters at1/16m³ per particle");
    assert!(
        new.iter()
            .all(|s| s.position[0] >= 12. && s.position[1] < 1.375)
    );
    // Model only the actual GPU reconcile branch, without inventing a CPU readback.
    for (old, seed) in physical.iter_mut().zip(&slots) {
        let shifted = [old.0[0] - 4., old.0[1], old.0[2]];
        old.0 = if old.1 != seed.generation || shifted.iter().any(|&v| !(0.0..16.).contains(&v)) {
            seed.position
        } else {
            shifted
        };
        old.1 = seed.generation;
        old.2 = seed.kind;
    }
    let edge = physical
        .iter()
        .filter(|p| p.2 == WATER && p.0[0] >= 12.)
        .count();
    assert_eq!(
        edge, 384,
        "The entering shallows cannot contain a hidden resolution increase"
    );
    let second = generation;
    let back = resolution.counts([384., 0., 0., 0.]).0;
    reconcile_with_anchors(
        &mut slots,
        &channel(false),
        back,
        [4., 0., 0.],
        [0.; 3],
        &mut generation,
        &mut anchors,
    );
    let new: Vec<_> = slots
        .iter()
        .filter(|s| s.kind == WATER && s.generation > second)
        .collect();
    assert_eq!(
        new.len(),
        1536,
        "Returning96m³ has the same stable per-particle volume"
    );
    assert!(new.iter().all(|s| s.position[0] < 4.));
    assert_eq!(slots.iter().filter(|s| s.kind == WATER).count(), 6144);
}

#[test]
fn a_confirmed_large_pool_uses_worker_geometry_without_camera_reseeding_or_voxel_recapture() {
    use terra_world::water::{FluidCell, WaterPatch};
    let mut app = saved_world();
    app.init_resource::<FarFluidSections>()
        .add_systems(Update, update_far);
    for x in 3..7 {
        for z in 3..5 {
            app.world_mut()
                .resource_mut::<EditJournal>()
                .set_fluid(
                    [x, 1, z],
                    FluidCell {
                        amount: 8,
                        source: true,
                        flow: [0; 2],
                    },
                )
                .unwrap();
        }
    }
    let epoch = app
        .world()
        .resource::<EditJournal>()
        .water_mesh_epoch(SectionPos::default());
    let patch = WaterPatch {
        positions: vec![[3., 2., 3.], [3., 2., 5.], [7., 2., 5.], [7., 2., 3.]],
        normals: vec![[0., 1., 0.]; 4],
        flow: vec![[0.; 2]; 4],
        depth_shore: vec![[1., 0.]; 4],
        surface_effects: vec![[0.; 4]; 4],
        wave_weights: vec![0.; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
    };
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let section = chunks.loaded.get_mut(&SectionPos::default()).unwrap();
        section.water = Some(patch.clone());
        section.water_epoch = epoch;
    }
    advance(&mut app);
    let input = app.world().resource::<ParticleFluidInput>();
    assert!(!input.enabled);
    assert!(input.particles.is_empty());
    assert_eq!(input.target_mass, [0.; 4]);
    let input_revision = input.revision;
    let surfaces = app.world().resource::<FarFluidSections>();
    assert_eq!(
        surfaces.sampled_cells, 0,
        "A published pool must not scan its volume again on the main thread"
    );
    assert_eq!(surfaces.body_patches, 1);
    assert_eq!(
        surfaces.sections[&SectionPos::default()]
            .published_water
            .as_ref(),
        Some(&patch)
    );
    let revision = surfaces.revision;
    for position in [[8., 14., 8.], [12., 20., 8.], [32., 48., 32.], [8., 3., 8.]] {
        app.world_mut().resource_mut::<ChunkFocus>().0 = position;
        advance(&mut app);
        assert_eq!(
            app.world().resource::<ParticleFluidInput>().revision,
            input_revision,
            "Flying near or above a reservoir must not inject particle energy"
        );
        let surfaces = app.world().resource::<FarFluidSections>();
        assert_eq!(surfaces.revision, revision);
        assert_eq!(surfaces.sampled_cells, 0);
        assert_eq!(surfaces.body_patches, 1);
    }
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    advance(&mut app);
    assert!(
        app.world()
            .resource::<FarFluidSections>()
            .sections
            .is_empty()
    );
}
