//! Only fillable vessels reserve a slot; other items follow ordinary dense belt motion.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{
    WorldDrop,
    infusion::{self, HoldPhase, HoldVisual},
};
mod geometry;
use geometry::{candidate, drive, relative};
pub use geometry::{carrying, geometry, inferred, loaded, stabilize};

pub(super) const ENTRY: f64 = -0.70;
const WORK_BUDGET: usize = 128;

#[derive(Default)]
pub(super) struct Runtime {
    cursor: Option<Cell>,
    stations: BTreeMap<Cell, Station>,
    views: BTreeMap<Cell, HoldVisual>,
}
struct Station {
    id: u64,
    item: ItemKind,
    credit: f32,
    view: HoldVisual,
}

pub(super) use geometry::holds_entry;

fn nozzle(position: Cell, chunks: &ChunkStore, journal: &EditJournal) -> Option<Cell> {
    let p = infusion::infuser_above(position);
    (chunks.block_at(infusion::gap_above(position)) == Some(0)
        && chunks.block_at(p) == Some(infusion::FLUID_INFUSER)
        && journal.block(p) == Some(infusion::FLUID_INFUSER)
        && journal
            .object_state(p)
            .is_some_and(|s| s.valid_for(p, infusion::FLUID_INFUSER)))
    .then_some(p)
}

fn mounted(position: Cell, journal: &EditJournal) -> bool {
    let p = infusion::infuser_above(position);
    journal.block(p) == Some(infusion::FLUID_INFUSER)
        && journal
            .object_state(p)
            .is_some_and(|s| s.valid_for(p, infusion::FLUID_INFUSER))
}

fn plan_fill(
    index: usize,
    position: Cell,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<(ItemKind, u8)> {
    let nozzle = nozzle(position, chunks, journal)?;
    let units = (1..=8)
        .rev()
        .find(|&units| crate::plumbing::infuser_available(chunks, journal, nozzle, units))?;
    infusion::fill(
        journal.gameplay().drops[index].stack.item,
        infusion::Liquid::Water,
        units,
    )
}

fn convert_one(
    index: usize,
    position: Cell,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
) -> bool {
    if journal.gameplay().drops[index].stack.count != 1
        || journal.gameplay().revision > u64::MAX - (terra_core::plumbing::MAX_COMPONENT as u64 + 2)
    {
        return false;
    }
    let Some((filled, units)) = plan_fill(index, position, chunks, journal) else {
        return false;
    };
    let Some(nozzle) = nozzle(position, chunks, journal) else {
        return false;
    };
    if !crate::plumbing::take_infuser(chunks, journal, nozzle, units).unwrap_or(false) {
        return false;
    }
    let game = journal.gameplay_mut();
    game.drops[index].stack.item = filled;
    game.revision += 1;
    true
}

/// Splitting at the existing upstream position creates no item displacement.
/// The admitted single separates forward through the normal swept occupancy test.
fn split_admission(
    index: usize,
    journal: &mut EditJournal,
    occupancy: &mut DropIndex,
) -> Option<usize> {
    let game = journal.gameplay();
    if game.drops[index].stack.count <= 1 || !game.can_drop() || game.revision == u64::MAX {
        return None;
    }
    let mut single = game.drops[index].clone();
    single.id = game.next_drop + 1;
    single.stack.count = 1;
    let game = journal.gameplay_mut();
    game.drops[index].stack.count -= 1;
    game.next_drop = single.id;
    occupancy
        .entry(single.position.map(|v| v.floor() as i64))
        .or_default()
        .push((single.id, single.position));
    let index = game.drops.len();
    game.drops.push(single);
    game.revision += 1;
    Some(index)
}

fn fill_progress(item: ItemKind, credit: f32, portion: u8) -> u16 {
    let (stored, capacity) = match item {
        ItemKind::GlassBottle => (0, 1),
        ItemKind::WaterBottle => (1, 1),
        ItemKind::Tool(terra_core::ToolKind::Bucket) => (0, 8),
        ItemKind::Tool(terra_core::ToolKind::WaterBucket) => (8, 8),
        _ => (
            item.watering_units(),
            item.tool_kind().map_or(0, |t| t.watering_capacity()),
        ),
    };
    if capacity == 0 {
        return 0;
    }
    let displayed = f32::from(stored) + credit / infusion::FILL_SECONDS * f32::from(portion);
    (displayed / f32::from(capacity) * 1000.).clamp(0., 1000.) as u16
}

struct Motion<'a> {
    chunks: &'a ChunkStore,
    properties: &'a terra_core::block_properties::BlockPropertiesCatalog,
    elapsed: f32,
}

fn park(index: usize, p: Cell, journal: &mut EditJournal) {
    let before = journal.gameplay().drops[index].clone();
    let drop = &mut journal.gameplay_mut().drops[index];
    stabilize(drop, p);
    if (drop.position, drop.velocity, drop.rotation, drop.grounded)
        != (
            before.position,
            before.velocity,
            before.rotation,
            before.grounded,
        )
    {
        journal.gameplay_mut().revision = journal.gameplay().revision.saturating_add(1);
    }
}

#[allow(clippy::too_many_arguments)]
fn move_drop(
    index: usize,
    p: Cell,
    facing: u8,
    destination: f64,
    speed: f64,
    motion: &Motion<'_>,
    journal: &mut EditJournal,
    occupancy: &mut DropIndex,
) -> f32 {
    let original = journal.gameplay().drops[index].clone();
    let (along, _) = relative(original.position, p, facing);
    let distance = (destination - along).clamp(
        -speed * f64::from(motion.elapsed),
        speed * f64::from(motion.elapsed),
    );
    let direction = kinetic::forward(facing);
    let spans: Vec<_> = [
        drive(p, facing, true, motion.chunks, journal),
        drive(p, facing, false, motion.chunks, journal),
    ]
    .into_iter()
    .flatten()
    .map(|(s, _)| s.span)
    .collect();
    let mut point = original.position;
    let steps = ((distance.abs() / 0.04).ceil() as usize).clamp(1, 64);
    for _ in 0..steps {
        let next =
            std::array::from_fn(|i| point[i] + direction[i] as f64 * distance / steps as f64);
        if !crate::kingdom_access::can_move_drop(motion.chunks, journal, point, next)
            || !drop_step_clear(occupancy, point, next, original.id)
            || !drop_clear_with_carriers(
                motion.chunks,
                journal,
                motion.properties,
                next,
                &spans,
                Some(p),
            )
        {
            break;
        }
        point = next;
    }
    let velocity = std::array::from_fn(|i| {
        ((point[i] - original.position[i]) / f64::from(motion.elapsed)) as f32
    });
    let drop = &mut journal.gameplay_mut().drops[index];
    drop.position = point;
    stabilize(drop, p);
    drop.velocity = velocity;
    let changed = (drop.position, drop.velocity, drop.rotation, drop.grounded)
        != (
            original.position,
            original.velocity,
            original.rotation,
            original.grounded,
        );
    if changed {
        move_indexed_drop(occupancy, drop.id, original.position, point);
        journal.gameplay_mut().revision = journal.gameplay().revision.saturating_add(1);
    }
    (((relative(point, p, facing).0 - along) / f64::from(motion.elapsed)).max(0.) as f32)
        .min(f32::from(kinetic::MAX_RPM) / 30.)
}

#[allow(clippy::too_many_arguments)]
fn advance_active(
    station: &mut Station,
    p: Cell,
    geometry: HoldVisual,
    indices: &mut BTreeMap<u64, usize>,
    motion: &Motion<'_>,
    journal: &mut EditJournal,
    occupancy: &mut DropIndex,
    assigned: &mut BTreeSet<u64>,
    admission_blocked: bool,
) {
    let Some(&index) = indices.get(&station.id) else {
        return;
    };
    let original = journal.gameplay().drops[index].clone();
    let (along, _) = relative(original.position, p, geometry.facing);
    let mut view = HoldVisual {
        active_drop: Some(station.id),
        phase: HoldPhase::Approaching,
        speed: 0.,
        ..geometry
    };
    if !loaded(p, motion.chunks, journal)
        || motion.chunks.block_at(infusion::gap_above(p)).is_none()
    {
        station.credit = 0.;
        station.view = view;
        return;
    }
    let compatible = mounted(p, journal) && infusion::fillable(original.stack.item);
    if station.item != original.stack.item {
        station.item = original.stack.item;
        station.credit = 0.;
    }
    let input =
        drive(p, geometry.facing, true, motion.chunks, journal).map_or(0., |(_, speed)| speed);
    let output =
        drive(p, geometry.facing, false, motion.chunks, journal).map_or(0., |(_, speed)| speed);
    let speed = if along < 0. {
        input.max(output)
    } else {
        output
    };
    if compatible && original.stack.count > 1 {
        station.credit = 0.;
        if (along - ENTRY).abs() > 0.00001 {
            // Legacy manually placed stacks recover smoothly to the admission plane too.
            view.speed = move_drop(
                index,
                p,
                geometry.facing,
                ENTRY,
                input.max(output),
                motion,
                journal,
                occupancy,
            );
        } else if let Some(single) = split_admission(index, journal, occupancy) {
            let drop = &journal.gameplay().drops[single];
            station.id = drop.id;
            assigned.insert(drop.id);
            indices.insert(drop.id, single);
            view.active_drop = Some(drop.id);
        }
        view.gates = !admission_blocked;
        if admission_blocked {
            view.speed = geometry.speed;
        }
    } else if compatible && along <= 0.015 && admission_blocked {
        // Existing bypass traffic drains before raising either separator.
        station.credit = 0.;
        view.speed = geometry.speed;
        if along < ENTRY {
            move_drop(
                index,
                p,
                geometry.facing,
                ENTRY,
                speed,
                motion,
                journal,
                occupancy,
            );
        } else {
            park(index, p, journal);
        }
    } else if compatible && along <= 0.015 {
        view.gates = true;
        if along < -0.015 {
            station.credit = 0.;
            view.speed = move_drop(
                index,
                p,
                geometry.facing,
                0.,
                speed,
                motion,
                journal,
                occupancy,
            );
        } else if let Some((_, portion)) = plan_fill(index, p, motion.chunks, journal) {
            view.phase = HoldPhase::Filling;
            station.credit = (station.credit + motion.elapsed).min(infusion::FILL_SECONDS);
            view.progress = fill_progress(original.stack.item, station.credit, portion);
            if station.credit >= infusion::FILL_SECONDS
                && convert_one(index, p, motion.chunks, journal)
            {
                station.credit = 0.;
                station.item = journal.gameplay().drops[index].stack.item;
                view.progress = fill_progress(station.item, 0., 0);
                if !infusion::fillable(station.item) {
                    view.phase = HoldPhase::Releasing;
                    view.gates = false;
                }
            }
        } else {
            station.credit = 0.;
            view.phase = HoldPhase::WaitingForFluid;
            view.progress = fill_progress(original.stack.item, 0., 0);
        }
    } else {
        station.credit = 0.;
        view.phase = HoldPhase::Releasing;
        view.progress = fill_progress(original.stack.item, 0., 0);
        view.speed = move_drop(
            index,
            p,
            geometry.facing,
            if output > 0. {
                along + speed * f64::from(motion.elapsed)
            } else {
                0.35
            },
            speed,
            motion,
            journal,
            occupancy,
        );
    }
    station.view = view;
}

#[allow(clippy::too_many_arguments)]
pub(super) fn advance(
    runtime: &mut Runtime,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    elapsed: f32,
    occupancy: &mut DropIndex,
    routed: &BTreeSet<u64>,
) -> BTreeSet<u64> {
    let mut assigned = BTreeSet::new();
    if !journal.has_holds() || journal.gameplay().drops.is_empty() {
        runtime.stations.clear();
        runtime.views.clear();
        runtime.cursor = None;
        journal.replace_infusion_views([]);
        return assigned;
    }
    let mut departures: Vec<_> = journal
        .gameplay()
        .drops
        .iter()
        .enumerate()
        .filter(|(_, drop)| !routed.contains(&drop.id) && routing_carrying(drop, journal).is_none())
        .filter_map(|(index, drop)| {
            geometry::departure(drop, chunks, journal)
                .map(|(segment, rpm, along)| (index, segment, rpm, along, drop.id))
        })
        .collect();
    departures.sort_unstable_by(|a, b| {
        a.1.span
            .controller
            .cmp(&b.1.span.controller)
            .then_with(|| b.3.total_cmp(&a.3))
            .then(a.4.cmp(&b.4))
    });
    let mut changed = false;
    for (index, segment, rpm, _, id) in departures.into_iter().take(WORK_BUDGET) {
        changed |= carry_on_belt(
            index, segment, rpm, chunks, journal, properties, elapsed, occupancy,
        );
        assigned.insert(id);
    }
    if changed {
        journal.gameplay_mut().revision = journal.gameplay().revision.saturating_add(1);
    }
    let mut groups = BTreeMap::<Cell, Vec<(u64, f64)>>::new();
    let mut indices = BTreeMap::new();
    for (index, drop) in journal
        .gameplay()
        .drops
        .iter()
        .take(terra_core::MAX_WORLD_DROPS)
        .enumerate()
    {
        indices.insert(drop.id, index);
        if routed.contains(&drop.id) || assigned.contains(&drop.id) {
            continue;
        }
        if let Some(p) = candidate(drop, chunks, journal) {
            let g = geometry(p, journal).unwrap();
            groups
                .entry(p)
                .or_default()
                .push((drop.id, relative(drop.position, p, g.facing).0));
            assigned.insert(drop.id);
        }
    }
    runtime.stations.retain(|p, station| {
        // Reversal changes queue order: release ownership before selecting the
        // leading item in the new direction, including saved-world recovery.
        geometry(*p, journal).is_some_and(|g| g.facing == station.view.facing)
            && groups
                .get(p)
                .is_some_and(|drops| drops.iter().any(|(id, _)| *id == station.id))
    });
    let positions: Vec<_> = groups.keys().copied().collect();
    if positions.is_empty() {
        runtime.views.clear();
        journal.replace_infusion_views([]);
        return assigned;
    }
    let start = runtime
        .cursor
        .map_or(0, |p| positions.partition_point(|next| *next <= p))
        % positions.len();
    let motion = Motion {
        chunks,
        properties,
        elapsed: elapsed.min(0.15),
    };
    runtime.views.retain(|p, _| groups.contains_key(p));
    for offset in 0..positions.len().min(WORK_BUDGET) {
        let p = positions[(start + offset) % positions.len()];
        runtime.cursor = Some(p);
        let g = geometry(p, journal).unwrap();
        let candidates = &groups[&p];
        let has_infuser = mounted(p, journal);
        if let std::collections::btree_map::Entry::Vacant(entry) = runtime.stations.entry(p)
            && let Some(&(id, _)) = candidates
                .iter()
                .filter(|(id, along)| {
                    has_infuser
                        && *along <= 0.015
                        && infusion::fillable(journal.gameplay().drops[indices[id]].stack.item)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        {
            let item = journal.gameplay().drops[indices[&id]].stack.item;
            entry.insert(Station {
                id,
                item,
                credit: 0.,
                view: g,
            });
        }
        if let Some(station) = runtime.stations.get_mut(&p) {
            let active_along = relative(
                journal.gameplay().drops[indices[&station.id]].position,
                p,
                g.facing,
            )
            .0;
            let admission_blocked = candidates.iter().any(|(id, along)| {
                *id != station.id && *along > active_along + 0.00001 && *along < 0.999999
            });
            advance_active(
                station,
                p,
                g,
                &mut indices,
                &motion,
                journal,
                occupancy,
                &mut assigned,
                admission_blocked,
            );
        }
        let active = runtime.stations.get(&p).map(|station| {
            (
                station.id,
                station.view.phase,
                relative(
                    journal.gameplay().drops[indices[&station.id]].position,
                    p,
                    g.facing,
                )
                .0,
            )
        });
        // Front-to-back motion preserves the ordinary belt's tight spacing.
        let mut stream = candidates.clone();
        stream.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        for (id, _) in stream {
            if active.is_some_and(|(active, _, _)| active == id) {
                continue;
            }
            if !loaded(p, chunks, journal) {
                continue;
            }
            let index = indices[&id];
            let (along, _) = relative(journal.gameplay().drops[index].position, p, g.facing);
            let fillable = has_infuser
                && along <= 0.015
                && infusion::fillable(journal.gameplay().drops[index].stack.item);
            let may_pass = active.is_none_or(|(_, phase, active_along)| {
                if phase == HoldPhase::Releasing {
                    !fillable
                } else {
                    along > active_along + 0.00001
                }
            });
            if may_pass {
                let input =
                    drive(p, g.facing, true, chunks, journal).map_or(0., |(_, speed)| speed);
                let output =
                    drive(p, g.facing, false, chunks, journal).map_or(0., |(_, speed)| speed);
                let speed = if along < 0. {
                    input.max(output)
                } else {
                    output
                };
                move_drop(
                    index,
                    p,
                    g.facing,
                    if output > 0. {
                        along + speed * f64::from(motion.elapsed)
                    } else {
                        0.35
                    },
                    speed,
                    &motion,
                    journal,
                    occupancy,
                );
            } else if along < ENTRY {
                let speed =
                    drive(p, g.facing, true, chunks, journal).map_or(0., |(_, speed)| speed);
                move_drop(
                    index, p, g.facing, ENTRY, speed, &motion, journal, occupancy,
                );
            } else {
                park(index, p, journal);
            }
        }
        let view = runtime.stations.get(&p).map_or(g, |station| station.view);
        runtime.views.insert(p, view);
    }
    journal.replace_infusion_views(runtime.views.values().copied());
    assigned
}
