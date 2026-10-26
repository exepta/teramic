//! Stable textile material IDs; white wool keeps its original saved ID.
use crate::beds::DyeColor;

pub const WHITE_WOOL: u16 = 103;
pub const WHITE_CARPET: u16 = 106;
pub const LAST_CARPET: u16 = 118;
pub const FIRST_DYED_WOOL: u16 = 119;
pub const LAST_DYED_WOOL: u16 = 130;
const COLORS: [DyeColor; 13] = [
    DyeColor::White,
    DyeColor::Red,
    DyeColor::Yellow,
    DyeColor::Blue,
    DyeColor::Orange,
    DyeColor::Green,
    DyeColor::Purple,
    DyeColor::Black,
    DyeColor::Lime,
    DyeColor::Pink,
    DyeColor::Gold,
    DyeColor::Silver,
    DyeColor::LightBlue,
];

pub const fn is_carpet(material: u16) -> bool {
    material >= WHITE_CARPET && material <= LAST_CARPET
}
pub const fn is_wool(material: u16) -> bool {
    material == WHITE_WOOL || material >= FIRST_DYED_WOOL && material <= LAST_DYED_WOOL
}
pub const fn is_dyed(material: u16) -> bool {
    material > WHITE_CARPET && material <= LAST_DYED_WOOL
}
pub fn color(material: u16) -> Option<DyeColor> {
    if material == WHITE_WOOL {
        return Some(DyeColor::White);
    }
    let index = if is_carpet(material) {
        material - WHITE_CARPET
    } else if is_wool(material) {
        material - FIRST_DYED_WOOL + 1
    } else {
        return None;
    };
    COLORS.get(usize::from(index)).copied()
}
pub fn dyed(material: u16, color: DyeColor) -> Option<u16> {
    let index = COLORS.iter().position(|&candidate| candidate == color)? as u16;
    if is_carpet(material) {
        Some(WHITE_CARPET + index)
    } else if is_wool(material) {
        Some(if index == 0 {
            WHITE_WOOL
        } else {
            FIRST_DYED_WOOL + index - 1
        })
    } else {
        None
    }
}
pub fn base(material: u16) -> Option<u16> {
    if is_carpet(material) {
        Some(WHITE_CARPET)
    } else if is_wool(material) {
        Some(WHITE_WOOL)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_dye_roundtrips_without_changing_the_white_wool_or_existing_shape_ids() {
        let mut ids = std::collections::BTreeSet::new();
        for base in [WHITE_WOOL, WHITE_CARPET] {
            for dye in DyeColor::ALL {
                let id = dyed(base, dye).unwrap();
                assert!(ids.insert(id));
                assert_eq!(color(id), Some(dye));
                assert!(terra_voxel::MaterialId::from_raw(id).is_some());
                for next in DyeColor::ALL {
                    assert_eq!(dyed(id, next), dyed(base, next));
                }
            }
        }
        assert_eq!(dyed(103, DyeColor::White), Some(103));
        for id in [104, 105, 131] {
            assert_eq!(base(id), None);
        }
    }

    #[test]
    fn every_textile_color_has_a_real_recipe_and_recolors_existing_fabric_atomically() {
        use crate::{
            Inventory, ItemKind,
            crafting::{CraftDestination, CraftStation, RecipeBook},
        };
        let book = RecipeBook::builtins();
        assert!(book.error.is_none(), "{:?}", book.error);
        for fabric in [WHITE_WOOL, WHITE_CARPET] {
            for color in DyeColor::ALL {
                let output = dyed(fabric, color).unwrap();
                assert!(
                    book.iter()
                        .any(|recipe| recipe.output.item == ItemKind::Block(output))
                );
                let mut bag = Inventory::default();
                bag.insert(ItemKind::Block(dyed(fabric, DyeColor::Blue).unwrap()), 1);
                bag.insert(ItemKind::Dye(color), 1);
                assert!(book.craft_grid(
                    &mut bag,
                    &[Some(0), None, None, Some(1)],
                    2,
                    CraftStation::Hand,
                    1,
                    CraftDestination::Inventory
                ));
                assert_eq!(bag.count(ItemKind::Block(output)), 1);
                assert_eq!(bag.count(ItemKind::Dye(color)), 0);
                assert_eq!(bag.occupied_slots(), 1);
            }
        }
    }

    #[test]
    fn carpets_use_the_exact_eighth_block_collision_and_wool_stays_a_full_opaque_cube() {
        let catalog = crate::block_properties::BlockPropertiesCatalog::load(
            &crate::asset_directory().join("terra/blocks"),
        )
        .unwrap();
        for color in DyeColor::ALL {
            let carpet = dyed(WHITE_CARPET, color).unwrap();
            let wool = dyed(WHITE_WOOL, color).unwrap();
            assert_eq!(
                catalog.shape(carpet),
                crate::block_shapes::builtin_boxes(98)
            );
            assert!(catalog.shape(wool).is_empty());
            assert!(
                !terra_voxel::MaterialId::from_raw(carpet)
                    .unwrap()
                    .is_opaque()
            );
            assert!(terra_voxel::MaterialId::from_raw(wool).unwrap().is_opaque());
        }
    }
}
