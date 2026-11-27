//! Collision rules shared by a world's workers and the colliders they publish.
use std::sync::OnceLock;
use terra_core::block_properties::BlockPropertiesCatalog;
use terra_voxel::{BlockSection, MaterialId};

pub(super) const SEPARATE_SHAPES: &[u16] = &[
    28, 29, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 99, 100, 141, 142, 143, 144, 145, 146,
    147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165,
    166, 167, 168, 169, 210, 211, 212, 213, 214, 215, 216, 217,
];

pub(super) struct CollisionMaterials {
    pub catalog: BlockPropertiesCatalog,
    cubes: Box<[bool]>,
    modeled: Box<[OnceLock<bool>]>,
}

impl CollisionMaterials {
    pub fn new(catalog: &BlockPropertiesCatalog) -> Self {
        // Unknown u16 IDs are solid cubes until their catalog supplies a shape.
        let mut cubes: Box<[_]> = (0..=u16::MAX)
            .map(|id| MaterialId::from_raw(id).is_none_or(MaterialId::is_solid))
            .collect();
        for id in catalog.1.keys().chain(SEPARATE_SHAPES) {
            cubes[usize::from(*id)] = false;
        }
        for id in terra_core::locomotive_parts::FIRST..=terra_core::locomotive_parts::LAST {
            cubes[usize::from(id)] = false;
        }
        Self {
            catalog: catalog.clone(),
            cubes,
            // Built-in shapes can load authored asset data on first access.
            // Only look up the materials actually seen in this world.
            modeled: (0..=u16::MAX).map(|_| OnceLock::new()).collect(),
        }
    }

    #[inline]
    pub fn cube(&self, id: u16) -> bool {
        self.cubes[usize::from(id)]
    }

    #[inline]
    pub fn has_shape(&self, id: u16) -> bool {
        *self.modeled[usize::from(id)].get_or_init(|| !self.catalog.shape(id).is_empty())
    }

    pub fn section_flags(&self, section: &BlockSection) -> (bool, bool) {
        let mut previous = None;
        let mut shaped = false;
        let mut crystal = false;
        for &material in section.blocks() {
            if previous != Some(material) {
                previous = Some(material);
                shaped |= self.has_shape(material);
                crystal |= terra_world::segments::is_crystal(material);
                if shaped && crystal {
                    break;
                }
            }
        }
        (shaped, crystal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terra_core::block_shapes::ShapeBox;

    #[test]
    fn cached_rules_match_the_cube_mask_and_shape_predicate_for_the_entire_id_space() {
        let mut catalog =
            BlockPropertiesCatalog::load(&terra_core::asset_directory().join("terra/blocks"))
                .unwrap();
        catalog.1.insert(
            65000,
            Arc::from([ShapeBox {
                minimum: [0.25; 3],
                maximum: [0.75; 3],
                hinged: false,
            }]),
        );
        catalog.1.insert(65001, Arc::from([]));
        let rules = CollisionMaterials::new(&catalog);
        for id in 0..=u16::MAX {
            let separate = catalog.1.contains_key(&id)
                || SEPARATE_SHAPES.contains(&id)
                || (terra_core::locomotive_parts::FIRST..=terra_core::locomotive_parts::LAST)
                    .contains(&id);
            assert_eq!(
                rules.cube(id),
                !separate && MaterialId::from_raw(id).is_none_or(MaterialId::is_solid),
                "cube {id}"
            );
            assert_eq!(
                rules.has_shape(id),
                !catalog.shape(id).is_empty(),
                "shape {id}"
            );
        }
    }
}
