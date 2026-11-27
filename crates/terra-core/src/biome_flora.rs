//! Orientable natural props include flowers and the carved furniture in a cherry grove.
pub const CACTUS: u16 = 422;
pub const FIRST_PROP: u16 = 423;
pub const LAST_PROP: u16 = 429;

pub const fn is_prop(material: u16) -> bool {
    matches!(material, FIRST_PROP..=LAST_PROP | 514..=515 | 521..=525 | 532..=540 | 549..=574 | 580 | 587..=595 | 599..=605 | 608..=611 | 624..=628 | 630..=631 | 648..=669 | 683..=701 | 710..=719 | 722..=723 | 732..=738 | 750..=758 | 760..=769 | 785..=790 | 792..=796)
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
        for id in (FIRST_PROP..=LAST_PROP)
            .chain(514..=515)
            .chain(521..=525)
            .chain(532..=540)
            .chain(587..=595)
            .chain(608..=611)
            .chain(648..=658)
            .chain(683..=693)
            .chain([662])
            .chain(599..=601)
            .chain(604..=605)
        {
            assert!(is_prop(id));
            assert!(!terra_voxel::MaterialId::from_raw(id).unwrap().is_solid());
            assert!(crate::block_shapes::builtin_boxes(id).is_empty());
        }
        assert!(is_prop(602) && is_prop(603));
        for (material, box_count) in [
            (terra_voxel::MaterialId::SakuraFoxStatue, 5),
            (terra_voxel::MaterialId::SakuraGardenLantern, 8),
            (terra_voxel::MaterialId::RunePillar, 3),
        ] {
            assert!(material.is_solid(), "{material:?}");
            assert_eq!(
                crate::block_shapes::builtin_boxes(material as u16).len(),
                box_count
            );
        }
        assert!(!is_prop(LAST_PROP + 1));
    }
}
