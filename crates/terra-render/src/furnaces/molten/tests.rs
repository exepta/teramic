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
        .init_resource::<EditJournal>()
        .insert_resource(WorldTerrain(
            terra_world::TerraGenerator::new(terra_core::WorldConfig {
                sea_level: -30.0,
                ..Default::default()
            })
            .unwrap(),
        ))
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
    let sources = VolcanicSources {
        fumaroles: vec![vent],
        ..default()
    };
    let t = (100..1000)
        .map(|i| f64::from(i) * 0.01)
        .find(|&t| {
            lava::plume_height(vent, t - 0.05) < FUMAROLE_HISS_HEIGHT
                && lava::plume_height(vent, t) >= FUMAROLE_HISS_HEIGHT
        })
        .unwrap();
    let steam_cues = |chunks: &ChunkStore, before, now| {
        let mut cues = Vec::new();
        sources.append_fumaroles(
            &mut Geometry::default(),
            (before, now),
            Vec2::ZERO,
            (Vec3::X, Vec3::Y),
            |p| chunks.block_at(p),
            |origin| cues.push(VolcanicSound::Steam(origin)),
        );
        cues
    };
    assert!(matches!(
        steam_cues(&chunks, Some(t - 0.05), t).as_slice(),
        [VolcanicSound::Steam(origin)] if origin.distance(Vec3::new(2.5, 2.008, 3.5)) < 1e-5
    ));
    for (a, b) in [(t, t + 0.08), (t, t), (t - 0.80, t), (t + 3., t + 3.08)] {
        assert!(steam_cues(&chunks, Some(a), b).is_empty());
    }
    assert!(steam_cues(&chunks, None, t).is_empty());
    let roof = [vent[0], vent[1] + 1, vent[2]];
    set(&mut chunks, roof, MaterialId::Stone as u16);
    assert!(steam_cues(&chunks, Some(t - 0.05), t).is_empty());
    set(&mut chunks, roof, 0);
    set(&mut chunks, vent, 0);
    assert!(steam_cues(&chunks, Some(t - 0.05), t).is_empty());
}

#[test]
fn fumaroles_are_clear_between_bursts_and_grow_vertically_from_their_mouth() {
    let vent = [5, 2, 7];
    let mut chunks = ChunkStore::default();
    set(&mut chunks, vent, FUMAROLE);
    let mut sources = VolcanicSources {
        fumaroles: vec![vent],
        ..default()
    };
    sources.seconds = (0..100)
        .map(|i| f64::from(i) * 0.1)
        .find(|&t| lava::plume_height(vent, t) > 3.35)
        .unwrap();
    let render = |sources: &VolcanicSources, chunks: &ChunkStore, wind| {
        let mut mesh = Geometry::default();
        sources.append_fumaroles(
            &mut mesh,
            (None, sources.seconds),
            wind,
            (Vec3::X, Vec3::Y),
            |p| chunks.block_at(p),
            |_| {},
        );
        mesh.positions
    };
    let calm = render(&sources, &chunks, Vec2::ZERO);
    let windy = render(&sources, &chunks, Vec2::X * 6.);
    assert_eq!(calm.len(), 19 * 4);
    assert_eq!(&calm[..4], &windy[..4]);
    let stem = calm[..4].iter().map(|&p| Vec3::from_array(p)).sum::<Vec3>() * 0.25;
    assert!(stem.with_y(3.).distance(Vec3::new(5.5, 3., 7.5)) < 1e-5);
    assert!((calm[0][1] - 3.008).abs() < 1e-5);
    assert!(calm.iter().any(|p| p[1] > 7.));
    let width = |quad: &[[f32; 3]]| (quad[1][0] - quad[0][0]).abs();
    let (minimum, maximum) = calm
        .chunks_exact(4)
        .map(width)
        .fold((f32::MAX, 0_f32), |(lo, hi), w| (lo.min(w), hi.max(w)));
    assert!(maximum > minimum * 4.);
    assert!(windy.iter().zip(&calm).any(|(w, c)| w[0] - c[0] > 0.9));

    let stone = MaterialId::Stone as u16;
    set(&mut chunks, [5, 3, 7], stone);
    assert!(render(&sources, &chunks, Vec2::ZERO).is_empty());
    set(&mut chunks, [4, 2, 7], stone);
    assert!(render(&sources, &chunks, Vec2::X).is_empty());

    set(&mut chunks, [5, 3, 7], 0);
    sources.seconds += 3.;
    assert_eq!(lava::plume_height(vent, sources.seconds), 0.);
    assert!(render(&sources, &chunks, Vec2::ZERO).is_empty());
}

#[test]
fn geysers_are_discovered_and_drawn_only_with_water_above_the_mouth() {
    use geyser_render::{GeyserWorld, append_vent};
    use terra_world::water::FluidCell;
    let cell = [5, 2, -2];
    let mouth = [5, 3, -2];
    let mut chunks = ChunkStore::default();
    set(&mut chunks, cell, GEYSER);
    set(&mut chunks, [5, 33, -2], 0);
    set(&mut chunks, [5, 17, -2], 0);
    let mut app = scene(chunks);
    app.update();
    assert!(app.world().resource::<VolcanicSources>().geysers.is_empty());
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            mouth,
            FluidCell {
                amount: 8,
                source: true,
                ..default()
            },
        )
        .unwrap();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(700));
    app.update();
    assert_eq!(app.world().resource::<VolcanicSources>().geysers, [cell]);
    let terrain = &app.world().resource::<WorldTerrain>().0;
    let chunks = app.world().resource::<ChunkStore>();
    let journal = app.world().resource::<EditJournal>();
    let world = GeyserWorld {
        chunks,
        journal,
        terrain,
    };
    let t = (10..2200)
        .map(|i| f64::from(i) * 0.01)
        .find(|&t| (0.85..0.95).contains(&geyser::phase(t, cell)))
        .unwrap();
    let draw = |world: &GeyserWorld, time| {
        let mut geometry = Geometry::default();
        append_vent(&mut geometry, world, cell, time, (Vec3::X, Vec3::Y));
        geometry.positions
    };
    let burst = draw(&world, t);
    assert!(burst.iter().any(|p| p[1] > 23.));
    assert!(burst.iter().any(|p| p[1] > 3.0 && p[1] < 4.0));
    let next_frame = draw(&world, t + 1.0 / 120.0);
    assert_ne!(
        burst, next_frame,
        "the same authority tick can draw different render frames"
    );
    assert!(draw(&world, t + 6.2).is_empty());
    let warning_time = (10..2200)
        .map(|i| f64::from(i) * 0.01)
        .find(|&t| geyser::boil_progress(t, cell).is_some_and(|p| (0.5..0.52).contains(&p)))
        .unwrap();
    let warning = draw(&world, warning_time);
    assert!(warning.iter().any(|p| (p[1] - 4.04).abs() < 0.001));
    assert!(warning.iter().all(|p| p[1] < 4.7));
    assert_ne!(warning, draw(&world, warning_time + 1.0 / 120.0));

    let mut lowered = journal.clone();
    lowered
        .set_fluid(
            mouth,
            FluidCell {
                amount: 4,
                source: true,
                ..default()
            },
        )
        .unwrap();
    let lowered_world = GeyserWorld {
        journal: &lowered,
        ..world
    };
    let lowered_warning = draw(&lowered_world, warning_time);
    assert!(lowered_warning.iter().any(|p| (p[1] - 3.54).abs() < 0.001));
    assert_eq!(warning.len(), lowered_warning.len());
    let mut dry = journal.clone();
    dry.set_fluid(mouth, FluidCell::default()).unwrap();
    let dry_world = GeyserWorld {
        journal: &dry,
        ..world
    };
    assert!(draw(&dry_world, t).is_empty());
    assert!(draw(&dry_world, warning_time).is_empty());

    let mut roof = journal.clone();
    roof.record([mouth[0], mouth[1] + 1, mouth[2]], MaterialId::Stone as u16)
        .unwrap();
    assert!(
        draw(
            &GeyserWorld {
                journal: &roof,
                ..world
            },
            warning_time
        )
        .is_empty()
    );
}
