//! Bounded pot contents are carried by the existing public object-state byte.
use crate::{ItemKind, ResourcePropKind, beds::DyeColor};
pub const MATERIAL: u16 = 131;
pub const STATES: u8 = 11;
pub fn contents(state: u8) -> Option<ItemKind> {
    match state {
        1 => Some(ItemKind::Prop(ResourcePropKind::Flower)),
        2..=8 => Some(ItemKind::Flower(
            DyeColor::FLOWER_COLORS[usize::from(state - 2)],
        )),
        9 => Some(ItemKind::Prop(ResourcePropKind::Mushroom)),
        10 => Some(ItemKind::Flower(DyeColor::Pink)),
        _ => None,
    }
}
pub fn state(item: ItemKind) -> Option<u8> {
    match item {
        ItemKind::Prop(ResourcePropKind::Flower) => Some(1),
        ItemKind::Flower(DyeColor::Pink) => Some(10),
        ItemKind::Flower(color) => DyeColor::FLOWER_COLORS
            .iter()
            .position(|&c| c == color)
            .map(|n| n as u8 + 2),
        ItemKind::Prop(ResourcePropKind::Mushroom) => Some(9),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pot_contents_preserve_each_flower_identity_and_mushroom_without_new_item_or_prop_ids() {
        assert_eq!(contents(0), None);
        assert_eq!(
            contents(9),
            Some(ItemKind::Prop(ResourcePropKind::Mushroom))
        );
        assert_eq!(state(ItemKind::Flower(DyeColor::Pink)), Some(10));
        for value in 1..STATES {
            let item = contents(value).unwrap();
            assert_eq!(state(item), Some(value));
            assert!(item.valid());
        }
        for item in [
            ItemKind::Block(1),
            ItemKind::Dye(DyeColor::Red),
            ItemKind::Prop(ResourcePropKind::Grass),
        ] {
            assert_eq!(state(item), None);
        }
        assert_eq!(contents(STATES), None);
    }
}
