//! Loaded, permission-checked steam servicing through existing water and shaft networks.
use crate::{edits::EditJournal, streaming::ChunkStore};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{
    kinetic, kingdom::KingdomPermission, plumbing, train_parts, train_power, trains::Vehicle,
};
type Cell = [i64; 3];
pub const WATER_PER_UNIT: f64 = train_power::BUCKET_WATER / 8.0;
const SERVICE_TICK: f32 = 0.2;
const VEHICLE_BUDGET: usize = 8;
#[derive(Default)]
pub struct Runtime {
    elapsed: f32,
    after: u64,
    clock: f64,
    serviced_at: BTreeMap<u64, f64>,
    dock_cursor: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct MechanicalDrive {
    pub vehicle: u64,
    pub rpm: f32,
    pub capacity: u16,
}
pub fn docks(chunks: &ChunkStore, journal: &EditJournal) -> Vec<Cell> {
    let mut cells = chunks
        .loaded
        .keys()
        .flat_map(|&section| {
            journal
                .blocks_in_section(section)
                .filter_map(move |(local, material)| {
                    let origin = section.origin().ok()?;
                    let cell = std::array::from_fn(|i| origin[i] + local[i] as i64);
                    (material == train_parts::STATION && chunks.block_at(cell) == Some(material))
                        .then_some(cell)
                })
        })
        .collect::<Vec<_>>();
    cells.sort_unstable();
    cells.truncate(64);
    cells
}
/// A parked assembly and all of its frozen machines must actually be loaded.
pub fn loaded(chunks: &ChunkStore, car: &Vehicle) -> bool {
    let bounds = car.model_bounds();
    let low = bounds[0].map(|v| v.floor() as i64);
    let high = bounds[1].map(|v| v.floor() as i64);
    let a = terra_voxel::global_to_section(low).0;
    let b = terra_voxel::global_to_section(high).0;
    (a.x..=b.x).all(|x| {
        (a.y..=b.y).all(|y| {
            (a.z..=b.z).all(|z| {
                chunks
                    .loaded
                    .contains_key(&terra_voxel::SectionPos::new(x, y, z))
            })
        })
    })
}
fn usable(chunks: &ChunkStore, journal: &EditJournal, car: &Vehicle, cell: Cell) -> bool {
    crate::kingdom_access::can_use(
        &journal.gameplay().kingdoms,
        chunks,
        car.owner,
        KingdomPermission::Inventories,
        cell,
    ) && crate::kingdom_access::can_transport(
        chunks,
        journal,
        cell,
        car.model_origin().map(|v| v.floor() as i64),
    )
}
fn near_dock(car: &Vehicle, cell: Cell) -> bool {
    near_bounds(car.model_bounds(), cell)
}
fn near_bounds(bounds: [[f64; 3]; 2], cell: Cell) -> bool {
    (0..3).all(|i| {
        cell[i] as f64 + 1.0 >= bounds[0][i] - if i == 1 { 3.0 } else { 4.0 }
            && cell[i] as f64 <= bounds[1][i] + if i == 1 { 3.0 } else { 4.0 }
    })
}
/// One physical shaft per dock and one dock per boiler prevent duplicate source capacity.
pub fn mechanical_drives(
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> BTreeMap<Cell, MechanicalDrive> {
    let mut result = BTreeMap::new();
    let mut used = BTreeSet::new();
    let ready = journal
        .gameplay()
        .trains
        .vehicles
        .values()
        .filter(|car| {
            car.motion.stopped()
                && car.motion.brake
                && car.steam.ready()
                && train_power::steam_installed(&car.parts)
                && loaded(chunks, car)
        })
        .map(|car| (car, car.model_bounds()))
        .collect::<Vec<_>>();
    for dock in docks(chunks, journal) {
        let Some(&(car, _)) = ready.iter().find(|(car, bounds)| {
            !used.contains(&car.id)
                && near_bounds(*bounds, dock)
                && usable(chunks, journal, car, dock)
        }) else {
            continue;
        };
        for direction in plumbing::DIRECTIONS {
            let cell = std::array::from_fn(|i| dock[i] + direction[i]);
            if chunks
                .block_at(cell)
                .is_some_and(|m| matches!(m, 144..=146))
                && journal.object_state(cell).is_some_and(|s| {
                    kinetic::axis(s.facing) == direction
                        || kinetic::axis(s.facing) == direction.map(|v| -v)
                })
                && usable(chunks, journal, car, cell)
            {
                result.insert(
                    cell,
                    MechanicalDrive {
                        vehicle: car.id,
                        rpm: 40.0,
                        capacity: 12800,
                    },
                );
                used.insert(car.id);
                break;
            }
        }
    }
    result
}
fn publish(journal: &mut EditJournal, mut car: Vehicle) {
    car.revision += 1;
    let state = journal.gameplay_mut();
    state.trains.vehicles.insert(car.id, car);
    state.trains.revision += 1;
    state.revision += 1;
}
fn can_publish(journal: &EditJournal, car: &Vehicle) -> bool {
    car.revision < u64::MAX
        && journal.gameplay().trains.revision < u64::MAX - 1
        && journal.gameplay().revision < u64::MAX - 2048
}
/// Each round handles at most eight vehicles; pipes lose exactly the water delivered.
pub fn advance(
    runtime: &mut Runtime,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    elapsed: f32,
) -> Result<usize, String> {
    if !elapsed.is_finite() || elapsed <= 0.0 {
        return Ok(0);
    }
    runtime.clock += f64::from(elapsed.min(0.25));
    runtime
        .serviced_at
        .retain(|id, _| journal.gameplay().trains.vehicles.contains_key(id));
    runtime.elapsed = (runtime.elapsed + elapsed).min(0.25);
    if runtime.elapsed < SERVICE_TICK {
        return Ok(0);
    }
    let tick = f64::from(std::mem::take(&mut runtime.elapsed));
    let drives = mechanical_drives(chunks, journal);
    let stations = docks(chunks, journal);
    // Recovered water is real finite storage and can be drained through the ordinary pipe network.
    for offset in 0..stations.len().min(VEHICLE_BUDGET) {
        let station = stations[(runtime.dock_cursor + offset) % stations.len()];
        if journal.gameplay().revision >= u64::MAX - 2048 {
            break;
        }
        if let Some(mut state) = journal.plumbing_at(station).cloned()
            && let Some(mut reserve) = state.train_reserve
            && reserve.water >= WATER_PER_UNIT
            && state.water.amount < 8
        {
            reserve.water -= WATER_PER_UNIT;
            state.water.amount += 1;
            state.train_reserve = (reserve.water > 0.0).then_some(reserve);
            state.flowing = true;
            journal.set_plumbing(station, state)?;
        }
    }
    runtime.dock_cursor = if stations.is_empty() {
        0
    } else {
        (runtime.dock_cursor + VEHICLE_BUDGET) % stations.len()
    };

    let mut ids = journal
        .gameplay()
        .trains
        .vehicles
        .keys()
        .copied()
        .filter(|id| *id > runtime.after)
        .take(VEHICLE_BUDGET)
        .collect::<Vec<_>>();
    if ids.is_empty() {
        ids = journal
            .gameplay()
            .trains
            .vehicles
            .keys()
            .copied()
            .take(VEHICLE_BUDGET)
            .collect();
    }
    let mut changed = 0;
    let serviced = ids.clone();
    for id in ids {
        runtime.after = id;
        let dt = runtime
            .serviced_at
            .insert(id, runtime.clock)
            .map_or(tick, |last| (runtime.clock - last).clamp(0.0, 2.0));
        let mut car = journal.gameplay().trains.vehicles[&id].clone();
        if !can_publish(journal, &car) || !loaded(chunks, &car) {
            continue;
        }
        let before = car.steam;
        if drives.iter().any(|(cell, drive)| {
            drive.vehicle == id
                && journal.kinetic_state(*cell).running()
                && journal.kinetic_state(*cell).stress > 0.0
        }) {
            car.steam.water = (car.steam.water - 4.0 * dt).max(0.0);
            car.steam.pressure_bar = (car.steam.pressure_bar - 0.45 * dt).max(0.0);
        }
        if car.motion.stopped()
            && car.motion.brake
            && car.parts.iter().any(|part| part.material == 254)
        {
            // A service dock first returns recovered water, then draws its finite pipe buffer.
            if let Some(&dock) = stations
                .iter()
                .find(|&&dock| near_dock(&car, dock) && usable(chunks, journal, &car, dock))
            {
                let mut state = journal.plumbing_at(dock).cloned().unwrap_or_default();
                let mut dirty = false;
                if let Some(mut reserve) = state.train_reserve {
                    let amount = reserve
                        .water
                        .min(train_power::WATER_CAPACITY - car.steam.water);
                    car.steam.water += amount;
                    reserve.water -= amount;
                    state.train_reserve = (reserve.water > 0.0).then_some(reserve);
                    dirty = amount > 0.0;
                }
                if state.water.amount > 0
                    && car.steam.water + WATER_PER_UNIT <= train_power::WATER_CAPACITY
                {
                    state.water.amount -= 1;
                    state.water.flow = [0; 2];
                    state.flowing = true;
                    car.steam.water += WATER_PER_UNIT;
                    dirty = true;
                }
                if dirty {
                    journal.set_plumbing(dock, state)?;
                }
            }
            // Existing nozzles and open pipe mouths can also fill the real tank directly.
            if car.steam.water + WATER_PER_UNIT <= train_power::WATER_CAPACITY
                && car.steam.water <= before.water
            {
                'tanks: for (index, _part) in car
                    .parts
                    .iter()
                    .enumerate()
                    .filter(|(_, part)| part.material == 254)
                {
                    let Some(center) = car.world_cell(index) else {
                        continue;
                    };
                    for offset in plumbing::DIRECTIONS.into_iter().chain([[0, 2, 0]]) {
                        let inlet = std::array::from_fn(|i| center[i] + offset[i]);
                        let Some(material) = chunks.block_at(inlet) else {
                            continue;
                        };
                        if !usable(chunks, journal, &car, inlet) {
                            continue;
                        }
                        let supplied = if material == plumbing::FLUID_INFUSER && offset[1] > 0 {
                            crate::plumbing::take_infuser(chunks, journal, inlet, 1)?
                        } else if plumbing::is_pipe(material)
                            && offset[1].abs() <= 1
                            && journal.object_state(inlet).is_some_and(|pose| {
                                plumbing::direction_index(offset.map(|v| -v)).is_some_and(|side| {
                                    crate::plumbing::connection_mask(
                                        journal,
                                        inlet,
                                        material,
                                        pose.facing,
                                        |cell| chunks.block_at(cell),
                                    ) & (1 << side)
                                        != 0
                                })
                            })
                        {
                            if let Some(mut state) = journal
                                .plumbing_at(inlet)
                                .cloned()
                                .filter(|state| state.water.amount > 0)
                            {
                                state.water.amount -= 1;
                                state.water.flow = [0; 2];
                                state.flowing = true;
                                journal.set_plumbing(inlet, state)?;
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        if supplied {
                            car.steam.water += WATER_PER_UNIT;
                            break 'tanks;
                        }
                    }
                }
            }
        }
        if car.steam != before {
            publish(journal, car);
            changed += 1;
        }
    }
    // A real coupled tender is a finite supply for the locomotive, not a second free tank.
    let links = serviced
        .into_iter()
        .filter(|id| {
            let car = &journal.gameplay().trains.vehicles[id];
            car.kind == terra_core::trains::VehicleKind::Locomotive
                && car.parts.iter().any(|part| part.material == 254)
        })
        .collect::<Vec<_>>();
    for id in links {
        let Ok(group) = journal.gameplay().trains.consist(id) else {
            continue;
        };
        if group.iter().any(|id| {
            let car = &journal.gameplay().trains.vehicles[id];
            car.couplings.iter().enumerate().any(|(index, link)| {
                link.is_some_and(|link| {
                    let other = &journal.gameplay().trains.vehicles[&link.vehicle];
                    car.coupling_distance(
                        if index == 0 {
                            terra_core::trains::CouplerEnd::Front
                        } else {
                            terra_core::trains::CouplerEnd::Rear
                        },
                        other,
                        link.end,
                    )
                    .is_none()
                })
            })
        }) {
            continue;
        }
        let mut locomotive = journal.gameplay().trains.vehicles[&id].clone();
        if !loaded(chunks, &locomotive) || !can_publish(journal, &locomotive) {
            continue;
        }
        let Some(donor) = group
            .iter()
            .filter_map(|other| journal.gameplay().trains.vehicles.get(other))
            .find(|other| {
                other.kind == terra_core::trains::VehicleKind::Tender
                    && other.owner == locomotive.owner
                    && other.steam.water > 0.0
                    && loaded(chunks, other)
                    && usable(
                        chunks,
                        journal,
                        &locomotive,
                        other.model_origin().map(|v| v.floor() as i64),
                    )
                    && can_publish(journal, other)
            })
        else {
            continue;
        };
        let mut donor = donor.clone();
        let amount = donor
            .steam
            .water
            .min(WATER_PER_UNIT)
            .min(train_power::WATER_CAPACITY - locomotive.steam.water);
        if amount <= 0.0 {
            continue;
        }
        donor.steam.water -= amount;
        locomotive.steam.water += amount;
        publish(journal, donor);
        publish(journal, locomotive);
        changed += 2;
    }
    Ok(changed)
}
