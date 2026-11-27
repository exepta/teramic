//! The rendered mesh and its mineable surface share the same fluid height and owner.
use crate::{edits::EditJournal, streaming::ChunkStore};
use bevy_math::{DQuat, DVec3};
use std::sync::LazyLock;
use terra_core::block_shapes::ShapeBox;
use terra_voxel::blocks::BlockHit;

pub fn is_floating_plant(material: u16) -> bool {
    matches!(material, 319 | 320 | 490)
}

pub(crate) fn can_float_over(material: u16) -> bool {
    matches!(material, 0 | 14 | 27)
        || (super::can_hold_fluid(material) && !super::space::partial(material))
}

#[derive(Clone, Copy)]
pub struct Attachment {
    natural_level: Option<f64>,
    pub offset: f32,
}

impl Attachment {
    pub fn at(chunks: &ChunkStore, journal: &EditJournal, cell: [i64; 3]) -> Option<Self> {
        let below = [cell[0], cell[1] - 1, cell[2]];
        let block = chunks.block_at(below)?;
        if !can_float_over(journal.block(below).unwrap_or(block)) {
            return None;
        }
        let natural_level = chunks
            .surface_column_at(cell[0], cell[2])
            .and_then(|c| c.water_columns().at(below[1]))
            .filter(|c| below[1] as f64 >= c.bed && (below[1] as f64) < c.level)
            .map(|c| {
                let capacity = f64::from(super::FLUID_CAPACITY);
                below[1] as f64 + ((c.level - below[1] as f64).min(1.) * capacity).ceil() / capacity
            });
        let mut attachment = Self {
            natural_level,
            offset: 0.,
        };
        attachment.offset = attachment.offset(cell, journal);
        Some(attachment)
    }

    pub fn offset(self, cell: [i64; 3], journal: &EditJournal) -> f32 {
        let below = [cell[0], cell[1] - 1, cell[2]];
        let level = journal.fluid(below).map_or(self.natural_level, |fluid| {
            (fluid.amount > 0).then_some(below[1] as f64 + fluid.height())
        });
        waterline_offset(cell[1], level)
    }
}

fn waterline_offset(y: i64, level: Option<f64>) -> f32 {
    level
        .filter(|level| level.is_finite() && level.ceil() == y as f64)
        .map_or(0.0, |level| (level - y as f64) as f32)
}

struct Model {
    bounds: ShapeBox,
    facets: Vec<[[f32; 3]; 3]>,
}

impl Model {
    fn load(bytes: &[u8]) -> Self {
        let scene = gltf::Gltf::from_slice(bytes).expect("built-in water plant GLB");
        let buffer = scene.blob.as_deref();
        let mut facets = Vec::new();
        let mut bounds = ShapeBox {
            minimum: [f64::INFINITY; 3],
            maximum: [f64::NEG_INFINITY; 3],
            hinged: false,
        };
        for primitive in scene.meshes().flat_map(|m| m.primitives()) {
            let reader = primitive.reader(|_| buffer);
            let vertices: Vec<_> = reader
                .read_positions()
                .expect("plant mesh positions")
                .collect();
            for vertex in &vertices {
                for (axis, &value) in vertex.iter().enumerate() {
                    bounds.minimum[axis] = bounds.minimum[axis].min(f64::from(value));
                    bounds.maximum[axis] = bounds.maximum[axis].max(f64::from(value));
                }
            }
            let indices: Vec<_> = reader
                .read_indices()
                .expect("indexed plant mesh")
                .into_u32()
                .collect();
            facets.extend(indices.chunks_exact(3).map(|tri| {
                [
                    vertices[tri[0] as usize],
                    vertices[tri[1] as usize],
                    vertices[tri[2] as usize],
                ]
            }));
        }
        Self { bounds, facets }
    }
}

fn model(material: u16) -> Option<&'static Model> {
    macro_rules! plant {
        ($file:literal) => {{
            static MODEL: LazyLock<Model> = LazyLock::new(|| {
                Model::load(include_bytes!(concat!(
                    "../../../../assets/terra/blocks/models/",
                    $file,
                    ".glb"
                )))
            });
            Some(&*MODEL)
        }};
    }
    match material {
        319 => plant!("lotus_flower"),
        320 => plant!("lily_pad"),
        490 => plant!("pond_blossoms"),
        _ => None,
    }
}

pub struct PlantSurface {
    model: &'static Model,
    owner: [i64; 3],
    translation: DVec3,
    facing: u8,
}

impl PlantSurface {
    pub fn at(
        chunks: &ChunkStore,
        journal: &EditJournal,
        owner: [i64; 3],
        material: u16,
    ) -> Option<Self> {
        let model = model(material)?;
        let offset = Attachment::at(chunks, journal, owner).map_or(0., |a| f64::from(a.offset));
        let translation = DVec3::from_array(owner.map(|v| v as f64)) + DVec3::Y * offset;
        let facing = journal.object_state(owner).unwrap_or_default().facing % 4;
        Some(Self {
            model,
            owner,
            translation,
            facing,
        })
    }

    pub fn bounds(&self) -> ShapeBox {
        let mut bounds = self.model.bounds.rotated_y(self.facing);
        for axis in 0..3 {
            bounds.minimum[axis] += self.translation[axis];
            bounds.maximum[axis] += self.translation[axis];
        }
        bounds
    }

    pub fn ray(&self, origin: [f64; 3], direction: [f64; 3], reach: f64) -> Option<BlockHit> {
        self.bounds().ray(origin, direction, reach)?;
        let center = DVec3::new(0.5, 0., 0.5);
        let rotation = DQuat::from_rotation_y(f64::from(self.facing) * std::f64::consts::FRAC_PI_2);
        let local_origin =
            (rotation.inverse() * (DVec3::from_array(origin) - self.translation - center) + center)
                .to_array();
        let local_direction = (rotation.inverse() * DVec3::from_array(direction)).to_array();
        let mut hit: Option<BlockHit> = None;
        for &triangle in &self.model.facets {
            let limit = hit.map_or(reach, |h| h.distance);
            if let Some(distance) =
                crate::ore_shapes::ray_triangle(triangle, local_origin, local_direction, limit)
            {
                let normal = rotation
                    * DVec3::from_array(crate::ore_shapes::normal(triangle).map(f64::from));
                let axis = normal.abs().max_position();
                let mut face = [0; 3];
                face[axis] = if direction[axis] > 0.0 { -1 } else { 1 };
                hit = Some(BlockHit {
                    position: self.owner,
                    distance,
                    normal: face,
                });
            }
        }
        hit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waterline_offsets_handle_negative_levels_without_moving_dry_or_submerged_models() {
        assert_eq!(waterline_offset(16, Some(15.375)), -0.625);
        assert_eq!(waterline_offset(-16, Some(-16.625)), -0.625);
        assert_eq!(waterline_offset(16, Some(16.0)), 0.0);
        for level in [None, Some(14.9), Some(16.1), Some(f64::NAN)] {
            assert_eq!(waterline_offset(16, level), 0.0);
        }
    }
    #[test]
    fn wet_plants_are_picked_on_the_mesh_across_sections_and_edits_address_the_owner() {
        use crate::{interaction_shapes, streaming::LoadedSection, water::FluidCell};
        use terra_core::block_properties::BlockPropertiesCatalog;
        use terra_voxel::{BlockSection, global_to_section};

        let owner = [-16, 0, -16];
        let lower = [-16, -1, -16];
        let props = BlockPropertiesCatalog::default();
        let mut chunks = ChunkStore::default();
        for p in [owner, lower] {
            let (section, _) = global_to_section(p);
            chunks.loaded.insert(
                section,
                LoadedSection {
                    data: BlockSection::empty(section).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        let mut journal = EditJournal::default();
        journal
            .set_fluid(
                lower,
                FluidCell {
                    amount: 3,
                    ..Default::default()
                },
            )
            .unwrap();
        let (section, local) = global_to_section(owner);
        for material in [319, 320, 490] {
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, material)
                .unwrap();
            let surface = PlantSurface::at(&chunks, &journal, owner, material).unwrap();
            let mut outline = None;
            assert!(interaction_shapes::visit_selection_boxes(
                &chunks,
                &journal,
                &props,
                owner,
                |b| outline = Some(b)
            ));
            let bounds = outline.unwrap();
            assert_eq!(bounds, surface.bounds());
            assert!(bounds.maximum[1] < -0.25 && bounds.minimum[1] == -0.625);
            let origin = [-15.65, 1.3, -15.54];
            let down = [0., -1., 0.];
            let hit =
                interaction_shapes::raycast(&chunks, &journal, &props, origin, down, 2.25).unwrap();
            assert_eq!(hit.position, owner);
            assert!(hit.distance > 1.55 && hit.distance <= 1.925);
            assert!(
                interaction_shapes::raycast(
                    &chunks,
                    &journal,
                    &props,
                    [-15.91, 1.3, -15.91],
                    down,
                    2.25
                )
                .is_none()
            );
            assert!(
                interaction_shapes::raycast_solid(&chunks, &journal, &props, origin, down, 2.25)
                    .is_none()
            );
            // This entire ray stays in the section below the owner.
            let side = interaction_shapes::raycast(
                &chunks,
                &journal,
                &props,
                [-15.99, -0.575, -15.54],
                [1., 0., 0.],
                0.9,
            )
            .unwrap();
            assert_eq!(side.position, owner);
            let (lower_section, lower_local) = global_to_section(lower);
            chunks
                .loaded
                .get_mut(&lower_section)
                .unwrap()
                .data
                .set_block(lower_local, 553)
                .unwrap();
            let in_kelp =
                interaction_shapes::raycast(&chunks, &journal, &props, origin, down, 2.25).unwrap();
            assert_eq!(in_kelp, hit);
            chunks
                .loaded
                .get_mut(&lower_section)
                .unwrap()
                .data
                .set_block(lower_local, 0)
                .unwrap();
        }
    }
}
