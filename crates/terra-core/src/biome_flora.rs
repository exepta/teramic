//! Authored, orientable plants share empty collision; cactus stays a solid terrain cube.
pub const CACTUS: u16 = 422;
pub const FIRST_PROP: u16 = 423;
pub const LAST_PROP: u16 = 429;

pub const fn is_prop(material: u16) -> bool {
    matches!(material, FIRST_PROP..=LAST_PROP)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plants_are_noncolliding_and_cactus_retains_its_complete_voxel_support() {
        assert!(!is_prop(CACTUS));
        assert!(
            terra_voxel::MaterialId::from_raw(CACTUS)
                .unwrap()
                .is_solid()
        );
        // Ordinary solid cubes use the voxel fallback, not authored furniture boxes.
        assert_eq!(
            crate::block_shapes::builtin_boxes(CACTUS),
            crate::block_shapes::builtin_boxes(3)
        );
        for id in FIRST_PROP..=LAST_PROP {
            assert!(is_prop(id));
            assert!(!terra_voxel::MaterialId::from_raw(id).unwrap().is_solid());
            assert!(crate::block_shapes::builtin_boxes(id).is_empty());
        }
        assert!(!is_prop(LAST_PROP + 1));
    }
}
