//! Additional material yields shared by player mining and automatic rail clearing.
use crate::{ItemKind, ItemStack};
use terra_voxel::MaterialId;

/// `None` leaves the existing block-drop policy in charge. Extra drops use a
/// coordinate hash, like leaf seed drops, so retry/order cannot reroll the loot.
pub fn blood_forest(material: u16, position: [i64; 3]) -> Option<[Option<ItemStack>; 2]> {
    let material = MaterialId::from_raw(material)?;
    let stack = |item, count| Some(ItemStack { item, count });
    let hash = position
        .into_iter()
        .fold(0x567a_7ce8_0fb1_98d3_u64, |h, axis| {
            let mut value = h ^ (axis as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
            value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            value ^ (value >> 31)
        });
    Some(match material {
        MaterialId::BloodLeaves => [
            stack(ItemKind::BloodLeaf, 1),
            hash.is_multiple_of(5).then_some(ItemStack {
                item: ItemKind::DarkSap,
                count: 1,
            }),
        ],
        MaterialId::BloodMushrooms => [
            stack(ItemKind::CrimsonMushroom, 1),
            stack(ItemKind::BloodSpore, 1),
        ],
        MaterialId::GiantBloodMushroom => [
            stack(ItemKind::CrimsonMushroom, 3),
            stack(ItemKind::BloodSpore, 2),
        ],
        MaterialId::BloodThornBush => [
            stack(ItemKind::ThornTwig, 2),
            stack(ItemKind::BloodBerry, 1),
        ],
        MaterialId::BloodFlower => [stack(ItemKind::BloodBerry, 1), None],
        MaterialId::BloodCrystalBlock | MaterialId::BloodCrystalPillar => {
            [stack(ItemKind::BloodCrystal, 2), None]
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plants_and_crystals_produce_items_and_leave_other_blocks_unchanged() {
        for (material, item, count) in [
            (406, ItemKind::BloodLeaf, 1),
            (412, ItemKind::CrimsonMushroom, 1),
            (419, ItemKind::CrimsonMushroom, 3),
            (413, ItemKind::ThornTwig, 2),
            (414, ItemKind::BloodBerry, 1),
            (409, ItemKind::BloodCrystal, 2),
            (421, ItemKind::BloodCrystal, 2),
        ] {
            let drops = blood_forest(material, [-1, -1, -1]).unwrap();
            assert_eq!(drops[0], Some(ItemStack { item, count }));
            assert!(drops.into_iter().flatten().all(|s| s.valid()));
        }
        for material in [0, 1, 7, 324, 400, 405, u16::MAX] {
            assert_eq!(blood_forest(material, [0; 3]), None);
        }
    }

    #[test]
    fn sap_has_stable_twenty_percent_chance_in_negative_regions() {
        let yields: Vec<_> = (-10_000..0)
            .map(|x| blood_forest(406, [x, -1, -31]).unwrap())
            .collect();
        let sap = yields.iter().filter(|drops| drops[1].is_some()).count();
        assert!((1800..2200).contains(&sap), "{sap}/10000 sap drops");
        for (x, expected) in (-10_000_i32..0).zip(&yields).rev() {
            assert_eq!(
                &blood_forest(406, [i64::from(x), -1, -31]).unwrap(),
                expected
            );
        }
    }
}
