//! Shadow-only opaque cube shells omit subpixel relief. Leaf pigment can weld
//! lighting seams for depth while retaining every original triangle and wind tag.
//! Middle/far depth batches preserve complete opaque geometry.
use crate::{
    blocks::TemplateSet,
    meshing::{
        DetailLevel,
        builder::{MeshBatch, append_greedy},
    },
};
use bevy::{ecs::system::SystemParam, platform::collections::hash_map::Entry, prelude::*};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, LazyLock};
use terra_voxel::meshing::BlockFace;

pub(crate) const LAYER: usize = 61;
#[derive(Component)]
pub(crate) struct ShadowProxy;

/// Like the original leaf caster, this proxy stops casting during a chunk fade.
#[derive(Component)]
pub(crate) struct LeafShadowProxy;

#[path = "leaf_shadows.rs"]
mod blades;
pub(crate) use blades::PROFILE_ERROR_METRES;
pub(super) use blades::append_leaf_shadows;
pub(super) use blades::append_profiles;

/// False marks the exact caster (a seam-welded mesh for nearby crowns); true
/// marks its profile, eligible only in coarser directional maps.
#[derive(Component, Clone, Copy)]
pub(crate) struct LeafProfileCaster(pub bool);

pub(super) fn profile_shadows_enabled() -> bool {
    static ENABLED: LazyLock<bool> = LazyLock::new(|| {
        !(std::env::args().any(|a| a.starts_with("--smoke-test"))
            && std::env::var("TERRA_BENCH_WELDED_LEAF_SHADOWS").is_ok_and(|v| v == "1"))
    });
    *ENABLED
}

pub(super) fn leaf_shadows_enabled() -> bool {
    static ENABLED: LazyLock<bool> = LazyLock::new(|| {
        !full_geometry_reference()
            && !(std::env::args().any(|arg| arg.starts_with("--smoke-test"))
                && std::env::var("TERRA_BENCH_ORIGINAL_LEAF_SHADOWS").is_ok_and(|v| v == "1"))
    });
    *ENABLED
}

#[derive(Resource)]
pub(crate) struct OpaqueShadowMaterial(pub Handle<StandardMaterial>);

pub(crate) fn prepare_material(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(OpaqueShadowMaterial(materials.add(StandardMaterial {
        cull_mode: None,
        double_sided: true,
        unlit: true,
        ..default()
    })));
}

#[derive(Component)]
pub(crate) struct ReplacedShadowCaster;

#[derive(SystemParam)]
pub(super) struct ExactInputs<'w, 's> {
    cache: Local<'s, ShadowMaterialCache>,
    standard: Option<Res<'w, Assets<StandardMaterial>>>,
    pub(super) foliage: Option<Res<'w, crate::foliage::FoliageAssets>>,
    wet: Option<Res<'w, crate::weather::surfaces::WetMaterials>>,
    depth: Option<Res<'w, crate::weather::surfaces::TerrainDepthMaterials>>,
    shadow: Option<Res<'w, OpaqueShadowMaterial>>,
}

fn exact_control(smoke: bool, setting: Option<&str>, full_reference: bool) -> bool {
    !smoke || (setting != Some("0") && !full_reference)
}

fn exact_enabled() -> bool {
    static ENABLED: LazyLock<bool> = LazyLock::new(|| {
        exact_control(
            std::env::args().any(|arg| arg.starts_with("--smoke-test")),
            std::env::var("TERRA_EXACT_LOD_SHADOWS").ok().as_deref(),
            full_geometry_reference(),
        )
    });
    *ENABLED
}

#[derive(Default)]
struct ShadowMaterialCache {
    palette_revision: Option<[u32; 4]>,
    standard_revision: Option<u32>,
    candidates: Vec<(usize, AssetId<StandardMaterial>, bool)>,
    casters: Arc<BTreeSet<usize>>,
}

impl ExactInputs<'_, '_> {
    pub(super) fn capture(
        &mut self,
        blocks: &Res<crate::blocks::BlockAssets>,
    ) -> Option<Arc<BTreeSet<usize>>> {
        if !exact_enabled() || self.shadow.is_none() {
            return None;
        }
        let standard = self.standard.as_ref()?;
        let foliage = self.foliage.as_ref()?;
        let wet = self.wet.as_ref()?;
        let depth = self.depth.as_ref()?;
        let revision = [
            blocks.last_changed().get(),
            foliage.last_changed().get(),
            wet.last_changed().get(),
            depth.last_changed().get(),
        ];
        let cache = &mut *self.cache;
        if cache.palette_revision != Some(revision) {
            cache.candidates = shadow_candidates(blocks, foliage, wet, depth);
            cache.casters = Arc::default();
            cache.palette_revision = Some(revision);
            cache.standard_revision = None;
        }
        if cache.standard_revision != Some(standard.last_changed().get()) {
            cache.refresh(standard);
            cache.standard_revision = Some(standard.last_changed().get());
        }
        Some(cache.casters.clone())
    }
}

impl ShadowMaterialCache {
    fn refresh(&mut self, standard: &Assets<StandardMaterial>) {
        for (slot, id, previous) in &mut self.candidates {
            let eligible = standard.get(*id).is_some_and(|material| {
                material.alpha_mode == AlphaMode::Opaque
                    && !material.unlit
                    && material.depth_bias == 0.0
            });
            if eligible != *previous {
                // In-flight jobs keep the material classification they started with.
                let casters = Arc::make_mut(&mut self.casters);
                if eligible {
                    casters.insert(*slot);
                } else {
                    casters.remove(slot);
                }
                *previous = eligible;
            }
        }
    }
}

fn shadow_candidates(
    blocks: &crate::blocks::BlockAssets,
    foliage: &crate::foliage::FoliageAssets,
    wet: &crate::weather::surfaces::WetMaterials,
    depth: &crate::weather::surfaces::TerrainDepthMaterials,
) -> Vec<(usize, AssetId<StandardMaterial>, bool)> {
    let Some(templates) = &blocks.templates else {
        return Vec::new();
    };
    if !crate::weather::surfaces::dry_terrain_uses_shared_surface_material() {
        return Vec::new();
    }
    blocks
        .materials
        .iter()
        .enumerate()
        .filter_map(|(slot, handle)| {
            // Only these static catalog materials can share exact depth geometry.
            (wet.0.contains_key(&slot)
                && depth.0.contains_key(&slot)
                && !templates.water_material(slot)
                && !templates.glass_material(slot)
                && !templates.torch_material(slot)
                && !templates.fog_material(slot)
                && crate::liquid_motion::Kind::material(templates, slot).is_none()
                && !templates.torch_palette.contains_key(&slot)
                && !templates.ore_materials.values().any(|&ore| ore == slot)
                && !foliage.materials.contains_key(&slot)
                && !foliage.fibres.contains_key(&slot)
                && !foliage.flames.contains_key(&slot)
                && !foliage.reference.contains_key(&slot))
            .then_some((slot, handle.id(), false))
        })
        .collect()
}

fn exact_batch(batch: &MeshBatch) -> bool {
    let indices = if batch.compact_indices.is_empty() {
        batch.indices.len()
    } else {
        batch.compact_indices.len()
    };
    batch.staged.is_none()
        && !batch.shadow_only
        && batch.shadow_sources.is_empty()
        && batch.rooted_wind.is_empty()
        && batch.growth_anchors.is_empty()
        && batch.growth_ranges.is_empty()
        && !batch.positions.is_empty()
        && indices != 0
        && indices % 3 == 0
        && (batch.indices.is_empty() || batch.compact_indices.is_empty())
        && batch.positions.iter().flatten().all(|v| v.is_finite())
        && batch
            .indices
            .iter()
            .all(|&i| (i as usize) < batch.positions.len())
        && batch
            .compact_indices
            .iter()
            .all(|&i| (i as usize) < batch.positions.len())
}

/// Preserve complete static depth geometry while reducing section caster batches.
pub(super) fn append_exact(
    batches: &mut Vec<MeshBatch>,
    materials: &BTreeSet<usize>,
    level: DetailLevel,
) -> Result<(), String> {
    if level == DetailLevel::Near || materials.is_empty() {
        return Ok(());
    }
    let mut groups = BTreeMap::new();
    for batch in batches.iter().filter(|batch| !batch.shadow_only) {
        let eligible = materials.contains(&batch.material) && exact_batch(batch);
        groups
            .entry(batch.material)
            .and_modify(|safe| *safe &= eligible)
            .or_insert(eligible);
    }
    let sources: Vec<_> = batches
        .iter()
        .filter(|batch| !batch.shadow_only && groups.get(&batch.material) == Some(&true))
        .collect();
    if sources.len() < 2 {
        return Ok(());
    }
    let _scope = terra_core::profiling::ScopeTimer::new("terrain.shadows.exact");
    let mut combined = MeshBatch {
        material: sources[0].material,
        shadow_only: true,
        shadow_sources: groups
            .into_iter()
            .filter_map(|(material, safe)| safe.then_some(material))
            .collect(),
        ..default()
    };
    // The static depth material consumes only position. UV, normal and paint
    // seams in the source meshes often duplicate the very same world corner.
    // Reuse its index across faces and materials without moving any triangle.
    let mut vertices = bevy::platform::collections::HashMap::<[u32; 3], u32>::default();
    let mut remap = Vec::new();
    for batch in &sources {
        remap.clear();
        for &position in &batch.positions {
            let index = match vertices.entry(position.map(f32::to_bits)) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => {
                    let index = u32::try_from(combined.positions.len())
                        .map_err(|_| "Shadow fusion overflow")?;
                    combined.positions.push(position);
                    *entry.insert(index)
                }
            };
            remap.push(index);
        }
        combined
            .indices
            .extend(batch.indices.iter().map(|&i| remap[i as usize]));
        combined
            .indices
            .extend(batch.compact_indices.iter().map(|&i| remap[usize::from(i)]));
    }
    // Retain the normal worker handoff bounds, including indivisible quad pairs.
    let proxies = crate::upload_budget::split_batches(vec![combined])?;
    if proxies.len() < sources.len() {
        batches.extend(proxies);
    }
    Ok(())
}

pub(crate) fn enabled(graphics: Option<&terra_core::GraphicsConfig>) -> bool {
    graphics.is_none_or(|graphics| graphics.shadow_quality != terra_core::ShadowQuality::Off)
}

pub(crate) fn release_disabled(
    mut commands: Commands,
    graphics: Option<Res<terra_core::GraphicsConfig>>,
    proxies: Query<(Entity, &Mesh3d), With<ShadowProxy>>,
    originals: Query<Entity, With<ReplacedShadowCaster>>,
    profile_sources: Query<Entity, (With<LeafProfileCaster>, Without<ShadowProxy>)>,
) {
    if enabled(graphics.as_deref()) || (proxies.is_empty() && originals.is_empty()) {
        return;
    }
    let proxies: Vec<_> = proxies
        .iter()
        .map(|(entity, mesh)| (entity, mesh.id()))
        .collect();
    let originals: Vec<_> = originals.iter().collect();
    for entity in &profile_sources {
        commands.entity(entity).remove::<LeafProfileCaster>();
    }
    commands.queue(move |world: &mut World| {
        let _scope = terra_core::profiling::ScopeTimer::new("terrain.shadows.release");
        let removed: std::collections::HashSet<_> = proxies.iter().map(|(_, mesh)| *mesh).collect();
        for (entity, _) in proxies {
            let _ = world.despawn(entity);
        }
        if let Some(mut meshes) = world.get_resource_mut::<Assets<Mesh>>() {
            for &mesh in &removed {
                meshes.remove(mesh);
            }
        }
        if !removed.is_empty()
            && let Some(mut rendered) = world.get_resource_mut::<super::RenderedSections>()
        {
            for section in rendered.bypass_change_detection().sections.values_mut() {
                section.meshes.retain(|mesh| !removed.contains(&mesh.id()));
            }
        }
        for entity in originals {
            if world.get::<ReplacedShadowCaster>(entity).is_some() {
                world.entity_mut(entity).remove::<ReplacedShadowCaster>();
                // Full color geometry remains a correct caster when shadows
                // return before the next optimized section composition.
                crate::transitions::restore_proxy_caster(world, entity);
            }
        }
    });
}

pub(crate) fn append(
    batches: &mut Vec<MeshBatch>,
    faces: &[BlockFace],
    templates: &TemplateSet,
    level: DetailLevel,
) -> Result<(), String> {
    if level != DetailLevel::Near || full_geometry_reference() {
        return Ok(());
    }
    // A silhouette that projects beyond the core casts with the color geometry.
    // The shadow replacement is per material batch, which can also contain
    // accents from a different block in this section.
    let sculpted = |id| {
        templates.block_details.micro_geometry
            && templates.sculpted_blocks.contains_key(&id)
            && !super::builder::coarse_authored(templates, id)
    };
    if faces.iter().all(|face| sculpted(face.material)) {
        return Ok(());
    }
    let mut relief_materials = BTreeSet::new();
    for id in faces.iter().map(|f| f.material).collect::<BTreeSet<_>>() {
        if sculpted(id) {
            let shape = &templates.sculpted_blocks[&id];
            relief_materials.extend(shape.slots);
            relief_materials.extend(shape.core.face_materials);
        }
    }
    let mut shadows = BTreeMap::new();
    let mut flat_shadows = BTreeMap::new();
    let mut source_materials = BTreeMap::<(usize, bool), BTreeSet<usize>>::new();
    let foliage_materials: BTreeSet<_> = terra_voxel::MaterialId::FOLIAGE
        .iter()
        .filter_map(|id| templates.blocks.get(id))
        .flat_map(|block| block.face_materials)
        .chain(
            templates
                .foliage_clusters
                .values()
                .flat_map(|cluster| cluster.slots()),
        )
        .collect();
    // Color batches can combine model and cube geometry with shared materials.
    // Keep those casters intact rather than replacing the model's shadow too.
    let model_materials: BTreeSet<_> = templates
        .blocks
        .values()
        .flat_map(|block| block.model.iter().map(|primitive| primitive.material))
        .collect();
    for &face in faces {
        // A cube proxy would erase the silhouette of projecting leaf clusters.
        // Keep the real leaf batch as caster, including conifer geometry. Also
        // protect shared slots: upload replaces casters by material, not by ID.
        if sculpted(face.material)
            || terra_voxel::MaterialId::FOLIAGE.contains(&face.material)
            || matches!(
                face.material,
                399 | terra_core::fog_blocks::MATERIAL | terra_world::lava::LAVA
            )
        {
            continue;
        }
        if let Some(original) = templates.blocks.get(&face.material) {
            let flat = original.render_kind == crate::blocks::definitions::BlockRenderKind::Cube
                && templates.opaque_cube_lods.contains(&face.material)
                && templates.cube_lods.contains_key(&face.material);
            let authored = original.render_kind
                == crate::blocks::definitions::BlockRenderKind::CustomMesh
                && original.model.is_empty()
                && templates.opaque_cube_lods.contains(&face.material);
            if templates
                .sculpted_blocks
                .get(&face.material)
                .is_some_and(|shape| {
                    shape.core.face_materials
                        != templates
                            .cube_lods
                            .get(&face.material)
                            .unwrap_or(original)
                            .face_materials
                })
            {
                continue;
            }
            let block = if authored {
                let Some(coarse) = templates.cube_lods.get(&face.material) else {
                    continue;
                };
                coarse
            } else {
                original
            };
            if block.render_kind != crate::blocks::definitions::BlockRenderKind::Cube
                || relief_materials.contains(&block.face_materials[face.side])
                || relief_materials.contains(&original.face_materials[face.side])
                || foliage_materials.contains(&block.face_materials[face.side])
                || foliage_materials.contains(&original.face_materials[face.side])
                || model_materials.contains(&original.face_materials[face.side])
                || authored
                    && original.faces[face.side].iter().any(|primitive| {
                        model_materials.contains(&primitive.material)
                            || foliage_materials.contains(&primitive.material)
                    })
                || block.side_cap.is_some_and(|cap| {
                    face.side / 2 != 1
                        && (model_materials.contains(&cap.material)
                            || foliage_materials.contains(&cap.material)
                            || relief_materials.contains(&cap.material))
                })
                || templates.water_material(block.face_materials[face.side])
                || templates.bile_material(block.face_materials[face.side])
                || templates.glass_material(block.face_materials[face.side])
                || templates
                    .ore_materials
                    .get(&face.material)
                    .is_some_and(|material| block.face_materials.contains(material))
            {
                continue;
            }
            let sources = source_materials
                .entry((block.face_materials[face.side], flat))
                .or_default();
            if authored {
                sources.extend(original.faces[face.side].iter().map(|p| p.material));
            } else {
                sources.insert(block.face_materials[face.side]);
                if let Some(cap) = block.side_cap {
                    source_materials
                        .entry((cap.material, flat))
                        .or_default()
                        .insert(cap.material);
                }
            }
            // Keep flat authored fallbacks separate until shell construction:
            // a shared material slot can also contain ordinary detailed cubes.
            append_greedy(
                if flat {
                    &mut flat_shadows
                } else {
                    &mut shadows
                },
                face,
                block,
            )?;
        }
    }
    let mut opaque: Option<MeshBatch> = None;
    for (mut batch, flat) in shadows
        .into_values()
        .map(|batch| (batch, false))
        .chain(flat_shadows.into_values().map(|batch| (batch, true)))
    {
        batch.shadow_only = true;
        batch.shadow_sources = source_materials
            .remove(&(batch.material, flat))
            .map_or_else(
                || vec![batch.material],
                |sources| sources.into_iter().collect(),
            );
        // Only opaque cubes reach this path. Their depth casters need position
        // alone; the original color and collision geometry remain unchanged.
        if !flat {
            recessed_shell(&mut batch)?;
        }
        batch.normals = Vec::new();
        batch.tangents = Vec::new();
        batch.uvs = Vec::new();
        // The shared depth-only material is two-sided: cave ceilings still
        // block sunlight without submitting every triangle twice.
        if let Some(combined) = &mut opaque {
            let base = u32::try_from(combined.positions.len())
                .map_err(|_| "Shadow mesh index overflow")?;
            combined.positions.extend(batch.positions);
            for index in batch.indices {
                combined.indices.push(
                    base.checked_add(index)
                        .ok_or("Shadow mesh index overflow")?,
                );
            }
            combined.shadow_sources.extend(batch.shadow_sources);
        } else {
            opaque = Some(batch);
        }
    }
    if let Some(batch) = opaque {
        let mut batch = batch;
        batch.shadow_sources.sort_unstable();
        batch.shadow_sources.dedup();
        batches.push(batch);
    }
    Ok(())
}

fn recessed_shell(batch: &mut MeshBatch) -> Result<(), String> {
    let mut positions = Vec::with_capacity(batch.positions.len() * 2);
    let mut indices = Vec::with_capacity(batch.indices.len() * 5);
    for (quad, normal) in batch
        .positions
        .chunks_exact(4)
        .zip(batch.normals.chunks_exact(4))
    {
        let base = u32::try_from(positions.len()).map_err(|_| "Shadow shell overflow")?;
        let center: [f32; 3] =
            std::array::from_fn(|axis| quad.iter().map(|p| p[axis]).sum::<f32>() * 0.25);
        positions.extend_from_slice(quad);
        for point in quad {
            positions.push(std::array::from_fn(|axis| {
                if normal[0][axis] != 0.0 {
                    point[axis] - normal[0][axis] * 0.065
                } else {
                    let delta = center[axis] - point[axis];
                    point[axis] + delta.signum() * delta.abs().min(0.065)
                }
            }));
        }
        // Canonical shared outer edges remain closed across cells/sections.
        // Only the broad interior retreats behind the color surface relief.
        for [a, b, c, d] in [
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
            [4, 5, 6, 7],
        ] {
            indices.extend([a, b, c, a, c, d].map(|i| base + i));
        }
    }
    batch.positions = positions;
    batch.indices = indices;
    Ok(())
}

fn full_geometry_reference() -> bool {
    static FULL: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::var("TERRA_BENCH_FULL_SHADOWS").is_ok_and(|value| value == "1")
            && std::env::args().any(|arg| arg.starts_with("--smoke-test"))
    });
    *FULL
}

#[cfg(test)]
#[path = "shadows_exact_tests.rs"]
mod exact_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_coarse_near_combines_flat_proxies_without_flattening_other_cube_shells() {
        let mut templates = crate::blocks::authored_terrain_templates();
        templates.blocks.insert(1, templates.cube_lods[&1].clone());
        let grass = BlockFace {
            minimum: [0; 3],
            width: 2,
            height: 1,
            side: 3,
            u_axis: 0,
            v_axis: 2,
            material: 1,
            edge_mask: 15,
            occupied_neighbor: false,
        };
        let mut proxies = Vec::new();
        append(&mut proxies, &[grass], &templates, DetailLevel::Near).unwrap();
        assert_eq!(proxies.len(), 1);
        assert!(proxies[0].shadow_only);
        assert_eq!(proxies[0].shadow_sources, vec![4]);
        assert_eq!(proxies[0].positions.len(), 4);
        assert_eq!(proxies[0].indices.len(), 6);
        assert!(proxies[0].positions.iter().all(|point| point[1] == 1.0));
        assert!(
            proxies[0].normals.is_empty()
                && proxies[0].tangents.is_empty()
                && proxies[0].uvs.is_empty()
        );
        let stone = BlockFace {
            minimum: [2, 0, 0],
            material: 3,
            width: 1,
            ..grass
        };
        // Reusing the grass slot must not flatten the stone's recessed shell.
        for material in [4, 7] {
            templates.blocks.insert(
                3,
                crate::blocks::BlockTemplate {
                    render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                    face_materials: [material; 6],
                    faces: crate::blocks::cube_faces(0.0, [material; 6], None),
                    side_cap: None,
                    model: Vec::new(),
                    details: Vec::new(),
                    bevel_width: 0.0,
                    surface: None,
                },
            );
            proxies.clear();
            append(&mut proxies, &[grass, stone], &templates, DetailLevel::Near).unwrap();
            assert_eq!(proxies.len(), 1);
            let proxy = &proxies[0];
            assert!(proxy.shadow_only);
            assert_eq!(
                proxy.shadow_sources,
                if material == 4 { vec![4] } else { vec![4, 7] }
            );
            assert_eq!(proxy.positions.len(), 12);
            assert_eq!(proxy.indices.len(), 36);
            assert_eq!(
                proxy
                    .positions
                    .iter()
                    .filter(|point| point[1] < 1.0)
                    .count(),
                4
            );
            assert!(
                proxy
                    .indices
                    .iter()
                    .all(|&index| (index as usize) < proxy.positions.len())
            );
        }
    }

    #[test]
    fn sculpted_grass_casts_its_relief_and_legacy_authored_grass_keeps_its_shadow_shell() {
        let mut templates = crate::blocks::authored_terrain_templates();
        let mut padded = [0; terra_voxel::meshing::PADDED_VOLUME];
        for x in 1..=2 {
            padded[x + 18 * (1 + 18)] = 1;
        }
        let geometry = terra_voxel::meshing::mesh_section(&padded).unwrap();
        let compose = |templates: &TemplateSet| {
            super::super::builder::compose_section_with_options(
                &geometry.faces,
                templates,
                terra_voxel::SectionPos::default(),
                &super::super::builder::RenderOptions {
                    water_level: None,
                    detail_level: DetailLevel::Near,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: default(),
                },
            )
            .unwrap()
        };
        let sculpted = compose(&templates);
        assert!(
            sculpted
                .iter()
                .flat_map(|b| &b.positions)
                .any(|p| p[1] > 1.0)
        );
        let mut visible = compose(&templates);
        append(&mut visible, &geometry.faces, &templates, DetailLevel::Near).unwrap();
        assert_eq!(visible, sculpted);
        templates.sculpted_blocks.clear();
        let original = compose(&templates);
        let mut batches = compose(&templates);
        assert!(batches[0].indices.len() >= 10 * 128 * 3);
        append(&mut batches, &geometry.faces, &templates, DetailLevel::Near).unwrap();
        assert_eq!(&batches[..original.len()], original);
        assert_eq!(batches.len(), original.len() + 1);
        let proxy = batches.last().unwrap();
        assert!(proxy.shadow_only);
        assert_eq!(
            proxy.shadow_sources,
            vec![templates.blocks[&1].face_materials[0]]
        );
        assert!(proxy.normals.is_empty() && proxy.tangents.is_empty() && proxy.uvs.is_empty());
        assert_eq!(proxy.positions.len(), geometry.faces.len() * 8);
        assert_eq!(proxy.indices.len(), geometry.faces.len() * 30);
        assert!(
            proxy
                .positions
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
        assert!(
            proxy
                .indices
                .iter()
                .all(|&index| (index as usize) < proxy.positions.len())
        );
        assert!(
            proxy
                .positions
                .iter()
                .all(|point| point.iter().all(|&v| (0.0..=2.0).contains(&v)))
        );

        let mut model = templates.clone();
        model.blocks.get_mut(&1).unwrap().model = model.blocks[&1].faces[0].clone();
        let mut skipped = Vec::new();
        append(&mut skipped, &geometry.faces, &model, DetailLevel::Near).unwrap();
        assert!(
            skipped.is_empty(),
            "standalone custom models keep their exact caster"
        );
        append(
            &mut skipped,
            &geometry.faces,
            &templates,
            DetailLevel::Middle,
        )
        .unwrap();
        assert!(
            skipped.is_empty(),
            "the existing coarse terrain already casts its own shadow"
        );
    }

    #[test]
    fn shared_opaque_shadow_material_keeps_both_sides_without_reverse_triangles() {
        let mut app = App::new();
        app.init_resource::<Assets<StandardMaterial>>()
            .add_systems(Startup, prepare_material);
        app.update();
        let handle = &app.world().resource::<OpaqueShadowMaterial>().0;
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let material = materials.get(handle).unwrap();
        assert_eq!(material.cull_mode, None);
        assert!(material.double_sided);
        assert_eq!(material.alpha_mode, AlphaMode::Opaque);
        assert!(material.base_color_texture.is_none());
        assert_eq!(materials.len(), 1);
    }

    #[test]
    fn only_explicitly_disabled_shadow_quality_skips_proxies() {
        assert!(enabled(None));
        for quality in [
            terra_core::ShadowQuality::Off,
            terra_core::ShadowQuality::Low,
            terra_core::ShadowQuality::Medium,
            terra_core::ShadowQuality::High,
        ] {
            assert_eq!(
                enabled(Some(&terra_core::GraphicsConfig {
                    shadow_quality: quality,
                    ..default()
                })),
                quality != terra_core::ShadowQuality::Off
            );
        }
    }

    #[test]
    fn disabling_proxies_releases_active_and_retired_meshes_but_preserves_color_geometry() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<super::super::RenderedSections>()
            .init_resource::<terra_core::GraphicsConfig>()
            .add_systems(Update, release_disabled);
        let handles: Vec<_> = (0..4)
            .map(|_| {
                app.world_mut()
                    .resource_mut::<Assets<Mesh>>()
                    .add(Rectangle::new(1.0, 1.0))
            })
            .collect();
        let root = app
            .world_mut()
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        let color = app
            .world_mut()
            .spawn((
                Mesh3d(handles[0].clone()),
                ReplacedShadowCaster,
                bevy::light::NotShadowCaster,
                ChildOf(root),
            ))
            .id();
        let glass = app
            .world_mut()
            .spawn((
                Mesh3d(handles[1].clone()),
                bevy::light::NotShadowCaster,
                ChildOf(root),
            ))
            .id();
        let proxy = app
            .world_mut()
            .spawn((Mesh3d(handles[2].clone()), ShadowProxy, ChildOf(root)))
            .id();
        let retired_root = app.world_mut().spawn_empty().id();
        let retired = app
            .world_mut()
            .spawn((
                Mesh3d(handles[3].clone()),
                ShadowProxy,
                ChildOf(retired_root),
            ))
            .id();
        let position = terra_voxel::SectionPos::default();
        app.world_mut()
            .resource_mut::<super::super::RenderedSections>()
            .sections
            .insert(
                position,
                super::super::RenderedSection {
                    folded_leaves: None,
                    micro_detail: true,
                    appearance_epoch: 0,
                    entity: root,
                    meshes: handles[..3].to_vec(),
                    mesh_epoch: 0,
                    detail_level: DetailLevel::Near,
                },
            );
        app.update();
        assert!(app.world().get_entity(proxy).is_ok());
        app.world_mut()
            .resource_mut::<terra_core::GraphicsConfig>()
            .shadow_quality = terra_core::ShadowQuality::Off;
        app.update();
        assert!(app.world().get_entity(proxy).is_err());
        assert!(app.world().get_entity(retired).is_err());
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        assert_eq!(app.world().get::<Mesh3d>(color).unwrap().0, handles[0]);
        assert!(
            app.world()
                .get::<bevy::light::NotShadowCaster>(color)
                .is_none()
        );
        assert!(
            app.world()
                .get::<bevy::light::NotShadowCaster>(glass)
                .is_some()
        );
        assert!(app.world().get::<ReplacedShadowCaster>(color).is_none());
        assert_eq!(
            app.world()
                .resource::<super::super::RenderedSections>()
                .sections[&position]
                .meshes,
            handles[..2]
        );
        for quality in [
            terra_core::ShadowQuality::High,
            terra_core::ShadowQuality::Off,
        ] {
            app.world_mut()
                .resource_mut::<terra_core::GraphicsConfig>()
                .shadow_quality = quality;
            app.update();
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
            assert!(
                app.world()
                    .get::<bevy::light::NotShadowCaster>(color)
                    .is_none()
            );
            assert!(
                app.world()
                    .get::<bevy::light::NotShadowCaster>(glass)
                    .is_some()
            );
        }
    }

    #[test]
    fn custom_models_sharing_a_cube_material_keep_their_original_depth_geometry() {
        let mut templates = TemplateSet::default();
        let cube = crate::blocks::BlockTemplate {
            render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
            face_materials: [4; 6],
            side_cap: None,
            faces: Default::default(),
            model: vec![],
            details: vec![],
            bevel_width: 0.22,
            surface: None,
        };
        let mut model = cube.clone();
        model.render_kind = crate::blocks::definitions::BlockRenderKind::CustomMesh;
        model.model.push(crate::blocks::PrimitiveTemplate {
            tangents: Vec::new(),
            positions: vec![],
            normals: vec![],
            uvs: vec![],
            indices: vec![],
            material: 4,
        });
        templates.blocks.insert(3, cube);
        templates.blocks.insert(20, model);
        let face = BlockFace {
            minimum: [0; 3],
            width: 16,
            height: 8,
            side: 3,
            u_axis: 0,
            v_axis: 2,
            material: 3,
            edge_mask: 15,
            occupied_neighbor: false,
        };
        let mut batches = Vec::new();
        append(&mut batches, &[face], &templates, DetailLevel::Near).unwrap();
        assert!(batches.is_empty());
        templates.blocks.get_mut(&20).unwrap().model[0].material = 5;
        append(&mut batches, &[face], &templates, DetailLevel::Near).unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].shadow_sources, vec![4]);
    }
    #[test]
    fn foliage_keeps_real_cutout_wind_and_outlying_geometry_as_its_shadow_caster() {
        for material in terra_voxel::MaterialId::FOLIAGE {
            for flat in [false, true] {
                let mut templates = TemplateSet::default();
                let block = crate::blocks::BlockTemplate {
                    render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                    face_materials: [4; 6],
                    side_cap: None,
                    faces: Default::default(),
                    model: vec![],
                    details: vec![],
                    bevel_width: 0.0,
                    surface: None,
                };
                templates.blocks.insert(material, block.clone());
                if flat {
                    // Even stale coarse metadata must never flatten masked foliage.
                    templates.cube_lods.insert(material, block);
                    templates.opaque_cube_lods.insert(material);
                }
                let faces = [BlockFace {
                    minimum: [0; 3],
                    width: 1,
                    height: 1,
                    side: 3,
                    u_axis: 0,
                    v_axis: 2,
                    material,
                    edge_mask: 15,
                    occupied_neighbor: false,
                }];
                let positions = vec![[-0.1, 1.0, 0.0], [1.1, 1.1, 0.0], [0.5, 1.0, 1.0]];
                let normals = vec![[0., 1., 0.]; 3];
                let uvs = vec![[0.2, 0.1], [0.4, 0.1], [0.3, 0.4]];
                let colors = vec![[1., 0.8, 0.5, 0.25]; 3];
                let mut batches = vec![MeshBatch {
                    positions: positions.clone(),
                    normals: normals.clone(),
                    uvs: uvs.clone(),
                    colors: colors.clone(),
                    indices: vec![0, 1, 2],
                    material: 4,
                    ..default()
                }];
                append(&mut batches, &faces, &templates, DetailLevel::Near).unwrap();
                assert_eq!(batches.len(), 1, "foliage {material}, flat={flat}");
                let batch = &batches[0];
                assert!(!batch.shadow_only && batch.shadow_sources.is_empty());
                assert_eq!(batch.positions, positions);
                assert_eq!(batch.normals, normals);
                assert_eq!(batch.uvs, uvs);
                assert_eq!(batch.colors, colors);
                assert_eq!(batch.indices, [0, 1, 2]);
            }
        }
    }

    #[test]
    fn opaque_proxies_never_replace_a_shared_leaf_material_slot() {
        let mut templates = TemplateSet::default();
        templates.foliage_clusters.insert(
            7,
            crate::blocks::FoliageClusterMaterials {
                leaves: 8,
                needles: Some(9),
            },
        );
        let mut faces = Vec::new();
        for (material, slot) in [(7, 4), (1, 4), (3, 6), (4, 8), (5, 9)] {
            templates.blocks.insert(
                material,
                crate::blocks::BlockTemplate {
                    render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                    face_materials: [slot; 6],
                    side_cap: None,
                    faces: Default::default(),
                    model: vec![],
                    details: vec![],
                    bevel_width: 0.0,
                    surface: None,
                },
            );
            faces.push(BlockFace {
                minimum: [0; 3],
                width: 1,
                height: 1,
                side: 3,
                u_axis: 0,
                v_axis: 2,
                material,
                edge_mask: 15,
                occupied_neighbor: false,
            });
        }
        let mut batches = Vec::new();
        append(&mut batches, &faces, &templates, DetailLevel::Near).unwrap();
        assert_eq!(
            batches.len(),
            1,
            "the independent stone slot still gets a proxy"
        );
        assert!(batches[0].shadow_only);
        assert_eq!(batches[0].shadow_sources, [6]);
        assert!(batches[0].normals.is_empty() && batches[0].uvs.is_empty());
    }

    #[test]
    fn opaque_cube_proxy_uses_positions_only_and_custom_geometry_keeps_its_own_shadow() {
        let mut templates = TemplateSet::default();
        let cube = crate::blocks::BlockTemplate {
            render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
            face_materials: [4; 6],
            side_cap: None,
            faces: Default::default(),
            model: vec![],
            details: vec![],
            bevel_width: 0.22,
            surface: None,
        };
        let mut custom = cube.clone();
        custom.render_kind = crate::blocks::definitions::BlockRenderKind::CustomMesh;
        let mut second_cube = cube.clone();
        second_cube.face_materials = [5; 6];
        templates.blocks.insert(3, cube);
        templates.blocks.insert(4, second_cube);
        templates.blocks.insert(20, custom);
        let face = BlockFace {
            minimum: [0; 3],
            width: 16,
            height: 8,
            side: 3,
            u_axis: 0,
            v_axis: 2,
            material: 3,
            edge_mask: 15,
            occupied_neighbor: false,
        };
        let mut batches = Vec::new();
        append(&mut batches, &[face], &templates, DetailLevel::Near).unwrap();
        assert_eq!(batches.len(), 1);
        assert!(batches[0].shadow_only);
        assert_eq!(batches[0].positions.len(), 8);
        assert_eq!(batches[0].indices.len(), 30);
        assert!(batches[0].normals.is_empty());
        assert!(batches[0].uvs.is_empty());
        batches.clear();
        append(
            &mut batches,
            &[
                face,
                BlockFace {
                    material: 4,
                    ..face
                },
            ],
            &templates,
            DetailLevel::Near,
        )
        .unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].positions.len(), 16);
        assert_eq!(batches[0].shadow_sources, vec![4, 5]);
        assert!(batches[0].indices.iter().all(|&index| index < 16));
        batches.clear();
        append(
            &mut batches,
            &[BlockFace {
                material: 20,
                ..face
            }],
            &templates,
            DetailLevel::Near,
        )
        .unwrap();
        assert!(batches.is_empty());
        append(&mut batches, &[face], &templates, DetailLevel::Middle).unwrap();
        assert!(batches.is_empty());
    }
}
