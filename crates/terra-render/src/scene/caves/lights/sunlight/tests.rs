use super::*;
use terra_voxel::BlockSection;
use terra_world::streaming::LoadedSection;

const OWNER: [i64; 3] = [8, 2, 8];

fn fixture() -> (ChunkStore, EditJournal) {
    let mut chunks = ChunkStore::default();
    for x in -1..=1 {
        for z in -1..=1 {
            for y in 0..=2 {
                let position = SectionPos::new(x, y, z);
                chunks.loaded.insert(
                    position,
                    LoadedSection {
                        data: BlockSection::empty(position).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
        }
    }
    put(&mut chunks, OWNER, 40);
    (chunks, EditJournal::default())
}

fn put(chunks: &mut ChunkStore, p: [i64; 3], material: u16) {
    let (section, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
}

fn sample(chunks: &ChunkStore, journal: &EditJournal, natural: f64, sun: Vec3) -> Visibility {
    let mut bounds = HashMap::new();
    classify(OWNER, Vec3::new(8.5, 2.98, 8.5), sun, |p| {
        if p[1] > MAX_Y {
            return Some(0);
        }
        let column = [p[0].div_euclid(16), p[2].div_euclid(16)];
        // Immutable, flat prepared-column upper bounds supplied to the same live
        // roof-bound function used by the production adapter.
        let top = *bounds
            .entry(column)
            .or_insert_with(|| known_top(chunks, journal, column, natural).unwrap());
        if p[1] as f64 > top {
            Some(0)
        } else {
            chunks
                .block_at(p)
                .map(|raw| journal.block(p).unwrap_or(raw))
        }
    })
}

#[test]
fn strong_sun_only_never_disables_torch_shadows_at_night_dusk_or_heavy_clouds() {
    assert!(sunlight(100_000.0, Vec3::Y).is_some());
    for lux in [0.0, 2200.0, 30_000.0, 59_999.0, f32::NAN] {
        assert!(sunlight(lux, Vec3::Y).is_none());
    }
    assert!(sunlight(100_000.0, Vec3::NEG_Y).is_none());
    for phase in [0.0, 0.24, 0.25, 0.26, 0.74, 0.75, 0.76] {
        let daylight = crate::scene::daylight::Daylight::at(phase);
        assert!(
            sunlight(daylight.illuminance(true), daylight.sun).is_none(),
            "{phase}"
        );
    }
    let noon = crate::scene::daylight::Daylight::at(0.5);
    assert!(sunlight(noon.illuminance(true), noon.sun).is_some());
    let mut state = State::default();
    state.entries.insert(
        OWNER,
        Entry {
            origin: Vec3::ZERO,
            checked: 0.0,
            visibility: Visibility::Sunlit,
            dependencies: Vec::new(),
        },
    );
    assert!(!state.shadows(OWNER));
    state.entries.clear();
    assert!(state.shadows(OWNER));
}

#[test]
fn sunlit_torch_preserves_roof_cave_and_side_wall_shadows_with_actual_loaded_geometry() {
    let (mut chunks, journal) = fixture();
    assert_eq!(sample(&chunks, &journal, 1.0, Vec3::Y), Visibility::Sunlit);
    // Thin/custom roofs also retain shadows; transparency/shape ambiguity is conservative.
    for roof in [3, 7, 15, 65, 74, 82] {
        put(&mut chunks, [8, 7, 8], roof);
        assert_eq!(
            sample(&chunks, &journal, 1.0, Vec3::Y),
            Visibility::Sheltered,
            "roof {roof}"
        );
    }
    put(&mut chunks, [8, 7, 8], 0);
    // A cave below its natural surface is not classified from height alone.
    put(&mut chunks, [8, 20, 8], 3);
    assert_eq!(
        sample(&chunks, &journal, 24.0, Vec3::Y),
        Visibility::Sheltered
    );
    put(&mut chunks, [8, 20, 8], 0);
    // Roof rays are free, but the actual oblique solar ray meets this wall.
    put(&mut chunks, [11, 5, 8], 3);
    assert_eq!(
        sample(&chunks, &journal, 1.0, Vec3::new(1.0, 1.0, 0.0).normalize()),
        Visibility::Sheltered
    );
}

#[test]
fn edited_roof_above_generated_height_retains_shadows_and_reopens_after_removal() {
    let (mut chunks, mut journal) = fixture();
    let dependency = Dependency::capture([0, 0], &journal);
    journal.record([8, 22, 8], 3).unwrap();
    assert!(dependency.changed(&journal));
    assert!(known_top(&chunks, &journal, [0, 0], 1.0).unwrap() >= 31.0);
    assert_eq!(
        sample(&chunks, &journal, 1.0, Vec3::Y),
        Visibility::Sheltered
    );
    // A known edit in an unloaded section is not proof of open sky.
    let section = chunks.loaded.remove(&SectionPos::new(0, 1, 0)).unwrap();
    assert_eq!(sample(&chunks, &journal, 1.0, Vec3::Y), Visibility::Unknown);
    chunks.loaded.insert(SectionPos::new(0, 1, 0), section);
    journal.record([8, 22, 8], 0).unwrap();
    assert_eq!(sample(&chunks, &journal, 1.0, Vec3::Y), Visibility::Sunlit);
    let dependency = Dependency::capture([0, 0], &journal);
    journal.record([40, 22, 40], 3).unwrap();
    assert!(
        !dependency.changed(&journal),
        "unrelated distant edits do not invalidate this source"
    );
}

#[test]
fn missing_roof_data_and_exhausted_probe_domains_never_count_as_sunlight() {
    let (mut chunks, journal) = fixture();
    chunks.loaded.remove(&SectionPos::new(0, 1, 0));
    assert_eq!(
        sample(&chunks, &journal, 22.0, Vec3::Y),
        Visibility::Unknown
    );
    let mut columns = Columns {
        chunks: &chunks,
        journal: &journal,
        bounds: HashMap::new(),
        dependencies: Vec::new(),
        probes: 0,
    };
    assert_eq!(
        columns.read([8, 3, 8]),
        None,
        "missing prepared metadata is not sky"
    );
    columns.probes = MAX_PROBES;
    assert_eq!(columns.read([8, 3, 8]), None);
    let mut count = 0;
    let result = classify(
        [8, -128, 8],
        Vec3::new(8.5, -127.0, 8.5),
        Vec3::new(1.0, 1.0, 0.0).normalize(),
        |_| {
            count += 1;
            Some(0)
        },
    );
    assert_eq!(result, Visibility::Sunlit);
    assert!(count < MAX_PROBES, "{count} bounded DDA probes");
}

#[test]
fn shadow_cache_is_bounded_by_the_admitted_quality_pool_and_clears_when_sun_fades() {
    let (mut chunks, journal) = fixture();
    let mut app = App::new();
    for x in 0..32 {
        let p = [x, 2, 8];
        put(&mut chunks, p, 40);
        app.world_mut().spawn(VoxelLamp(
            p,
            BlockLight {
                color: [1.0; 3],
                intensity: 1000.0,
                range: 28.0,
                falloff: Default::default(),
                flicker: 0.0,
                cave_intensity: 1.0,
                offset: None,
                shadows: true,
            },
            Vec3::new(x as f32 + 0.5, 2.98, 8.5),
        ));
    }
    let sun = app
        .world_mut()
        .spawn((
            crate::scene::daylight::Sun,
            DirectionalLight {
                illuminance: 100_000.0,
                ..default()
            },
            Transform::from_translation(Vec3::Y).looking_at(Vec3::ZERO, Vec3::Z),
        ))
        .id();
    app.init_resource::<Time>()
        .init_resource::<State>()
        .insert_resource(chunks)
        .insert_resource(journal)
        .insert_resource(WorldTerrain(
            terra_world::TerraGenerator::new(Default::default()).unwrap(),
        ))
        .add_systems(Update, update);
    app.update();
    let state = app.world().resource::<State>();
    assert_eq!(state.entries.len(), 8);
    assert!(
        state
            .entries
            .values()
            .all(|e| e.visibility == Visibility::Unknown)
    );
    assert!(state.shadows(OWNER));
    app.world_mut()
        .get_mut::<DirectionalLight>(sun)
        .unwrap()
        .illuminance = 0.0;
    app.update();
    assert!(app.world().resource::<State>().entries.is_empty());
    assert!(app.world().resource::<State>().shadows(OWNER));
}

#[test]
fn cave_or_underwater_view_restores_sunlit_source_shadows_without_waiting_for_expiry() {
    let (chunks, journal) = fixture();
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<State>()
        .init_resource::<crate::scene::caves::CaveState>()
        .init_resource::<crate::water::UnderwaterState>()
        .insert_resource(chunks)
        .insert_resource(journal)
        .insert_resource(WorldTerrain(
            terra_world::TerraGenerator::new(Default::default()).unwrap(),
        ))
        .add_systems(Update, update);
    app.world_mut().spawn((
        crate::scene::daylight::Sun,
        DirectionalLight {
            illuminance: 100_000.0,
            ..default()
        },
        Transform::from_translation(Vec3::Y).looking_at(Vec3::ZERO, Vec3::Z),
    ));
    app.world_mut().spawn(VoxelLamp(
        OWNER,
        BlockLight {
            color: [1.0; 3],
            intensity: 1000.0,
            range: 28.0,
            falloff: Default::default(),
            flicker: 0.0,
            cave_intensity: 1.0,
            offset: None,
            shadows: true,
        },
        Vec3::new(8.5, 2.98, 8.5),
    ));
    app.update();
    assert_eq!(app.world().resource::<State>().entries.len(), 1);
    for guard in 0..5 {
        *app.world_mut()
            .resource_mut::<crate::scene::caves::CaveState>() = Default::default();
        *app.world_mut()
            .resource_mut::<crate::water::UnderwaterState>() = Default::default();
        match guard {
            0 => {
                app.world_mut()
                    .resource_mut::<crate::scene::caves::CaveState>()
                    .roof_coverage = 0.1
            }
            1 => {
                app.world_mut()
                    .resource_mut::<crate::scene::caves::CaveState>()
                    .blend = 0.1
            }
            2 => {
                app.world_mut()
                    .resource_mut::<crate::water::UnderwaterState>()
                    .submerged = true
            }
            3 => {
                app.world_mut()
                    .resource_mut::<crate::water::UnderwaterState>()
                    .blend = 0.1
            }
            _ => {
                app.world_mut()
                    .resource_mut::<crate::water::UnderwaterState>()
                    .blend = f32::NAN
            }
        }
        app.world_mut().resource_mut::<State>().entries.insert(
            OWNER,
            Entry {
                origin: Vec3::new(8.5, 2.98, 8.5),
                checked: 0.0,
                visibility: Visibility::Sunlit,
                dependencies: Vec::new(),
            },
        );
        assert!(!app.world().resource::<State>().shadows(OWNER));
        app.update();
        assert!(
            app.world().resource::<State>().entries.is_empty(),
            "guard {guard}"
        );
        assert!(app.world().resource::<State>().shadows(OWNER));
    }
}

fn cached_app() -> App {
    let (chunks, journal) = fixture();
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<State>()
        .insert_resource(chunks)
        .insert_resource(journal)
        .insert_resource(WorldTerrain(
            terra_world::TerraGenerator::new(Default::default()).unwrap(),
        ))
        .add_message::<SectionPublished>()
        .add_message::<SectionRemoved>()
        .add_message::<SectionEditApplied>()
        .add_message::<PreparedColumnChanged>()
        .add_systems(Update, update);
    app.world_mut().spawn((
        crate::scene::daylight::Sun,
        DirectionalLight {
            illuminance: 100_000.0,
            ..default()
        },
        Transform::from_translation(Vec3::Y).looking_at(Vec3::ZERO, Vec3::Z),
    ));
    app.world_mut().spawn(VoxelLamp(
        OWNER,
        BlockLight {
            color: [1.0; 3],
            intensity: 1000.0,
            range: 28.0,
            falloff: Default::default(),
            flicker: 0.0,
            cave_intensity: 1.0,
            offset: None,
            shadows: true,
        },
        Vec3::new(8.5, 2.98, 8.5),
    ));
    // Settle resource change detection before supplying the known-open cached result.
    app.update();
    cache_open_sky(&mut app);
    app
}

fn cache_open_sky(app: &mut App) {
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(
        sample(app.world().resource::<ChunkStore>(), journal, 1.0, Vec3::Y),
        Visibility::Sunlit
    );
    let dependency = Dependency::capture([0, 0], journal);
    let checked = app.world().resource::<Time>().elapsed_secs();
    app.world_mut().resource_mut::<State>().entries.insert(
        OWNER,
        Entry {
            origin: Vec3::new(8.5, 2.98, 8.5),
            checked,
            visibility: Visibility::Sunlit,
            dependencies: vec![dependency],
        },
    );
}

fn advance_cache_time(app: &mut App) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(50));
}

#[test]
fn metadata_edit_messages_reuse_rays_but_a_new_roof_invalidates_before_expiry() {
    let mut app = cached_app();
    let position = global_to_section(OWNER).0;
    for epoch in 0..4 {
        let mut delta = app
            .world()
            .resource::<EditJournal>()
            .section_delta(position);
        match epoch {
            0 => delta.fluid_epoch += 1,
            1 => delta.prop_epoch += 1,
            2 => delta.appearance_epoch += 1,
            _ => {} // Repeated metadata still emits the same notification.
        }
        app.world_mut()
            .resource_mut::<EditJournal>()
            .bypass_change_detection()
            .import_delta(delta)
            .unwrap();
        app.world_mut()
            .write_message(SectionEditApplied { position });
        advance_cache_time(&mut app);
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.entries[&OWNER].checked, 0.0, "epoch {epoch}");
        assert_eq!(state.entries[&OWNER].visibility, Visibility::Sunlit);
        assert!(!state.shadows(OWNER));
    }

    let roof = [8, 7, 8];
    app.world_mut()
        .resource_mut::<EditJournal>()
        .bypass_change_detection()
        .record(roof, 3)
        .unwrap();
    assert_eq!(
        sample(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            1.0,
            Vec3::Y,
        ),
        Visibility::Sheltered,
        "the confirmed roof really intersects the previously open rays"
    );
    app.world_mut()
        .write_message(SectionEditApplied { position });
    advance_cache_time(&mut app);
    app.update();
    let state = app.world().resource::<State>();
    assert_eq!(
        state.entries[&OWNER].checked,
        app.world().resource::<Time>().elapsed_secs()
    );
    assert!(state.entries[&OWNER].checked < REFRESH_SECONDS);
    // Missing prepared metadata remains conservative; never preserve Sunlit
    // after the real terrain revision changes, even with change detection bypassed.
    assert_ne!(state.entries[&OWNER].visibility, Visibility::Sunlit);
    assert!(state.shadows(OWNER));
}

#[test]
fn publication_removal_and_prepared_column_messages_still_invalidate_cached_rays() {
    let mut app = cached_app();
    let position = global_to_section(OWNER).0;
    for event in 0..3 {
        cache_open_sky(&mut app);
        let before = app.world().resource::<State>().entries[&OWNER].checked;
        match event {
            0 => {
                app.world_mut().write_message(SectionPublished { position });
            }
            1 => {
                app.world_mut().write_message(SectionRemoved { position });
            }
            _ => {
                app.world_mut().write_message(PreparedColumnChanged {
                    coordinate: [position.x, position.z],
                });
            }
        }
        advance_cache_time(&mut app);
        app.update();
        let state = app.world().resource::<State>();
        assert!(state.entries[&OWNER].checked > before, "event {event}");
        assert!(state.shadows(OWNER));
    }
}
