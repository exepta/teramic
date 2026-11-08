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
#[derive(Resource, Default)]
struct Probe {
    initialized: bool,
    frames: u32,
    transactions: u8,
    cursor: MessageCursor<TrainRequest>,
    location: Option<Location>,
}
pub(super) fn install(app: &mut App) {
    if std::env::var("TERRA_TRAIN_INVENTORY_LIVE").as_deref() != Ok("1") {
        return;
    }
    app.init_resource::<Probe>()
        .add_systems(Update, run.after(super::update));
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut probe: Mut<Probe>| {
        if !probe.initialized {
            if !world.get_resource::<crate::menu::MenuState>().is_some_and(|menu| menu.page == crate::menu::MenuPage::Main) { return; }
            let owner = PlayerUuid::from_u128(1);
            let parts = terra_core::train_blueprint::template([0, 2, 2], 0, VehicleKind::Locomotive).into_iter().map(|cell| TrainPart {offset: cell.cell.map(|v| v as i16), material: cell.material, facing: cell.facing, wood: Default::default()}).collect::<Vec<_>>();
            let engine = parts.iter().position(|part| part.material == 218).unwrap() as u16;
            let car = Vehicle { id: 1, owner, kind: VehicleKind::Locomotive, anchor: [0;3], parts, cargo: [(engine, Inventory::default())].into(), blocks: Default::default(), payloads: Default::default(), couplings: [None;2], revision: 7, motion: Default::default(), burn_seconds: 0.0 };
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
            world.write_message(terra_player::CaptureRequest(false));
            probe.initialized=true;
            return;
        }
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
            world.spawn(Screenshot::primary_window()).observe(save_to_disk(if probe.frames < 360 { if cargo {"target/live217/engine-filled.png"} else {"target/live217/engine-before.png"} } else if cargo {"target/live217/storage-filled.png"} else {"target/live217/storage-before.png"}));
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
