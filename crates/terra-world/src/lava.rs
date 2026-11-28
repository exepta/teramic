//! Static calderas are implicit reservoirs. Only edits and their connected
//! flow fronts need authority work. Flow state travels with normal section edits.
use crate::{
    edits::{BlockObjectState, EditJournal, LiquidState},
    streaming::{
        BlockEditRequest, ChunkStore, SectionPublished, WorldAccess, commit_block_edit,
        commit_object_state,
    },
};
use bevy::prelude::*;
use std::collections::{HashSet, VecDeque};
use terra_core::block_properties::BlockPropertiesCatalog;
use terra_voxel::{MaterialId as M, global_to_section};

mod buckets;
pub use buckets::{pour, take_source, target};

pub const LAVA: u16 = M::Lava as u16;
pub const FUMAROLE: u16 = M::Fumarole as u16;
pub const POROUS_DECORATIONS: [u16; 6] = [
    M::AshBush as u16,
    M::BurntGrass as u16,
    M::CharredSapling as u16,
    M::EmberCrystals as u16,
    M::FossilBones as u16,
    M::AshMushrooms as u16,
];
const SIDES: [[i64; 3]; 4] = [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]];
pub(crate) const NEIGHBORS: [[i64; 3]; 6] = [
    [-1, 0, 0],
    [1, 0, 0],
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
];
fn add(a: [i64; 3], b: [i64; 3]) -> [i64; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}

#[derive(Default)]
pub(crate) struct Front {
    queue: VecDeque<[i64; 3]>,
    pending: HashSet<[i64; 3]>,
}
impl Front {
    fn insert(&mut self, p: [i64; 3]) {
        if self.pending.insert(p) {
            self.queue.push_back(p);
        }
    }
    fn pop(&mut self) -> Option<[i64; 3]> {
        let p = self.queue.pop_front()?;
        self.pending.remove(&p);
        Some(p)
    }
}

impl ChunkStore {
    pub fn lava_at(&self, position: [i64; 3]) -> bool {
        self.block_at(position) == Some(LAVA)
    }
    pub(crate) fn wake_lava(&mut self, p: [i64; 3]) {
        self.lava_front.insert(p);
        for d in NEIGHBORS {
            self.lava_front.insert(add(p, d));
        }
    }
}

pub(crate) fn after_edit(chunks: &mut ChunkStore, p: [i64; 3], before: u16, after: u16) {
    if before == LAVA || after == LAVA || NEIGHBORS.iter().any(|d| chunks.lava_at(add(p, *d))) {
        chunks.wake_lava(p);
    }
}

/// This also sees manually poured and flowing water. No generation requests
/// are made by a heat query in unloaded terrain.
pub fn wet(chunks: &ChunkStore, journal: &EditJournal, p: [i64; 3]) -> bool {
    let Some(raw) = chunks.block_at(p) else {
        return false;
    };
    if !crate::water::can_hold_fluid(raw) {
        return false;
    }
    journal.fluid(p).map_or_else(
        || {
            raw == 27
                || chunks.surface_column_at(p[0], p[2]).is_some_and(|c| {
                    crate::water::falls::sampled(c.water_columns(), p, journal).amount > 0
                })
        },
        |f| f.amount > 0,
    )
}

fn state(chunks: &ChunkStore, journal: &EditJournal, p: [i64; 3]) -> Option<LiquidState> {
    chunks.lava_at(p).then(|| {
        journal
            .object_state(p)
            .and_then(|s| s.liquid)
            .unwrap_or(LiquidState {
                origin: p,
                source: true,
                volume: 8,
                age: 0,
            })
    })
}

pub fn level(journal: &EditJournal, p: [i64; 3]) -> f64 {
    journal
        .object_state(p)
        .and_then(|s| s.liquid)
        .map_or(1.0, |s| f64::from(s.volume) / 8.)
}

/// A porous decoration can be flooded between opposing streams or in a
/// pocket enclosed by rock and melt at the bank. A single wet neighbour
/// next to open ground cannot flood a plant perched on a dry shoreline.
pub fn immersed_decoration(chunks: &ChunkStore, cell: [i64; 3], raw: u16) -> bool {
    if !POROUS_DECORATIONS.contains(&raw) {
        return false;
    }
    let sides = SIDES.map(|d| chunks.block_at(add(cell, d)));
    let wet = sides.map(|m| m == Some(LAVA));
    (wet[0] && wet[1])
        || (wet[2] && wet[3])
        || (wet.contains(&true)
            && sides
                .into_iter()
                .all(|m| m == Some(LAVA) || m.and_then(M::from_raw).is_some_and(M::is_opaque)))
}

pub fn occupies(chunks: &ChunkStore, cell: [i64; 3]) -> bool {
    chunks
        .block_at(cell)
        .is_some_and(|raw| raw == LAVA || immersed_decoration(chunks, cell, raw))
}

/// Shared lattice sample for both the visible surface and body contact.
#[derive(Clone, Copy)]
pub struct SurfaceCorner {
    pub height: f64,
    pub downhill: [f32; 2],
}

/// One-meter steps in generated channels share the downstream lip. The
/// slope stays inside the upper lava voxel, leaving higher banks and separate
/// drops intact. The four columns have the same ordering on each chunk side.
pub fn natural_slope_corner(
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
    journal: &EditJournal,
    vertex: [i64; 3],
) -> Option<SurfaceCorner> {
    let cells =
        [[-1, -1, -1], [-1, -1, 0], [0, -1, -1], [0, -1, 0]].map(|offset| add(vertex, offset));
    let mut upper = [false; 4];
    let mut lower = [false; 4];
    for (i, cell) in cells.into_iter().enumerate() {
        match block(cell) {
            Some(LAVA) => {
                if journal.block(cell) == Some(LAVA) || block(add(cell, [0, 1, 0]))? == LAVA {
                    return None;
                }
                upper[i] = true;
            }
            Some(0) => {
                let below = add(cell, [0, -1, 0]);
                if block(below) == Some(LAVA) {
                    if journal.block(below) == Some(LAVA) {
                        return None;
                    }
                    lower[i] = true;
                } else if block(add(below, [0, -1, 0])) == Some(LAVA) {
                    // A third level at this vertex belongs to a real ledge.
                    return None;
                }
            }
            Some(_) => {}
            None => return None,
        }
    }
    let grade = |a: usize, b: usize| {
        f32::from(u8::from(upper[a] && lower[b])) - f32::from(u8::from(lower[a] && upper[b]))
    };
    let edge_grades = [grade(0, 2), grade(1, 3), grade(0, 1), grade(2, 3)];
    edge_grades
        .iter()
        .any(|&grade| grade != 0.)
        .then_some(SurfaceCorner {
            height: 0.,
            downhill: [
                edge_grades[0] + edge_grades[1],
                edge_grades[2] + edge_grades[3],
            ],
        })
}

pub fn surface_corner(
    chunks: &ChunkStore,
    journal: &EditJournal,
    vertex: [i64; 3],
) -> SurfaceCorner {
    let mut total = 0.;
    let mut wet_cells = 0;
    let mut dry_cells = 0;
    let mut heights = [[None; 2]; 2];
    let mut moving = false;
    let mut covered = false;
    for dx in -1..=0 {
        for dz in -1..=0 {
            let cell = add(vertex, [dx, -1, dz]);
            let h = match chunks.block_at(cell) {
                Some(LAVA) => {
                    let h = level(journal, cell);
                    wet_cells += 1;
                    total += h;
                    moving |= journal.block(cell) == Some(LAVA);
                    covered |= chunks.lava_at(add(cell, [0, 1, 0]));
                    h
                }
                Some(0) => {
                    dry_cells += 1;
                    0.
                }
                _ => continue,
            };
            heights[(dx + 1) as usize][(dz + 1) as usize] = Some(h as f32);
        }
    }
    if !moving
        && !covered
        && wet_cells > 0
        && let Some(slope) = natural_slope_corner(chunks.block_sampler(), journal, vertex)
    {
        return slope;
    }
    // Keep the foot of a fall joined to the vertical sheet. A spreading toe
    // feathers into exposed ground.
    let height = if covered || !moving || wet_cells == 0 {
        1.
    } else {
        total / (f64::from(wet_cells) + 0.6 * f64::from(dry_cells))
    };
    // Rock blocks the current. A missing height across a basalt wall must
    // not look like a drop to zero: it used to pull a flat pool's texture
    // into triangles against all four banks.
    let drop = |a: Option<f32>, b| a.zip(b).map_or(0., |(a, b)| a - b);
    let downhill = [
        drop(heights[0][0], heights[1][0]) + drop(heights[0][1], heights[1][1]),
        drop(heights[0][0], heights[0][1]) + drop(heights[1][0], heights[1][1]),
    ];
    SurfaceCorner {
        height,
        downhill: if moving { downhill } else { [0.; 2] },
    }
}

pub fn height_at(
    chunks: &ChunkStore,
    journal: &EditJournal,
    cell: [i64; 3],
    x: f64,
    z: f64,
) -> f64 {
    let h = |dx, dz| surface_corner(chunks, journal, add(cell, [dx, 1, dz])).height;
    let u = (x - cell[0] as f64).clamp(0., 1.);
    let v = (z - cell[2] as f64).clamp(0., 1.);
    // Same diagonal as the two triangles emitted by the lava mesher.
    let h0 = h(0, 0);
    let h2 = h(1, 1);
    let local = if u > v {
        h0 * (1. - u) + h(1, 0) * (u - v) + h2 * v
    } else {
        h0 * (1. - v) + h(0, 1) * (v - u) + h2 * u
    };
    cell[1] as f64 + local
}

fn incoming(chunks: &ChunkStore, journal: &EditJournal, p: [i64; 3]) -> Option<LiquidState> {
    let feed_at = |cell| {
        let feed = state(chunks, journal, cell)?;
        // Unloaded sources may still feed the loaded run. A bucket or a
        // quenched source cuts the supply immediately, including below drops.
        (feed.source
            || chunks.block_at(feed.origin).is_none()
            || state(chunks, journal, feed.origin).is_some_and(|s| s.source))
        .then_some(feed)
    };
    let above = add(p, [0, 1, 0]);
    if let Some(feed) = feed_at(above)
        && feed.origin[1].abs_diff(p[1]) <= 512
    {
        return Some(LiquidState {
            origin: feed.origin,
            volume: 8,
            age: 0,
            source: false,
        });
    }
    SIDES
        .iter()
        .filter_map(|d| {
            let neighbor = add(p, *d);
            let feed = feed_at(neighbor)?;
            let below = add(neighbor, [0, -1, 0]);
            let bed = chunks.block_at(below)?;
            // A free-falling column supplies its foot only. Treating the
            // column under a ledge as a floor grows suspended sheets at every Y.
            if bed == 0 || (bed == LAVA && !state(chunks, journal, below)?.source) {
                return None;
            }
            // Earlier saves stored only volume. Recover a conservative path
            // length until that cell next receives an authoritative feed.
            let travelled = if feed.source {
                0
            } else if feed.age == 0 && !feed.source && feed.volume < 8 {
                (8 - feed.volume) * 2 - 1
            } else {
                feed.age
            };
            let steps = travelled + 1;
            if steps > LiquidState::LAVA_RUN {
                return None;
            }
            Some(LiquidState {
                origin: feed.origin,
                volume: 8 - steps.div_ceil(2),
                age: steps,
                source: false,
            })
        })
        .min_by_key(|s| (s.age, s.origin))
}

/// Ejected melt briefly feeds the normal flow solver before its thick core
/// freezes. Only natural volcanic substrate accepts ejecta; settled bombs and
/// player edits cannot seed a growing pile over repeated eruptions.
pub fn deposit_ejecta(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
    bed: [i64; 3],
    molten: bool,
) {
    if journal.block(bed).is_some()
        || !matches!(
            chunks.block_at(bed).and_then(M::from_raw),
            Some(
                M::Ash
                    | M::CompactedAsh
                    | M::ScorchedDirt
                    | M::DarkVolcanicStone
                    | M::VolcanicGravel
                    | M::ColumnarBasalt
                    | M::VolcanicObsidian
                    | M::Stone
            )
        )
    {
        return;
    }
    let p = add(bed, [0, 1, 0]);
    if journal.block(p).is_some() || chunks.block_at(p) != Some(0) || wet(chunks, journal, p) {
        return;
    }
    let material = if molten {
        LAVA
    } else {
        M::DarkVolcanicStone as u16
    };
    if matches!(
        commit_block_edit(
            chunks,
            journal,
            &BlockEditRequest {
                position: p,
                expected_material: 0,
                material
            },
            properties
        ),
        Ok(true)
    ) && molten
    {
        let liquid = LiquidState {
            origin: p,
            volume: 8,
            source: true,
            age: 8,
        };
        if let Err(error) = commit_object_state(
            chunks,
            journal,
            p,
            Some(BlockObjectState {
                facing: 7,
                liquid: Some(liquid),
                ..Default::default()
            }),
        ) {
            tracing::warn!(%error,?p,"Invalid volcanic impact state");
        }
    }
}

pub fn ignite(chunks: &ChunkStore, journal: &mut EditJournal, p: [i64; 3]) {
    if chunks
        .block_at(p)
        .and_then(M::from_raw)
        .is_some_and(M::flammable)
    {
        crate::fire::ignite_from_lava(chunks, journal, p);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn tick(
    time: Res<Time>,
    access: Res<WorldAccess>,
    mut published: MessageReader<SectionPublished>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    properties: Res<BlockPropertiesCatalog>,
    config: Option<Res<terra_core::GameConfig>>,
    mut last: Local<f64>,
) {
    if !access.authoritative() {
        published.clear();
        return;
    }
    for event in published.read() {
        // Restored edits on either side can feed into this newly loaded section.
        for offset in [
            [0, 0, 0],
            [-1, 0, 0],
            [1, 0, 0],
            [0, -1, 0],
            [0, 1, 0],
            [0, 0, -1],
            [0, 0, 1],
        ] {
            let Ok(owner) = event.position.offset(offset) else {
                continue;
            };
            for (cell, raw) in journal.blocks_in_section(owner) {
                let Ok(p) = owner.global_block(cell) else {
                    continue;
                };
                if owner != event.position
                    && !NEIGHBORS
                        .iter()
                        .any(|&d| global_to_section(add(p, d)).0 == event.position)
                {
                    continue;
                }
                after_edit(&mut chunks, p, raw, raw);
            }
        }
    }
    let now = time.elapsed_secs_f64();
    if now - *last < 0.34 {
        return;
    }
    *last = now;
    let count = chunks.lava_front.queue.len().min(384);
    // Commit a single generation of the front per interval.
    let pending: Vec<_> = (0..count).filter_map(|_| chunks.lava_front.pop()).collect();
    let mut moves = Vec::new();
    for p in pending {
        let Some(raw) = chunks.block_at(p) else {
            continue;
        };
        if p.iter().any(|v| v.unsigned_abs() > 1_000_000)
            || (config.as_ref().is_none_or(|c| c.world.bounded_world)
                && !(-128..=356).contains(&p[1]))
        {
            continue;
        }
        if raw != 0 && raw != LAVA {
            if M::from_raw(raw).is_some_and(M::flammable)
                && config
                    .as_deref()
                    .is_none_or(|c| c.world.game_rules.fire_spread)
                && NEIGHBORS.iter().any(|d| chunks.lava_at(add(p, *d)))
            {
                crate::fire::ignite_from_lava(&chunks, &mut journal, p);
            }
            continue;
        }
        let current = state(&chunks, &journal, p);
        if current.is_some() && NEIGHBORS.iter().any(|&d| wet(&chunks, &journal, add(p, d))) {
            moves.push((p, raw, M::LavaCrust as u16, None));
            continue;
        }
        if let Some(mut melt) = current.filter(|s| s.source && s.age > 0) {
            melt.age -= 1;
            if melt.age == 0 {
                moves.push((p, raw, M::LavaCrust as u16, None));
            } else {
                moves.push((p, raw, LAVA, Some(melt)));
            }
            continue;
        }
        if current.is_some_and(|s| s.source) || wet(&chunks, &journal, p) {
            continue;
        }
        let next = incoming(&chunks, &journal, p);
        if next != current {
            moves.push((p, raw, if next.is_some() { LAVA } else { 0 }, next));
        }
    }
    for (p, before, material, next) in moves {
        let edit = BlockEditRequest {
            position: p,
            expected_material: before,
            material,
        };
        if before != material
            && !matches!(
                commit_block_edit(&mut chunks, &mut journal, &edit, &properties),
                Ok(true)
            )
        {
            continue;
        }
        if let Some(liquid) = next {
            let obj = BlockObjectState {
                facing: liquid.volume - 1,
                liquid: Some(liquid),
                ..Default::default()
            };
            if let Err(error) = commit_object_state(&mut chunks, &mut journal, p, Some(obj)) {
                tracing::warn!(%error,?p,"Invalid lava front");
            }
        }
        chunks.wake_lava(p);
    }
}

/// Height of the hot core during a fumarole's pressure burst.
/// The renderer and the damage check share the quiet phase between bursts.
pub fn plume_height(p: [i64; 3], seconds: f64) -> f64 {
    let offset = ((p[0] as u64).wrapping_mul(31) ^ (p[2] as u64).wrapping_mul(17)) % 401;
    let wave = ((seconds * 0.94 + offset as f64 * 0.093).sin() * 0.5 + 0.5).powi(3);
    let burst = ((wave - 0.38) / 0.62).clamp(0., 1.);
    3.43 * burst * burst * (3. - 2. * burst)
}

/// Feet and body size share the representation used by creature movement.
pub fn thermal_damage(
    chunks: &ChunkStore,
    journal: &EditJournal,
    feet: [f64; 3],
    size: [f64; 3],
    seconds: f64,
) -> f32 {
    if feet.iter().any(|v| !v.is_finite() || v.abs() > 1_000_000.)
        || size.iter().any(|v| !v.is_finite() || *v <= 0. || *v > 128.)
    {
        return 0.;
    }
    let low = std::array::from_fn(|i| {
        (feet[i] - if i == 1 { 4. } else { size[i] * 0.35 }).floor() as i64
    });
    let high = std::array::from_fn(|i| {
        (feet[i]
            + if i == 1 {
                size[i].min(4.)
            } else {
                size[i] * 0.35
            })
        .floor() as i64
    });
    let low = global_to_section(low).0;
    let high = global_to_section(high).0;
    let near = (low.z..=high.z).any(|z| {
        (low.x..=high.x).any(|x| {
            (low.y..=high.y).any(|y| {
                chunks
                    .loaded
                    .get(&terra_voxel::SectionPos::new(x, y, z))
                    .is_some_and(|s| s.data.may_contain(LAVA) || s.data.may_contain(FUMAROLE))
            })
        })
    });
    if !near {
        return 0.;
    }
    let mut result = 0.0f32;
    for dx in [-0.34, 0., 0.34] {
        for dz in [-0.34, 0., 0.34] {
            let x = feet[0] + size[0] * dx;
            let z = feet[2] + size[2] * dz;
            let bottom = feet[1].floor() as i64;
            let rock_x = x.floor() as i64;
            let rock_z = z.floor() as i64;
            for y in bottom..=(feet[1] + size[1].min(4.)).floor() as i64 {
                let cell = [rock_x, y, rock_z];
                if occupies(chunks, cell) && feet[1] < height_at(chunks, journal, cell, x, z) {
                    return 8.;
                }
            }
            for y in (bottom - 4..=bottom).rev() {
                let cell = [rock_x, y, rock_z];
                let raw = chunks.block_at(cell);
                if raw == Some(FUMAROLE) {
                    let above = feet[1] - (y as f64 + 1.0);
                    if above >= 0.
                        && above < plume_height(cell, seconds)
                        && (x - (rock_x as f64 + 0.5)).hypot(z - (rock_z as f64 + 0.5)) < 0.39
                        && chunks.block_at([rock_x, y + 1, rock_z]) == Some(0)
                    {
                        result = 3.;
                    }
                    break;
                }
                if raw != Some(0) {
                    break;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests;
