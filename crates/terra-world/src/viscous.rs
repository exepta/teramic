//! Shared, bounded contact sampling for authority, prediction and creature movement.
use crate::{edits::EditJournal, streaming::ChunkStore};
use terra_core::{bile, roads};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViscousContact {
    pub immersion: f32,
    pub movement_multiplier: f32,
}
impl Default for ViscousContact {
    fn default() -> Self {
        Self {
            immersion: 0.,
            movement_multiplier: 1.,
        }
    }
}

const SIDES: [(usize, [i64; 3]); 4] = [
    (0, [-1, 0, 0]),
    (1, [1, 0, 0]),
    (4, [0, 0, -1]),
    (5, [0, 0, 1]),
];

fn amount(journal: &EditJournal, p: [i64; 3], material: u16) -> u8 {
    let state = journal.object_state(p).unwrap_or_default();
    state.liquid.map_or_else(
        || {
            if material == bile::FLUID {
                (state.facing + 1) * 8
            } else {
                roads::tar_amount(state.facing) * 8
            }
        },
        |state| {
            if material == bile::FLUID {
                bile::fine_volume(state.volume, state.age)
            } else {
                state.volume
            }
        },
    )
}

/// Actual static liquid footprint, matching the shared volume-preserving leading
/// wedge. A cell identifier alone must not slow a player on still-dry ground.
pub fn surface_at(chunks: &ChunkStore, journal: &EditJournal, point: [f64; 3]) -> Option<f64> {
    if point.iter().any(|v| !v.is_finite() || v.abs() > 990_000.) {
        return None;
    }
    let p = point.map(|v| v.floor() as i64);
    let material = chunks.block_at(p)?;
    if !matches!(material, bile::FLUID | roads::WET_TAR) {
        return None;
    }
    let volume = amount(journal, p, material);
    let strongest = SIDES
        .into_iter()
        .filter_map(|(side, d)| {
            let q = std::array::from_fn(|i| p[i] + d[i]);
            (chunks.block_at(q) == Some(material)).then(|| (side, amount(journal, q, material)))
        })
        .max_by_key(|&(_, volume)| volume);
    let front = strongest.and_then(|(side, head)| {
        let d = SIDES.iter().find(|(s, _)| *s == side).unwrap().1;
        let ahead = std::array::from_fn(|i| p[i] - d[i]);
        let leading = if material == bile::FLUID {
            chunks.block_at(ahead) != Some(material)
        } else {
            chunks.block_at(ahead) == Some(crate::roads::PATH)
        };
        leading
            .then(|| {
                terra_core::fluid_surface::Front::new(
                    f32::from(volume) / 64.,
                    f32::from(head) / 64.,
                    usize::from(side >= 4),
                    side % 2 == 0,
                )
            })
            .flatten()
    });
    let base = if material == roads::WET_TAR {
        f64::from(roads::PATH_HEIGHT)
    } else {
        0.
    };
    let height = if let Some(front) = front {
        let axis = if front.axis == 0 { 0 } else { 2 };
        f64::from(front.height_at((point[axis] - p[axis] as f64) as f32)?)
            / if material == roads::WET_TAR { 16. } else { 1. }
    } else if material == bile::FLUID {
        journal.object_state(p).and_then(|s| s.liquid).map_or_else(
            || f64::from(bile::height((volume / 8).max(1))),
            |s| f64::from(bile::state_height(s.volume, s.age)),
        )
    } else {
        f64::from(volume) / 1024.
    };
    Some(p[1] as f64 + base + height)
}

/// Galle is swimmable liquid. Solid Galle blocks and dry road surfaces are not.
pub fn swimming_column(
    chunks: &ChunkStore,
    journal: &EditJournal,
    point: [f64; 3],
) -> Option<crate::water::WaterColumn> {
    if point.iter().any(|v| !v.is_finite() || v.abs() > 990_000.) {
        return None;
    }
    let mut p = point.map(|v| v.floor() as i64);
    if chunks.block_at(p) != Some(bile::FLUID) {
        return None;
    }
    let mut level = surface_at(chunks, journal, point)?;
    if point[1] >= level {
        return None;
    }
    let mut bed = p[1] as f64;
    for _ in 0..64 {
        let below = [p[0], p[1] - 1, p[2]];
        if chunks.block_at(below) != Some(bile::FLUID) || amount(journal, below, bile::FLUID) != 64
        {
            break;
        }
        bed = below[1] as f64;
        p = below;
    }
    p = point.map(|v| v.floor() as i64);
    for _ in 0..64 {
        if amount(journal, p, bile::FLUID) != 64 {
            break;
        }
        let above = [p[0], p[1] + 1, p[2]];
        if chunks.block_at(above) != Some(bile::FLUID) {
            break;
        }
        let probe = [point[0], above[1] as f64 + 0.001, point[2]];
        let Some(top) = surface_at(chunks, journal, probe) else {
            break;
        };
        level = top;
        p = above;
    }
    Some(crate::water::WaterColumn {
        level,
        bed,
        depth: level - bed,
    })
}

/// Feet position and full dimensions, using nine footprint samples rather than
/// scanning an unbounded entity AABB. Wet tar adheres even as a very thin film.
pub fn body_contact(
    chunks: &ChunkStore,
    journal: &EditJournal,
    feet: [f64; 3],
    size: [f64; 3],
) -> ViscousContact {
    if feet.iter().any(|v| !v.is_finite() || v.abs() > 990_000.)
        || size
            .iter()
            .any(|v| !v.is_finite() || !(0.001..=64.).contains(v))
    {
        return ViscousContact::default();
    }
    let mut contact = ViscousContact::default();
    for x in [-0.35, 0., 0.35] {
        for z in [-0.35, 0., 0.35] {
            let sample = [
                feet[0] + size[0] * x,
                feet[1] + 0.015,
                feet[2] + size[2] * z,
            ];
            for y in [sample[1].floor() as i64, (sample[1] - 0.08).floor() as i64] {
                let point = [sample[0], y as f64 + 0.001, sample[2]];
                let p = point.map(|v| v.floor() as i64);
                let Some(material) = chunks.block_at(p) else {
                    continue;
                };
                let Some(level) = surface_at(chunks, journal, point) else {
                    continue;
                };
                if feet[1] > level + 0.045 || feet[1] + size[1] <= y as f64 {
                    continue;
                }
                if material == roads::WET_TAR {
                    if feet[1] >= y as f64 + f64::from(roads::PATH_HEIGHT) - 0.04 {
                        contact.immersion = contact.immersion.max(0.15);
                        contact.movement_multiplier = contact.movement_multiplier.min(0.38);
                    }
                } else {
                    let immersion =
                        ((level - feet[1] + 0.025) / (size[1] * 0.55)).clamp(0., 1.) as f32;
                    if immersion > 0. {
                        contact.immersion = contact.immersion.max(immersion);
                        contact.movement_multiplier =
                            contact.movement_multiplier.min(0.55 - 0.25 * immersion);
                    }
                }
            }
        }
    }
    contact
}

#[cfg(test)]
mod tests;
