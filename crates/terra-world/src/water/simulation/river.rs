//! Natural river falls retain their channel when landing on terraced banks.
//! Provenance is reconstructed from actual upstream cells, without new save data.
use super::*;
const TRACE_LIMIT: usize = 512;
const CACHE_LIMIT: usize = 4096;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Origin {
    Other,
    NaturalStill,
    NaturalRiver,
}

fn column(sampler: &mut Sampler<'_>, p: [i64; 3]) -> crate::SurfaceColumn {
    *sampler
        .river_columns
        .entry([p[0], p[2]])
        .or_insert_with(|| {
            sampler
                .chunks
                .surface_column_at(p[0], p[2])
                .copied()
                .unwrap_or_else(|| {
                    sampler
                        .terrain
                        .surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5)
                })
        })
}
fn unrestricted(sampler: &Sampler<'_>, journal: &EditJournal, p: [i64; 3]) -> bool {
    journal.block(p).is_some()
        || adjacent(p, [0, -1, 0]).is_some_and(|q| journal.block(q).is_some())
        || sampler
            .chunks
            .block_at(p)
            .is_some_and(super::super::space::partial)
}
fn in_channel(sampler: &mut Sampler<'_>, p: [i64; 3]) -> bool {
    let surface_column = column(sampler, p);
    surface_column.water_flow != [0.0; 2] || surface_column.water_level.is_some()
        // The caller checks voxel permeability. Open passages under the
        // adjacent bank can receive river water at and below their inlet.
        || (p[1] as f64 + 0.5) < surface_column.height
}
/// Follow only the actual stored lateral direction or vertical supply. An
/// explicit player block/fixture ends the proof and retains ordinary physics.
fn river_origin(sampler: &mut Sampler<'_>, journal: &EditJournal, start: [i64; 3]) -> bool {
    if !sampler.terrain.config().rivers {
        return false;
    }
    let mut current_cell = start;
    let mut trail = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut origin = Origin::Other;
    let mut origin_proven = false;
    while trail.len() < TRACE_LIMIT && sampler.river_origins.len() + trail.len() < CACHE_LIMIT {
        if let Some(known) = sampler.river_origins.get(&current_cell) {
            origin = *known;
            origin_proven = true;
            break;
        }
        if !seen.insert(current_cell) {
            break;
        }
        trail.push(current_cell);
        if unrestricted(sampler, journal, current_cell) {
            origin_proven = true;
            break;
        }
        let natural = sampler.natural(current_cell);
        let cell = journal.fluid(current_cell).unwrap_or(natural);
        if natural.amount > 0 {
            origin_proven = true;
            let surface_column = column(sampler, current_cell);
            if super::super::fluid::from_column(
                surface_column.water_columns().intervals[0],
                current_cell[1],
            )
            .amount
                > 0
            {
                origin = if surface_column.water_flow != [0.0; 2] {
                    Origin::NaturalRiver
                } else {
                    Origin::NaturalStill
                };
            }
            break;
        }
        if cell.amount == 0 {
            break;
        }
        if cell.source {
            origin_proven = true;
            break;
        }
        let direction = if cell.flow != [0, 0] {
            [-i64::from(cell.flow[0]), 0, -i64::from(cell.flow[1])]
        } else {
            [0, 1, 0]
        };
        let Some(parent) = adjacent(current_cell, direction) else {
            break;
        };
        current_cell = parent;
    }
    // Ordinary bucket/pipe paths never need expensive terrain-column samples.
    // A still lake becomes a river source only after its real flow enters the
    // generator's mapped channel; the ocean elsewhere retains normal physics.
    for p in trail.into_iter().rev() {
        if origin == Origin::NaturalStill && column(sampler, p).water_flow != [0.0; 2] {
            origin = Origin::NaturalRiver;
        }
        if origin_proven {
            sampler.river_origins.insert(p, origin);
        }
    }
    sampler.river_origins.get(&start) == Some(&Origin::NaturalRiver)
}

pub(super) fn lateral_allowed(
    sampler: &mut Sampler<'_>,
    journal: &EditJournal,
    target: [i64; 3],
    upstream: [i64; 3],
) -> bool {
    if !sampler.terrain.config().rivers
        || unrestricted(sampler, journal, target)
        || !river_origin(sampler, journal, upstream)
    {
        return true;
    }
    in_channel(sampler, target)
}
pub(super) fn bank_overflow(sampler: &mut Sampler<'_>, journal: &EditJournal, p: [i64; 3]) -> bool {
    sampler.terrain.config().rivers
        && !unrestricted(sampler, journal, p)
        && river_origin(sampler, journal, p)
        && !in_channel(sampler, p)
}

#[cfg(test)]
mod tests;
