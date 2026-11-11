//! Opt-in projection gallery: validated station snapshots, then native E/F1 input.
//! The integrated authority and its player inventory are never changed by these fixtures.
use super::*;
use terra_core::{
    GraphicsConfig, Inventory, ItemStack, brewing, campfire, crusher, furnace, logistics, mixer,
};
use terra_network::protocol::inventory::{ContainerHalf, ContainerSnapshot, ContainerView};
use terra_voxel::global_to_section;
use terra_world::streaming::ChunkStore;
use tilt_ui::ElementClasses;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Surface {
    Furnace,
    Crusher,
    Mixer,
    Cauldron,
    Campfire,
    Chest,
    DoubleChest,
    Workbench,
    FilteredFunnel,
    Tunnel,
    FilteredTunnel,
    Distributor,
}
impl Surface {
    const ALL: [Self; 12] = [
        Self::Furnace,
        Self::Crusher,
        Self::Mixer,
        Self::Cauldron,
        Self::Campfire,
        Self::Chest,
        Self::DoubleChest,
        Self::Workbench,
        Self::FilteredFunnel,
        Self::Tunnel,
        Self::FilteredTunnel,
        Self::Distributor,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Furnace => "furnace",
            Self::Crusher => "crusher",
            Self::Mixer => "mixer",
            Self::Cauldron => "cauldron",
            Self::Campfire => "campfire",
            Self::Chest => "chest",
            Self::DoubleChest => "double-chest",
            Self::Workbench => "workbench-linked",
            Self::FilteredFunnel => "filtered-funnel",
            Self::Tunnel => "tunnel",
            Self::FilteredTunnel => "filtered-tunnel",
            Self::Distributor => "distributor",
        }
    }
    fn material(self) -> u16 {
        match self {
            Self::Furnace => 59,
            Self::Crusher => crusher::MATERIAL,
            Self::Mixer => mixer::MATERIAL,
            Self::Cauldron => brewing::MATERIAL,
            Self::Campfire => campfire::MATERIAL,
            Self::Chest | Self::DoubleChest | Self::Workbench => 29,
            Self::FilteredFunnel => logistics::FILTERED_FUNNEL,
            Self::Tunnel => logistics::TUNNEL,
            Self::FilteredTunnel => logistics::FILTERED_TUNNEL,
            Self::Distributor => logistics::BELT_DISTRIBUTOR,
        }
    }
    fn workspace(self) -> &'static str {
        match self {
            Self::Furnace => "furnace-workspace",
            Self::Crusher => "crusher-workspace",
            Self::Mixer => "mixer-workspace",
            Self::Cauldron => "cauldron-workspace",
            Self::Campfire => "campfire-workspace",
            Self::Chest | Self::DoubleChest => "container-grid",
            Self::Workbench => "crafting-panel",
            Self::FilteredFunnel => "funnel-filter-workspace",
            Self::Tunnel | Self::FilteredTunnel => "tunnel-extraction-mode",
            Self::Distributor => "distributor-filter-slots",
        }
    }
}
#[derive(Resource, Default)]
struct Gallery {
    index: usize,
    compact: bool,
    hud_only: bool,
    layout_only: bool,
    diagnostic_at: Option<Instant>,
    anchor: [i64; 3],
    saved_blocks: Vec<([i64; 3], u16)>,
    projected: Option<ContainerSnapshot>,
    kinetics: Vec<terra_network::protocol::session::KineticVisual>,
    portrait_rotation: f32,
}

pub(super) fn install(app: &mut App) {
    if std::env::var_os("TERRA_INVENTORY_PORTRAIT_SMOKE").is_some() {
        crate::menu_smoke::install_test_pointer(app);
    }
    app.insert_resource(Gallery {
        hud_only: std::env::var("TERRA_HUD_ONLY_SMOKE").is_ok_and(|value| value == "1"),
        layout_only: std::env::var("TERRA_LAYOUT_ONLY_SMOKE").is_ok_and(|value| value == "1"),
        ..default()
    })
    .add_systems(
        PreUpdate,
        run.before(bevy::picking::PickingSystems::ProcessInput),
    );
}
fn inventory(slots: usize) -> Inventory {
    Inventory {
        slots: vec![ItemStack::EMPTY; slots],
        ..default()
    }
}
fn stack(item: ItemKind, count: u16) -> ItemStack {
    ItemStack { item, count }
}
fn snapshot(surface: Surface, anchor: [i64; 3]) -> Result<ContainerSnapshot, String> {
    let mut snapshot = ContainerSnapshot {
        anchor,
        view: ContainerView::Primary,
        attached_campfire: false,
        workbench: None,
        furnace: None,
        crusher: None,
        mixer: None,
        filter: None,
        distributor: None,
        tunnel_random: None,
        cauldron: None,
        campfire: None,
        halves: vec![ContainerHalf {
            position: anchor,
            inventory: inventory(terra_core::containers::CHEST_SLOTS),
        }],
    };
    match surface {
        Surface::Furnace => {
            let recipe = furnace::recipes()
                .iter()
                .find(|r| r.key.contains("iron"))
                .unwrap_or(&furnace::recipes()[0]);
            let mut items = inventory(furnace::SLOTS);
            items.slots[furnace::INPUT] = stack(recipe.input, 8);
            items.slots[furnace::FUEL] = stack(furnace::fuels()[0].item, 12);
            items.slots[furnace::OUTPUT] = recipe.output;
            items.slots[furnace::BELLOWS] =
                stack(ItemKind::Bellows(terra_core::BellowsKind::Stone), 1);
            snapshot.halves[0].inventory = items;
            snapshot.furnace = Some(furnace::FurnaceView {
                progress: 0.42,
                burn_remaining: 18.0,
                burn_total: 30.0,
                recipe: Some(recipe.id),
                smoke_cells: 0,
                vented: true,
            });
        }
        Surface::Crusher => {
            let recipe = &crusher::recipes()[0];
            let mut items = inventory(crusher::SLOTS);
            items.slots[crusher::INPUT] = stack(recipe.input, 8);
            items.slots[crusher::OUTPUT] = recipe.output;
            snapshot.halves[0].inventory = items;
            snapshot.crusher = Some(crusher::CrusherView {
                progress: 0.56,
                duration: recipe.seconds,
                recipe: Some(recipe.id),
                working: true,
            });
        }
        Surface::Mixer => {
            let target = [170, 90, 45];
            let recipe = mixer::recipe(target);
            let mut items = inventory(mixer::SLOTS);
            for (index, input) in recipe.inputs.iter().enumerate() {
                items.slots[index] = *input;
            }
            items.slots[mixer::OUTPUT] = recipe.output;
            snapshot.halves[0].inventory = items;
            snapshot.mixer = Some(mixer::MixerView {
                target,
                progress: 0.35,
                working: true,
            });
        }
        Surface::Cauldron => {
            let recipe = &brewing::recipes()[0];
            let mut items = inventory(brewing::SLOTS);
            for (index, input) in recipe.inputs.iter().enumerate() {
                items.slots[index] = *input;
            }
            items.slots[brewing::WATER] =
                stack(ItemKind::Tool(terra_core::ToolKind::WaterBucket), 1);
            items.slots[brewing::BOTTLES] = stack(ItemKind::GlassBottle, 6);
            items.slots[brewing::SPOON] =
                stack(ItemKind::Tool(terra_core::ToolKind::StoneSpoon), 1);
            items.slots[brewing::OUTPUT] = recipe.output();
            snapshot.halves[0].inventory = items;
            snapshot.attached_campfire = true;
            snapshot.cauldron = Some(brewing::CauldronView {
                recipe: Some(recipe.id),
                water: 2,
                progress: 0.63,
                heated: true,
                working: true,
            });
        }
        Surface::Campfire => {
            let recipe = &campfire::recipes()[0];
            let mut items = inventory(campfire::SLOTS);
            items.slots[campfire::INPUT] = stack(recipe.input, 4);
            items.slots[campfire::FUEL] = stack(furnace::fuels()[0].item, 8);
            items.slots[campfire::OUTPUT] = recipe.output;
            snapshot.halves[0].inventory = items;
            snapshot.attached_campfire = true;
            snapshot.view = ContainerView::AttachedCampfire;
            snapshot.campfire = Some(campfire::CampfireView {
                lit: true,
                burn_remaining: 12.0,
                burn_total: 24.0,
                recipe: Some(recipe.id),
                progress: 0.48,
            });
        }
        Surface::Chest | Surface::DoubleChest | Surface::Workbench => {
            for (index, material) in [3, 4, 16, 17, 18, 135, 6, 7, 8].into_iter().enumerate() {
                snapshot.halves[0].inventory.slots[index] =
                    stack(ItemKind::Block(material), (index + 1) as u16);
            }
            if surface == Surface::DoubleChest {
                let mut other = inventory(terra_core::containers::CHEST_SLOTS);
                other.slots[0] = stack(ItemKind::GlassBottle, 12);
                other.slots[17] = stack(ItemKind::Block(3), 32);
                snapshot.halves.push(ContainerHalf {
                    position: [anchor[0] + 1, anchor[1], anchor[2]],
                    inventory: other,
                });
            }
            if surface == Surface::Workbench {
                snapshot.workbench = Some([anchor[0] + 1, anchor[1], anchor[2]]);
            }
        }
        Surface::FilteredFunnel
        | Surface::Tunnel
        | Surface::FilteredTunnel
        | Surface::Distributor => {
            snapshot.halves.clear();
            if matches!(surface, Surface::FilteredFunnel | Surface::FilteredTunnel) {
                snapshot.filter = Some(logistics::FilterConfig {
                    item: Some(ItemKind::Block(3)),
                    allowed: true,
                });
            }
            if matches!(surface, Surface::Tunnel | Surface::FilteredTunnel) {
                snapshot.tunnel_random = Some(true);
            }
            if surface == Surface::Distributor {
                snapshot.distributor = Some(logistics::DistributorConfig {
                    filters: [
                        logistics::FilterConfig {
                            item: Some(ItemKind::Block(3)),
                            allowed: true,
                        },
                        logistics::FilterConfig {
                            item: Some(ItemKind::Block(18)),
                            allowed: false,
                        },
                        logistics::FilterConfig::default(),
                    ],
                });
            }
        }
    }
    if !snapshot.valid() {
        return Err(format!("Invalid {} styling snapshot", surface.name()));
    }
    Ok(snapshot)
}
fn set_block(world: &mut World, position: [i64; 3], material: u16) -> Result<(), String> {
    let (section, local) = global_to_section(position);
    let mut chunks = world.resource_mut::<ChunkStore>();
    let section = chunks
        .loaded
        .get_mut(&section)
        .ok_or("Gallery anchor section is not loaded")?;
    section
        .data
        .set_block(local, material)
        .map_err(|error| format!("Gallery anchor: {error:?}"))?;
    Ok(())
}
fn project(world: &mut World, gallery: &mut Gallery) -> Result<(), String> {
    if let Some(scroll) = element(world, "inventory-scroll")
        && let Some(mut offset) = world.get_mut::<bevy::ui::ScrollPosition>(scroll)
    {
        offset.0 = Vec2::ZERO;
    }
    let surface = Surface::ALL[gallery.index];
    set_block(world, gallery.anchor, surface.material())?;
    if surface == Surface::Workbench {
        set_block(
            world,
            [gallery.anchor[0] + 1, gallery.anchor[1], gallery.anchor[2]],
            28,
        )?;
    }
    gallery.projected = Some(snapshot(surface, gallery.anchor)?);
    pin(world, gallery);
    Ok(())
}
fn pin(world: &mut World, gallery: &Gallery) {
    if let Some(snapshot) = &gallery.projected {
        let mut session = world.resource_mut::<ClientSession>();
        if session.container.as_ref() != Some(snapshot) {
            session.container = Some(snapshot.clone());
        }
        if matches!(
            Surface::ALL[gallery.index],
            Surface::Mixer | Surface::Crusher
        ) {
            session
                .kinetics
                .retain(|drive| drive.position != gallery.anchor);
            session
                .kinetics
                .push(terra_network::protocol::session::KineticVisual {
                    position: gallery.anchor,
                    rpm: 32.0,
                    capacity: 16.0,
                    available_capacity: 8.0,
                    stress: 8.0,
                    overloaded: false,
                });
        }
    }
}
fn viewport(world: &mut World) -> Result<Vec2, String> {
    world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|window| window.physical_size().as_vec2())
        .ok_or_else(|| "No gallery window".into())
}
fn rectangle(world: &World, entity: Entity) -> Result<(Vec2, Vec2), String> {
    let node = world
        .get::<ComputedNode>(entity)
        .ok_or("Missing computed geometry")?;
    let transform = world
        .get::<bevy::ui::UiGlobalTransform>(entity)
        .ok_or("Missing UI transform")?;
    let half = node.size() * 0.5;
    let corners = [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(-half.x, half.y),
        Vec2::new(half.x, half.y),
    ]
    .map(|corner| transform.transform_point2(corner));
    Ok(corners.into_iter().fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(low, high), corner| (low.min(corner), high.max(corner)),
    ))
}
fn inside(world: &World, entity: Entity, size: Vec2, label: &str) -> Result<(), String> {
    let (low, high) = rectangle(world, entity)?;
    if (high - low).min_element() < 3.0
        || low.min_element() < -2.0
        || high.cmpgt(size + Vec2::splat(2.0)).any()
    {
        return Err(format!(
            "Surface {label} escapes viewport: {low:?} .. {high:?}, viewport {size:?}"
        ));
    }
    Ok(())
}
fn no_content_scroll(world: &World, entity: Entity, label: &str) -> Result<(), String> {
    let style = world
        .get::<Node>(entity)
        .ok_or("Missing inventory content style")?;
    let geometry = world
        .get::<ComputedNode>(entity)
        .ok_or("Missing inventory content geometry")?;
    if style.overflow.y == bevy::ui::OverflowAxis::Scroll
        || geometry
            .content_size()
            .cmpgt(geometry.size() + Vec2::splat(1.5))
            .any()
    {
        return Err(format!(
            "Inventory content must fit without scrolling: {label}, size={:?}, content={:?}, overflow={:?}",
            geometry.size(),
            geometry.content_size(),
            style.overflow
        ));
    }
    Ok(())
}
fn fixed_cells(world: &mut World, root: Entity, size: Vec2) -> Result<usize, String> {
    if world
        .query::<(Entity, &ElementClasses)>()
        .iter(world)
        .any(|(entity, classes)| {
            descendant_of(world, entity, root)
                && classes.classes.iter().any(|class| class == "item-hotkey")
        })
    {
        return Err("Inventory cells must not contain redundant hotbar key labels".into());
    }
    let layout_width = if world.resource::<InventoryView>().linked_container {
        1044.0
    } else {
        682.0
    };
    let preference = world.resource::<GraphicsConfig>().ui_scale.min(1.0);
    let minimum = (size.y * 0.048).min(size.x * 33.6 / layout_width * 0.88) * preference;
    let result_roots = [
        "inventory-craft-result",
        "crafting-output",
        "furnace-output",
        "crusher-output",
        "mixer-output",
        "cauldron-output",
        "campfire-output",
    ]
    .into_iter()
    .filter_map(|id| element(world, id))
    .collect::<Vec<_>>();
    let equipment_roots = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .filter_map(|(entity, id)| (id.0 == "equipment-slot").then_some(entity))
        .collect::<Vec<_>>();
    let mut cells = 0;
    let mut smallest = f32::INFINITY;
    for (entity, id, geometry) in world
        .query::<(Entity, &ElementId, &ComputedNode)>()
        .iter(world)
    {
        if id.0 != "inventory-cell" || !descendant_of(world, entity, root) {
            continue;
        }
        let logical = geometry.size() * geometry.inverse_scale_factor();
        let result = result_roots
            .iter()
            .any(|result| descendant_of(world, entity, *result));
        let equipment = equipment_roots
            .iter()
            .any(|&root| descendant_of(world, entity, root));
        let expected = Vec2::splat(if equipment {
            25.0
        } else if result {
            38.4
        } else {
            33.6
        });
        if !logical.abs_diff_eq(expected, 0.8 + geometry.inverse_scale_factor()) {
            return Err(format!(
                "{} slot must be {:?} logical pixels, got {logical:?}",
                if result {
                    "Result"
                } else {
                    "Inventory/catalog"
                },
                expected
            ));
        }
        inside(
            world,
            entity,
            size,
            if result {
                "38.4px result"
            } else {
                "33.6px inventory/catalog cell"
            },
        )?;
        let (low, high) = rectangle(world, entity)?;
        let painted = high - low;
        smallest = smallest.min(painted.min_element());
        if (!equipment && painted.min_element() + 1.5 < minimum)
            || (painted.x - painted.y).abs() > 1.5
        {
            return Err(format!(
                "Inventory/catalog slot is too small or stretched on screen: {painted:?}, minimum {minimum}, viewport {size:?}"
            ));
        }
        let transform = *world
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .ok_or("Missing slot picking transform")?;
        let center = (low + high) * 0.5;
        if !geometry.contains_point(transform, center)
            || geometry.contains_point(transform, Vec2::new(high.x + 2.0, center.y))
        {
            return Err("Scaled slot picking no longer matches its painted bounds".into());
        }
        cells += 1;
    }
    if cells < 27 {
        return Err(format!("Fixed layout lost player cells ({cells})"));
    }
    info!(
        ?size,
        cells, smallest, "Responsive inventory painted cells verified"
    );
    Ok(cells)
}
fn trash_bounds(world: &mut World) -> Result<(), String> {
    if !world.resource::<InventoryView>().creative
        || !world.resource::<InventoryView>().browser_visible
    {
        return Ok(());
    }
    let browser = element(world, "item-browser").ok_or("Missing catalog")?;
    let trash = element(world, "inventory-trash").ok_or("Missing trash beside catalog")?;
    let (browser_low, browser_high) = rectangle(world, browser)?;
    let (_, trash_high) = rectangle(world, trash)?;
    let scale = world
        .get::<ComputedNode>(browser)
        .unwrap()
        .inverse_scale_factor()
        / world
            .get::<bevy::ui::UiGlobalTransform>(browser)
            .unwrap()
            .matrix2
            .x_axis
            .length();
    let gap = (browser_low.x - trash_high.x) * scale;
    let bottom = (browser_high.y - trash_high.y) * scale;
    if !(-1.5..=16.0).contains(&gap) || !(-1.5..=16.0).contains(&bottom) {
        return Err(format!(
            "Trash is not directly left/bottom of catalog: horizontal gap={gap}, bottom gap={bottom}"
        ));
    }
    Ok(())
}
fn responsive_bounds(world: &mut World, size: Vec2) -> Result<(), String> {
    let view = world.resource::<InventoryView>();
    let linked = view.linked_container;
    let layout_width = if linked { 1044.0 } else { 682.0 };
    let layout_height = if view.player { 439.2 } else { 480.0 };
    let shell = element(world, "inventory-shell").ok_or("Missing responsive shell")?;
    let (mut low, mut high) = rectangle(world, shell)?;
    if linked {
        let storage = element(world, "workbench-storage").ok_or("Missing attached chest")?;
        let (storage_low, storage_high) = rectangle(world, storage)?;
        low = low.min(storage_low);
        high = high.max(storage_high);
    }
    let preference = world.resource::<GraphicsConfig>().ui_scale.min(1.0);
    let minimum_height =
        (size.y * 0.67).min(size.x * 0.82 * layout_height / layout_width) * preference;
    let painted = high - low;
    if painted.y + 2.0 < minimum_height {
        return Err(format!(
            "Responsive inventory does not fill the available viewport: painted {painted:?}, minimum height {minimum_height}, viewport {size:?}"
        ));
    }
    let titles = world
        .query::<(Entity, &ElementClasses)>()
        .iter(world)
        .filter_map(|(entity, classes)| {
            classes
                .classes
                .iter()
                .any(|class| class == "inventory-reference-title")
                .then_some(entity)
        })
        .collect::<Vec<_>>();
    for title in titles {
        inside(world, title, size, "inventory title")?;
    }
    let center = (low + high) * 0.5;
    let expected_center = size * Vec2::new(0.5, 0.465);
    if (center - expected_center)
        .abs()
        .cmpgt(Vec2::splat(4.0))
        .any()
    {
        return Err(format!(
            "Responsive inventory combination is not centered: center {center:?}, viewport {size:?}"
        ));
    }
    let hud = element(world, "inventory-hud-hotbar").ok_or("Missing responsive HUD hotbar")?;
    inside(world, hud, size, "responsive HUD frame")?;
    let (hud_low, _) = rectangle(world, hud)?;
    if high.y > hud_low.y + 2.0 {
        return Err(format!(
            "Responsive inventory overlaps the HUD frame: inventory bottom {}, hotbar top {}",
            high.y, hud_low.y
        ));
    }
    let hud_parts = world
        .query::<(Entity, &ElementClasses)>()
        .iter(world)
        .filter(|(entity, classes)| {
            descendant_of(world, *entity, hud)
                && classes
                    .classes
                    .iter()
                    .any(|class| matches!(class.as_str(), "reference-hud-slot" | "hud-slot-key"))
        })
        .map(|(entity, classes)| {
            (
                entity,
                classes
                    .classes
                    .iter()
                    .any(|class| class == "reference-hud-slot"),
            )
        })
        .collect::<Vec<_>>();
    let mut hud_slots = 0;
    for (entity, slot) in hud_parts {
        inside(world, entity, size, "responsive HUD slot/key")?;
        if slot {
            let (low, high) = rectangle(world, entity)?;
            if (high - low).min_element() + 1.5 < size.y * 0.029 * preference {
                return Err(format!(
                    "HUD slot is too small on screen: {:?}, viewport {size:?}",
                    high - low
                ));
            }
            hud_slots += 1;
        }
    }
    if hud_slots != 9 {
        return Err(format!("Responsive HUD lost slots: {hud_slots}"));
    }
    info!(
        ?size,
        ?painted,
        linked,
        "Responsive inventory painted bounds verified"
    );
    Ok(())
}
fn player_bounds(world: &mut World) -> Result<(), String> {
    let size = viewport(world)?;
    let view = world.resource::<InventoryView>();
    if !view.open || !view.player || view.items.len() != 18 || view.hotbar.len() != 9 {
        return Err("Player layout must contain 18 backpack and nine hotbar cells".into());
    }
    let root = element(world, "inventory-workspace").ok_or("Missing player workspace")?;
    fixed_cells(world, root, size)?;
    for id in [
        "inventory-shell",
        "inventory-panel",
        "inventory-scroll",
        "inventory-grid",
        "inventory-hotbar",
        "item-browser",
    ] {
        let entity = element(world, id).ok_or_else(|| format!("Missing player layout {id}"))?;
        inside(world, entity, size, id)?;
        if id == "inventory-scroll" {
            no_content_scroll(world, entity, id)?;
        }
    }
    trash_bounds(world)?;
    responsive_bounds(world, size)
}
fn mixer_picker_bounds(world: &mut World) -> Result<(), String> {
    let picker = element(world, "mixer-color").ok_or("Missing mixer color picker")?;
    let preview = world
        .query::<(Entity, &tilt_ui::ControlPart)>()
        .iter(world)
        .find_map(|(entity, part)| {
            (part.owner == picker && part.kind == tilt_ui::ControlPartKind::Preview)
                .then_some(entity)
        })
        .ok_or("Missing mixer color preview")?;
    let (field_low, field_high) = rectangle(world, picker)?;
    let (preview_low, preview_high) = rectangle(world, preview)?;
    let preview_size = preview_high - preview_low;
    let preview_center = (preview_low + preview_high) * 0.5;
    let field_center = (field_low + field_high) * 0.5;
    if preview_size.min_element() < 5.0
        || (preview_size.x - preview_size.y).abs() > 1.5
        || preview_low.cmplt(field_low - Vec2::splat(0.75)).any()
        || preview_high.cmpgt(field_high + Vec2::splat(0.75)).any()
        || (preview_center.y - field_center.y).abs() > 1.5
    {
        return Err(format!(
            "Mixer color preview must be square, contained and vertically centered: field={field_low:?}..{field_high:?}, preview={preview_low:?}..{preview_high:?}"
        ));
    }
    // The editable color text is rendered on the picker itself. Its ::value
    // control part is hidden and cannot describe the painted text or caret.
    let layout = world
        .get::<bevy::text::TextLayoutInfo>(picker)
        .ok_or("Missing native mixer color text layout")?;
    let editor = world
        .get::<bevy::text::EditableText>(picker)
        .ok_or("Missing native mixer color editor")?;
    let geometry = world
        .get::<ComputedNode>(picker)
        .ok_or("Missing native mixer color geometry")?;
    let transform = world
        .get::<bevy::ui::UiGlobalTransform>(picker)
        .ok_or("Missing native mixer color transform")?;
    if layout.glyphs.is_empty() || layout.size.min_element() <= 0.0 {
        return Err("Mixer color text has no painted glyphs".into());
    }
    let origin = geometry.content_box().min - editor.viewport.offset;
    let text_corners = [
        origin,
        origin + Vec2::new(layout.size.x, 0.0),
        origin + Vec2::new(0.0, layout.size.y),
        origin + layout.size,
    ]
    .map(|point| transform.transform_point2(point));
    let (text_low, text_high) = text_corners.into_iter().fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(low, high), point| (low.min(point), high.max(point)),
    );
    let text_center = (text_low + text_high) * 0.5;
    // A line box includes font ascenders and descenders, so allow a small
    // font-relative optical offset without accepting a displaced label.
    let center_tolerance = (text_high.y - text_low.y) * 0.2 + 1.5;
    if text_low.cmplt(field_low - Vec2::splat(0.75)).any()
        || text_high.cmpgt(field_high + Vec2::splat(0.75)).any()
        || text_low.x < preview_high.x + 0.75
        || (text_center.y - preview_center.y).abs() > center_tolerance
    {
        return Err(format!(
            "Native mixer color text must stay inside the field, beside and centered on its preview: field={field_low:?}..{field_high:?}, preview={preview_low:?}..{preview_high:?}, text={text_low:?}..{text_high:?}"
        ));
    }
    info!(
        ?field_low,
        ?field_high,
        ?preview_low,
        ?preview_high,
        ?text_low,
        ?text_high,
        "Native mixer color picker geometry verified"
    );
    Ok(())
}
fn surface_bounds(world: &mut World, surface: Surface) -> Result<(), String> {
    let size = viewport(world)?;
    let mut roots = vec![
        "inventory-shell",
        "inventory-panel",
        "inventory-grid",
        "inventory-hotbar",
        surface.workspace(),
    ];
    if surface == Surface::Workbench {
        roots.extend([
            "workbench-storage",
            "workbench-container-grid",
            "crafting-grid",
            "crafting-output",
        ]);
    }
    if surface == Surface::FilteredTunnel {
        roots.push("funnel-filter-workspace");
    }
    if world.resource::<InventoryView>().player || !world.resource::<InventoryView>().open {
        return Err(format!(
            "{} did not select its station layout",
            surface.name()
        ));
    }
    if surface == Surface::Tunnel
        && (world.resource::<InventoryView>().filtered_funnel
            || element(world, "funnel-filter-workspace").is_some())
    {
        return Err("Unfiltered tunnel incorrectly exposes a filter".into());
    }
    let shell = element(world, "inventory-shell").ok_or("Missing station shell")?;
    let workspace = element(world, "inventory-workspace").ok_or("Missing station workspace")?;
    for id in roots {
        let entity = element(world, id).ok_or_else(|| format!("{} lacks {id}", surface.name()))?;
        inside(world, entity, size, id)?;
    }
    let content = element(world, "inventory-scroll").ok_or("Missing station content")?;
    no_content_scroll(world, content, "inventory-scroll")?;
    fixed_cells(world, workspace, size)?;
    trash_bounds(world)?;
    if surface == Surface::Mixer {
        mixer_picker_bounds(world)?;
    }
    if surface == Surface::Workbench {
        let storage =
            element(world, "workbench-storage").ok_or("Missing left-hand attached chest")?;
        let (_, storage_high) = rectangle(world, storage)?;
        let (shell_low, _) = rectangle(world, shell)?;
        if storage_high.x > shell_low.x + 1.5 {
            return Err("Attached chest must be in its own panel left of the main frame".into());
        }
        no_content_scroll(world, storage, "attached chest")?;
    }
    let view = world.resource::<InventoryView>();
    if view.items.len() != 18 || view.hotbar.len() != 9 {
        return Err("Station lost the 18 + 9 player slots".into());
    }
    if surface == Surface::DoubleChest && view.container_items.len() != 36 {
        return Err("Double chest lost a physical inventory half".into());
    }
    if surface == Surface::Workbench
        && (!view.linked_container || !view.assembly || view.container_items.len() != 18)
    {
        return Err("Linked workbench did not retain crafting and chest storage".into());
    }
    if surface == Surface::Workbench {
        let crafting = world.resource::<terra_ui::CraftingView>();
        if !crafting.workbench
            || crafting.grid.len() != 9
            || crafting.message.contains("no longer available")
        {
            return Err(format!(
                "Linked workbench is unavailable or lacks its 3x3 grid: workbench={} cells={} message={}",
                crafting.workbench,
                crafting.grid.len(),
                crafting.message
            ));
        }
    }
    responsive_bounds(world, size)
}
fn debug_bounds(world: &mut World) -> Result<(), String> {
    let size = viewport(world)?;
    let root =
        element(world, "debug-overlay").ok_or("Native F1 did not produce the debug overlay")?;
    inside(world, root, size, "debug-overlay")?;
    if world.get::<ComputedNode>(root).is_some_and(|node| {
        let border = node.border();
        (border.min_inset + border.max_inset).element_sum() > 0.1
    }) || world
        .get::<BackgroundColor>(root)
        .is_some_and(|background| background.0.alpha() > 0.01)
    {
        return Err("F1 debug display still has a border or panel background".into());
    }
    let mut diagnostic_rows = 0;
    for (entity, text, node, visible, shadow) in world
        .query::<(
            Entity,
            &Text,
            &ComputedNode,
            &InheritedVisibility,
            Option<&TextShadow>,
        )>()
        .iter(world)
    {
        if !descendant_of(world, entity, root)
            || !visible.get()
            || node.is_empty()
            || text.0.trim().is_empty()
        {
            continue;
        }
        let valid_shadow = shadow.is_some_and(|shadow| {
            let color = shadow.color.to_srgba();
            shadow.offset.distance_squared(Vec2::splat(2.0)) < 0.0001
                && color.red.abs() < 0.001
                && color.green.abs() < 0.001
                && color.blue.abs() < 0.001
                && color.alpha > 0.99
        });
        if !valid_shadow {
            return Err(format!(
                "Rendered F1 text lacks its black (2, 2) shadow: text={:?}, shadow={shadow:?}",
                text.0
            ));
        }
        if text.0.starts_with("FPS:") {
            diagnostic_rows += 1;
        }
    }
    if diagnostic_rows == 0 {
        return Err("F1 shadow check found no rendered FPS diagnostic row".into());
    }
    let view = world.resource::<terra_ui::DebugOverlay>();
    if !view.visible
        || view.xyz.is_empty()
        || view.frame_ms.is_empty()
        || view.facing.is_empty()
        || view.render_sections.is_empty()
        || view.world_time.is_empty()
        || view.weather.is_empty()
    {
        return Err("F1 diagnostic projection lacks actual session data".into());
    }
    if let Some(confirmed) = world.resource::<ClientSession>().confirmed_state() {
        let displayed = view
            .xyz
            .split_whitespace()
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "F1 coordinates are not numeric world coordinates")?;
        if displayed.len() != 3
            || Vec3::from_slice(&displayed).distance(Vec3::from_array(confirmed.position)) > 0.5
        {
            return Err(format!(
                "F1 coordinates differ from the authoritative player: {} / {:?}",
                view.xyz, confirmed.position
            ));
        }
    }
    if element(world, "target-card").is_some() {
        return Err("Target card overlaps the F1 diagnostics".into());
    }
    Ok(())
}
fn target_bounds(world: &mut World) -> Result<(), String> {
    let size = viewport(world)?;
    let target =
        element(world, "target-card").ok_or("Actual ground target did not produce its HUD")?;
    inside(world, target, size, "target-card")
}
fn aim_at_ground(world: &mut World) {
    let eye = world
        .resource::<ClientSession>()
        .confirmed_state()
        .map(|player| {
            Vec3::from_array(player.position) + Vec3::Y * terra_player::PLAYER_EYE_OFFSET
        });
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        if let Some(eye) = eye {
            camera.translation = eye;
        }
        camera.rotation = Quat::from_rotation_x(-0.9);
    }
}
fn restore(world: &mut World, gallery: &mut Gallery) -> Result<(), String> {
    for (position, material) in gallery.saved_blocks.drain(..) {
        set_block(world, position, material)?;
    }
    world.resource_mut::<ClientSession>().kinetics = gallery.kinetics.clone();
    Ok(())
}
fn baseline(world: &World, state: &Inspection) -> Result<(), String> {
    if state.baseline.as_ref() != Some(&world.resource::<ClientSession>().inventory) {
        return Err(format!(
            "Projection gallery changed the authoritative player inventory: before={:?}, after={:?}",
            state.baseline,
            world.resource::<ClientSession>().inventory
        ));
    }
    Ok(())
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        world.resource_scope(|world, mut gallery: Mut<Gallery>| {
            if state.stage == u8::MAX { return; }
            let result = (|| -> Result<bool, String> {
                if state.start.elapsed() > Duration::from_secs(300) { return Err(format!("Surfaces gallery timeout at stage {} / {:?}", state.stage, Surface::ALL[gallery.index])); }
                if state.failed { return Err("Surface screenshot is blank or has the wrong physical dimensions".into()); }
                if gallery.diagnostic_at.is_none_or(|at| at.elapsed() >= Duration::from_secs(5)) {
                    gallery.diagnostic_at = Some(Instant::now());
                    let previews = previews_ready(world);
                    if state.stage >= 5 {
                        let cameras = world.query_filtered::<(Entity, &Transform, Option<&GlobalTransform>, Option<&terra_player::CameraLook>), With<terra_player::TerraCamera>>()
                            .iter(world).map(|(entity, local, global, look)| (entity, local.translation, local.rotation,
                                global.map(GlobalTransform::translation), look.map(terra_player::CameraLook::rotation))).collect::<Vec<_>>();
                        info!(input=?world.resource::<CameraInputState>(), blocked=world.resource::<PlayerInputBlock>().0,
                            target=?world.resource::<terra_player::TargetWorld>().0,
                            target_visible=world.resource::<terra_ui::TargetView>().visible,
                            confirmed=?world.resource::<ClientSession>().confirmed_state().map(|player| player.position), ?cameras,
                            "Native HUD input and picking diagnostics");
                    }
                    info!(stage=state.stage, surface=Surface::ALL[gallery.index].name(), compact=gallery.compact,
                        seconds=state.start.elapsed().as_secs(), page=?world.resource::<MenuState>().page,
                        session_ready=world.resource::<ClientSession>().ready,
                        render_ready=world.resource::<terra_render::RenderStatus>().ready,
                        player_preview=world.resource::<terra_render::PlayerPreviewImage>().ready,
                        prewarming=world.resource::<InventoryView>().prewarming, previews_ready=previews,
                        pending_previews=world.resource::<terra_render::RenderBlockPreviews>().pending,
                        loaded_sections=world.resource::<ChunkStore>().loaded.len(), anchor=?gallery.anchor,
                        anchor_loaded=world.resource::<ChunkStore>().block_at(gallery.anchor).is_some(),
                        captured=state.captured, "Native surfaces gallery readiness");
                }
                pin(world, &gallery);
                if let Some(code) = state.key_release {
                    if state.since.elapsed() > Duration::from_millis(180) {
                        os_pointer(world, &[tilt_ui::serde_json::json!({"key":[code,0]})])?;
                        state.key_release = None;
                        state.since = Instant::now();
                    }
                    return Ok(false);
                }
                if state.stage != 0 && state.since.elapsed() < Duration::from_millis(1000) { return Ok(false); }
                match state.stage {
                    0 => {
                        if !world.resource::<ClientSession>().ready || !world.resource::<terra_render::RenderStatus>().ready
                            || !world.resource::<terra_render::PlayerPreviewImage>().ready
                            || world.resource::<terra_render::RenderBlockPreviews>().pending != 0 { return Ok(false); }
                        if world.resource::<MenuState>().page != MenuPage::Playing {
                            world.resource_mut::<MenuState>().session_active = true;
                            world.write_message(MenuAction::Command("resume".into()));
                            return Ok(false);
                        }
                        if world.resource::<InventoryView>().prewarming { return Ok(false); }
                        if gallery.hud_only && !gallery.layout_only {
                            gallery.compact = true;
                            gallery.kinetics = world.resource::<ClientSession>().kinetics.clone();
                            state.baseline = Some(world.resource::<ClientSession>().inventory.clone());
                            resize_reference(world, (1024,640), true);
                            world.write_message(InventoryAction::Toggle);
                            advance(&mut state, 13);
                            return Ok(false);
                        }
                        let confirmed = world.resource::<ClientSession>().confirmed_state()
                            .map(|player| Vec3::from_array(player.position));
                        let position = if let Some(player) = confirmed { player } else {
                            world.query_filtered::<&Transform, With<terra_player::TerraCamera>>().iter(world)
                                .next().map(|transform| transform.translation).ok_or("No confirmed gallery player/camera")?
                        };
                        let base = position.to_array().map(|coordinate| coordinate.floor() as i64);
                        let chunks = world.resource::<ChunkStore>();
                        let near_loaded = |anchor: &[i64; 3]| {
                            let adjacent = [anchor[0] + 1, anchor[1], anchor[2]];
                            chunks.block_at(*anchor).is_some() && chunks.block_at(adjacent).is_some()
                                && Vec3::from_array(adjacent.map(|coordinate| coordinate as f32 + 0.5)).distance(position) < 7.0
                        };
                        // PlayerBody transforms can be child-local. Keep the projected workbench near the real player.
                        let anchor = [[2,-1,0], [2,-2,0], [1,-1,0], [0,-1,0], [0,-2,0], [-2,-1,0], [0,-3,0]]
                            .map(|offset| std::array::from_fn(|axis| base[axis] + offset[axis]))
                            .into_iter().find(near_loaded).or_else(|| {
                                let (focus, _) = global_to_section(base);
                                chunks.loaded.keys().map(|section| [section.x * 16 + 8, section.y * 16 + 8, section.z * 16 + 8])
                                    .filter(near_loaded).min_by_key(|anchor| {
                                        let (section, _) = global_to_section(*anchor);
                                        section.x.abs_diff(focus.x) + section.y.abs_diff(focus.y) + section.z.abs_diff(focus.z)
                                    })
                            });
                        let Some(anchor) = anchor else { return Ok(false); };
                        gallery.anchor = anchor;
                        let adjacent = [gallery.anchor[0] + 1, gallery.anchor[1], gallery.anchor[2]];
                        let saved = [gallery.anchor, adjacent].map(|position|
                            world.resource::<ChunkStore>().block_at(position).map(|material| (position, material)));
                        if saved.iter().any(Option::is_none) { return Ok(false); }
                        gallery.saved_blocks = saved.into_iter().flatten().collect();
                        gallery.kinetics = world.resource::<ClientSession>().kinetics.clone();
                        state.baseline = Some(world.resource::<ClientSession>().inventory.clone());
                        resize_reference(world, wide_reference_size(), true);
                        if std::env::var_os("TERRA_INVENTORY_NOTICE").is_some() {
                            world.write_message(terra_world::streaming::BlockEditRequest {
                                position: [990_000, 0, 990_000], expected_material: 0, material: 3,
                            });
                        }
                        world.write_message(InventoryAction::Toggle);
                        advance(&mut state, 1);
                    }
                    1 => {
                        if !size_ready(world, wide_reference_size()) || !world.resource::<InventoryView>().open { return Ok(false); }
                        if gallery.layout_only {
                            if !previews_ready(world) { return Ok(false); }
                            player_bounds(world)?;
                            if std::env::var_os("TERRA_INVENTORY_NOTICE").is_some() {
                                let toast = element(world, "network-error-toast").ok_or("Missing error toast")?;
                                if !world.get::<tilt_ui::ToastState>(toast).is_some_and(|toast| toast.open) {
                                    return Err("Rejected edit did not open the error toast".into());
                                }
                            }
                            shot(world, if std::env::var_os("TERRA_INVENTORY_NOTICE").is_some() {
                                "network-error-toast"
                            } else { "fixed-player-wide" })?;
                            advance(&mut state, 15);
                        } else {
                            project(world, &mut gallery)?;
                            advance(&mut state, 2);
                        }
                    }
                    2 => {
                        if !previews_ready(world) { return Ok(false); }
                        baseline(world, &state)?;
                        let surface = Surface::ALL[gallery.index];
                        surface_bounds(world, surface)?;
                        shot(world, &format!("surface-{}-{}", surface.name(), if gallery.compact { "compact" } else { "wide" }))?;
                        let inspect_picker = surface == Surface::Mixer
                            && std::env::var("TERRA_PICKER_SMOKE").is_ok_and(|value| value == "1");
                        let inspect_linked = surface == Surface::Workbench && std::env::var_os("TERRA_INVENTORY_PORTRAIT_SMOKE").is_some();
                        advance(&mut state, if inspect_picker { 16 } else if inspect_linked { 40 } else { 3 });
                    }
                    16 => {
                        // The preceding closed-picker screenshot gets a full
                        // frame before the real palette is opened.
                        let picker = element(world, "mixer-color").ok_or("Missing mixer color picker before opening palette")?;
                        tilt_ui::set_color_picker_open(world, picker, true);
                        advance(&mut state, 17);
                    }
                    17 => {
                        let picker = element(world, "mixer-color").ok_or("Missing open mixer color picker")?;
                        if !world.get::<tilt_ui::ColorPickerState>(picker).is_some_and(|picker| picker.open) {
                            return Err("Mixer color palette did not open".into());
                        }
                        let popup = world.query::<(Entity, &tilt_ui::ControlPart)>().iter(world)
                            .find_map(|(entity, part)| (part.owner == picker && part.kind == tilt_ui::ControlPartKind::Popup).then_some(entity))
                            .ok_or("Missing mixer color palette geometry")?;
                        let size = viewport(world)?;
                        inside(world, popup, size, "mixer color palette")?;
                        mixer_picker_bounds(world)?;
                        baseline(world, &state)?;
                        shot(world, &format!("surface-mixer-picker-open-{}", if gallery.compact { "compact" } else { "wide" }))?;
                        advance(&mut state, 18);
                    }
                    18 => {
                        let picker = element(world, "mixer-color").ok_or("Missing mixer color picker before closing palette")?;
                        tilt_ui::set_color_picker_open(world, picker, false);
                        baseline(world, &state)?;
                        advance(&mut state, 3);
                    }
                    3 => {
                        if gallery.index + 1 < Surface::ALL.len() {
                            gallery.index += 1;
                            project(world, &mut gallery)?;
                            advance(&mut state, 2);
                        } else if !gallery.compact {
                            gallery.index = 0;
                            gallery.compact = true;
                            if gallery.layout_only {
                                gallery.projected = None;
                                world.resource_mut::<ClientSession>().container = None;
                            }
                            resize_reference(world, (1024,640), false);
                            advance(&mut state, 4);
                        } else if gallery.layout_only {
                            let picker_captures = if std::env::var("TERRA_PICKER_SMOKE").is_ok_and(|value| value == "1") { 2 } else { 0 };
                            if state.captured < Surface::ALL.len() * 2 + 2 + picker_captures { return Ok(false); }
                            gallery.projected = None;
                            world.resource_mut::<ClientSession>().container = None;
                            baseline(world, &state)?;
                            restore(world, &mut gallery)?;
                            info!(captures=state.captured, "Native inventory-layout191 passed: 24 station views plus two player views, no redundant inventory hotkeys, 33.6px logical cells and 38.4px results, readable painted cells and viewport fill, no content scroll, left-hand attached chest, trash beside catalog, centered composite, HUD fully visible, unchanged authoritative inventory, no OS input");
                            return Ok(true);
                        } else {
                            // Mouse.move activates the native TERAMIC window; a key-only uinput command does not.
                            let shell = element(world, "inventory-shell").ok_or("Missing station shell before native E")?;
                            pointer(world, shell, None)?;
                            advance(&mut state, 12);
                        }
                    }
                    4 => {
                        if !size_ready(world, (1024,640)) { return Ok(false); }
                        if gallery.layout_only {
                            if !world.resource::<InventoryView>().open {
                                world.write_message(InventoryAction::Toggle);
                                state.since = Instant::now();
                                return Ok(false);
                            }
                            if !world.resource::<InventoryView>().player || !previews_ready(world) { return Ok(false); }
                            player_bounds(world)?;
                            shot(world, "fixed-player-compact")?;
                            advance(&mut state, if std::env::var_os("TERRA_INVENTORY_PORTRAIT_SMOKE").is_some() { 30 } else { 15 });
                        } else {
                            project(world, &mut gallery)?;
                            advance(&mut state, 2);
                        }
                    }
                    40 => {
                        let view=world.resource::<InventoryView>();
                        info!(ready=view.sort_ready, open=view.open, sortable=view.storage_sortable, "Linked sorting pointer preflight");
                        crate::menu_smoke::pointer(world,"sort-linked-menu","",true)?;
                        advance(&mut state,41);
                    }
                    41 => {
                        crate::menu_smoke::pointer(world,"sort-linked-menu","",false)?;
                        advance(&mut state,42);
                    }
                    42 => {
                        let size=viewport(world)?;
                        let popup=element(world,"inventory-sort-linked-options").ok_or_else(||format!("Linked storage dropup did not open: ready={} projected_open={}",world.resource::<InventoryView>().sort_ready,world.resource::<InventoryView>().sort_storage_open))?;
                        inside(world,popup,size,"linked sorting dropup")?;
                        let dismisses:Vec<_>=world.query::<(Entity,&ElementId)>().iter(world).filter_map(|(entity,id)|(id.0=="inventory-sort-dismiss").then_some(entity)).collect();
                        if dismisses.is_empty() { return Err("Missing linked-menu dismiss layer".into()); }
                        let option=element(world,"sort-linked-name").ok_or("Missing linked A-Z option")?;
                        let stack=&world.resource::<bevy::ui::UiStack>().uinodes;
                        if dismisses.iter().any(|dismiss|stack.iter().position(|e|*e==option)<=stack.iter().position(|e|e==dismiss)) { return Err("Linked sort choices are blocked by a dismiss layer".into()); }
                        baseline(world,&state)?;
                        shot(world,&format!("linked-sort-dropup-{}",if gallery.compact {"compact"}else{"wide"}))?;
                        crate::menu_smoke::pointer(world,"sort-linked-menu","",true)?;
                        advance(&mut state,43);
                    }
                    43 => {
                        crate::menu_smoke::pointer(world,"sort-linked-menu","",false)?;
                        advance(&mut state,44);
                    }
                    44 => {
                        if element(world,"inventory-sort-linked-options").is_some() {return Err("Linked sorting menu did not close".into());}
                        info!(compact=gallery.compact,"Native linked sorting dropup stack and pointer toggle passed");
                        advance(&mut state,3);
                    }
                    30 => {
                        gallery.portrait_rotation = world.resource::<terra_ui::InventoryPortraitRotation>().rotation;
                        crate::menu_smoke::pointer(world, "inventory-character-preview", "", true)?;
                        let window = world.query_filtered::<Entity,With<PrimaryWindow>>().single(world).map_err(|e|e.to_string())?;
                        world.write_message(bevy::input::mouse::MouseButtonInput { button: MouseButton::Left, state: bevy::input::ButtonState::Pressed, window });
                        advance(&mut state,31);
                    }
                    31 => {
                        crate::menu_smoke::move_test_pointer(world, Vec2::new(70.0,0.0))?;
                        advance(&mut state,32);
                    }
                    32 => {
                        let window = world.query_filtered::<Entity,With<PrimaryWindow>>().single(world).map_err(|e|e.to_string())?;
                        world.write_message(bevy::input::mouse::MouseButtonInput { button: MouseButton::Left, state: bevy::input::ButtonState::Released, window });
                        crate::menu_smoke::pointer(world, "inventory-character-preview", "", false)?;
                        let pose = *world.resource::<terra_ui::InventoryPortraitRotation>();
                        if !pose.active || (pose.rotation-gallery.portrait_rotation).abs()<0.1 { return Err("Inventory character did not rotate after captured pointer drag".into()); }
                        if !world.resource::<terra_render::RenderInventoryPortraitImage>().ready { return Err("Rotated inventory model has no live render target".into()); }
                        baseline(world,&state)?;
                        shot(world,"player-portrait-rotated-compact")?;
                        info!(rotation=pose.rotation,"Native inventory portrait drag passed");
                        advance(&mut state,33);
                    }
                    33 => {
                        crate::menu_smoke::pointer(world,"sort-player-menu","",true)?;
                        advance(&mut state,34);
                    }
                    34 => {
                        crate::menu_smoke::pointer(world,"sort-player-menu","",false)?;
                        advance(&mut state,35);
                    }
                    35 => {
                        let size=viewport(world)?;
                        for id in ["inventory-sort-player-options","sort-player-name","sort-player-id","sort-player-quantity"] {
                            let entity=element(world,id).ok_or_else(||format!("Sort dropup is missing {id}"))?;
                            inside(world,entity,size,id)?;
                        }
                        baseline(world,&state)?;
                        shot(world,"player-sort-dropup-compact")?;
                        crate::menu_smoke::pointer(world,"sort-player-menu","",true)?;
                        advance(&mut state,36);
                    }
                    36 => {
                        crate::menu_smoke::pointer(world,"sort-player-menu","",false)?;
                        advance(&mut state,37);
                    }
                    37 => {
                        if element(world,"inventory-sort-player-options").is_some() { return Err("Sort dropup did not close after second funnel click".into()); }
                        baseline(world,&state)?;
                        info!("Native sorting dropup bounds and toggle passed");
                        advance(&mut state,15);
                    }
                    15 => {
                        project(world, &mut gallery)?;
                        advance(&mut state, 2);
                    }
                    13 => {
                        if !size_ready(world, (1024,640)) || !world.resource::<InventoryView>().open || !previews_ready(world) { return Ok(false); }
                        let shell = element(world, "inventory-shell").ok_or("Missing player inventory before native E")?;
                        pointer(world, shell, None)?;
                        advance(&mut state, 12);
                    }
                    12 => {
                        if !world.query_filtered::<&Window, With<PrimaryWindow>>().iter(world).next().is_some_and(|window| window.focused) {
                            return Err("Native pointer did not focus TERAMIC before E".into());
                        }
                        native_key(world, &mut state, 18)?;
                        advance(&mut state, 5);
                    }
                    5 => {
                        if world.resource::<InventoryView>().open { return Err("Native E did not close the station inventory".into()); }
                        baseline(world, &state)?;
                        // The server never owned this styling-only container. Release it only after native E really closed the UI.
                        gallery.projected = None;
                        world.resource_mut::<ClientSession>().container = None;
                        restore(world, &mut gallery)?;
                        // Inventory smoke startup intentionally disables input acceptance. Enable the normal capture path for these HUD checks.
                        world.insert_resource(CameraInputState::default());
                        world.write_message(terra_player::CaptureRequest(true));
                        aim_at_ground(world);
                        advance(&mut state, 6);
                    }
                    6 => {
                        if !world.resource::<CameraInputState>().captured || !world.resource::<terra_ui::TargetView>().visible {
                            if state.since.elapsed() < Duration::from_secs(8) { return Ok(false); }
                            return Err("Gameplay did not regain capture/a real ground target after E".into());
                        }
                        target_bounds(world)?;
                        shot(world, "surface-target-compact")?;
                        native_key(world, &mut state, 59)?;
                        advance(&mut state, 7);
                    }
                    7 => {
                        debug_bounds(world)?;
                        shot(world, "surface-debug-compact")?;
                        native_key(world, &mut state, 59)?;
                        advance(&mut state, 8);
                    }
                    8 => {
                        if world.resource::<terra_core::DebugSettings>().enabled { return Err("Native F1 did not hide diagnostics".into()); }
                        resize_reference(world, wide_reference_size(), false);
                        advance(&mut state, 9);
                    }
                    9 => {
                        if !size_ready(world, wide_reference_size()) { return Ok(false); }
                        target_bounds(world)?;
                        shot(world, "surface-target-wide")?;
                        native_key(world, &mut state, 59)?;
                        advance(&mut state, 10);
                    }
                    10 => {
                        debug_bounds(world)?;
                        shot(world, "surface-debug-wide")?;
                        native_key(world, &mut state, 59)?;
                        advance(&mut state, 11);
                    }
                    _ => {
                        let picker_captures = if !gallery.hud_only && std::env::var("TERRA_PICKER_SMOKE").is_ok_and(|value| value == "1") { 2 } else { 0 };
                        let expected = if gallery.hud_only { 4 } else { Surface::ALL.len() * 2 + 4 + picker_captures };
                        if state.captured < expected { return Ok(false); }
                        baseline(world, &state)?;
                        if world.resource::<InventoryView>().open || world.resource::<terra_core::DebugSettings>().enabled { return Err("Native close/toggle cleanup failed".into()); }
                        info!(captures=state.captured, hud_only=gallery.hud_only, "Native surfaces188 passed: validated projection-only stations, 28px cells and 38.4px results, no content scroll, viewport bounds, 18+9 player slots, unfiltered tunnel without filter, linked workbench, unchanged authoritative inventory, actual E/F1, borderless diagnostics and target HUD at both sizes");
                        return Ok(true);
                    }
                }
                Ok(false)
            })();
            match result {
                Ok(true) => { world.write_message(AppExit::Success); state.stage = u8::MAX; }
                Ok(false) => {}
                Err(error) => {
                    gallery.projected = None;
                    let _ = restore(world, &mut gallery);
                    error!(%error, "Native surfaces gallery failed");
                    world.write_message(AppExit::error());
                    state.stage = u8::MAX;
                }
            }
        });
    });
}
