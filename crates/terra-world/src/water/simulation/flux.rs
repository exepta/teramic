//! Local finite-volume transfers. A source injects mass; every other move pays
//! its donor in the same authority step. No target cell copies an upstream level.
use super::*;

pub(super) fn advance(
    sampler: &mut Sampler<'_>,
    journal: &mut EditJournal,
    p: [i64; 3],
    step: &mut FluidStep,
) -> Result<(), FluidError> {
    advance_with_supply(sampler, journal, p, step, false)
}

/// Authority already proved this cell belongs to an actual source body. Keep
/// its paid narrow falling jet joined even across a replenished model whose
/// own flow byte is calm rather than an incoming-direction breadcrumb.
pub(super) fn advance_supplied(
    sampler: &mut Sampler<'_>,
    journal: &mut EditJournal,
    p: [i64; 3],
    step: &mut FluidStep,
) -> Result<(), FluidError> {
    advance_with_supply(sampler, journal, p, step, true)
}

fn advance_with_supply(
    sampler: &mut Sampler<'_>,
    journal: &mut EditJournal,
    p: [i64; 3],
    step: &mut FluidStep,
    source_fed: bool,
) -> Result<(), FluidError> {
    let Some(mut cell) = sampler.cell(journal, p) else {
        return Ok(());
    };
    // Existing local water already touches props even when an unrelated
    // unloaded neighbor makes its next flow update wait.
    if cell.amount > 0 {
        step.wetted.push(p);
    }
    // Keep existing structure/river probes and their unloaded-section contract.
    let (incoming, waiting, _) = supply::incoming(sampler, journal, p);
    if supply::advance_construction(sampler, journal, p, &mut cell, step)? {
        return Ok(());
    }
    if let Some(section) = waiting {
        journal.sleep_fluid(p, section);
        return Ok(());
    }
    if cell.amount == 0 {
        // Only wake the donor of a reachable downward or lateral transfer.
        // Air above a pool and the dry lip of a path cannot pull water up.
        if incoming.amount > 0 {
            let direction = if incoming.flow == [0; 2] {
                [0, 1, 0]
            } else {
                [
                    -i64::from(incoming.flow[0]),
                    0,
                    -i64::from(incoming.flow[1]),
                ]
            };
            if let Some(q) = adjacent(p, direction) {
                journal.wake_fluid(q);
            }
        }
        return Ok(());
    }
    let Some(down) = adjacent(p, [0, -1, 0]) else {
        return Ok(());
    };
    let Some(below) = sampler.cell(journal, down) else {
        journal.sleep_fluid(p, global_to_section(down).0);
        return Ok(());
    };
    let capacity = |sampler: &mut Sampler<'_>, journal: &EditJournal, q| {
        sampler
            .space(journal, q)
            .clamp(FluidCell {
                amount: FLUID_CAPACITY,
                ..Default::default()
            })
            .amount
    };
    let can_fall = sampler.can_drain(journal, p, cell, down);
    // Keep a continuous thin jet while a real reservoir still feeds it. Once
    // that path is cut, every remaining finite unit is free to fall normally.
    let retained =
        u8::from(!cell.source && can_fall && (source_fed || source_feeds(sampler, journal, p)));
    if can_fall && below.source && !cell.source {
        let moved = cell.amount.saturating_sub(retained).min(2);
        cell.amount -= moved;
        pools::preflight(journal, &[(p, cell)])?;
        step.absorbed += u64::from(moved);
        step.transferred += u64::from(moved);
        step.changed |= journal.set_simulated_fluid(p, cell)?;
        return Ok(());
    }
    let falling = can_fall && !below.source && below.amount < capacity(sampler, journal, down);
    let dirs = if falling {
        vec![[0, -1, 0]]
    } else {
        vec![[-1, 0, 0], [0, 0, -1], [1, 0, 0], [0, 0, 1]]
    };
    let mut moved_any = false;
    for d in dirs {
        let Some(q) = adjacent(p, d) else { continue };
        if !sampler
            .chunks
            .block_at(q)
            .is_some_and(super::super::can_hold_fluid)
        {
            continue;
        }
        let Some(mut other) = sampler.cell(journal, q) else {
            journal.sleep_fluid(p, global_to_section(q).0);
            continue;
        };
        let probe = FluidCell {
            amount: capacity(sampler, journal, q),
            ..other
        };
        if other.source || !sampler.connected(journal, p, cell, q, probe) {
            continue;
        }
        if !falling && !river::lateral_allowed(sampler, journal, q, p) {
            continue;
        }
        let space = capacity(sampler, journal, q).saturating_sub(other.amount);
        let pressure = if falling {
            cell.amount.saturating_sub(retained)
        } else if cell.source {
            cell.amount.saturating_sub(other.amount)
        } else {
            cell.amount.saturating_sub(other.amount) / 2
        };
        let moved = pressure
            .min(space)
            .min(if falling { 2 } else { 1 })
            .min(cell.amount);
        if moved == 0 {
            continue;
        }
        other.amount += moved;
        // A partial model's height byte must reach its actual exposed void.
        // A hidden low layer would be discarded by the sampler, losing the
        // donor's paid volume; construction pressure opens that first layer.
        if sampler.space(journal, q).clamp(other).amount != other.amount {
            continue;
        }
        other.flow = [d[0] as i8, d[2] as i8];
        let mut paid = cell;
        paid.amount -= moved;
        if !cell.source && sampler.space(journal, p).clamp(paid).amount != paid.amount {
            continue;
        }
        let mut updates = vec![(q, other)];
        if !cell.source {
            updates.push((p, paid));
        }
        pools::preflight(journal, &updates)?;
        step.changed |= journal.set_simulated_fluid(q, other)?;
        if cell.source {
            step.sourced += u64::from(moved);
        } else {
            cell = paid;
            step.changed |= journal.set_simulated_fluid(p, cell)?;
        }
        step.transferred += u64::from(moved);
        moved_any = true;
        step.wetted.push(q);
        sampler.river_origins.remove(&q);
        sampler.river_origins.remove(&p);
        if cell.amount == 0 {
            break;
        }
    }
    if !cell.source
        && cell.amount > 0
        && cell.flow != [0; 2]
        && !moved_any
        && !source_feeds(sampler, journal, p)
    {
        cell.flow = [0; 2];
        pools::preflight(journal, &[(p, cell)])?;
        step.changed |= journal.set_simulated_fluid(p, cell)?;
    }
    Ok(())
}

/// Follow the stored incoming direction or actual vertical supply, never an
/// arbitrary wet neighbor. Finite cycles cannot manufacture a sustaining head.
fn source_feeds(sampler: &mut Sampler<'_>, journal: &EditJournal, mut p: [i64; 3]) -> bool {
    let mut seen = HashSet::new();
    for _ in 0..64 {
        if !seen.insert(p) {
            return false;
        }
        let Some(cell) = sampler.cell(journal, p) else {
            return false;
        };
        if cell.amount == 0 {
            return false;
        }
        if cell.source {
            return true;
        }
        if let Some(above) = adjacent(p, [0, 1, 0])
            && let Some(water) = sampler.cell(journal, above)
            && water.amount > 0
            && sampler.can_drain(journal, above, water, p)
        {
            p = above;
            continue;
        }
        if cell.flow == [0; 2] {
            return false;
        }
        let Some(back) = adjacent(p, [-i64::from(cell.flow[0]), 0, -i64::from(cell.flow[1])])
        else {
            return false;
        };
        let Some(water) = sampler.cell(journal, back) else {
            return false;
        };
        if water.amount == 0 || !sampler.connected(journal, back, water, p, cell) {
            return false;
        }
        p = back;
    }
    false
}
