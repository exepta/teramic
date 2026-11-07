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
        if neighbor.amount == 0 || !super::river::lateral_allowed(sampler, journal, p, q) {
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

/// Exterior water around construction remains part of its connected source
/// body, including placed pools above the procedural water table. Lateral
/// attenuation must not cut a one-eighth pit at every fence rail or stair turn.
/// A proven small pool loses all source flags during intake, so this connected
/// replenishment then ceases and its finite quantity can drain normally.
pub(super) fn ambient_construction(
    sampler: &mut Sampler<'_>,
    journal: &EditJournal,
    p: [i64; 3],
) -> Option<FluidCell> {
    use std::collections::{HashMap, HashSet, VecDeque};
    const MAX_CONSTRUCTION_CELLS: usize = 32;
    fn construction(material: u16) -> bool {
        material != terra_voxel::MaterialId::Path as u16 && super::super::space::partial(material)
    }
    // The path's 1/16 depression is terrain relief, not an immersed model.
    // Keep its normal one-eighth lateral gradient; a construction-head bypass
    // would flood an otherwise sealed generated irrigation channel over this
    // cosmetic lip. Its actual free space still accepts falling water.
    if sampler.chunks.block_at(p) == Some(terra_voxel::MaterialId::Path as u16) {
        return None;
    }
    let partial = |position| sampler.chunks.block_at(position).is_some_and(construction);
    let own_partial = partial(p);
    if !own_partial
        && !super::super::fluid::NEIGHBORS
            .into_iter()
            .any(|direction| adjacent(p, direction).is_some_and(&partial))
    {
        return None;
    }
    let natural = sampler.natural(p);
    let mut wanted = FluidCell::default();
    let maximum = sampler
        .space(journal, p)
        .clamp(FluidCell {
            amount: FLUID_CAPACITY,
            ..Default::default()
        })
        .amount;
    let full = FluidCell {
        amount: FLUID_CAPACITY,
        ..Default::default()
    };
    let mut seen = HashSet::from([p]);
    let mut parents = HashMap::new();
    let mut pending = VecDeque::from([p]);
    while let Some(at) = pending.pop_front() {
        for direction in super::super::fluid::NEIGHBORS {
            let Some(q) = adjacent(at, direction) else {
                continue;
            };
            let q_partial = sampler.chunks.block_at(q).is_some_and(construction);
            if !own_partial && at == p && !q_partial {
                continue;
            }
            let Some(source) = sampler.cell(journal, q) else {
                continue;
            };
            if source.source && source.amount > 0 {
                let natural_source = sampler.natural(q);
                let implicit = sampler.chunks.block_at(q)
                    != Some(terra_voxel::MaterialId::FluidWater as u16)
                    && natural_source.amount > 0
                    && source.amount == natural_source.amount;
                let level = |position: [i64; 3], natural: FluidCell| {
                    if implicit && natural.amount > 0 {
                        natural.amount
                    } else {
                        (i64::from(source.amount)
                            + (q[1] - position[1]) * i64::from(FLUID_CAPACITY))
                        .clamp(0, i64::from(FLUID_CAPACITY)) as u8
                    }
                };
                let target = sampler.space(journal, p).clamp(FluidCell {
                    amount: level(p, natural),
                    ..Default::default()
                });
                if target.amount > wanted.amount {
                    let mut right = q;
                    let mut right_water = source;
                    let mut cursor = Some(at);
                    let mut connected = true;
                    while let Some(left) = cursor {
                        let natural = sampler.natural(left);
                        let left_water = sampler.space(journal, left).clamp(FluidCell {
                            amount: level(left, natural),
                            ..Default::default()
                        });
                        if !sampler.connected(journal, right, right_water, left, left_water) {
                            connected = false;
                            break;
                        }
                        right = left;
                        right_water = left_water;
                        cursor = parents.get(&left).copied();
                    }
                    if connected {
                        wanted = target;
                        if wanted.amount == maximum {
                            return Some(wanted);
                        }
                    }
                }
            }
            if q_partial
                && seen.len() < MAX_CONSTRUCTION_CELLS
                && !seen.contains(&q)
                && sampler.connected(journal, at, full, q, full)
            {
                seen.insert(q);
                parents.insert(q, at);
                pending.push_back(q);
            }
        }
    }
    (wanted.amount > 0).then_some(wanted)
}
