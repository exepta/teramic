use crate::{TerraGenerator, edits::EditJournal};
use std::collections::HashMap;

pub const FLUID_CAPACITY: u8 = 8;

#[derive(Debug, thiserror::Error)]
pub enum FluidError {
    #[error("invalid liquid cell: amount must be 0..=8 and flow components -1..=1")]
    InvalidCell,
    #[error(transparent)]
    Block(#[from] terra_voxel::blocks::BlockError),
}

/// Surface height in eighths of a cell (solid sub-volumes are excluded). Generated water is an implicit reservoir;
/// water leaving that reservoir is finite and never creates another source.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FluidCell {
    pub amount: u8,
    pub flow: [i8; 2],
    pub source: bool,
}
impl FluidCell {
    pub fn validate(self) -> bool {
        self.amount <= FLUID_CAPACITY
            && self.flow.into_iter().all(|v| (-1..=1).contains(&v))
            && (!self.source || self.amount > 0)
    }
    pub fn height(self) -> f64 {
        f64::from(self.amount) / f64::from(FLUID_CAPACITY)
    }
}
pub type FluidSnapshot = HashMap<[i64; 3], FluidCell>;

pub fn fluid_cell_at(
    terrain: &TerraGenerator,
    journal: &EditJournal,
    global: [i64; 3],
    block: u16,
) -> FluidCell {
    if !can_hold_fluid(block) || !terrain.config().water {
        return FluidCell::default();
    }
    let cell = journal
        .fluid(global)
        .unwrap_or_else(|| natural_cell(terrain, global));
    if super::space::partial(block) {
        super::space::FluidSpace::at(journal, global, |p| {
            if p == global {
                Some(block)
            } else {
                journal.block(p)
            }
        })
        .clamp(cell)
    } else {
        cell
    }
}

pub fn can_hold_fluid(block: u16) -> bool {
    super::space::partial(block)
        || matches!(
            terra_voxel::MaterialId::from_raw(block),
            Some(
                terra_voxel::MaterialId::Air
                    | terra_voxel::MaterialId::FluidWater
                    | terra_voxel::MaterialId::Seagrass
                    | terra_voxel::MaterialId::SaltCrystal
                    | terra_voxel::MaterialId::DripstoneTip
                    | terra_voxel::MaterialId::IceCrystal
                    | terra_voxel::MaterialId::CaveVine
                    | terra_voxel::MaterialId::CaveRoot
            )
        )
}

pub(super) fn natural_cell(terrain: &TerraGenerator, global: [i64; 3]) -> FluidCell {
    from_columns(
        terrain.water_columns(global[0] as f64 + 0.5, global[2] as f64 + 0.5),
        global[1],
    )
}

pub(crate) fn from_columns(columns: super::WaterColumns, y: i64) -> FluidCell {
    from_column(columns.at(y), y)
}

pub(crate) fn from_column(column: Option<super::WaterColumn>, y: i64) -> FluidCell {
    let Some(column) = column else {
        return FluidCell::default();
    };
    let y = y as f64;
    if y < column.bed || y >= column.level {
        return FluidCell::default();
    }
    FluidCell {
        amount: ((column.level - y).min(1.0) * f64::from(FLUID_CAPACITY))
            .ceil()
            .clamp(1.0, f64::from(FLUID_CAPACITY)) as u8,
        flow: [0, 0],
        source: true,
    }
}

pub(crate) fn adjacent(global: [i64; 3], offset: [i64; 3]) -> Option<[i64; 3]> {
    Some([
        global[0].checked_add(offset[0])?,
        global[1].checked_add(offset[1])?,
        global[2].checked_add(offset[2])?,
    ])
}

pub(crate) const NEIGHBORS: [[i64; 3]; 6] = [
    [0, -1, 0],
    [-1, 0, 0],
    [1, 0, 0],
    [0, 0, -1],
    [0, 0, 1],
    [0, 1, 0],
];
