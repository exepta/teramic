use bevy::{
    prelude::*,
    sprite::{BorderRect, TextureSlicer},
    ui::widget::{ImageNode, NodeImageMode},
};
use tilt_ui::{ElementClasses, ElementId};

#[derive(Component, PartialEq)]
pub(crate) struct CatalogGridCapacity(pub usize);

pub(crate) fn install(app: &mut App) {
    app.add_systems(
        PostUpdate,
        (
            nine_slice,
            inventory_aspect,
            inventory_slot_size,
            catalog_slot_size,
            content_density,
            responsive_inventory_scale,
        )
            .chain()
            .before(bevy::ui::UiSystems::Prepare),
    );
}

// Keep the compact layout in logical pixels and enlarge the complete workspace.
// Window::size already removes OS DPI; UiScale is removed from this local zoom
// so neither DPI nor the user's scale preference is applied twice.
fn responsive_inventory_scale(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    graphics: Res<terra_core::GraphicsConfig>,
    ui_scale: Res<UiScale>,
    view: Res<crate::InventoryView>,
    mut roots: Query<(&ElementId, &ComputedNode, &mut UiTransform)>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("ui.frames.responsive_inventory_scale");
    let Some(window) = windows.iter().next() else {
        return;
    };
    let viewport = window.size();
    for (id, geometry, mut transform) in &mut roots {
        let zoom = match id.0.as_str() {
            "inventory-workspace" => inventory_zoom(
                viewport,
                graphics.ui_scale,
                ui_scale.0,
                view.linked_container,
            ),
            "inventory-hud" => hotbar_zoom(viewport, graphics.ui_scale, ui_scale.0),
            _ => continue,
        };
        if transform.scale != Vec2::splat(zoom) {
            transform.scale = Vec2::splat(zoom);
        }
        if id.0 == "inventory-workspace" {
            // Leave room for the larger HUD hotbar below the open inventory.
            let offset = Val::Px(-viewport.y * 0.05 / ui_scale.0.max(0.1));
            if transform.translation.y != offset {
                transform.translation.y = offset;
            }
        }
        if id.0 == "inventory-hud" {
            // Scale around the center while keeping the bottom rail and key
            // labels above the lower screen edge, including a selected name.
            let height = geometry.size().y * geometry.inverse_scale_factor();
            let offset = Val::Px(-(zoom - 1.0) * (height * 0.5 + 24.0));
            if transform.translation.y != offset {
                transform.translation.y = offset;
            }
        }
    }
}

pub(crate) fn inventory_zoom(viewport: Vec2, requested: f32, global: f32, linked: bool) -> f32 {
    let width = if linked { 1044.0 } else { 682.0 };
    // The extra 18 pixels include the title above the 400px machine frame.
    let desired =
        (viewport.x * 0.94 / width).min(viewport.y * 0.77 / 418.0) * requested.clamp(0.75, 1.5);
    let fit = (viewport.x * 0.96 / width).min(viewport.y * 0.82 / 418.0);
    desired.min(fit).max(0.1) / global.max(0.1)
}

fn hotbar_zoom(viewport: Vec2, requested: f32, global: f32) -> f32 {
    let desired = (viewport.y / 560.0).min(viewport.x * 0.90 / 352.0) * requested.clamp(0.75, 1.5);
    0.7 * desired.min(viewport.x * 0.94 / 352.0).max(0.1) / global.max(0.1)
}

// Reduce display density of labels/glyphs on large screens independently of
// the inventory cells. Value, placeholder and caret share their control scale.
#[allow(clippy::type_complexity)]
fn content_density(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    parents: Query<(
        Option<&ElementId>,
        Option<&tilt_ui::TiltElement>,
        Option<&ChildOf>,
    )>,
    mut nodes: Query<
        (
            Entity,
            Option<&TextFont>,
            Option<&tilt_ui::TiltElement>,
            Option<&tilt_ui::ControlPart>,
            &mut UiTransform,
        ),
        With<Node>,
    >,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("ui.frames.content_density");
    let Some(window) = windows.iter().next() else {
        return;
    };
    let factor = (1080.0 / window.physical_height().max(1) as f32)
        .sqrt()
        .min(1.0);
    for (entity, font, element, part, mut transform) in &mut nodes {
        if font.is_none()
            && !element.is_some_and(|e| {
                matches!(
                    e.kind,
                    tilt_ui::ElementKind::Icon
                        | tilt_ui::ElementKind::Input
                        | tilt_ui::ElementKind::TextArea
                )
            })
        {
            continue;
        }
        // Scale the input as one unit so text, caret, selection and picking
        // retain the same coordinate system. Its surrounding click target stays.
        if part.is_some() {
            continue;
        }
        let mut ancestor = Some(entity);
        let mut inventory = false;
        let mut nested_control = false;
        while let Some(current) = ancestor {
            let Ok((id, element, parent)) = parents.get(current) else {
                break;
            };
            if current != entity
                && element.is_some_and(|e| {
                    matches!(
                        e.kind,
                        tilt_ui::ElementKind::Input
                            | tilt_ui::ElementKind::TextArea
                            | tilt_ui::ElementKind::ChoiceBox
                            | tilt_ui::ElementKind::ColorPicker
                    )
                })
            {
                nested_control = true;
            }
            if id.is_some_and(|id| matches!(id.0.as_str(), "inventory-workspace" | "inventory-hud"))
            {
                inventory = true;
                break;
            }
            ancestor = parent.map(ChildOf::parent);
        }
        if inventory && !nested_control && transform.scale != Vec2::splat(factor) {
            transform.scale = Vec2::splat(factor);
        }
        if inventory
            && !nested_control
            && element.is_some_and(|e| {
                matches!(
                    e.kind,
                    tilt_ui::ElementKind::Input
                        | tilt_ui::ElementKind::TextArea
                        | tilt_ui::ElementKind::ChoiceBox
                        | tilt_ui::ElementKind::ColorPicker
                )
            })
        {
            // Color pickers have an intrinsic width in a centered row. Keep
            // their chip and editable value centered together as they shrink.
            let left = if element.is_some_and(|e| e.kind == tilt_ui::ElementKind::ColorPicker) {
                Val::Px(0.0)
            } else {
                Val::Percent(-50.0 * (1.0 - factor))
            };
            if transform.translation.x != left {
                transform.translation.x = left;
            }
        }
    }
}

fn inventory_aspect(mut nodes: Query<(&ElementClasses, &mut Node)>) {
    let _scope = terra_core::profiling::ScopeTimer::new("ui.frames.inventory_aspect");
    for (classes, mut node) in &mut nodes {
        let has = |name| classes.classes.iter().any(|class| class == name);
        let ratio = if has("inventory-shell") {
            if node.aspect_ratio.is_some() {
                node.aspect_ratio = None;
            }
            continue;
        } else if has("square-player-slot") {
            1.0
        } else {
            continue;
        };
        if node.aspect_ratio != Some(ratio) {
            node.aspect_ratio = Some(ratio);
        }
    }
}

// Fixed logical pixels; workspace zoom handles the visible size independently.
fn inventory_slot_size(
    mut nodes: Query<(
        Option<&ElementId>,
        &ElementClasses,
        &ComputedNode,
        &mut Node,
    )>,
    view: Res<crate::InventoryView>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("ui.frames.inventory_slot_size");
    let cell = 33.6;
    for (id, classes, computed, mut node) in &mut nodes {
        // Whole physical pixels keep all rows/columns square before local zoom.
        let inverse = computed.inverse_scale_factor().max(0.01);
        let cell = (cell / inverse).round() * inverse;
        let id = id.map_or("", |id| id.0.as_str());
        let has = |name| classes.classes.iter().any(|class| class == name);
        let grid = match id {
            "inventory-grid" => Some((9, 2)),
            "inventory-hotbar" => Some((9, 1)),
            "container-grid" => Some((9, view.container_items.len().div_ceil(9).max(1))),
            "workbench-container-grid" => Some((9, view.container_items.len().div_ceil(9).max(1))),
            "inventory-crafting" => Some((2, 2)),
            "crafting-grid" => Some((3, 3)),
            _ if has("mixer-input") => Some((3, 2)),
            _ if has("alchemy-inputs") => Some((3, 1)),
            _ => None,
        };
        let single = matches!(id, "inventory-craft-result" | "crafting-output")
            || has("furnace-slot")
            || has("furnace-result")
            || has("funnel-filter-slot")
            || has("reference-equipment");
        if grid.is_none() && !single {
            continue;
        }
        let mut next = node.clone();
        if let Some((columns, rows)) = grid {
            let px = |value| match value {
                Val::Px(value) => value,
                _ => 0.0,
            };
            let inset_x = px(node.padding.left)
                + px(node.padding.right)
                + px(node.border.left)
                + px(node.border.right);
            let inset_y = px(node.padding.top)
                + px(node.padding.bottom)
                + px(node.border.top)
                + px(node.border.bottom);
            next.column_gap = Val::Px(2.0);
            next.row_gap = Val::Px(2.0);
            next.grid_template_columns = vec![RepeatedGridTrack::px(columns as u16, cell)];
            next.grid_template_rows = vec![RepeatedGridTrack::px(rows as u16, cell)];
            next.grid_auto_rows = vec![GridTrack::px(cell)];
            next.width = Val::Px(columns as f32 * cell + (columns - 1) as f32 * 2.0 + inset_x);
            next.height = Val::Px(rows as f32 * cell + (rows - 1) as f32 * 2.0 + inset_y);
            next.flex_shrink = 0.0;
        } else {
            let side = if has("reference-equipment") {
                25.0
            } else if matches!(id, "inventory-craft-result" | "crafting-output")
                || has("furnace-result")
            {
                38.4
            } else {
                cell
            };
            let side = (side / inverse).round() * inverse;
            next.width = Val::Px(side);
            next.height = Val::Px(side);
            next.flex_shrink = 0.0;
        }
        if *node != next {
            *node = next;
        }
    }
}

fn catalog_slot_size(
    mut commands: Commands,
    mut nodes: Query<(
        Entity,
        &ElementId,
        &ComputedNode,
        &mut Node,
        Option<&CatalogGridCapacity>,
    )>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("ui.frames.catalog_slot_size");
    let Some(cell) = nodes.iter().find_map(|(_, id, computed, node, _)| {
        if id.0 != "inventory-grid" || computed.size().x < 90.0 {
            return None;
        }
        let inset = computed.content_inset();
        let gap = match node.column_gap {
            Val::Px(px) => px / computed.inverse_scale_factor(),
            _ => 0.0,
        };
        Some((computed.size().x - inset.min_inset.x - inset.max_inset.x - gap * 8.0) / 9.0)
    }) else {
        return;
    };
    for (entity, id, computed, mut node, capacity) in &mut nodes {
        let logical_cell = cell * computed.inverse_scale_factor();
        if id.0 == "inventory-crafting" {
            let gap = match node.column_gap {
                Val::Px(px) => px,
                _ => 3.0,
            };
            let side = Val::Px(logical_cell * 2.0 + gap);
            if node.width != side {
                node.width = side;
            }
            if node.height != side {
                node.height = side;
            }
            let tracks = vec![RepeatedGridTrack::px(2, logical_cell)];
            if node.grid_template_columns != tracks {
                node.grid_template_columns = tracks.clone();
            }
            if node.grid_template_rows != tracks {
                node.grid_template_rows = tracks;
            }
            continue;
        }
        if id.0 == "inventory-craft-result" {
            let inverse = computed.inverse_scale_factor().max(0.01);
            let side = Val::Px((38.4 / inverse).round() * inverse);
            if node.width != side {
                node.width = side;
            }
            if node.height != side {
                node.height = side;
            }
            continue;
        }
        if id.0 != "inventory-browser-grid" || computed.size().min_element() < cell {
            continue;
        }
        let inset = computed.content_inset();
        let space = computed.size() - inset.min_inset - inset.max_inset;
        let physical_gap = match node.column_gap {
            Val::Px(px) => px / computed.inverse_scale_factor(),
            _ => 0.0,
        };
        let tracks = ((space + Vec2::splat(physical_gap + 0.5)) / (cell + physical_gap))
            .floor()
            .max(Vec2::ONE);
        let columns = vec![RepeatedGridTrack::px(
            tracks.x as u16,
            cell * computed.inverse_scale_factor(),
        )];
        let rows = vec![RepeatedGridTrack::px(
            tracks.y as u16,
            cell * computed.inverse_scale_factor(),
        )];
        if node.grid_template_columns != columns {
            node.grid_template_columns = columns;
        }
        if node.grid_template_rows != rows {
            node.grid_template_rows = rows;
        }
        let next = CatalogGridCapacity((tracks.x * tracks.y) as usize);
        if capacity != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
}

fn nine_slice(mut images: Query<(&ElementClasses, &ComputedNode, &mut ImageNode)>) {
    let _scope = terra_core::profiling::ScopeTimer::new("ui.frames.nine_slice");
    for (classes, node, mut image) in &mut images {
        if classes
            .classes
            .iter()
            .any(|class| class == "painted-hotbar-frame")
        {
            // Clip the transparent padding in the source; retain the original PNG.
            let rect = Some(Rect::new(0.0, 195.0, 2172.0, 515.0));
            if image.rect != rect {
                image.rect = rect;
            }
            if image.image_mode != NodeImageMode::Stretch {
                image.image_mode = NodeImageMode::Stretch;
            }
            continue;
        }
        if classes
            .classes
            .iter()
            .any(|class| class == "painted-menu-button-frame")
        {
            let rect = Some(Rect::new(55.0, 140.0, 2117.0, 565.0));
            if image.rect != rect {
                image.rect = rect;
            }
            if image.image_mode != NodeImageMode::Stretch {
                image.image_mode = NodeImageMode::Stretch;
            }
            continue;
        }
        if classes
            .classes
            .iter()
            .any(|class| class == "inventory-reference-frame")
        {
            let scale = (node.size().x / 2600.0).clamp(0.20, 0.55);
            let mode = NodeImageMode::Sliced(TextureSlicer {
                border: BorderRect::all(128.0),
                max_corner_scale: scale,
                ..default()
            });
            if image.image_mode != mode {
                image.image_mode = mode;
            }
            continue;
        }
        if !classes
            .classes
            .iter()
            .any(|class| class == "ornate-frame-art")
        {
            continue;
        }
        // Compact browsers have only 6-10 px of inset. Keep the ornament
        // inside that space without changing inventory slot dimensions.
        let corner_scale = if node.size().x > 0.0 {
            (node.size().x / 4000.0).clamp(0.05, 0.10)
        } else {
            0.10
        };
        let mode = NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect::all(196.0),
            max_corner_scale: corner_scale,
            ..default()
        });
        if image.image_mode != mode {
            image.image_mode = mode;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_and_hotbar_grow_with_resolution_and_fit_linked_storage() {
        let mut previous = 0.0;
        for viewport in [
            Vec2::new(1024.0, 640.0),
            Vec2::new(1280.0, 720.0),
            Vec2::new(1920.0, 1080.0),
            Vec2::new(2560.0, 1440.0),
            Vec2::new(3840.0, 2160.0),
        ] {
            let zoom = inventory_zoom(viewport, 1.0, 1.0, false);
            assert!(28.0 * zoom > previous);
            previous = 28.0 * zoom;
            assert!(400.0 * zoom >= viewport.y * 0.73);
            assert!(400.0 * zoom <= viewport.y * 0.82);
            assert!(28.0 * hotbar_zoom(viewport, 1.0, 1.0) >= viewport.y * 0.034);
            for linked in [false, true] {
                for preference in [0.75, 1.0, 1.1, 1.5] {
                    let zoom = inventory_zoom(viewport, preference, 1.0, linked);
                    let width = if linked { 1044.0 } else { 682.0 };
                    assert!(width * zoom <= viewport.x * 0.96 + 0.01);
                    assert!(418.0 * zoom <= viewport.y * 0.82 + 0.01);
                }
            }
        }
    }

    #[test]
    fn responsive_inventory_size_is_independent_of_global_scale_and_dpi() {
        let physical = Vec2::new(2560.0, 1440.0);
        for linked in [false, true] {
            for preference in [0.75, 1.0, 1.1, 1.5] {
                let expected = inventory_zoom(physical, preference, 1.0, linked);
                let expected_hotbar = hotbar_zoom(physical, preference, 1.0);
                for dpi in [1.0, 1.25, 1.5, 2.0] {
                    for global in [0.75, 1.0, 1.5, 2.0] {
                        let actual = inventory_zoom(physical / dpi, preference, global, linked)
                            * global
                            * dpi;
                        assert!((actual - expected).abs() < 0.001);
                        let hotbar = hotbar_zoom(physical / dpi, preference, global) * global * dpi;
                        assert!((hotbar - expected_hotbar).abs() < 0.001);
                    }
                }
            }
        }
        assert!(
            inventory_zoom(physical, 0.75, 1.0, false) < inventory_zoom(physical, 1.0, 1.0, false)
        );
        assert!(
            inventory_zoom(physical, 1.0, 1.0, false) < inventory_zoom(physical, 1.5, 1.0, false)
        );
    }

    #[test]
    fn content_density_scales_input_as_a_unit_at_hd_through_4k() {
        for height in [720, 1080, 1440, 2160] {
            for dpi in [1.0, 2.0] {
                let mut app = App::new();
                app.add_systems(Update, content_density);
                let mut window = Window::default();
                window.resolution.set_scale_factor_override(Some(dpi));
                window
                    .resolution
                    .set_physical_resolution(height * 16 / 9, height);
                app.world_mut().spawn((window, bevy::window::PrimaryWindow));
                let root = app
                    .world_mut()
                    .spawn((Node::default(), ElementId("inventory-workspace".into())))
                    .id();
                let input = app
                    .world_mut()
                    .spawn((
                        Node::default(),
                        tilt_ui::TiltElement {
                            kind: tilt_ui::ElementKind::Input,
                        },
                        ChildOf(root),
                    ))
                    .id();
                let value = app
                    .world_mut()
                    .spawn((
                        Node::default(),
                        TextFont::default(),
                        tilt_ui::ControlPart {
                            owner: input,
                            kind: tilt_ui::ControlPartKind::Value,
                        },
                        ChildOf(input),
                    ))
                    .id();
                app.update();
                let expected = (1080.0 / height as f32).sqrt().min(1.0);
                assert!(
                    app.world()
                        .get::<UiTransform>(input)
                        .unwrap()
                        .scale
                        .abs_diff_eq(Vec2::splat(expected), 0.001)
                );
                assert_eq!(
                    app.world().get::<UiTransform>(value).unwrap().scale,
                    Vec2::ONE
                );
            }
        }
    }

    #[test]
    fn frame_styles_use_supported_tilt_ui_properties() {
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../../../src-ui/styles/interface-182.css"
        ))
        .unwrap();
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../../../src-ui/styles/interface-183.css"
        ))
        .unwrap();
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../../../src-ui/styles/interface-184.css"
        ))
        .unwrap();
        for stylesheet in [
            include_str!("../../../src-ui/styles/common-surfaces-185.css"),
            include_str!("../../../src-ui/styles/machines-185.css"),
            include_str!("../../../src-ui/styles/recipes-185.css"),
            include_str!("../../../src-ui/styles/debug-target-185.css"),
            include_str!("../../../src-ui/styles/polish-186.css"),
            include_str!("../../../src-ui/styles/surfaces-187.css"),
            include_str!("../../../src-ui/styles/inventory-uniform-187.css"),
            include_str!("../../../src-ui/styles/inventory-188.css"),
            include_str!("../../../src-ui/styles/settings-188.css"),
            include_str!("../../../src-ui/styles/hotbar-188.css"),
            include_str!("../../../src-ui/styles/inventory-type-190.css"),
            include_str!("../../../src-ui/styles/inventory-191.css"),
            include_str!("../../../src-ui/styles/create-world.css"),
        ] {
            tilt_ui::tilt_ui_css::parse_stylesheet(stylesheet).unwrap();
        }
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../../../src-ui/components/ornate-frame.component.css"
        ))
        .unwrap();
    }
}
