//! Small deterministic loot sets. Only the server may commit these after death.
use super::CreatureKind;
use crate::{ItemKind, ItemStack};

pub fn forest_loot(kind: CreatureKind, variant: u8, id: u64) -> [Option<ItemStack>; 4] {
    if kind == CreatureKind::StoneGolem {
        let roll = id.wrapping_mul(0x9e3779b97f4a7c15).rotate_left(21);
        let stack = |item, count| Some(ItemStack { item, count });
        return [
            stack(
                ItemKind::Block(terra_voxel::MaterialId::Stone as u16),
                2 + (roll % 5) as u16,
            ),
            stack(
                ItemKind::Block(terra_voxel::MaterialId::PetrifiedLog as u16),
                1 + ((roll >> 8) % 3) as u16,
            ),
            (roll >> 21).is_multiple_of(13).then_some(ItemStack {
                item: ItemKind::RuneCore,
                count: 1,
            }),
            (roll >> 36).is_multiple_of(17).then_some(ItemStack {
                item: ItemKind::ForestCrystal,
                count: 1,
            }),
        ];
    }
    if kind.is_dragon() {
        use super::dragons::DragonLoot::*;
        let roll = (id ^ (id >> 33))
            .wrapping_mul(0x9e3779b97f4a7c15)
            .rotate_left(21);
        let (scale, secondary) = match kind {
            CreatureKind::DarkDragon => (
                DarkScale,
                match roll % 100 {
                    0 => ShadowHeart,
                    1..=5 => ShadowCore,
                    6..=15 => Blood,
                    16..=27 => ShadowCrystal,
                    28..=42 => Membrane,
                    43..=57 => Fang,
                    _ => Hide,
                },
            ),
            CreatureKind::FireDragon => (
                Hide,
                match roll % 5 {
                    0 => Horn,
                    1 => Membrane,
                    2 => Claw,
                    _ => Fang,
                },
            ),
            CreatureKind::EarthDragon => (
                EarthScale,
                match roll % 100 {
                    0..=7 => EarthCrystal,
                    8..=19 => Dust,
                    20..=39 => Fang,
                    40..=65 => Hide,
                    _ => RawMeat,
                },
            ),
            CreatureKind::WaterDragon => (
                WaterScale,
                match roll % 100 {
                    0..=2 => WaterHeart,
                    3..=10 => IceEssence,
                    11..=27 => Horn,
                    28..=44 => Claw,
                    45..=67 => Membrane,
                    _ => WaterEssence,
                },
            ),
            _ => unreachable!(),
        };
        return [
            Some(ItemStack {
                item: ItemKind::DragonLoot(scale),
                count: 3 + (roll % 4) as u16,
            }),
            Some(ItemStack {
                item: ItemKind::DragonLoot(secondary),
                count: 1,
            }),
            None,
            None,
        ];
    }
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
            None,
            None,
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
        _ => return [None; 4],
    };
    [Some(ItemStack { item, count }), extra, None, None]
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
                    assert_eq!(loot, [None; 4]);
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

#[cfg(test)]
#[test]
fn stone_guardian_loot_includes_two_guaranteed_stacks_and_independent_rare_crystals() {
    let mut counts = [0; 2];
    let mut together = false;
    for id in 1..=1024 {
        let [stone, wood, core, crystal] =
            forest_loot(CreatureKind::StoneGolem, (id % 2) as u8, id);
        let stone = stone.unwrap();
        let wood = wood.unwrap();
        assert_eq!(
            stone.item,
            ItemKind::Block(terra_voxel::MaterialId::Stone as u16)
        );
        assert!((2..=6).contains(&stone.count));
        assert!((1..=3).contains(&wood.count));
        counts[0] += usize::from(core.is_some());
        counts[1] += usize::from(crystal.is_some());
        together |= core.is_some() && crystal.is_some();
    }
    assert!(counts.iter().all(|n| (20..130).contains(n)));
    assert!(together);
}
