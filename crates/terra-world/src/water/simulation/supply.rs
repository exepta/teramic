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
                        let permeable_model =
                            sampler.chunks.block_at(right).is_some_and(|material| {
                                terra_core::construction::is_fence(material)
                                    || terra_core::plumbing::is_part(material)
                            });
                        let connected_face = if permeable_model {
                            let space = sampler.space(journal, right).clone();
                            space.connected(
                                right_water,
                                sampler.space(journal, left),
                                left_water,
                                std::array::from_fn(|i| left[i] - right[i]),
                            )
                        } else {
                            sampler.connected(journal, right, right_water, left, left_water)
                        };
                        if !connected_face {
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

/// Fill the actual free region around a model gradually. A geometric height
/// below a lower slab is empty solid, so its first visible film starts at the
/// first nonempty height; every stored unit is still debited or source-accounted.
pub(super) fn advance_construction(
    sampler: &mut Sampler<'_>,
    journal: &mut EditJournal,
    p: [i64; 3],
    cell: &mut FluidCell,
    step: &mut FluidStep,
) -> Result<bool, FluidError> {
    if cell.source {
        return Ok(false);
    }
    let partial = sampler
        .chunks
        .block_at(p)
        .is_some_and(super::super::space::partial);
    let supplied = ambient_construction(sampler, journal, p);
    // An open lower-slab lip can overtop into actual air without turning every
    // waterlogged construction into an outlet. Keep its first visible layer,
    // and let the receiving air continue through ordinary gravity immediately.
    if !partial
        && supplied.is_none()
        && matches!(sampler.chunks.block_at(p), Some(0 | 27))
        && cell.amount < FLUID_CAPACITY
        && let Some((q, water, paid)) = finite_lip(sampler, journal, p, *cell)
    {
        let next = FluidCell {
            amount: cell.amount + 1,
            source: false,
            flow: [(p[0] - q[0]) as i8, (p[2] - q[2]) as i8],
        };
        pools::preflight(journal, &[(p, next), (q, paid)])?;
        step.changed |= journal.set_simulated_fluid(p, next)?;
        step.changed |= journal.set_simulated_fluid(q, paid)?;
        if water.source {
            step.sourced += 1;
        }
        step.transferred += 1;
        step.wetted.push(p);
        *cell = next;
        return Ok(false);
    }
    let above = adjacent(p, [0, 1, 0]);
    let gravity = above
        .and_then(|q| sampler.cell(journal, q).map(|water| (q, water)))
        .filter(|(q, water)| {
            partial && water.amount > 0 && sampler.can_drain(journal, *q, *water, p)
        });
    // A finite terrace can reach a model's first exposed layer without
    // inventing a source. Ordinary one-unit entry below that layer would be
    // entirely inside the model's solid and discarded by the sampler.
    let lateral = if partial && supplied.is_none() && gravity.is_none() {
        [[-1, 0, 0], [0, 0, -1], [1, 0, 0], [0, 0, 1]]
            .into_iter()
            .filter_map(|d| {
                let q = adjacent(p, d)?;
                let water = sampler.cell(journal, q)?;
                if water.source || water.amount.saturating_sub(cell.amount) < 2 {
                    return None;
                }
                let target = sampler.space(journal, p).clamp(water);
                (target.amount > 0 && sampler.connected(journal, q, water, p, target))
                    .then_some((q, water))
            })
            .max_by_key(|(_, water)| water.amount)
    } else {
        None
    };
    let head = supplied
        .or_else(|| {
            gravity.map(|_| {
                sampler.space(journal, p).clamp(FluidCell {
                    amount: FLUID_CAPACITY,
                    ..Default::default()
                })
            })
        })
        .or_else(|| lateral.map(|(_, water)| sampler.space(journal, p).clamp(water)));
    let Some(head) = head.filter(|head| head.amount > 0) else {
        return Ok(false);
    };
    let previous = journal.fluid(p).map_or(cell.amount, |water| water.amount);
    let next = (previous.saturating_add(1)..=head.amount).find_map(|amount| {
        let candidate = sampler.space(journal, p).clamp(FluidCell {
            amount,
            source: false,
            flow: [0; 2],
        });
        (candidate.amount == amount).then_some(candidate)
    });
    if let Some(next) = next {
        let moved = next.amount - previous;
        let donor = if supplied.is_none() {
            gravity.filter(|(_, water)| !water.source).or(lateral)
        } else {
            None
        };
        if donor.is_some_and(|(_, water)| water.amount < moved) {
            return Ok(true);
        }
        if let Some((q, mut water)) = donor {
            water.amount -= moved;
            if sampler.space(journal, q).clamp(water).amount != water.amount {
                return Ok(true);
            }
        }
        let mut updates = vec![(p, next)];
        if let Some((q, mut water)) = donor {
            water.amount -= moved;
            updates.push((q, water));
        }
        pools::preflight(journal, &updates)?;
        for (q, water) in updates {
            step.changed |= journal.set_simulated_fluid(q, water)?;
        }
        if donor.is_none() {
            step.sourced += u64::from(moved);
        }
        step.transferred += u64::from(moved);
        step.wetted.push(p);
        *cell = next;
    } else if cell.flow != [0; 2] {
        cell.flow = [0; 2];
        pools::preflight(journal, &[(p, *cell)])?;
        step.changed |= journal.set_simulated_fluid(p, *cell)?;
    }
    // A real downward inlet may continue through a permeable model. Lateral
    // bank-only waterlogging still stays sealed by its existing face rules.
    let through_flow = gravity.is_some()
        && adjacent(p, [0, -1, 0]).is_some_and(|below| {
            let Some(water) = sampler.cell(journal, below) else {
                return false;
            };
            let capacity = sampler
                .space(journal, below)
                .clamp(FluidCell {
                    amount: FLUID_CAPACITY,
                    ..Default::default()
                })
                .amount;
            sampler.can_drain(journal, p, *cell, below) && (water.source || water.amount < capacity)
        });
    Ok(!through_flow)
}

/// A closed source-free pool must not swallow work arriving through its model
/// wall. Real reservoir pressure and funded finite lips retain their intake.
pub(super) fn model_intake(
    sampler: &mut Sampler<'_>,
    journal: &EditJournal,
    p: [i64; 3],
    cell: FluidCell,
) -> bool {
    ambient_construction(sampler, journal, p).is_some_and(|head| head.amount > cell.amount)
        || finite_lip(sampler, journal, p, cell).is_some()
}

fn finite_lip(
    sampler: &mut Sampler<'_>,
    journal: &EditJournal,
    p: [i64; 3],
    cell: FluidCell,
) -> Option<([i64; 3], FluidCell, FluidCell)> {
    if !matches!(sampler.chunks.block_at(p), Some(0 | 27)) || cell.amount >= FLUID_CAPACITY {
        return None;
    }
    for d in [[-1, 0, 0], [0, 0, -1], [1, 0, 0], [0, 0, 1]] {
        let Some(q) = adjacent(p, d) else { continue };
        let lower_slab = sampler
            .chunks
            .block_at(q)
            .is_some_and(terra_core::construction::is_slab)
            && journal.object_state(q).unwrap_or_default().facing < 4;
        if !lower_slab {
            continue;
        }
        let Some(water) = sampler.cell(journal, q) else {
            continue;
        };
        let difference = water.amount.saturating_sub(cell.amount);
        if water.amount <= 5 || difference == 0 {
            continue;
        }
        // A real reservoir replenishes this lip, so its last eighth can level
        // with the receiving pool. A finite film must keep the two-unit gap
        // that prevents the same unit swapping straight back next update.
        if !water.source
            && difference < 2
            && ambient_construction(sampler, journal, q)
                .is_none_or(|head| head.amount < water.amount)
        {
            continue;
        }
        let mut paid = water;
        if !water.source {
            paid.amount -= 1;
        }
        if sampler.space(journal, q).clamp(paid).amount != paid.amount {
            continue;
        }
        let face = sampler.space(journal, q).clone();
        if face.connected(
            water,
            sampler.space(journal, p),
            water,
            std::array::from_fn(|i| p[i] - q[i]),
        ) {
            return Some((q, water, paid));
        }
    }
    None
}
