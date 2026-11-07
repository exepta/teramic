//! Passive conveyor filling. Conversion is explicit; unsupported liquids never become water.
use crate::{ItemKind, ToolKind};

pub const HOLD_BLOCK: u16 = 170;
pub const FLUID_INFUSER: u16 = 171;
/// A complete air cell separates the platform from the suspended device.
pub const INFUSER_OFFSET: i64 = 2;
pub const GAP_OFFSET: i64 = 1;
pub const fn infuser_above(hold: [i64; 3]) -> [i64; 3] {
    [hold[0], hold[1] + INFUSER_OFFSET, hold[2]]
}
pub const fn hold_below(infuser: [i64; 3]) -> [i64; 3] {
    [infuser[0], infuser[1] - INFUSER_OFFSET, infuser[2]]
}
pub const fn gap_above(hold: [i64; 3]) -> [i64; 3] {
    [hold[0], hold[1] + GAP_OFFSET, hold[2]]
}
pub const HOLD_TOP: f64 = 0.625;
pub const DROP_HEIGHT: f64 = HOLD_TOP + 0.13;
pub const FILL_SECONDS: f32 = 0.6;
pub const MAX_HOLD_VISUALS: usize = 128;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HoldPhase {
    #[default]
    Idle,
    Approaching,
    WaitingForFluid,
    Filling,
    Releasing,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HoldVisual {
    pub position: [i64; 3],
    pub facing: u8,
    /// Bit n corresponds to kinetic::forward(n), independent of the saved model yaw.
    pub connections: u8,
    pub active_drop: Option<u64>,
    pub phase: HoldPhase,
    /// Fill progress in thousandths, excluding unconsumed liquid from the stored item.
    pub progress: u16,
    pub gates: bool,
    /// Actual inherited deck motion in blocks per second along facing.
    pub speed: f32,
}

impl HoldVisual {
    pub fn valid(self) -> bool {
        self.position.iter().all(|v| v.unsigned_abs() <= 1_000_000)
            && self.facing < 4
            && self.connections < 16
            && self.progress <= 1000
            && self.active_drop.is_none_or(|id| id > 0)
            && (self.phase == HoldPhase::Idle) == self.active_drop.is_none()
            && self.speed.is_finite()
            && (0.0..=f32::from(crate::kinetic::MAX_RPM) / 30.0).contains(&self.speed)
            && (!self.gates
                || matches!(
                    self.phase,
                    HoldPhase::Approaching | HoldPhase::WaitingForFluid | HoldPhase::Filling
                ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Liquid {
    Water,
}

pub fn fillable(item: ItemKind) -> bool {
    match item {
        ItemKind::GlassBottle | ItemKind::Tool(ToolKind::Bucket) => true,
        _ => item
            .tool_kind()
            .is_some_and(|tool| tool.watering_capacity() > item.watering_units()),
    }
}

/// At most eight finite network units are consumed in one atomic update.
pub fn fill(item: ItemKind, liquid: Liquid, available: u8) -> Option<(ItemKind, u8)> {
    if !item.valid() || available == 0 || available > 8 {
        return None;
    }
    match (liquid, item) {
        (Liquid::Water, ItemKind::GlassBottle) => Some((ItemKind::WaterBottle, 1)),
        (Liquid::Water, ItemKind::Tool(ToolKind::Bucket)) if available >= 8 => {
            Some((ItemKind::Tool(ToolKind::WaterBucket), 8))
        }
        (Liquid::Water, _) => {
            let tool = item.tool_kind()?;
            let units = tool
                .watering_capacity()
                .saturating_sub(item.watering_units())
                .min(available);
            (units > 0).then(|| {
                (
                    item.with_watering_units(item.watering_units() + units)
                        .unwrap(),
                    units,
                )
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hold_visual_has_validated_authoritative_single_item_phase_and_roundtrips() {
        let hold = [-17, -1, 16];
        assert_eq!(infuser_above(hold), [-17, 1, 16]);
        assert_eq!(gap_above(hold), [-17, 0, 16]);
        assert_eq!(hold_below(infuser_above(hold)), hold);
        assert!(
            HoldVisual {
                speed: 1.,
                ..Default::default()
            }
            .valid()
        );
        let view = HoldVisual {
            position: [-17, 4, -1],
            facing: 3,
            connections: 10,
            active_drop: Some(7),
            phase: HoldPhase::Filling,
            progress: 500,
            gates: true,
            speed: 0.,
        };
        assert!(view.valid());
        assert_eq!(
            serde_json::from_str::<HoldVisual>(&serde_json::to_string(&view).unwrap()).unwrap(),
            view
        );
        for invalid in [
            HoldVisual {
                active_drop: None,
                ..view
            },
            HoldVisual {
                phase: HoldPhase::Idle,
                ..view
            },
            HoldVisual { facing: 4, ..view },
            HoldVisual {
                connections: 16,
                ..view
            },
            HoldVisual {
                progress: 1001,
                ..view
            },
            HoldVisual {
                speed: f32::NAN,
                ..view
            },
            HoldVisual { speed: -1., ..view },
            HoldVisual {
                active_drop: Some(0),
                ..view
            },
            HoldVisual {
                phase: HoldPhase::Releasing,
                ..view
            },
        ] {
            assert!(!invalid.valid());
        }
    }
    #[test]
    fn water_fills_exact_vessels_and_tops_up_the_same_can_without_refilling_full_items() {
        assert_eq!(
            fill(ItemKind::GlassBottle, Liquid::Water, 8),
            Some((ItemKind::WaterBottle, 1))
        );
        assert_eq!(
            fill(ItemKind::Tool(ToolKind::Bucket), Liquid::Water, 7),
            None
        );
        assert_eq!(
            fill(ItemKind::Tool(ToolKind::Bucket), Liquid::Water, 8),
            Some((ItemKind::Tool(ToolKind::WaterBucket), 8))
        );
        for tool in [
            ToolKind::WoodWateringCan,
            ToolKind::IronWateringCan,
            ToolKind::GoldWateringCan,
            ToolKind::DiamondWateringCan,
            ToolKind::RubyWateringCan,
        ] {
            let empty = ItemKind::Tool(tool);
            let partial = empty
                .with_watering_units(tool.watering_capacity() - 2)
                .unwrap();
            let full = empty.with_watering_units(tool.watering_capacity()).unwrap();
            assert!(fillable(partial));
            assert_eq!(fill(partial, Liquid::Water, 8), Some((full, 2)));
            assert!(!fillable(full));
            assert_eq!(full.preview_item(), empty);
            assert_eq!(full.max_stack(), 1);
            assert_eq!(full.with_watering_units(0), Some(empty));
            assert_eq!(
                serde_json::from_str::<ItemKind>(&serde_json::to_string(&full).unwrap()).unwrap(),
                full
            );
        }
        for item in [
            ItemKind::Block(3),
            ItemKind::WaterBottle,
            ItemKind::Tool(ToolKind::WaterBucket),
            ItemKind::Tool(ToolKind::IronHoe),
        ] {
            assert!(!fillable(item));
            assert_eq!(fill(item, Liquid::Water, 8), None);
        }
        assert!(
            !ItemKind::FilledWateringCan {
                tool: ToolKind::Bucket,
                units: 1
            }
            .valid()
        );
        assert!(
            !ItemKind::FilledWateringCan {
                tool: ToolKind::RubyWateringCan,
                units: 25
            }
            .valid()
        );
    }

    #[test]
    fn legacy_empty_can_stacks_remain_loadable_and_transform_one_preserves_every_can() {
        use crate::{INVENTORY_SLOTS, Inventory, ItemStack, STACK_LIMIT};
        let empty = ItemKind::Tool(ToolKind::IronWateringCan);
        let full = empty.with_watering_units(12).unwrap();
        assert_eq!(empty.max_stack(), STACK_LIMIT);
        let mut bag = Inventory::default();
        assert!(bag.creative_set(
            0,
            ItemStack {
                item: empty,
                count: STACK_LIMIT
            }
        ));
        let loaded: Inventory =
            serde_json::from_str(&serde_json::to_string(&bag).unwrap()).unwrap();
        assert!(loaded.valid());
        let revision = bag.revision;
        assert!(bag.transform_one(0, full));
        assert_eq!(bag.slot(0).unwrap().item, full);
        assert_eq!(bag.count(empty), STACK_LIMIT - 1);
        assert_eq!(bag.count(full), 1);
        assert_eq!(bag.revision, revision + 1);
        let mut packed = Inventory {
            slots: vec![
                ItemStack {
                    item: ItemKind::Block(3),
                    count: STACK_LIMIT
                };
                INVENTORY_SLOTS
            ],
            ..Default::default()
        };
        packed.slots[0] = ItemStack {
            item: empty,
            count: STACK_LIMIT,
        };
        let before = packed.clone();
        assert!(!packed.transform_one(0, full));
        assert_eq!(packed, before);
        packed.slots[1] = ItemStack {
            item: empty,
            count: 1,
        };
        assert!(packed.transform_one(0, full));
        assert_eq!(packed.count(empty), STACK_LIMIT);
        assert_eq!(packed.count(full), 1);
        assert!(packed.valid());
    }
}
