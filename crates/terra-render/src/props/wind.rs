use bevy::{
    camera::primitives::{Aabb, MeshAabb},
    math::Vec3A,
    mesh::{MeshVertexAttribute, VertexAttributeValues},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{
        render_resource::{AsBindGroup, VertexFormat},
        storage::ShaderBuffer,
    },
    shader::ShaderRef,
};
use terra_world::props::PropKind;

pub(super) use terra_world::props::{WIND_AMPLITUDE, WIND_FLUTTER};

/// Section-batched rooted vegetation, never a separate entity for each plant.
#[derive(Component)]
pub struct WindProp;

pub(crate) const ATTRIBUTE_GROWTH_ANCHOR: MeshVertexAttribute = MeshVertexAttribute::new(
    "Terra_GrowthAnchor",
    0x4752_4f57_414e_4348,
    VertexFormat::Float32x3,
);

#[derive(Resource, Default, Debug)]
pub struct PropGrowthStatus {
    pub transitions: u64,
    pub active_until: f32,
    pub shader_time: f32,
    pub(super) active: std::collections::HashMap<terra_world::props::PropId, super::growth::Pose>,
    pub(super) crop_cleanup: std::collections::HashMap<terra_voxel::SectionPos, f32>,
}

impl PropGrowthStatus {
    pub fn progress(&self, id: terra_world::props::PropId) -> Option<f32> {
        self.active
            .get(&id)
            .map(|pose| ((self.shader_time - pose.start) / super::growth::SECONDS).clamp(0.0, 1.0))
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(crate) struct PropExtension {
    #[uniform(100)]
    wind: Vec4,
    #[storage(101, read_only)]
    weather: Handle<ShaderBuffer>,
}

impl Default for PropExtension {
    fn default() -> Self {
        Self {
            wind: Vec4::new(WIND_AMPLITUDE, WIND_FLUTTER, 1.05, 0.0),
            weather: crate::weather::storage::WEATHER_BUFFER_HANDLE.clone(),
        }
    }
}

impl MaterialExtension for PropExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_prop_fragment.wesl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/terra_prop_fragment.wesl".into()
    }
    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/terra_prop_fragment.wesl".into()
    }
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_props.wesl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/terra_props.wesl".into()
    }
    fn deferred_vertex_shader() -> ShaderRef {
        "shaders/terra_props.wesl".into()
    }
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if layout.0.contains(Mesh::ATTRIBUTE_COLOR)
            && layout
                .0
                .get_layout(&[Mesh::ATTRIBUTE_COLOR.at_shader_location(5)])?
                .attributes[0]
                .format
                == VertexFormat::Unorm8x4
        {
            descriptor
                .vertex
                .shader_defs
                .push("PACKED_PROP_COLOR".into());
        }
        if layout.0.contains(ATTRIBUTE_GROWTH_ANCHOR) {
            let prepass = descriptor.vertex.shader_defs.iter().any(|definition| {
                matches!(definition, bevy::shader::ShaderDefVal::Bool(name, true) if name == "PREPASS_PIPELINE")
            });
            let attributes = if prepass {
                vec![
                    Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                    Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
                    Mesh::ATTRIBUTE_UV_1.at_shader_location(2),
                    Mesh::ATTRIBUTE_NORMAL.at_shader_location(3),
                    Mesh::ATTRIBUTE_COLOR.at_shader_location(7),
                    ATTRIBUTE_GROWTH_ANCHOR.at_shader_location(8),
                ]
            } else {
                vec![
                    Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                    Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
                    Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
                    Mesh::ATTRIBUTE_UV_1.at_shader_location(3),
                    Mesh::ATTRIBUTE_COLOR.at_shader_location(5),
                    ATTRIBUTE_GROWTH_ANCHOR.at_shader_location(8),
                ]
            };
            descriptor.vertex.buffers = vec![layout.0.get_layout(&attributes)?];
            descriptor.vertex.shader_defs.push("PROP_GROWTH".into());
        }
        Ok(())
    }
}

pub(crate) type PropMaterial = ExtendedMaterial<StandardMaterial, PropExtension>;

pub(super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<PropMaterial>::default())
        .init_resource::<PropGrowthStatus>()
        .add_systems(Update, update_growth_clock);
}

pub(super) fn update_growth_clock(time: Option<Res<Time>>, mut status: ResMut<PropGrowthStatus>) {
    if status.shader_time >= status.active_until {
        return;
    }
    let now = time
        .as_deref()
        .map_or(0.0, Time::elapsed_secs)
        .min(status.active_until);
    if now <= status.shader_time {
        return;
    }
    status.shader_time = now;
}

pub(super) fn parameters(kind: PropKind, variant: u8, height: f32, scale: f32) -> [f32; 2] {
    terra_world::props::wind_parameters(kind, variant, height, scale)
}
pub(super) fn padded_bounds(mesh: &Mesh) -> Option<Aabb> {
    let mut bounds = mesh.get_aabb()?;
    let weight = match mesh.attribute(Mesh::ATTRIBUTE_UV_0)? {
        VertexAttributeValues::Float32x2(values) => values
            .iter()
            .map(|[height, flexibility]| {
                (height * flexibility).abs()
                    * if *flexibility < 0.0 {
                        1.0
                    } else {
                        terra_world::props::MAX_WEATHER_WIND
                    }
            })
            .fold(0.0_f32, f32::max),
        _ => return None,
    };
    let maximum = weight * (WIND_AMPLITUDE + WIND_FLUTTER);
    bounds.half_extents += Vec3A::new(maximum, 0.0, maximum);
    Some(bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animated_weather_and_growth_keep_prop_and_fade_materials_unchanged() {
        use crate::weather::storage::{WEATHER_BUFFER_HANDLE, WeatherPayload};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<PropMaterial>()
            .init_resource::<PropGrowthStatus>()
            .init_resource::<WeatherPayload>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(100),
            ))
            .add_systems(Update, update_growth_clock)
            .add_systems(PostUpdate, crate::weather::storage::update);
        let material = PropMaterial {
            base: StandardMaterial::default(),
            extension: PropExtension::default(),
        };
        let handle = app
            .world_mut()
            .resource_mut::<Assets<PropMaterial>>()
            .add(material.clone());
        let mut fade = material.clone();
        fade.base.base_color.set_alpha(0.35);
        let fade = app
            .world_mut()
            .resource_mut::<Assets<PropMaterial>>()
            .add(fade);
        app.world_mut()
            .resource_mut::<PropGrowthStatus>()
            .active_until = 8.0;
        app.update();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<PropMaterial>>>()
            .clear();
        let mut previous_time = app.world().resource::<PropGrowthStatus>().shader_time;
        for step in 1..=32 {
            let visual = crate::weather::WeatherVisual {
                rain: step as f32 / 32.0,
                wind: Vec2::new(step as f32 * 0.25, 2.0),
                ..default()
            };
            let expected_wind = crate::weather::foliage_wind(Some(&visual));
            let expected_tint = Vec3::from_array(crate::appearance::visual_weather_tint(&visual));
            app.insert_resource(visual);
            app.update();
            assert_eq!(
                app.world_mut()
                    .resource_mut::<Messages<AssetEvent<PropMaterial>>>()
                    .drain()
                    .filter(|event| matches!(event, AssetEvent::Modified { .. }))
                    .count(),
                0
            );
            let growth = app.world().resource::<PropGrowthStatus>().shader_time;
            assert!(growth > previous_time);
            previous_time = growth;
            let payload = app.world().resource::<WeatherPayload>();
            assert_eq!(payload.0.wind, expected_wind);
            assert_eq!(payload.0.tint.truncate(), expected_tint);
            assert_eq!(payload.0.surface.w, growth);
            let materials = app.world().resource::<Assets<PropMaterial>>();
            for handle in [&handle, &fade] {
                let current = materials.get(handle).unwrap();
                assert_eq!(current.extension.weather, WEATHER_BUFFER_HANDLE);
                assert_eq!(current.extension.wind, material.extension.wind);
            }
            assert_eq!(
                materials.get(&handle).unwrap().base.base_color,
                Color::WHITE
            );
            assert_eq!(materials.get(&fade).unwrap().base.base_color.alpha(), 0.35);
        }
    }
}
