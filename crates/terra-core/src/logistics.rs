//! Passive inventory ports. Existing IDs and low-facing legacy modes stay valid.
pub const HOPPER: u16 = 152;
pub const IMPORTER: u16 = 153;
pub const TUNNEL: u16 = HOPPER;
pub const FUNNEL: u16 = IMPORTER;
pub const FILTERED_FUNNEL: u16 = 157;
pub const BELT_MERGER: u16 = 158;
pub const FILTERED_TUNNEL: u16 = 159;
pub const BELT_DISTRIBUTOR: u16 = 160;
pub const ITEMS_PER_TRANSFER: u16 = 1;
pub const TRANSFER_SECONDS: f32 = 0.5;
pub const PORT_BUDGET: usize = 64;

pub const fn is_port(material: u16) -> bool {
    matches!(
        material,
        HOPPER | IMPORTER | FILTERED_FUNNEL | BELT_MERGER | FILTERED_TUNNEL | BELT_DISTRIBUTOR
    )
}

pub const fn is_funnel(material: u16) -> bool {
    matches!(material, FUNNEL | FILTERED_FUNNEL)
}

pub const fn supports_filter(material: u16) -> bool {
    matches!(material, FILTERED_FUNNEL | FILTERED_TUNNEL)
}

pub const fn is_tunnel(material: u16) -> bool {
    matches!(material, TUNNEL | FILTERED_TUNNEL)
}

pub const fn is_router(material: u16) -> bool {
    matches!(material, BELT_MERGER | BELT_DISTRIBUTOR)
}

/// Distributor outputs are left, forward, right relative to its single rear input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DistributorConfig {
    pub filters: [FilterConfig; 3],
}

impl DistributorConfig {
    pub fn valid(self) -> bool {
        self.filters.into_iter().all(FilterConfig::valid)
    }
}

pub fn distributor_output(facing: u8, output: u8) -> Option<[i64; 3]> {
    let offset = *[1, 0, 3].get(usize::from(output))?;
    Some(crate::kinetic::forward((facing % 4 + offset) % 4))
}

/// A ghost selection, never an item-owning inventory slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterConfig {
    pub item: Option<crate::ItemKind>,
    pub allowed: bool,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            item: None,
            allowed: true,
        }
    }
}

impl FilterConfig {
    pub fn valid(self) -> bool {
        self.item.is_none_or(crate::ItemKind::valid)
    }

    pub fn accepts(self, item: crate::ItemKind) -> bool {
        self.item
            .is_none_or(|selected| (selected == item) == self.allowed)
    }
}

pub const fn inventory(material: u16) -> bool {
    matches!(material, 29 | 59 | 99 | 156 | 161 | 162)
}

pub const fn extracts(material: u16, facing: u8) -> bool {
    matches!(material, HOPPER | FILTERED_TUNNEL) == (facing < 4)
}

pub fn flow(facing: u8) -> [i64; 3] {
    let forward = crate::kinetic::forward(facing % 4);
    if facing < 4 {
        forward
    } else {
        forward.map(|v| -v)
    }
}

pub const fn default_facing(material: u16, orientation: u8, target: u16) -> u8 {
    orientation % 4
        + if extracts(material, 0) == (target != 29) {
            0
        } else {
            4
        }
}

pub const fn source_output(material: u16) -> Option<usize> {
    match material {
        59 => Some(crate::furnace::OUTPUT),
        crate::crusher::MATERIAL => Some(crate::crusher::OUTPUT),
        crate::mixer::MATERIAL => Some(crate::mixer::OUTPUT),
        crate::brewing::MATERIAL => Some(crate::brewing::OUTPUT),
        crate::campfire::MATERIAL => Some(crate::campfire::OUTPUT),
        _ => None,
    }
}

pub const fn input_slots(material: u16) -> usize {
    match material {
        59 => 2,
        crate::crusher::MATERIAL => 1,
        crate::mixer::MATERIAL => crate::mixer::INPUT_SLOTS,
        crate::brewing::MATERIAL => crate::brewing::OUTPUT,
        crate::campfire::MATERIAL => crate::campfire::OUTPUT,
        _ => 0,
    }
}

pub fn input_capacity(material: u16, slot: usize, item: crate::ItemKind) -> u16 {
    match material {
        59 => crate::furnace::slot_capacity(slot, item),
        crate::crusher::MATERIAL => crate::crusher::slot_capacity(slot, item),
        crate::mixer::MATERIAL => crate::mixer::slot_capacity(slot, item),
        crate::brewing::MATERIAL => crate::brewing::slot_capacity(slot, item),
        crate::campfire::MATERIAL => crate::campfire::slot_capacity(slot, item),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_filtered_port_variants_support_ghost_filters() {
        for material in [TUNNEL, FUNNEL, BELT_MERGER, BELT_DISTRIBUTOR] {
            assert!(!supports_filter(material));
        }
        for material in [FILTERED_FUNNEL, FILTERED_TUNNEL] {
            assert!(supports_filter(material));
        }
    }

    #[test]
    fn distributor_labels_follow_player_left_and_right_at_every_yaw() {
        for (facing, forward) in [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]]
            .into_iter()
            .enumerate()
        {
            let right = [-forward[2], 0, forward[0]];
            assert_eq!(distributor_output(facing as u8, 0), Some(right.map(|v| -v)));
            assert_eq!(distributor_output(facing as u8, 1), Some(forward));
            assert_eq!(distributor_output(facing as u8, 2), Some(right));
        }
        assert_eq!(distributor_output(0, 3), None);
    }

    #[test]
    fn mode_bit_reverses_arrow_and_preserves_legacy_modes_and_orientation() {
        for material in [HOPPER, IMPORTER, FILTERED_FUNNEL] {
            for facing in 0..4 {
                assert_eq!(extracts(material, facing), material == HOPPER);
                assert_ne!(extracts(material, facing), extracts(material, facing + 4));
                assert_eq!(flow(facing + 4), flow(facing).map(|v| -v));
                for target in [29, 59, 99] {
                    let raw = default_facing(material, facing, target);
                    assert_eq!(raw % 4, facing);
                    assert_eq!(extracts(material, raw), target != 29);
                }
            }
        }
    }

    #[test]
    fn filters_match_exact_identity_without_owning_or_conflating_rgb_items() {
        use crate::{ItemKind, beds::DyeColor};
        let red = ItemKind::Dye(DyeColor::Rgb([255, 0, 0]));
        let blue = ItemKind::Dye(DyeColor::Rgb([0, 0, 255]));
        for item in [red, blue, ItemKind::Block(1)] {
            assert!(FilterConfig::default().accepts(item));
            assert!(
                FilterConfig {
                    item: None,
                    allowed: false
                }
                .accepts(item)
            );
        }
        assert!(
            FilterConfig {
                item: Some(red),
                allowed: true
            }
            .accepts(red)
        );
        assert!(
            !FilterConfig {
                item: Some(red),
                allowed: true
            }
            .accepts(blue)
        );
        assert!(
            !FilterConfig {
                item: Some(red),
                allowed: false
            }
            .accepts(red)
        );
        assert!(
            FilterConfig {
                item: Some(red),
                allowed: false
            }
            .accepts(blue)
        );
        assert!(
            !FilterConfig {
                item: Some(ItemKind::Block(u16::MAX)),
                allowed: true
            }
            .valid()
        );
    }
}
