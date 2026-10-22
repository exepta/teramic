use super::*;
use terra_voxel::SectionPos;

/// Gravity has priority; lateral supply loses one level on every supported
/// step. Strict attenuation prevents isolated pools/cycles from self-feeding.
pub(super) fn incoming(
    sampler: &mut Sampler<'_>,
    journal: &EditJournal,
    p: [i64; 3],
) -> (FluidCell, Option<SectionPos>, bool) {
    let mut wanted = FluidCell::default();
    let mut waiting = None;
    let mut source = false;
    if let Some(above) = adjacent(p, [0, 1, 0]) {
        match sampler.cell(journal, above) {
            Some(cell) if cell.amount > 0 && sampler.can_drain(journal, above, cell, p) => {
                return (
                    FluidCell {
                        amount: sampler
                            .space(journal, p)
                            .clamp(FluidCell {
                                amount: FLUID_CAPACITY,
                                ..Default::default()
                            })
                            .amount,
                        ..Default::default()
                    },
                    None,
                    cell.source,
                );
            }
            None => waiting = Some(global_to_section(above).0),
            _ => {}
        }
    }
    let directions = [[-1, 0, 0], [0, 0, -1], [1, 0, 0], [0, 0, 1]];
    let rotation = p[0].wrapping_add(p[2]).rem_euclid(4) as usize;
    for i in 0..4 {
        let direction = directions[(i + rotation) % 4];
        let Some(q) = adjacent(p, direction) else {
            continue;
        };
        let Some(neighbor) = sampler.cell(journal, q) else {
            waiting = Some(global_to_section(q).0);
            continue;
        };
        if neighbor.amount == 0 {
            continue;
        }
        let Some(down) = adjacent(q, [0, -1, 0]) else {
            continue;
        };
        let Some(floor) = sampler.chunks.block_at(down) else {
            waiting = Some(global_to_section(down).0);
            continue;
        };
        if super::super::can_hold_fluid(floor)
            && sampler.can_drain(journal, q, neighbor, down)
            && (!neighbor.source || !sampler.cell(journal, down).is_some_and(|cell| cell.source))
        {
            continue;
        }
        // A fractional natural surface may contain just one eighth of a cell.
        let level = neighbor
            .amount
            .saturating_sub(1)
            .max(u8::from(neighbor.source));
        let candidate = sampler.space(journal, p).clamp(FluidCell {
            amount: level,
            ..Default::default()
        });
        if candidate.amount > wanted.amount && sampler.connected(journal, q, neighbor, p, candidate)
        {
            wanted.amount = candidate.amount;
            wanted.flow = [-direction[0] as i8, -direction[2] as i8];
            source = neighbor.source;
        }
    }
    (wanted, waiting, source)
}
