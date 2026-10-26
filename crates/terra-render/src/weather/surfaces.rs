//! Persistent surface-condition payloads share the existing terrain batches.
use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    render::storage::ShaderBuffer,
    shader::ShaderRef,
};
use std::collections::{HashMap, HashSet};

#[path = "exposure.rs"]
pub(crate) mod exposure;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(crate) struct WetExtension {
    #[storage(101, read_only)]
    pub weather: Handle<ShaderBuffer>,
    #[uniform(102)]
    pub detail: Vec4,
}

impl Default for WetExtension {
    fn default() -> Self {
        Self {
            weather: super::storage::WEATHER_BUFFER_HANDLE,
            detail: Vec4::ZERO,
        }
    }
}

impl MaterialExtension for WetExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_wet_surface.wgsl".into()
    }
    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/terra_wet_surface.wgsl".into()
    }
}
pub(crate) type WetMaterial = ExtendedMaterial<StandardMaterial, WetExtension>;

#[derive(Resource, Default)]
pub(crate) struct WetMaterials(pub HashMap<usize, Handle<WetMaterial>>);

pub(super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<WetMaterial>::default())
        .init_resource::<WetMaterials>()
        .add_systems(
            Update,
            prepare
                .after(crate::appearance::update_weather_materials)
                .before(crate::meshing::upload_sections),
        );
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    blocks: Res<crate::blocks::BlockAssets>,
    standard: Res<Assets<StandardMaterial>>,
    mut handles: ResMut<WetMaterials>,
    mut materials: ResMut<Assets<WetMaterial>>,
    wet: Query<(), With<MeshMaterial3d<WetMaterial>>>,
    mut status: ResMut<super::WeatherRenderStatus>,
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
) {
    status.wet_batches = wet.iter().count();
    let changed: HashSet<_> = events
        .read()
        .filter_map(|event| match event {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id } => Some(*id),
            _ => None,
        })
        .collect();
    // Only one shared wet handle per existing opaque material, not per block,
    // plant, section or particle. Dry batches continue using their old handle.
    for (slot, handle) in blocks.materials.iter().enumerate() {
        let Some(base) = standard.get(handle) else {
            continue;
        };
        let settings = blocks
            .templates
            .as_ref()
            .map_or_else(Default::default, |t| t.block_details);
        let family = blocks.templates.as_ref().map_or(0.0, |templates| {
            [(3, 1.0), (2, 2.0), (1, 3.0), (4, 4.0), (5, 5.0)]
                .into_iter()
                .find_map(|(id, family)| {
                    templates
                        .blocks
                        .get(&id)
                        .filter(|block| {
                            if id == 1 {
                                block.face_materials[3] == slot
                            } else {
                                block.face_materials.contains(&slot)
                            }
                        })
                        .map(|_| family)
                })
                .unwrap_or(0.0)
        });
        let detail = Vec4::new(
            if settings.material_detail {
                family
            } else {
                -family
            },
            f32::from(settings.normal_mapping),
            f32::from(settings.surface_variation),
            match settings.detail_quality {
                terra_core::BlockDetailQuality::Low => 0.4,
                terra_core::BlockDetailQuality::Medium => 0.7,
                terra_core::BlockDetailQuality::High => 1.0,
            },
        );
        if let Some(wet) = handles.0.get(&slot) {
            let needs_update = changed.contains(&handle.id());
            if needs_update && let Some(mut material) = materials.get_mut(wet) {
                material.base = base.clone();
            }
            if materials
                .get(wet)
                .is_some_and(|m| m.extension.detail != detail)
                && let Some(mut material) = materials.get_mut(wet)
            {
                material.extension.detail = detail;
            }
            continue;
        }
        if !matches!(base.alpha_mode, AlphaMode::Opaque) || base.unlit {
            continue;
        }
        handles.0.insert(
            slot,
            materials.add(WetMaterial {
                base: base.clone(),
                extension: WetExtension {
                    detail,
                    ..Default::default()
                },
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_shader_switch_reuses_materials_and_leaves_weather_binding_enabled() {
        use crate::blocks::{BlockTemplate, TemplateSet, definitions::BlockRenderKind};
        let mut app = App::new();
        app.init_resource::<crate::blocks::BlockAssets>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<WetMaterial>>()
            .init_resource::<WetMaterials>()
            .init_resource::<super::super::WeatherRenderStatus>()
            .add_message::<AssetEvent<StandardMaterial>>()
            .add_systems(Update, prepare);
        let handle = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            1,
            BlockTemplate {
                surface: None,
                model: Vec::new(),
                render_kind: BlockRenderKind::Cube,
                face_materials: [0; 6],
                side_cap: None,
                bevel_width: 0.0,
                faces: Default::default(),
                details: Vec::new(),
            },
        );
        {
            let mut blocks = app.world_mut().resource_mut::<crate::blocks::BlockAssets>();
            blocks.materials.push(handle);
            blocks.templates = Some(std::sync::Arc::new(templates));
        }
        app.update();
        let wet = app.world().resource::<WetMaterials>().0[&0].clone();
        for enabled in [false, true, false, true] {
            std::sync::Arc::make_mut(
                app.world_mut()
                    .resource_mut::<crate::blocks::BlockAssets>()
                    .templates
                    .as_mut()
                    .unwrap(),
            )
            .block_details
            .material_detail = enabled;
            app.update();
            let assets = app.world().resource::<Assets<WetMaterial>>();
            let material = assets.get(&wet).unwrap();
            assert_eq!(
                material.extension.detail.x,
                if enabled { 3.0 } else { -3.0 }
            );
            assert_eq!(
                material.extension.weather,
                super::super::storage::WEATHER_BUFFER_HANDLE
            );
            assert_eq!(assets.len(), 1);
            assert_eq!(app.world().resource::<WetMaterials>().0[&0], wet);
        }
    }

    #[test]
    fn changing_rain_does_not_invalidate_shared_surface_materials() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_resource::<crate::blocks::BlockAssets>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WetMaterial>()
            .init_resource::<WetMaterials>()
            .init_resource::<super::super::WeatherRenderStatus>()
            .init_resource::<super::super::WeatherVisual>()
            .add_systems(Update, prepare);
        let base = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        app.world_mut()
            .resource_mut::<crate::blocks::BlockAssets>()
            .materials
            .push(base);
        for _ in 0..3 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<WetMaterial>>>()
            .clear();
        let wet = app.world().resource::<WetMaterials>().0[&0].clone();
        for step in 0..16 {
            {
                let mut visual = app
                    .world_mut()
                    .resource_mut::<super::super::WeatherVisual>();
                visual.rain = step as f32 / 16.0;
                visual.wetness = visual.rain;
            }
            app.update();
            assert!(
                app.world_mut()
                    .resource_mut::<Messages<AssetEvent<WetMaterial>>>()
                    .drain()
                    .next()
                    .is_none()
            );
            assert_eq!(app.world().resource::<WetMaterials>().0[&0], wet);
        }
        let materials = app.world().resource::<Assets<WetMaterial>>();
        assert_eq!(materials.len(), 1);
        assert_eq!(
            materials.get(&wet).unwrap().extension.weather,
            super::super::storage::WEATHER_BUFFER_HANDLE
        );
    }

    #[test]
    fn shared_wet_handles_follow_modified_base_materials_without_recreation() {
        let mut app = App::new();
        app.init_resource::<crate::blocks::BlockAssets>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<WetMaterial>>()
            .init_resource::<WetMaterials>()
            .init_resource::<super::super::WeatherRenderStatus>()
            .add_message::<AssetEvent<StandardMaterial>>()
            .add_systems(Update, prepare);
        let base = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        app.world_mut()
            .resource_mut::<crate::blocks::BlockAssets>()
            .materials
            .push(base.clone());
        app.update();
        let wet = app.world().resource::<WetMaterials>().0[&0].clone();
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&base)
            .unwrap()
            .perceptual_roughness = 0.73;
        app.world_mut()
            .write_message(AssetEvent::<StandardMaterial>::Modified { id: base.id() });
        app.update();
        assert_eq!(app.world().resource::<WetMaterials>().0[&0], wet);
        assert_eq!(app.world().resource::<Assets<WetMaterial>>().len(), 1);
        assert_eq!(
            app.world()
                .resource::<Assets<WetMaterial>>()
                .get(&wet)
                .unwrap()
                .base
                .perceptual_roughness,
            0.73
        );
    }
}
