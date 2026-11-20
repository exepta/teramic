//! One persistent density texture and one implicit-surface draw for the near domain.
use super::*;
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    image::{ImageSampler, ImageSamplerDescriptor},
    mesh::MeshVertexBufferLayoutRef,
    render::render_resource::*,
    shader::ShaderRef,
};
pub(super) const FIELD_IMAGE: Handle<Image> = uuid_handle!("c08180d2-3b15-4548-8a60-e7072a909103");
pub(super) const TAR_IMAGE: Handle<Image> = uuid_handle!("63919d2b-217b-418d-92ea-af7bce61a0b0");
const REST_IMAGE: Handle<Image> = uuid_handle!("93066d42-4234-465a-a195-560d46e600a1");
const REST_TAR_IMAGE: Handle<Image> = uuid_handle!("93066d42-4234-465a-a195-560d46e600a2");
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct FluidMaterial {
    #[texture(0, dimension = "3d")]
    #[sampler(1)]
    field: Handle<Image>,
    #[texture(2, dimension = "2d_array")]
    #[sampler(3)]
    tar: Handle<Image>,
    #[uniform(4)]
    domain: Vec4,
    #[uniform(5)]
    settings: Vec4,
    #[texture(6, dimension = "3d")]
    #[sampler(7)]
    rest_field: Handle<Image>,
    #[texture(8, dimension = "2d_array")]
    #[sampler(9)]
    rest_tar: Handle<Image>,
    #[uniform(10)]
    rest_origin: Vec4,
}
impl Material for FluidMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_particle_surface.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
        ])?];
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_write_enabled = Some(true);
            depth.bias = default();
        }
        Ok(())
    }
}
#[derive(Resource)]
struct Surface {
    entity: Entity,
    material: Handle<FluidMaterial>,
}
/// Active and parked fluid geometry share the same material and optical shader.
#[derive(Resource)]
pub(super) struct ParkedMaterials(pub [Handle<FluidMaterial>; 4]);
/// Confirmed confined water volumes share world-fluid optics without competing
/// with the captured world domain: basin and pipe contents have their own bounds.
#[derive(Resource)]
pub(crate) struct HydraulicMaterials(pub [Handle<FluidMaterial>; 4]);
impl HydraulicMaterials {
    pub(crate) fn for_slot(
        &self,
        templates: &crate::blocks::TemplateSet,
        slot: usize,
    ) -> Option<Handle<FluidMaterial>> {
        [
            (14, WATER),
            (27, WATER),
            (terra_core::bile::FLUID, GALLE),
            (terra_core::roads::WET_TAR, TAR),
        ]
        .into_iter()
        .find_map(|(id, kind)| {
            templates
                .blocks
                .get(&id)
                .is_some_and(|b| b.face_materials.contains(&slot))
                .then(|| self.0[kind as usize].clone())
        })
    }
}
pub(super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<FluidMaterial>::default())
        .add_systems(Startup, setup)
        .add_systems(
            PostUpdate,
            update.before(bevy::transform::TransformSystems::Propagate),
        );
}
fn image(dimension: TextureDimension, depth: u32) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: depth,
        },
        dimension,
        vec![0; 128 * 128 * depth as usize * 8],
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage =
        TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}
pub(crate) fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FluidMaterial>>,
) {
    let _ = images.insert(FIELD_IMAGE.id(), image(TextureDimension::D3, 128));
    let _ = images.insert(TAR_IMAGE.id(), image(TextureDimension::D2, 16));
    let empty = RestFluidGrid::default();
    let _ = images.insert(REST_IMAGE.id(), rest_image(&empty));
    let _ = images.insert(REST_TAR_IMAGE.id(), rest_tar_image(&empty));
    let material = materials.add(FluidMaterial {
        field: FIELD_IMAGE,
        tar: TAR_IMAGE,
        domain: Vec4::ZERO,
        settings: Vec4::new(1., 0., 0., 0.),
        rest_field: REST_IMAGE,
        rest_tar: REST_TAR_IMAGE,
        rest_origin: Vec4::ZERO,
    });
    commands.insert_resource(ParkedMaterials(std::array::from_fn(|kind| {
        materials.add(FluidMaterial {
            field: FIELD_IMAGE,
            tar: TAR_IMAGE,
            domain: Vec4::ZERO,
            settings: Vec4::new(1., 0., 0., kind as f32 + 1.),
            rest_field: REST_IMAGE,
            rest_tar: REST_TAR_IMAGE,
            rest_origin: Vec4::ZERO,
        })
    })));
    commands.insert_resource(HydraulicMaterials(std::array::from_fn(|kind| {
        materials.add(FluidMaterial {
            field: FIELD_IMAGE,
            tar: TAR_IMAGE,
            domain: Vec4::ZERO,
            settings: Vec4::new(1., 0., 0., kind as f32 + 5.),
            rest_field: REST_IMAGE,
            rest_tar: REST_TAR_IMAGE,
            rest_origin: Vec4::ZERO,
        })
    })));
    let entity = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::from_size(Vec3::splat(DOMAIN_SIZE)))),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::splat(DOMAIN_SIZE * 0.5)),
            crate::miniature::layers(),
            // Prewarm the surface pipeline while the initial, empty domain is
            // outside the camera. Its uniform discards empty draws cheaply.
            bevy::camera::visibility::NoFrustumCulling,
            bevy::light::NotShadowCaster,
            Name::new("GPU position-based fluid surface"),
        ))
        .id();
    commands.insert_resource(Surface { entity, material });
}
#[allow(clippy::too_many_arguments)]
fn update(
    input: Res<ParticleFluidInput>,
    status: Res<ParticleFluidStatus>,
    surface: Option<Res<Surface>>,
    parked: Option<Res<ParkedMaterials>>,
    hydraulic: Option<Res<HydraulicMaterials>>,
    rest: Res<RestFluidGrid>,
    mut images: ResMut<Assets<Image>>,
    mut uploaded: Local<u64>,
    daylight: Option<Res<crate::scene::daylight::Daylight>>,
    mut materials: ResMut<Assets<FluidMaterial>>,
    mut transforms: Query<&mut Transform>,
) {
    let Some(surface) = surface else {
        return;
    };
    let domain = super::coverage_uniform(Some(&input), Some(&status));
    if *uploaded != rest.revision {
        let _ = images.insert(REST_IMAGE.id(), rest_image(&rest));
        let _ = images.insert(REST_TAR_IMAGE.id(), rest_tar_image(&rest));
        *uploaded = rest.revision;
    }
    let rest_origin = Vec3::from_array(rest.origin)
        .extend(f32::from(rest.revision > 0 && rest.origin == input.origin));
    let daylight = daylight
        .as_ref()
        .map_or(1.0, |day| day.daylight)
        .clamp(0.12, 1.0);
    let settings = Vec4::new(
        (daylight * 128.0).round() / 128.0,
        f32::from(input.target_mass[TAR as usize] > 0.0),
        f32::from(input.target_mass[GALLE as usize] > 0.0),
        0.0,
    );
    if materials.get(&surface.material).is_some_and(|m| {
        m.domain != domain || m.settings != settings || m.rest_origin != rest_origin
    }) && let Some(mut material) = materials.get_mut(&surface.material)
    {
        material.domain = domain;
        material.settings = settings;
        material.rest_origin = rest_origin;
    }
    if let Some(parked) = parked {
        for handle in &parked.0 {
            if materials
                .get(handle)
                .is_some_and(|m| m.domain != domain || m.settings.x != settings.x)
                && let Some(mut material) = materials.get_mut(handle)
            {
                material.domain = domain;
                material.settings.x = settings.x;
            }
        }
    }
    if let Some(hydraulic) = hydraulic {
        for handle in &hydraulic.0 {
            if materials
                .get(handle)
                .is_some_and(|m| m.settings.x != settings.x)
                && let Some(mut material) = materials.get_mut(handle)
            {
                material.settings.x = settings.x;
            }
        }
    }
    let translation = Vec3::from_array(input.origin) + Vec3::splat(DOMAIN_SIZE * 0.5);
    if let Ok(mut transform) = transforms.get_mut(surface.entity)
        && transform.translation != translation
    {
        transform.translation = translation;
    }
}

// Signed fields stay in[-1,1]; films stay in[0,16]. The explicit conversion
// keeps these small CPU uploads independent of a GPU readback or a new crate.
fn half(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 255) as i32 - 127 + 15;
    if exponent <= 0 {
        return sign;
    }
    if exponent >= 31 {
        return sign | 0x7c00;
    }
    let rounded = (bits & 0x7f_ffff) + 0x1000;
    if rounded & 0x80_0000 != 0 {
        return sign | (((exponent + 1) as u16) << 10);
    }
    sign | ((exponent as u16) << 10) | ((rounded >> 13) as u16 & 1023)
}
fn rest_texture(dimension: TextureDimension, depth: u32, data: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: REST_SIDE as u32,
            height: REST_SIDE as u32,
            depth_or_array_layers: depth,
        },
        dimension,
        data,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}
fn rest_image(rest: &RestFluidGrid) -> Image {
    let mut data = Vec::with_capacity(REST_NODES * 8);
    for i in 0..REST_NODES {
        for kind in 0..4 {
            let value = rest.fields[kind]
                .get(i)
                .copied()
                .unwrap_or(-1.)
                .clamp(-1., 1.);
            data.extend_from_slice(&half(value).to_le_bytes());
        }
    }
    rest_texture(TextureDimension::D3, REST_SIDE as u32, data)
}
fn rest_tar_image(rest: &RestFluidGrid) -> Image {
    let mut values = vec![[0f32; 4]; REST_SIDE * REST_SIDE * 16];
    for film in rest.films.iter().filter(|f| f.kind == TAR) {
        let layer = film.minimum[1].floor() as i32;
        if !(0..16).contains(&layer) {
            continue;
        }
        let lo = film.minimum;
        let hi = film.maximum;
        for z in lo[2].floor().max(0.) as usize..=hi[2].ceil().min(16.) as usize {
            for x in lo[0].floor().max(0.) as usize..=hi[0].ceil().min(16.) as usize {
                let value = &mut values[x + REST_SIDE * (z + REST_SIDE * layer as usize)];
                value[0] += hi[1];
                value[1] += lo[1];
                value[3] += 1.;
                if (x as f32) < hi[0] && (z as f32) < hi[2] {
                    value[2] = 1.;
                }
            }
        }
    }
    let mut data = Vec::with_capacity(values.len() * 8);
    for value in values {
        for v in value {
            data.extend_from_slice(&half(v).to_le_bytes());
        }
    }
    rest_texture(TextureDimension::D2, 16, data)
}

#[cfg(test)]
mod reconstruction_tests {
    use super::*;
    #[test]
    fn confined_basin_pipe_and_held_liquids_keep_the_shared_optics_inside_the_world_domain() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<FluidMaterial>>()
            .init_resource::<ParticleFluidInput>()
            .init_resource::<ParticleFluidStatus>()
            .init_resource::<RestFluidGrid>()
            .insert_resource(crate::scene::daylight::Daylight {
                daylight: 0.5,
                ..default()
            })
            .add_systems(Startup, setup)
            .add_systems(PostUpdate, update);
        app.update();
        let handles = app.world().resource::<HydraulicMaterials>().0.clone();
        let mut templates = crate::blocks::TemplateSet::default();
        for (slot, id) in [14, 27, terra_core::bile::FLUID, terra_core::roads::WET_TAR]
            .into_iter()
            .enumerate()
        {
            templates.blocks.insert(
                id,
                crate::blocks::BlockTemplate {
                    surface: None,
                    render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                    bevel_width: 0.,
                    faces: crate::blocks::cube_faces(0., [slot; 6], None),
                    face_materials: [slot; 6],
                    side_cap: None,
                    details: vec![],
                    model: vec![],
                },
            );
        }
        let hydraulic = app.world().resource::<HydraulicMaterials>();
        assert_eq!(
            hydraulic.for_slot(&templates, 0),
            Some(handles[WATER as usize].clone())
        );
        assert_eq!(
            hydraulic.for_slot(&templates, 1),
            Some(handles[WATER as usize].clone())
        );
        assert_eq!(
            hydraulic.for_slot(&templates, 2),
            Some(handles[GALLE as usize].clone())
        );
        assert_eq!(
            hydraulic.for_slot(&templates, 3),
            Some(handles[TAR as usize].clone())
        );
        assert_eq!(hydraulic.for_slot(&templates, 4), None);
        let assets = app.world().resource::<Assets<FluidMaterial>>();
        for (kind, handle) in handles.iter().enumerate() {
            let material = assets.get(handle).unwrap();
            assert_eq!(material.settings.w, kind as f32 + 5.);
            assert_eq!((material.settings.w as u32 - 1) % 4, kind as u32);
            assert_eq!(material.settings.x, 0.5);
            assert_eq!(material.domain, Vec4::ZERO);
            assert_eq!(material.field, FIELD_IMAGE);
            assert_eq!(material.tar, TAR_IMAGE);
            assert!(!FluidMaterial::enable_shadows());
        }
        app.world_mut()
            .resource_mut::<crate::scene::daylight::Daylight>()
            .daylight = 0.25;
        app.update();
        let assets = app.world().resource::<Assets<FluidMaterial>>();
        assert!(
            handles
                .iter()
                .all(|h| assets.get(h).unwrap().settings.x == 0.25)
        );
    }

    #[test]
    fn signed_node_upload_retains_the_zero_level_and_tar_stays_a_thin_legal_film() {
        assert_eq!(half(0.), 0);
        assert_eq!(half(1.), 0x3c00);
        assert_eq!(half(-1.), 0xbc00);
        assert_eq!(half(16.), 0x4c00);
        let mut rest = RestFluidGrid::default();
        rest.fields[0][0] = 0.;
        let image = rest_image(&rest);
        assert_eq!(image.data.as_ref().unwrap().len(), REST_NODES * 8);
        assert_eq!(&image.data.as_ref().unwrap()[..2], &0u16.to_le_bytes());
        rest.films.push(FluidFilm {
            minimum: [2., 3.9375, 4.],
            maximum: [3., 3.95, 5.],
            kind: TAR,
        });
        let image = rest_tar_image(&rest);
        let data = image.data.unwrap();
        let at = |x: usize, z: usize, channel: usize| {
            u16::from_le_bytes(
                data[(x + REST_SIDE * (z + REST_SIDE * 3)) * 8 + channel * 2..][..2]
                    .try_into()
                    .unwrap(),
            )
        };
        assert_eq!(at(2, 4, 2), half(1.));
        assert_eq!(at(3, 4, 2), 0);
        assert!(at(2, 4, 0) > at(2, 4, 1));
    }
}
