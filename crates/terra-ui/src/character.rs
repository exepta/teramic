//! Transactional local and per-server character editing with a live shared avatar.
use crate::menu::{MenuDialog, MenuPage, MenuPaths, MenuState, ServerMenu, SessionRequest};
use bevy::{prelude::*, ui::widget::ImageNode};
use serde::Serialize;
use terra_core::{
    CharacterAppearance, CharacterSelection, ClothingMaterial, Config, PlayerGender, PlayerProfile,
    ServerCharacterProfiles,
};
use tilt_ui::{ElementId, html_shared};
#[cfg(test)]
#[path = "character_tests.rs"]
mod tests;

#[derive(Resource, Clone, Copy, Default, PartialEq)]
pub struct CharacterPreviewSelection {
    pub gender: PlayerGender,
    pub appearance: CharacterAppearance,
    pub rotation: f32,
}

#[derive(Resource, Default)]
pub struct CharacterPreviewImage {
    pub image: Handle<Image>,
    pub ready: bool,
    pub hairstyles: Handle<Image>,
}

#[derive(Resource)]
struct LocalCharacterProfile(PlayerProfile);

#[derive(Clone)]
struct ServerTarget {
    host: String,
    port: u16,
    title: String,
}

#[derive(Resource)]
struct CharacterDraft {
    tab: String,
    selection: CharacterSelection,
    original: CharacterSelection,
    target: Option<ServerTarget>,
    pending: Option<SessionRequest>,
    rotation: f32,
    error: String,
}

#[derive(Clone, Serialize, PartialEq)]
pub struct HairstyleView {
    pub id: String,
    pub label: String,
    pub preview: String,
    pub selected: bool,
}

#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct CharacterView {
    pub tab: String,
    pub skin: String,
    pub eyes: String,
    pub right_eye: String,
    pub highlights: bool,
    pub highlight_color: String,
    pub boots_color: String,
    pub cape_color: String,
    pub tunic: bool,
    pub boots: bool,
    pub cape: bool,
    pub open: bool,
    pub male: bool,
    pub context: String,
    pub hint: String,
    pub styles: Vec<HairstyleView>,
    pub hair: String,
    pub shirt: String,
    pub pants: String,
    pub shirt_material: String,
    pub pants_material: String,
    pub boots_material: String,
    pub cape_material: String,
    pub error: String,
    pub save_label: String,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CharacterView>()
        .init_resource::<CharacterPreviewImage>()
        .add_systems(
            Update,
            (drag_preview, synchronize)
                .chain()
                .after(crate::menu::MenuInputSet),
        )
        .add_systems(
            PostUpdate,
            (bind_preview, fit_editor).before(bevy::ui::UiSystems::Prepare),
        );
}

/// A drag starts only on the preview, then remains captured until release/focus loss.
fn drag_preview(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    nodes: Query<(&ElementId, &ComputedNode, &UiGlobalTransform)>,
    draft: Option<ResMut<CharacterDraft>>,
    mut previous: Local<Option<f32>>,
) {
    let Some(mut draft) = draft else {
        *previous = None;
        return;
    };
    let Some(window) = windows.iter().next() else {
        return;
    };
    if !window.focused || !mouse.pressed(MouseButton::Left) {
        *previous = None;
        return;
    }
    let Some(cursor) = window.physical_cursor_position() else {
        *previous = None;
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        *previous = nodes
            .iter()
            .any(|(id, node, transform)| {
                id.0 == "character-preview"
                    && transform.try_inverse().is_some_and(|inverse| {
                        Rect::from_center_size(Vec2::ZERO, node.size())
                            .contains(inverse.transform_point2(cursor))
                    })
            })
            .then_some(cursor.x);
    } else if let Some(old) = previous.replace(cursor.x) {
        let width = nodes
            .iter()
            .find(|(id, _, _)| id.0 == "character-preview")
            .map(|(_, node, transform)| {
                (transform.transform_point2(Vec2::X * node.size().x)
                    - transform.transform_point2(Vec2::ZERO))
                .length()
            })
            .unwrap_or(300.0)
            .max(1.0);
        draft.rotation = (draft.rotation + (cursor.x - old) / width * std::f32::consts::TAU)
            .rem_euclid(std::f32::consts::TAU);
    } else {
        *previous = None;
    }
}

fn overrides(world: &World) -> Result<ServerCharacterProfiles, String> {
    let path = world
        .resource::<MenuPaths>()
        .configs
        .join("server-characters.toml");
    if path.exists() {
        ServerCharacterProfiles::load(path).map_err(|e| e.to_string())
    } else {
        Ok(ServerCharacterProfiles::default())
    }
}

fn server_target(world: &World, request: &SessionRequest) -> Result<Option<ServerTarget>, String> {
    match request {
        SessionRequest::JoinServer(id) => {
            let entry = world
                .resource::<ServerMenu>()
                .list
                .servers
                .iter()
                .find(|entry| entry.id == *id)
                .ok_or("Server is no longer available")?;
            entry.validate().map_err(|e| e.to_string())?;
            Ok(Some(ServerTarget {
                host: entry.address.clone(),
                port: entry.port,
                title: entry.name.clone(),
            }))
        }
        SessionRequest::JoinAddress(address) => Ok(Some(ServerTarget {
            host: address.ip().to_string(),
            port: address.port(),
            title: address.to_string(),
        })),
        _ => Ok(None),
    }
}

fn local_profile(world: &World) -> PlayerProfile {
    world.get_resource::<LocalCharacterProfile>().map_or_else(
        || world.resource::<PlayerProfile>().clone(),
        |profile| profile.0.clone(),
    )
}

/// Returns no profile while the first-join editor is waiting for an explicit choice.
pub fn prepare_character_profile(
    world: &mut World,
    request: &SessionRequest,
) -> Result<Option<PlayerProfile>, String> {
    let profile = local_profile(world);
    let Some(target) = server_target(world, request)? else {
        return Ok(Some(profile));
    };
    let saved = overrides(world)?;
    if saved
        .get(profile.uuid, &target.host, target.port)
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return saved
            .apply(&profile, &target.host, target.port)
            .map(Some)
            .map_err(|e| e.to_string());
    }
    open(world, Some(target), Some(request.clone()))?;
    world.resource_mut::<MenuState>().page = MenuPage::Servers;
    Ok(None)
}

/// Installs the exact appearance sent in the connection ticket without changing local defaults.
pub fn activate_character_profile(world: &mut World, profile: PlayerProfile) {
    if !world.contains_resource::<LocalCharacterProfile>() {
        let original = world.resource::<PlayerProfile>().clone();
        world.insert_resource(LocalCharacterProfile(original));
    }
    world.insert_resource(profile);
}

/// Restores the local default after leaving or failing to open a session.
pub fn restore_character_profile(world: &mut World) {
    if let Some(profile) = world.remove_resource::<LocalCharacterProfile>() {
        world.insert_resource(profile.0);
    }
}

fn open(
    world: &mut World,
    target: Option<ServerTarget>,
    pending: Option<SessionRequest>,
) -> Result<(), String> {
    if world.resource::<MenuState>().session_active {
        return Err("Leave the world before editing this character".into());
    }
    if !world.resource::<MenuState>().profile_loaded {
        return Err("Repair profile.toml before editing this character".into());
    }
    let profile = local_profile(world);
    let selection = if let Some(target) = &target {
        overrides(world)?
            .get(profile.uuid, &target.host, target.port)
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| CharacterSelection::from(&profile))
    } else {
        CharacterSelection::from(&profile)
    };
    world.insert_resource(CharacterDraft {
        tab: "hair".into(),
        selection,
        original: selection,
        target,
        pending,
        rotation: 0.0,
        error: String::new(),
    });
    let mut menu = world.resource_mut::<MenuState>();
    menu.dialog = MenuDialog::Character;
    menu.error.clear();
    menu.status.clear();
    Ok(())
}

pub(crate) fn command(world: &mut World, command: &str) -> Result<bool, String> {
    let modal = world.resource::<MenuState>().dialog.clone();
    if modal == MenuDialog::None {
        if command == "character-open" && world.resource::<MenuState>().page == MenuPage::Main {
            open(world, None, None)?;
            return Ok(true);
        }
        if command == "server-character" && world.resource::<MenuState>().page == MenuPage::Servers
        {
            let server = world.resource::<ServerMenu>();
            let request = if let Some(address) = server.lan_selected {
                SessionRequest::JoinAddress(address)
            } else {
                SessionRequest::JoinServer(server.selected.ok_or("Select a server first")?)
            };
            let target = server_target(world, &request)?;
            open(world, target, None)?;
            return Ok(true);
        }
        return Ok(false);
    }
    if modal != MenuDialog::Character {
        return Ok(false);
    }
    if !world.contains_resource::<CharacterDraft>() {
        return Ok(true);
    }
    match command {
        "character-cancel" | "dialog-cancel" | "back" => close(world),
        "character-save" => {
            if let Err(error) = save(world) {
                world.resource_mut::<CharacterDraft>().error = error;
            }
        }
        "character-male" | "character-female" => {
            let mut draft = world.resource_mut::<CharacterDraft>();
            draft.selection.gender = if command == "character-male" {
                PlayerGender::Male
            } else {
                PlayerGender::Female
            };
            let count = CharacterAppearance::hairstyle_count(draft.selection.gender);
            draft.selection.appearance.hairstyle =
                draft.selection.appearance.hairstyle.min(count - 1);
        }
        "character-tunic" => {
            let mut draft = world.resource_mut::<CharacterDraft>();
            draft.selection.appearance.tunic = !draft.selection.appearance.tunic;
        }
        "character-boots" => {
            let mut draft = world.resource_mut::<CharacterDraft>();
            draft.selection.appearance.boots = !draft.selection.appearance.boots;
        }
        "character-cape" => {
            let mut draft = world.resource_mut::<CharacterDraft>();
            draft.selection.appearance.cape = !draft.selection.appearance.cape;
        }
        "character-highlights" => {
            let mut draft = world.resource_mut::<CharacterDraft>();
            draft.selection.appearance.hair_highlights =
                !draft.selection.appearance.hair_highlights;
        }
        "character-random" => {
            let tick = world
                .get_resource::<Time>()
                .map_or(1, |time| time.elapsed().as_nanos() as u64);
            let mut draft = world.resource_mut::<CharacterDraft>();
            let look = &mut draft.selection;
            look.appearance.hairstyle =
                (tick % u64::from(CharacterAppearance::hairstyle_count(look.gender))) as u8;
            look.appearance.hair_color = [
                [59, 31, 19],
                [111, 65, 36],
                [170, 106, 51],
                [207, 170, 91],
                [190, 181, 162],
            ][(tick.rotate_right(7) % 5) as usize];
            look.appearance.shirt_color = [
                [235, 216, 185],
                [92, 121, 72],
                [119, 51, 44],
                [74, 106, 134],
            ][(tick.rotate_right(11) % 4) as usize];
            look.appearance.pants_color =
                [[109, 80, 54], [74, 66, 54], [58, 73, 79]][(tick.rotate_right(17) % 3) as usize];
            look.appearance.shirt_material =
                ClothingMaterial::ALL[(tick.rotate_right(23) % 4) as usize];
            look.appearance.pants_material =
                ClothingMaterial::ALL[(tick.rotate_right(29) % 4) as usize];
            look.appearance.boots_material =
                ClothingMaterial::ALL[(tick.rotate_right(31) % 4) as usize];
            look.appearance.cape_material =
                ClothingMaterial::ALL[(tick.rotate_right(37) % 4) as usize];
            look.appearance.hair_highlights = tick & 1 != 0;
            look.appearance.hair_highlight_color =
                [[207, 170, 91], [166, 80, 55], [203, 194, 169], [89, 57, 37]]
                    [(tick.rotate_right(41) % 4) as usize];
        }
        "character-reset" => {
            let mut draft = world.resource_mut::<CharacterDraft>();
            draft.selection = draft.original;
            draft.rotation = 0.0;
            draft.error.clear();
        }
        "character-left" | "character-right" => {
            world.resource_mut::<CharacterDraft>().rotation += if command == "character-left" {
                0.35
            } else {
                -0.35
            };
        }
        _ => {
            if let Some(tab) = command.strip_prefix("character-tab-")
                && ["body", "hair", "face", "clothing"].contains(&tab)
            {
                world.resource_mut::<CharacterDraft>().tab = tab.into();
            }
            if let Some(index) = command
                .strip_prefix("character-hair-")
                .and_then(|v| v.parse::<u8>().ok())
            {
                let mut draft = world.resource_mut::<CharacterDraft>();
                if index < CharacterAppearance::hairstyle_count(draft.selection.gender) {
                    draft.selection.appearance.hairstyle = index;
                }
            }
        }
    }
    // A modal editor consumes commands from controls behind it as well.
    Ok(true)
}

pub(crate) fn change(world: &mut World, control: &str, value: &str) -> bool {
    if world.resource::<MenuState>().dialog != MenuDialog::Character {
        return false;
    }
    let Some(mut draft) = world.get_resource_mut::<CharacterDraft>() else {
        return true;
    };
    if matches!(
        control,
        "character-shirt-material"
            | "character-pants-material"
            | "character-boots-material"
            | "character-cape-material"
    ) {
        let Some(material) = ClothingMaterial::from_key(value) else {
            draft.error = "Bitte ein verfügbares Kleidungs-Material auswählen.".into();
            return true;
        };
        let appearance = &mut draft.selection.appearance;
        match control {
            "character-shirt-material" => appearance.shirt_material = material,
            "character-pants-material" => appearance.pants_material = material,
            "character-boots-material" => appearance.boots_material = material,
            "character-cape-material" => appearance.cape_material = material,
            _ => unreachable!(),
        }
        draft.error.clear();
        return true;
    }
    let Some(color) = parse_color(value) else {
        draft.error = "Bitte eine deckende Farbe auswählen.".into();
        return true;
    };
    match control {
        "character-skin-color" => draft.selection.appearance.skin_color = color,
        "character-eye-color" => {
            let appearance = &mut draft.selection.appearance;
            appearance.right_eye_color = Some(appearance.resolved_right_eye_color());
            appearance.eye_color = color;
        }
        "character-right-eye-color" => draft.selection.appearance.right_eye_color = Some(color),
        "character-boots-color" => draft.selection.appearance.boots_color = color,
        "character-cape-color" => draft.selection.appearance.cape_color = color,
        "character-hair-color" => draft.selection.appearance.hair_color = color,
        "character-highlight-color" => draft.selection.appearance.hair_highlight_color = color,
        "character-shirt-color" => draft.selection.appearance.shirt_color = color,
        "character-pants-color" => draft.selection.appearance.pants_color = color,
        _ => return true,
    }
    draft.error.clear();
    true
}

fn parse_color(value: &str) -> Option<[u8; 3]> {
    let color = tilt_ui::tilt_ui_css::parse_color_value(value).ok()?;
    if color.alpha < 0.999 {
        return None;
    }
    Some([color.red, color.green, color.blue].map(|v| (v * 255.0).round() as u8))
}

fn save(world: &mut World) -> Result<(), String> {
    let draft = world.resource::<CharacterDraft>();
    draft.selection.validate().map_err(|e| e.to_string())?;
    let pending = draft.pending.clone();
    let configs = world.resource::<MenuPaths>().configs.clone();
    let mut profile = local_profile(world);
    if let Some(target) = &draft.target {
        let mut saved = overrides(world)?;
        saved
            .set(profile.uuid, &target.host, target.port, draft.selection)
            .map_err(|e| e.to_string())?;
        saved
            .save(configs.join("server-characters.toml"))
            .map_err(|e| e.to_string())?;
    } else {
        draft.selection.apply_to(&mut profile);
        let path = PlayerProfile::selected_path(&configs).map_err(|e| e.to_string())?;
        profile.save(path).map_err(|e| e.to_string())?;
        world.insert_resource(profile);
    }
    close(world);
    if let Some(request) = pending {
        let mut menu = world.resource_mut::<MenuState>();
        menu.page = MenuPage::Loading;
        menu.status = "Connecting to server ...".into();
        world.write_message(request);
    }
    Ok(())
}

fn close(world: &mut World) {
    world.resource_mut::<MenuState>().dialog = MenuDialog::None;
    world.remove_resource::<CharacterDraft>();
    world.remove_resource::<CharacterPreviewSelection>();
}

fn synchronize(world: &mut World) {
    if world
        .get_resource::<MenuState>()
        .is_none_or(|menu| menu.dialog != MenuDialog::Character)
    {
        world.remove_resource::<CharacterDraft>();
        world.remove_resource::<CharacterPreviewSelection>();
        if world.resource::<CharacterView>().open {
            *world.resource_mut::<CharacterView>() = CharacterView::default();
        }
        return;
    }
    let Some(draft) = world.get_resource::<CharacterDraft>() else {
        return;
    };
    let selection = CharacterPreviewSelection {
        gender: draft.selection.gender,
        appearance: draft.selection.appearance,
        rotation: draft.rotation,
    };
    let male = draft.selection.gender == PlayerGender::Male;
    let color = |[r, g, b]: [u8; 3]| format!("#{r:02x}{g:02x}{b:02x}");
    let next = CharacterView {
        tab: draft.tab.clone(),
        skin: color(draft.selection.appearance.skin_color),
        eyes: color(draft.selection.appearance.eye_color),
        right_eye: color(draft.selection.appearance.resolved_right_eye_color()),
        highlights: draft.selection.appearance.hair_highlights,
        highlight_color: color(draft.selection.appearance.hair_highlight_color),
        boots_color: color(draft.selection.appearance.boots_color),
        cape_color: color(draft.selection.appearance.cape_color),
        tunic: draft.selection.appearance.tunic,
        boots: draft.selection.appearance.boots,
        cape: draft.selection.appearance.cape,
        open: true,
        male,
        context: draft.target.as_ref().map_or_else(
            || "Dein Standardcharakter".into(),
            |target| format!("Charakter für {}", target.title),
        ),
        hint: if draft.pending.is_some() {
            "Erster Beitritt: Übernimm deinen Charakter oder passe ihn für diesen Server an.".into()
        } else if draft.target.is_some() {
            "Diese Auswahl gilt nur für diesen Server. Dein Standardcharakter bleibt erhalten."
                .into()
        } else {
            "Für Einzelspieler und als Vorlage beim ersten Beitritt zu einem Server.".into()
        },
        styles: (0..CharacterAppearance::hairstyle_count(draft.selection.gender))
            .map(|i| HairstyleView {
                id: format!("character-hair-{i}"),
                preview: format!("character-style-{i}"),
                label: format!("{}{n}", if male { "M" } else { "F" }, n = i + 1),
                selected: i == draft.selection.appearance.hairstyle,
            })
            .collect(),
        hair: color(draft.selection.appearance.hair_color),
        shirt: color(draft.selection.appearance.shirt_color),
        pants: color(draft.selection.appearance.pants_color),
        shirt_material: draft.selection.appearance.shirt_material.key().into(),
        pants_material: draft.selection.appearance.pants_material.key().into(),
        boots_material: draft.selection.appearance.boots_material.key().into(),
        cape_material: draft.selection.appearance.cape_material.key().into(),
        error: draft.error.clone(),
        save_label: if draft.pending.is_some() {
            "Speichern & beitreten"
        } else {
            "Speichern"
        }
        .into(),
    };
    if world.get_resource::<CharacterPreviewSelection>() != Some(&selection) {
        world.insert_resource(selection);
    }
    if *world.resource::<CharacterView>() != next {
        *world.resource_mut::<CharacterView>() = next;
    }
}

fn bind_preview(
    preview: Res<CharacterPreviewImage>,
    parents: Query<&ComputedNode>,
    mut nodes: Query<(
        &ElementId,
        &ChildOf,
        &mut ImageNode,
        &mut Node,
        &tilt_ui::ImageMetadata,
    )>,
) {
    if preview.image == Handle::default() {
        return;
    }
    for (id, parent, mut image, mut node, metadata) in &mut nodes {
        if let Some(index) = metadata
            .alt
            .as_deref()
            .and_then(|alt| alt.strip_prefix("character-style-"))
            .and_then(|value| value.parse::<u8>().ok())
        {
            image.image = preview.hairstyles.clone();
            image.color = Color::WHITE;
            let x = f32::from(index % 5) * 200.0;
            let y = f32::from(index / 5) * 200.0;
            image.rect = Some(Rect::new(x, y, x + 200.0, y + 200.0));
        }
        if id.0 != "character-preview" {
            continue;
        }
        if image.image != preview.image {
            image.image = preview.image.clone();
            image.color = Color::WHITE;
        }
        // TiltUI has no object-fit; contain the live target without distorting the body.
        if let Ok(parent) = parents.get(parent.parent()) {
            let available = parent.size() * parent.inverse_scale_factor();
            let ratio = 640.0 / 896.0;
            let height = available.y.min(available.x / ratio).max(0.0);
            let size = Vec2::new(height * ratio, height);
            let offset = (available - size) * 0.5;
            let node = &mut *node;
            for (current, next) in [
                (&mut node.width, size.x),
                (&mut node.height, size.y),
                (&mut node.left, offset.x),
                (&mut node.top, offset.y),
            ] {
                if *current != Val::Px(next) {
                    *current = Val::Px(next);
                }
            }
        }
    }
}

fn fit_editor(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Res<UiScale>,
    graphics: Res<terra_core::GraphicsConfig>,
    mut panels: Query<(&ElementId, &mut UiTransform)>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let viewport = window.size();
    let fit = (viewport.x * 0.92 / 1160.0).min(viewport.y * 0.92 / 820.0);
    let desired = (viewport.y / 980.0).clamp(0.9, 2.0) * graphics.ui_scale;
    let zoom = desired.min(fit).max(0.1) / scale.0.max(0.1);
    for (id, mut transform) in &mut panels {
        if id.0 == "character-editor" && transform.scale != Vec2::splat(zoom) {
            transform.scale = Vec2::splat(zoom);
        }
    }
}
