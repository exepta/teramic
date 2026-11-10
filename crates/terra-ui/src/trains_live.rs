//! Opt-in native DOM/picking regression, with controlled authoritative inventory acknowledgements.
use super::*;
use bevy::{
    camera::RenderTarget,
    picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput},
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PrimaryWindow, WindowRef},
};
use terra_core::{
    Inventory, ItemKind, MineralItem, PlayerUuid,
    trains::{TrainPart, Vehicle},
};
use terra_network::{
    client::TrainRequest,
    protocol::trains::{TrainCargoSnapshot, TrainResult},
};
#[path = "trains_yard_live.rs"]
mod yard;
#[derive(Resource, Default)]
struct Probe {
    initialized: bool,
    workshop: bool,
    model: bool,
    failed: bool,
    frames: u32,
    transactions: u8,
    cursor: MessageCursor<TrainRequest>,
    location: Option<Location>,
}
pub(super) fn install(app: &mut App) {
    yard::install(app);
    if std::env::var("TERRA_TRAIN_INVENTORY_LIVE").as_deref() != Ok("1")
        && std::env::var("TERRA_TRAIN_WORKSHOP_LIVE").as_deref() != Ok("1")
        && std::env::var("TERRA_TRAIN_MODEL_LIVE").as_deref() != Ok("1")
    {
        return;
    }
    app.init_resource::<Probe>()
        .add_systems(Update, run.after(super::update));
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut probe: Mut<Probe>| {
        if !probe.initialized {
            if !world.get_resource::<crate::menu::MenuState>().is_some_and(|menu| menu.page == crate::menu::MenuPage::Main) { return; }
            if world.get_resource::<crate::menu::StartupProgress>().is_some_and(|progress| progress.active) { return; }
            probe.model = std::env::var("TERRA_TRAIN_MODEL_LIVE").as_deref() == Ok("1");
            probe.workshop = probe.model || std::env::var("TERRA_TRAIN_WORKSHOP_LIVE").as_deref() == Ok("1");
            let owner = PlayerUuid::from_u128(1);
            let guide = if probe.workshop { terra_core::train_blueprint::building_template_with_width([0,2,2],0,VehicleKind::Locomotive,8,3) } else { terra_core::train_blueprint::template([0,2,2],0,VehicleKind::Locomotive) };
            let parts = guide.into_iter().map(|cell| TrainPart {offset: cell.cell.map(|v| v as i16), material: cell.material, facing: cell.facing, wood: Default::default()}).collect::<Vec<_>>();
            let engine = parts.iter().position(|part| part.material == 218).unwrap() as u16;
            let car = Vehicle { name: String::new(), id: 1, owner, kind: VehicleKind::Locomotive, anchor: [0;3], parts, cargo: [(engine, Inventory::default())].into(), blocks: Default::default(), payloads: Default::default(), couplings: [None;2], revision: 7, motion: Default::default(), burn_seconds: 0.0, steam: Default::default() };
            assert!(car.valid());
            world.resource_mut::<crate::menu::MenuState>().page = crate::menu::MenuPage::Playing;
            let identity = {
                let mut session = world.resource_mut::<ClientSession>();
                session.ready = true; session.uuid = Some(owner); session.inventory = Inventory::default();
                session.inventory.creative_set(8, ItemStack {item: ItemKind::Mineral(MineralItem::Coal), count:128});
                session.trains.vehicles = vec![car.view()];
                session.train_cargo = Some(TrainCargoSnapshot {vehicle:1, part:engine, revision:7, inventory:Inventory::default()});
                (session.connection, session.uuid)
            };
            let mut state = world.resource_mut::<TrainUiState>(); state.open=true; state.cargo=true; state.selected=Some(1); state.identity=Some(identity);
            if probe.workshop { state.cargo=false; state.selected=None; state.station=Some([0;3]); state.name="TERAMIC EXPRESS".into(); }
            if probe.model { state.open=false; state.cargo=false; }
            if probe.model {
                // The normal vehicle renderer requires loaded, retired source
                // sections. Supply explicit empty visual-fixture sections; this
                // does not generate or modify a user's saved world.
                let owners: std::collections::BTreeSet<_> = world.resource::<ClientSession>().trains.vehicles[0].parts.iter().map(|part| terra_voxel::global_to_section(part.offset.map(i64::from)).0).collect();
                for owner in owners {
                    world.resource_mut::<terra_world::streaming::ChunkStore>().loaded.insert(owner, terra_world::streaming::LoadedSection {
                        data: terra_voxel::BlockSection::empty(owner).unwrap(),
                        mesh: Some(Default::default()), mesh_epoch: 0, water_epoch: 0, water: None, props: Vec::new(),
                    });
                }
            }
            world.write_message(terra_player::CaptureRequest(false));
            probe.initialized=true;
            return;
        }
        if probe.model { model(world, &mut probe); return; }
        if probe.workshop { workshop(world, &mut probe); return; }
        if world.query::<&tilt_ui::ElementId>().iter(world).filter(|id| id.0 == "train-slot").count() != 45 { return; }
        probe.frames += 1;
        if probe.frames > 900 { world.write_message(bevy::app::AppExit::error()); return; }
        let requests = probe.cursor.read(world.resource::<Messages<TrainRequest>>()).cloned().collect::<Vec<_>>();
        for request in requests {
            if let TrainAction::Cargo {gesture, revision, ..} = request.0.action {
                probe.transactions += 1;
                let mut session = world.resource_mut::<ClientSession>();
                let mut cargo = session.train_cargo.clone().unwrap();
                let mut chests = [cargo.inventory];
                assert!(terra_core::containers::apply_gesture_reserved(&mut session.inventory, &mut chests, gesture.clone(), [0;terra_core::INVENTORY_SLOTS]));
                cargo.inventory=chests[0].clone(); cargo.revision=revision+1;
                info!(?gesture, player_coal=session.inventory.count(ItemKind::Mineral(MineralItem::Coal)), engine_coal=cargo.inventory.count(ItemKind::Mineral(MineralItem::Coal)), "LIVE TRAIN native pointer transaction");
                session.train_cargo=Some(cargo.clone());
                session.last_train_result=Some(TrainResult {request:request.0.request, outcome:TrainOutcome::Cargo(cargo)});
            }
        }
        let window = world.query_filtered::<Entity, With<PrimaryWindow>>().single(world).unwrap();
        if matches!(probe.frames, 120 | 240 | 480 | 600) {
            let cargo = matches!(probe.frames, 240 | 600);
            let targets = world.query::<(Entity, &tilt_ui::ElementClasses)>().iter(world).filter_map(|(entity, classes)| (classes.classes.iter().any(|class| class == "train-slot") && world.get::<bevy::ui::InteractionDisabled>(entity).is_none()).then_some(entity)).collect::<Vec<_>>();
            assert_eq!(targets.len(), 1, "one occupied source must be clickable");
            let button = targets[0];
            let target = world.get::<Children>(button).and_then(|children| children.iter().find(|child| world.get::<bevy::ui::widget::ImageNode>(*child).is_some())).unwrap_or(button);
            let geometry = world.get::<UiGlobalTransform>(target).unwrap();
            let position = geometry.translation / world.get::<Window>(window).unwrap().resolution.scale_factor();
            info!(?position, size=?world.get::<ComputedNode>(target).unwrap().size(), "LIVE TRAIN actual materialized slot");
            let location = Location {target: RenderTarget::Window(WindowRef::Entity(window)).normalize(Some(window)).unwrap(), position};
            probe.location=Some(location);
            let folder = std::path::PathBuf::from(std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "target/live217".into())); std::fs::create_dir_all(&folder).unwrap();
            world.spawn(Screenshot::primary_window()).observe(save_to_disk(folder.join(if probe.frames < 360 { if cargo {"engine-filled.png"} else {"engine-before.png"} } else if cargo {"storage-filled.png"} else {"storage-before.png"})));
        }
        if let Some(location) = probe.location.clone() {
            world.get_mut::<Window>(window).unwrap().set_cursor_position(Some(location.position));
            world.write_message(PointerInput::new(PointerId::Mouse, location.clone(), PointerAction::Move {delta:Vec2::ZERO}));
            if matches!(probe.frames, 122 | 242 | 482 | 602) { world.write_message(PointerInput::new(PointerId::Mouse, location.clone(), PointerAction::Press(PointerButton::Primary))); }
            if matches!(probe.frames, 124 | 244 | 484 | 604) { world.write_message(PointerInput::new(PointerId::Mouse, location, PointerAction::Release(PointerButton::Primary))); }
        }
        if probe.frames == 360 {
            let session=world.resource::<ClientSession>();
            assert_eq!(session.inventory.count(ItemKind::Mineral(MineralItem::Coal)),128);
            assert_eq!(session.train_cargo.as_ref().unwrap().inventory.count(ItemKind::Mineral(MineralItem::Coal)),0);
            assert_eq!(probe.transactions,2,"both real clicks must reach the action handler");
            info!("LIVE TRAIN PASS: actual DOM + native picking, transfer and return128coal");
            let mut session = world.resource_mut::<ClientSession>();
            session.trains.vehicles[0].parts[12].material = 216;
            session.inventory = Inventory::default();
            session.inventory.creative_set(9, ItemStack {item: ItemKind::Block(3), count: 7});
        }
        if probe.frames == 720 {
            let session = world.resource::<ClientSession>();
            assert_eq!(probe.transactions, 4);
            assert_eq!(session.inventory.count(ItemKind::Block(3)), 7);
            assert_eq!(session.train_cargo.as_ref().unwrap().inventory.count(ItemKind::Block(3)), 0);
            info!("LIVE TRAIN PASS: storage transfer and return7stone; four native clicks confirmed");
            world.write_message(bevy::app::AppExit::Success);
        }
    });
}

fn workshop(world: &mut World, probe: &mut Probe) {
    if probe.failed {
        return;
    }
    probe.frames += 1;
    if probe.frames == 1 {
        // Deliberately occupy the miniature and environment layers at the same
        // coordinates as the train studio. Their geometry must never leak into it.
        let mesh = world
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(24.0, 24.0, 24.0));
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.0, 1.0),
                unlit: true,
                cull_mode: None,
                ..default()
            });
        for layer in [terra_core::miniature::WORLD_LAYER, 26] {
            world.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(0.0, 4.0, 3.0),
                bevy::camera::visibility::RenderLayers::layer(layer),
            ));
        }
    }
    if probe.frames == 100 {
        let studio = world
            .query::<(&Camera, &bevy::camera::visibility::RenderLayers)>()
            .iter(world)
            .find_map(|(camera, layers)| (camera.order == -13).then_some(layers.clone()))
            .expect("train studio camera is ready");
        for layer in [
            0,
            26,
            terra_core::miniature::WORLD_LAYER,
            terra_core::miniature::CREATURE_LAYER,
        ] {
            assert!(!studio.intersects(&bevy::camera::visibility::RenderLayers::layer(layer)));
        }
        info!("Native train preview isolation passed with overlapping magenta terrain sentinels");
    }
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .unwrap();
    if probe.frames == 60 {
        world
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_physical_resolution(1920, 1080);
    }
    if probe.frames == 180 {
        world
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_physical_resolution(1280, 720);
    }
    if probe.frames == 300 {
        let part = world.resource::<ClientSession>().trains.vehicles[0]
            .parts
            .iter()
            .position(|part| part.material == terra_core::train_parts::CAB)
            .unwrap() as u16;
        let owner = world.resource::<ClientSession>().uuid.unwrap();
        world.resource_mut::<ClientSession>().train_seat = Some(terra_core::trains::SeatBinding {
            player: owner,
            vehicle: 1,
            part,
        });
        world.resource_mut::<TrainUiState>().selected = Some(1);
    }
    if probe.frames == 360 {
        world
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_physical_resolution(1920, 1080);
    }
    if probe.frames == 480 {
        world
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_physical_resolution(1280, 720);
    }
    if probe.frames == 400 {
        let position = world
            .query::<(&tilt_ui::ElementId, &UiGlobalTransform)>()
            .iter(world)
            .find_map(|(id, transform)| (id.0 == "destroy").then_some(transform.translation))
            .expect("destroy button is materialized");
        let position = position
            / world
                .get::<Window>(window)
                .unwrap()
                .resolution
                .scale_factor();
        world
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(position));
        probe.location = Some(Location {
            target: RenderTarget::Window(WindowRef::Entity(window))
                .normalize(Some(window))
                .unwrap(),
            position,
        });
    }
    if let Some(location) = probe.location.clone()
        && (400..=404).contains(&probe.frames)
    {
        world.write_message(PointerInput::new(
            PointerId::Mouse,
            location.clone(),
            PointerAction::Move { delta: Vec2::ZERO },
        ));
        if probe.frames == 402 {
            world.write_message(PointerInput::new(
                PointerId::Mouse,
                location.clone(),
                PointerAction::Press(PointerButton::Primary),
            ));
        }
        if probe.frames == 404 {
            world.write_message(PointerInput::new(
                PointerId::Mouse,
                location,
                PointerAction::Release(PointerButton::Primary),
            ));
        }
    }
    if matches!(probe.frames, 420 | 540) {
        assert!(
            world.resource::<TrainView>().confirm_destroy,
            "native button click must reveal destruction confirmation"
        );
    }
    if matches!(probe.frames, 120 | 240 | 420 | 540) {
        let folder = std::path::PathBuf::from(
            std::env::var("TERRA_CAPTURE_DIR")
                .unwrap_or_else(|_| "target/live220/screenshots".into()),
        );
        std::fs::create_dir_all(&folder).unwrap();
        let label = match probe.frames {
            120 => "train-workshop-1080.png",
            240 => "train-workshop-720.png",
            420 => "train-driving-1080.png",
            _ => "train-driving-720.png",
        };
        let primary = if world.resource::<TrainUiState>().selected.is_some() {
            "drive-start"
        } else {
            "blueprint-auto"
        };
        let auto_build = world
            .query::<&tilt_ui::ElementId>()
            .iter(world)
            .any(|id| id.0 == primary);
        let name = world
            .query::<&tilt_ui::ElementId>()
            .iter(world)
            .any(|id| id.0 == "train-name");
        let bounds = workshop_bounds(world, window);
        if !auto_build || !name || bounds.is_err() {
            error!(auto_build, name, ?bounds, page=?world.resource::<crate::menu::MenuState>().page, open=world.resource::<TrainUiState>().open, ready=world.resource::<ClientSession>().ready, "Native train workshop failed: required controls missing or clipped");
            probe.failed = true;
            world.write_message(bevy::app::AppExit::error());
            return;
        }
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(folder.join(label)));
        info!(
            label,
            "Native train workshop: auto-build button and editable name materialized"
        );
    }
    if probe.frames == 600 {
        info!("Native train workshop responsive capture passed: 1080p and 720p; UI-only fixture");
        world.write_message(bevy::app::AppExit::Success);
    }
}

/// Inspect painted coordinates, including the responsive transform and window scale.
fn workshop_bounds(world: &mut World, window: Entity) -> Result<(), String> {
    fn bounds(node: &ComputedNode, transform: &UiGlobalTransform) -> (Vec2, Vec2) {
        let half = node.size() * 0.5;
        [
            Vec2::new(-half.x, -half.y),
            Vec2::new(half.x, -half.y),
            Vec2::new(-half.x, half.y),
            half,
        ]
        .into_iter()
        .map(|point| transform.transform_point2(point))
        .fold(
            (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
            |(low, high), point| (low.min(point), high.max(point)),
        )
    }
    let root = world
        .query::<(Entity, &tilt_ui::ElementId)>()
        .iter(world)
        .find_map(|(entity, id)| (id.0 == "train-workshop-panel").then_some(entity))
        .ok_or("Missing workshop panel")?;
    let (low, high) = bounds(
        world
            .get::<ComputedNode>(root)
            .ok_or("Missing panel geometry")?,
        world
            .get::<UiGlobalTransform>(root)
            .ok_or("Missing panel transform")?,
    );
    let size = world
        .get::<Window>(window)
        .ok_or("Missing window")?
        .resolution
        .physical_size()
        .as_vec2();
    if low.cmplt(Vec2::ZERO).any() || high.cmpgt(size).any() {
        return Err(format!("Panel outside window: {low:?} {high:?}, {size:?}"));
    }
    let primary = if world.resource::<TrainUiState>().selected.is_some() {
        "drive-start"
    } else {
        "blueprint-auto"
    };
    let mut controls = 0;
    for (entity, id, node, transform) in world
        .query::<(
            Entity,
            &tilt_ui::ElementId,
            &ComputedNode,
            &UiGlobalTransform,
        )>()
        .iter(world)
    {
        if !matches!(
            id.0.as_str(),
            "blueprint-auto"
                | "train-name"
                | "close"
                | "name-save"
                | "wheelbase-dec"
                | "wheelbase-inc"
                | "blueprint"
                | "blueprint-build"
                | "blueprint-clear"
                | "assemble"
                | "edit"
                | "seat"
                | "leave"
                | "destroy"
                | "destroy-confirm"
                | "destroy-cancel"
                | "train-vehicle"
                | "previous"
                | "next"
                | "clear"
                | "front"
                | "second-front"
                | "couple"
                | "uncouple"
                | "drive-start"
                | "drive-stop"
                | "drive-brake"
                | "engine"
                | "cargo"
                | "width:1"
                | "width:3"
                | "turn-left"
                | "turn-right"
        ) && !id.0.starts_with("kind:")
        {
            continue;
        }
        let mut parent = entity;
        while parent != root {
            let Some(next) = world.get::<ChildOf>(parent) else {
                break;
            };
            parent = next.parent();
        }
        if parent != root {
            continue;
        }
        let (a, b) = bounds(node, transform);
        if a.cmplt(low + Vec2::splat(8.0)).any() || b.cmpgt(high - Vec2::splat(8.0)).any() {
            return Err(format!(
                "Control {} touches frame: {a:?} {b:?}, panel {low:?} {high:?}",
                id.0
            ));
        }
        if id.0 == primary || matches!(id.0.as_str(), "train-name" | "close") {
            controls += 1;
        }
    }
    let model = world
        .query::<(&tilt_ui::ElementId, &bevy::ui::widget::ImageNode)>()
        .iter(world)
        .find_map(|(id, image)| (id.0 == "train-model-preview").then_some(image));
    if model.is_none_or(|image| image.image == Handle::default()) {
        return Err("Real train model preview was not bound".into());
    }
    if controls != 3 {
        return Err(format!(
            "Expected three visible workshop controls, found {controls}"
        ));
    }
    info!(
        ?low,
        ?high,
        controls,
        "Native workshop painted bounds fit window and frame"
    );
    Ok(())
}

/// Visual-only exhaust capture; authoritative driving is tested separately.
fn model(world: &mut World, probe: &mut Probe) {
    if probe.failed {
        return;
    }
    probe.frames += 1;
    let translation = (f64::from(probe.frames) / 120.0).sin() * 0.3;
    {
        let mut session = world.resource_mut::<ClientSession>();
        let car = &mut session.trains.vehicles[0];
        car.motion.speed_mps = 10.0;
        car.motion.fuel_available = true;
        car.motion.brake = false;
        car.motion.translation[0] = translation;
        // Two captures exercise visibly different wheel/crank poses, using the
        // same distance-driven animation as authoritative vehicles.
        car.motion.wheel_distance_m = f64::from(probe.frames) * 0.04;
    }
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera =
            Transform::from_xyz(10.0, 9.0, 16.0).looking_at(Vec3::new(0.5, 4.6, 6.0), Vec3::Y);
    }
    if matches!(probe.frames, 240 | 420) {
        let mesh = world
            .query::<(&Name, &bevy::mesh::Mesh3d)>()
            .iter(world)
            .find_map(|(name, mesh)| {
                (name.as_str() == "Locomotive exhaust").then_some(mesh.0.clone())
            });
        let vertices = mesh
            .as_ref()
            .and_then(|handle| world.resource::<Assets<Mesh>>().get(handle))
            .map_or(0, Mesh::count_vertices);
        let body = world
            .query::<&Name>()
            .iter(world)
            .any(|name| name.as_str() == "Assembled train");
        if vertices <= 32 || !body {
            error!(vertices, body, "Native model/exhaust did not materialize");
            probe.failed = true;
            world.write_message(bevy::app::AppExit::error());
            return;
        }
        let folder = std::path::PathBuf::from(std::env::var("TERRA_CAPTURE_DIR").unwrap());
        std::fs::create_dir_all(&folder).unwrap();
        let name = if probe.frames == 240 {
            "locomotive-model.png"
        } else {
            "locomotive-exhaust.png"
        };
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(folder.join(name)));
        info!(
            vertices,
            "Native locomotive model and exhaust are visible; visual-only fixture"
        );
    }
    if probe.frames == 480 {
        world.write_message(bevy::app::AppExit::Success);
    }
}
