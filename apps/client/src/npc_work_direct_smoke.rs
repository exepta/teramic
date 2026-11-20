//! Optional direct-worker acceptance after an actual acknowledged kingdom founding.
//! Selection uses normal camera raycasts and input; all state comes from the server.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    ui::UiGlobalTransform,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{GameAction, GameLanguage, GameplayConfig, InputBinding};
use terra_network::client::{ClientChatCommands, ClientSession, ReplicatedCreatures};
use terra_player::{
    CameraInputState, CaptureRequest, TargetBlock, TargetCreature, TerraCamera,
    WorldInteractionBlock,
};
use terra_ui::{ElementId, KingdomMenuAction, NpcWorkView};
use terra_world::streaming::ChunkStore;

pub(crate) fn requested() -> bool {
    std::env::var("TERRA_NPC_WORK_DIRECT_GALLERY").as_deref() == Ok("1")
}

#[derive(Resource)]
struct Gallery {
    started: Instant,
    since: Instant,
    step: u8,
    chest: crate::inventory_benchmark::chest::ChestFixture,
    npc: Option<u64>,
    fallback_spawned: bool,
    kingdom_open_requested: bool,
    kingdom_checked: bool,
    aim_index: usize,
    first: [i64; 3],
    second: [i64; 3],
    old_edit: Option<(u64, terra_network::protocol::session::EditOutcome)>,
    old_blocks: [Option<u16>; 2],
    old_area: [String; 6],
    captures: usize,
    capture_pending: bool,
    capture_failed: bool,
    language: GameLanguage,
    case: usize,
    release: Option<GameAction>,
    right_release: bool,
    station: Option<crate::inventory_benchmark::chest::ChestFixture>,
    original_avatar: Option<Entity>,
    sex: Option<terra_core::creatures::CreatureSex>,
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
        .ok_or("Missing direct NPC node geometry")?;
    let transform = world
        .get::<UiGlobalTransform>(entity)
        .ok_or("Missing direct NPC node transform")?;
    Ok(Rect::from_center_size(
        transform.translation,
        node.size() * transform.matrix2.x_axis.length(),
    ))
}

fn contains(outer: Rect, inner: Rect, tolerance: f32) -> bool {
    !inner.min.cmplt(outer.min - Vec2::splat(tolerance)).any()
        && !inner.max.cmpgt(outer.max + Vec2::splat(tolerance)).any()
}

/// Reusable bounds check for a real opened dialog, including the private slot grid.
/// Scrolled content may be outside the viewport vertically, never horizontally.
pub(crate) fn check_dialog_bounds(world: &mut World) -> Result<(), String> {
    let dialog = element(world, "npc-work-dialog").ok_or("Missing direct NPC dialog")?;
    let scroll =
        element(world, "npc-work-dialog-scroll").ok_or("Missing direct NPC scrolling body")?;
    let close = element(world, "npc-work-close").ok_or("Missing direct NPC close button")?;
    let outer = rect(world, dialog)?;
    let body = rect(world, scroll)?;
    let safe = Rect::from_corners(outer.min + Vec2::splat(20.0), outer.max - Vec2::splat(20.0));
    if !contains(safe, body, 2.0) || !contains(safe, rect(world, close)?, 2.0) {
        return Err(format!(
            "Direct NPC content/close button overlaps decorative rim: {body:?}/{safe:?}"
        ));
    }
    let viewport = world
        .query::<&Window>()
        .iter(world)
        .next()
        .map(|window| {
            Vec2::new(
                window.physical_width() as f32,
                window.physical_height() as f32,
            )
        })
        .ok_or("Missing acceptance window")?;
    if !contains(Rect::from_corners(Vec2::ZERO, viewport), outer, 2.0) {
        return Err(format!(
            "Direct NPC dialog leaves viewport: {outer:?}/{viewport:?}"
        ));
    }
    let editor = element(world, "npc-work-editor").ok_or("Missing direct NPC work editor")?;
    if rect(world, editor)?.width() < body.width() * 0.9 {
        return Err("Direct NPC editor does not use the dialog content width".into());
    }
    let slots: Vec<_> = world
        .query::<(Entity, &tilt_ui::ElementClasses)>()
        .iter(world)
        .filter(|(_, classes)| {
            classes
                .classes
                .iter()
                .any(|class| class == "npc-direct-slot")
        })
        .map(|(entity, _)| entity)
        .collect();
    if slots.len() != terra_core::npc_jobs::WORKER_SLOTS {
        return Err(format!(
            "Expected nine real NPC inventory slots, found {}",
            slots.len()
        ));
    }
    for entity in slots {
        let bounds = rect(world, entity)?;
        if bounds.min.x < body.min.x - 2.0 || bounds.max.x > body.max.x + 2.0 {
            return Err(format!(
                "Direct NPC inventory slot leaves content width: {bounds:?}/{body:?}"
            ));
        }
    }
    let captions: Vec<_> = world
        .query::<(Entity, &Text)>()
        .iter(world)
        .filter(|(_, text)| !text.0.trim().is_empty())
        .map(|(entity, text)| (entity, text.0.clone()))
        .collect();
    for id in [
        "npc-work-pick-area",
        "npc-work-pick-storage",
        "npc-work-assign",
    ] {
        let entity =
            element(world, id).ok_or_else(|| format!("Missing direct NPC control {id}"))?;
        let bounds = rect(world, entity)?;
        if bounds.min.x < body.min.x - 2.0 || bounds.max.x > body.max.x + 2.0 {
            return Err(format!(
                "Direct NPC control {id} leaves content width: {bounds:?}/{body:?}"
            ));
        }
        let mut caption_found = false;
        for (caption, text) in &captions {
            let mut current = Some(*caption);
            let mut belongs = false;
            for _ in 0..16 {
                let Some(ancestor) = current else { break };
                if ancestor == entity {
                    belongs = true;
                    break;
                }
                current = world.get::<ChildOf>(ancestor).map(ChildOf::parent);
            }
            if !belongs {
                continue;
            }
            caption_found = true;
            if !contains(bounds, rect(world, *caption)?, 2.0) {
                return Err(format!(
                    "Direct NPC caption {text:?} does not fit its content-sized control {id}"
                ));
            }
            if world
                .get::<TextLayout>(*caption)
                .is_some_and(|layout| layout.linebreak != bevy::text::LineBreak::NoWrap)
            {
                return Err(format!(
                    "Direct NPC action caption unexpectedly wraps: {text:?}"
                ));
            }
        }
        if !caption_found {
            return Err(format!(
                "Missing translated caption for direct NPC action {id}"
            ));
        }
    }
    let area = element(world, "npc-work-pick-area").ok_or("Missing area picker")?;
    let storage = element(world, "npc-work-pick-storage").ok_or("Missing storage picker")?;
    let (area, storage) = (rect(world, area)?, rect(world, storage)?);
    if (area.center().y - storage.center().y).abs() > 2.0 || area.max.x > storage.min.x {
        return Err("Direct NPC pickers do not remain beside each other".into());
    }
    if area.width() <= storage.width() + 5.0 || area.width() >= body.width() * 0.8 {
        return Err("Direct NPC action widths do not follow their translated text".into());
    }
    Ok(())
}

fn reveal(world: &mut World, id: &str) -> Result<bool, String> {
    let entity = element(world, id).ok_or_else(|| format!("Missing direct NPC control {id}"))?;
    let scroll =
        element(world, "npc-work-dialog-scroll").ok_or("Missing direct NPC scroll body")?;
    let target = rect(world, entity)?;
    let viewport = rect(world, scroll)?;
    if contains(viewport, target, 2.0) {
        return Ok(true);
    }
    let scale = world
        .get::<UiGlobalTransform>(scroll)
        .map_or(1.0, |pose| pose.matrix2.x_axis.length().max(0.1));
    let mut offset = world
        .get_mut::<ScrollPosition>(scroll)
        .ok_or("Direct NPC body is not scrollable")?;
    offset.0.y += (target.center().y - viewport.center().y) / scale;
    Ok(false)
}

fn shot(
    world: &mut World,
    gallery: &mut Gallery,
    name: &str,
    selection: bool,
) -> Result<(), String> {
    if selection {
        let hud = element(world, "npc-work-selection").ok_or("Missing selection mode HUD")?;
        let bounds = rect(world, hud)?;
        let window = world
            .query::<&Window>()
            .iter(world)
            .next()
            .ok_or("Missing native window")?;
        let viewport = Rect::from_corners(
            Vec2::ZERO,
            Vec2::new(
                window.physical_width() as f32,
                window.physical_height() as f32,
            ),
        );
        if !contains(viewport, bounds, 2.0) {
            return Err(format!("Work selection HUD leaves viewport: {bounds:?}"));
        }
    } else {
        check_dialog_bounds(world)?;
    }
    capture(world, gallery, name)
}

fn capture(world: &mut World, gallery: &mut Gallery, name: &str) -> Result<(), String> {
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
                gallery.captures += 1;
                gallery.capture_pending = false;
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

fn area(view: &NpcWorkView) -> [String; 6] {
    [
        view.min_x.clone(),
        view.min_y.clone(),
        view.min_z.clone(),
        view.max_x.clone(),
        view.max_y.clone(),
        view.max_z.clone(),
    ]
}

fn unchanged(world: &World, gallery: &Gallery) -> Result<(), String> {
    if world.resource::<ClientSession>().last_edit != gallery.old_edit {
        return Err("The work selection tool sent a world edit".into());
    }
    for (position, expected) in [gallery.first, gallery.second]
        .into_iter()
        .zip(gallery.old_blocks)
    {
        if world.resource::<ChunkStore>().block_at(position) != expected {
            return Err("The work selection tool changed a selected block".into());
        }
    }
    Ok(())
}

fn resize(world: &mut World, width: u32, height: u32, language: GameLanguage) {
    for mut window in world.query::<&mut Window>().iter_mut(world) {
        window.resolution.set_physical_resolution(width, height);
    }
    world.resource_mut::<GameplayConfig>().language = language;
}

/// Hook from npc_work_smoke::run when requested, before its legacy gallery.
pub(crate) fn run(world: &mut World) -> Result<bool, String> {
    if !world.contains_resource::<Gallery>() {
        world.insert_resource(Gallery {
            started: Instant::now(),
            since: Instant::now(),
            step: 0,
            chest: Default::default(),
            npc: None,
            fallback_spawned: false,
            kingdom_open_requested: false,
            kingdom_checked: false,
            aim_index: 0,
            first: [0; 3],
            second: [0; 3],
            old_edit: None,
            old_blocks: [None; 2],
            old_area: Default::default(),
            captures: 0,
            capture_pending: false,
            capture_failed: false,
            language: world.resource::<GameplayConfig>().language,
            case: 0,
            release: None,
            right_release: false,
            station: None,
            original_avatar: None,
            sex: None,
        });
    }
    world.resource_scope(|world, mut gallery: Mut<Gallery>| {
        if gallery.started.elapsed() > Duration::from_secs(110) { return Err(format!("Direct NPC acceptance timed out at step {}", gallery.step)); }
        if gallery.capture_failed { return Err("Direct NPC capture contains no rendered UI".into()); }
        if let Some(release) = gallery.release.take() { action(world, release, false); }
        if gallery.right_release { world.resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Right); gallery.right_release = false; }
        let selection = world.resource::<NpcWorkView>().selection_active;
        world.resource_mut::<CameraInputState>().captured = selection || gallery.step <= 2;
        if gallery.capture_pending || gallery.since.elapsed() < Duration::from_millis(450) { return Ok(false); }
        match gallery.step {
            0 => {
                world.write_message(KingdomMenuAction::Close);
                world.write_message(CaptureRequest(true));
            }
            1 => {
                if !gallery.kingdom_checked {
                    if !gallery.kingdom_open_requested {
                        action(world, GameAction::Kingdom, true);
                        gallery.release = Some(GameAction::Kingdom);
                        gallery.kingdom_open_requested = true;
                        return Ok(false);
                    }
                    let view = world.resource::<terra_ui::KingdomView>();
                    if !view.error.is_empty() { return Err(view.error.clone()); }
                    if !view.visible || view.pending || !view.bound { return Ok(false); }
                    info!("Native Kingdom hotkey opened the authoritative own-territory keep");
                    gallery.kingdom_checked = true;
                    world.write_message(KingdomMenuAction::Close);
                    return Ok(false);
                }
                if world.resource::<terra_ui::KingdomView>().visible { return Ok(false); }
                if !gallery.chest.prepare(world)? { return Ok(false); }
                gallery.first = gallery.chest.position();
                // Reuse the fixture's actual loaded-ground raycast. A guessed
                // cell behind the chest or keep may be occluded or just air.
                let Some(second) = crate::inventory_benchmark::chest::choose_ground(world) else { return Ok(false); };
                gallery.second = second;
                gallery.old_edit = world.resource::<ClientSession>().last_edit;
                gallery.old_blocks = [gallery.first, gallery.second].map(|position| world.resource::<ChunkStore>().block_at(position));
            }
            2 => {
                let Some(pose) = world.resource::<ClientSession>().confirmed_state() else { return Ok(false); };
                let eye = Vec3::from_array(pose.position) + Vec3::Y * terra_player::PLAYER_EYE_OFFSET;
                let uuid = world.resource::<ClientSession>().uuid;
                let replicas = world.resource::<ReplicatedCreatures>();
                let mut candidates: Vec<_> = replicas.creatures.values()
                    .filter(|track| track.current.kind == terra_core::creatures::CreatureKind::Villager
                        && track.current.health > 0.0 && terra_core::creatures::villagers::is_civilian(track.current.variant)
                        && replicas.bonds.get(&track.current.id).and_then(|bond| bond.owner).is_none_or(|owner| Some(owner) == uuid))
                    .map(|track| (track.current.id, track.position() + Vec3::Y)).filter(|(_, position)| eye.distance(*position) < 5.0).collect();
                candidates.sort_by(|left, right| eye.distance_squared(left.1).total_cmp(&eye.distance_squared(right.1)));
                if let Some(&(id, target)) = candidates.get(gallery.aim_index % candidates.len().max(1)) {
                    aim_point(world, target);
                    if gallery.npc != Some(id) {
                        gallery.npc = Some(id);
                        gallery.since = Instant::now();
                        return Ok(false);
                    }
                    if world.resource::<TargetCreature>().0 == Some(id) {
                        world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
                        gallery.right_release = true;
                    } else {
                        if gallery.since.elapsed() > Duration::from_secs(2) {
                            gallery.aim_index += 1;
                            gallery.npc = None;
                            gallery.since = Instant::now();
                            if gallery.aim_index >= candidates.len() && !gallery.fallback_spawned {
                                world.resource_mut::<ClientChatCommands>().0.push_back(terra_network::client::ChatCommand::Send("/spawn teramic::villager".into()));
                                gallery.fallback_spawned = true;
                                gallery.aim_index = 0;
                            }
                        }
                        return Ok(false);
                    }
                } else {
                    if !gallery.fallback_spawned {
                        world.resource_mut::<ClientChatCommands>().0.push_back(terra_network::client::ChatCommand::Send("/spawn teramic::villager".into()));
                        gallery.fallback_spawned = true;
                    }
                    return Ok(false);
                }
            }
            3 => {
                let view = world.resource::<NpcWorkView>();
                if !view.direct || !view.visible || !view.inventory_ready { return Ok(false); }
                if !view.error.is_empty() { return Err(view.error.clone()); }
                let detail = world.resource::<ClientSession>().work.detail.as_ref().ok_or("Direct UI has no authoritative private worker detail")?;
                if Some(detail.npc) != gallery.npc || view.private_inventory.len() != terra_core::npc_jobs::WORKER_SLOTS {
                    return Err("Direct inspection does not match the right-clicked NPC and private nine-slot inventory".into());
                }
                crate::menu_smoke::emit(world, "npc-work-field-kind", Some("collector"))?;
                resize(world, 1600, 900, GameLanguage::German);
            }
            4 => { shot(world, &mut gallery, "npc-direct-de-wide", false)?; }
            5 => {
                if !reveal(world, "npc-work-pick-area")? { return Ok(false); }
                gallery.old_area = area(world.resource::<NpcWorkView>());
                crate::menu_smoke::emit(world, "npc-work-pick-area", None)?;
            }
            6 => {
                if !world.resource::<NpcWorkView>().selection_active { return Ok(false); }
                if world.resource::<NpcWorkView>().direct || !world.resource::<WorldInteractionBlock>().0 { return Err("Area picker did not leave dialog and block world edits".into()); }
                unchanged(world, &gallery)?;
                shot(world, &mut gallery, "npc-selection-de-wide", true)?;
            }
            7 => { action(world, GameAction::Pause, true); gallery.release = Some(GameAction::Pause); }
            8 => {
                if !world.resource::<NpcWorkView>().direct || world.resource::<NpcWorkView>().selection_active { return Ok(false); }
                if area(world.resource::<NpcWorkView>()) != gallery.old_area { return Err("Cancelling area selection changed the work draft".into()); }
                unchanged(world, &gallery)?;
                crate::menu_smoke::emit(world, "npc-work-pick-area", None)?;
            }
            9 | 11 => {
                if !world.resource::<NpcWorkView>().selection_active { return Ok(false); }
                let target = if gallery.step == 9 { gallery.first } else { gallery.second };
                aim_point(world, Vec3::from_array(target.map(|v| v as f32 + 0.5)));
                if world.resource::<TargetBlock>().0.is_none_or(|hit| hit.position != target) { return Ok(false); }
                action(world, GameAction::Break, true);
                gallery.release = Some(GameAction::Break);
            }
            10 => {
                if !world.resource::<NpcWorkView>().selection_active { return Err("The first corner completed the work area too early".into()); }
                unchanged(world, &gallery)?;
            }
            12 => {
                if !world.resource::<NpcWorkView>().direct { return Ok(false); }
                let mut expected = terra_core::npc_jobs::WorkArea::new(gallery.first, gallery.second).map_err(|error| format!("Invalid fixture area: {error:?}"))?;
                // Collector/Farmer/Lumberjack selections include the cell above
                // the picked surface, where actual drops/crops/roots live.
                expected.max[1] += 1;
                let view = world.resource::<NpcWorkView>();
                let actual = area(view);
                let reference = [expected.min[0], expected.min[1], expected.min[2], expected.max[0], expected.max[1], expected.max[2]].map(|v| v.to_string());
                if actual != reference { return Err(format!("Two actual corner picks yielded the wrong area: {actual:?}/{reference:?}")); }
                unchanged(world, &gallery)?;
                if !reveal(world, "npc-work-pick-storage")? { return Ok(false); }
                crate::menu_smoke::emit(world, "npc-work-pick-storage", None)?;
                resize(world, 960, 640, GameLanguage::English);
            }
            13 => {
                if !world.resource::<NpcWorkView>().selection_active { return Ok(false); }
                shot(world, &mut gallery, "npc-selection-en-compact", true)?;
            }
            14 => {
                aim_point(world, Vec3::from_array(gallery.first.map(|v| v as f32 + 0.5)));
                if world.resource::<TargetBlock>().0.is_none_or(|hit| hit.position != gallery.first) { return Ok(false); }
                action(world, GameAction::Break, true);
                gallery.release = Some(GameAction::Break);
            }
            15 => {
                if !world.resource::<NpcWorkView>().direct { return Ok(false); }
                let view = world.resource::<NpcWorkView>();
                if [view.storage_x.clone(), view.storage_y.clone(), view.storage_z.clone()] != gallery.first.map(|v| v.to_string()) {
                    return Err("Real storage selection did not retain the acknowledged chest position".into());
                }
                unchanged(world, &gallery)?;
                gallery.case = 0;
            }
            16 => {
                let cases = [(1600, 900, GameLanguage::English), (960, 640, GameLanguage::German), (960, 640, GameLanguage::English)];
                let (width, height, language) = cases[gallery.case];
                resize(world, width, height, language);
            }
            17 => {
                if !reveal(world, "npc-work-assign")? { return Ok(false); }
                let name = ["npc-direct-en-wide", "npc-direct-de-compact", "npc-direct-en-compact"][gallery.case];
                shot(world, &mut gallery, name, false)?;
                gallery.case += 1;
                if gallery.case < 3 { gallery.step = 15; }
            }
            18 => {
                if !world.resource::<NpcWorkView>().can_assign { return Err(format!("Direct selection draft cannot be assigned: {:?}", world.resource::<NpcWorkView>())); }
                crate::menu_smoke::emit(world, "npc-work-assign", None)?;
            }
            19 => {
                let view = world.resource::<NpcWorkView>();
                if view.pending || !view.has_order { return Ok(false); }
                if !view.error.is_empty() { return Err(view.error.clone()); }
                let worker = world.resource::<ClientSession>().work.workers.iter().find(|worker| Some(worker.npc) == gallery.npc).ok_or("Assigned worker absent from server work snapshot")?;
                if worker.order.as_ref().is_none_or(|order| order.kind != terra_core::npc_jobs::WorkKind::Collector || order.storage != gallery.first) {
                    return Err("Direct assignment was not confirmed by the server with the selected storage".into());
                }
                crate::menu_smoke::emit(world, "npc-work-enabled", Some("false"))?;
            }
            20 => {
                let view = world.resource::<NpcWorkView>();
                if view.pending || view.enabled { return Ok(false); }
                if !view.error.is_empty() { return Err(view.error.clone()); }
                if !reveal(world, "npc-work-assign")? { return Ok(false); }
                shot(world, &mut gallery, "npc-direct-en-compact-paused", false)?;
            }
            21 => {
                gallery.sex = gallery.npc.and_then(|npc| world.resource::<ReplicatedCreatures>().creatures.get(&npc)).map(|track| track.current.sex());
                gallery.original_avatar = world.query::<(Entity, &terra_render::creatures::CreatureAvatar)>().iter(world)
                    .find_map(|(entity, avatar)| (Some(avatar.0) == gallery.npc).then_some(entity));
                if gallery.original_avatar.is_none() { return Ok(false); }
                crate::menu_smoke::emit(world, "npc-work-field-kind", Some("lumberjack"))?;
            }
            22 => { crate::menu_smoke::emit(world, "npc-work-assign", None)?; }
            23 => {
                if world.resource::<NpcWorkView>().pending { return Ok(false); }
                if !world.resource::<NpcWorkView>().error.is_empty() { return Err(world.resource::<NpcWorkView>().error.clone()); }
                let worker = world.resource::<ClientSession>().work.workers.iter().find(|worker| Some(worker.npc) == gallery.npc).ok_or("Missing assigned lumberjack")?;
                if worker.order.as_ref().is_none_or(|order| order.kind != terra_core::npc_jobs::WorkKind::Lumberjack)
                    || worker.status != terra_core::npc_jobs::WorkStatus::Waiting(terra_core::npc_jobs::WaitReason::MissingStation) { return Ok(false); }
                let track = gallery.npc.and_then(|npc| world.resource::<ReplicatedCreatures>().creatures.get(&npc)).ok_or("Missing replicated lumberjack")?;
                if track.current.variant != terra_core::creatures::villagers::UNEMPLOYED { return Err("Lumberjack skin changed before a station existed".into()); }
                shot(world, &mut gallery, "npc-lumberjack-missing-station-compact", false)?;
            }
            24 => {
                world.write_message(terra_ui::NpcWorkUiAction::Close);
                world.write_message(CaptureRequest(true));
                gallery.station = Some(crate::inventory_benchmark::chest::ChestFixture::for_material(terra_core::profession_stations::LUMBERJACK));
            }
            25 => {
                if !gallery.station.as_mut().ok_or("Missing station fixture")?.prepare(world)? { return Ok(false); }
            }
            26 => {
                let Some(track) = gallery.npc.and_then(|npc| world.resource::<ReplicatedCreatures>().creatures.get(&npc)) else { return Ok(false); };
                if track.current.variant != terra_core::creatures::villagers::LUMBERJACK { return Ok(false); }
                if Some(track.current.sex()) != gallery.sex { return Err("Profession change altered the NPC sex".into()); }
                let target = track.position() + Vec3::Y;
                let avatar = world.query::<(Entity, &terra_render::creatures::CreatureAvatar)>().iter(world)
                    .find_map(|(entity, avatar)| (Some(avatar.0) == gallery.npc).then_some(entity));
                if avatar.is_none() || avatar == gallery.original_avatar { return Ok(false); }
                aim_point(world, target);
            }
            27 => {
                capture(world, &mut gallery, "npc-lumberjack-profession-compact")?;
                info!(npc=?gallery.npc, sex=?gallery.sex, "Native lumberjack station passed: missing-table status, real table placement, physical visit and refreshed profession avatar preserve identity and sex");
            }
            _ => {
                if gallery.captures < 9 { return Ok(false); }
                world.resource_mut::<GameplayConfig>().language = gallery.language;
                info!(npc=?gallery.npc, captures=gallery.captures, fallback_spawned=gallery.fallback_spawned,
                    "Native direct NPC work passed: real RMB target, private inventory, cancel and actual area/storage picks without edits, server assignment/pause, DE/EN at two viewport sizes");
                return Ok(true);
            }
        }
        gallery.step += 1;
        gallery.since = Instant::now();
        Ok(false)
    })
}
