//! Persistent surface-condition payloads share the existing terrain batches.
use bevy::{
    ecs::system::SystemParamItem,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::storage::ShaderBuffer,
    render::{
        RenderApp, RenderStartup,
        render_resource::{
            AsBindGroup, AsBindGroupError, BindGroupBuilder, BindGroupLayout, BindGroupLayoutEntry,
            BindlessDescriptor, BindlessSlabResourceLimit,
        },
        renderer::RenderDevice,
        settings::WgpuFeatures,
    },
    shader::ShaderRef,
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

#[path = "exposure.rs"]
pub(crate) mod exposure;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bindless(index_table(range(101..103), binding(100)))]
#[data(102, WetParameters, binding_array(102))]
pub(crate) struct WetExtension {
    #[storage(101, read_only, binding_array(101))]
    pub weather: Handle<ShaderBuffer>,
    pub detail: Vec4,
}

#[derive(Clone, bevy::render::render_resource::ShaderType)]
pub(crate) struct WetParameters {
    detail: Vec4,
}

impl From<&WetExtension> for WetParameters {
    fn from(bindings: &WetExtension) -> Self {
        Self {
            detail: bindings.detail,
        }
    }
}

fn legacy_wet_bindings(smoke: bool, setting: Option<&str>) -> bool {
    smoke && setting == Some("1")
}

fn wet_binding_slots(legacy: bool) -> Option<BindlessSlabResourceLimit> {
    if legacy {
        None
    } else {
        ExtendedWetMaterial::bindless_slot_count()
    }
}

fn legacy_bindings_enabled() -> bool {
    static LEGACY: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        legacy_wet_bindings(
            std::env::args().any(|arg| arg.starts_with("--smoke-test")),
            std::env::var("TERRA_LEGACY_WET_BINDINGS").ok().as_deref(),
        )
    });
    *LEGACY
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
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_liquid_vertex.wesl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/terra_liquid_vertex.wesl".into()
    }
    fn deferred_vertex_shader() -> ShaderRef {
        "shaders/terra_liquid_vertex.wesl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/terra_imprint_prepass.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_wet_surface.wesl".into()
    }
    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/terra_wet_surface.wesl".into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        terrain_depth_culling(descriptor, key.mesh_key);
        if layout.0.contains(crate::liquid_motion::FROM) {
            let mut attributes = vec![Mesh::ATTRIBUTE_POSITION.at_shader_location(0)];
            let prepass = descriptor.vertex.shader_defs.iter().any(|definition| {
                matches!(definition, bevy::shader::ShaderDefVal::Bool(name, true) if name == "PREPASS_PIPELINE")
            });
            let locations = if prepass {
                [3, 1, 2, 4, 7]
            } else {
                [1, 2, 3, 4, 5]
            };
            for (attribute, location) in [
                Mesh::ATTRIBUTE_NORMAL,
                Mesh::ATTRIBUTE_UV_0,
                Mesh::ATTRIBUTE_UV_1,
                Mesh::ATTRIBUTE_TANGENT,
                Mesh::ATTRIBUTE_COLOR,
            ]
            .into_iter()
            .zip(locations)
            {
                if layout.0.contains(attribute) {
                    attributes.push(attribute.at_shader_location(location));
                }
            }
            attributes.push(crate::liquid_motion::FROM.at_shader_location(11));
            attributes.push(crate::liquid_motion::DATA.at_shader_location(12));
            descriptor.vertex.buffers = vec![layout.0.get_layout(&attributes)?];
            descriptor.vertex.shader_defs.push("LIQUID_MOTION".into());
        }
        Ok(())
    }
}

#[derive(Asset, Reflect, Debug, Clone, Default)]
pub(crate) struct TerrainDepthExtension;

// No bindings or index tables are added to StandardMaterial. Deriving an empty
// bindless group would add a second (empty) index table on its binding 0.
impl AsBindGroup for TerrainDepthExtension {
    type Data = ();
    type Param = ();

    fn label() -> &'static str {
        "TerrainDepthExtension"
    }

    fn bindless_slot_count() -> Option<BindlessSlabResourceLimit> {
        Some(BindlessSlabResourceLimit::Auto)
    }

    fn bind_group_data(&self) -> Self::Data {}

    fn build_bind_group(
        &self,
        _layout: &BindGroupLayout,
        _render_device: &RenderDevice,
        _param: &mut SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
        _output: &mut BindGroupBuilder,
    ) -> Result<(), AsBindGroupError> {
        Ok(())
    }

    fn bind_group_layout_entries(
        _render_device: &RenderDevice,
        _force_no_bindless: bool,
    ) -> Vec<BindGroupLayoutEntry> {
        Vec::new()
    }

    fn bindless_descriptor() -> Option<BindlessDescriptor> {
        Some(BindlessDescriptor {
            resources: Cow::Borrowed(&[]),
            buffers: Cow::Borrowed(&[]),
            index_tables: Cow::Borrowed(&[]),
        })
    }
}

impl MaterialExtension for TerrainDepthExtension {
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        terrain_depth_culling(descriptor, key.mesh_key);
        Ok(())
    }
}

fn terrain_depth_culling(
    descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
    key: bevy::pbr::MeshPipelineKey,
) {
    // DEPTH_PREPASS also marks forward views that can read the depth texture.
    // Only real depth-only pipelines need two-sided roofs for shadow coverage.
    let prepass = descriptor.vertex.shader_defs.iter().any(|definition| {
        matches!(definition, bevy::shader::ShaderDefVal::Bool(name, true) if name == "PREPASS_PIPELINE")
    });
    let writes_color = descriptor
        .fragment
        .as_ref()
        .is_some_and(|fragment| fragment.targets.iter().any(Option::is_some));
    if prepass && !writes_color && key.contains(bevy::pbr::MeshPipelineKey::DEPTH_PREPASS) {
        descriptor.primitive.cull_mode = None;
    }
}

pub(crate) fn dry_terrain_uses_shared_surface_material() -> bool {
    static REFERENCE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::args().any(|arg| arg == "--smoke-test-caves")
            && std::env::var("TERRA_CAVE_LOD_SHADOW_REFERENCE").is_ok_and(|value| value == "1")
    });
    !*REFERENCE
}
type ExtendedWetMaterial = ExtendedMaterial<StandardMaterial, WetExtension>;

#[derive(Asset, Reflect, Debug, Clone, Default)]
pub(crate) struct WetMaterial {
    pub base: StandardMaterial,
    pub extension: WetExtension,
}

fn force_wet_fallback(force_no_bindless: bool, legacy: bool, supported: bool) -> bool {
    force_no_bindless || legacy || !supported
}

fn wet_features_supported(features: WgpuFeatures) -> bool {
    features.contains(
        WgpuFeatures::TEXTURE_BINDING_ARRAY
            | WgpuFeatures::BUFFER_BINDING_ARRAY
            | WgpuFeatures::STORAGE_RESOURCE_BINDING_ARRAY
            | WgpuFeatures::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
    )
}

impl AsBindGroup for WetMaterial {
    type Data = <ExtendedWetMaterial as AsBindGroup>::Data;
    type Param = <ExtendedWetMaterial as AsBindGroup>::Param;

    fn label() -> &'static str {
        "WetMaterial"
    }

    fn bind_group_data(&self) -> Self::Data {
        bevy::render::combined_bind_group::bind_group_data(&self.base, &self.extension)
    }

    fn bindless_slot_count() -> Option<BindlessSlabResourceLimit> {
        wet_binding_slots(legacy_bindings_enabled())
    }

    fn bindless_supported(render_device: &RenderDevice) -> bool {
        !legacy_bindings_enabled()
            && wet_features_supported(render_device.features())
            && ExtendedWetMaterial::bindless_supported(render_device)
    }

    fn build_bind_group(
        &self,
        layout: &BindGroupLayout,
        render_device: &RenderDevice,
        param: &mut SystemParamItem<'_, '_, Self::Param>,
        force_no_bindless: bool,
        output: &mut BindGroupBuilder,
    ) -> Result<(), AsBindGroupError> {
        let fallback = force_wet_fallback(
            force_no_bindless,
            legacy_bindings_enabled(),
            Self::bindless_supported(render_device),
        );
        bevy::render::combined_bind_group::build_bind_group(
            &self.base,
            &self.extension,
            layout,
            render_device,
            param,
            fallback,
            output,
        )
    }

    fn bind_group_layout_entries(
        render_device: &RenderDevice,
        force_no_bindless: bool,
    ) -> Vec<BindGroupLayoutEntry> {
        // Both halves must fall back together when buffer arrays are unavailable.
        ExtendedWetMaterial::bind_group_layout_entries(
            render_device,
            force_wet_fallback(
                force_no_bindless,
                legacy_bindings_enabled(),
                Self::bindless_supported(render_device),
            ),
        )
    }

    fn bindless_descriptor() -> Option<BindlessDescriptor> {
        if legacy_bindings_enabled() {
            None
        } else {
            let mut descriptor = ExtendedWetMaterial::bindless_descriptor()?;
            // Unused binding-array slots must fit the fixed weather structure,
            // not the engine's generic 16-byte storage-buffer fallback.
            use bevy::render::render_resource::ShaderType;
            for buffer in descriptor.buffers.to_mut() {
                if buffer.bindless_index.0 == 101 {
                    buffer.size = Some(super::storage::WeatherGpuData::min_size().get() as usize);
                }
            }
            Some(descriptor)
        }
    }
}

impl Material for WetMaterial {
    fn vertex_shader() -> ShaderRef {
        ExtendedWetMaterial::vertex_shader()
    }
    fn fragment_shader() -> ShaderRef {
        ExtendedWetMaterial::fragment_shader()
    }
    fn alpha_mode(&self) -> AlphaMode {
        // Opaque prepasses omit their fragment stage in Bevy. Soft ground needs
        // that stage for real footprint cutouts, without blending its coverage.
        if self.base.alpha_mode == AlphaMode::Opaque
            && matches!(self.extension.detail.x.abs(), 4.0 | 5.0 | 7.0)
        {
            AlphaMode::Mask(0.0)
        } else {
            self.base.alpha_mode()
        }
    }
    fn opaque_render_method(&self) -> bevy::material::OpaqueRendererMethod {
        self.base.opaque_render_method()
    }
    fn depth_bias(&self) -> f32 {
        self.base.depth_bias()
    }
    fn reads_view_transmission_texture(&self) -> bool {
        self.base.reads_view_transmission_texture()
    }
    fn enable_prepass() -> bool {
        ExtendedWetMaterial::enable_prepass()
    }
    fn enable_shadows() -> bool {
        ExtendedWetMaterial::enable_shadows()
    }
    fn enable_oit() -> bool {
        ExtendedWetMaterial::enable_oit()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        ExtendedWetMaterial::prepass_vertex_shader()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        ExtendedWetMaterial::prepass_fragment_shader()
    }
    fn deferred_vertex_shader() -> ShaderRef {
        ExtendedWetMaterial::deferred_vertex_shader()
    }
    fn deferred_fragment_shader() -> ShaderRef {
        ExtendedWetMaterial::deferred_fragment_shader()
    }
    fn specialize(
        pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        ExtendedWetMaterial::specialize(
            pipeline,
            descriptor,
            layout,
            bevy::pbr::MaterialPipelineKey {
                mesh_key: key.mesh_key,
                bind_group_data: key.bind_group_data,
            },
        )
    }
}

pub(crate) type TerrainDepthMaterial = ExtendedMaterial<StandardMaterial, TerrainDepthExtension>;

#[derive(Resource, Default)]
pub(crate) struct WetMaterials(pub HashMap<usize, Handle<WetMaterial>>);

#[derive(Resource, Default)]
pub(crate) struct TerrainDepthMaterials(pub HashMap<usize, Handle<TerrainDepthMaterial>>);

pub(super) fn install(app: &mut App) {
    app.add_plugins((
        MaterialPlugin::<WetMaterial>::default(),
        MaterialPlugin::<TerrainDepthMaterial>::default(),
    ))
    .init_resource::<WetMaterials>()
    .init_resource::<TerrainDepthMaterials>()
    .add_systems(
        Update,
        prepare
            .after(crate::appearance::update_weather_materials)
            .before(crate::meshing::upload_sections),
    );
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.add_systems(RenderStartup, report_wet_bindings);
    }
}

fn report_wet_bindings(device: Res<RenderDevice>) {
    tracing::info!(
        legacy_smoke_control = legacy_bindings_enabled(),
        bindless = WetMaterial::bindless_supported(&device)
            && WetMaterial::bindless_slot_count().is_some(),
        "Terrain wet material GPU binding mode"
    );
}

/// Use block surface metadata rather than a short list of terrain IDs, so
/// construction cubes share the same subtle, world-continuous close detail.
fn detail_family(templates: &crate::blocks::TemplateSet, slot: usize) -> f32 {
    use crate::blocks::definitions::{BlockRenderKind, CubeSurface};
    if templates
        .blocks
        .get(&terra_core::bile::FLUID)
        .is_some_and(|b| b.face_materials.contains(&slot))
    {
        return 14.0;
    }
    if templates
        .blocks
        .get(&terra_core::roads::WET_TAR)
        .is_some_and(|b| b.face_materials[3] == slot)
    {
        return 7.0;
    }
    for (id, family) in [(306, 8.0), (400, 9.0)] {
        if templates
            .blocks
            .get(&id)
            .is_some_and(|b| b.face_materials.contains(&slot))
        {
            return family;
        }
    }
    if templates.authored_terrain_material(slot) {
        return 6.0;
    }
    for (id, family) in [(3, 1.0), (2, 2.0), (1, 3.0), (4, 4.0), (5, 5.0)] {
        if templates.blocks.get(&id).is_some_and(|b| {
            if id == 1 {
                b.face_materials[3] == slot
            } else {
                b.face_materials.contains(&slot)
            }
        }) {
            return family;
        }
    }
    templates
        .blocks
        .values()
        .filter(|b| b.render_kind == BlockRenderKind::Cube && b.face_materials.contains(&slot))
        .map(|b| match b.surface.as_ref().map(|s| s.style()) {
            Some(CubeSurface::Wood) => 10.0,
            Some(CubeSurface::Rock) => 11.0,
            Some(CubeSurface::Wool) => 12.0,
            Some(CubeSurface::Earth | CubeSurface::Farmland) => 2.0,
            Some(CubeSurface::Turf) => 3.0,
            Some(CubeSurface::Sand) => 4.0,
            Some(CubeSurface::Snow) => 5.0,
            _ => 13.0,
        })
        .min_by(f32::total_cmp)
        .unwrap_or(0.0)
}

#[derive(Default)]
struct DetailFamilies {
    source: Option<std::sync::Arc<crate::blocks::TemplateSet>>,
    values: HashMap<usize, f32>,
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    blocks: Res<crate::blocks::BlockAssets>,
    standard: Res<Assets<StandardMaterial>>,
    mut handles: ResMut<WetMaterials>,
    mut materials: ResMut<Assets<WetMaterial>>,
    mut depth_handles: ResMut<TerrainDepthMaterials>,
    mut depth_materials: ResMut<Assets<TerrainDepthMaterial>>,
    wet: Query<(), With<MeshMaterial3d<WetMaterial>>>,
    mut status: ResMut<super::WeatherRenderStatus>,
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
    mut families: Local<DetailFamilies>,
) {
    if !families
        .source
        .as_ref()
        .zip(blocks.templates.as_ref())
        .is_some_and(|(old, current)| std::sync::Arc::ptr_eq(old, current))
    {
        families.source = blocks.templates.clone();
        families.values.clear();
    }
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
    // One shared wet handle per opaque surface or translucent Galle material,
    // never per block or liquid level. Dry terrain uses stock PBR with a binding-free
    // depth extension, retaining StandardMaterial's bindless resource layout.
    for (slot, handle) in blocks.materials.iter().enumerate() {
        let Some(base) = standard.get(handle) else {
            continue;
        };
        if let Some(depth) = depth_handles.0.get(&slot) {
            if changed.contains(&handle.id())
                && let Some(mut material) = depth_materials.get_mut(depth)
            {
                material.base = base.clone();
            }
        } else if matches!(base.alpha_mode, AlphaMode::Opaque) && !base.unlit {
            depth_handles.0.insert(
                slot,
                depth_materials.add(TerrainDepthMaterial {
                    base: base.clone(),
                    extension: TerrainDepthExtension,
                }),
            );
        }
        let settings = blocks
            .templates
            .as_ref()
            .map_or_else(Default::default, |t| t.block_details);
        let family = if let Some(templates) = blocks.templates.as_ref() {
            *families
                .values
                .entry(slot)
                .or_insert_with(|| detail_family(templates, slot))
        } else {
            0.0
        };
        let family = if family == 13.0 && base.alpha_mode != AlphaMode::Opaque {
            0.0
        } else {
            family
        };
        let detail = Vec4::new(
            if (6.0..=9.0).contains(&family) || family == 14.0 {
                -family
            } else if settings.material_detail {
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
        // Galle deliberately retains its translucent material. Its paid mesh
        // still needs the liquid vertex/fragment path; falling back to stock
        // StandardMaterial would ignore every interpolation attribute.
        if (!matches!(base.alpha_mode, AlphaMode::Opaque) && family != 14.0) || base.unlit {
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
    fn soft_ground_keeps_its_cutout_fragment_in_depth_only_prepasses_without_blending() {
        for family in [-5.0, -4.0, 0.0, 1.0, 4.0, 5.0] {
            let material = WetMaterial {
                extension: WetExtension {
                    detail: Vec4::new(family, 0.0, 0.0, 0.0),
                    ..Default::default()
                },
                ..Default::default()
            };
            assert_eq!(
                material.alpha_mode(),
                if family.abs() == 4.0 || family.abs() == 5.0 {
                    AlphaMode::Mask(0.0)
                } else {
                    AlphaMode::Opaque
                }
            );
            let mut blended = material.clone();
            blended.base.alpha_mode = AlphaMode::Blend;
            assert_eq!(blended.alpha_mode(), AlphaMode::Blend);
        }
    }

    #[test]
    fn legacy_wet_binding_control_is_smoke_only_and_fallback_is_atomic() {
        for setting in [None, Some("0"), Some("1"), Some("true")] {
            assert!(!legacy_wet_bindings(false, setting));
            assert_eq!(legacy_wet_bindings(true, setting), setting == Some("1"));
        }
        assert_eq!(wet_binding_slots(true), None);
        assert_eq!(
            wet_binding_slots(false),
            StandardMaterial::bindless_slot_count()
        );
        for forced in [false, true] {
            for legacy in [false, true] {
                for supported in [false, true] {
                    assert_eq!(
                        force_wet_fallback(forced, legacy, supported),
                        forced || legacy || !supported
                    );
                }
            }
        }
        let required = WgpuFeatures::TEXTURE_BINDING_ARRAY
            | WgpuFeatures::BUFFER_BINDING_ARRAY
            | WgpuFeatures::STORAGE_RESOURCE_BINDING_ARRAY
            | WgpuFeatures::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING;
        assert!(wet_features_supported(required));
        for feature in required.iter() {
            assert!(!wet_features_supported(required & !feature));
        }
    }

    #[test]
    fn wet_bindless_descriptor_preserves_standard_indices_and_sized_weather_fallback() {
        use bevy::render::render_resource::{BindlessResourceType, ShaderType};
        let standard = StandardMaterial::bindless_descriptor().unwrap();
        let wet = WetMaterial::bindless_descriptor().unwrap();
        assert_eq!(
            &wet.resources[..standard.resources.len()],
            &*standard.resources
        );
        assert_eq!(wet.resources[101], BindlessResourceType::ShaderBuffer);
        assert_eq!(wet.resources[102], BindlessResourceType::DataBuffer);
        assert_eq!(wet.index_tables.len(), standard.index_tables.len() + 1);
        for (actual, expected) in wet.index_tables.iter().zip(standard.index_tables.iter()) {
            assert_eq!(actual.indices, expected.indices);
            assert_eq!(actual.binding_number, expected.binding_number);
        }
        let table = wet.index_tables.last().unwrap();
        assert_eq!(table.indices.start.0, 101);
        assert_eq!(table.indices.end.0, 103);
        assert_eq!(table.binding_number.0, 100);
        for expected in standard.buffers.iter() {
            let actual = wet
                .buffers
                .iter()
                .find(|b| b.bindless_index == expected.bindless_index)
                .unwrap();
            assert_eq!(actual.binding_number, expected.binding_number);
            assert_eq!(actual.size, expected.size);
        }
        let weather = wet
            .buffers
            .iter()
            .find(|b| b.bindless_index.0 == 101)
            .unwrap();
        assert_eq!(weather.binding_number.0, 101);
        assert_eq!(
            weather.size,
            Some(super::super::storage::WeatherGpuData::min_size().get() as usize)
        );
        assert_eq!(weather.size, Some(16 * (6 + 64 + 1024 + 2)));
        let parameters = wet
            .buffers
            .iter()
            .find(|b| b.bindless_index.0 == 102)
            .unwrap();
        assert_eq!(parameters.binding_number.0, 102);
        assert_eq!(parameters.size, Some(16));
    }

    #[test]
    fn wet_parameters_keep_the_existing_vec4_bytes_and_shared_weather_handle() {
        use bevy::render::render_resource::{ShaderType, encase};
        assert_eq!(WetParameters::min_size().get(), 16);
        let mut payloads = Vec::new();
        for detail in [
            Vec4::new(-6.0, 1.0, 0.0, 0.7),
            Vec4::new(2.0, 0.0, 1.0, 0.4),
        ] {
            let extension = WetExtension {
                detail,
                ..default()
            };
            let parameters = WetParameters::from(&extension);
            assert_eq!(parameters.detail, detail);
            let mut actual = encase::UniformBuffer::new(Vec::<u8>::new());
            actual.write(&parameters).unwrap();
            let mut previous = encase::UniformBuffer::new(Vec::<u8>::new());
            previous.write(&detail).unwrap();
            assert_eq!(actual.as_ref(), previous.as_ref());
            payloads.push(actual.into_inner());
            assert_eq!(
                extension.weather,
                super::super::storage::WEATHER_BUFFER_HANDLE
            );
        }
        assert_ne!(payloads[0], payloads[1]);
    }

    #[test]
    fn wet_material_preserves_standard_flags_alpha_transmission_and_shader_selection() {
        for alpha_mode in [
            AlphaMode::Opaque,
            AlphaMode::Mask(0.4),
            AlphaMode::Blend,
            AlphaMode::Premultiplied,
        ] {
            for alpha in [0.0, 0.3, 1.0] {
                let material = WetMaterial {
                    base: StandardMaterial {
                        base_color: Color::linear_rgba(0.2, 0.4, 0.6, alpha),
                        alpha_mode,
                        double_sided: true,
                        cull_mode: None,
                        unlit: true,
                        depth_bias: 3.0,
                        specular_transmission: 0.5,
                        opaque_render_method: bevy::material::OpaqueRendererMethod::Deferred,
                        ..default()
                    },
                    ..default()
                };
                let reference = ExtendedWetMaterial {
                    base: material.base.clone(),
                    extension: material.extension.clone(),
                };
                assert_eq!(material.alpha_mode(), reference.alpha_mode());
                assert_eq!(
                    material.opaque_render_method(),
                    reference.opaque_render_method()
                );
                assert_eq!(material.depth_bias(), reference.depth_bias());
                assert_eq!(
                    material.reads_view_transmission_texture(),
                    reference.reads_view_transmission_texture()
                );
                assert!(material.bind_group_data() == reference.bind_group_data());
                assert_eq!(material.base.base_color.alpha(), alpha);
            }
        }
        for (actual, expected) in [
            (
                WetMaterial::vertex_shader(),
                ExtendedWetMaterial::vertex_shader(),
            ),
            (
                WetMaterial::prepass_vertex_shader(),
                ExtendedWetMaterial::prepass_vertex_shader(),
            ),
            (
                WetMaterial::prepass_fragment_shader(),
                ExtendedWetMaterial::prepass_fragment_shader(),
            ),
            (
                WetMaterial::deferred_vertex_shader(),
                ExtendedWetMaterial::deferred_vertex_shader(),
            ),
        ] {
            assert!(match (actual, expected) {
                (ShaderRef::Default, ShaderRef::Default) => true,
                (ShaderRef::Handle(actual), ShaderRef::Handle(expected)) => actual == expected,
                (ShaderRef::Path(actual), ShaderRef::Path(expected)) => actual == expected,
                _ => false,
            });
        }
        for shader in [
            WetMaterial::fragment_shader(),
            WetMaterial::deferred_fragment_shader(),
        ] {
            assert!(
                matches!(shader, ShaderRef::Path(path) if path.to_string() == "shaders/terra_wet_surface.wesl")
            );
        }
        assert_eq!(
            WetMaterial::enable_prepass(),
            ExtendedWetMaterial::enable_prepass()
        );
        assert_eq!(
            WetMaterial::enable_shadows(),
            ExtendedWetMaterial::enable_shadows()
        );
        assert_eq!(WetMaterial::enable_oit(), ExtendedWetMaterial::enable_oit());
    }

    #[test]
    fn wet_shader_compiles_both_binding_layouts_and_preserves_stock_fade_and_pbr_paths() {
        use wesl::{
            pass::{Features, condcomp},
            resolver::VirtualResolver,
            syntax::{GlobalDeclaration, TranslationUnit},
        };
        let source = include_str!("../../../../assets/shaders/terra_wet_surface.wesl");
        for bindless in [false, true] {
            for prepass in [false, true] {
                for payload in [false, true] {
                    let mut module = source.parse::<TranslationUnit>().unwrap();
                    let mut features = Features::new();
                    features.set("BINDLESS", bindless);
                    features.set("PREPASS_PIPELINE", prepass);
                    features.set("VERTEX_UVS_B", payload);
                    features.set("VERTEX_COLORS", true);
                    condcomp(&mut module, &features).unwrap();
                    let text = module.to_string();
                    assert_eq!(text.contains("@binding(100)"), bindless);
                    assert_eq!(text.contains("binding_array<TerraWeather>"), bindless);
                    assert_eq!(text.contains("array<WetParameters>"), bindless);
                    assert_eq!(text.contains("var<uniform> terra_detail"), !bindless);
                    let function = |name: &str| {
                        module
                            .global_declarations
                            .iter()
                            .find_map(|declaration| match &**declaration {
                                GlobalDeclaration::Function(function)
                                    if function.ident.name().as_str() == name =>
                                {
                                    Some(function.body.to_string())
                                }
                                _ => None,
                            })
                            .unwrap()
                    };
                    let opacity = function("terrain_opacity");
                    assert!(opacity.contains("base_color.a"));
                    assert_eq!(opacity.contains("material_indices"), bindless);
                    let fragment = function("fragment");
                    for operation in [
                        "pbr_input_from_standard_material",
                        "alpha_discard",
                        "apply_decals",
                    ] {
                        assert!(fragment.contains(operation));
                    }
                    assert_eq!(fragment.contains("terrain_opacity(input)"), payload);
                    assert_eq!(fragment.contains("wet_indices[slot]"), bindless && payload);
                    assert_eq!(fragment.contains("deferred_output"), prepass);
                    if !payload {
                        assert!(!fragment.contains("terra_weather"));
                        assert!(!fragment.contains("terra_detail"));
                    }

                    // Compile the actual binding declarations without mocking PBR imports.
                    module.imports.clear();
                    module.global_declarations.retain(|declaration| {
                        matches!(
                            &**declaration,
                            GlobalDeclaration::Struct(_) | GlobalDeclaration::Declaration(_)
                        )
                    });
                    let mut resolver = VirtualResolver::new();
                    let path: wesl::syntax::ModulePath = "package::wet_bindings".parse().unwrap();
                    resolver.add_translation_unit(path.clone(), module);
                    resolver.add_module(
                        "constants".parse().unwrap(),
                        "public const MATERIAL_BIND_GROUP = 3u;".into(),
                    );
                    let mut compiler = wesl::Compiler::default().with_resolver(resolver);
                    compiler.options.keep_main = true;
                    compiler.compile_module(&path).unwrap();
                }
            }
        }
    }

    #[test]
    fn dry_depth_extension_preserves_stock_shaders_and_bindless_tables() {
        assert!(matches!(
            TerrainDepthExtension::vertex_shader(),
            ShaderRef::Default
        ));
        assert!(matches!(
            TerrainDepthExtension::fragment_shader(),
            ShaderRef::Default
        ));
        assert!(matches!(
            TerrainDepthExtension::prepass_vertex_shader(),
            ShaderRef::Default
        ));
        assert!(matches!(
            TerrainDepthExtension::prepass_fragment_shader(),
            ShaderRef::Default
        ));
        assert!(matches!(
            TerrainDepthExtension::deferred_vertex_shader(),
            ShaderRef::Default
        ));
        assert!(matches!(
            TerrainDepthExtension::deferred_fragment_shader(),
            ShaderRef::Default
        ));
        assert_eq!(
            TerrainDepthMaterial::bindless_slot_count(),
            StandardMaterial::bindless_slot_count()
        );
        let base = StandardMaterial::bindless_descriptor().unwrap();
        let combined = TerrainDepthMaterial::bindless_descriptor().unwrap();
        assert_eq!(combined.resources, base.resources);
        assert_eq!(combined.buffers.len(), base.buffers.len());
        assert_eq!(combined.index_tables.len(), base.index_tables.len());
        for (combined, base) in combined.index_tables.iter().zip(base.index_tables.iter()) {
            assert_eq!(combined.indices, base.indices);
            assert_eq!(combined.binding_number, base.binding_number);
        }
        let extension = TerrainDepthExtension::bindless_descriptor().unwrap();
        assert!(extension.resources.is_empty());
        assert!(extension.buffers.is_empty());
        assert!(extension.index_tables.is_empty());
    }

    #[test]
    fn terrain_roofs_are_two_sided_only_in_actual_depth_only_passes() {
        use bevy::pbr::MeshPipelineKey;
        use bevy::render::render_resource::{
            ColorTargetState, ColorWrites, Face, FragmentState, RenderPipelineDescriptor,
            TextureFormat,
        };
        for cull in [Some(Face::Back), Some(Face::Front), None] {
            for (prepass, writes_color, key, expected) in [
                (false, true, MeshPipelineKey::empty(), cull),
                (false, true, MeshPipelineKey::DEPTH_PREPASS, cull),
                (
                    false,
                    true,
                    MeshPipelineKey::DEPTH_PREPASS | MeshPipelineKey::NORMAL_PREPASS,
                    cull,
                ),
                (true, false, MeshPipelineKey::DEPTH_PREPASS, None),
                (
                    true,
                    false,
                    MeshPipelineKey::DEPTH_PREPASS | MeshPipelineKey::UNCLIPPED_DEPTH_ORTHO,
                    None,
                ),
                (
                    true,
                    true,
                    MeshPipelineKey::DEPTH_PREPASS | MeshPipelineKey::NORMAL_PREPASS,
                    cull,
                ),
                (
                    true,
                    true,
                    MeshPipelineKey::DEPTH_PREPASS | MeshPipelineKey::DEFERRED_PREPASS,
                    cull,
                ),
            ] {
                let mut descriptor = RenderPipelineDescriptor::default();
                descriptor.primitive.cull_mode = cull;
                if prepass {
                    descriptor
                        .vertex
                        .shader_defs
                        .push("PREPASS_PIPELINE".into());
                }
                if writes_color {
                    descriptor.fragment = Some(FragmentState {
                        targets: vec![Some(ColorTargetState {
                            format: TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: ColorWrites::ALL,
                        })],
                        ..default()
                    });
                }
                terrain_depth_culling(&mut descriptor, key);
                assert_eq!(descriptor.primitive.cull_mode, expected);
            }
        }
    }

    #[test]
    fn construction_surface_metadata_selects_shared_world_detail_without_remapping_authored_turf() {
        use crate::blocks::{
            BlockTemplate, TemplateSet,
            definitions::{BlockRenderKind, CubeSurface},
            surface::SurfaceTemplate,
        };
        let mut templates = TemplateSet::default();
        for (id, slot, style) in [
            (900, 40, Some(CubeSurface::Wood)),
            (901, 41, Some(CubeSurface::Rock)),
            (902, 42, Some(CubeSurface::Wool)),
            (903, 43, None),
        ] {
            templates.blocks.insert(
                id,
                BlockTemplate {
                    surface: style.map(SurfaceTemplate::new),
                    render_kind: BlockRenderKind::Cube,
                    bevel_width: 0.0,
                    faces: default(),
                    face_materials: [slot; 6],
                    side_cap: None,
                    details: vec![],
                    model: vec![],
                },
            );
        }
        assert_eq!(detail_family(&templates, 40), 10.0);
        assert_eq!(detail_family(&templates, 41), 11.0);
        assert_eq!(detail_family(&templates, 42), 12.0);
        assert_eq!(detail_family(&templates, 43), 13.0);
        assert_eq!(detail_family(&templates, 44), 0.0);
        let mut spirit = templates.blocks[&903].clone();
        spirit.face_materials = [45; 6];
        templates.blocks.insert(306, spirit);
        assert_eq!(detail_family(&templates, 45), 8.0);
        templates.blocks.get_mut(&903).unwrap().render_kind = BlockRenderKind::CustomMesh;
        assert_eq!(detail_family(&templates, 43), 0.0);
    }

    #[test]
    fn translucent_galle_receives_the_flow_shader_while_other_transparency_stays_stock() {
        use crate::blocks::{
            BlockTemplate, TemplateSet,
            definitions::{BlockDefinition, BlockRenderKind},
        };
        let definition: BlockDefinition =
            serde_json::from_str(include_str!("../../../../assets/terra/blocks/bile.json"))
                .unwrap();
        let alpha = definition.materials["Bile"].base_color[3];
        assert!(alpha < 1.0);
        let mut app = App::new();
        app.init_resource::<crate::blocks::BlockAssets>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<WetMaterial>>()
            .init_resource::<WetMaterials>()
            .init_resource::<Assets<TerrainDepthMaterial>>()
            .init_resource::<TerrainDepthMaterials>()
            .init_resource::<super::super::WeatherRenderStatus>()
            .add_message::<AssetEvent<StandardMaterial>>()
            .add_systems(Update, prepare);
        let mut templates = TemplateSet::default();
        for (slot, id) in [terra_core::bile::FLUID, 15].into_iter().enumerate() {
            let handle = app
                .world_mut()
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: Color::linear_rgba(1.0, 1.0, 1.0, alpha),
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                });
            app.world_mut()
                .resource_mut::<crate::blocks::BlockAssets>()
                .materials
                .push(handle);
            templates.blocks.insert(
                id,
                BlockTemplate {
                    surface: None,
                    model: Vec::new(),
                    render_kind: BlockRenderKind::Boxes,
                    face_materials: [slot; 6],
                    side_cap: None,
                    bevel_width: 0.0,
                    faces: Default::default(),
                    details: Vec::new(),
                },
            );
        }
        app.world_mut()
            .resource_mut::<crate::blocks::BlockAssets>()
            .templates = Some(std::sync::Arc::new(templates));
        app.update();
        let wet = app.world().resource::<WetMaterials>();
        let galle = wet
            .0
            .get(&0)
            .expect("translucent Galle must use the liquid shader")
            .clone();
        assert!(
            !wet.0.contains_key(&1),
            "ordinary glass retains its stock renderer"
        );
        assert!(app.world().resource::<TerrainDepthMaterials>().0.is_empty());
        let materials = app.world().resource::<Assets<WetMaterial>>();
        let material = materials.get(&galle).unwrap();
        assert_eq!(material.alpha_mode(), AlphaMode::Blend);
        assert_eq!(material.base.base_color.alpha(), alpha);
        assert_eq!(material.extension.detail.x, -14.0);
        assert_eq!(
            material.extension.weather,
            super::super::storage::WEATHER_BUFFER_HANDLE
        );
        app.update();
        assert_eq!(app.world().resource::<WetMaterials>().0[&0], galle);
        assert_eq!(app.world().resource::<Assets<WetMaterial>>().len(), 1);
    }

    #[test]
    fn surface_shader_switch_reuses_materials_and_leaves_weather_binding_enabled() {
        use crate::blocks::{BlockTemplate, TemplateSet, definitions::BlockRenderKind};
        let mut app = App::new();
        app.init_resource::<crate::blocks::BlockAssets>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<WetMaterial>>()
            .init_resource::<WetMaterials>()
            .init_resource::<Assets<TerrainDepthMaterial>>()
            .init_resource::<TerrainDepthMaterials>()
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
        let depth = app.world().resource::<TerrainDepthMaterials>().0[&0].clone();
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
            assert_eq!(app.world().resource::<TerrainDepthMaterials>().0[&0], depth);
            assert_eq!(
                app.world().resource::<Assets<TerrainDepthMaterial>>().len(),
                1
            );
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
            .init_asset::<TerrainDepthMaterial>()
            .init_resource::<TerrainDepthMaterials>()
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
            .init_resource::<Assets<TerrainDepthMaterial>>()
            .init_resource::<TerrainDepthMaterials>()
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
        let depth = app.world().resource::<TerrainDepthMaterials>().0[&0].clone();
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
        assert_eq!(app.world().resource::<TerrainDepthMaterials>().0[&0], depth);
        assert_eq!(
            app.world().resource::<Assets<TerrainDepthMaterial>>().len(),
            1
        );
        assert_eq!(
            app.world()
                .resource::<Assets<TerrainDepthMaterial>>()
                .get(&depth)
                .unwrap()
                .base
                .perceptual_roughness,
            0.73
        );
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
