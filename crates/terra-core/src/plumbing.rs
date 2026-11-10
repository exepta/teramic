//! Geometry and capacity contracts for the existing finite-water network.
use serde::{Deserialize, Serialize};

pub type Cell = [i64; 3];
pub const WOOD_PIPE: u16 = 163;
pub const STONE_PIPE: u16 = 164;
pub const IRON_PIPE: u16 = 165;
pub const GLASS_PIPE: u16 = 172;
pub const VALVE: u16 = 166;
pub const FAUCET: u16 = 167;
pub const PUMP: u16 = 168;
pub const BASIN: u16 = 169;
pub const FLUID_INFUSER: u16 = crate::infusion::FLUID_INFUSER;
/// The assembly block is also a finite-water service connection.
pub const TRAIN_DOCK: u16 = crate::train_parts::STATION;
pub const CELL_CAPACITY: u8 = 8;
pub const MAX_COMPONENT: usize = 512;
pub const MAX_BASIN: usize = 250;
pub const DIRECTIONS: [Cell; 6] = [
    [-1, 0, 0],
    [1, 0, 0],
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
];

/// Confirmed external water entering this node through one physical open mouth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntakeActivity {
    pub source: Cell,
    pub direction: [i8; 3],
    pub units: u8,
}
impl IntakeActivity {
    pub fn valid_for(self, inlet: Cell) -> bool {
        inlet.iter().all(|value| value.unsigned_abs() <= 1_000_000)
            && (1..=CELL_CAPACITY).contains(&self.units)
            && self
                .source
                .iter()
                .all(|value| value.unsigned_abs() <= 1_000_000)
            && DIRECTIONS.contains(&self.direction.map(i64::from))
            && (0..3).all(|axis| self.source[axis] == inlet[axis] + i64::from(self.direction[axis]))
    }
}

pub const fn is_part(material: u16) -> bool {
    matches!(
        material,
        WOOD_PIPE..=BASIN | FLUID_INFUSER | GLASS_PIPE | TRAIN_DOCK
    )
}
pub const fn is_pipe(material: u16) -> bool {
    matches!(material, WOOD_PIPE..=IRON_PIPE | GLASS_PIPE)
}
pub const fn orientation(material: u16, facing: u8) -> u8 {
    if material == VALVE {
        facing & 7
    } else if material == FAUCET {
        facing & 3
    } else {
        facing
    }
}
pub const fn valid_facing(material: u16, facing: u8) -> bool {
    match material {
        WOOD_PIPE..=IRON_PIPE | GLASS_PIPE => facing < 6,
        VALVE => facing & !15 == 0 && facing & 7 < 6,
        FAUCET => matches!(facing, 0..=11 | 16..=19),
        PUMP => facing < 4,
        BASIN => facing < 64,
        FLUID_INFUSER | TRAIN_DOCK => facing < 4,
        _ => false,
    }
}
pub const fn open(material: u16, facing: u8) -> bool {
    match material {
        VALVE => facing & 8 == 0,
        FAUCET => faucet_level(facing) > 0,
        _ => true,
    }
}
pub const fn toggled(material: u16, facing: u8) -> Option<u8> {
    match material {
        VALVE => Some(facing ^ 8),
        FAUCET => Some(faucet_facing(
            facing,
            if faucet_level(facing) == 0 { 1 } else { 0 },
        )),
        _ => None,
    }
}
/// Legacy open facings remain level one; bit 2 still means fully closed.
pub const fn faucet_level(facing: u8) -> u8 {
    match facing & !3 {
        0 => 1,
        8 => 2,
        16 => 3,
        _ => 0,
    }
}
pub const fn faucet_facing(facing: u8, level: u8) -> u8 {
    (facing & 3)
        | match level {
            0 => 4,
            1 => 0,
            2 => 8,
            _ => 16,
        }
}
pub const fn adjusted_faucet(facing: u8, closing: bool) -> u8 {
    let level = faucet_level(facing);
    faucet_facing(
        facing,
        if closing {
            level.saturating_sub(1)
        } else if level < 3 {
            level + 1
        } else {
            3
        },
    )
}
pub const fn faucet_units(facing: u8) -> u8 {
    match faucet_level(facing) {
        1 => 1,
        2 => 2,
        3 => 4,
        _ => 0,
    }
}

pub fn direction_index(direction: Cell) -> Option<usize> {
    DIRECTIONS.iter().position(|value| *value == direction)
}
pub fn ports(material: u16, facing: u8) -> u8 {
    if !valid_facing(material, facing) {
        return 0;
    }
    if is_pipe(material) || matches!(material, BASIN | TRAIN_DOCK) {
        return 63;
    }
    // A downward nozzle consumes supply. Its lower side is never a pipe outlet.
    if material == FLUID_INFUSER {
        return 63 & !(1 << 2);
    }
    let axis = if material == VALVE {
        crate::kinetic::axis(orientation(material, facing))
    } else {
        crate::kinetic::forward(orientation(material, facing))
    };
    let positive = direction_index(axis).unwrap();
    if material == FAUCET {
        1 << (positive ^ 1)
    } else {
        (1 << positive) | (1 << (positive ^ 1))
    }
}
pub fn connects(material: u16, facing: u8, direction: Cell) -> bool {
    direction_index(direction).is_some_and(|index| ports(material, facing) & (1 << index) != 0)
}
pub fn joined(material: u16, facing: u8, other: u16, other_facing: u8, direction: Cell) -> bool {
    connects(material, facing, direction)
        && connects(other, other_facing, direction.map(|value| -value))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BasinShape {
    pub origin: Cell,
    pub size: [u8; 3],
}
impl BasinShape {
    pub fn valid(self) -> bool {
        (1..=5).contains(&self.size[0])
            && (1..=10).contains(&self.size[1])
            && (1..=5).contains(&self.size[2])
            && self
                .origin
                .iter()
                .all(|value| value.unsigned_abs() <= 990_000)
    }
    pub fn capacity(self) -> u16 {
        if !self.valid() {
            return 0;
        }
        self.size.into_iter().map(u16::from).product::<u16>() * u16::from(CELL_CAPACITY)
    }
    pub fn contains(self, position: Cell) -> bool {
        (0..3).all(|axis| {
            position[axis] >= self.origin[axis]
                && position[axis] < self.origin[axis] + i64::from(self.size[axis])
        })
    }
    /// A common liquid level across each layer, without filling one column first.
    pub fn levelled_cells(self, amount: u16) -> impl Iterator<Item = (Cell, u8)> {
        let valid = self.valid();
        let area = if valid {
            u16::from(self.size[0]) * u16::from(self.size[2])
        } else {
            1
        };
        self.cells()
            .take(if valid { MAX_BASIN } else { 0 })
            .map(move |cell| {
                let layer = (cell[1] - self.origin[1]) as u16;
                let units = amount.saturating_sub(layer * area * 8).min(area * 8);
                let index = (cell[2] - self.origin[2]) as u16 * u16::from(self.size[0])
                    + (cell[0] - self.origin[0]) as u16;
                (cell, (units / area + u16::from(index < units % area)) as u8)
            })
    }
    pub fn cells(self) -> impl Iterator<Item = Cell> {
        (0..self.size[1]).flat_map(move |y| {
            (0..self.size[2]).flat_map(move |z| {
                (0..self.size[0]).map(move |x| {
                    [
                        self.origin[0] + i64::from(x),
                        self.origin[1] + i64::from(y),
                        self.origin[2] + i64::from(z),
                    ]
                })
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn glass_pipe_appends_a_stable_id_and_joins_every_existing_hydraulic_material() {
        assert_eq!(GLASS_PIPE, 172);
        assert_eq!(terra_voxel::MaterialId::GlassPipe as u16, GLASS_PIPE);
        assert!(is_part(GLASS_PIPE) && is_pipe(GLASS_PIPE));
        for facing in 0..6 {
            assert!(valid_facing(GLASS_PIPE, facing));
            assert_eq!(ports(GLASS_PIPE, facing), 63);
            for other in [WOOD_PIPE, STONE_PIPE, IRON_PIPE, GLASS_PIPE] {
                for direction in DIRECTIONS {
                    assert!(joined(GLASS_PIPE, facing, other, facing, direction));
                }
            }
        }
        assert!(!valid_facing(GLASS_PIPE, 6));
        assert_eq!(ports(GLASS_PIPE, 6), 0);
        for other in [VALVE, FAUCET, PUMP, BASIN, FLUID_INFUSER] {
            for facing in 0..4 {
                for direction in DIRECTIONS {
                    assert_eq!(
                        joined(GLASS_PIPE, 0, other, facing, direction),
                        connects(other, facing, direction.map(|v| -v)),
                    );
                }
            }
        }
    }

    #[test]
    fn faucet_steps_preserve_legacy_states_orientation_and_saturate_at_both_ends() {
        for direction in 0..4 {
            assert_eq!(faucet_level(direction), 1);
            assert_eq!(faucet_level(direction | 4), 0);
            let mut facing = direction | 4;
            for level in [1, 2, 3, 3] {
                facing = adjusted_faucet(facing, false);
                assert_eq!(faucet_level(facing), level);
                assert_eq!(orientation(FAUCET, facing), direction);
                assert!(valid_facing(FAUCET, facing));
                assert!(open(FAUCET, facing));
            }
            for level in [2, 1, 0, 0] {
                facing = adjusted_faucet(facing, true);
                assert_eq!(faucet_level(facing), level);
                assert_eq!(orientation(FAUCET, facing), direction);
            }
        }
        assert!(faucet_units(faucet_facing(0, 1)) < faucet_units(faucet_facing(0, 2)));
        assert!(faucet_units(faucet_facing(0, 2)) < faucet_units(faucet_facing(0, 3)));
    }

    #[test]
    fn valves_keep_physical_ports_but_cut_supply_and_faucets_have_only_a_back_inlet() {
        for facing in 0..6 {
            assert_eq!(ports(VALVE, facing), ports(VALVE, facing | 8));
            assert!(open(VALVE, facing));
            assert!(!open(VALVE, facing | 8));
            assert_eq!(ports(VALVE, facing).count_ones(), 2);
        }
        for facing in 0..4 {
            assert!(connects(
                FAUCET,
                facing,
                crate::kinetic::forward(facing).map(|value| -value)
            ));
            assert!(!connects(FAUCET, facing, crate::kinetic::forward(facing)));
        }
        let largest = BasinShape {
            origin: [-17, -1, 15],
            size: [5, 10, 5],
        };
        assert!(largest.valid());
        assert_eq!(largest.capacity(), 2000);
        assert_eq!(largest.cells().count(), MAX_BASIN);
    }
}
