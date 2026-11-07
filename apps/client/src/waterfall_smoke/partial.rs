//! Real offline solver and rendered partial-block water, using the normal edit path.
use super::*;
use terra_core::plumbing;
use terra_world::{edits::BlockObjectState, streaming::commit_object_state, water::FluidCell};

const VIEWS: [&str; 9] = [
    "slab-stair-fence-basins",
    "waterlogged-partial-pool",
    "waterlogged-pipe-valve-fall",
    "all-pipe-wall-roof-surrounds",
    "partial-basins-close",
    "waterlogged-enclosed-pipes",
    "fence-inward-water-top",
    "waterlogged-wall-parts",
    "underwater-wall-pipes",
];
const WET: [(i64, u16, u8); 7] = [
    (20, plumbing::WOOD_PIPE, 4),
    (23, plumbing::STONE_PIPE, 0),
    (26, plumbing::IRON_PIPE, 1),
    (29, plumbing::GLASS_PIPE, 4),
    (32, plumbing::VALVE, 4),
    (35, 90, 0),
    (37, 74, 0),
];

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    floor: i64,
    built: bool,
    stage: usize,
    pending: bool,
    captures: usize,
    error: Option<String>,
}

pub(super) fn install(app: &mut App) {
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Inspection {
            start: Instant::now(),
            since: Instant::now(),
            floor: 64,
            built: false,
            stage: 0,
            pending: false,
            captures: 0,
            error: None,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .after(terra_world::streaming::StreamingSet::Dispatch),
        );
}

fn setup(world: &mut World) {
    let terrain = &world.resource::<WorldTerrain>().0;
    let top = [[8., 0.], [40., 0.], [8., 31.], [40., 31.], [24., 16.]]
        .into_iter()
        .map(|[x, z]| terrain.surface_height(x, z))
        .fold(32.0_f64, f64::max);
    let floor = ((top.ceil() as i64 + 47).div_euclid(16)) * 16;
    world.resource_mut::<Inspection>().floor = floor;
    *world.resource_mut::<WorldClock>() = WorldClock {
        phase: 0.5,
        paused: true,
        ..Default::default()
    };
    world
        .resource_mut::<WorldWeather>()
        .set_pattern(WeatherPattern::Clear);
    world.resource_mut::<ChunkFocus>().0 = [24., floor as f64 + 5., 16.];
}

fn view(floor: i64, stage: usize) -> Transform {
    let (target, offset) = match stage {
        0 => (Vec3::new(25., 1., 15.), Vec3::new(10., 13., 16.)),
        1 => (Vec3::new(27., 1.8, 5.), Vec3::new(8., 5., 9.)),
        2 => (Vec3::new(11.5, 3.5, 3.5), Vec3::new(5., 3., 8.)),
        3 => (Vec3::new(24.5, 2., 28.5), Vec3::new(11., 7., 13.)),
        4 => (Vec3::new(25.5, 1.25, 15.), Vec3::new(3., 6., 9.)),
        5 => (Vec3::new(24., 2., 23.), Vec3::new(8., 8., 11.)),
        6 => (Vec3::new(36.5, 1.5, 15.5), Vec3::new(0.01, 8., 0.01)),
        7 => (Vec3::new(28., 3.2, 2.5), Vec3::new(5., 1., 5.)),
        _ => (Vec3::new(27., 3.2, 1.8), Vec3::new(2., -0.8, 3.7)),
    };
    let target = target + Vec3::Y * floor as f32;
    Transform::from_translation(target + offset).looking_at(target, Vec3::Y)
}

fn pose(world: &mut World, position: [i64; 3], material: u16, facing: u8) -> Result<(), String> {
    super::edit(world, position, material)?;
    world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
        commit_object_state(
            &mut chunks,
            &mut world.resource_mut::<EditJournal>(),
            position,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        )
    })
}

fn fluid(world: &mut World, position: [i64; 3], amount: u8, source: bool) -> Result<(), String> {
    world
        .resource_mut::<EditJournal>()
        .set_fluid(
            position,
            FluidCell {
                amount,
                source,
                ..Default::default()
            },
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn build(world: &mut World, floor: i64) -> Result<(), String> {
    for x in 8..=40 {
        for z in 0..=31 {
            super::edit(world, [x, floor, z], 3)?;
        }
    }
    // Full sources must settle below the solid half of the slab rim.
    for x in 14..=17 {
        for z in 14..=17 {
            if x == 14 || x == 17 || z == 14 || z == 17 {
                pose(world, [x, floor + 1, z], 82, 0)?;
            } else {
                fluid(world, [x, floor + 1, z], 8, true)?;
            }
        }
    }
    // The exact source-connected stair inner-corner regression, shifted +10 X.
    for x in 24..=27 {
        for z in 13..=16 {
            if x == 24 || x == 27 || z == 13 || z == 16 {
                super::edit(world, [x, floor + 1, z], 3)?;
            }
        }
    }
    for z in [14, 15] {
        super::edit(world, [25, floor + 1, z], 27)?;
    }
    pose(world, [26, floor + 1, 14], 74, 1)?;
    pose(world, [26, floor + 1, 15], 74, 0)?;
    // A real standalone fence ring, with open terrain outside (no stone wrapper).
    for x in 35..=37 {
        for z in 14..=16 {
            if x == 35 || x == 37 || z == 14 || z == 16 {
                pose(world, [x, floor + 1, z], 90, 0)?;
            }
        }
    }
    super::edit(world, [36, floor + 1, 15], 27)?;
    // Fully wet neighbours must not leave a dry voxel shell around a model.
    for x in 18..=39 {
        for z in 1..=9 {
            for y in 1..=3 {
                let wall = x == 18 || x == 39 || z == 1 || z == 9;
                super::edit(world, [x, floor + y, z], if wall { 3 } else { 27 })?;
            }
        }
    }
    for (x, material, facing) in WET {
        let p = [x, floor + 3, 5];
        pose(world, p, material, facing)?;
    }
    for (x, material, facing) in [
        (21, plumbing::WOOD_PIPE, 4),
        (24, plumbing::GLASS_PIPE, 4),
        (27, 74, 1),
        (30, 82, 4),
        (33, plumbing::VALVE, 4),
    ] {
        pose(world, [x, floor + 3, 2], material, facing)?;
    }
    if super::edit(world, [30, floor + 2, 6], 40).is_ok() {
        return Err("Torch placement in existing water was accepted".into());
    }
    for x in 8..=10 {
        for z in 10..=12 {
            if x != 9 || z != 11 {
                super::edit(world, [x, floor + 1, z], 3)?;
            }
        }
    }
    let torch = [9, floor + 1, 11];
    pose(world, torch, 40, 0)?;
    fluid(world, torch, 8, true)?;
    // An actual falling sheet intersects both a round pipe and a valve.
    for x in 9..=13 {
        for y in 1..=6 {
            super::edit(world, [x, floor + y, 2], 3)?;
            if x == 9 || x == 13 {
                for z in 3..=5 {
                    super::edit(world, [x, floor + y, z], 3)?;
                }
            }
        }
    }
    for x in 10..=12 {
        super::edit(world, [x, floor + 6, 3], 27)?;
    }
    pose(world, [11, floor + 4, 3], plumbing::GLASS_PIPE, 4)?;
    pose(world, [11, floor + 2, 3], plumbing::VALVE, 4)?;
    // Wet pipes in tight stone shafts must retain water instead of gaining infill.
    for (x, material) in [
        (12, plumbing::WOOD_PIPE),
        (19, plumbing::STONE_PIPE),
        (26, plumbing::IRON_PIPE),
        (33, plumbing::GLASS_PIPE),
    ] {
        for xx in x - 1..=x + 1 {
            for z in 22..=24 {
                for y in 1..=3 {
                    if y == 3 && z == 24 {
                        continue;
                    }
                    super::edit(
                        world,
                        [xx, floor + y, z],
                        if xx == x && z == 23 {
                            if y <= 2 { 27 } else { 0 }
                        } else {
                            3
                        },
                    )?;
                }
            }
        }
        pose(world, [x, floor + 2, 23], material, 4)?;
        pose(world, [x, floor + 3, 23], material, 4)?;
    }
    // Same wall/roof enclosure convention for all ordinary pipe materials.
    for (x, material) in [
        (14, plumbing::WOOD_PIPE),
        (21, plumbing::STONE_PIPE),
        (28, plumbing::IRON_PIPE),
        (35, plumbing::GLASS_PIPE),
    ] {
        for xx in x - 1..=x + 1 {
            for z in 27..=29 {
                for y in 1..=2 {
                    super::edit(world, [xx, floor + y, z], 3)?;
                }
            }
        }
        for p in [[x, floor + 2, 28], [x, floor + 3, 28]] {
            pose(world, p, material, 4)?;
        }
        for p in [[x, floor + 1, 28], [x, floor + 1, 29], [x, floor + 1, 30]] {
            pose(world, p, material, 0)?;
        }
    }
    Ok(())
}

fn verify(world: &World, floor: i64) -> Result<(), String> {
    let chunks = world.resource::<ChunkStore>();
    let journal = world.resource::<EditJournal>();
    let terrain = &world.resource::<WorldTerrain>().0;
    let mut units = 0;
    for x in 15..=16 {
        for z in 15..=16 {
            let p = [x, floor + 1, z];
            let cell = chunks.fluid_cell_at(terrain, journal, p, 0);
            if cell.amount != 4 || !cell.source {
                return Err(format!("Finite slab basin changed at {p:?}: {cell:?}"));
            }
            units += u16::from(cell.amount);
        }
    }
    for (x, z) in [(13, 15), (18, 15), (15, 13), (15, 18)] {
        let p = [x, floor + 1, z];
        if chunks
            .fluid_cell_at(terrain, journal, p, chunks.block_at(p).unwrap_or(0))
            .amount
            > 0
        {
            return Err(format!(
                "Water leaked through a slab below its crest: {p:?}"
            ));
        }
    }
    for (p, offsets) in [
        ([26, floor + 1, 15], [0.15, 0.15]),
        ([35, floor + 1, 15], [0.85, 0.15]),
        ([36, floor + 1, 14], [0.15, 0.85]),
    ] {
        let point = [
            p[0] as f64 + offsets[0],
            p[1] as f64 + 0.8,
            p[2] as f64 + offsets[1],
        ];
        let column = chunks.water_column_at_point(terrain, journal, point);
        if column.is_none_or(|c| (c.level - (floor + 2) as f64).abs() > 1e-6) {
            return Err(format!(
                "Source-connected partial pocket lacks its shared level at {point:?}: {column:?}"
            ));
        }
        if chunks
            .fluid_cell_at(terrain, journal, p, chunks.block_at(p).unwrap_or(0))
            .source
        {
            return Err(format!("Ambient contact invented a source at {p:?}"));
        }
    }
    let post = [35.5, floor as f64 + 1.8, 15.5];
    if chunks
        .water_column_at_point(terrain, journal, post)
        .is_some()
    {
        return Err("Fence post itself became liquid".into());
    }
    for (x, z) in [(34, 15), (38, 15), (36, 13), (36, 17)] {
        let p = [x, floor + 1, z];
        if chunks.fluid_cell_at(terrain, journal, p, 0).amount != 0 {
            return Err(format!("Fence bank leaked at {p:?}"));
        }
    }
    for x in 35..=37 {
        for z in 14..=16 {
            for u in [0.2, 0.8] {
                for v in [0.2, 0.8] {
                    let point = [x as f64 + u, floor as f64 + 1.2, z as f64 + v];
                    let expected =
                        point[0] > 35.5 && point[0] < 37.5 && point[2] > 14.5 && point[2] < 16.5;
                    if chunks
                        .water_column_at_point(terrain, journal, point)
                        .is_some()
                        != expected
                    {
                        return Err(format!("Incorrect inward fence water at {point:?}"));
                    }
                }
            }
        }
    }
    let torch = [9, floor + 1, 11];
    if chunks.block_at(torch) != Some(0)
        || !journal
            .fluid(torch)
            .is_some_and(|cell| cell.source && cell.amount == 8)
        || journal
            .gameplay()
            .drops
            .iter()
            .filter(|drop| drop.stack.item == terra_core::ItemKind::Block(40))
            .count()
            != 1
    {
        return Err("Flooded torch was not removed once with its water preserved".into());
    }
    for (x, material, facing) in WET {
        let p = [x, floor + 3, 5];
        let cell = chunks.fluid_cell_at(terrain, journal, p, material);
        if chunks.block_at(p) != Some(material)
            || journal.object_state(p).is_none_or(|s| s.facing != facing)
            || cell.amount != 8
            || !cell.source
        {
            return Err(format!(
                "Waterlogged fixture source/pose changed: {p:?} {cell:?}"
            ));
        }
        let probe = [x as f64 + 0.15, floor as f64 + 3.85, 5.15];
        if chunks
            .water_column_at_point(terrain, journal, probe)
            .is_none_or(|c| (c.level - (floor + 4) as f64).abs() > 1e-6)
        {
            return Err(format!("Waterlogged free pocket is dry at {probe:?}"));
        }
    }
    for (x, material) in [
        (12, plumbing::WOOD_PIPE),
        (19, plumbing::STONE_PIPE),
        (26, plumbing::IRON_PIPE),
        (33, plumbing::GLASS_PIPE),
    ] {
        let p = [x, floor + 2, 23];
        let water = chunks.fluid_cell_at(terrain, journal, p, material);
        if water.amount != 8 || !water.source {
            return Err(format!("Pipe displaced its shaft water at {p:?}"));
        }
    }
    info!(
        bank_units = units,
        "Native partial-water basin levels, slab containment and fence post exclusion verified"
    );
    Ok(())
}

fn rendered(world: &World, floor: i64) -> bool {
    let chunks = world.resource::<ChunkStore>();
    let journal = world.resource::<EditJournal>();
    let status = world.resource::<RenderStatus>();
    [0, 1, 2].into_iter().all(|x| {
        [0, 1].into_iter().all(|z| {
            let owner = terra_voxel::SectionPos::new(x, floor.div_euclid(16), z);
            let section = chunks.loaded.get(&owner);
            status.section_ready(owner, section, None)
                && section.is_some_and(|s| {
                    s.mesh_epoch == journal.mesh_epoch(owner)
                        && s.water_epoch == journal.water_mesh_epoch(owner)
                })
        })
    }) && status.ready
        && world.resource::<WaterStatus>().ready
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        if state.stage == usize::MAX {
            return;
        }
        let view = view(state.floor, state.stage);
        for mut t in world
            .query_filtered::<&mut Transform, With<TerraCamera>>()
            .iter_mut(world)
        {
            *t = view;
        }
        for mut t in world
            .query_filtered::<&mut Transform, With<PlayerBody>>()
            .iter_mut(world)
        {
            t.translation = view.translation;
        }
        world.resource_mut::<ChunkFocus>().0 = [24., state.floor as f64 + 5., 16.];
        let result = (|| -> Result<bool, String> {
            if let Some(error) = &state.error {
                return Err(error.clone());
            }
            if state.start.elapsed() > Duration::from_secs(75) {
                return Err(format!(
                    "Partial-water timeout: stage={} captures={}",
                    state.stage, state.captures
                ));
            }
            if !world.resource::<WorldAccess>().authoritative() {
                return Err("Partial fixture requires authoritative offline simulation".into());
            }
            if state.pending {
                return Ok(false);
            }
            if !state.built {
                let chunks = world.resource::<ChunkStore>();
                if ![8, 24, 40].into_iter().all(|x| {
                    [0, 31].into_iter().all(|z| {
                        [state.floor, state.floor + 7]
                            .into_iter()
                            .all(|y| chunks.block_at([x, y, z]).is_some())
                    })
                }) || !world.resource::<RenderStatus>().ready
                {
                    return Ok(false);
                }
                build(world, state.floor)?;
                state.built = true;
                state.since = Instant::now();
                return Ok(false);
            }
            if state.stage == VIEWS.len() {
                return Ok(super::captures_finished(world));
            }
            if state.since.elapsed() < Duration::from_secs(if state.stage == 0 { 6 } else { 1 })
                || !rendered(world, state.floor)
            {
                return Ok(false);
            }
            verify(world, state.floor)?;
            if state.stage == 8 {
                let medium = world.resource::<terra_render::UnderwaterState>();
                if medium.depth < 0.5 || medium.blend < 0.95 {
                    return Err(format!(
                        "Underwater pipe inspection is not submerged: {medium:?}"
                    ));
                }
            }
            let folder = std::env::var_os("TERRA_CAPTURE_DIR")
                .map(PathBuf::from)
                .ok_or("Set TERRA_CAPTURE_DIR")?;
            std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            world
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    folder.join(format!("{}.png", VIEWS[state.stage])),
                ))
                .observe(
                    |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                        if event.image.data.as_ref().is_none_or(|bytes| {
                            bytes
                                .chunks_exact(4)
                                .filter(|p| p[..3].iter().any(|v| *v > 20))
                                .count()
                                < 1000
                        }) {
                            state.error = Some("Blank partial-water capture".into());
                        }
                        state.captures += 1;
                        state.stage += 1;
                        state.pending = false;
                        state.since = Instant::now();
                    },
                );
            state.pending = true;
            Ok(false)
        })();
        match result {
            Ok(true) => {
                info!(
                    captures = state.captures,
                    seconds = state.start.elapsed().as_secs_f64(),
                    "Native partial water and pipe surrounds passed"
                );
                world.write_message(AppExit::Success);
                state.stage = usize::MAX;
            }
            Err(error) => {
                error!(%error,stage=state.stage,"Native partial water and pipe surrounds failed");
                world.write_message(AppExit::error());
                state.stage = usize::MAX;
            }
            Ok(false) => {}
        }
    });
}
