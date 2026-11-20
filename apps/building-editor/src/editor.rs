use crate::document::{AuthoredCell, Document};
use bevy::prelude::*;
use std::path::PathBuf;
use terra_core::{GameConfig, ItemKind};
use terra_player::{CaptureRequest, PlayerInputBlock, TargetBlock, TerraCamera};
use terra_world::{
    edits::EditJournal,
    placement::Layout,
    streaming::{BlockEditRequest, ChunkStore, commit_block_edit, commit_object_state},
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Panel {
    #[default]
    Playing,
    Menu,
    Items,
    Load,
    Save,
    Options,
    Overwrite,
    Discard,
    Quit,
}
#[derive(Resource)]
pub struct Editor {
    pub panel: Panel,
    pub selected: ItemKind,
    pub search: String,
    pub category: String,
    pub page: usize,
    pub directory: PathBuf,
    pub filename: String,
    pub name: String,
    pub pending_path: Option<PathBuf>,
    pub error: String,
    pub status: String,
    pub english: bool,
    pub pending_panel: Option<Panel>,
    pub cooldown: f32,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            panel: Panel::Playing,
            selected: ItemKind::Block(16),
            search: String::new(),
            category: String::new(),
            page: 0,
            directory: terra_core::asset_directory().join("buildings"),
            filename: "new-building".into(),
            name: "New building".into(),
            pending_path: None,
            error: String::new(),
            status: String::new(),
            english: false,
            pending_panel: None,
            cooldown: 0.0,
        }
    }
}
#[derive(Message, Clone, Debug)]
pub enum EditorAction {
    Command { command: String, value: String },
    Change { control: String, value: String },
}

pub fn panel(world: &mut World, next: Panel) {
    world.resource_mut::<Editor>().panel = next;
    world.resource_mut::<PlayerInputBlock>().0 = next != Panel::Playing;
    world.write_message(CaptureRequest(next == Panel::Playing));
}
pub fn keyboard(world: &mut World) {
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let escape = keys.just_pressed(KeyCode::Escape);
    let inventory = keys.just_pressed(KeyCode::KeyE);
    let save = keys.just_pressed(KeyCode::KeyS)
        && (keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight));
    let current = world.resource::<Editor>().panel;
    let typing = world
        .get_resource::<bevy::input_focus::InputFocus>()
        .and_then(|focus| focus.get())
        .and_then(|entity| world.get::<tilt_ui::ElementId>(entity))
        .is_some_and(|id| {
            matches!(
                id.0.as_str(),
                "editor-search"
                    | "editor-directory"
                    | "editor-filename"
                    | "editor-name"
                    | "editor-speed"
                    | "editor-sensitivity"
                    | "editor-platform-size"
            )
        });
    if escape {
        panel(
            world,
            if current == Panel::Playing {
                Panel::Menu
            } else if current == Panel::Menu {
                Panel::Playing
            } else {
                Panel::Menu
            },
        );
    } else if inventory && !typing && matches!(current, Panel::Playing | Panel::Items) {
        panel(
            world,
            if current == Panel::Playing {
                Panel::Items
            } else {
                Panel::Playing
            },
        );
    } else if save {
        panel(world, Panel::Save);
    }
}
pub fn actions(world: &mut World) {
    let actions: Vec<_> = world
        .resource_mut::<Messages<EditorAction>>()
        .drain()
        .collect();
    for action in actions {
        let result = match action {
            EditorAction::Command { command, value } => command_action(world, &command, &value),
            EditorAction::Change { control, value } => change(world, &control, &value),
        };
        if let Err(error) = result {
            world.resource_mut::<Editor>().error = error;
        }
    }
}
fn command_action(world: &mut World, command: &str, value: &str) -> Result<(), String> {
    world.resource_mut::<Editor>().error.clear();
    match command {
        "resume" => panel(world, Panel::Playing),
        "menu" => panel(world, Panel::Menu),
        "items" => panel(world, Panel::Items),
        "options" => panel(world, Panel::Options),
        "save" => panel(world, Panel::Save),
        "load" => {
            if world.resource::<Document>().dirty() {
                world.resource_mut::<Editor>().pending_panel = Some(Panel::Load);
                panel(world, Panel::Discard);
            } else {
                panel(world, Panel::Load);
            }
        }
        "new" => {
            if world.resource::<Document>().dirty() {
                world.resource_mut::<Editor>().pending_panel = Some(Panel::Playing);
                panel(world, Panel::Discard);
            } else {
                crate::scene::install_document(world, Document::default())?;
                panel(world, Panel::Playing);
            }
        }
        "discard" => {
            let next = world
                .resource_mut::<Editor>()
                .pending_panel
                .take()
                .unwrap_or(Panel::Menu);
            if next == Panel::Playing {
                crate::scene::install_document(world, Document::default())?;
            }
            panel(world, next);
        }
        "quit" => {
            if world.resource::<Document>().dirty() {
                panel(world, Panel::Quit);
            } else {
                world.write_message(AppExit::Success);
            }
        }
        "quit-confirm" => {
            world.write_message(AppExit::Success);
        }
        "select-item" => {
            let palette = world.resource::<terra_player::interaction::BlockPalette>();
            let item = terra_ui::creative_items(palette)
                .into_iter()
                .find(|item| item.identifier() == value)
                .ok_or("Unknown real catalog item")?;
            world.resource_mut::<Editor>().selected = item;
            world
                .resource_mut::<terra_player::interaction::ActiveItem>()
                .0 = Some(item);
            panel(world, Panel::Playing);
        }
        "category" => {
            let mut editor = world.resource_mut::<Editor>();
            editor.category = value.into();
            editor.page = 0;
        }
        "previous" => {
            let mut editor = world.resource_mut::<Editor>();
            editor.page = editor.page.saturating_sub(1);
        }
        "next" => world.resource_mut::<Editor>().page += 1,
        "parent" => {
            let parent = world
                .resource::<Editor>()
                .directory
                .parent()
                .map(PathBuf::from)
                .ok_or("No parent directory")?;
            world.resource_mut::<Editor>().directory = parent;
        }
        "file" => {
            let path = PathBuf::from(value);
            if path.is_dir() {
                world.resource_mut::<Editor>().directory = path;
            } else if world.resource::<Editor>().panel == Panel::Save {
                world.resource_mut::<Editor>().filename = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .into();
                world.resource_mut::<Editor>().pending_path = Some(path);
            } else {
                let template = terra_core::building_jobs::template::BuildingTemplate::load(&path)?;
                let document = Document::from_template(template, Some(path))?;
                crate::scene::install_document(world, document)?;
                panel(world, Panel::Playing);
            }
        }
        "default" => {
            let path = terra_core::asset_directory()
                .join(terra_core::building_jobs::template::DEFAULT_HOUSE_PATH);
            let document = Document::from_template(
                terra_core::building_jobs::template::BuildingTemplate::load(&path)?,
                Some(path),
            )?;
            crate::scene::install_document(world, document)?;
            panel(world, Panel::Playing);
        }
        "write" => save(world, false)?,
        "overwrite" => save(world, true)?,
        "windowed" => {
            for mut window in world.query::<&mut Window>().iter_mut(world) {
                window.mode = bevy::window::WindowMode::Windowed;
            }
        }
        "borderless" => {
            for mut window in world.query::<&mut Window>().iter_mut(world) {
                window.mode = bevy::window::WindowMode::BorderlessFullscreen(
                    bevy::window::MonitorSelection::Current,
                );
            }
        }
        "vsync" => {
            for mut window in world.query::<&mut Window>().iter_mut(world) {
                window.present_mode = if window.present_mode == bevy::window::PresentMode::AutoVsync
                {
                    bevy::window::PresentMode::AutoNoVsync
                } else {
                    bevy::window::PresentMode::AutoVsync
                };
            }
        }
        _ => return Err(format!("Unknown editor command: {command}")),
    }
    Ok(())
}
fn change(world: &mut World, control: &str, value: &str) -> Result<(), String> {
    match control {
        "search" => {
            let mut editor = world.resource_mut::<Editor>();
            editor.search = value.into();
            editor.page = 0;
        }
        "filename" => {
            world.resource_mut::<Editor>().filename = value.trim_end_matches(".json").into();
            world.resource_mut::<Editor>().pending_path = None;
        }
        "name" => world.resource_mut::<Editor>().name = value.into(),
        "directory" => {
            let path = PathBuf::from(value);
            if !path.is_dir() {
                return Err("Directory does not exist / Verzeichnis nicht vorhanden".into());
            }
            world.resource_mut::<Editor>().directory = path;
        }
        "speed" => {
            let speed = value.parse::<f32>().map_err(|_| "Invalid flight speed")?;
            if !speed.is_finite() || !(0.1..=100.0).contains(&speed) {
                return Err("Flight speed must be 0.1–100".into());
            }
            world.resource_mut::<GameConfig>().client.fly_speed = speed;
        }
        "sensitivity" => {
            let v = value
                .parse::<f32>()
                .map_err(|_| "Invalid mouse sensitivity")?;
            if !v.is_finite() || !(0.0001..=0.02).contains(&v) {
                return Err("Mouse sensitivity must be 0.0001–0.02".into());
            }
            world.resource_mut::<GameConfig>().client.mouse_sensitivity = v;
        }
        "language" => world.resource_mut::<Editor>().english = value == "en",
        "platform-size" => {
            let size = value
                .parse::<u16>()
                .map_err(|_| "Invalid support size / Ungültige Bauhilfengröße")?;
            crate::scene::resize_workspace(world, size)?;
        }
        _ => return Err(format!("Unknown editor setting: {control}")),
    }
    Ok(())
}
fn same_path(a: &std::path::Path, b: &std::path::Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn save(world: &mut World, overwrite: bool) -> Result<(), String> {
    let editor = world.resource::<Editor>();
    if overwrite && (editor.panel != Panel::Overwrite || editor.pending_path.is_none()) {
        return Err("An explicit overwrite confirmation is required".into());
    }
    let path = editor
        .pending_path
        .clone()
        .unwrap_or_else(|| editor.directory.join(format!("{}.json", editor.filename)));
    if !overwrite && path.exists() {
        world.resource_mut::<Editor>().pending_path = Some(path);
        panel(world, Panel::Overwrite);
        return Ok(());
    }
    let mut document = world.resource::<Document>().clone();
    // Keep a loaded template's identity when replacing its actual file. A
    // distinct Save As filename creates a new catalog identity.
    let same_file = document
        .path
        .as_ref()
        .is_some_and(|original| same_path(original, &path));
    if overwrite && path.exists() {
        // Replacing an existing catalog entry edits that entry's stable ID,
        // even when the authored document was loaded from another file.
        document.id = terra_core::building_jobs::template::BuildingTemplate::load(&path)?.id;
    } else if !same_file {
        document.id = editor.filename.clone();
    }
    document.name = editor.name.trim().into();
    let template = document.template()?;
    crate::document::save_template(&path, &template, overwrite)?;
    document.path = Some(path.clone());
    document.saved_revision = document.revision;
    world.insert_resource(document);
    {
        let mut editor = world.resource_mut::<Editor>();
        editor.status = format!("Saved / Gespeichert: {}", path.display());
        editor.pending_path = None;
    }
    panel(world, Panel::Menu);
    Ok(())
}

pub fn interact(world: &mut World) {
    let elapsed = world.resource::<Time>().delta_secs();
    let cooldown = (world.resource::<Editor>().cooldown - elapsed).max(0.0);
    world.resource_mut::<Editor>().cooldown = cooldown;
    if world.resource::<Editor>().panel != Panel::Playing
        || !world
            .resource::<terra_player::CameraInputState>()
            .capture_ready()
    {
        return;
    }
    let keys = world.resource::<ButtonInput<MouseButton>>();
    let breaking = keys.pressed(MouseButton::Left);
    let placing = keys.just_pressed(MouseButton::Right);
    if !placing && (!breaking || world.resource::<Editor>().cooldown > 0.0) {
        return;
    }
    let Some(hit) = world.resource::<TargetBlock>().0 else {
        return;
    };
    let result = if breaking {
        remove(world, hit.position)
    } else {
        place(world, hit)
    };
    if let Err(error) = result {
        world.resource_mut::<Editor>().error = error;
    } else {
        world.resource_mut::<Editor>().error.clear();
        world.resource_mut::<Editor>().cooldown = 0.15;
    }
}
fn remove(world: &mut World, position: [i64; 3]) -> Result<(), String> {
    if !world.resource::<Document>().cells.contains_key(&position) {
        return Err(
            "The support platform is not part of the building / Bauhilfe wird nicht abgebaut"
                .into(),
        );
    }
    let journal = world.resource::<EditJournal>();
    let state = journal.object_state(position).unwrap_or_default();
    let mut positions = vec![position];
    if let Some(peer) = state.peer {
        positions.push(peer);
    }
    let mut plan = Layout {
        edits: Vec::new(),
        states: Vec::new(),
        belts: Vec::new(),
    };
    for p in positions {
        let material = world
            .resource::<ChunkStore>()
            .block_at(p)
            .ok_or("Chunk is not ready")?;
        plan.edits.push(BlockEditRequest {
            position: p,
            expected_material: material,
            material: 0,
        });
        plan.states.push((p, None));
    }
    apply(world, plan)
}
fn place(world: &mut World, hit: terra_voxel::blocks::BlockHit) -> Result<(), String> {
    let selected = match world.resource::<Editor>().selected {
        ItemKind::WoodenBlock { material, wood } => {
            let native = ItemKind::for_wood(material, wood);
            if native.wood_species() == Some(wood) {
                native
            } else {
                ItemKind::WoodenBlock { material, wood }
            }
        }
        item => item,
    };
    if selected.block_material().is_none() {
        return Err("This real inventory item is not a placeable building block / Dieses Inventaritem ist kein Baublock".into());
    }
    if matches!(selected, ItemKind::WoodenBlock { .. }) {
        return Err("This wood-specific variant has no exact schema-1 representation; choose its native wood block / Diese Holzvariante hat keine verlustfreie Schema-1-Darstellung".into());
    }
    if matches!(
        selected,
        ItemKind::TintedBlock { .. } | ItemKind::DyedBed { .. }
    ) {
        return Err("This tint cannot be stored in building schema 1 / Diese Färbung kann Schema 1 nicht speichern".into());
    }
    let transform = world
        .query_filtered::<&Transform, With<TerraCamera>>()
        .iter(world)
        .next()
        .copied()
        .ok_or("Camera is missing")?;
    let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let merge = if selected.block_material() == Some(29)
        && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight))
    {
        if world.resource::<ChunkStore>().block_at(hit.position) == Some(29) && hit.normal[1] == 0 {
            terra_core::containers::ChestMerge::Target(hit.position)
        } else {
            terra_core::containers::ChestMerge::Separate
        }
    } else {
        terra_core::containers::ChestMerge::Auto
    };
    let plan = terra_world::placement::resolve_with_chest(
        world.resource::<ChunkStore>(),
        world.resource::<EditJournal>(),
        hit,
        selected,
        (
            transform.translation.to_array().map(f64::from),
            (*transform.forward()).to_array().map(f64::from),
        ),
        [yaw, pitch],
        false,
        merge,
    )?;
    apply(world, plan)
}
fn apply(world: &mut World, plan: Layout) -> Result<(), String> {
    let support_y = world.resource::<Document>().anchor[1] + crate::workspace::PLATFORM_Y;
    if plan.edits.iter().any(|edit| edit.position[1] == support_y)
        || plan
            .states
            .iter()
            .any(|(position, _)| position[1] == support_y)
    {
        return Err("The support surface is protected; resize it in Options / Die Bauhilfe ist geschützt; Größe in den Optionen ändern".into());
    }
    if plan
        .edits
        .iter()
        .any(|edit| !world.resource::<Document>().contains(edit.position))
    {
        return Err(
            "Stay inside the gold building outline / Innerhalb des goldenen Bauumrisses bauen"
                .into(),
        );
    }
    let properties = world
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .clone();
    let previous = world.resource::<Document>().clone();
    let committed = world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
        let mut journal = world.resource_mut::<EditJournal>();
        for edit in &plan.edits {
            commit_block_edit(&mut chunks, &mut journal, edit, &properties)?;
        }
        for &(p, state) in &plan.states {
            commit_object_state(&mut chunks, &mut journal, p, state)?;
        }
        Ok::<(), String>(())
    });
    if let Err(error) = committed {
        crate::scene::install_document(world, previous)?;
        return Err(error);
    }
    // Shared support rules can remove dependent lamps/paired pieces. Snapshot
    // the real post-transaction voxel state rather than retaining phantom cells.
    let anchor = world.resource::<Document>().anchor;
    let journal = world.resource::<EditJournal>();
    let chunks = world.resource::<ChunkStore>();
    let mut cells = std::collections::BTreeMap::new();
    for y in crate::document::MINIMUM[1]..=crate::document::MAXIMUM[1] {
        for z in crate::document::MINIMUM[2]..=crate::document::MAXIMUM[2] {
            for x in crate::document::MINIMUM[0]..=crate::document::MAXIMUM[0] {
                let p = [x + anchor[0], y + anchor[1], z + anchor[2]];
                // The support plane lies below MINIMUM and is never scanned
                // into authored data, even after resizing or model edits.
                if let Some(material) = journal
                    .block(p)
                    .or_else(|| chunks.block_at(p))
                    .filter(|material| *material != 0)
                {
                    cells.insert(
                        p,
                        AuthoredCell {
                            material,
                            state: journal.object_state(p).unwrap_or_default(),
                        },
                    );
                }
            }
        }
    }
    let mut document = world.resource_mut::<Document>();
    document.cells = cells;
    document.revision = document.revision.saturating_add(1);
    Ok(())
}

pub fn close_requested(
    mut events: MessageReader<bevy::window::WindowCloseRequested>,
    mut actions: MessageWriter<EditorAction>,
) {
    if events.read().next().is_some() {
        actions.write(EditorAction::Command {
            command: "quit".into(),
            value: String::new(),
        });
    }
}
#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
