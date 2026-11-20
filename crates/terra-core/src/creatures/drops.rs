//! Small deterministic loot sets. Only the server may commit these after death.
use super::CreatureKind;
use crate::{ItemKind, ItemStack};

pub fn forest_loot(kind: CreatureKind, variant: u8, id: u64) -> [Option<ItemStack>; 2] {
    if kind == CreatureKind::SpiritGloom {
        let mut hash = id.wrapping_add(u64::from(variant) * 0x9e37_79b9);
        hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        hash ^= hash >> 27;
        return [
            hash.is_multiple_of(3).then_some(ItemStack {
                item: ItemKind::GloomEssence,
                count: 1,
            }),
            (hash
                % (if variant == super::gloom::LIGHT {
                    16
                } else {
                    32
                })
                == 1)
                .then_some(ItemStack {
                    item: ItemKind::LightSplitter,
                    count: 1,
                }),
        ];
    }
    let (item, count, extra) = match kind {
        CreatureKind::Skeleton => (ItemKind::Bone, 2 + u16::from(variant.min(2)), None),
        CreatureKind::Raven => (ItemKind::Bone, 1, None),
        CreatureKind::BloodSlime => (ItemKind::BloodSlimeGel, 2 + (id % 2) as u16, None),
        CreatureKind::BloodStalker => (ItemKind::BloodCrystal, 1, None),
        CreatureKind::BloodWisp => (ItemKind::BloodSpore, 2, None),
        CreatureKind::ThornBrute => (
            ItemKind::ThornTwig,
            3,
            Some(ItemStack {
                item: ItemKind::DarkSap,
                count: 1,
            }),
        ),
        _ => return [None, None],
    };
    [Some(ItemStack { item, count }), extra]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loot_is_stable_valid_bounded_and_existing_animals_are_unchanged() {
        for kind in CreatureKind::ALL {
            for variant in 0..kind.variant_count() {
                let loot = forest_loot(kind, variant, 42);
                assert_eq!(loot, forest_loot(kind, variant, 42));
                assert!(loot.iter().flatten().all(|s| s.valid()));
                if (kind as u8) < 24 && kind != CreatureKind::SpiritGloom {
                    assert_eq!(loot, [None, None]);
                }
            }
        }
        assert_eq!(
            forest_loot(CreatureKind::Skeleton, 2, 1)[0].unwrap().count,
            4
        );
        assert_eq!(
            forest_loot(CreatureKind::ThornBrute, 0, 1)
                .iter()
                .flatten()
                .count(),
            2
        );
    }
    #[test]
    fn gloom_drops_are_real_rare_resources_and_remain_stable_after_reload() {
        let mut essence = 0;
        let mut crystal = 0;
        for id in 1..=1024 {
            let loot = forest_loot(CreatureKind::SpiritGloom, 0, id);
            assert_eq!(loot, forest_loot(CreatureKind::SpiritGloom, 0, id));
            for stack in loot.into_iter().flatten() {
                assert!(stack.valid());
                match stack.item {
                    ItemKind::GloomEssence => essence += 1,
                    ItemKind::LightSplitter => crystal += 1,
                    _ => panic!("Gloom resource"),
                }
            }
        }
        assert!((240..440).contains(&essence));
        assert!((10..65).contains(&crystal));
    }
    #[test]
    fn forest_items_roundtrip_stable_identifiers_and_food_rules() {
        for item in [
            ItemKind::BloodBerry,
            ItemKind::CrimsonMushroom,
            ItemKind::ThornTwig,
            ItemKind::BloodCrystal,
            ItemKind::DarkSap,
            ItemKind::BloodLeaf,
            ItemKind::BloodSpore,
            ItemKind::BloodSlimeGel,
            ItemKind::Bone,
            ItemKind::GloomEssence,
            ItemKind::LightSplitter,
        ] {
            assert!(item.valid());
            assert!(ItemKind::EXTRA.contains(&item));
            assert_eq!(
                serde_json::from_str::<ItemKind>(&serde_json::to_string(&item).unwrap()).unwrap(),
                item
            );
            assert!(item.identifier().starts_with("teramic::"));
        }
        assert!(crate::food(ItemKind::BloodBerry).is_some());
        assert!(crate::food(ItemKind::CrimsonMushroom).is_some());
        assert!(crate::food(ItemKind::BloodSlimeGel).is_none());
    }
}
