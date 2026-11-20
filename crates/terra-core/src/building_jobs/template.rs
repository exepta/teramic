//! Editable authored house data. Persisted jobs keep their own resolved steps.
mod catalog;
mod placement;
pub use catalog::*;
pub use placement::{cell_inputs, rotated_facing};

use super::*;
use crate::{ItemKind, ItemStack};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};

pub const TEMPLATE_SCHEMA: u32 = 1;
pub const DEFAULT_TEMPLATE_DIRECTORY: &str = "buildings";
pub const DEFAULT_HOUSE_PATH: &str = "buildings/templates/village/residential.json";
pub const MAX_TEMPLATE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateCell {
    pub position: [i64; 3],
    pub material: u16,
    pub facing: u8,
    /// Rotate the actual orientation, including all stair/slab states, with the house.
    /// False preserves fixed state bytes such as pot contents and lantern mounts.
    pub rotate_facing: bool,
    pub peer: Option<[i64; 3]>,
    pub secondary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateStep {
    pub phase: BuildPhase,
    pub cells: Vec<TemplateCell>,
    /// The full input for this atomic step; paired beds/doors cost one item.
    pub costs: Vec<ItemStack>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingTemplate {
    pub schema_version: u32,
    pub id: String,
    #[serde(default = "catalog::default_display_name")]
    pub display_name: String,
    #[serde(default)]
    pub category: TemplateCategory,
    /// Local foot position selected by the player's ground click.
    pub anchor: [i64; 3],
    pub bounds: BuildArea,
    pub steps: Vec<TemplateStep>,
    /// Exact aggregate input, without runtime site-specific scaffolding.
    pub materials: Vec<ItemStack>,
}

pub fn material_totals(steps: &[TemplateStep]) -> Result<Vec<ItemStack>, String> {
    let mut totals = BTreeMap::<ItemKind, u32>::new();
    for stack in steps.iter().flat_map(|step| &step.costs) {
        let count = totals.entry(stack.item).or_default();
        *count = count
            .checked_add(u32::from(stack.count))
            .ok_or("Building material total overflows")?;
    }
    totals
        .into_iter()
        .map(|(item, count)| {
            Ok(ItemStack {
                item,
                count: u16::try_from(count).map_err(|_| "Building material total exceeds 65535")?,
            })
        })
        .collect()
}

fn normalized(point: [i64; 3], anchor: [i64; 3]) -> Option<[i64; 3]> {
    Some([
        point[0].checked_sub(anchor[0])?,
        point[1].checked_sub(anchor[1])?,
        point[2].checked_sub(anchor[2])?,
    ])
}

fn house_cell(point: [i64; 3]) -> bool {
    // The current site planner reserves the surrounding ramp and working ring.
    (-4..=4).contains(&point[0]) && (-2..=9).contains(&point[1]) && (-6..=4).contains(&point[2])
}

/// The current builder assembles its paid interior stair/deck before walls and
/// removes it before furnishing. Permanent cells there must wait for that phase.
fn reserved_work_cell([x, y, z]: [i64; 3]) -> bool {
    x == -2 && (0..=3).contains(&y) && ((-2 + y)..=1).contains(&z)
        || y == 3 && (-2..=2).contains(&x) && (-2..=2).contains(&z) && !(x == -2 && z <= 0)
}

fn valid_pose(cell: &TemplateCell) -> bool {
    if cell.rotate_facing && cell.facing >= 4 && !crate::construction::is_shape(cell.material) {
        return false;
    }
    if cell.material == crate::flower_pot::MATERIAL {
        !cell.rotate_facing && cell.facing < crate::flower_pot::STATES
    } else if crate::lantern::is_part(cell.material) {
        !cell.rotate_facing && crate::lantern::valid_facing(cell.material, cell.facing)
    } else if crate::construction::is_shape(cell.material) {
        crate::construction::valid_facing(cell.material, cell.facing)
    } else {
        cell.facing < 4
    }
}

impl BuildingTemplate {
    pub fn from_json(source: &str) -> Result<Self, String> {
        if source.len() > MAX_TEMPLATE_BYTES {
            return Err("Building template exceeds 2 MiB".into());
        }
        let template: Self = serde_json::from_str(source).map_err(|error| error.to_string())?;
        template.validate()?;
        Ok(template)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let load = || {
            if std::fs::metadata(path)
                .map_err(|error| error.to_string())?
                .len()
                > MAX_TEMPLATE_BYTES as u64
            {
                return Err("Building template exceeds 2 MiB".into());
            }
            let mut source = String::new();
            std::fs::File::open(path)
                .map_err(|error| error.to_string())?
                .take(MAX_TEMPLATE_BYTES as u64 + 1)
                .read_to_string(&mut source)
                .map_err(|error| error.to_string())?;
            Self::from_json(&source)
        };
        load().map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != TEMPLATE_SCHEMA || !valid_template_id(&self.id) {
            return Err("Unsupported building schema or unsafe template ID".into());
        }
        if !catalog::valid_display_name(&self.display_name)
            || self.materials.len() > MAX_TEMPLATE_MATERIALS
        {
            return Err("Invalid building display name or too many material kinds".into());
        }
        if !self.bounds.valid()
            || !self.bounds.contains(self.anchor)
            || !normalized(self.bounds.min, self.anchor).is_some_and(house_cell)
            || !normalized(self.bounds.max, self.anchor).is_some_and(house_cell)
            || self.steps.is_empty()
            || self.steps.len() > MAX_BUILD_STEPS
        {
            return Err("Building template exceeds the current residential site envelope".into());
        }
        let mut seen = BTreeSet::new();
        let mut previous = BuildPhase::Foundation;
        for (index, step) in self.steps.iter().enumerate() {
            if !(1..=2).contains(&step.cells.len())
                || step.phase == BuildPhase::Cleanup
                || step.phase < previous
            {
                return Err(format!(
                    "Invalid phase or atomic cell count at building step {index}"
                ));
            }
            previous = step.phase;
            // The current site planner assembles walls/roof by above-ground
            // height. Reject buried cells rather than silently omitting their
            // placements and showing material prices for a different plan.
            if matches!(step.phase, BuildPhase::Walls | BuildPhase::Roof)
                && step
                    .cells
                    .iter()
                    .any(|cell| normalized(cell.position, self.anchor).is_none_or(|p| p[1] < 0))
            {
                return Err(format!(
                    "Walls and roof require a nonnegative local height at step {index}"
                ));
            }
            if step.phase != BuildPhase::Furnishing
                && step.cells.iter().any(|cell| {
                    normalized(cell.position, self.anchor).is_some_and(reserved_work_cell)
                })
            {
                return Err(format!(
                    "Building step {index} overlaps reserved interior working space"
                ));
            }
            for cell in &step.cells {
                if cell.material == 0
                    || crate::roads::is_tar(cell.material)
                    || terra_voxel::MaterialId::from_raw(cell.material).is_none()
                    || !self.bounds.contains(cell.position)
                    || !seen.insert(cell.position)
                    || !valid_pose(cell)
                {
                    return Err(format!(
                        "Invalid or duplicate building cell {:?} at step {index}",
                        cell.position
                    ));
                }
            }
            let first = &step.cells[0];
            if first.secondary {
                return Err(format!(
                    "Building step {index} starts with a secondary half"
                ));
            }
            if step.cells.len() == 2 {
                let second = &step.cells[1];
                if first.peer != Some(second.position)
                    || second.peer != Some(first.position)
                    || !second.secondary
                    || first.material != second.material
                    || first.facing != second.facing
                    || first.rotate_facing != second.rotate_facing
                    || !matches!(first.material,28|29|59|65..=67|71..=73|139)
                {
                    return Err(format!("Broken paired object at building step {index}"));
                }
                let a = first.position;
                let b = second.position;
                if a.into_iter()
                    .zip(b)
                    .map(|(a, b)| (a - b).abs())
                    .sum::<i64>()
                    != 1
                {
                    return Err(format!(
                        "Paired building cells must be adjacent at step {index}"
                    ));
                }
                let direction = match first.facing {
                    0 => [0, 0, 1],
                    1 => [1, 0, 0],
                    2 => [0, 0, -1],
                    _ => [-1, 0, 0],
                };
                let expected = if matches!(first.material, 59 | 65..=67 | 139) {
                    [0, 1, 0]
                } else if matches!(first.material, 28 | 29) {
                    [direction[2], 0, -direction[0]]
                } else {
                    direction
                };
                let offset: [i64; 3] = std::array::from_fn(|axis| b[axis] - a[axis]);
                if offset != expected || !first.rotate_facing {
                    return Err(format!(
                        "Paired object direction does not match its pose at step {index}"
                    ));
                }
            } else if first.peer.is_some() {
                return Err(format!("Missing paired half at building step {index}"));
            }
            let expected = cell_inputs(&step.cells)?;
            if step.costs != expected {
                return Err(format!(
                    "Building step {index} inputs do not match its placed material and contents"
                ));
            }
        }
        if self.materials != material_totals(&self.steps)? {
            return Err("Building material list does not match step inputs".into());
        }
        Ok(())
    }

    pub fn placed(&self, origin: [i64; 3], facing: u8) -> Result<Vec<BuildStep>, String> {
        self.validate()?;
        if facing > 3
            || origin
                .iter()
                .any(|coordinate| coordinate.unsigned_abs() > 998_900)
        {
            return Err("Invalid building placement origin or facing".into());
        }
        let point = |position: [i64; 3]| {
            let [x, y, z] = std::array::from_fn(|axis| position[axis] - self.anchor[axis]);
            let [x, z] = match facing {
                0 => [x, z],
                1 => [z, -x],
                2 => [-x, -z],
                _ => [-z, x],
            };
            [origin[0] + x, origin[1] + y, origin[2] + z]
        };
        let mut steps: Vec<BuildStep> = self
            .steps
            .iter()
            .map(|step| {
                Ok(BuildStep {
                    phase: step.phase,
                    costs: step.costs.clone(),
                    cells: step
                        .cells
                        .iter()
                        .map(|cell| {
                            Ok(BuildCell {
                                position: point(cell.position),
                                material: cell.material,
                                facing: rotated_facing(cell, facing)?,
                                peer: cell.peer.map(point),
                                secondary: cell.secondary,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        steps.sort_by_key(|step| {
            (
                step.phase,
                step.cells[0].position[1],
                step.cells[0].position[2],
                step.cells[0].position[0],
            )
        });
        Ok(steps)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod catalog_tests;

#[cfg(test)]
mod placement_tests;
