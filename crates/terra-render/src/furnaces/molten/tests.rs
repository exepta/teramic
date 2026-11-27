use super::*;
use bevy::ecs::system::SystemState;
use std::time::Duration;
use terra_core::{GameConfig, block_properties::BlockPropertiesCatalog};
use terra_voxel::{BlockSection, MaterialId, global_to_section};
use terra_world::streaming::LoadedSection;

fn set(chunks: &mut ChunkStore, p: [i64; 3], raw: u16) {
    let (section, local) = global_to_section(p);
    chunks
        .loaded
        .entry(section)
        .or_insert_with(|| LoadedSection {
            data: BlockSection::empty(section).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        })
        .data
        .set_block(local, raw)
        .unwrap();
}

fn scene(chunks: ChunkStore) -> App {
    let mut app = App::new();
    app.insert_resource(chunks)
        .insert_resource(WorldAccess::Local)
        .init_resource::<Time>()
        .init_resource::<Time<Real>>()
        .init_resource::<GameConfig>()
        .init_resource::<VolcanicSources>()
        .add_systems(Update, discover);
    app.world_mut()
        .spawn((TerraCamera, Transform::from_xyz(2., 3., 2.)));
    app
}

#[test]
fn natural_and_poured_surfaces_are_exposed_and_invalidated_before_the_next_scan() {
    let mut chunks = ChunkStore::default();
    let natural = [-1, 3, -2];
    let poured = [13, 4, 3];
    let vent = [6, 15, 7];
    let sealed = [1, 4, 8];
    let buried = [13, 3, 3];
    let stone = MaterialId::Stone as u16;
    set(&mut chunks, natural, LAVA);
    set(&mut chunks, buried, LAVA);
    set(&mut chunks, vent, FUMAROLE);
    set(&mut chunks, [6, 16, 7], 0);
    set(&mut chunks, sealed, FUMAROLE);
    set(&mut chunks, [1, 5, 8], stone);
    assert!(
        lava::pour(
            &mut chunks,
            &mut EditJournal::default(),
            &BlockPropertiesCatalog::default(),
            poured
        )
        .unwrap()
    );

    let mut app = scene(chunks);
    app.update();
    let sources = app.world().resource::<VolcanicSources>();
    assert_eq!(sources.surfaces, [natural, poured]);
    assert_eq!(sources.fumaroles, [vent]);
    assert_eq!(sources.nearest_lava(), Some(Vec3::new(-0.5, 3.5, -1.5)));

    // No time has elapsed. Remove one source and roof the others, including
    // a roof across the vertical section boundary.
    let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
    set(&mut chunks, natural, 0);
    set(&mut chunks, [13, 5, 3], stone);
    set(&mut chunks, [6, 16, 7], stone);
    app.update();
    let sources = app.world().resource::<VolcanicSources>();
    assert!(sources.surfaces.is_empty() && sources.fumaroles.is_empty());
    assert_eq!(sources.nearest_lava(), None);
}

#[test]
fn dense_lake_keeps_one_stable_bubble_source_per_patch_and_caps_both_budgets() {
    let mut chunks = ChunkStore::default();
    for z in -42..42 {
        for x in -42..42 {
            let mat = if x % 8 == 0 && z % 8 == 0 {
                FUMAROLE
            } else {
                LAVA
            };
            set(&mut chunks, [x, 1, z], mat);
        }
    }
    let mut app = scene(chunks);
    app.update();
    let sources = app.world().resource::<VolcanicSources>();
    assert_eq!((sources.surfaces.len(), sources.fumaroles.len()), (48, 16));
    let patches: std::collections::HashSet<_> = sources
        .surfaces
        .iter()
        .map(|p| p.map(|v| v.div_euclid(6)))
        .collect();
    assert_eq!(patches.len(), 48);
    let before = sources.surfaces.clone();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(1));
    app.update();
    assert_eq!(app.world().resource::<VolcanicSources>().surfaces, before);
}

#[test]
fn teleport_rescans_immediately_and_marks_the_new_sound_timeline() {
    let mut chunks = ChunkStore::default();
    let distant = [101, 3, 1];
    set(&mut chunks, [1, 3, 1], LAVA);
    set(&mut chunks, distant, LAVA);
    let mut app = scene(chunks);
    app.update();
    let world = app.world_mut();
    world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .single_mut(world)
        .unwrap()
        .translation
        .x = 101.;
    world
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(80));
    app.update();
    let sources = app.world().resource::<VolcanicSources>();
    assert_eq!(sources.surfaces, [distant]);
    assert_eq!(sources.continuous_from, sources.seconds);
}

fn frame(
    chunks: &ChunkStore,
    sources: &mut VolcanicSources,
    before: f64,
    now: f64,
) -> (Geometry, Vec<VolcanicSound>) {
    let mut world = World::new();
    world.init_resource::<Messages<VolcanicSound>>();
    let mut params = SystemState::<MessageWriter<VolcanicSound>>::new(&mut world);
    let mut geometry = Geometry::default();
    sources.seconds = now;
    sources.append(
        &mut geometry,
        chunks,
        &EditJournal::default(),
        before,
        &mut params.get_mut(&mut world).unwrap(),
    );
    (
        geometry,
        world
            .resource_mut::<Messages<VolcanicSound>>()
            .drain()
            .collect(),
    )
}

#[test]
fn bubble_swells_then_bursts_once_with_sparks_and_no_catch_up_audio() {
    let mut chunks = ChunkStore::default();
    let cell = [-4, 1, -1];
    set(&mut chunks, cell, LAVA);
    let mut sources = VolcanicSources {
        surfaces: vec![cell],
        ..default()
    };
    let key = seed(cell);
    let t = (100..2600)
        .map(|i| f64::from(i) * 0.01)
        .find(|&t| (0.30..0.32).contains(&bubble_age(t, key)))
        .unwrap();
    let (swelling, sounds) = frame(&chunks, &mut sources, t - 0.12, t - 0.09);
    assert_eq!(swelling.positions.len(), 2 * 24);
    assert!(sounds.is_empty());
    let (sparks, sounds) = frame(&chunks, &mut sources, t - 0.09, t);
    assert_eq!(sparks.positions.len(), 5 * 24);
    assert!(matches!(sounds.as_slice(), [VolcanicSound::Bubble(_)]));
    for (before, now) in [(t, t + 0.02), (t, t), (t + 0.06, t), (t - 2., t)] {
        assert!(frame(&chunks, &mut sources, before, now).1.is_empty());
    }
    sources.continuous_from = t;
    assert!(frame(&chunks, &mut sources, t - 0.09, t).1.is_empty());
    set(
        &mut chunks,
        [cell[0], cell[1] + 1, cell[2]],
        MaterialId::Stone as u16,
    );
    let (hidden, sounds) = frame(&chunks, &mut sources, t - 0.09, t);
    assert!(hidden.positions.is_empty() && sounds.is_empty());
}

#[test]
fn fumarole_hisses_at_plume_onset_once_and_only_while_the_vent_is_exposed() {
    let mut chunks = ChunkStore::default();
    let vent = [2, 1, 3];
    set(&mut chunks, vent, FUMAROLE);
    let mut sources = VolcanicSources {
        fumaroles: vec![vent],
        ..default()
    };
    let t = (100..1000)
        .map(|i| f64::from(i) * 0.01)
        .find(|&t| lava::plume_height(vent, t - 0.05) < 1.7 && lava::plume_height(vent, t) >= 1.7)
        .unwrap();
    assert!(matches!(
        frame(&chunks, &mut sources, t - 0.05, t).1.as_slice(),
        [VolcanicSound::Steam(_)]
    ));
    for (a, b) in [(t, t + 0.08), (t, t), (t - 0.80, t)] {
        assert!(frame(&chunks, &mut sources, a, b).1.is_empty());
    }
    let roof = [vent[0], vent[1] + 1, vent[2]];
    set(&mut chunks, roof, MaterialId::Stone as u16);
    assert!(frame(&chunks, &mut sources, t - 0.05, t).1.is_empty());
    set(&mut chunks, roof, 0);
    set(&mut chunks, vent, 0);
    assert!(frame(&chunks, &mut sources, t - 0.05, t).1.is_empty());
}

#[test]
fn geyser_recharges_between_synchronized_bursts_and_a_roof_stops_its_jet() {
    let cell = [5, 2, -2];
    let mut chunks = ChunkStore::default();
    set(&mut chunks, cell, GEYSER);
    set(&mut chunks, [5, 33, -2], 0);
    set(&mut chunks, [5, 17, -2], 0);
    let mut app = scene(chunks);
    app.update();
    let sources = app.world().resource::<VolcanicSources>();
    assert_eq!(sources.geysers, [cell]);
    let mut sources = VolcanicSources {
        geysers: vec![cell],
        ..default()
    };
    let chunks = app.world_mut().resource_mut::<ChunkStore>().into_inner();
    let t = (10..1800)
        .map(|i| f64::from(i) * 0.01)
        .find(|t| geyser_surge(*t - 0.05, cell) < 0.2 && geyser_surge(*t, cell) >= 0.2)
        .unwrap();
    let sounds = frame(chunks, &mut sources, t - 0.05, t).1;
    assert!(matches!(sounds.as_slice(), [VolcanicSound::Steam(_)]));
    sources.seconds = t + 0.8;
    let mut spray = Geometry::default();
    sources.append_geysers(
        &mut spray,
        chunks,
        &EditJournal::default(),
        Vec3::X,
        Vec3::Y,
    );
    assert!(spray.positions.iter().any(|p| p[1] > 23.));
    sources.seconds = t + 6.2;
    let mut idle = Geometry::default();
    sources.append_geysers(&mut idle, chunks, &EditJournal::default(), Vec3::X, Vec3::Y);
    assert!(idle.positions.iter().all(|p| p[1] < 4.));
    set(chunks, [5, 3, -2], MaterialId::Stone as u16);
    let mut covered = Geometry::default();
    sources.append_geysers(
        &mut covered,
        chunks,
        &EditJournal::default(),
        Vec3::X,
        Vec3::Y,
    );
    assert!(covered.positions.is_empty());
}
