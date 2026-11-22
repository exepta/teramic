//! Pot plants reuse canonical prop geometry and existing shared section materials.
use super::{DetailLevel, builder::MeshBatch};
use crate::blocks::TemplateSet;
use bevy::math::Vec3;
use std::collections::{BTreeMap, HashMap};

#[derive(Default)]
pub(super) struct Profiles(HashMap<u8, Vec<MeshBatch>>);

impl Profiles {
    pub(super) fn append(
        &mut self,
        batches: &mut BTreeMap<usize, MeshBatch>,
        state: u8,
        cell: [i32; 3],
        templates: &TemplateSet,
    ) -> Result<(), String> {
        let Some(item) = terra_core::flower_pot::contents(state) else {
            return Ok(());
        };
        let geometry = match self.0.entry(state) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let (kind, variant) = match item {
                    terra_core::ItemKind::Flower(color) => (
                        terra_world::props::PropKind::Flower,
                        terra_core::crusher::flower_variant(color),
                    ),
                    terra_core::ItemKind::Prop(terra_core::ResourcePropKind::Flower) => {
                        (terra_world::props::PropKind::Flower, 0)
                    }
                    _ => (terra_world::props::PropKind::Mushroom, 0),
                };
                let mut geometry = crate::props::template(
                    kind,
                    variant,
                    DetailLevel::Near,
                    &templates.prop_materials,
                )?;
                let mut low = Vec3::splat(f32::INFINITY);
                let mut high = Vec3::splat(f32::NEG_INFINITY);
                for p in geometry.iter().flat_map(|mesh| &mesh.positions) {
                    low = low.min(Vec3::from_array(*p));
                    high = high.max(Vec3::from_array(*p));
                }
                let size = high - low;
                if !size.is_finite() || size.min_element() <= 0.0 {
                    return Err("Invalid flower pot plant geometry".into());
                }
                let scale = (0.55 / size.y).min(0.58 / size.x.max(size.z));
                let root = Vec3::new((low.x + high.x) * 0.5, low.y, (low.z + high.z) * 0.5);
                for mesh in &mut geometry {
                    for p in &mut mesh.positions {
                        *p = ((Vec3::from_array(*p) - root) * scale + Vec3::new(0.5, 0.28, 0.5))
                            .to_array();
                    }
                }
                entry.insert(geometry)
            }
        };
        let offset = Vec3::from_array(cell.map(|v| v as f32));
        for mesh in geometry {
            let batch = batches.entry(mesh.material).or_insert_with(|| MeshBatch {
                material: mesh.material,
                ..Default::default()
            });
            let base =
                u32::try_from(batch.positions.len()).map_err(|_| "Flower pot batch overflow")?;
            batch.colors.resize(batch.positions.len(), [1.0; 4]);
            batch.positions.extend(
                mesh.positions
                    .iter()
                    .map(|p| (Vec3::from_array(*p) + offset).to_array()),
            );
            batch.normals.extend_from_slice(&mesh.normals);
            batch.uvs.extend_from_slice(&mesh.uvs);
            batch.append_tangents(base as usize, mesh.tangents.iter().copied());
            if mesh.colors.is_empty() {
                batch
                    .colors
                    .extend(std::iter::repeat_n([1.0; 4], mesh.positions.len()));
            } else {
                batch.colors.extend_from_slice(&mesh.colors);
            }
            for &index in &mesh.indices {
                batch
                    .indices
                    .push(base.checked_add(index).ok_or("Flower pot index overflow")?);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_potted_plants_are_grounded_fitted_and_batched_with_a_strictly_bounded_per_job_cache() {
        let templates = TemplateSet::default();
        let mut profiles = Profiles::default();
        let mut batches = BTreeMap::new();
        for _ in 0..3 {
            for state in 1..terra_core::flower_pot::STATES {
                profiles
                    .append(&mut batches, state, [0; 3], &templates)
                    .unwrap();
            }
        }
        assert_eq!(
            profiles.0.len(),
            usize::from(terra_core::flower_pot::STATES - 1)
        );
        assert!(!batches.is_empty());
        for mesh in batches.values() {
            assert_eq!(mesh.colors.len(), mesh.positions.len());
            assert!(
                mesh.indices
                    .iter()
                    .all(|&i| (i as usize) < mesh.positions.len())
            );
            assert!(mesh.positions.iter().all(|p| p[0] >= 0.20
                && p[0] <= 0.80
                && p[1] >= 0.2799
                && p[1] <= 0.8301
                && p[2] >= 0.20
                && p[2] <= 0.80));
        }
        let count: usize = batches.values().map(|mesh| mesh.positions.len()).sum();
        profiles
            .append(&mut batches, 0, [0; 3], &templates)
            .unwrap();
        assert_eq!(
            batches
                .values()
                .map(|mesh| mesh.positions.len())
                .sum::<usize>(),
            count
        );
    }
}
