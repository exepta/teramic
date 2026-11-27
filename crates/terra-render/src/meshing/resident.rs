use super::{builder::MeshBatch, staging::StagedMesh};
use crate::blocks::TemplateSet;
use bevy::{camera::primitives::Aabb, mesh::VertexAttributeValues, prelude::*};
use std::sync::{Arc, Weak};
use terra_voxel::meshing::BlockMesh;

#[derive(Debug, PartialEq)]
pub(crate) enum MeshSource {
    Shadow(ShadowPiece),
    Static(StaticPiece),
}

impl MeshSource {
    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::Shadow(piece) => piece.bytes,
            Self::Static(piece) => piece.bytes,
        }
    }

    /// Material registration determines whether the upload includes wind bounds
    /// and bypasses static frame compression. Capture the actual choice at staging.
    pub(super) fn set_wind_layout(&mut self, wind: bool) {
        if let Self::Static(piece) = self {
            piece.wind = wind;
        }
    }

    pub(super) fn has_wind_layout(&self) -> bool {
        matches!(self, Self::Static(piece) if piece.wind)
    }

    pub(super) fn supports_wind_layout(&self, wind: bool) -> bool {
        match self {
            Self::Static(piece) => piece.wind == wind,
            Self::Shadow(_) => true,
        }
    }
}

/// Near shadow shells depend solely on their immutable faces, templates and split ordinal.
#[derive(Debug)]
pub(crate) struct ShadowPiece {
    geometry: Weak<BlockMesh>,
    templates: Weak<TemplateSet>,
    index: usize,
    bytes: usize,
}

impl PartialEq for ShadowPiece {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.bytes == other.bytes
            && self.geometry.ptr_eq(&other.geometry)
            && self.templates.ptr_eq(&other.templates)
    }
}

impl ShadowPiece {
    pub(super) fn identify(
        batches: &mut [MeshBatch],
        geometry: &Arc<BlockMesh>,
        templates: &Arc<TemplateSet>,
    ) {
        for (index, batch) in batches
            .iter_mut()
            .filter(|b| b.shadow_only && !b.paired_leaf_shadow)
            .enumerate()
        {
            batch.source = Some(MeshSource::Shadow(Self {
                geometry: Arc::downgrade(geometry),
                templates: Arc::downgrade(templates),
                index,
                // These depth-only pieces contain a position and an index stream.
                bytes: batch.positions.len() * 12
                    + batch.compact_indices.len() * 2
                    + batch.indices.len() * 4,
            }));
        }
    }
}

/// Final immutable vertex streams. Geometry can survive unrelated edits, new
/// neighbours or a detail-level change in another material of the same section.
#[derive(Debug)]
pub(crate) struct StaticPiece {
    templates: Weak<TemplateSet>,
    material: usize,
    digest: blake3::Hash,
    depth_only: bool,
    wind: bool,
    bytes: usize,
}

impl PartialEq for StaticPiece {
    fn eq(&self, other: &Self) -> bool {
        self.material == other.material
            && self.depth_only == other.depth_only
            && self.wind == other.wind
            && self.bytes == other.bytes
            && self.digest == other.digest
            && self.templates.ptr_eq(&other.templates)
    }
}

impl StaticPiece {
    pub(super) fn identify(batches: &mut [MeshBatch], templates: &Arc<TemplateSet>) {
        for batch in batches {
            let bytes = crate::upload_budget::batch_bytes(batch);
            if bytes < 4096
                || batch.source.is_some()
                || (batch.shadow_only
                    && (!batch.normals.is_empty() || !batch.packed_normals.is_empty()))
                || !batch.growth_anchors.is_empty()
                || !batch.growth_ranges.is_empty()
                || !batch.bile_motion.is_empty()
                || templates.water_material(batch.material)
                || templates.torch_material(batch.material)
                || templates.torch_palette.contains_key(&batch.material)
                || crate::liquid_motion::Kind::material(templates, batch.material).is_some()
            {
                continue;
            }
            let tint = match &batch.half_colors {
                Some(colors @ VertexAttributeValues::Float16x4(_)) => colors.get_bytes(),
                Some(_) => continue,
                None => &[],
            };
            // All mutable stages (lighting, splitting, packing) have finished.
            // Include lengths so that different attribute layouts and index
            // formats cannot alias even if their concatenated bytes agree.
            let mut digest = blake3::Hasher::new();
            digest.update(&[
                u8::from(batch.tiled_uv_range),
                u8::from(batch.shadow_only && batch.paired_leaf_shadow),
            ]);
            for stream in [
                bytemuck::cast_slice(&batch.positions),
                bytemuck::cast_slice(&batch.normals),
                bytemuck::cast_slice(&batch.packed_normals),
                bytemuck::cast_slice(&batch.tangents),
                bytemuck::cast_slice(&batch.packed_tangents),
                bytemuck::cast_slice(&batch.uvs),
                bytemuck::cast_slice(&batch.packed_uvs),
                bytemuck::cast_slice(&batch.rooted_wind),
                bytemuck::cast_slice(&batch.surface_water),
                bytemuck::cast_slice(&batch.colors),
                bytemuck::cast_slice(&batch.packed_colors),
                tint,
                bytemuck::cast_slice(&batch.indices),
                bytemuck::cast_slice(&batch.compact_indices),
            ] {
                digest.update(&stream.len().to_le_bytes());
                digest.update(stream);
            }
            batch.source = Some(MeshSource::Static(Self {
                material: batch.material,
                templates: Arc::downgrade(templates),
                depth_only: batch.shadow_only,
                wind: false,
                digest: digest.finalize(),
                bytes,
            }));
        }
    }
}

struct Resident {
    source: MeshSource,
    mesh: AssetId<Mesh>,
    damp: bool,
    rooted: bool,
    opaque_shadow: bool,
    bounds: Option<Aabb>,
}

#[derive(Component, Default)]
pub(super) struct PublishedMeshes(Vec<Resident>);

impl PublishedMeshes {
    pub(super) fn record(&mut self, source: MeshSource, mesh: &StagedMesh) {
        if matches!(&source, MeshSource::Static(_)) || mesh.opaque_shadow {
            self.0.push(Resident {
                source,
                damp: mesh.damp,
                rooted: mesh.rooted,
                opaque_shadow: mesh.opaque_shadow,
                mesh: mesh.handle.id(),
                bounds: mesh.bounds,
            });
        }
    }

    pub(super) fn insert(self, root: &mut EntityCommands) {
        if !self.0.is_empty() {
            root.insert(self);
        }
    }

    pub(super) fn find(
        &self,
        piece: &MeshSource,
        owned: &[Handle<Mesh>],
        meshes: &Assets<Mesh>,
    ) -> Option<StagedMesh> {
        let old = self.0.iter().find(|entry| &entry.source == piece)?;
        // Turning off shadows can release the asset while keeping the section.
        let handle = owned.iter().find(|handle| handle.id() == old.mesh)?;
        if !meshes.contains(old.mesh) {
            return None;
        }
        Some(StagedMesh {
            handle: handle.clone(),
            bounds: old.bounds,
            opaque_shadow: old.opaque_shadow,
            liquid: None,
            rooted: old.rooted,
            damp: old.damp,
            coastal_quads: 0,
            far: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch() -> MeshBatch {
        MeshBatch {
            positions: vec![[0.5, 0.0, 0.0]; 384],
            compact_indices: vec![0, 1, 2],
            ..default()
        }
    }

    fn source(mut batch: MeshBatch, templates: &Arc<TemplateSet>) -> Option<MeshSource> {
        StaticPiece::identify(std::slice::from_mut(&mut batch), templates);
        batch.source
    }

    #[test]
    fn streaming_identity_includes_paint_exposure_layout_material_and_templates() {
        let templates = Arc::new(TemplateSet::default());
        let expected = source(batch(), &templates);
        assert!(expected.is_some());
        assert_eq!(source(batch(), &templates), expected);
        assert_ne!(source(batch(), &Arc::new(TemplateSet::default())), expected);
        let changes: &[fn(&mut MeshBatch)] = &[
            |b| b.positions[1][0] += 0.001,
            |b| b.compact_indices.swap(0, 1),
            |b| b.material += 1,
            |b| b.surface_water = vec![[0.6, 0.7]; 384],
            |b| b.packed_colors = vec![[100, 150, 120, 255]; 384],
            |b| b.packed_uvs = vec![[0, 1000]; 384],
            |b| b.rooted_wind = vec![[0.0, 1.0]; 384],
            |b| b.shadow_only = true,
        ];
        for modify in changes {
            let mut changed = batch();
            modify(&mut changed);
            assert_ne!(source(changed, &templates), expected);
        }
        let mut padded = source(batch(), &templates).unwrap();
        padded.set_wind_layout(true);
        assert_ne!(Some(padded), expected);
    }
}
