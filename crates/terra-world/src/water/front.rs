//! Shared moving waterline used by rendering, immersion and optical sampling.
use super::{FluidCell, can_hold_fluid};
use crate::{TerraGenerator, edits::EditJournal, streaming::ChunkStore};

/// Both sections at a vertical boundary sample the receiving cell's inlet.
/// An isolated free jet narrows, while a flooded layer stays joined to its banks.
pub(crate) fn inlet_width(p: [i64; 3], cell: FluidCell, wet: impl Fn([i64; 3]) -> bool) -> f64 {
    if cell.source || cell.amount >= 8 || (0..2).any(|axis| super::tongue::wet_flank(p, axis, &wet))
    {
        1.
    } else {
        cell.height().sqrt()
    }
}

pub fn leading_front(
    chunks: &ChunkStore,
    terrain: &TerraGenerator,
    journal: &EditJournal,
    p: [i64; 3],
    cell: FluidCell,
) -> Option<terra_core::fluid_surface::Front> {
    if cell.source || cell.amount == 0 || cell.amount >= 8 || cell.flow == [0; 2] {
        return None;
    }
    let flow = cell.flow;
    let axis = usize::from(flow[0] == 0);
    let component = flow[axis];
    let mut back = p;
    let mut ahead = p;
    let world_axis = if axis == 0 { 0 } else { 2 };
    back[world_axis] -= i64::from(component);
    ahead[world_axis] += i64::from(component);
    let material = chunks.block_at(ahead)?;
    if !super::tongue::front_boundary(material)
        || chunks
            .fluid_cell_at(terrain, journal, ahead, material)
            .amount
            > 0
        || super::tongue::wet_flank(p, axis, |q| {
            chunks
                .block_at(q)
                .is_some_and(|m| chunks.fluid_cell_at(terrain, journal, q, m).amount > 0)
        })
    {
        return None;
    }
    let m = chunks.block_at(back)?;
    let head = chunks.fluid_cell_at(terrain, journal, back, m).height() as f32;
    terra_core::fluid_surface::Front::new(cell.height() as f32, head, axis, component > 0)
}
pub fn flowing_surface_height(
    chunks: &ChunkStore,
    terrain: &TerraGenerator,
    journal: &EditJournal,
    position: [f64; 3],
) -> Option<f64> {
    let p = position.map(|v| v.floor() as i64);
    let material = chunks.block_at(p)?;
    if !can_hold_fluid(material) {
        return None;
    }
    let cell = chunks.fluid_cell_at(terrain, journal, p, material);
    if cell.amount == 0 {
        return None;
    }
    // A supplied vertical stream spends its fractional volume on width, as
    // the fluid mesher does, and remains continuous over the cell's height.
    // Sampling it as a shallow puddle would make the visible jet's center dry.
    if !cell.source && cell.amount < 8 && matches!(material, 0 | 27) {
        let above = [p[0], p[1] + 1, p[2]];
        if let Some(above_material) = chunks.block_at(above) {
            let supplied = chunks.fluid_cell_at(terrain, journal, above, above_material);
            if supplied.amount > 0
                && super::space::FluidSpace::at(journal, p, |q| chunks.block_at(q)).connected(
                    FluidCell { amount: 8, ..cell },
                    &super::space::FluidSpace::at(journal, above, |q| chunks.block_at(q)),
                    supplied,
                    [0, 1, 0],
                )
            {
                let at = |q| {
                    chunks
                        .block_at(q)
                        .map(|m| chunks.fluid_cell_at(terrain, journal, q, m))
                };
                let wet = |q| at(q).is_some_and(|c| c.amount > 0);
                let top = inlet_width(p, cell, wet);
                let below = [p[0], p[1] - 1, p[2]];
                let bottom = at(below)
                    .filter(|c| c.amount > 0)
                    .filter(|_| matches!(chunks.block_at(below), Some(0 | 27)))
                    .map_or(top, |c| inlet_width(below, c, wet));
                let span = [0, 2]
                    .into_iter()
                    .map(|i| 2. * (position[i] - p[i] as f64 - 0.5).abs())
                    .fold(0., f64::max);
                let local_y = position[1] - p[1] as f64;
                if span > bottom + (top - bottom) * local_y {
                    return None;
                }
                let surface = if span > top {
                    (bottom - span) / (bottom - top)
                } else {
                    1.
                };
                return Some(p[1] as f64 + surface);
            }
        }
    }
    let height = if let Some(front) = leading_front(chunks, terrain, journal, p, cell) {
        let axis = if front.axis == 0 { 0 } else { 2 };
        if let Some(tongue) = super::tongue::Tongue::on_bed(
            front,
            p,
            |q| chunks.block_at(q),
            |q| {
                chunks
                    .block_at(q)
                    .is_some_and(|m| chunks.fluid_cell_at(terrain, journal, q, m).amount > 0)
            },
        ) {
            let xz = [0, 2].map(|i| (position[i] - p[i] as f64) as f32);
            f64::from(tongue.height_at(xz)?)
        } else {
            f64::from(front.height_at((position[axis] - p[axis] as f64) as f32)?)
        }
    } else {
        cell.height()
    };
    Some(p[1] as f64 + height)
}
