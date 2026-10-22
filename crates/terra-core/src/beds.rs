//! Bed fabrics and flower pigments shared by inventory, world state and rendering.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DyeColor {
    Red,
    Yellow,
    Blue,
    Orange,
    Green,
    Purple,
    White,
}
impl DyeColor {
    pub const ALL: [Self; 6] = [
        Self::Red,
        Self::Yellow,
        Self::Blue,
        Self::Orange,
        Self::Green,
        Self::Purple,
    ];
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Yellow => "yellow",
            Self::Blue => "blue",
            Self::Orange => "orange",
            Self::Green => "green",
            Self::Purple => "purple",
            Self::White => "white",
        }
    }
    pub const fn tint(self) -> [f32; 4] {
        match self {
            Self::Red => [0.78, 0.065, 0.075, 1.0],
            Self::Yellow => [0.95, 0.73, 0.075, 1.0],
            Self::Blue => [0.065, 0.20, 0.79, 1.0],
            Self::Orange => [0.95, 0.30, 0.055, 1.0],
            Self::Green => [0.28, 0.59, 0.075, 1.0],
            Self::Purple => [0.48, 0.10, 0.66, 1.0],
            Self::White => [0.95, 0.94, 0.90, 1.0],
        }
    }
}
pub const fn default_bed_color(_material: u16) -> DyeColor {
    DyeColor::White
}
pub const fn is_bed(material: u16) -> bool {
    matches!(material, 71..=73)
}
pub fn supports_bed_floor(material: u16) -> bool {
    if !terra_voxel::MaterialId::from_raw(material).is_some_and(|id| {
        id.is_solid()
            && (id.is_opaque()
                || matches!(
                    id,
                    terra_voxel::MaterialId::Glass
                        | terra_voxel::MaterialId::Ice
                        | terra_voxel::MaterialId::SolidWater
                ))
    }) {
        return false;
    }
    let boxes = crate::block_shapes::builtin_boxes(material);
    boxes.is_empty()
        || boxes.iter().any(|shape| {
            shape.minimum[0] <= 0.0
                && shape.maximum[0] >= 1.0
                && shape.minimum[2] <= 0.0
                && shape.maximum[2] >= 1.0
                && (shape.maximum[1] - 1.0).abs() < 0.000001
        })
}
pub fn bed_color(material: u16, facing: u8) -> DyeColor {
    facing
        .checked_sub(4)
        .and_then(|value| DyeColor::ALL.get(usize::from(value / 4)))
        .copied()
        .unwrap_or(default_bed_color(material))
}
pub fn dyed_facing(facing: u8, color: DyeColor) -> u8 {
    if color == DyeColor::White {
        return facing % 4;
    }
    4 + 4 * DyeColor::ALL
        .iter()
        .position(|value| *value == color)
        .unwrap_or(0) as u8
        + facing % 4
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn beds_need_a_full_height_support_surface_not_a_low_or_hollow_model() {
        for material in [1, 2, 3, 4, 14, 15, 16, 20, 49] {
            assert!(supports_bed_floor(material));
        }
        for material in [0, 7, 27, 28, 29, 40, 51, 59, 65, 71, 74, 82, 90, 97] {
            assert!(
                !supports_bed_floor(material),
                "unsupported floor {material}"
            );
        }
    }
    #[test]
    fn dye_state_preserves_each_orientation_and_round_trips_through_inventory() {
        for material in 71..=73 {
            for facing in 0..4 {
                for color in DyeColor::ALL {
                    let dyed = dyed_facing(facing, color);
                    assert_eq!(dyed % 4, facing);
                    assert_eq!(bed_color(material, dyed), color);
                    let item = crate::ItemKind::DyedBed { material, color };
                    assert!(item.valid());
                    assert_eq!(item.block_material(), Some(material));
                    let json = serde_json::to_string(&item).unwrap();
                    assert_eq!(
                        serde_json::from_str::<crate::ItemKind>(&json).unwrap(),
                        item
                    );
                }
            }
        }
        assert!(
            !crate::ItemKind::DyedBed {
                material: 16,
                color: DyeColor::Red
            }
            .valid()
        );
    }
}
