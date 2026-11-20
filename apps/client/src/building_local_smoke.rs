//! Optional native construction acceptance, entered after real kingdom founding.
//! Every roster, chest, claim, order and voxel is acknowledged by the local server.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    ui::UiGlobalTransform,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    GameAction, GameLanguage, GameplayConfig, InputBinding, ItemKind,
    building_jobs::{BuildArea, BuildStatus, BuildWait},
    kingdom::{ClaimKind, KingdomId},
};
use terra_network::{
    client::{ClientSession, ContainerRequest, InventoryRequest},
    protocol::{
        inventory::{ContainerCommand, InventoryAction, InventoryCommand},
        kingdom::{KingdomAction, KingdomOutcome},
    },
};
use terra_player::{CameraInputState, CaptureRequest, TargetBlock, TerraCamera};
use terra_ui::{BuildingView, ElementId, KingdomMenuAction, KingdomView};
use terra_world::{edits::EditJournal, streaming::ChunkStore};

pub(crate) fn requested() -> bool {
    std::env::var("TERRA_BUILDING_LOCAL_GALLERY").as_deref() == Ok("1")
}

const ORIGIN: [i64; 3] = [24, 12, 24];
const PLANKS: ItemKind = ItemKind::Block(terra_voxel::MaterialId::OakPlanks as u16);
const STONE: ItemKind = ItemKind::Block(terra_voxel::MaterialId::StructureStone as u16);

#[derive(Resource)]
struct Gallery {
    started: Instant,
    since: Instant,
    step: u8,
    kingdom: KingdomId,
    keep: [i64; 3],
    chest: crate::inventory_benchmark::chest::ChestFixture,
    planks_granted: bool,
    planks_transferred: bool,
    areas_requested: bool,
    area_capture_done: bool,
    area_view_ready: Option<Instant>,
    walk: Option<WalkApproach>,
    second: [i64; 3],
    storage: Option<BuildArea>,
    last_edit: Option<(u64, terra_network::protocol::session::EditOutcome)>,
    claim_request: u64,
    job: u64,
    npc: u64,
    release: Option<GameAction>,
    captures: usize,
    capture_pending: bool,
    capture_failed: bool,
    language: GameLanguage,
}

struct WalkApproach {
    destination: [f32; 2],
    started: Instant,
    braking: bool,
    settled: Option<(Instant, u64)>,
    reported: bool,
}

impl Gallery {
    fn advance(&mut self) {
        self.step += 1;
        self.since = Instant::now();
    }
}

fn element(world: &mut World, id: &str) -> Option<Entity> {
    world
        .query::<(Entity, &ElementId, &ComputedNode)>()
        .iter(world)
        .find_map(|(entity, name, node)| {
            (name.0 == id && node.size().min_element() > 0.0).then_some(entity)
        })
}

fn rect(world: &World, entity: Entity) -> Result<Rect, String> {
    let node = world
        .get::<ComputedNode>(entity)
        .ok_or("Missing construction layout")?;
    let pose = world
        .get::<UiGlobalTransform>(entity)
        .ok_or("Missing construction transform")?;
    Ok(Rect::from_center_size(
        pose.translation,
        node.size() * pose.matrix2.x_axis.length(),
    ))
}

fn contains(outer: Rect, inner: Rect) -> bool {
    !inner.min.cmplt(outer.min - Vec2::splat(2.0)).any()
        && !inner.max.cmpgt(outer.max + Vec2::splat(2.0)).any()
}

fn belongs(world: &World, mut entity: Entity, ancestor: Entity) -> bool {
    for _ in 0..48 {
        if entity == ancestor {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}

fn check_bounds(world: &mut World, selection: bool) -> Result<(), String> {
    let window = world
        .query::<&Window>()
        .iter(world)
        .next()
        .ok_or("Missing construction window")?;
    let viewport = Rect::from_corners(Vec2::ZERO, window.resolution.physical_size().as_vec2());
    let id = if selection {
        "building-selection"
    } else {
        "kingdom-dialog"
    };
    let panel = element(world, id).ok_or_else(|| format!("Missing {id}"))?;
    let panel_rect = rect(world, panel)?;
    if !contains(viewport, panel_rect) {
        return Err(format!(
            "Construction panel leaves viewport: {id}/{panel_rect:?}/{viewport:?}"
        ));
    }
    if selection {
        return Ok(());
    }
    let scroll = element(world, "kingdom-scroll").ok_or("Missing keep scrolling body")?;
    let body = rect(world, scroll)?;
    if !contains(
        Rect::from_corners(
            panel_rect.min + Vec2::splat(16.0),
            panel_rect.max - Vec2::splat(16.0),
        ),
        body,
    ) {
        return Err("Building Local content overlaps the decorative keep border".into());
    }
    let controls: Vec<_> = world
        .query_filtered::<(Entity, &ElementId, &ComputedNode), With<tilt_ui::TiltButton>>()
        .iter(world)
        .filter(|(_, id, node)| id.0.starts_with("building-") && node.size().min_element() > 0.0)
        .map(|(entity, id, _)| (entity, id.0.clone()))
        .collect();
    if controls.is_empty() {
        return Err("No actual construction buttons were available for caption checks".into());
    }
    let captions: Vec<_> = world
        .query::<(Entity, &Text)>()
        .iter(world)
        .filter(|(_, text)| !text.0.trim().is_empty())
        .map(|(entity, text)| (entity, text.0.clone()))
        .collect();
    for (control, id) in controls {
        let bounds = rect(world, control)?;
        if bounds.min.x < body.min.x - 2.0 || bounds.max.x > body.max.x + 2.0 {
            return Err(format!(
                "Construction action leaves the content width: {id}/{bounds:?}/{body:?}"
            ));
        }
        for (caption, text) in &captions {
            if !belongs(world, *caption, control) {
                continue;
            }
            if !contains(bounds, rect(world, *caption)?) {
                return Err(format!(
                    "Construction caption does not fit its natural-width button: {id}/{text:?}"
                ));
            }
            if id != "building-select-template"
                && world
                    .get::<TextLayout>(*caption)
                    .is_some_and(|layout| layout.linebreak != bevy::text::LineBreak::NoWrap)
            {
                return Err(format!("Construction button caption wraps: {id}/{text:?}"));
            }
        }
    }
    Ok(())
}

fn reveal(world: &mut World, id: &str) -> Result<bool, String> {
    let entity = element(world, id).ok_or_else(|| format!("Missing construction control {id}"))?;
    let scroll = element(world, "kingdom-scroll").ok_or("Missing keep scroll")?;
    let target = rect(world, entity)?;
    let viewport = rect(world, scroll)?;
    if contains(viewport, target) {
        return Ok(true);
    }
    let scale = world
        .get::<UiGlobalTransform>(scroll)
        .map_or(1.0, |pose| pose.matrix2.x_axis.length().max(0.1));
    world
        .get_mut::<ScrollPosition>(scroll)
        .ok_or("Keep is not scrollable")?
        .0
        .y += (target.center().y - viewport.center().y) / scale;
    Ok(false)
}

pub(crate) fn emit_button(world: &mut World, entity: Entity) -> Result<(), String> {
    let handler = world
        .get::<tilt_ui::StaticAttributes>(entity)
        .and_then(|attributes| {
            attributes
                .attributes
                .iter()
                .find(|attribute| attribute.name == "onclick")
        })
        .map(|attribute| attribute.value.clone())
        .ok_or("The actual craftsman button has no HTML click handler")?;
    world.write_message(tilt_ui::HtmlEvent {
        target: entity,
        kind: "click",
        value: None,
        submitter: None,
        data: Default::default(),
        form_data: None,
        handler,
    });
    Ok(())
}

fn capture(
    world: &mut World,
    gallery: &mut Gallery,
    name: &str,
    selection: bool,
) -> Result<(), String> {
    check_bounds(world, selection)?;
    capture_frame(world, gallery, name)
}

fn capture_frame(world: &mut World, gallery: &mut Gallery, name: &str) -> Result<(), String> {
    let root = PathBuf::from(std::env::var_os("TERRA_CAPTURE_DIR").ok_or("Set TERRA_CAPTURE_DIR")?);
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(root.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut gallery: ResMut<Gallery>| {
                let colors = event
                    .image
                    .data
                    .as_deref()
                    .unwrap_or_default()
                    .chunks_exact(4)
                    .step_by(113)
                    .map(|pixel| [pixel[0], pixel[1], pixel[2]])
                    .collect::<std::collections::HashSet<_>>();
                gallery.capture_failed |= colors.len() < 16;
                gallery.capture_pending = false;
                gallery.captures += 1;
            },
        );
    gallery.capture_pending = true;
    Ok(())
}

fn action(world: &mut World, action: GameAction, pressed: bool) {
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(
            || action.default_binding(),
            |controls| controls.binding(action),
        );
    let key = |world: &mut World, code| {
        let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
        if pressed {
            keys.press(code);
        } else {
            keys.release(code);
        }
    };
    match binding {
        InputBinding::Key(code) => key(world, code),
        InputBinding::Chord {
            modifier,
            key: code,
        } => {
            key(world, modifier);
            key(world, code);
        }
        InputBinding::Mouse(button) => {
            let mut mouse = world.resource_mut::<ButtonInput<MouseButton>>();
            if pressed {
                mouse.press(button);
            } else {
                mouse.release(button);
            }
        }
    }
}

fn aim_point(world: &mut World, target: Vec3) {
    let eye = world
        .resource::<ClientSession>()
        .confirmed_state()
        .map(|pose| Vec3::from_array(pose.position) + Vec3::Y * terra_player::PLAYER_EYE_OFFSET);
    for (mut camera, mut look) in world
        .query_filtered::<(&mut Transform, &mut terra_player::CameraLook), With<TerraCamera>>()
        .iter_mut(world)
    {
        let rotation = Transform::from_translation(eye.unwrap_or(camera.translation))
            .looking_at(target, Vec3::Y)
            .rotation;
        camera.rotation = rotation;
        if !look.rotation().abs_diff_eq(rotation, 0.00001) {
            *look = default();
        }
    }
}

fn aim_block(world: &mut World, position: [i64; 3]) -> bool {
    aim_point(
        world,
        Vec3::from_array(position.map(|value| value as f32 + 0.5)),
    );
    world
        .resource::<TargetBlock>()
        .0
        .is_some_and(|hit| hit.position == position)
}

/// Walking is real input/prediction/authority, never a fixture teleport.
fn walk_to(
    world: &mut World,
    destination: [f32; 2],
    approach: &mut Option<WalkApproach>,
) -> Result<bool, String> {
    let confirmed = world
        .resource::<ClientSession>()
        .confirmed_state()
        .ok_or("Missing founder pose")?;
    let predicted = world
        .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
        .iter(world)
        .next()
        .ok_or("Missing actual predicting founder body")?
        .translation;
    if approach
        .as_ref()
        .is_some_and(|state| state.destination != destination)
    {
        *approach = None;
    }
    let state = approach.get_or_insert_with(|| WalkApproach {
        destination,
        started: Instant::now(),
        braking: false,
        settled: None,
        reported: false,
    });
    let body = confirmed.position;
    let delta = Vec2::new(destination[0] - body[0], destination[1] - body[2]);
    let prediction_delta = Vec2::new(destination[0] - predicted.x, destination[1] - predicted.z);
    let speed = Vec3::from_array(confirmed.velocity).length();
    if state.started.elapsed() > Duration::from_secs(30) {
        action(world, GameAction::Forward, false);
        action(world, GameAction::Precision, false);
        return Err(format!(
            "Native walk did not settle: destination={destination:?}, authority={:?}, prediction={predicted:?}, velocity={:?}, ack={}",
            confirmed.position, confirmed.velocity, confirmed.ack
        ));
    }
    // An old authority snapshot crossing the target is insufficient: prediction
    // can still be ahead, with movement commands or flight inertia outstanding.
    if delta.length() < 0.5 && prediction_delta.length() < 0.5 && speed < 0.08 {
        action(world, GameAction::Forward, false);
        action(world, GameAction::Precision, false);
        let (since, ack) = state
            .settled
            .get_or_insert_with(|| (Instant::now(), confirmed.ack));
        if since.elapsed() >= Duration::from_millis(350) && confirmed.ack > *ack {
            if !state.reported {
                info!(?destination,position=?confirmed.position,velocity=?confirmed.velocity,
                    "Native Building Local actual movement settled before UI transition");
                state.reported = true;
            }
            return Ok(true);
        }
        return Ok(false);
    }
    state.settled = None;
    if prediction_delta.length() <= 0.25 + speed * 0.20 {
        state.braking = true;
    }
    if state.braking {
        action(world, GameAction::Forward, false);
        action(world, GameAction::Precision, true);
        if speed < 0.08 {
            state.braking = false;
        }
        return Ok(false);
    }
    world.resource_mut::<CameraInputState>().captured = true;
    action(
        world,
        GameAction::Precision,
        delta.length().min(prediction_delta.length()) < 3.0,
    );
    // Steer from the real predicting body rather than lagging authority geometry.
    aim_point(
        world,
        Vec3::new(
            body[0] + prediction_delta.x,
            body[1] + terra_player::PLAYER_EYE_OFFSET,
            body[2] + prediction_delta.y,
        ),
    );
    action(world, GameAction::Forward, true);
    Ok(false)
}

fn open_building(world: &mut World, keep: [i64; 3]) -> Result<bool, String> {
    let view = world.resource::<KingdomView>();
    if !view.error.is_empty() {
        return Err(view.error.clone());
    }
    if !view.visible {
        world.write_message(KingdomMenuAction::Open(keep));
        return Ok(false);
    }
    if view.pending || !view.bound {
        return Ok(false);
    }
    if !view.building_tab {
        crate::menu_smoke::emit(world, "kingdom-tab-building", None)?;
        return Ok(false);
    }
    let building = world.resource::<BuildingView>();
    if !building.error.is_empty() {
        return Err(building.error.clone());
    }
    Ok(building.visible && !building.pending && element(world, "building-local").is_some())
}

fn inventory(world: &mut World, action: InventoryAction) {
    world.write_message(InventoryRequest(InventoryCommand {
        revision: world.resource::<ClientSession>().inventory.revision,
        action,
    }));
}

fn job(world: &World, id: u64) -> Option<&terra_network::protocol::building_jobs::BuildingJobView> {
    world.resource::<ClientSession>().buildings.jobs.get(&id)
}

fn selected_blocks_unchanged(world: &World, gallery: &Gallery) -> Result<(), String> {
    if world.resource::<ClientSession>().last_edit != gallery.last_edit {
        return Err("Construction selection sent an unintended world edit".into());
    }
    if world
        .resource::<ChunkStore>()
        .block_at(gallery.chest.position())
        != Some(terra_voxel::MaterialId::Chest as u16)
    {
        return Err("Construction selection damaged the real storage chest".into());
    }
    Ok(())
}

/// Called by kingdom_ui_smoke at stage 100 after the actual founding response.
pub(crate) fn run(world: &mut World) -> Result<bool, String> {
    if !world.contains_resource::<Gallery>() {
        let session = world.resource::<ClientSession>();
        let record = session
            .uuid
            .and_then(|uuid| session.kingdoms.active_for_member(uuid))
            .ok_or("Building acceptance requires the real founded kingdom")?;
        let (kingdom, keep) = (record.id, record.location);
        world.insert_resource(Gallery {
            started: Instant::now(),
            since: Instant::now(),
            step: 0,
            kingdom,
            keep,
            chest: Default::default(),
            planks_granted: false,
            planks_transferred: false,
            areas_requested: false,
            area_capture_done: false,
            area_view_ready: None,
            walk: None,
            second: [0; 3],
            storage: None,
            last_edit: None,
            claim_request: 0,
            job: 0,
            npc: 0,
            release: None,
            captures: 0,
            capture_pending: false,
            capture_failed: false,
            language: world.resource::<GameplayConfig>().language,
        });
    }
    world.resource_scope(|world, mut gallery: Mut<Gallery>| {
        if gallery.started.elapsed() > Duration::from_secs(140) {
            return Err(format!("Building Local acceptance timed out at step {}: view={:?}, job={:?}", gallery.step,
                world.resource::<BuildingView>(), job(world,gallery.job)));
        }
        if gallery.capture_failed { return Err("Construction capture contains no rendered scene/UI".into()); }
        if let Some(release) = gallery.release.take() { action(world, release, false); }
        let selection = world.resource::<BuildingView>().selection_active;
        world.resource_mut::<CameraInputState>().captured = selection || (gallery.step <= 6) || matches!(gallery.step, 16|17);
        if gallery.capture_pending || gallery.since.elapsed() < Duration::from_millis(350) { return Ok(false); }
        match gallery.step {
            0 => {
                world.write_message(KingdomMenuAction::Close);
                world.write_message(CaptureRequest(true));
            }
            1 => {
                if world.resource::<KingdomView>().visible || !gallery.chest.prepare(world)? { return Ok(false); }
                inventory(world, InventoryAction::CreativeSet {slot:0,item:STONE,count:128});
            }
            2 => {
                if !world.resource::<ClientSession>().inventory.slot(0).is_some_and(|stack| stack.item == STONE && stack.count == 128) { return Ok(false); }
                if !gallery.planks_granted {
                    inventory(world,InventoryAction::CreativeSet {slot:1,item:PLANKS,count:128});
                    gallery.planks_granted=true;
                    return Ok(false);
                }
                if !world.resource::<ClientSession>().inventory.slot(1).is_some_and(|stack| stack.item == PLANKS && stack.count == 128) { return Ok(false); }
                if !aim_block(world,gallery.chest.position()) { return Ok(false); }
                inventory(world,InventoryAction::OpenContainer {position:gallery.chest.position()});
            }
            3 => {
                let session = world.resource::<ClientSession>();
                let Some(container) = session.container.as_ref().filter(|view| view.anchor == gallery.chest.position()) else { return Ok(false); };
                world.write_message(ContainerRequest(ContainerCommand {
                    inventory_revision:session.inventory.revision,anchor:container.anchor,view:container.view,
                    revisions:container.halves.iter().map(|half| half.inventory.revision).collect(),
                    reserved:[0;terra_core::INVENTORY_SLOTS],action:terra_core::containers::ContainerGesture::QuickMove {slot:0},
                }));
            }
            4 => {
                let session = world.resource::<ClientSession>();
                let Some(container)=session.container.as_ref().filter(|view|view.anchor==gallery.chest.position()) else {return Ok(false);};
                let stone_stored=container.halves.iter().any(|half|half.inventory.count(STONE)>=128);
                if !stone_stored || session.inventory.slot(0).is_some() { return Ok(false); }
                if !gallery.planks_transferred {
                    let command=ContainerCommand {
                        inventory_revision:session.inventory.revision,anchor:container.anchor,view:container.view,
                        revisions:container.halves.iter().map(|half|half.inventory.revision).collect(),
                        reserved:[0;terra_core::INVENTORY_SLOTS],action:terra_core::containers::ContainerGesture::QuickMove {slot:1},
                    };
                    world.write_message(ContainerRequest(command));
                    gallery.planks_transferred=true;
                    return Ok(false);
                }
                let planks_stored=container.halves.iter().any(|half|half.inventory.count(PLANKS)>=128);
                if !planks_stored || session.inventory.slot(1).is_some() {return Ok(false);}
                info!(position=?gallery.chest.position(), "Native Building Local real town chest stocked with 128 structure stone and 128 planks");
                gallery.last_edit = session.last_edit;
                inventory(world,InventoryAction::CloseContainer);
            }
            5 => {
                if world.resource::<ClientSession>().container.is_some() { return Ok(false); }
                world.resource_mut::<GameplayConfig>().language = GameLanguage::German;
                if !open_building(world,gallery.keep)? { return Ok(false); }
            }
            6 => {
                if !open_building(world,gallery.keep)? { return Ok(false); }
                if world.resource::<BuildingView>().workers.is_empty() { return Ok(false); }
                if world.resource::<BuildingView>().templates.len()<2 || !world.resource::<BuildingView>().template_selected {return Ok(false);}
                if !world.resource::<BuildingView>().templates.iter().any(|template|template.id==terra_core::building_jobs::HOUSE_TEMPLATE&&template.selected) {
                    return Err("Native default house was not selected from the real server catalog".into());
                }
                if world.resource::<ClientSession>().work.workers.iter().any(|worker|worker.order.is_some()) {
                    return Err("Building acceptance expected F80's real unemployed founding roster".into());
                }
                capture(world,&mut gallery,"building-local-de-wide",false)?;
            }
            7 => {
                if !reveal(world,"building-pick-storage")? { return Ok(false); }
                crate::menu_smoke::emit(world,"building-pick-storage",None)?;
            }
            8 => {
                if !selection || element(world,"building-selection").is_none() { return Ok(false); }
                if !world.resource::<terra_player::WorldInteractionBlock>().0 {
                    return Err("Building selection does not reserve world editing".into());
                }
                capture(world,&mut gallery,"building-storage-selection-de-wide",true)?;
            }
            9 => {
                if !aim_block(world,gallery.chest.position()) { return Ok(false); }
                action(world,GameAction::Break,true); gallery.release=Some(GameAction::Break);
            }
            10 => {
                selected_blocks_unchanged(world,&gallery)?;
                let Some(ground)=crate::inventory_benchmark::chest::choose_ground(world) else { return Ok(false); };
                gallery.second=ground;
                let first=gallery.chest.position();
                gallery.storage=Some(BuildArea::new(std::array::from_fn(|i| first[i].min(ground[i])),
                    std::array::from_fn(|i| first[i].max(ground[i]) + i64::from(i==1)))
                    .map_err(|error| format!("Invalid native chest selection: {error:?}"))?);
            }
            11 => {
                if !aim_block(world,gallery.second) { return Ok(false); }
                action(world,GameAction::Break,true);gallery.release=Some(GameAction::Break);
            }
            12 => {
                selected_blocks_unchanged(world,&gallery)?;
                if !open_building(world,gallery.keep)? { return Ok(false); }
                if !world.resource::<BuildingView>().can_save_storage { return Ok(false); }
                if !reveal(world,"building-save-storage")? { return Ok(false); }
                crate::menu_smoke::emit(world,"building-save-storage",None)?;
            }
            13 => {
                let session=world.resource::<ClientSession>();
                if session.buildings.settlements.get(&gallery.kingdom).and_then(|town|town.storage) != gallery.storage { return Ok(false); }
                if !gallery.areas_requested {
                    if !reveal(world,"building-toggle-areas")? {return Ok(false);}
                    crate::menu_smoke::emit(world,"building-toggle-areas",None)?;
                    gallery.areas_requested=true;
                    return Ok(false);
                }
                if world.resource::<terra_core::work_areas::WorkAreaDisplay>().target != Some(terra_core::work_areas::WorkAreaTarget::Settlement(gallery.kingdom)) {return Ok(false);}
                let request=world.resource_mut::<ClientSession>().kingdom_request(KingdomAction::ClaimChunk {
                    id:gallery.kingdom,chunk:[1,1],kind:ClaimKind::ProtectedBuilding,
                }).ok_or("Native construction claim request was rate limited")?;
                gallery.claim_request=request.0.request;
                world.write_message(request);
            }
            14 => {
                let session=world.resource::<ClientSession>();
                if let Some(result)=session.last_kingdom_result.as_ref().filter(|result|result.request==gallery.claim_request)
                    && let KingdomOutcome::Rejected(error)=&result.outcome {
                        return Err(format!("Native house Build claim rejected: {error:?}"));
                    }
                if !session.kingdoms.claim_at([1,1]).is_some_and(|claim|claim.kingdom==gallery.kingdom&&claim.kind==ClaimKind::ProtectedBuilding) { return Ok(false); }
                world.write_message(KingdomMenuAction::Close);
                world.write_message(CaptureRequest(true));
            }
            15 => { if world.resource::<KingdomView>().visible { return Ok(false); } }
            16 => {
                if !gallery.area_capture_done {
                    // View the real saved box from outside instead of capturing
                    // with the camera inside it. Movement still uses real input.
                    if !walk_to(world,[8.0,19.0],&mut gallery.walk)? { return Ok(false); }
                    aim_point(world,Vec3::new(8.0,12.5,25.0));
                    let view=world.resource::<terra_ui::WorkAreasView>();
                    if !view.visible || !view.available {return Ok(false);}
                    let Some(hud)=element(world,"work-area-overlay") else {return Ok(false);};
                    let geometry=rect(world,hud)?;
                    if geometry.width()<=0.0 || geometry.height()<=0.0 {return Ok(false);}
                    let window=world.query::<&Window>().iter(world).next().ok_or("Missing world window")?;
                    let viewport=Rect::from_corners(Vec2::ZERO,window.resolution.physical_size().as_vec2());
                    if !contains(viewport,geometry) {return Err("Saved-area legend leaves the viewport".into());}
                    let ready=gallery.area_view_ready.get_or_insert_with(Instant::now);
                    if ready.elapsed()<Duration::from_millis(400) {return Ok(false);}
                    let mut confirmed=None;
                    terra_network::client::work_areas::visit_saved_areas(world.resource::<ClientSession>(),world.resource::<terra_core::work_areas::WorkAreaDisplay>().target,|mark| {
                        if mark.kind==terra_core::work_areas::WorkAreaKind::Storage {confirmed=Some(mark.bounds);}
                    });
                    if confirmed!=gallery.storage {return Err("Displayed area differs from the actually saved town storage".into());}
                    capture_frame(world,&mut gallery,"building-saved-areas-de-wide")?;
                    gallery.area_capture_done=true;
                    info!("Native Building Local saved area HUD and confirmed bounds acknowledged");
                    return Ok(false);
                }
                if !walk_to(world,[8.0,19.0],&mut gallery.walk)? { return Ok(false); }
            }
            17 => { if !walk_to(world,[24.0,21.0],&mut gallery.walk)? { return Ok(false); } }
            18 => { if !open_building(world,gallery.keep)? { return Ok(false); } }
            19 => {
                if !reveal(world,"building-pick-house")? { return Ok(false); }
                crate::menu_smoke::emit(world,"building-pick-house",None)?;
            }
            20 => {
                let ground=[ORIGIN[0],ORIGIN[1]-1,ORIGIN[2]];
                // Aim at the visible top face, within the normal 4.5 m reach.
                // A shallow ray towards the buried cell center hits nearer ground.
                aim_point(world,Vec3::new(ground[0] as f32+0.5,ground[1] as f32+0.99,ground[2] as f32+0.5));
                if !selection || !world.resource::<TargetBlock>().0.is_some_and(|hit|hit.position==ground) {
                    if gallery.since.elapsed()>Duration::from_secs(8) {
                        return Err(format!("House surface pick failed within eight seconds: selection={selection}, expected={ground:?}, target={:?}, player={:?}, loaded={:?}",
                            world.resource::<TargetBlock>().0,world.resource::<ClientSession>().confirmed_state(),world.resource::<ChunkStore>().block_at(ground)));
                    }
                    return Ok(false);
                }
                action(world,GameAction::Break,true);gallery.release=Some(GameAction::Break);
            }
            21 => {
                selected_blocks_unchanged(world,&gallery)?;
                if !open_building(world,gallery.keep)? || !world.resource::<BuildingView>().can_queue { return Ok(false); }
                if !reveal(world,"building-queue-house")? { return Ok(false); }
                crate::menu_smoke::emit(world,"building-queue-house",None)?;
            }
            22 => {
                let session=world.resource::<ClientSession>();
                let Some(created)=session.buildings.jobs.values().find(|job|job.kingdom==gallery.kingdom&&job.origin==ORIGIN) else {
                    let view=world.resource::<BuildingView>();
                    if !view.error.is_empty() { return Err(format!("Actual house UI rejected: {}",view.error)); }
                    return Ok(false);
                };
                if created.required_materials.is_empty() || !created.workers.is_empty()
                    || created.template != terra_core::building_jobs::HOUSE_TEMPLATE
                    || !matches!(created.status,BuildStatus::Waiting(BuildWait::NoWorkers)|BuildStatus::Queued) {
                    return Err(format!("New real house did not expose requirements and missing workers: {created:?}"));
                }
                gallery.job=created.id;
                for mut window in world.query::<&mut Window>().iter_mut(world) { window.resolution.set_physical_resolution(960,640); }
                world.resource_mut::<GameplayConfig>().language=GameLanguage::English;
            }
            23 => {
                if !open_building(world,gallery.keep)? || !reveal(world,"building-pause")? { return Ok(false); }
                capture(world,&mut gallery,"building-job-en-compact",false)?;
            }
            24 => {
                if !reveal(world,"building-add-craftsman")? { return Ok(false); }
                if !world.resource::<BuildingView>().workers.iter().any(|worker|!worker.registered&&worker.can_assign) {
                    return Err("No real unassigned founding villager can become a craftsman".into());
                }
                let button=element(world,"building-add-craftsman").ok_or("Actual craftsman button disappeared")?;
                emit_button(world,button)?;
            }
            25 => {
                // Keyed row entities and the roster vector may have different orders.
                // Read the NPC resolved by the real clicked row's inline $set.
                let Some(npc)=world.resource::<tilt_ui::UiBindingStore>().json_path("buildingIntent.npc")
                    .and_then(|value|value.as_str().and_then(|npc|npc.parse::<u64>().ok())) else {return Ok(false);};
                if npc==0 {return Err("Actual craftsman button resolved a zero NPC id".into());}
                gallery.npc=npc;
                let session=world.resource::<ClientSession>();
                if !session.buildings.settlements.get(&gallery.kingdom).is_some_and(|town|town.craftsmen.contains(&gallery.npc))
                    || !job(world,gallery.job).is_some_and(|job|job.workers.contains(&gallery.npc)) { return Ok(false); }
                if session.work.workers.iter().any(|worker|worker.npc==gallery.npc&&worker.order.is_some()) {
                    return Err("Craftsman registration fabricated an ordinary per-worker work area".into());
                }
                info!(npc=gallery.npc,job=gallery.job,"Native Building Local real craftsman enrolled without work area");
            }
            26 => {
                let progress=job(world,gallery.job).map_or(0.0,|job|job.progress());
                let journal=world.resource::<EditJournal>();
                let changed=(18..=29).any(|x|(17..=29).any(|z|(10..=21).any(|y|journal.block([x,y,z]).is_some_and(|material|material!=0))));
                if progress <= 0.0 || !changed { return Ok(false); }
                if !open_building(world,gallery.keep)? || !reveal(world,"building-pause")? { return Ok(false); }
                capture(world,&mut gallery,"building-progress-en-compact",false)?;
                info!(job=gallery.job,progress,"Native Building Local actual non-air house voxel placement acknowledged");
            }
            27 => {
                if !open_building(world,gallery.keep)? || !reveal(world,"building-pause")? { return Ok(false); }
                crate::menu_smoke::emit(world,"building-pause",None)?;
            }
            28 => {
                if !job(world,gallery.job).is_some_and(|job|!job.enabled&&job.status==BuildStatus::Paused) { return Ok(false); }
                if !open_building(world,gallery.keep)? || !reveal(world,"building-resume")? { return Ok(false); }
                capture(world,&mut gallery,"building-paused-en-compact",false)?;
            }
            _ => {
                world.resource_mut::<GameplayConfig>().language=gallery.language;
                action(world,GameAction::Forward,false);
                if gallery.captures!=6 { return Err(format!("Expected six Building Local captures, got {}",gallery.captures)); }
                info!(npc=gallery.npc,job=gallery.job,captures=gallery.captures,
                    "Native Building Local passed: real storage HUD selection, chest transfer, claim, house costs, craftsman without area, visible voxel progress and pause; DE/EN natural-width geometry");
                return Ok(true);
            }
        }
        info!(step=gallery.step,"Native Building Local acceptance step completed");
        gallery.advance();
        Ok(false)
    })
}
