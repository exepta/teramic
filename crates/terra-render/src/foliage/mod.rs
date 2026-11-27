pub(crate) mod mask;
mod motes;
pub(crate) mod packing;
mod particles;

use crate::blocks::BlockAssets;
use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{render_resource::AsBindGroup, storage::ShaderBuffer},
    shader::ShaderRef,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub(crate) use mask::masked_canopy;
pub(crate) const WIND_LEAF_ROUGHNESS: f32 = 0.91;
const OAK_WIND: Vec4 = Vec4::new(0.043, 0.010, 0.85, 0.0);

// Only the current transform is certified. Motion vectors always read their
// real previous matrix, including on the first frame after a rotating fade.
const TRANSLATED_CROWN: u32 = 0x544c4546;

fn tag_translated_crowns(
    mut commands: Commands,
    changed: Query<
        (Entity, &GlobalTransform, Option<&bevy::mesh::MeshTag>),
        (
            Or<(
                With<WindFoliage>,
                With<crate::meshing::shadows::LeafShadowProxy>,
            )>,
            Changed<GlobalTransform>,
        ),
    >,
) {
    use bevy::mesh::MeshTag;
    for (entity, transform, tag) in &changed {
        let translated = transform.affine().matrix3 == bevy::math::Mat3A::IDENTITY;
        if translated && tag.is_none() {
            commands
                .entity(entity)
                .insert(MeshTag::with_type::<WindFoliage>(TRANSLATED_CROWN));
        } else if !translated && tag.is_some_and(|tag| tag.value == TRANSLATED_CROWN) {
            commands.entity(entity).remove::<MeshTag>();
        }
    }
}

/// Maximum wind displacement, also used to expand the section batch bounds.
pub(crate) const WIND_BOUNDS: f32 = 0.065 * terra_world::props::MAX_WEATHER_WIND;

#[derive(Component)]
pub struct WindFoliage;

#[derive(Component)]
pub struct WindFibre;

/// Render-fixture baseline only, never a persisted gameplay graphics option.
#[derive(Resource)]
pub struct FoliageReferenceBenchmark;

impl Plugin for FoliageReferenceBenchmark {
    fn build(&self, app: &mut App) {
        app.insert_resource(FoliageReferenceBenchmark).add_systems(
            Update,
            prepare_reference
                .after(prepare_materials)
                .before(crate::meshing::upload_sections),
        );
    }
}

#[derive(Resource, Default, Debug)]
pub struct FoliageStatus {
    pub material_variants: usize,
    pub falling_leaves: usize,
    pub particle_batches: usize,
    pub spawned_leaves: u64,
    pub rooted_fibre_materials: usize,
    pub magical_motes: usize,
    pub magical_batches: usize,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(LeafKey)]
pub(crate) struct LeafExtension {
    #[uniform(100)]
    wind: Vec4,
    #[storage(101, read_only)]
    weather: Handle<ShaderBuffer>,
    opaque_paint: bool,
    profile_shadow: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct LeafKey {
    flame: bool,
    opaque_paint: bool,
    profile_shadow: bool,
    oak_wind: bool,
}
impl From<&LeafExtension> for LeafKey {
    fn from(extension: &LeafExtension) -> Self {
        Self {
            flame: extension.wind.w > 1.5,
            opaque_paint: extension.opaque_paint,
            profile_shadow: extension.profile_shadow,
            oak_wind: extension.opaque_paint && extension.wind == OAK_WIND,
        }
    }
}

impl MaterialExtension for LeafExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_foliage.wesl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/terra_foliage.wesl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/terra_foliage_prepass.wesl".into()
    }
    fn deferred_vertex_shader() -> ShaderRef {
        "shaders/terra_foliage.wesl".into()
    }
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if key.bind_group_data.flame {
            descriptor.vertex.shader_defs.push("FLAME_GEOMETRY".into());
        }
        if key.bind_group_data.opaque_paint
            && let Some(fragment) = &mut descriptor.fragment
        {
            fragment.shader_defs.push("OPAQUE_LEAF_PAINT".into());
        }
        if key.bind_group_data.profile_shadow
            && let Some(fragment) = &mut descriptor.fragment
        {
            fragment.shader_defs.push("LEAF_SHADOW_PROFILE".into());
        }
        // Mining pieces and inventory previews share the leaf material but retain
        // float vertex colors. Decode only the packed section vertex layout.
        if layout.0.contains(Mesh::ATTRIBUTE_COLOR)
            && layout
                .0
                .get_layout(&[Mesh::ATTRIBUTE_COLOR.at_shader_location(5)])?
                .attributes[0]
                .format
                == bevy::render::render_resource::VertexFormat::Unorm8x4
        {
            descriptor
                .vertex
                .shader_defs
                .push("PACKED_LEAF_COLOR".into());
            if key.bind_group_data.oak_wind && !layout.0.contains(Mesh::ATTRIBUTE_UV_1) {
                descriptor.vertex.shader_defs.push("DRY_OAK_WIND".into());
            }
        }
        Ok(())
    }
}

pub(crate) type LeafMaterial = ExtendedMaterial<StandardMaterial, LeafExtension>;

#[derive(Resource, Default)]
pub(crate) struct FoliageAssets {
    pub materials: HashMap<usize, Handle<LeafMaterial>>,
    /// Snapshot shared with chunk workers. Only registered crown materials enter here.
    pub primary_uv_only: Arc<HashSet<usize>>,
    /// Leaf material slots with immutable, verified opaque pigment in the depth shader.
    pub opaque_blades: Arc<HashSet<usize>>,
    pub profile_shadows: HashMap<usize, Handle<LeafMaterial>>,
    pub fibres: HashMap<usize, Handle<LeafMaterial>>,
    pub flames: HashMap<usize, Handle<LeafMaterial>>,
    pub reference: HashMap<usize, Handle<StandardMaterial>>,
}

pub(crate) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<LeafMaterial>::default())
        .init_resource::<FoliageAssets>()
        .init_resource::<FoliageStatus>()
        .add_systems(
            PostUpdate,
            tag_translated_crowns.after(TransformSystems::Propagate),
        );
    particles::install(app);
    motes::install(app);
}

pub(crate) fn prepare_materials(
    blocks: Res<BlockAssets>,
    standard: Res<Assets<StandardMaterial>>,
    mut assets: ResMut<FoliageAssets>,
    mut materials: ResMut<Assets<LeafMaterial>>,
    mut status: ResMut<FoliageStatus>,
) {
    let Some(templates) = blocks.templates.as_ref() else {
        return;
    };
    // Material slots, not texture names, isolate leaf cubes from terrain,
    // snow caps, trunks and custom models that may reuse the same image.
    if let Some(torch) = templates.blocks.get(&40) {
        for primitive in &torch.model {
            let slot = primitive.material;
            if assets.flames.contains_key(&slot) {
                continue;
            }
            let Some(base) = blocks.materials.get(slot).and_then(|h| standard.get(h)) else {
                continue;
            };
            if base.emissive.red + base.emissive.green + base.emissive.blue < 0.01 {
                continue;
            }
            let mut base = base.clone();
            base.unlit = true;
            base.emissive *= 1.1;
            assets.flames.insert(
                slot,
                materials.add(ExtendedMaterial {
                    base,
                    extension: LeafExtension {
                        wind: Vec4::new(0.035, 0.050, 4.5, 2.0),
                        weather: crate::weather::storage::WEATHER_BUFFER_HANDLE.clone(),
                        opaque_paint: false,
                        profile_shadow: false,
                    },
                }),
            );
        }
    }
    for id in terra_voxel::MaterialId::FOLIAGE {
        let Some(block) = templates.blocks.get(&id) else {
            continue;
        };
        for slot in block
            .face_materials
            .into_iter()
            .chain(
                templates
                    .sculpted_blocks
                    .get(&id)
                    .into_iter()
                    .flat_map(|s| s.core.face_materials),
            )
            .chain(
                templates
                    .foliage_clusters
                    .get(&id)
                    .into_iter()
                    .flat_map(|cluster| cluster.slots()),
            )
        {
            if assets.materials.contains_key(&slot) {
                continue;
            }
            let Some(base) = blocks.materials.get(slot).and_then(|h| standard.get(h)) else {
                continue;
            };
            let mut base = base.clone();
            base.alpha_mode = AlphaMode::Mask(0.5);
            // Cutout clusters must remain visible through gaps from either side.
            base.double_sided = true;
            base.cull_mode = None;
            base.perceptual_roughness = WIND_LEAF_ROUGHNESS;
            if packing::uses_primary_uv_only(&base) {
                Arc::make_mut(&mut assets.primary_uv_only).insert(slot);
            }
            let opaque_paint = id == 7
                && block.face_materials.contains(&slot)
                && base
                    .base_color_texture
                    .as_ref()
                    .is_some_and(|image| Some(image.id()) == blocks.opaque_oak_paint);
            if opaque_paint {
                Arc::make_mut(&mut assets.opaque_blades).insert(slot);
            }
            let handle = materials.add(ExtendedMaterial {
                base,
                extension: LeafExtension {
                    wind: OAK_WIND.with_w(
                        if matches!(id, 132 | 133 | 310..=312 | 606..=607 | 406 | 585..=586 | 644 | 647 | 682) {
                            -1.0
                        } else {
                            0.0
                        },
                    ),
                    weather: crate::weather::storage::WEATHER_BUFFER_HANDLE.clone(),
                    opaque_paint,
                    profile_shadow: false,
                },
            });
            if opaque_paint {
                let mut profile = materials
                    .get(&handle)
                    .expect("just created foliage material")
                    .clone();
                profile.extension.profile_shadow = true;
                assets.profile_shadows.insert(slot, materials.add(profile));
            }
            assets.materials.insert(slot, handle);
        }
    }
    for id in (21..=26).chain([50, 54]) {
        let Some(block) = templates.blocks.get(&id) else {
            continue;
        };
        for &slot in &block.face_materials {
            if assets.fibres.contains_key(&slot) {
                continue;
            }
            let Some(base) = blocks
                .materials
                .get(slot)
                .and_then(|handle| standard.get(handle))
            else {
                continue;
            };
            let handle = materials.add(ExtendedMaterial {
                base: base.clone(),
                extension: LeafExtension {
                    wind: Vec4::new(0.048, 0.009, 0.85, 1.0),
                    weather: crate::weather::storage::WEATHER_BUFFER_HANDLE.clone(),
                    opaque_paint: false,
                    profile_shadow: false,
                },
            });
            assets.fibres.insert(slot, handle);
        }
    }
    status.material_variants = assets.materials.len();
    status.rooted_fibre_materials = assets.fibres.len();
}

fn prepare_reference(
    mut foliage: ResMut<FoliageAssets>,
    leaves: Res<Assets<LeafMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
) {
    let pending: Vec<_> = foliage
        .materials
        .iter()
        .filter(|(slot, _)| !foliage.reference.contains_key(slot))
        .map(|(&slot, handle)| (slot, handle.clone()))
        .collect();
    for (slot, handle) in pending {
        if let Some(leaf) = leaves.get(&handle) {
            let mut base = leaf.base.clone();
            base.alpha_mode = AlphaMode::Opaque;
            foliage.reference.insert(slot, standard.add(base));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{BlockTemplate, TemplateSet, definitions::BlockRenderKind};

    #[test]
    fn translated_crown_tag_follows_the_propagated_transform_and_respects_other_mesh_roles() {
        use bevy::mesh::MeshTag;
        let mut app = App::new();
        app.add_systems(Update, tag_translated_crowns);
        let exact = app
            .world_mut()
            .spawn((WindFoliage, GlobalTransform::IDENTITY))
            .id();
        let other = app
            .world_mut()
            .spawn((WindFoliage, GlobalTransform::IDENTITY, MeshTag::new(19)))
            .id();
        let mining = app.world_mut().spawn(GlobalTransform::IDENTITY).id();
        app.update();
        assert_eq!(
            app.world().get::<MeshTag>(exact).unwrap().value,
            TRANSLATED_CROWN
        );
        assert_eq!(app.world().get::<MeshTag>(other).unwrap().value, 19);
        assert!(app.world().get::<MeshTag>(mining).is_none());
        for pose in [
            Transform::from_rotation(Quat::from_rotation_x(0.2)),
            Transform::from_scale(Vec3::new(1.0, 0.9, 1.0)),
            Transform::from_scale(Vec3::new(-1.0, 1.0, 1.0)),
        ] {
            app.world_mut()
                .entity_mut(exact)
                .insert(GlobalTransform::from(pose));
            app.update();
            assert!(app.world().get::<MeshTag>(exact).is_none());
            app.world_mut()
                .entity_mut(exact)
                .insert(GlobalTransform::from_translation(Vec3::new(
                    -42.0, 3.0, 8.0,
                )));
            app.update();
            assert_eq!(
                app.world().get::<MeshTag>(exact).unwrap().value,
                TRANSLATED_CROWN
            );
        }
    }

    #[test]
    fn changing_weather_keeps_leaf_fibre_and_flame_materials_unchanged() {
        use crate::weather::storage::{WEATHER_BUFFER_HANDLE, WeatherPayload};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<LeafMaterial>()
            .init_resource::<WeatherPayload>()
            .add_systems(PostUpdate, crate::weather::storage::update);
        let mut materials = app.world_mut().resource_mut::<Assets<LeafMaterial>>();
        let leaf = materials.add(LeafMaterial {
            base: StandardMaterial::default(),
            extension: LeafExtension {
                wind: Vec4::ZERO,
                weather: WEATHER_BUFFER_HANDLE.clone(),
                opaque_paint: false,
                profile_shadow: false,
            },
        });
        let fibre = materials.add(LeafMaterial {
            base: StandardMaterial::default(),
            extension: LeafExtension {
                wind: Vec4::new(0.048, 0.009, 0.85, 1.0),
                weather: WEATHER_BUFFER_HANDLE.clone(),
                opaque_paint: false,
                profile_shadow: false,
            },
        });
        let flame = materials.add(LeafMaterial {
            base: StandardMaterial::default(),
            extension: LeafExtension {
                wind: Vec4::new(0.0, 0.0, 0.0, 2.0),
                weather: WEATHER_BUFFER_HANDLE.clone(),
                opaque_paint: false,
                profile_shadow: false,
            },
        });
        app.update();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<LeafMaterial>>>()
            .clear();
        let shapes: Vec<_> = [&leaf, &fibre, &flame]
            .into_iter()
            .map(|handle| {
                app.world()
                    .resource::<Assets<LeafMaterial>>()
                    .get(handle)
                    .unwrap()
                    .extension
                    .wind
            })
            .collect();
        for step in 1..=32 {
            let visual = crate::weather::WeatherVisual {
                wind: Vec2::new(step as f32 * 0.25, 2.0),
                ..default()
            };
            let expected = crate::weather::foliage_wind(Some(&visual));
            app.insert_resource(visual);
            app.update();
            assert_eq!(
                app.world_mut()
                    .resource_mut::<Messages<AssetEvent<LeafMaterial>>>()
                    .drain()
                    .filter(|event| matches!(event, AssetEvent::Modified { .. }))
                    .count(),
                0
            );
            assert_eq!(app.world().resource::<WeatherPayload>().0.wind, expected);
            let materials = app.world().resource::<Assets<LeafMaterial>>();
            for (handle, shape) in [&leaf, &fibre, &flame].into_iter().zip(&shapes) {
                let extension = &materials.get(handle).unwrap().extension;
                assert_eq!(extension.weather, WEATHER_BUFFER_HANDLE);
                assert_eq!(extension.wind, *shape);
            }
        }
    }

    #[test]
    fn only_flame_materials_compile_the_fire_vertex_path() {
        let key = |mode| {
            LeafKey::from(&LeafExtension {
                wind: Vec4::new(0.04, 0.01, 0.85, mode),
                weather: crate::weather::storage::WEATHER_BUFFER_HANDLE.clone(),
                opaque_paint: false,
                profile_shadow: false,
            })
        };
        assert_eq!(key(0.0), key(1.0));
        assert_eq!(key(-1.0), key(0.0));
        assert!(!key(0.0).flame);
        assert!(key(2.0).flame);
        assert_ne!(key(1.0), key(2.0));
    }

    #[test]
    fn seasonal_leaf_alpha_is_masked_in_depth_and_shadow_prepasses_too() {
        let ShaderRef::Path(path) = LeafExtension::prepass_fragment_shader() else {
            panic!("Leaves require their vertex-aware prepass fragment shader");
        };
        assert_eq!(path.to_string(), "shaders/terra_foliage_prepass.wesl");
        let shader = include_str!("../../../../assets/shaders/terra_foliage_prepass.wesl");
        assert!(shader.contains("if wind.w >= 0.5"));
        assert!(
            shader.contains("pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);")
        );
        assert!(shader.contains("output_color.a *= in.color.a;"));
        assert!(shader.contains("if output_color.a < alpha_cutoff"));
        assert!(shader.contains("out.normal ="));
        assert!(
            shader.contains("out.motion_vector = pbr_prepass_functions::calculate_motion_vector")
        );
        assert!(shader.contains("out.frag_depth = in.unclipped_depth;"));
        assert_eq!(shader.matches("leaf_alpha_discard(in);").count(), 2);
        // Both partial loss and complete winter loss must expose the wood
        // behind the crown; an unchanged texture mask alone cannot do that.
        let cutoff = 0.5;
        for (texture_alpha, seasonal_alpha, discarded) in [
            (0.8, 0.55, true),
            (1.0, 0.0, true),
            (1.0, 1.0, false),
            (0.2, 1.0, true),
        ] {
            assert_eq!(texture_alpha * seasonal_alpha < cutoff, discarded);
        }
        assert!(matches!(
            LeafExtension::fragment_shader(),
            ShaderRef::Default
        ));
        assert!(matches!(
            LeafExtension::deferred_fragment_shader(),
            ShaderRef::Default
        ));
    }

    #[test]
    fn cap_fibre_materials_preserve_opaque_base_without_leaf_masks_or_extra_per_strip_materials() {
        let mut app = App::new();
        app.init_resource::<BlockAssets>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<LeafMaterial>>()
            .init_resource::<FoliageAssets>()
            .init_resource::<FoliageStatus>()
            .add_systems(Update, prepare_materials);
        let base = StandardMaterial {
            base_color: Color::srgb(0.8, 0.2, 0.1),
            alpha_mode: AlphaMode::Opaque,
            perceptual_roughness: 0.8,
            ..default()
        };
        let handle = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(base.clone());
        app.world_mut()
            .resource_mut::<BlockAssets>()
            .materials
            .push(handle);
        let mut templates = TemplateSet::default();
        for id in 21..=26 {
            templates.blocks.insert(
                id,
                BlockTemplate {
                    render_kind: BlockRenderKind::Cube,
                    face_materials: [0; 6],
                    side_cap: None,
                    faces: Default::default(),
                    model: Vec::new(),
                    details: Vec::new(),
                    bevel_width: 0.0,
                    surface: None,
                },
            );
        }
        app.world_mut().resource_mut::<BlockAssets>().templates =
            Some(std::sync::Arc::new(templates));
        app.update();
        app.update();
        let assets = app.world().resource::<FoliageAssets>();
        assert!(assets.materials.is_empty());
        assert_eq!(assets.fibres.len(), 1);
        let material = app
            .world()
            .resource::<Assets<LeafMaterial>>()
            .get(&assets.fibres[&0])
            .unwrap();
        assert_eq!(material.base.alpha_mode, AlphaMode::Opaque);
        assert_eq!(material.base.base_color, base.base_color);
        assert_eq!(
            material.base.perceptual_roughness,
            base.perceptual_roughness
        );
        assert_eq!(material.extension.wind.w, 1.0);
        assert_eq!(
            app.world()
                .resource::<FoliageStatus>()
                .rooted_fibre_materials,
            1
        );
    }

    #[test]
    fn only_leaf_slots_receive_shared_masked_materials() {
        for reference in [false, true] {
            let mut app = App::new();
            app.init_resource::<BlockAssets>()
                .init_resource::<Assets<StandardMaterial>>()
                .init_resource::<Assets<LeafMaterial>>()
                .init_resource::<FoliageAssets>()
                .init_resource::<FoliageStatus>()
                .add_systems(Update, prepare_materials);
            if reference {
                app.add_plugins(FoliageReferenceBenchmark);
            }
            let mut templates = TemplateSet::default();
            for (index, id) in (1..=10).chain([406]).enumerate() {
                let base = app
                    .world_mut()
                    .resource_mut::<Assets<StandardMaterial>>()
                    .add(StandardMaterial {
                        base_color: Color::linear_rgb((index + 1) as f32 / 11.0, 0.6, 0.2),
                        ..default()
                    });
                let slot = app.world().resource::<BlockAssets>().materials.len();
                app.world_mut()
                    .resource_mut::<BlockAssets>()
                    .materials
                    .push(base);
                templates.blocks.insert(
                    id,
                    BlockTemplate {
                        surface: None,
                        render_kind: BlockRenderKind::Cube,
                        bevel_width: 0.0,
                        faces: Default::default(),
                        face_materials: [slot; 6],
                        side_cap: None,
                        details: Vec::new(),
                        model: Vec::new(),
                    },
                );
            }
            for (id, slot, needles) in [(7, 11, Some(12)), (406, 13, None)] {
                templates.foliage_clusters.insert(
                    id,
                    crate::blocks::FoliageClusterMaterials {
                        leaves: slot,
                        needles,
                    },
                );
                for _ in std::iter::once(slot).chain(needles) {
                    let base = app
                        .world_mut()
                        .resource_mut::<Assets<StandardMaterial>>()
                        .add(StandardMaterial::default());
                    app.world_mut()
                        .resource_mut::<BlockAssets>()
                        .materials
                        .push(base);
                }
            }
            app.world_mut().resource_mut::<BlockAssets>().templates =
                Some(std::sync::Arc::new(templates));
            app.update();
            app.update();
            let assets = app.world().resource::<FoliageAssets>();
            assert_eq!(assets.materials.len(), 7);
            assert_eq!(assets.reference.len(), if reference { 7 } else { 0 });
            let materials = app.world().resource::<Assets<LeafMaterial>>();
            assert_eq!(materials.len(), 7);
            for slot in 0..14 {
                assert_eq!(
                    assets.materials.contains_key(&slot),
                    matches!(slot, 6..=8 | 10..=13)
                );
                if let Some(handle) = assets.materials.get(&slot) {
                    let material = materials.get(handle).unwrap();
                    assert_eq!(material.base.alpha_mode, AlphaMode::Mask(0.5));
                    assert_eq!(material.base.cull_mode, None);
                    assert!(material.base.double_sided);
                    assert_eq!(
                        material.base.base_color.to_linear().red,
                        if slot < 11 {
                            (slot + 1) as f32 / 11.0
                        } else {
                            1.0
                        }
                    );
                    assert_eq!(
                        material.extension.wind.w,
                        if matches!(slot, 10 | 13) { -1.0 } else { 0.0 },
                        "Blood foliage keeps its authored red through the seasons; ordinary leaves still change"
                    );
                    assert!(material.extension.wind.x + material.extension.wind.y < WIND_BOUNDS);
                    if let Some(handle) = assets.reference.get(&slot) {
                        let opaque = app
                            .world()
                            .resource::<Assets<StandardMaterial>>()
                            .get(handle)
                            .unwrap();
                        assert_eq!(opaque.alpha_mode, AlphaMode::Opaque);
                        assert_eq!(opaque.base_color, material.base.base_color);
                        assert_eq!(opaque.base_color_texture, material.base.base_color_texture);
                        assert_eq!(
                            opaque.perceptual_roughness,
                            material.base.perceptual_roughness
                        );
                        assert_eq!(opaque.cull_mode, material.base.cull_mode);
                    }
                }
            }
        }
    }
}
