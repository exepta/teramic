//! Reproject cached cartography in the existing TiltUI image nodes.
use super::{Atlas, raster::Projection};
use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, PartialEq)]
pub(crate) struct MapMaterial {
    #[uniform(0)]
    row_x: Vec4,
    #[uniform(1)]
    row_y: Vec4,
    #[uniform(2)]
    clip: Vec4,
    #[uniform(7)]
    overlay_x: Vec4,
    #[uniform(8)]
    overlay_y: Vec4,
    #[uniform(9)]
    player: Vec4,
    #[uniform(10)]
    grid_x: Vec4,
    #[uniform(11)]
    grid_y: Vec4,
    #[texture(3)]
    #[sampler(4)]
    terrain: Handle<Image>,
    #[texture(5)]
    #[sampler(6)]
    overlay: Handle<Image>,
}
impl UiMaterial for MapMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_map_ui.wesl".into()
    }
}
pub(crate) fn install(app: &mut App) {
    app.add_plugins(MapRenderPlugin);
}

struct MapRenderPlugin;

impl Plugin for MapRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, bind.before(bevy::ui::UiSystems::Prepare));
    }

    fn finish(&self, app: &mut App) {
        // TiltUI registers asset sources before AssetPlugin. Its materials must
        // wait until the default asset and render plugins have been built.
        app.add_plugins(UiMaterialPlugin::<MapMaterial>::default());
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn material(
    materials: &mut Assets<MapMaterial>,
    handle: &mut Handle<MapMaterial>,
    terrain: &Handle<Image>,
    overlay: &Handle<Image>,
    current: &Projection,
    reference: &Projection,
    overlay_reference: &Projection,
    level: i64,
    reference_level: i64,
    overlay_reference_level: i64,
    player: [f32; 3],
    yaw: f32,
    grid: bool,
) {
    let rows = current.uv_transform_from(reference, level, reference_level);
    let overlay_rows = current.uv_transform_from(overlay_reference, level, overlay_reference_level);
    let origin = current.world_at([0.0, 0.0]);
    let x = current.world_at([current.size() as f32, 0.0]);
    let y = current.world_at([0.0, current.size() as f32]);
    let value = MapMaterial {
        row_x: rows[0],
        row_y: rows[1],
        overlay_x: overlay_rows[0],
        overlay_y: overlay_rows[1],
        player: current.player_transform(player, level, yaw),
        grid_x: Vec4::new(
            (x[0] - origin[0]) as f32 / 16.0,
            (y[0] - origin[0]) as f32 / 16.0,
            (origin[0] / 16.0).rem_euclid(1.0) as f32,
            if grid { 1.0 } else { 0.0 },
        ),
        grid_y: Vec4::new(
            (x[1] - origin[1]) as f32 / 16.0,
            (y[1] - origin[1]) as f32 / 16.0,
            (origin[1] / 16.0).rem_euclid(1.0) as f32,
            0.0,
        ),
        clip: Vec4::new(
            if current.is_circular() { 0.338 } else { 0.0 },
            0.0,
            current.size() as f32,
            current.aspect(),
        ),
        terrain: terrain.clone(),
        overlay: overlay.clone(),
    };
    if *handle != Handle::default()
        && let Some(mut material) = materials.get_mut(&*handle)
    {
        if *material != value {
            *material = value;
        }
    } else {
        *handle = materials.add(value);
    }
}
pub(super) fn hide(materials: &mut Assets<MapMaterial>, handle: &Handle<MapMaterial>) {
    if *handle != Handle::default()
        && let Some(mut material) = materials.get_mut(handle)
        && material.clip.y != 1.0
    {
        material.clip.y = 1.0;
    }
}
fn bind(
    mut commands: Commands,
    atlas: Res<Atlas>,
    mut nodes: Query<(
        Entity,
        &tilt_ui::ElementId,
        &mut ImageNode,
        Option<&MaterialNode<MapMaterial>>,
    )>,
) {
    for (entity, id, mut image, material) in &mut nodes {
        let canvas = match id.0.as_str() {
            "map-surface" => &atlas.mini,
            "full-map-surface" => &atlas.full,
            _ => continue,
        };
        if image.color != Color::NONE {
            image.color = Color::NONE;
        }
        if canvas.material == Handle::default() {
            if material.is_some() {
                commands
                    .entity(entity)
                    .remove::<MaterialNode<MapMaterial>>();
            }
            continue;
        }
        if image.image != canvas.terrain {
            image.image = canvas.terrain.clone();
        }
        if material.is_none_or(|m| m.0 != canvas.material) {
            commands
                .entity(entity)
                .insert(MaterialNode(canvas.material.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_swap_and_live_motion_refresh_both_transforms_between_marker_ticks() {
        use crate::minimap::{Canvas, PaintKey, PaintResult, RenderAssets, present};
        use bevy::{
            ecs::system::SystemState,
            tasks::{AsyncComputeTaskPool, TaskPool},
        };
        use std::collections::HashMap;
        use terra_core::MapPalette;
        use terra_world::cartography::MapLayer;

        let old = Projection::at([0.0; 2], 192, 0.0, false, true, 320, true);
        let refreshed = Projection::at([12.0, -3.0], 192, 0.0, false, true, 320, true).cached();
        let palette = MapPalette::default();
        let key = PaintKey {
            center: [12, 80, -3],
            layer: MapLayer::Surface,
            span: 192,
            relief: true,
            three_dimensional: false,
            aspect: 1.0f32.to_bits(),
            revision: 1,
            palette: palette.fingerprint(),
        };
        let result = PaintResult {
            pixels: vec![120; 320 * 320 * 4],
            projection: refreshed.clone(),
            level: 80,
            key,
        };
        let task = AsyncComputeTaskPool::get_or_init(TaskPool::new).spawn(async move { result });
        let start = std::time::Instant::now();
        while !task.is_finished() {
            assert!(start.elapsed() < std::time::Duration::from_secs(5));
            std::thread::yield_now();
        }
        let mut canvas = Canvas {
            reference: Some(old.clone()),
            overlay_reference: Some(old.clone()),
            reference_level: 80,
            overlay_reference_level: 80,
            next_frame: 10.0,
            next_terrain: 10.0,
            job: Some(task),
            ..default()
        };
        let mut world = World::new();
        world.insert_resource(Assets::<Image>::default());
        world.insert_resource(Assets::<MapMaterial>::default());
        let mut state = SystemState::<RenderAssets>::new(&mut world);
        let mut assets = state.get_mut(&mut world).unwrap();
        let overlay_handle = canvas.overlay.clone();
        for (tick, center) in [[13.0, -4.0], [15.0, -6.0], [18.0, -9.0]]
            .into_iter()
            .enumerate()
        {
            let current = Projection::at(center, 192, tick as f32 * 0.1, true, true, 320, true);
            let level = [80, 104, -40][tick];
            let player = [center[0] as f32, level as f32, center[1] as f32];
            present(
                &mut canvas,
                &HashMap::new(),
                MapLayer::Surface,
                &palette,
                &current,
                level,
                192,
                1,
                1.0 + tick as f64 * 0.01,
                24,
                0.0,
                &[],
                1.0,
                player,
                &mut assets,
                None,
            );
            assert_eq!(
                canvas.overlay, overlay_handle,
                "marker upload must retain its FPS cap"
            );
            let material = assets.materials.get(&canvas.material).unwrap();
            let terrain = current.uv_transform_from(&refreshed, level, 80);
            let overlay = current.uv_transform_from(&old, level, 80);
            assert_eq!(material.row_x, terrain[0]);
            assert_eq!(material.row_y, terrain[1]);
            assert_eq!(material.overlay_x, overlay[0]);
            assert_eq!(material.overlay_y, overlay[1]);
            assert_eq!(material.clip.y, 0.0);
            assert_eq!(material.clip.z, 320.0);
            assert_eq!(material.player.truncate().truncate(), Vec2::splat(160.0));
            let center_uv = Vec3::new(0.5, 0.5, 1.0);
            let sampled = Vec2::new(
                material.row_x.truncate().dot(center_uv),
                material.row_y.truncate().dot(center_uv),
            );
            let expected = Vec2::from_array(refreshed.project(player, 80)) / 320.0;
            assert!(
                sampled.distance(expected) < 0.00001,
                "the centered arrow must sample the same terrain point after a ground-height change"
            );
        }
    }

    #[test]
    fn map_material_registration_waits_for_the_late_asset_plugin() {
        let mut app = App::new();
        install(&mut app);
        assert!(!app.world().contains_resource::<Assets<MapMaterial>>());
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<bevy::shader::Shader>()
            .init_asset_loader::<bevy::shader::ShaderLoader>();
        app.finish();
        assert!(app.world().contains_resource::<Assets<MapMaterial>>());
        assert!(app.is_plugin_added::<UiMaterialPlugin<MapMaterial>>());
    }
}
