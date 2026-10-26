//! Short native equipment lifecycle check using saved items and real UDP transactions.
use bevy::{
    camera::RenderTarget,
    picking::{
        PickingSystems,
        pointer::{Location, PointerAction, PointerId, PointerInput, PointerLocation},
    },
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    window::WindowRef,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    AccessoryKind, ArmorItem, ArmorMaterial, ArmorPiece, CapeItem, EquipmentSlot, EquipmentSlots,
    GameConfig, GameMode, Inventory, ItemKind, PlayerProfile, PlayerStats,
};
use terra_network::{
    client::{ClientSession, InventoryRequest},
    protocol::inventory::{InventoryAction, InventoryCommand, InventoryOutcome},
};

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Survival;
    config.server.natural_mob_limit = Some(0);
    config.world.biomes = false;
    config.world.trees = false;
    config.world.caves = false;
    config.world.rivers = false;
    config.world.water = false;
    config.world.base_height = 16.0;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    let path = std::env::temp_dir().join(format!("terra-equipment-{}", std::process::id()));
    if path.exists() {
        return Err("Equipment fixture needs a fresh save directory".into());
    }
    let profile_path = PlayerProfile::selected_path(&terra_core::menu_config_directory())
        .map_err(|e| e.to_string())?;
    let profile = PlayerProfile::load_for_client(profile_path).map_err(|e| e.to_string())?;
    let mut bag = Inventory::default();
    let mut stats = PlayerStats {
        equipment: desired(0),
        ..default()
    };
    for item in ArmorItem::ALL
        .map(ItemKind::Armor)
        .into_iter()
        .chain(AccessoryKind::ALL.map(ItemKind::Accessory))
        .chain(fixture_capes().map(ItemKind::Cape))
    {
        if !stats.equipment.iter().any(|(_, equipped)| equipped == item) {
            assert_eq!(bag.insert(item, 1), 0);
        }
    }
    stats.survival.apply_equipment(stats.equipment);
    stats.survival.hunger = 50.0;
    // Two simultaneous debuffs must remain separate cards and only one heart coating.
    stats
        .survival
        .effects
        .apply(terra_core::EffectKind::Poison, 240.0, 1);
    stats
        .survival
        .effects
        .apply(terra_core::EffectKind::Freeze, 240.0, 1);
    stats
        .survival
        .effects
        .apply(terra_core::EffectKind::Regeneration, 240.0, 3);
    stats
        .survival
        .effects
        .apply(terra_core::EffectKind::ShieldPower, 240.0, 1);
    let mut journal = terra_world::edits::EditJournal::default();
    journal.gameplay_mut().inventories.insert(profile.uuid, bag);
    journal.update_player_stats(profile.uuid, stats);
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: usize,
    configured: bool,
    pending: bool,
    response: Option<u64>,
    max_meshes: usize,
    captures: usize,
    seen_capes: std::collections::HashSet<CapeItem>,
    hovered_cell: Option<Entity>,
    tooltip_bounds: Option<Rect>,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-equipment") {
        return;
    }
    app.insert_resource(terra_player::ControllerMode::Walk)
        .insert_resource(terra_player::CameraPerspective::ThirdPerson)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Inspection {
            start: Instant::now(),
            since: Instant::now(),
            stage: 0,
            configured: false,
            pending: false,
            response: None,
            max_meshes: 0,
            captures: 0,
            seen_capes: default(),
            hovered_cell: None,
            tooltip_bounds: None,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            PostUpdate,
            fixture_scale.before(bevy::ui::UiSystems::Prepare),
        )
        .add_systems(
            PreUpdate,
            focus
                .after(bevy::input::InputSystems)
                .before(PickingSystems::ProcessInput),
        )
        .add_systems(Last, run);
}

fn tooltip_test_scale(stage: usize) -> Option<f32> {
    match stage {
        3 => Some(1.5),
        7 => Some(0.8),
        _ => None,
    }
}

fn fixture_scale(inspection: Res<Inspection>, mut scale: ResMut<bevy::ui::UiScale>) {
    if inspection.configured
        && let Some(expected) = tooltip_test_scale(inspection.stage)
        && scale.0 != expected
    {
        // Menu fitting runs in Update; hold the exact fixture scale through UI layout.
        scale.0 = expected;
    }
}

fn setup(world: &mut World) {
    let mut menu = world.resource_mut::<terra_ui::menu::MenuState>();
    menu.page = terra_ui::menu::MenuPage::Playing;
    menu.session_active = true;
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world
        .resource_mut::<terra_core::GraphicsConfig>()
        .vegetation_density = 0.0;
    world
        .resource_mut::<terra_core::GameplayConfig>()
        .minimap
        .enabled = false;
    world.write_message(terra_player::CaptureRequest(true));
    for mut projection in world
        .query_filtered::<&mut Projection, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        if let Projection::Perspective(projection) = &mut *projection {
            projection.fov = 65.0_f32.to_radians();
        }
    }
    world.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-2.0, 4.0, -6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn focus(world: &mut World) {
    for mut window in world
        .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
        .iter_mut(world)
    {
        window.bypass_change_detection().focused = true;
    }
    let hovered = world.resource::<Inspection>().hovered_cell;
    if !world.resource::<terra_ui::InventoryView>().open {
        return;
    }
    let Some(physical) = hovered.and_then(|cell| {
        world
            .get::<bevy::ui::UiGlobalTransform>(cell)
            .map(|transform| transform.translation)
    }) else {
        return;
    };
    let Some((window, scale)) = world
        .query_filtered::<(Entity, &Window), With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|(entity, window)| (entity, window.scale_factor()))
    else {
        return;
    };
    let Some(target) = RenderTarget::Window(WindowRef::Entity(window)).normalize(Some(window))
    else {
        return;
    };
    // Exercise the real picking pipeline without warping the desktop cursor or sending clicks.
    world.write_message(PointerInput::new(
        PointerId::Mouse,
        Location {
            target,
            position: physical / scale,
        },
        PointerAction::Move { delta: Vec2::ZERO },
    ));
}

fn desired(stage: usize) -> EquipmentSlots {
    let mut slots = EquipmentSlots::default();
    let material = ArmorMaterial::ALL
        .get(stage / 2)
        .copied()
        .unwrap_or(ArmorMaterial::Ruby);
    for piece in ArmorPiece::ALL {
        slots.set(
            piece.slot(),
            Some(ItemKind::Armor(ArmorItem { material, piece })),
        );
    }
    for kind in AccessoryKind::ALL {
        slots.set(kind.slot(), Some(ItemKind::Accessory(kind)));
    }
    if stage < 14 {
        let color = if stage == 10 || stage == 11 {
            terra_core::beds::DyeColor::Blue
        } else if stage >= 12 {
            terra_core::beds::DyeColor::Purple
        } else {
            terra_core::beds::DyeColor::White
        };
        slots.set(
            EquipmentSlot::Cape,
            Some(ItemKind::Cape(CapeItem { material, color })),
        );
    }
    slots
}

fn fixture_capes() -> [CapeItem; 7] {
    let mut capes = [CapeItem {
        material: ArmorMaterial::Leather,
        color: terra_core::beds::DyeColor::White,
    }; 7];
    capes[..5].copy_from_slice(&CapeItem::ALL_WHITE);
    capes[5] = CapeItem {
        material: ArmorMaterial::Ruby,
        color: terra_core::beds::DyeColor::Blue,
    };
    capes[6] = CapeItem {
        material: ArmorMaterial::Ruby,
        color: terra_core::beds::DyeColor::Purple,
    };
    capes
}

fn next_action(session: &ClientSession, target: EquipmentSlots) -> Option<InventoryAction> {
    for slot in EquipmentSlot::ALL {
        if session.stats.equipment.get(slot) == target.get(slot) {
            continue;
        }
        if session.stats.equipment.get(slot).is_some() {
            return Some(InventoryAction::EquipmentQuickMove {
                slot: slot.index() as u8,
            });
        }
        if let Some(item) = target.get(slot) {
            let index = session
                .inventory
                .slots
                .iter()
                .position(|stack| stack.item == item && stack.count == 1)?;
            return Some(InventoryAction::QuickMove { slot: index as u8 });
        }
    }
    None
}

fn verify_tooltip(
    world: &World,
    tip: Entity,
    pointer: Vec2,
    viewport: Vec2,
    name: &str,
    identifier: &str,
) -> Result<Rect, String> {
    if !world
        .get::<InheritedVisibility>(tip)
        .is_some_and(|visibility| visibility.get())
    {
        return Err("Tooltip is open but not inherited-visible to the UI renderer".into());
    }
    let node = world
        .get::<ComputedNode>(tip)
        .ok_or("Tooltip has no computed layout")?;
    let transform = world
        .get::<bevy::ui::UiGlobalTransform>(tip)
        .ok_or("Tooltip has no render transform")?;
    let top_left = transform.affine().transform_point2(-node.size() * 0.5);
    let bounds = Rect::from_corners(top_left, top_left + node.size());
    let expected = expected_tooltip_position(
        pointer,
        node.size(),
        viewport,
        10.0 / node.inverse_scale_factor,
    );
    if !bounds.min.abs_diff_eq(expected, 1.5) {
        return Err(format!(
            "Tooltip ten-CSS-pixel placement mismatch: actual={:?}, expected={expected:?}, pointer={pointer:?}, inverse_scale={}",
            bounds.min, node.inverse_scale_factor
        ));
    }
    if node.size().min_element() < 20.0
        || bounds.min.cmplt(-Vec2::splat(2.0)).any()
        || bounds.max.cmpgt(viewport + Vec2::splat(2.0)).any()
        || pointer.distance(pointer.clamp(bounds.min, bounds.max)) > 40.0
        || bounds.contains(pointer)
    {
        return Err(format!(
            "Tooltip is not visibly adjacent to its real picking cursor: {bounds:?}, cursor={pointer:?}, viewport={viewport:?}"
        ));
    }
    let mut pending = vec![tip];
    let mut text = String::new();
    let mut rendered_text = 0;
    while let Some(entity) = pending.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
        if let Some(value) = world.get::<Text>(entity) {
            text.push_str(&value.0);
            text.push('\n');
            if value.0.trim().is_empty() {
                continue;
            }
            let color = world
                .get::<TextColor>(entity)
                .ok_or("Tooltip text has no rendered color")?;
            if color.0.to_srgba().alpha < 0.7 {
                return Err(format!(
                    "Tooltip text is nearly transparent: {:?}: {:?}",
                    value.0, color.0
                ));
            }
            if world
                .get::<bevy::text::TextLayoutInfo>(entity)
                .is_some_and(|layout| !layout.glyphs.is_empty())
            {
                rendered_text += 1;
            }
        }
    }
    if !text.contains(name) || !text.contains(identifier) || rendered_text < 2 {
        return Err(format!(
            "Tooltip renders the wrong item or has no actual glyphs: expected={name}/{identifier}, text={text:?}, rendered={rendered_text}"
        ));
    }
    Ok(bounds)
}

fn expected_tooltip_position(pointer: Vec2, size: Vec2, viewport: Vec2, gap: f32) -> Vec2 {
    let mut top_left = pointer + Vec2::splat(gap);
    for axis in 0..2 {
        if top_left[axis] + size[axis] > viewport[axis] - gap {
            top_left[axis] = pointer[axis] - gap - size[axis];
        }
    }
    top_left.clamp(Vec2::ZERO, (viewport - size).max(Vec2::ZERO))
}

fn tooltip_pixels_visible(image: &Image, bounds: Rect) -> bool {
    let Some(data) = image.data.as_ref() else {
        return false;
    };
    let width = image.texture_descriptor.size.width as usize;
    let height = image.texture_descriptor.size.height as usize;
    let min = bounds.min.max(Vec2::ZERO).as_uvec2();
    let max = bounds
        .max
        .min(Vec2::new(width as f32, height as f32))
        .as_uvec2();
    let mut bright = 0;
    let mut dark = 0;
    let mut count = 0;
    for y in min.y as usize..max.y as usize {
        for x in min.x as usize..max.x as usize {
            let Some(pixel) = data.get((y * width + x) * 4..(y * width + x + 1) * 4) else {
                return false;
            };
            bright += usize::from(
                pixel[..3].iter().all(|channel| *channel > 85)
                    && pixel[..3].iter().any(|channel| *channel > 145),
            );
            dark += usize::from(pixel[..3].iter().all(|channel| *channel < 65));
            count += 1;
        }
    }
    count > 400 && bright > 30 && dark > count / 3
}

fn check(world: &mut World, target: EquipmentSlots, inventory: bool) -> Result<(), String> {
    let session = world.resource::<ClientSession>();
    if session.stats.equipment != target || !session.stats.equipment.valid() {
        return Err("Wrong replicated equipment".into());
    }
    let carried = session
        .inventory
        .slots
        .iter()
        .filter(|stack| {
            matches!(
                stack.item,
                ItemKind::Armor(_) | ItemKind::Accessory(_) | ItemKind::Cape(_)
            ) && stack.valid()
        })
        .count();
    if carried + target.iter().count() != 36 || session.inventory.cursor.is_some() {
        return Err("Equipment duplication, loss or leftover cursor".into());
    }
    let armor = session.stats.survival.armor_points;
    if (armor - target.bonuses().armor_points).abs() > 0.001 {
        return Err("Armor is not derived from the loadout".into());
    }
    let expected_effect = session
        .stats
        .survival
        .active_effects()
        .next()
        .map(|e| e.kind);
    let render = world.resource::<terra_ui::VitalRenderStatus>();
    if render.primary_effect != expected_effect
        || !render.poison
        || render.freeze
        || render.regeneration
    {
        return Err("Heart mixes status effects".into());
    }
    let hud = world.resource::<terra_ui::SurvivalView>();
    if hud.show_armor != (armor > 0.0)
        || (hud.armor - armor / terra_core::equipment::MAX_ARMOR_POINTS * 100.0).abs() > 0.01
    {
        return Err("Armor HUD mismatch".into());
    }
    if inventory {
        if let Some(expected) = tooltip_test_scale(world.resource::<Inspection>().stage) {
            let actual = world.resource::<bevy::ui::UiScale>().0;
            if (actual - expected).abs() > 0.001 {
                return Err(format!(
                    "Tooltip fixture layout did not retain requested UiScale: actual={actual}, expected={expected}"
                ));
            }
        }
        let view = world.resource::<terra_ui::InventoryView>();
        if !view.open
            || view.equipment.len() != terra_core::EQUIPMENT_SLOTS
            || view.equipment.iter().filter(|v| !v.item.empty).count() != target.iter().count()
        {
            return Err("Equipment cells do not match authority".into());
        }
        let scroll = world
            .query::<(&terra_ui::ElementId, &ComputedNode)>()
            .iter(world)
            .find(|(id, _)| id.0 == "inventory-scroll")
            .map(|(_, node)| *node)
            .ok_or("Missing inventory body")?;
        if scroll.content_size.y > scroll.size().y + 2.0
            || scroll.scrollbar_size.max_element() > 1.0
        {
            return Err(format!(
                "Player inventory still requires scrolling: {scroll:?}"
            ));
        }
        let hovered = world
            .resource::<Inspection>()
            .hovered_cell
            .ok_or("No hover probe")?;
        let empty = world.resource::<Inspection>().stage == 15;
        let owner = world
            .get::<ChildOf>(hovered)
            .ok_or("Unparented hover target")?
            .parent();
        let tip = world
            .query::<(Entity, &terra_ui::TooltipOwner)>()
            .iter(world)
            .find(|(_, settings)| settings.0 == owner)
            .map(|(entity, _)| entity)
            .ok_or("Missing persistent slot tooltip")?;
        let disabled = world.get::<bevy::ui::InteractionDisabled>(tip).is_some();
        let open = world
            .get::<Node>(tip)
            .is_some_and(|node| node.display != Display::None)
            && world.get::<Visibility>(tip) != Some(&Visibility::Hidden);
        if disabled != empty || open == empty {
            return Err(format!(
                "Hover tooltip mismatch without any click: empty={empty} disabled={disabled} open={open}"
            ));
        }
        if !empty {
            let expected_slot = if world.resource::<Inspection>().stage >= 13 {
                EquipmentSlot::Cape
            } else {
                EquipmentSlot::Helmet
            };
            let expected = world
                .resource::<terra_ui::InventoryView>()
                .equipment
                .iter()
                .find(|slot| slot.kind == expected_slot.slug())
                .ok_or("Missing hovered equipment metadata")?;
            let name = expected.item.name.clone();
            let identifier = expected.item.identifier.clone();
            let pointer = world
                .query::<(&PointerId, &PointerLocation)>()
                .iter(world)
                .find_map(|(id, location)| {
                    (*id == PointerId::Mouse)
                        .then(|| location.location())
                        .flatten()
                        .map(|location| location.position)
                })
                .ok_or("No real picking mouse location")?;
            let (scale, viewport) = world
                .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
                .iter(world)
                .next()
                .map(|window| {
                    (
                        window.scale_factor(),
                        Vec2::new(
                            window.physical_width() as f32,
                            window.physical_height() as f32,
                        ),
                    )
                })
                .ok_or("Missing tooltip viewport")?;
            let bounds = verify_tooltip(world, tip, pointer * scale, viewport, &name, &identifier)?;
            world.resource_mut::<Inspection>().tooltip_bounds = Some(bounds);
        } else {
            world.resource_mut::<Inspection>().tooltip_bounds = None;
        }
        let root = world
            .query::<(Entity, &terra_ui::ElementId)>()
            .iter(world)
            .find(|(_, id)| id.0 == "inventory-equipment")
            .map(|(entity, _)| entity)
            .ok_or("Missing materialized equipment grid")?;
        let mut pending = vec![root];
        let mut cells = 0;
        let mut icons = 0;
        while let Some(entity) = pending.pop() {
            if let Some(children) = world.get::<Children>(entity) {
                pending.extend(children.iter());
            }
            if world
                .get::<terra_ui::ElementId>(entity)
                .is_some_and(|id| id.0 == "inventory-cell")
            {
                cells += 1;
                if world
                    .get::<ComputedNode>(entity)
                    .is_none_or(|node| node.size().min_element() < 10.0)
                {
                    return Err("Equipment cell has no real layout".into());
                }
            }
            if world
                .get::<tilt_ui::ImageMetadata>(entity)
                .and_then(|meta| meta.alt.as_deref())
                .is_some_and(|alt| alt.starts_with("Item preview teramic::"))
            {
                let image = world
                    .get::<ImageNode>(entity)
                    .ok_or("Equipment preview image not materialized")?;
                if world
                    .resource::<Assets<Image>>()
                    .get(&image.image)
                    .is_none()
                {
                    return Err("Equipment preview did not bind a real asset".into());
                }
                let mut parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
                let mut button = None;
                while let Some(ancestor) = parent {
                    if world
                        .get::<terra_ui::ElementId>(ancestor)
                        .is_some_and(|id| id.0 == "inventory-cell")
                    {
                        button = Some(ancestor);
                        break;
                    }
                    parent = world.get::<ChildOf>(ancestor).map(ChildOf::parent);
                }
                let button = button.ok_or("Unparented equipment image")?;
                let rect = |entity| -> Option<Rect> {
                    let node = world.get::<ComputedNode>(entity)?;
                    let transform = world.get::<bevy::ui::UiGlobalTransform>(entity)?;
                    Some(Rect::from_center_size(transform.translation, node.size()))
                };
                let image_rect = rect(entity).ok_or("Missing equipment image layout")?;
                let slot_rect = rect(button).ok_or("Missing equipment button layout")?;
                if image_rect.min.cmplt(slot_rect.min - Vec2::ONE).any()
                    || image_rect.max.cmpgt(slot_rect.max + Vec2::ONE).any()
                {
                    return Err("Equipment preview overflows its fixed slot".into());
                }
                icons += 1;
            }
        }
        if cells != terra_core::EQUIPMENT_SLOTS || icons != target.iter().count() {
            return Err(format!(
                "Actual equipment UI binding mismatch: {cells} cells, {icons} item images"
            ));
        }
    }
    if let Some(ItemKind::Cape(cape)) = target.get(EquipmentSlot::Cape) {
        world.resource_mut::<Inspection>().seen_capes.insert(cape);
    }
    let seen = world.resource::<Inspection>().seen_capes.len();
    let armor_render = world.resource::<terra_render::ArmorRenderStatus>();
    let expected_parts = 16
        + if target.get(EquipmentSlot::Cape).is_some() {
            3
        } else {
            0
        };
    if !(120 + seen * 3..=120 + seen * 6).contains(&armor_render.cached_meshes)
        || armor_render.cached_materials != 7
        || armor_render.equipped_parts != expected_parts
    {
        return Err(format!(
            "Wrong bounded wearable lifecycle: {armor_render:?}, expected {expected_parts} parts"
        ));
    }
    let meshes = world.resource::<Assets<Mesh>>().len();
    world.resource_mut::<Inspection>().max_meshes =
        world.resource::<Inspection>().max_meshes.max(meshes);
    info!(
        armor,
        carried, meshes, inventory, "Native equipment/cape check passed"
    );
    Ok(())
}

fn run(world: &mut World) {
    if world.resource::<Inspection>().start.elapsed() > Duration::from_secs(120) {
        error!("Equipment fixture timed out");
        world.write_message(AppExit::error());
        return;
    }
    let stage = world.resource::<Inspection>().stage;
    if stage >= 16 {
        let previews = world.resource::<terra_render::RenderBlockPreviews>();
        let portrait = world.resource::<terra_render::PlayerPreviewImage>();
        if previews.error.is_some() || portrait.error.is_some() {
            error!("Equipment preview failed");
            world.write_message(AppExit::error());
        } else if previews.pending == 0 && portrait.ready {
            info!(
                captures = world.resource::<Inspection>().captures,
                max_meshes = world.resource::<Inspection>().max_meshes,
                "Native equipment passed all five sets, white/dyed capes, cape removal, ten slots and bounded caches"
            );
            world.write_message(AppExit::Success);
        }
        return;
    }
    let session = world.resource::<ClientSession>();
    if !session.ready
        || session.teleporting
        || !world.resource::<terra_render::RenderStatus>().ready
    {
        return;
    }
    if let Some(response) = world.resource::<Inspection>().response {
        if session.inventory_response == response {
            return;
        }
        if session.last_inventory != Some(InventoryOutcome::Applied) {
            error!(?session.last_inventory, "Equipment transaction rejected");
            world.write_message(AppExit::error());
            return;
        }
        world.resource_mut::<Inspection>().response = None;
    }
    let target = desired(stage);
    let session = world.resource::<ClientSession>();
    if let Some(action) = next_action(session, target) {
        let command = InventoryCommand {
            revision: session.inventory.revision,
            action,
        };
        let response = session.inventory_response;
        world.resource_mut::<Inspection>().response = Some(response);
        world.resource_mut::<Inspection>().since = Instant::now();
        world.write_message(InventoryRequest(command));
        return;
    }
    let inventory = stage % 2 == 1;
    if !world.resource::<Inspection>().configured {
        for mut window in world
            .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
            .iter_mut(world)
        {
            window.resolution.set(
                if stage == 3 {
                    1920.0
                } else if stage >= 12 {
                    360.0
                } else {
                    1280.0
                },
                if stage == 3 {
                    1440.0
                } else if stage == 11 {
                    600.0
                } else {
                    800.0
                },
            );
        }
        if let Some(scale) = tooltip_test_scale(stage) {
            world.resource_mut::<bevy::ui::UiScale>().0 = scale;
        }
        *world.resource_mut::<terra_player::CameraPerspective>() = if inventory {
            terra_player::CameraPerspective::ThirdPersonFront
        } else {
            terra_player::CameraPerspective::ThirdPerson
        };
        world.write_message(if inventory {
            terra_ui::InventoryAction::Toggle
        } else {
            terra_ui::InventoryAction::Close
        });
        let mut state = world.resource_mut::<Inspection>();
        state.configured = true;
        state.hovered_cell = None;
        state.tooltip_bounds = None;
        state.since = Instant::now();
        return;
    }
    if inventory
        && world.resource::<Inspection>().hovered_cell.is_none()
        && world.resource::<Inspection>().since.elapsed() >= Duration::from_millis(600)
    {
        let root = world
            .query::<(Entity, &terra_ui::ElementId)>()
            .iter(world)
            .find(|(_, id)| id.0 == "inventory-equipment")
            .map(|(entity, _)| entity);
        if let Some(root) = root {
            let mut pending = vec![root];
            let mut cells = Vec::new();
            while let Some(entity) = pending.pop() {
                if let Some(children) = world.get::<Children>(entity) {
                    pending.extend(children.iter());
                }
                if world
                    .get::<terra_ui::ElementId>(entity)
                    .is_some_and(|id| id.0 == "inventory-cell")
                    && let (Some(node), Some(transform)) = (
                        world.get::<ComputedNode>(entity),
                        world.get::<bevy::ui::UiGlobalTransform>(entity),
                    )
                    && !node.is_empty()
                {
                    cells.push((entity, transform.translation));
                }
            }
            cells.sort_by(|(_, a), (_, b)| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
            if let Some((cell, _)) = if stage >= 13 {
                cells.last()
            } else {
                cells.first()
            }
            .copied()
            {
                world.resource_mut::<Inspection>().hovered_cell = Some(cell);
            }
        }
    }
    if world.resource::<Inspection>().pending
        || world.resource::<Inspection>().since.elapsed() < Duration::from_millis(1800)
        || !world.resource::<terra_render::PlayerPreviewImage>().ready
        || world
            .resource::<terra_render::RenderBlockPreviews>()
            .pending
            != 0
    {
        return;
    }
    if let Err(error) = check(world, target, inventory) {
        error!(%error, stage, "Native F02 failed");
        world.write_message(AppExit::error());
        return;
    }
    let folder = PathBuf::from(
        std::env::var_os("TERRA_CAPTURE_DIR").unwrap_or_else(|| "/tmp/terra-equipment".into()),
    );
    if let Err(error) = std::fs::create_dir_all(&folder) {
        error!(%error);
        world.write_message(AppExit::error());
        return;
    }
    let material = match stage {
        10 | 11 => "ruby-blue",
        12 | 13 => "ruby-purple-compact",
        14 | 15 => "ruby-no-cape",
        _ => ArmorMaterial::ALL[stage / 2].slug(),
    };
    let name = format!(
        "armor-{material}-{}.png",
        if inventory { "inventory" } else { "world" }
    );
    world.resource_mut::<Inspection>().pending = true;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(name)))
        .observe(
            |event: On<ScreenshotCaptured>,
             mut state: ResMut<Inspection>,
             mut exit: MessageWriter<AppExit>| {
                if event
                    .image
                    .data
                    .as_ref()
                    .is_none_or(|data| data.iter().filter(|&&v| v > 30).count() < 1000)
                {
                    error!("Blank equipment capture");
                    exit.write(AppExit::error());
                    return;
                }
                if let Some(bounds) = state.tooltip_bounds
                    && !tooltip_pixels_visible(&event.image, bounds)
                {
                    error!(
                        ?bounds,
                        "Tooltip has no contrasting text/background pixels in its measured bounds"
                    );
                    exit.write(AppExit::error());
                    return;
                }
                state.stage += 1;
                state.captures += 1;
                state.configured = false;
                state.pending = false;
            },
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tooltip_scale_probes_use_existing_stages_and_preserve_ten_css_pixels() {
        assert_eq!(tooltip_test_scale(3), Some(1.5));
        assert_eq!(tooltip_test_scale(7), Some(0.8));
        assert_eq!(tooltip_test_scale(11), None);
        let pointer = Vec2::new(100.0, 120.0);
        let size = Vec2::new(180.0, 100.0);
        let viewport = Vec2::new(1280.0, 800.0);
        for scale in [0.8, 1.5] {
            assert!(
                expected_tooltip_position(pointer, size * scale, viewport, 10.0 * scale)
                    .abs_diff_eq(pointer + Vec2::splat(10.0 * scale), 0.001)
            );
        }
    }
    #[test]
    fn fixture_visits_every_material_and_dyed_capes_then_removes_only_the_cape() {
        for stage in 0..10 {
            assert_eq!(desired(stage).iter().count(), 10);
        }
        assert!(matches!(
            desired(10).get(EquipmentSlot::Cape),
            Some(ItemKind::Cape(CapeItem {
                color: terra_core::beds::DyeColor::Blue,
                ..
            }))
        ));
        assert!(matches!(
            desired(12).get(EquipmentSlot::Cape),
            Some(ItemKind::Cape(CapeItem {
                color: terra_core::beds::DyeColor::Purple,
                ..
            }))
        ));
        assert_eq!(desired(14).get(EquipmentSlot::Cape), None);
        assert_eq!(desired(15).iter().count(), 9);
    }
}
