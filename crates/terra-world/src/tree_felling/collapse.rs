//! Bounded cosmetic snapshot of an accepted, exactly attributed tree cut.
use super::{MAX_LEAVES, MAX_WOOD, Plan};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use terra_voxel::{SectionPos, global_to_section};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Collapse {
    pub foot: [i64; 3],
    pub direction: [f32; 2],
    pub wood_material: u16,
    pub leaves_material: u16,
    pub sections: Vec<CollapseSection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollapseSection {
    pub position: SectionPos,
    pub wood: Vec<u16>,
    pub leaves: Vec<u16>,
}

impl CollapseSection {
    pub fn cells(&self) -> impl Iterator<Item = (u16, bool)> + '_ {
        self.wood
            .iter()
            .copied()
            .map(|cell| (cell, true))
            .chain(self.leaves.iter().copied().map(|cell| (cell, false)))
    }
}

impl Collapse {
    pub(super) fn from_plan(plan: &Plan, direction: [f32; 2]) -> Self {
        let length = direction[0].hypot(direction[1]);
        let direction = if length.is_finite() && length > 0.001 {
            direction.map(|v| v / length)
        } else {
            [1.0, 0.0]
        };
        let mut sections = BTreeMap::<_, CollapseSection>::new();
        for (positions, wood) in [(&plan.wood, true), (&plan.leaves, false)] {
            for &position in positions {
                let (section, cell) = global_to_section(position);
                let entry = sections
                    .entry((section.x, section.y, section.z))
                    .or_insert_with(|| CollapseSection {
                        position: section,
                        wood: Vec::new(),
                        leaves: Vec::new(),
                    });
                let index = (cell[0] + 16 * (cell[1] + 16 * cell[2])) as u16;
                if wood {
                    &mut entry.wood
                } else {
                    &mut entry.leaves
                }
                .push(index);
            }
        }
        for section in sections.values_mut() {
            section.wood.sort_unstable();
            section.leaves.sort_unstable();
        }
        Self {
            foot: plan.foot,
            direction,
            wood_material: plan.material,
            leaves_material: plan.leaves_material,
            sections: sections.into_values().collect(),
        }
    }

    pub fn valid(&self) -> bool {
        let length = self.direction[0].hypot(self.direction[1]);
        if self.foot.iter().any(|v| v.unsigned_abs() > 999_000)
            || !length.is_finite()
            || (length - 1.0).abs() > 0.001
            || !matches!(self.wood_material, 6 | 10 | 36 | 134)
            || !matches!(self.leaves_material, 7..=9 | 132 | 133 | 195)
            || self.sections.is_empty()
            || self.sections.len() > 128
        {
            return false;
        }
        let base = global_to_section(self.foot).0;
        let mut sections = HashSet::new();
        let mut wood = 0;
        let mut leaves = 0;
        let mut includes_foot = false;
        for section in &self.sections {
            let p = section.position;
            if !sections.insert(p)
                || p.x.abs_diff(base.x) > 8
                || p.y.abs_diff(base.y) > 8
                || p.z.abs_diff(base.z) > 8
                || section.wood.len() + section.leaves.len() > 4096
            {
                return false;
            }
            let mut cells = [false; 4096];
            for (cell, is_wood) in section.cells() {
                if cell >= 4096 || std::mem::replace(&mut cells[usize::from(cell)], true) {
                    return false;
                }
                includes_foot |= is_wood && self.cell_position(p, cell) == Some(self.foot);
            }
            wood += section.wood.len();
            leaves += section.leaves.len();
        }
        includes_foot && (1..=MAX_WOOD).contains(&wood) && leaves <= MAX_LEAVES
    }

    pub fn cell_position(&self, section: SectionPos, index: u16) -> Option<[i64; 3]> {
        if index >= 4096 {
            return None;
        }
        let origin = section.origin().ok()?;
        let i = i64::from(index);
        Some([
            origin[0] + i % 16,
            origin[1] + i / 16 % 16,
            origin[2] + i / 256,
        ])
    }

    /// Cosmetic pivot is derived from owned wood, never the clicked upper branch.
    /// This keeps the existing compact network snapshot compatible.
    pub fn visual_origin(&self) -> [i64; 3] {
        let minimum = self
            .sections
            .iter()
            .flat_map(|section| {
                section
                    .wood
                    .iter()
                    .filter_map(|&index| self.cell_position(section.position, index))
            })
            .map(|cell| cell[1])
            .min()
            .unwrap_or(self.foot[1]);
        let mut sum = [0_i64; 2];
        let mut count = 0_i64;
        for cell in self
            .sections
            .iter()
            .flat_map(|section| {
                section
                    .wood
                    .iter()
                    .filter_map(|&index| self.cell_position(section.position, index))
            })
            .filter(|cell| cell[1] <= minimum + 1)
        {
            sum[0] += cell[0];
            sum[1] += cell[2];
            count += 1;
        }
        if count == 0 {
            return self.foot;
        }
        [sum[0].div_euclid(count), minimum, sum[1].div_euclid(count)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Collapse {
        Collapse {
            foot: [-1, 15, -17],
            direction: [0.0, -1.0],
            wood_material: 6,
            leaves_material: 7,
            sections: vec![CollapseSection {
                position: SectionPos { x: -1, y: 0, z: -2 },
                wood: vec![4095],
                leaves: vec![4094],
            }],
        }
    }

    #[test]
    fn packed_cells_preserve_negative_coordinates_and_reject_duplicates() {
        let mut event = sample();
        assert!(event.valid());
        event.sections[0].leaves.push(4095);
        assert!(!event.valid());
        event = sample();
        event.sections.push(event.sections[0].clone());
        assert!(!event.valid());
    }

    #[test]
    fn cosmetic_snapshot_has_bounded_finite_positions_and_a_real_trunk() {
        for invalid in [f32::NAN, f32::INFINITY, 0.0, 2.0] {
            let mut event = sample();
            event.direction = [invalid, 0.0];
            assert!(!event.valid());
        }
        let mut event = sample();
        event.sections[0].wood.clear();
        assert!(!event.valid());
        event = sample();
        event.sections[0].position.x += 9;
        assert!(!event.valid());
    }
}
