//! Opt-in native acceptance through real bindings, HTML handlers and UDP authority.
//! This driver never writes camera-mode/UI state, player poses or building replicas.
use bevy::{
    input::{ButtonState, mouse::MouseButtonInput},
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    ui::UiGlobalTransform,
    window::{CursorMoved, PrimaryWindow, WindowEvent},
};
use std::{
    path::PathBuf,
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};
use terra_core::{
    GameAction, GameLanguage, GameplayConfig, InputBinding,
    building_jobs::BuildArea,
    kingdom::{ClaimKind, KingdomId},
    work_areas::ConstructionAreaDisplay,
};
use terra_network::{
    client::{ClientSession, ReplicatedCreatures},
    protocol::building_jobs::BuildingAction,
    protocol::kingdom::{KingdomAction, KingdomOutcome},
};
use terra_player::{ConstructionCamera, PlayerBody, TerraCamera};
use terra_ui::{
    BuildModeView, ElementId, KingdomResourcesView, KingdomView, NpcWorkView, WorkAreasView,
};
use terra_world::streaming::ChunkStore;

const COTTAGE: &str = "spruce-cottage-v1";
const STORAGE: BuildArea = BuildArea {
    min: [2, 12, 18],
    max: [5, 14, 21],
};
const RESIDENTIAL: BuildArea = BuildArea {
    min: [17, 12, 17],
    max: [30, 12, 30],
};

pub(crate) fn requested() -> bool {
    std::env::var("TERRA_CONSTRUCTION_MODE_GALLERY").as_deref() == Ok("1")
}

#[derive(Resource, Default)]
struct Cursor(Option<Vec2>);
#[derive(Resource)]
struct Gallery {
    started: Instant,
    since: Instant,
    step: u8,
    kingdom: KingdomId,
    key_release: Vec<KeyCode>,
    mouse_release: bool,
    right_release: bool,
    claim: u64,
    area_request: u64,
    origin: [i64; 3],
    npc: u64,
    job: u64,
    authority: Vec3,
    prediction: Vec3,
    camera: Transform,
    saved: bool,
    save: Option<Mutex<mpsc::Receiver<Result<(), String>>>>,
    preview_acknowledged: bool,
    capture_pending: bool,
    capture_failed: bool,
    captures: usize,
    overlay_entities: Vec<Entity>,
    overlay_pulse: f32,
    overlay_drift: [f32; 2],
    language: GameLanguage,
}
impl Gallery {
    fn advance(&mut self) {
        self.step += 1;
        self.since = Instant::now();
    }
}

fn window(world: &mut World) -> Result<Entity, String> {
    world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("Missing construction window".into())
}

/// Keep fixture coordinates inside ECS, without moving the desktop pointer.
fn maintain_cursor(world: &mut World) {
    let Some(point) = world.resource::<Cursor>().0 else {
        return;
    };
    let Ok(entity) = window(world) else {
        return;
    };
    let Some(mut viewport) = world.get_mut::<Window>(entity) else {
        return;
    };
    let delta = viewport.cursor_position().map(|old| point - old);
    viewport
        .bypass_change_detection()
        .set_cursor_position(Some(point));
    let event = CursorMoved {
        window: entity,
        position: point,
        delta,
    };
    world.write_message(event.clone());
    world.write_message(WindowEvent::CursorMoved(event));
}
fn cursor(world: &mut World, point: Vec2) {
    world.resource_mut::<Cursor>().0 = Some(point);
    maintain_cursor(world);
}
fn mouse(world: &mut World, pressed: bool) -> Result<(), String> {
    let window = window(world)?;
    world.write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        window,
    });
    Ok(())
}
fn right_mouse(world: &mut World, pressed: bool) -> Result<(), String> {
    let window = window(world)?;
    world.write_message(MouseButtonInput {
        button: MouseButton::Right,
        state: if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        window,
    });
    Ok(())
}
fn pulse(world: &mut World, gallery: &mut Gallery, action: GameAction) -> Result<(), String> {
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(|| action.default_binding(), |config| config.binding(action));
    let keys = match binding {
        InputBinding::Key(key) => vec![key],
        InputBinding::Chord { modifier, key } => vec![modifier, key],
        InputBinding::Mouse(_) => {
            return Err(format!(
                "Native construction expects keyboard binding for {}",
                action.label()
            ));
        }
    };
    for key in &keys {
        crate::minimap_smoke::key_event(world, *key, true);
    }
    gallery.key_release.extend(keys);
    Ok(())
}
fn element(world: &mut World, id: &str) -> Option<Entity> {
    world
        .query::<(Entity, &ElementId, &ComputedNode)>()
        .iter(world)
        .find_map(|(entity, name, node)| {
            (name.0 == id && node.size().min_element() > 0.0).then_some(entity)
        })
}
fn rect(world: &World, entity: Entity) -> Option<Rect> {
    let node = world.get::<ComputedNode>(entity)?;
    let pose = world.get::<UiGlobalTransform>(entity)?;
    let half = node.size() * 0.5;
    let (low, high) = [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(-half.x, half.y),
        Vec2::new(half.x, half.y),
    ]
    .into_iter()
    .map(|point| pose.transform_point2(point))
    .fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(low, high), point| (low.min(point), high.max(point)),
    );
    Some(Rect::from_corners(low, high))
}
fn contains(outer: Rect, inner: Rect) -> bool {
    (inner.min + Vec2::splat(2.0)).cmpge(outer.min).all()
        && (inner.max - Vec2::splat(2.0)).cmple(outer.max).all()
}
fn descendant(world: &World, mut entity: Entity, root: Entity) -> bool {
    for _ in 0..48 {
        if entity == root {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}
/// Physical content width excludes border, padding and the live scrollbar gutter.
fn content_width(world: &World, entity: Entity) -> Option<(f32, f32)> {
    let node = world.get::<ComputedNode>(entity)?;
    let pose = world.get::<UiGlobalTransform>(entity)?;
    let inset = node.content_inset();
    let half = node.size().x * 0.5;
    let left = pose
        .transform_point2(Vec2::new(-half + inset.min_inset.x, 0.0))
        .x;
    let right = pose
        .transform_point2(Vec2::new(half - inset.max_inset.x, 0.0))
        .x;
    Some((left.min(right), left.max(right)))
}

fn bounds(world: &mut World) -> Result<(), String> {
    let entity = window(world)?;
    let viewport = Rect::from_corners(
        Vec2::ZERO,
        world
            .get::<Window>(entity)
            .ok_or("Missing window layout")?
            .resolution
            .physical_size()
            .as_vec2(),
    );
    for id in [
        "kingdom-resources",
        "build-mode-toolbar",
        "build-mode-panel",
        "build-mode-costs",
        "work-area-overlay",
    ] {
        if let Some(entity) = element(world, id) {
            let geometry = rect(world, entity).ok_or_else(|| format!("Missing layout {id}"))?;
            if !contains(viewport, geometry) {
                return Err(format!(
                    "Construction UI leaves viewport: {id}/{geometry:?}/{viewport:?}"
                ));
            }
        }
    }
    let buttons = world
        .query_filtered::<(Entity, &ElementId, &ComputedNode), With<tilt_ui::TiltButton>>()
        .iter(world)
        .filter(|(_, id, node)| id.0.starts_with("build-mode-") && node.size().min_element() > 0.0)
        .map(|(entity, id, _)| (entity, id.0.clone()))
        .collect::<Vec<_>>();
    if buttons.is_empty() {
        return Err("No rendered construction buttons for caption validation".into());
    }
    let texts = world
        .query::<(Entity, &Text)>()
        .iter(world)
        .filter(|(_, text)| !text.0.trim().is_empty())
        .map(|(entity, text)| (entity, text.0.clone()))
        .collect::<Vec<_>>();
    for (button, id) in buttons {
        let geometry = rect(world, button).ok_or("Missing button geometry")?;
        for (entity, text) in &texts {
            if descendant(world, *entity, button)
                && let Some(caption) = rect(world, *entity)
                && !contains(geometry, caption)
            {
                return Err(format!(
                    "Construction caption escapes button: {id}/{text:?}/{caption:?}/{geometry:?}"
                ));
            }
        }
    }
    if let Some(card) = element(world, "build-mode-costs") {
        let (card_left, card_right) =
            content_width(world, card).ok_or("Missing costs content width")?;
        let elements = world
            .query::<(Entity, &tilt_ui::ElementClasses, &ComputedNode)>()
            .iter(world)
            .filter(|(entity, _, node)| !node.is_empty() && descendant(world, *entity, card))
            .map(|(entity, classes, _)| (entity, classes.classes.clone()))
            .collect::<Vec<_>>();
        let scroll = elements
            .iter()
            .find(|(_, classes)| classes.iter().any(|class| class == "build-mode-materials"))
            .map(|(entity, _)| *entity)
            .ok_or("Missing actual material scroll viewport")?;
        let (scroll_left, scroll_right) =
            content_width(world, scroll).ok_or("Missing scroll content width")?;
        let left = card_left.max(scroll_left);
        let right = card_right.min(scroll_right);
        let mut checked = 0;
        for (row, classes) in &elements {
            if !classes.iter().any(|class| class == "build-mode-material") {
                continue;
            }
            let geometry = rect(world, *row).ok_or("Missing material row layout")?;
            if geometry.min.x < left - 2.0 || geometry.max.x > right + 2.0 {
                return Err(format!(
                    "Construction material row escapes costs/scroll content width: {geometry:?}/{left}..{right}"
                ));
            }
            for (entity, text) in &texts {
                if !descendant(world, *entity, *row) {
                    continue;
                }
                let Some(caption) = rect(world, *entity) else {
                    continue;
                };
                if caption.min.x < geometry.min.x - 2.0
                    || caption.max.x > geometry.max.x + 2.0
                    || caption.min.x < left - 2.0
                    || caption.max.x > right + 2.0
                {
                    return Err(format!(
                        "Construction material caption is clipped by row/scrollbar: {text:?}/{caption:?}/{left}..{right}"
                    ));
                }
                checked += 1;
            }
        }
        if checked == 0 {
            return Err("No actual material names/counts available for width validation".into());
        }
    }
    Ok(())
}
/// The central lower viewport excludes the catalog, costs, minimap and kingdom
/// toast. A colorful UI around an otherwise empty fog/sky frame is insufficient.
fn terrain_samples(data: &[u8], width: usize, height: usize, bgra: bool) -> (usize, usize) {
    let mut sampled = 0;
    let mut terrain = 0;
    for y in (height * 52 / 100..height * 72 / 100).step_by(3) {
        for x in (width * 38 / 100..width * 65 / 100).step_by(3) {
            let Some(pixel) = data.get((y * width + x) * 4..(y * width + x) * 4 + 4) else {
                continue;
            };
            let [red, green, blue] = if bgra {
                [pixel[2], pixel[1], pixel[0]]
            } else {
                [pixel[0], pixel[1], pixel[2]]
            };
            sampled += 1;
            // Grass, soil and the orange silhouette have warm/green reflectance.
            // The uniform blue fog visible in the failed native frame does not.
            if u16::from(red.max(green)) > u16::from(blue) + 12 {
                terrain += 1;
            }
        }
    }
    (sampled, terrain)
}

fn capture(
    world: &mut World,
    gallery: &mut Gallery,
    name: &str,
    validate: bool,
) -> Result<(), String> {
    if validate {
        bounds(world)?;
    }
    let directory =
        PathBuf::from(std::env::var_os("TERRA_CAPTURE_DIR").ok_or("Set TERRA_CAPTURE_DIR")?);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let frame_name = name.to_owned();
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("{name}.png"))))
        .observe(
            move |event: On<ScreenshotCaptured>, mut gallery: ResMut<Gallery>| {
                let colors = event
                    .image
                    .data
                    .as_deref()
                    .unwrap_or_default()
                    .chunks_exact(4)
                    .step_by(113)
                    .map(|pixel| [pixel[0], pixel[1], pixel[2]])
                    .collect::<std::collections::HashSet<_>>();
                let bgra = matches!(
                    event.image.texture_descriptor.format,
                    bevy::render::render_resource::TextureFormat::Bgra8Unorm
                        | bevy::render::render_resource::TextureFormat::Bgra8UnormSrgb
                );
                let (sampled, terrain) = terrain_samples(
                    event.image.data.as_deref().unwrap_or_default(),
                    event.image.texture_descriptor.size.width as usize,
                    event.image.texture_descriptor.size.height as usize,
                    bgra,
                );
                let visible = sampled > 0 && terrain * 100 >= sampled;
                info!(
                    frame = frame_name,
                    sampled, terrain, visible, "Native construction world crop visibility"
                );
                gallery.capture_failed |= colors.len() < 16 || validate && !visible;
                gallery.capture_pending = false;
                gallery.captures += 1;
            },
        );
    gallery.capture_pending = true;
    Ok(())
}
fn camera_pose(world: &mut World) -> Option<Transform> {
    world
        .query_filtered::<&Transform, With<TerraCamera>>()
        .iter(world)
        .next()
        .copied()
}
fn avatar(world: &mut World) -> Result<(Vec3, Vec3), String> {
    let authority = Vec3::from_array(
        world
            .resource::<ClientSession>()
            .confirmed_state()
            .ok_or("Missing confirmed avatar")?
            .position,
    );
    let prediction = world
        .query_filtered::<&Transform, With<PlayerBody>>()
        .iter(world)
        .next()
        .ok_or("Missing actual avatar body")?
        .translation;
    Ok((authority, prediction))
}
fn stationary(world: &mut World, gallery: &Gallery) -> Result<(), String> {
    if !gallery.saved {
        return Ok(());
    }
    let (authority, prediction) = avatar(world)?;
    if authority.distance(gallery.authority) > 0.12
        || prediction.distance(gallery.prediction) > 0.12
    {
        return Err(format!(
            "Detached construction moved the avatar: authority={authority:?}/{:?}, prediction={prediction:?}/{:?}",
            gallery.authority, gallery.prediction
        ));
    }
    Ok(())
}
fn viewport_point(world: &mut World, point: Vec3) -> Option<Vec2> {
    world
        .query_filtered::<(&Camera, &GlobalTransform), With<TerraCamera>>()
        .iter(world)
        .next()
        .and_then(|(camera, pose)| camera.world_to_viewport(pose, point).ok())
}
fn unobstructed_pointer(world: &mut World, point: Vec2) -> bool {
    let Ok(entity) = window(world) else {
        return false;
    };
    let Some(viewport) = world.get::<Window>(entity) else {
        return false;
    };
    if point.cmplt(Vec2::splat(4.0)).any()
        || point
            .cmpgt(Vec2::new(viewport.width() - 4.0, viewport.height() - 4.0))
            .any()
    {
        return false;
    }
    let point = point * viewport.resolution.scale_factor();
    !world
        .query::<(&ElementId, &ComputedNode, &UiGlobalTransform)>()
        .iter(world)
        .any(|(id, node, pose)| {
            matches!(
                id.0.as_str(),
                "build-mode-panel"
                    | "build-mode-toolbar"
                    | "build-mode-costs"
                    | "mini-map"
                    | "kingdom-resources"
                    | "work-area-overlay"
            ) && !node.is_empty()
                && node.contains_point(*pose, point)
        })
}
fn select_cottage(world: &mut World) -> Result<(), String> {
    let mut buttons = world
        .query::<(Entity, &ElementId, &ComputedNode)>()
        .iter(world)
        .filter(|(_, id, node)| id.0 == "build-mode-template" && node.size().min_element() > 0.0)
        .filter_map(|(entity, _, _)| rect(world, entity).map(|bounds| (entity, bounds.min.y)))
        .collect::<Vec<_>>();
    buttons.sort_by(|a, b| a.1.total_cmp(&b.1));
    if buttons.len() < 2 {
        return Err("Actual construction catalog did not render both template cards".into());
    }
    let index = world
        .resource::<BuildModeView>()
        .templates
        .iter()
        .position(|template| template.id == COTTAGE)
        .ok_or("Cottage is absent from the real visible catalog")?;
    let entity = buttons
        .get(index)
        .ok_or("Visible cottage card is absent")?
        .0;
    if !world
        .get::<tilt_ui::StaticAttributes>(entity)
        .is_some_and(|attributes| {
            attributes
                .attributes
                .iter()
                .any(|a| a.name == "onclick" && a.value.contains("build_mode_action"))
        })
    {
        return Err("Cottage template card has no actual build_mode_action HTML handler".into());
    }
    if world
        .resource_mut::<tilt_ui::UiBindingStore>()
        .set_json_path(
            "buildModeIntent.template",
            serde_json::Value::String(COTTAGE.into()),
        )
        .is_none()
    {
        return Err("Missing mutable construction template intent alias".into());
    }
    // The public mutable intent is precisely the value the card's inline $set
    // supplies; execute its registered component handler against that real card.
    world.write_message(tilt_ui::HtmlEvent {
        target: entity,
        kind: "click",
        value: None,
        submitter: None,
        data: Default::default(),
        form_data: None,
        handler: "build_mode_action".into(),
    });
    Ok(())
}
fn stock(world: &mut World) -> Result<bool, String> {
    let view = world.resource::<KingdomResourcesView>();
    if !view.visible || view.rows.len() != 4 || view.rows.iter().any(|row| row.amount != "∞") {
        return Ok(false);
    }
    let session = world.resource::<ClientSession>();
    if !session.buildings.unlimited_resources
        || session.buildings.stock.configured
        || session.game_mode != terra_core::GameMode::Creative
    {
        return Err(
            "Construction acceptance expected acknowledged Creative stock without a town chest"
                .into(),
        );
    }
    Ok(element(world, "kingdom-resources").is_some())
}
fn ground(world: &World) -> Option<[i64; 3]> {
    let chunks = world.resource::<ChunkStore>();
    (5..24).rev().find_map(|y| {
        (chunks.block_at([24, y, 24]).is_some_and(|material| {
            terra_voxel::MaterialId::from_raw(material)
                .is_some_and(terra_voxel::MaterialId::is_solid)
        }) && chunks.block_at([24, y + 1, 24]) == Some(0))
        .then_some([24, y + 1, 24])
    })
}
fn choose_npc(world: &mut World) -> Option<(u64, Vec2)> {
    let session = world.resource::<ClientSession>();
    let candidates = world
        .resource::<ReplicatedCreatures>()
        .creatures
        .values()
        .filter_map(|track| {
            let npc = &track.current;
            (npc.kind == terra_core::creatures::CreatureKind::Villager
                && npc.health > 0.0
                && terra_core::creatures::villagers::is_civilian(npc.variant)
                && session
                    .work
                    .workers
                    .iter()
                    .any(|worker| worker.npc == npc.id))
            .then_some((
                npc.id,
                Vec3::from_array(npc.position.map(|p| p as f32)) + Vec3::Y * 1.2,
            ))
        })
        .collect::<Vec<_>>();
    candidates.into_iter().find_map(|(id, position)| {
        viewport_point(world, position)
            .filter(|point| unobstructed_pointer(world, *point))
            .map(|point| (id, point))
    })
}

fn save_ready(gallery: &mut Gallery) -> Result<bool, String> {
    let result = gallery
        .save
        .as_ref()
        .ok_or("Missing actual save request receiver")?
        .lock()
        .map_err(|_| "Construction save receiver was poisoned")?
        .try_recv();
    match result {
        Ok(result) => {
            result?;
            gallery.save = None;
            Ok(true)
        }
        Err(mpsc::TryRecvError::Empty) => Ok(false),
        Err(mpsc::TryRecvError::Disconnected) => {
            Err("Construction save service disconnected".into())
        }
    }
}

fn verify_saved_job(world: &World, gallery: &Gallery) -> Result<(), String> {
    let directory = PathBuf::from(
        std::env::var_os("TERRA_KINGDOM_SAVE_DIR")
            .ok_or("Missing isolated construction save directory")?,
    );
    if !directory.join("level.data").is_file() {
        return Err("Acknowledged save did not write the real level.data".into());
    }
    let journal = terra_storage::load_edits(
        &directory,
        world.resource::<terra_core::GameConfig>().world.id,
    )?;
    let game = journal.gameplay();
    if !game.valid() {
        return Err("Reloaded construction GameplayState is invalid".into());
    }
    let owner = world
        .resource::<ClientSession>()
        .uuid
        .ok_or("Missing actual construction owner")?;
    let job = game
        .buildings
        .jobs
        .get(&gallery.job)
        .ok_or("Real saved construction job is absent after reload")?;
    if job.id != gallery.job
        || job.kingdom != gallery.kingdom
        || job.owner != owner
        || job.template != COTTAGE
        || job.origin != gallery.origin
        || !job.free_resources
    {
        return Err(format!(
            "Reloaded construction job changed identity/template/site/Creative policy: {job:?}"
        ));
    }
    let settlement = game
        .buildings
        .settlements
        .get(&gallery.kingdom)
        .ok_or("Real saved construction job has no persisted owner settlement")?;
    if settlement.owner != owner || settlement.owner != job.owner || settlement.storage.is_some() {
        return Err(format!(
            "Reloaded Creative settlement changed ownership or invented storage: {settlement:?}"
        ));
    }
    Ok(())
}

fn request_area(
    world: &mut World,
    gallery: &mut Gallery,
    action: BuildingAction,
) -> Result<(), String> {
    let request = world
        .resource_mut::<ClientSession>()
        .make_building_request(action)
        .ok_or("Native saved-area request was unavailable")?;
    gallery.area_request = request.0.request;
    world.write_message(request);
    Ok(())
}

fn check_area_result(world: &World, gallery: &Gallery) -> Result<(), String> {
    if let Some(result) = world
        .resource::<ClientSession>()
        .last_building_result
        .as_ref()
        && result.request == gallery.area_request
        && let Some(error) = result.error
    {
        return Err(format!(
            "Authoritative saved-area fixture rejected: {error:?}"
        ));
    }
    Ok(())
}

fn visible_overlay_entities(world: &mut World) -> Vec<Entity> {
    let mut entities = world
        .query::<(Entity, &Name, &Mesh3d, &Visibility)>()
        .iter(world)
        .filter(|(_, name, _, visibility)| {
            (name.as_str().starts_with("Saved work area contour:")
                || name.as_str() == "Build chunk border contour")
                && **visibility != Visibility::Hidden
        })
        .map(|(entity, _, _, _)| entity)
        .collect::<Vec<_>>();
    entities.sort_unstable();
    entities
}

fn automatic_areas_ready(world: &mut World, gallery: &Gallery) -> Result<bool, String> {
    let display = world.resource::<ConstructionAreaDisplay>();
    let view = world.resource::<WorkAreasView>();
    if display.kingdom != Some(gallery.kingdom)
        || !display.visible
        || !world.resource::<BuildModeView>().areas_visible
        || !view.visible
        || !view.available
        || !["storage", "residential", "build-chunks"]
            .into_iter()
            .all(|kind| view.items.iter().any(|row| row.kind == kind))
    {
        return Ok(false);
    }
    let status = world.resource::<terra_render::WorkAreaOverlayStatus>();
    if !status.visible
        || status.area_segments == 0
        || status.build_segments == 0
        || status.vertices == 0
    {
        return Ok(false);
    }
    if !status.pulse.is_finite() {
        return Err("Area-border animation produced a non-finite pulse".into());
    }
    let names = world
        .query::<(&Name, &Mesh3d, &Visibility)>()
        .iter(world)
        .filter(|(_, _, visibility)| **visibility != Visibility::Hidden)
        .map(|(name, _, _)| name.as_str().to_owned())
        .collect::<std::collections::HashSet<_>>();
    Ok([
        "Saved work area contour: storage",
        "Saved work area contour: residential",
        "Build chunk border contour",
    ]
    .into_iter()
    .all(|name| names.contains(name)))
}

fn verify_saved_areas(world: &World, gallery: &Gallery) -> Result<(), String> {
    let path = std::env::var_os("TERRA_KINGDOM_SAVE_DIR")
        .map(PathBuf::from)
        .ok_or("Missing isolated area save directory")?;
    let journal =
        terra_storage::load_edits(&path, world.resource::<terra_core::GameConfig>().world.id)?;
    let game = journal.gameplay();
    let site = game
        .buildings
        .settlements
        .get(&gallery.kingdom)
        .ok_or("Actual saved settlement is absent after area setup")?;
    if !game.valid()
        || site.storage != Some(STORAGE)
        || site.residential != Some(RESIDENTIAL)
        || !game.buildings.jobs.contains_key(&gallery.job)
    {
        return Err(
            "Actual saved storage/residential areas or original construction job changed on disk"
                .into(),
        );
    }
    Ok(())
}

/// Called after the existing native fixture receives the real founding response.
pub(crate) fn run(world: &mut World) -> Result<bool, String> {
    if !world.contains_resource::<Gallery>() {
        let session = world.resource::<ClientSession>();
        let record = session
            .uuid
            .and_then(|id| session.kingdoms.active_for_member(id))
            .ok_or("Construction acceptance requires a real founded kingdom")?;
        let kingdom = record.id;
        let language = world.resource::<GameplayConfig>().language;
        world.init_resource::<Cursor>();
        world
            .resource_mut::<Schedules>()
            .get_mut(First)
            .ok_or("Missing native input schedule")?
            .add_systems(maintain_cursor.in_set(bevy::picking::PickingSystems::PostInput));
        world.insert_resource(Gallery {
            started: Instant::now(),
            since: Instant::now(),
            step: 0,
            kingdom,
            key_release: Vec::new(),
            mouse_release: false,
            right_release: false,
            claim: 0,
            area_request: 0,
            origin: [0; 3],
            npc: 0,
            job: 0,
            authority: Vec3::ZERO,
            prediction: Vec3::ZERO,
            camera: Transform::default(),
            saved: false,
            save: None,
            preview_acknowledged: false,
            capture_pending: false,
            capture_failed: false,
            captures: 0,
            overlay_entities: Vec::new(),
            overlay_pulse: 0.0,
            overlay_drift: [0.0; 2],
            language,
        });
    }
    world.resource_scope(|world,mut gallery:Mut<Gallery>| {
        for key in gallery.key_release.drain(..) {crate::minimap_smoke::key_event(world,key,false);}
        if gallery.mouse_release {mouse(world,false)?; gallery.mouse_release=false;}
        if gallery.right_release {right_mouse(world,false)?; gallery.right_release=false;}
        if gallery.started.elapsed()>Duration::from_secs(120) {return Err(format!("Construction mode timed out at step {}: {:?}; NPC={:?}; camera={:?}",gallery.step,world.resource::<BuildModeView>(),world.resource::<NpcWorkView>(),world.resource::<ConstructionCamera>()));}
        gallery.preview_acknowledged |= world.resource::<ClientSession>().buildings.preview.as_ref().is_some_and(|preview|preview.id==COTTAGE);
        if gallery.capture_failed {return Err("Native construction screenshot contains no rendered scene/UI".into());}
        stationary(world,&gallery)?;
        if gallery.capture_pending || gallery.since.elapsed()<Duration::from_millis(400) {return Ok(false);}
        match gallery.step {
            0 => {
                world.resource_mut::<GameplayConfig>().language=GameLanguage::German;
                crate::menu_smoke::emit(world,"kingdom-close",None)?;
            }
            1 => {
                if world.resource::<KingdomView>().visible {return Ok(false);}
                let state=world.resource::<ClientSession>().confirmed_state().ok_or("Missing real avatar state")?;
                if Vec3::from_array(state.velocity).length()>0.08 {return Ok(false);}
                (gallery.authority,gallery.prediction)=avatar(world)?;
                gallery.camera=camera_pose(world).ok_or("Missing gameplay camera")?;
                gallery.saved=true;
                pulse(world,&mut gallery,GameAction::BuildMode)?;
            }
            2 => {
                if !world.resource::<ConstructionCamera>().active || !world.resource::<BuildModeView>().catalog || !stock(world)? {return Ok(false);}
                if world.resource::<BuildModeView>().templates.len()<2 || element(world,"build-mode-panel").is_none() {return Ok(false);}
                let pose=camera_pose(world).ok_or("Missing detached camera")?;
                if pose.translation.y<gallery.camera.translation.y+20.0 || (pose.rotation*Vec3::NEG_Z).y> -0.65 {return Err("TAB did not create the actual elevated top-down camera".into());}
                capture(world,&mut gallery,"construction-catalog-de-wide",true)?;
                info!("Native construction mode TAB and Creative four-resource HUD acknowledged");
            }
            3 => {select_cottage(world)?;}
            4 => {
                let view=world.resource::<BuildModeView>();
                if !view.placing || view.catalog || view.template_id!=COTTAGE || !view.preview_ready {return Ok(false);}
                if view.materials.is_empty() || view.materials.iter().any(|row|!row.sufficient||!row.supply.starts_with('∞')) {return Ok(false);}
                info!(transient_preview_observed=gallery.preview_acknowledged,"Native construction cached server preview readiness acknowledged");
                let request=world.resource_mut::<ClientSession>().kingdom_request(KingdomAction::ClaimChunk {id:gallery.kingdom,chunk:[1,1],kind:ClaimKind::ProtectedBuilding}).ok_or("Build claim request was unavailable")?;
                gallery.claim=request.0.request;
                world.write_message(request);
            }
            5 => {
                let session=world.resource::<ClientSession>();
                if let Some(result)=session.last_kingdom_result.as_ref().filter(|result|result.request==gallery.claim)
                    && let KingdomOutcome::Rejected(error)=&result.outcome {return Err(format!("Actual construction claim rejected: {error:?}"));}
                if !session.kingdoms.claim_at([1,1]).is_some_and(|claim|claim.kingdom==gallery.kingdom&&claim.kind==ClaimKind::ProtectedBuilding) {return Ok(false);}
                // A placing template intentionally ignores NPC clicks. Cancel
                // through real right-button input before testing management.
                if world.resource::<BuildModeView>().placing {
                    let center=world.resource::<ConstructionCamera>().center;
                    let point=viewport_point(world,center)
                        .filter(|point|unobstructed_pointer(world,*point))
                        .ok_or("Construction cancel needs an unobstructed world pointer")?;
                    cursor(world,point);
                    right_mouse(world,true)?;
                    gallery.right_release=true;
                    gallery.since=Instant::now();
                    return Ok(false);
                }
                let Some((npc,point))=choose_npc(world) else{return Ok(false);};
                gallery.npc=npc; cursor(world,point);
            }
            6 => {
                if let Some((_,point))=choose_npc(world) {cursor(world,point);}
                mouse(world,true)?;gallery.mouse_release=true;
            }
            7 => {
                let view=world.resource::<NpcWorkView>();
                if !view.direct || !view.inventory_ready {return Ok(false);}
                let detail=world.resource::<ClientSession>().work.detail.as_ref().ok_or("NPC dialog showed no real private inventory response")?;
                gallery.npc=detail.npc;
                if view.private_inventory.len()!=terra_core::npc_jobs::WORKER_SLOTS {return Err("NPC management did not render its real nine-slot inventory".into());}
                if !world.resource::<ConstructionCamera>().active {return Err("Opening NPC dialog exited detached construction".into());}
                capture(world,&mut gallery,"construction-npc-de-wide",false)?;
                pulse(world,&mut gallery,GameAction::Pause)?;
                info!(npc=gallery.npc,"Native construction mode real NPC click and private inventory acknowledged");
            }
            8 => {
                if world.resource::<NpcWorkView>().direct {return Ok(false);}
                if !world.resource::<ConstructionCamera>().active {return Err("ESC closed construction instead of only its NPC dialog".into());}
                if !world.resource::<BuildModeView>().placing {
                    if !world.resource::<BuildModeView>().catalog {
                        crate::menu_smoke::emit(world,"build-mode-catalog",None)?;
                    } else {
                        if element(world,"build-mode-panel").is_none() {return Ok(false);}
                        select_cottage(world)?;
                    }
                    gallery.since=Instant::now();
                    return Ok(false);
                }
                if !world.resource::<BuildModeView>().preview_ready {return Ok(false);}
                crate::minimap_smoke::key_event(world,KeyCode::KeyD,true);
            }
            9 => {
                if world.resource::<ConstructionCamera>().center.x<23.0 {return Ok(false);}
                crate::minimap_smoke::key_event(world,KeyCode::KeyD,false);
            }
            10 => {
                let Some(origin)=ground(world) else{return Ok(false);};
                gallery.origin=origin;
                let target=Vec3::new(origin[0] as f32+0.5,origin[1] as f32+0.005,origin[2] as f32+0.5);
                let Some(point)=viewport_point(world,target).filter(|point|unobstructed_pointer(world,*point)) else{return Err("Actual cottage site is outside the unobstructed construction viewport".into());};
                cursor(world,point);
            }
            11 => {
                let location=format!("{} / {} / {}",gallery.origin[0],gallery.origin[1],gallery.origin[2]);
                if world.resource::<BuildModeView>().location!=location {return Ok(false);}
                let visible=world.query::<(&Name,&Mesh3d,&Visibility)>().iter(world).any(|(name,_,visibility)|name.as_str()=="Construction blueprint preview" && *visibility!=Visibility::Hidden);
                if !visible {return Ok(false);}
                capture(world,&mut gallery,"construction-blueprint-de-wide",true)?;
                info!(origin=?gallery.origin,"Native construction mode real orange server-template mesh and ground raycast acknowledged");
            }
            12 => {mouse(world,true)?;gallery.mouse_release=true;}
            13 => {
                let session=world.resource::<ClientSession>();
                let Some(job)=session.buildings.jobs.values().find(|job|job.kingdom==gallery.kingdom&&job.template==COTTAGE&&job.origin==gallery.origin) else {
                    if !world.resource::<BuildModeView>().pending && !world.resource::<BuildModeView>().error.is_empty() {return Err(world.resource::<BuildModeView>().error.clone());}
                    return Ok(false);
                };
                if !job.free_resources || !session.buildings.unlimited_resources || session.buildings.settlements.get(&gallery.kingdom).is_none_or(|town|town.storage.is_some()) {return Err("Creative construction did not retain authoritative free_resources without a chest".into());}
                gallery.job=job.id;
                for mut window in world.query::<&mut Window>().iter_mut(world) {window.resolution.set_physical_resolution(960,640);}
                world.resource_mut::<GameplayConfig>().language=GameLanguage::English;
                info!(job=gallery.job,template=COTTAGE,"Native construction mode actual cottage queue and Creative free_resources acknowledged");
            }
            14 => {
                if world.resource::<BuildModeView>().pending || !stock(world)? {return Ok(false);}
                capture(world,&mut gallery,"construction-queued-en-compact",true)?;
            }
            15 => {pulse(world,&mut gallery,GameAction::BuildMode)?;}
            16 => {
                if world.resource::<ConstructionCamera>().active || world.resource::<BuildModeView>().visible {return Ok(false);}
                let camera=camera_pose(world).ok_or("Missing restored gameplay camera")?;
                if camera.translation.distance(gallery.camera.translation)>0.15 || !camera.rotation.abs_diff_eq(gallery.camera.rotation,0.002) {return Err(format!("Construction exit did not restore camera: {camera:?}/{:?}",gallery.camera));}
                if gallery.captures!=4 {return Err(format!("Expected four construction captures, got {}",gallery.captures));}
                gallery.save=Some(Mutex::new(world.resource::<terra_server::IntegratedServer>().request_save()?));
            }
            17 => {
                if !save_ready(&mut gallery)? {return Ok(false);}
                verify_saved_job(world,&gallery)?;
                info!(job=gallery.job,kingdom=?gallery.kingdom,template=COTTAGE,"Native construction mode persisted Creative job and owner settlement acknowledged");
            }
            18 => {
                let kingdom=gallery.kingdom;
                request_area(world,&mut gallery,BuildingAction::SetStorage {kingdom,area:STORAGE})?;
            }
            19 => {
                check_area_result(world,&gallery)?;
                if world.resource::<ClientSession>().buildings.settlements.get(&gallery.kingdom).is_none_or(|site|site.storage!=Some(STORAGE)) {return Ok(false);}
                let kingdom=gallery.kingdom;
                request_area(world,&mut gallery,BuildingAction::SetResidential {kingdom,area:Some(RESIDENTIAL),template:COTTAGE.into()})?;
            }
            20 => {
                check_area_result(world,&gallery)?;
                if world.resource::<ClientSession>().buildings.settlements.get(&gallery.kingdom).is_none_or(|site|site.residential!=Some(RESIDENTIAL)) {return Ok(false);}
                for mut viewport in world.query::<&mut Window>().iter_mut(world) {viewport.resolution.set_physical_resolution(1600,900);}
                world.resource_mut::<GameplayConfig>().language=GameLanguage::German;
                pulse(world,&mut gallery,GameAction::BuildMode)?;
            }
            21 => {
                if !automatic_areas_ready(world,&gallery)? {return Ok(false);}
                // Close the real catalog so the actual drifting mist is unobstructed.
                crate::menu_smoke::emit(world,"build-mode-catalog",None)?;
            }
            22 => {
                if world.resource::<BuildModeView>().catalog || !automatic_areas_ready(world,&gallery)? {return Ok(false);}
                gallery.overlay_entities=visible_overlay_entities(world);
                gallery.overlay_pulse=world.resource::<terra_render::WorkAreaOverlayStatus>().pulse;
                gallery.overlay_drift=world.resource::<terra_render::WorkAreaOverlayStatus>().mist_drift;
                if gallery.overlay_entities.len()<3 {return Err("Saved work areas and red Build boundaries have no actual visible meshes".into());}
                capture(world,&mut gallery,"construction-areas-de-wide",true)?;
            }
            23 => {
                if world.resource::<ConstructionAreaDisplay>().visible {
                    crate::minimap_smoke::key_event(world,KeyCode::KeyH,true);
                    gallery.key_release.push(KeyCode::KeyH);
                    return Ok(false);
                }
                let display=world.resource::<ConstructionAreaDisplay>();
                if display.visible || world.resource::<BuildModeView>().areas_visible
                    || world.resource::<terra_render::WorkAreaOverlayStatus>().visible
                    || !visible_overlay_entities(world).is_empty() {return Ok(false);}
                if !world.resource::<ConstructionCamera>().active {return Err("H exited construction instead of hiding its areas".into());}
                capture(world,&mut gallery,"construction-areas-hidden-de-wide",true)?;
            }
            24 => {
                if !world.resource::<ConstructionAreaDisplay>().visible {
                    crate::minimap_smoke::key_event(world,KeyCode::KeyH,true);
                    gallery.key_release.push(KeyCode::KeyH);
                    return Ok(false);
                }
                if !automatic_areas_ready(world,&gallery)? {return Ok(false);}
                if visible_overlay_entities(world)!=gallery.overlay_entities {return Err("H hide/show unnecessarily replaced cached contour entities".into());}
                let animation_pulse=world.resource::<terra_render::WorkAreaOverlayStatus>().pulse;
                if (animation_pulse-gallery.overlay_pulse).abs()<0.0001 {return Ok(false);}
                let drift=world.resource::<terra_render::WorkAreaOverlayStatus>().mist_drift;
                if drift.iter().any(|value|!value.is_finite()) {return Err("Area mist uploaded nonfinite advection".into());}
                if Vec2::from_array(drift).distance(Vec2::from_array(gallery.overlay_drift))<0.1 {return Ok(false);}
                capture(world,&mut gallery,"construction-areas-animated-de-wide",true)?;
                info!(?drift,"Native construction spatial mist advection and second visible capture acknowledged");
                info!(areas=gallery.overlay_entities.len(),pulse=animation_pulse,"Native construction automatic saved areas, red Build boundaries and real H hide/show acknowledged");
            }
            25 => {
                // Wait for the visible screenshot observer before changing visibility.
                if world.resource::<ConstructionCamera>().active {
                    pulse(world,&mut gallery,GameAction::BuildMode)?;
                    return Ok(false);
                }
                if world.resource::<ConstructionCamera>().active || world.resource::<ConstructionAreaDisplay>().kingdom.is_some() {return Ok(false);}
                gallery.save=Some(Mutex::new(world.resource::<terra_server::IntegratedServer>().request_save()?));
            }
            26 => {
                if !save_ready(&mut gallery)? {return Ok(false);}
                verify_saved_areas(world,&gallery)?;
                if gallery.captures!=7 {return Err(format!("Expected seven real construction captures, got {}",gallery.captures));}
            }
            _ => {
                world.resource_mut::<GameplayConfig>().language=gallery.language;
                world.resource_mut::<Cursor>().0=None;
                info!(job=gallery.job,npc=gallery.npc,captures=gallery.captures,"Native construction mode passed: real TAB, detached pan, four-resource HUD, cottage HTML selection/server preview/orange mesh, ground-click queue, Creative free build without storage, private NPC dialog, ESC retention, restored avatar/camera, automatic saved areas, red Build boundaries and H hide/show; DE/EN bounds");
                return Ok(true);
            }
        }
        info!(step=gallery.step,"Native construction mode acceptance step completed");
        gallery.advance();
        Ok(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_crop_rejects_blue_sky_even_with_colorful_ui_and_accepts_actual_ground_channels() {
        for bgra in [false, true] {
            let (width, height) = (100, 100);
            let mut pixels = vec![0; width * height * 4];
            for y in 0..height {
                for x in 0..width {
                    let rgb = if (38..65).contains(&x) && (52..72).contains(&y) {
                        [133, 163, 185]
                    } else {
                        [210, 140, 40]
                    };
                    let offset = (y * width + x) * 4;
                    let [red, green, blue] = rgb;
                    pixels[offset..offset + 4].copy_from_slice(&if bgra {
                        [blue, green, red, 255]
                    } else {
                        [red, green, blue, 255]
                    });
                }
            }
            let (samples, ground) = terrain_samples(&pixels, width, height, bgra);
            assert!(samples > 0);
            assert_eq!(ground, 0, "Colorful UI must not conceal a missing world");
            for y in 52..72 {
                for x in 38..65 {
                    let offset = (y * width + x) * 4;
                    pixels[offset..offset + 4].copy_from_slice(&if bgra {
                        [40, 160, 100, 255]
                    } else {
                        [100, 160, 40, 255]
                    });
                }
            }
            let (samples, ground) = terrain_samples(&pixels, width, height, bgra);
            assert_eq!(ground, samples);
        }
    }
}
