//! Short native cultivation fixture with a real integrated Survival authority.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    CropKind, CropState, GameAction, GameConfig, GameMode, InputBinding, ItemKind, PlayerProfile,
};
use terra_network::client::ClientSession;
use terra_world::{
    edits::EditJournal,
    props::{PropId, WorldProp},
    streaming::ChunkStore,
};

fn root() -> PathBuf {
    std::env::var_os("TERRA_FARMING_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("terra-farming-{}", std::process::id()))
        })
}
fn support(index: usize, y: i64) -> [i64; 3] {
    // These adjacent species/coordinates used to cancel into the same crop ID.
    [4 + index as i64, y, 22]
}
fn growing(index: usize, y: i64) -> [i64; 3] {
    [6 + index as i64 * 2, y, 20]
}
fn ground(config: &terra_core::WorldConfig) -> Result<i64, String> {
    let terrain = terra_world::TerraGenerator::new(config.clone()).map_err(|e| e.to_string())?;
    Ok((terrain.surface_height(8.5, 24.5) - 0.5).floor() as i64)
}
fn plot(
    journal: &mut EditJournal,
    position: [i64; 3],
    crop: CropKind,
    stage: u8,
    id: u64,
    almost_ripe: bool,
) -> Result<(), String> {
    journal
        .record(position, terra_core::farming::FARMLAND)
        .map_err(|e| e.to_string())?;
    for dy in 1..=3 {
        journal
            .record([position[0], position[1] + dy, position[2]], 0)
            .map_err(|e| e.to_string())?;
    }
    let mut state = CropState::new(crop, id);
    state.moisture = 1.0;
    state.progress_seconds = if almost_ripe {
        crop.definition().growth_seconds - 2.0
    } else {
        crop.definition().growth_seconds * f32::from(stage) / 7.0
    };
    let prop = WorldProp {
        id: PropId(id),
        kind: crop.resource_prop().into(),
        position: [
            position[0] as f64 + 0.5,
            position[1] as f64 + 1.0,
            position[2] as f64 + 0.5,
        ],
        rotation: 0.0,
        scale: 1.0,
        variant: state.stage(),
        section: terra_voxel::global_to_section(position).0,
        removed: false,
    };
    journal.place_prop(prop).map_err(|e| e.to_string())?;
    journal.set_crop(position, state)?;
    Ok(())
}
pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Survival;
    config.server.natural_mob_limit = Some(0);
    config.world.biomes = false;
    config.world.landscape_revision = 0;
    config.world.trees = false;
    config.world.caves = false;
    config.world.rivers = false;
    config.world.water = false;
    config.world.base_height = 16.0;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = false;
    config.world.time.start_hour = 12.0;
    config.world.seasons.start_season = terra_core::Season::Spring;
    let path = root();
    if path.exists() {
        return Err("Farming fixture requires a fresh directory".into());
    }
    let profile = PlayerProfile::load_for_client(
        PlayerProfile::selected_path(&terra_core::menu_config_directory())
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let y = ground(&config.world)?;
    let mut journal = EditJournal::default();
    for crop in CropKind::ALL {
        journal
            .gameplay_mut()
            .inventories
            .entry(profile.uuid)
            .or_default()
            .insert(ItemKind::Crop(crop.seed()), 3);
    }
    for item in [
        ItemKind::Tool(terra_core::ToolKind::IronHoe),
        ItemKind::Tool(terra_core::ToolKind::IronWateringCan),
        ItemKind::Tool(terra_core::ToolKind::IronScythe),
        ItemKind::Prop(terra_core::ResourcePropKind::Scarecrow),
    ] {
        journal
            .gameplay_mut()
            .inventories
            .entry(profile.uuid)
            .or_default()
            .insert(item, 1);
    }
    // Isolated, saved plots. Nothing is added to normal world generation.
    let generator = terra_world::SectionGenerator::new(
        terra_world::TerraGenerator::new(config.world.clone()).map_err(|e| e.to_string())?,
    );
    for x in -1..=1 {
        for z in 0..=1 {
            let section = terra_voxel::SectionPos {
                x,
                y: y.div_euclid(16),
                z,
            };
            for prop in generator
                .build(section, 0, &Default::default(), false)
                .map_err(|e| e.to_string())?
                .props
            {
                journal
                    .remove_prop(section, prop.id)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    for (row, crop) in CropKind::ALL.into_iter().enumerate() {
        for stage in 0..8 {
            plot(
                &mut journal,
                [1 + i64::from(stage) * 2, y, 10 + row as i64 * 3],
                crop,
                stage,
                0xFA00_0000 + row as u64 * 16 + u64::from(stage),
                false,
            )?;
        }
        plot(
            &mut journal,
            growing(row, y),
            crop,
            6,
            0xFA00_0100 + row as u64,
            true,
        )?;
        let soil = support(row, y);
        journal.record(soil, 2).map_err(|e| e.to_string())?;
        for dy in 1..=3 {
            journal
                .record([soil[0], y + dy, soil[2]], 0)
                .map_err(|e| e.to_string())?;
        }
    }
    for z in [11, 14, 17] {
        for x in 0..=16 {
            journal
                .record([x, y, z], terra_voxel::MaterialId::FluidWater as u16)
                .map_err(|e| e.to_string())?;
        }
    }
    for (index, x) in [7, 9].into_iter().enumerate() {
        plot(
            &mut journal,
            [x, y, 20],
            CropKind::Wheat,
            7,
            0xFA00_0200 + index as u64,
            false,
        )?;
    }
    plot(
        &mut journal,
        [10, y, 22],
        CropKind::Wheat,
        4,
        0xFA00_0300,
        false,
    )?;
    for dy in 1..=3 {
        journal
            .record([11, y + dy, 22], 0)
            .map_err(|e| e.to_string())?;
        journal
            .record([13, y + dy, 24], 0)
            .map_err(|e| e.to_string())?;
    }
    journal.record([11, y, 22], 2).map_err(|e| e.to_string())?;
    journal
        .record([6, y, 24], terra_core::farming::FARMLAND)
        .map_err(|e| e.to_string())?;
    for dy in 1..=3 {
        journal
            .record([6, y + dy, 24], 0)
            .map_err(|e| e.to_string())?;
    }
    journal
        .record([13, y, 24], terra_voxel::MaterialId::FluidWater as u16)
        .map_err(|e| e.to_string())?;
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Fixture {
    start: Instant,
    since: Instant,
    stage: u8,
    crop: usize,
    y: i64,
    captures: u8,
    requested: u8,
    bad_capture: bool,
    edits: bevy::ecs::message::MessageCursor<terra_network::client::ConfirmedEdit>,
    tools: bevy::ecs::message::MessageCursor<terra_network::client::FarmToolRequest>,
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-farming") {
        return;
    }
    app.insert_resource(Fixture {
        start: Instant::now(),
        since: Instant::now(),
        stage: 0,
        crop: 0,
        y: i64::MIN,
        captures: 0,
        requested: 0,
        bad_capture: false,
        edits: Default::default(),
        tools: Default::default(),
    })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PostStartup, setup)
    .add_systems(PreUpdate, input.after(bevy::input::InputSystems))
    .add_systems(
        Update,
        showcase
            .after(terra_player::PlayerSet::Movement)
            .before(terra_player::PlayerSet::Targeting),
    )
    .add_systems(Last, run);
}
fn setup(world: &mut World) {
    let mut menu = world.resource_mut::<terra_ui::menu::MenuState>();
    menu.page = terra_ui::menu::MenuPage::Playing;
    menu.session_active = true;
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world
        .resource_mut::<terra_core::GameplayConfig>()
        .minimap
        .enabled = false;
    world.write_message(terra_player::CaptureRequest(true));
    let mut controls = world.resource_mut::<terra_core::ControlsConfig>();
    controls
        .rebind(GameAction::Place, InputBinding::Key(KeyCode::KeyU))
        .expect("fixture use");
    controls
        .rebind(GameAction::Break, InputBinding::Mouse(MouseButton::Left))
        .expect("fixture break");
}
fn crop_prop(world: &World, position: [i64; 3]) -> Option<WorldProp> {
    world
        .resource::<ChunkStore>()
        .loaded
        .get(&terra_voxel::global_to_section(position).0)?
        .props
        .iter()
        .find(|p| {
            !p.removed
                && p.kind.crop().is_some()
                && terra_world::props::placement::support(p) == position
        })
        .copied()
}
fn input(world: &mut World) {
    let (stage, index, y, elapsed) = {
        let f = world.resource::<Fixture>();
        (f.stage, f.crop, f.y, f.since.elapsed().as_secs_f32())
    };
    // Like the map fixture, own logical focus without stealing desktop focus.
    for mut window in world
        .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
        .iter_mut(world)
    {
        window.bypass_change_detection().focused = true;
    }
    world
        .resource_mut::<bevy::input::mouse::AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    let use_held = stage == 2 && (0.20..0.40).contains(&elapsed)
        || matches!(stage, 12 | 14 | 16 | 18 | 20) && elapsed > 0.20;
    let break_held = stage == 4;
    let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
    if use_held {
        keys.press(KeyCode::KeyU);
    } else {
        keys.release(KeyCode::KeyU);
    }
    {
        let mut mouse = world.resource_mut::<ButtonInput<MouseButton>>();
        if break_held {
            mouse.press(MouseButton::Left);
        } else {
            mouse.release(MouseButton::Left);
        }
    }
    if matches!(stage, 1..=4) {
        let position = if stage == 4 {
            growing(0, y)
        } else {
            support(index, y)
        };
        let point = Vec3::from_array(position.map(|v| v as f32))
            + if stage == 4 {
                Vec3::new(0.5, 1.65, 0.5)
            } else {
                Vec3::new(0.5, 1.001, 0.5)
            };
        for mut camera in world
            .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
            .iter_mut(world)
        {
            camera.look_at(point, Vec3::Y);
        }
    }
    let aim = match stage {
        11 | 12 | 19 | 20 => Some(Vec3::new(11.5, y as f32 + 1.001, 22.5)),
        13 | 14 => Some(Vec3::new(13.5, y as f32 + 0.85, 24.5)),
        15 | 16 => {
            Some(Vec3::from_array(support(0, y).map(|v| v as f32)) + Vec3::new(0.5, 1.001, 0.5))
        }
        17 | 18 => Some(Vec3::new(8.5, y as f32 + 1.55, 20.5)),
        22 | 24..=26 => Some(Vec3::new(10.52, y as f32 + 1.25, 22.52)),
        23 => Some(Vec3::new(6.5, y as f32 + 1.001, 24.5)),
        _ => None,
    };
    if let Some(point) = aim {
        for mut camera in world
            .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
            .iter_mut(world)
        {
            camera.look_at(point, Vec3::Y);
        }
    }
}
fn showcase(world: &mut World) {
    let (stage, y) = {
        let f = world.resource::<Fixture>();
        (f.stage, f.y)
    };
    if (7..=10).contains(&stage) || stage == 21 {
        if stage == 21 {
            world
                .resource_mut::<terra_core::GameplayConfig>()
                .minimap
                .enabled = true;
        }
        world
            .resource_mut::<terra_network::client::SelectedInventorySlot>()
            .0 = None;
        let (camera, target) = if stage == 21 {
            let prop = scarecrow(world, [11, y, 22]).expect("placed scarecrow");
            let rotation = Quat::from_rotation_y(prop.rotation);
            let root = Vec3::from_array(prop.position.map(|v| v as f32));
            (
                root + rotation * Vec3::new(0.9, 1.6, 3.0),
                root + Vec3::Y * 1.05,
            )
        } else if stage == 7 {
            (
                Vec3::new(19.0, y as f32 + 12.5, 27.0),
                Vec3::new(8.0, y as f32 + 1.3, 13.0),
            )
        } else {
            let row = usize::from(stage - 8).min(2);
            let z = 10.0 + row as f32 * 3.0;
            (
                Vec3::new(12.5, y as f32 + 3.5, z + 4.8),
                Vec3::new(13.0, y as f32 + 1.6, z + 0.5),
            )
        };
        for mut transform in world
            .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
            .iter_mut(world)
        {
            *transform = Transform::from_translation(camera).looking_at(target, Vec3::Y);
        }
    }
}
fn advance(f: &mut Fixture, stage: u8) {
    f.stage = stage;
    f.since = Instant::now();
    info!(stage, crop = f.crop, "Farming fixture stage");
}
fn capture(world: &mut World, label: &str) -> Result<(), String> {
    let path = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("captures"));
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.join(format!("farming-{label}.png"))))
        .observe(|event: On<ScreenshotCaptured>, mut f: ResMut<Fixture>| {
            f.bad_capture |= event.image.data.as_ref().is_none_or(|bytes| {
                bytes
                    .chunks_exact(4)
                    .filter(|p| p[0] > 20 || p[1] > 20 || p[2] > 20)
                    .count()
                    < 1000
            });
            f.captures += 1;
        });
    Ok(())
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut f: Mut<Fixture>| {
        if f.stage == 255 {
            return;
        }
        if let Err(error) = step(world, &mut f) {
            error!(%error, stage = f.stage, "Native farming fixture failed");
            f.stage = 255;
            world.write_message(bevy::app::AppExit::error());
        }
    });
}
fn step(world: &mut World, f: &mut Fixture) -> Result<(), String> {
    for event in f
        .tools
        .read(world.resource::<Messages<terra_network::client::FarmToolRequest>>())
    {
        info!(stage = f.stage, ?event, "Native farming tool input");
    }
    for event in f
        .edits
        .read(world.resource::<Messages<terra_network::client::ConfirmedEdit>>())
    {
        info!(stage = f.stage, ?event, "Native farming authority result");
    }
    if f.start.elapsed() > Duration::from_secs(100) || f.bad_capture {
        return Err(format!(
            "Farming fixture timed out/blank: stage={}, crop={}, y={}, target={:?}, active={:?}, render={:?}, ready={}, captured={}, blocked={}, water={:?}, growing={:?}, use={:?}",
            f.stage,
            f.crop,
            f.y,
            world.resource::<terra_player::TargetWorld>().0,
            world.resource::<terra_player::interaction::ActiveItem>().0,
            world.resource::<terra_render::RenderStatus>().error,
            world.resource::<terra_render::RenderStatus>().ready,
            world.resource::<terra_player::CameraInputState>().captured,
            world.resource::<terra_player::PlayerInputBlock>().0,
            world.resource::<ClientSession>().watering_reserve,
            (f.y != i64::MIN).then(|| std::array::from_fn::<_, 3, _>(|i| {
                crop_prop(world, growing(i, f.y)).map(|p| (p.kind, p.variant))
            })),
            world
                .resource::<terra_core::ControlsConfig>()
                .binding(GameAction::Place),
        ));
    }
    let session = world.resource::<ClientSession>();
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    // The authoritative flat fixture config arrives with the world handshake.
    if session.ready && f.y == i64::MIN {
        f.y = ground(&world.resource::<GameConfig>().world)?;
        info!(y = f.y, "Farming fixture authoritative ground");
    }
    let elapsed = f.since.elapsed().as_secs_f32();
    let kind = CropKind::ALL[f.crop.min(2)];
    match f.stage {
        0 if session.ready
            && world.resource::<terra_render::RenderStatus>().ready
            && CropKind::ALL.into_iter().enumerate().all(|(i, _)| {
                crop_prop(world, growing(i, f.y)).is_some_and(|p| p.variant == 7)
            }) =>
        {
            if CropKind::ALL
                .into_iter()
                .any(|crop| session.inventory.count(ItemKind::Crop(crop.seed())) != 3)
            {
                return Err("Seed inventory did not replicate".into());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(0));
            world.write_message(terra_player::CaptureRequest(true));
            advance(f, 1);
        }
        1 if elapsed > 0.5 && world.resource::<terra_player::CameraInputState>().captured => {
            advance(f, 2);
        }
        2 if elapsed > 0.5
            && crop_prop(world, support(f.crop, f.y))
                .is_some_and(|p| p.kind.crop() == Some(kind)) =>
        {
            if session.inventory.count(ItemKind::Crop(kind.seed())) != 2 {
                return Err("Sowing did not consume exactly one selected seed".into());
            }
            if world
                .resource::<ChunkStore>()
                .block_at(support(f.crop, f.y))
                != Some(terra_core::farming::FARMLAND)
            {
                return Err("Sowing did not prepare the soil".into());
            }
            f.crop += 1;
            if f.crop < 3 {
                world.write_message(terra_ui::InventoryAction::Hotbar(f.crop as u8));
                advance(f, 1);
            } else {
                world.write_message(terra_ui::InventoryAction::Hotbar(0));
                advance(f, 3);
            }
        }
        3 if elapsed > 0.3 => {
            advance(f, 4);
        }
        4 if crop_prop(world, growing(0, f.y)).is_none() => {
            info!(
                harvested = session.stats.props_harvested,
                "Crop removed through held Break; waiting for authority drops"
            );
            advance(f, 5);
        }
        5 if elapsed > 1.0 => {
            let seed_stock = session
                .inventory
                .count(ItemKind::Crop(CropKind::Wheat.seed()));
            let produce = session
                .inventory
                .count(ItemKind::Crop(CropKind::Wheat.produce()));
            let drops = &session.drops;
            let wheat_total: u16 = drops
                .iter()
                .filter(|d| d.stack.item == ItemKind::Crop(CropKind::Wheat.produce()))
                .map(|d| d.stack.count)
                .sum::<u16>()
                + produce;
            let seeds_total: u16 = drops
                .iter()
                .filter(|d| d.stack.item == ItemKind::Crop(CropKind::Wheat.seed()))
                .map(|d| d.stack.count)
                .sum::<u16>()
                + seed_stock;
            if wheat_total == 0 || seeds_total < 4 {
                return Err(format!(
                    "Ripe harvest lost produce/seeds: wheat={wheat_total}, seeds={seeds_total}"
                ));
            }
            world.write_message(terra_ui::InventoryAction::Toggle);
            advance(f, 6);
        }
        6 if elapsed > 1.0
            && world.resource::<terra_ui::InventoryView>().open
            && world
                .resource::<terra_render::RenderBlockPreviews>()
                .pending
                == 0 =>
        {
            let previews = world.resource::<terra_ui::ItemPreviewImages>();
            if terra_core::CropItem::ALL
                .into_iter()
                .any(|item| !previews.0.contains_key(&ItemKind::Crop(item)))
            {
                return Err("Missing farming item preview".into());
            }
            if f.requested == 0 {
                capture(world, "inventory")?;
                f.requested = 1;
            }
            if f.captures >= 1 {
                world.write_message(terra_ui::InventoryAction::Close);
                advance(f, 7);
            }
        }
        7..=10 if elapsed > 1.0 => {
            let count = f.stage - 5;
            if f.captures < count {
                if f.requested < count {
                    capture(
                        world,
                        match f.stage {
                            7 => "stages",
                            8 => "wheat",
                            9 => "corn",
                            _ => "lettuce",
                        },
                    )?;
                    f.requested = count;
                }
            } else if f.stage < 10 {
                advance(f, f.stage + 1);
            } else {
                world.write_message(terra_ui::InventoryAction::Hotbar(3));
                world.write_message(terra_player::CaptureRequest(true));
                advance(f, 11);
            }
        }
        11 | 13 | 15 | 17 | 19 if elapsed > 0.6 => advance(f, f.stage + 1),
        12 if elapsed > 0.5
            && world.resource::<ChunkStore>().block_at([11, f.y, 22])
                == Some(terra_core::farming::FARMLAND) =>
        {
            world.write_message(terra_ui::InventoryAction::Hotbar(4));
            advance(f, 13);
        }
        14 if elapsed > 0.5 && session.watering_reserve.0 > 0 => {
            info!(reserve = ?session.watering_reserve, "Watering can refilled from a real water cell");
            advance(f, 15);
        }
        16 if elapsed > 0.5 && session.watering_reserve.0 < session.watering_reserve.1 => {
            if session.watering_reserve.0 == 0 {
                return Err("Watering can did not retain unused water".into());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(5));
            advance(f, 17);
        }
        18 if elapsed > 0.5
            && [7, 8, 9]
                .into_iter()
                .all(|x| crop_prop(world, [x, f.y, 20]).is_none()) =>
        {
            if CropKind::ALL
                .into_iter()
                .enumerate()
                .any(|(i, _)| crop_prop(world, support(i, f.y)).is_none())
            {
                return Err("Scythe removed an unripe crop outside the harvest area".into());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(6));
            advance(f, 19);
        }
        20 if elapsed > 0.5 && scarecrow(world, [11, f.y, 22]).is_some() => advance(f, 21),
        21 if elapsed > 2.0 => {
            if f.requested < 6 {
                capture(world, "scarecrow")?;
                f.requested = 6;
            }
            if f.captures >= 6 {
                world.write_message(terra_ui::InventoryAction::Hotbar(3));
                advance(f, 22);
            }
        }
        22 if elapsed > 1.0 => {
            let view = world.resource::<terra_ui::TargetView>();
            let growth = view.stats.iter().find(|row| {
                row.id == "crop-growth" && row.bar && row.value > 0.0 && row.value < 100.0
            });
            let watering = view
                .stats
                .iter()
                .any(|row| row.id == "crop-moisture" && row.bar)
                && view
                    .stats
                    .iter()
                    .any(|row| row.id == "crop-water-needed" && !row.text.is_empty());
            if view.visible && growth.is_some() && watering {
                if f.requested < 7 {
                    capture(world, "growth-hud")?;
                    f.requested = 7;
                }
                if f.captures >= 7 {
                    advance(f, 23);
                }
            }
        }
        23 if elapsed > 1.0 => {
            let view = world.resource::<terra_ui::TargetView>();
            if view.visible
                && view.stats.iter().any(|row| {
                    row.id == "crop-seeds" && !row.bar && row.text == "Keine Samen erkannt"
                })
            {
                if f.requested < 8 {
                    capture(world, "empty-farmland-hud")?;
                    f.requested = 8;
                }
                if f.captures >= 8 {
                    let growth = world.resource::<terra_render::PropGrowthStatus>();
                    if growth.transitions < 3 {
                        return Err("Sown crops did not use the GPU growth transition".into());
                    }
                    info!(
                        transitions = growth.transitions,
                        "Native crop growth clock active for planted crops"
                    );
                    if std::env::var_os("TERRA_FARM_GROWTH_INSPECT").is_some() {
                        // Replay one visual stage after the real Survival cycle.
                        // The authority/save remains untouched by this gallery edit.
                        let support = [10, f.y, 22];
                        let mut prop =
                            crop_prop(world, support).ok_or("Missing growth inspection crop")?;
                        prop.variant = 5;
                        let mut delta = world.resource::<EditJournal>().section_delta(prop.section);
                        *delta
                            .placed_props
                            .iter_mut()
                            .find(|saved| saved.id == prop.id)
                            .ok_or("Missing visual crop delta")? = prop;
                        delta.prop_epoch = delta.prop_epoch.saturating_add(1);
                        world.resource_mut::<EditJournal>().import_delta(delta)?;
                        if let Some(section) = world
                            .resource_mut::<ChunkStore>()
                            .loaded
                            .get_mut(&prop.section)
                            && let Some(crop) =
                                section.props.iter_mut().find(|crop| crop.id == prop.id)
                        {
                            *crop = prop;
                        }
                        world.write_message(terra_world::streaming::SectionPublished {
                            position: prop.section,
                        });
                        advance(f, 24);
                    } else {
                        finish(world, f)?;
                    }
                }
            }
        }
        24..=26 => {
            let progress = world
                .resource::<terra_render::PropGrowthStatus>()
                .progress(PropId(0xFA00_0300));
            let ready = match f.stage {
                24 => progress.is_some_and(|p| p > 0.02 && p < 0.35),
                25 => progress.is_some_and(|p| (0.45..0.9).contains(&p)),
                _ => elapsed > 1.5 && progress.is_none_or(|p| p >= 0.999),
            };
            if ready {
                let count = f.stage - 15;
                if f.requested < count {
                    capture(
                        world,
                        match f.stage {
                            24 => "grow-start",
                            25 => "grow-middle",
                            _ => "grow-complete",
                        },
                    )?;
                    f.requested = count;
                }
                if f.captures >= count {
                    if f.stage == 26 {
                        finish(world, f)?;
                    } else {
                        advance(f, f.stage + 1);
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn scarecrow(world: &World, support: [i64; 3]) -> Option<WorldProp> {
    world
        .resource::<ChunkStore>()
        .loaded
        .get(&terra_voxel::global_to_section(support).0)
        .and_then(|section| {
            section
                .props
                .iter()
                .find(|prop| {
                    !prop.removed
                        && prop.kind == terra_world::props::PropKind::Scarecrow
                        && terra_world::props::placement::support(prop) == support
                })
                .copied()
        })
}

fn finish(world: &mut World, f: &mut Fixture) -> Result<(), String> {
    let id = world.resource::<GameConfig>().world.id;
    world
        .remove_resource::<terra_server::IntegratedServer>()
        .ok_or("Missing farming authority")?
        .finish()?;
    let loaded = terra_storage::load_edits(&root(), id)?;
    for i in 0..3 {
        let state = loaded
            .crop_at(support(i, f.y))
            .ok_or("Sown crop did not persist")?;
        if state.kind != CropKind::ALL[i] || !state.valid() {
            return Err("Saved crop state invalid".into());
        }
    }
    if loaded.crop_at(growing(0, f.y)).is_some()
        || [7, 8, 9]
            .into_iter()
            .any(|x| loaded.crop_at([x, f.y, 20]).is_some())
    {
        return Err("Harvested crop revived after saving".into());
    }
    if !loaded
        .section_delta(terra_voxel::global_to_section([11, f.y, 22]).0)
        .placed_props
        .iter()
        .any(|prop| prop.kind == terra_world::props::PropKind::Scarecrow && !prop.removed)
    {
        return Err("Placed scarecrow did not persist".into());
    }
    info!(
        captures = f.captures,
        crops = loaded.crops().count(),
        "Native farming cycle passed: sowing, growth, manual harvest, hoe, watering refill/pour, scythe AoE, scarecrow, growth HUD and save/reload"
    );
    f.stage = 255;
    world.write_message(bevy::app::AppExit::Success);
    Ok(())
}
