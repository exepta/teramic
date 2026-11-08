//! Bounded rail-following motion. Missing track, unloaded cells and collisions fail closed.
use crate::{edits::EditJournal, rails, streaming::ChunkStore};
use terra_core::rails::{Endpoint, Path, RouteNetwork, Sample};
use terra_core::{
    train_motion::{RailCursor, StopReason},
    trains::Vehicle,
};

fn path(network: &RouteNetwork, cursor: RailCursor) -> Option<Path> {
    network
        .get(cursor.anchor)?
        .active_paths()
        .into_iter()
        .find(|path| path.index == cursor.path)
}
fn loaded(chunks: &ChunkStore, journal: &EditJournal, path: Path) -> bool {
    rails::complete(journal, path.piece)
        && rails::cells(path.piece)
            .into_iter()
            .all(|cell| chunks.block_at(cell) == Some(path.piece.material))
}
fn sample(path: Path, cursor: RailCursor) -> Sample {
    let mut point = path.sample_distance(cursor.distance);
    if !cursor.forward {
        point.tangent = point.tangent.map(|v| -v);
    }
    point
}
/// Follow active endpoints for at most 64 modules, including reverse traversal.
pub fn walk(
    chunks: &ChunkStore,
    journal: &EditJournal,
    network: &RouteNetwork,
    mut cursor: RailCursor,
    meters: f64,
) -> Result<(RailCursor, Sample), StopReason> {
    if !meters.is_finite() {
        return Err(StopReason::InvalidTrack);
    }
    let direction = if meters < 0.0 { -1.0 } else { 1.0 };
    let mut remaining = meters.abs();
    for _ in 0..64 {
        let path = path(network, cursor).ok_or(StopReason::InvalidTrack)?;
        if !loaded(chunks, journal, path) {
            return Err(StopReason::Unloaded);
        }
        let increasing = cursor.forward == (direction > 0.0);
        let available = if increasing {
            path.length() - cursor.distance
        } else {
            cursor.distance
        };
        if remaining <= available + 1e-9 {
            cursor.distance = (cursor.distance + remaining * if increasing { 1.0 } else { -1.0 })
                .clamp(0.0, path.length());
            return Ok((cursor, sample(path, cursor)));
        }
        remaining -= available.max(0.0);
        let endpoint = Endpoint {
            anchor: cursor.anchor,
            port: path.ports[usize::from(increasing)],
        };
        let connection = network
            .connected_endpoint(endpoint)
            .ok_or(StopReason::EndOfTrack)?;
        let next = network
            .get(connection.anchor)
            .ok_or(StopReason::EndOfTrack)?
            .active_paths()
            .into_iter()
            .find(|path| path.ports.contains(&connection.port))
            .ok_or(StopReason::InvalidTrack)?;
        let at_start = next.ports[0] == connection.port;
        cursor = RailCursor {
            anchor: connection.anchor,
            path: next.index,
            distance: if at_start { 0.0 } else { next.length() },
            forward: at_start == (direction > 0.0),
        };
    }
    Err(StopReason::InvalidTrack)
}
fn initialize(
    chunks: &ChunkStore,
    journal: &EditJournal,
    network: &RouteNetwork,
    car: &Vehicle,
) -> Result<(RailCursor, [f64; 3], f64), StopReason> {
    let facing = car.build_yaw();
    let forward = [facing.sin(), 0.0, facing.cos()];
    let mut bogies = car
        .parts
        .iter()
        .enumerate()
        .filter(|(_, p)| p.material == terra_core::train_parts::BOGIE)
        .collect::<Vec<_>>();
    bogies.sort_by(|(_, a), (_, b)| {
        let score = |p: &terra_core::trains::TrainPart| {
            f64::from(p.offset[0]) * forward[0] + f64::from(p.offset[2]) * forward[2]
        };
        score(a).total_cmp(&score(b))
    });
    if bogies.len() < 2 {
        return Err(StopReason::InvalidTrack);
    }
    let (_, back) = bogies[0];
    let (_, front) = bogies[bogies.len() - 1];
    let wheelbase = ((f64::from(front.offset[0] - back.offset[0])).powi(2)
        + (f64::from(front.offset[2] - back.offset[2])).powi(2))
    .sqrt();
    if !(3.0..=15.0).contains(&wheelbase) {
        return Err(StopReason::InvalidTrack);
    }
    let pivot = [
        f64::from(front.offset[0]) + 0.5,
        f64::from(front.offset[1]) - 1.0 + terra_core::rails::DECK_HEIGHT
            - terra_core::trains::MODEL_Y_OFFSET,
        f64::from(front.offset[2]) + 0.5,
    ];
    if let Some(cursor) = car.motion.cursor {
        let current = path(network, cursor).ok_or(StopReason::InvalidTrack)?;
        if !loaded(chunks, journal, current) {
            return Err(StopReason::Unloaded);
        }
        return Ok((cursor, pivot, wheelbase));
    }
    let origin = car.world_point(pivot);
    let mut candidates = std::collections::BTreeMap::new();
    let cell = origin.map(|v| v.floor() as i64);
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                if let Some(piece) =
                    rails::owner_at(journal, [cell[0] + x, cell[1] + y, cell[2] + z])
                {
                    candidates.insert(piece.anchor, piece);
                }
            }
        }
    }
    let mut best: Option<(f64, RailCursor)> = None;
    for piece in candidates.into_values() {
        for path in piece.active_paths() {
            if !loaded(chunks, journal, path) {
                continue;
            }
            for step in 0..=128 {
                let distance = path.length() * f64::from(step) / 128.0;
                let point = path.sample_distance(distance);
                let error = (0..3)
                    .map(|i| (point.position[i] - origin[i]).powi(2))
                    .sum::<f64>();
                if error <= 0.75 * 0.75 && best.as_ref().is_none_or(|(old, _)| error < *old) {
                    let aligned =
                        point.tangent[0] * forward[0] + point.tangent[2] * forward[2] >= 0.0;
                    best = Some((
                        error,
                        RailCursor {
                            anchor: piece.anchor,
                            path: path.index,
                            distance,
                            forward: aligned,
                        },
                    ));
                }
            }
        }
    }
    let cursor = car
        .motion
        .cursor
        .or(best.map(|(_, cursor)| cursor))
        .ok_or(StopReason::InvalidTrack)?;
    Ok((cursor, pivot, wheelbase))
}
/// Locate the trailing bogie by rigid chord distance, not by arc length.
fn trailing(
    chunks: &ChunkStore,
    journal: &EditJournal,
    network: &RouteNetwork,
    cursor: RailCursor,
    front: Sample,
    wheelbase: f64,
) -> Result<Sample, StopReason> {
    let distance = |point: Sample| {
        (0..3)
            .map(|i| (point.position[i] - front.position[i]).powi(2))
            .sum::<f64>()
            .sqrt()
    };
    let mut low = wheelbase;
    let mut high = wheelbase;
    let mut point = walk(chunks, journal, network, cursor, -high)?.1;
    if (distance(point) - wheelbase).abs() < 1e-6 {
        return Ok(point);
    }
    for _ in 0..32 {
        high += 0.5;
        point = walk(chunks, journal, network, cursor, -high)?.1;
        if distance(point) >= wheelbase {
            break;
        }
    }
    if distance(point) < wheelbase {
        return Err(StopReason::InvalidTrack);
    }
    for _ in 0..12 {
        let middle = (low + high) * 0.5;
        point = walk(chunks, journal, network, cursor, -middle)?.1;
        if distance(point) < wheelbase {
            low = middle;
        } else {
            high = middle;
        }
    }
    Ok(walk(chunks, journal, network, cursor, -((low + high) * 0.5))?.1)
}
fn set_pose(car: &mut Vehicle, pivot: [f64; 3], front: Sample, back: Sample) {
    let d = std::array::from_fn::<_, 3, _>(|i| front.position[i] - back.position[i]);
    let heading = d[0].atan2(d[2]);
    car.motion.yaw = (heading - car.build_yaw() + std::f64::consts::PI)
        .rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI;
    car.motion.pitch = -d[1].atan2(d[0].hypot(d[2]));
    car.motion.pivot = pivot;
    let base = std::array::from_fn::<_, 3, _>(|i| {
        car.anchor[i] as f64
            + if i == 1 {
                terra_core::trains::MODEL_Y_OFFSET
            } else {
                0.0
            }
    });
    car.motion.translation = std::array::from_fn(|i| front.position[i] - base[i] - pivot[i]);
}
fn continuous_contacts(
    old: &Vehicle,
    next: &Vehicle,
    pivot: [f64; 3],
    wheelbase: f64,
    meters: f64,
) -> Result<(), StopReason> {
    if old.motion.cursor.is_none() {
        return Ok(());
    }
    let mut rear = pivot;
    rear[0] -= old.build_yaw().sin() * wheelbase;
    rear[2] -= old.build_yaw().cos() * wheelbase;
    for local in [pivot, rear] {
        let a = old.world_point(local);
        let b = next.world_point(local);
        let distance = (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt();
        if distance > meters.abs() * 3.0 + 0.05 {
            return Err(StopReason::InvalidTrack);
        }
    }
    Ok(())
}
/// Consume one actual item only when its previous burn budget is exhausted.
fn burn(car: &mut Vehicle, dt: f64) -> bool {
    if !car.parts.iter().any(|p| p.material == 218) {
        car.motion.stop_reason = StopReason::NoEngine;
        return false;
    }
    if car.burn_seconds <= 0.0 {
        for (index, inventory) in &mut car.cargo {
            if car.parts[usize::from(*index)].material != 218 {
                continue;
            }
            if let Some((slot, stack, seconds)) = inventory
                .slots
                .iter()
                .copied()
                .enumerate()
                .find_map(|(slot, stack)| {
                    (stack.count > 0)
                        .then(|| {
                            terra_core::furnace::fuel_seconds(stack.item)
                                .map(|seconds| (slot, stack, seconds))
                        })
                        .flatten()
                })
                && inventory.take_from(slot, stack.item)
            {
                car.burn_seconds = f64::from(seconds);
                break;
            }
        }
    }
    let powered = car.burn_seconds > 0.0;
    if powered {
        car.burn_seconds = (car.burn_seconds - dt).max(0.0);
    }
    powered
}
pub fn fuel_available(car: &Vehicle) -> bool {
    car.burn_seconds > 0.0
        || car.cargo.iter().any(|(index, inventory)| {
            car.parts[usize::from(*index)].material == 218
                && inventory.slots.iter().any(|stack| {
                    stack.count > 0 && terra_core::furnace::fuel_seconds(stack.item).is_some()
                })
        })
}
/// Advance one rigid car. The caller checks the whole consist before publication.
pub fn advance(
    chunks: &ChunkStore,
    journal: &EditJournal,
    network: &RouteNetwork,
    car: &Vehicle,
    dt: f64,
) -> Result<Vehicle, StopReason> {
    let mut next = car.clone();
    let (cursor, pivot, wheelbase) = initialize(chunks, journal, network, car)?;
    let limit = rails::ballast::speed_limit(chunks, journal, cursor.anchor, 12.0)
        .ok_or(StopReason::InvalidTrack)?;
    let powered = if car.motion.throttle != 0 && !car.motion.brake {
        burn(&mut next, dt)
    } else {
        fuel_available(car)
    };
    next.motion.fuel_available = fuel_available(&next);
    let speed = terra_core::train_motion::speed_step(
        car.motion.speed_mps,
        car.motion.throttle,
        car.motion.brake,
        powered,
        limit,
        dt,
    );
    let meters = (car.motion.speed_mps + speed) * 0.5 * dt;
    // Look ahead by the stopping distance. Known end points start braking before impact.
    let stopping = (speed * speed / 12.0 + 0.25).min(32.0);
    if speed.abs() > 0.01
        && walk(chunks, journal, network, cursor, speed.signum() * stopping).is_err()
    {
        next.motion.speed_mps =
            terra_core::train_motion::speed_step(car.motion.speed_mps, 0, true, powered, limit, dt);
        next.motion.brake = true;
        next.motion.stop_reason = StopReason::EndOfTrack;
    } else {
        next.motion.speed_mps = speed;
        next.motion.stop_reason = if car.motion.brake {
            StopReason::Braking
        } else if !powered {
            if car.parts.iter().any(|p| p.material == 218) {
                StopReason::NoFuel
            } else {
                StopReason::NoEngine
            }
        } else {
            StopReason::Ready
        };
    }
    let meters = if next.motion.brake {
        (car.motion.speed_mps + next.motion.speed_mps) * 0.5 * dt
    } else {
        meters
    };
    let (cursor, front) = walk(chunks, journal, network, cursor, meters)?;
    let back = trailing(chunks, journal, network, cursor, front, wheelbase)?;
    set_pose(&mut next, pivot, front, back);
    continuous_contacts(car, &next, pivot, wheelbase, meters)?;
    next.motion.cursor = Some(cursor);
    next.motion.wheel_distance_m += meters;
    Ok(next)
}
/// Other cars follow the same rail route with their own wheelbase and rigid pose.
pub fn follow(
    chunks: &ChunkStore,
    journal: &EditJournal,
    network: &RouteNetwork,
    car: &Vehicle,
    meters: f64,
    speed: f64,
) -> Result<Vehicle, StopReason> {
    let (cursor, pivot, wheelbase) = initialize(chunks, journal, network, car)?;
    let (cursor, front) = walk(chunks, journal, network, cursor, meters)?;
    let back = trailing(chunks, journal, network, cursor, front, wheelbase)?;
    let mut next = car.clone();
    set_pose(&mut next, pivot, front, back);
    continuous_contacts(car, &next, pivot, wheelbase, meters)?;
    next.motion.cursor = Some(cursor);
    next.motion.speed_mps = speed;
    next.motion.wheel_distance_m += meters;
    next.motion.brake = false;
    Ok(next)
}
/// A finite voxel probe budget prevents a decorated train from blocking the server thread.
pub fn clear(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    car: &Vehicle,
) -> Result<(), StopReason> {
    let mut probes = 0;
    for index in 0..car.parts.len() {
        for shape in car.transformed_shapes(index) {
            clear_shape(chunks, journal, properties, shape, &mut probes)?;
        }
    }
    Ok(())
}
/// Bound every local shape corner relative to the actual bogie rotation pivot.
fn rotation_radius(
    part: &terra_core::trains::TrainPart,
    shape: terra_core::block_shapes::ShapeBox,
    pivot: [f64; 3],
) -> f64 {
    (0..3)
        .map(|axis| {
            let offset = f64::from(part.offset[axis]) - pivot[axis];
            (offset + shape.minimum[axis])
                .abs()
                .max((offset + shape.maximum[axis]).abs())
                .powi(2)
        })
        .sum::<f64>()
        .sqrt()
}
/// Validate the full swept bounds, including thin obstacles between two poses.
/// Rotation padding bounds the second derivative of the two Euler rotations.
pub fn clear_swept(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    old: &Vehicle,
    next: &Vehicle,
) -> Result<Vec<terra_core::block_shapes::ShapeBox>, StopReason> {
    let yaw = (next.motion.yaw - old.motion.yaw + std::f64::consts::PI)
        .rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI;
    let angle = yaw.abs() + (next.motion.pitch - old.motion.pitch).abs();
    let mut probes = 0;
    let mut swept = Vec::new();
    for index in 0..next.parts.len() {
        let before = old.transformed_shapes(index);
        let after = next.transformed_shapes(index);
        let local = next.part_shapes(index);
        for ((before, after), local) in before.into_iter().zip(after).zip(local) {
            let radius = rotation_radius(&next.parts[index], local, next.motion.pivot);
            let padding = radius * angle * angle / 8.0;
            let shape = terra_core::block_shapes::ShapeBox {
                minimum: std::array::from_fn(|i| before.minimum[i].min(after.minimum[i]) - padding),
                maximum: std::array::from_fn(|i| before.maximum[i].max(after.maximum[i]) + padding),
                hinged: false,
            };
            clear_shape(chunks, journal, properties, shape, &mut probes)?;
            swept.push(shape);
        }
    }
    Ok(swept)
}
/// Cars in a consist may touch through couplers but cannot pass through bodies.
pub fn consist_clear(cars: &[Vehicle]) -> Result<(), StopReason> {
    if cars.len() > terra_core::trains::MAX_CONSIST {
        return Err(StopReason::InvalidTrack);
    }
    let bounds = cars.iter().map(Vehicle::model_bounds).collect::<Vec<_>>();
    type PartShapes = Vec<(u16, terra_core::block_shapes::ShapeBox)>;
    let mut shapes: Vec<Option<PartShapes>> = vec![None; cars.len()];
    let mut probes = 0;
    let intersects = |a: [[f64; 3]; 2], b: [[f64; 3]; 2]| {
        (0..3).all(|axis| a[1][axis].min(b[1][axis]) - a[0][axis].max(b[0][axis]) > 1e-6)
    };
    for first in 0..cars.len() {
        for second in first + 1..cars.len() {
            if !intersects(bounds[first], bounds[second]) {
                continue;
            }
            for index in [first, second] {
                if shapes[index].is_none() {
                    shapes[index] = Some(
                        cars[index]
                            .parts
                            .iter()
                            .enumerate()
                            .flat_map(|(part_index, part)| {
                                cars[index]
                                    .transformed_shapes(part_index)
                                    .into_iter()
                                    .map(move |shape| (part.material, shape))
                            })
                            .collect(),
                    );
                }
            }
            for (material_a, a) in shapes[first].as_ref().unwrap() {
                for (material_b, b) in shapes[second].as_ref().unwrap() {
                    probes += 1;
                    if probes > 131_072 {
                        return Err(StopReason::Obstructed);
                    }
                    if (*material_a != terra_core::train_parts::COUPLER
                        || *material_b != terra_core::train_parts::COUPLER)
                        && intersects([a.minimum, a.maximum], [b.minimum, b.maximum])
                    {
                        return Err(StopReason::Obstructed);
                    }
                }
            }
        }
    }
    Ok(())
}
fn clear_shape(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    shape: terra_core::block_shapes::ShapeBox,
    probes: &mut usize,
) -> Result<(), StopReason> {
    for x in shape.minimum[0].floor() as i64..=(shape.maximum[0] - 1e-7).floor() as i64 {
        for y in shape.minimum[1].floor() as i64..=(shape.maximum[1] - 1e-7).floor() as i64 {
            for z in shape.minimum[2].floor() as i64..=(shape.maximum[2] - 1e-7).floor() as i64 {
                *probes += 1;
                if *probes > 16_384 {
                    return Err(StopReason::Obstructed);
                }
                let cell = [x, y, z];
                let Some(material) = chunks.block_at(cell) else {
                    return Err(StopReason::Unloaded);
                };
                if material == 0 || material == 27 || terra_core::rails::is_rail(material) {
                    continue;
                }
                if crate::interaction_shapes::space_intersects(
                    chunks,
                    journal,
                    properties,
                    cell,
                    shape.minimum,
                    shape.maximum,
                ) {
                    return Err(StopReason::Obstructed);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::LoadedSection;
    use terra_core::{
        Inventory, ItemKind, ItemStack, PlayerUuid, train_blueprint, train_parts,
        trains::{TrainPart, VehicleKind},
    };
    use terra_voxel::{BlockSection, global_to_section};
    fn set(chunks: &mut ChunkStore, cell: [i64; 3], material: u16) {
        let (owner, local) = global_to_section(cell);
        chunks
            .loaded
            .entry(owner)
            .or_insert_with(|| LoadedSection {
                data: BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            })
            .data
            .set_block(local, material)
            .unwrap();
    }
    fn fixture() -> (ChunkStore, EditJournal, Vehicle) {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for x in -3..=3 {
            for y in -2..=6 {
                for z in -4..=40 {
                    set(&mut chunks, [x, y, z], 0);
                }
            }
        }
        for z in (0..=32).step_by(4) {
            let piece =
                terra_core::rails::Piece::new([0, 0, z], terra_core::rails::STRAIGHT, 0).unwrap();
            for support in piece.supports().iter() {
                set(
                    &mut chunks,
                    std::array::from_fn(|i| piece.anchor[i] + i64::from(support.support[i])),
                    3,
                );
            }
            let plan = rails::plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
            rails::commit(&mut chunks, &mut journal, &plan).unwrap();
        }
        let mut parts = train_blueprint::template([0, 2, 2], 0, VehicleKind::Locomotive)
            .into_iter()
            .map(|part| TrainPart {
                offset: part.cell.map(|v| v as i16),
                material: part.material,
                facing: part.facing,
                wood: Default::default(),
            })
            .collect::<Vec<_>>();
        // The guide may already provide a fuel engine; replace only a cargo cell if needed.
        let engine = parts
            .iter()
            .position(|p| p.material == 218)
            .unwrap_or_else(|| {
                let index = parts
                    .iter()
                    .position(|p| p.material == train_parts::CARGO)
                    .unwrap();
                parts[index].material = 218;
                index
            });
        let mut cargo = parts
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.material, 29 | 216 | 218))
            .map(|(i, _)| (i as u16, Inventory::default()))
            .collect::<std::collections::BTreeMap<_, _>>();
        cargo.insert(
            engine as u16,
            Inventory {
                slots: vec![ItemStack {
                    item: ItemKind::Block(18),
                    count: 2,
                }],
                ..Default::default()
            },
        );
        let car = Vehicle {
            id: 1,
            owner: PlayerUuid::from_u128(1),
            kind: VehicleKind::Locomotive,
            anchor: [0; 3],
            parts,
            cargo,
            blocks: Default::default(),
            payloads: Default::default(),
            couplings: [None; 2],
            revision: 1,
            motion: Default::default(),
            burn_seconds: 0.0,
        };
        (chunks, journal, car)
    }
    #[test]
    fn fueled_train_consumes_one_real_item_moves_and_brakes_on_the_same_rails() {
        let (chunks, journal, mut car) = fixture();
        let network = rails::network(&journal).unwrap();
        car.motion.throttle = 3;
        car.motion.brake = false;
        for _ in 0..120 {
            car = advance(&chunks, &journal, &network, &car, 1.0 / 60.0).unwrap();
        }
        assert!(car.motion.speed_mps > 2.0);
        assert!(car.motion.translation[2] > 1.0);
        assert_eq!(
            car.cargo
                .values()
                .map(|inv| u32::from(inv.count(ItemKind::Block(18))))
                .sum::<u32>(),
            1
        );
        assert!(car.burn_seconds > 0.0);
        car.motion.brake = true;
        for _ in 0..60 {
            car = advance(&chunks, &journal, &network, &car, 1.0 / 60.0).unwrap();
        }
        assert!(car.motion.stopped());
        let saved = serde_json::to_vec(&car).unwrap();
        let loaded: Vehicle = serde_json::from_slice(&saved).unwrap();
        assert_eq!(loaded, car);
    }
    #[test]
    fn empty_engine_and_missing_loaded_track_cannot_create_motion_or_consume_inventory() {
        let (mut chunks, journal, mut car) = fixture();
        let network = rails::network(&journal).unwrap();
        car.motion.throttle = 3;
        car.motion.brake = false;
        car.cargo.values_mut().for_each(|inv| inv.slots.clear());
        let next = advance(&chunks, &journal, &network, &car, 0.05).unwrap();
        assert_eq!(next.motion.speed_mps, 0.0);
        assert_eq!(next.motion.stop_reason, StopReason::NoFuel);
        let owner = global_to_section([0, 0, 4]).0;
        chunks.loaded.remove(&owner);
        assert!(matches!(
            advance(&chunks, &journal, &network, &car, 0.05),
            Err(StopReason::Unloaded) | Err(StopReason::InvalidTrack)
        ));
    }
    #[test]
    fn cursor_crosses_real_module_endpoints_and_reverse_returns_to_the_start() {
        let (chunks, journal, car) = fixture();
        let network = rails::network(&journal).unwrap();
        let (cursor, _, _) = initialize(&chunks, &journal, &network, &car).unwrap();
        let (start, point) = walk(&chunks, &journal, &network, cursor, 0.0).unwrap();
        let (end, _) = walk(&chunks, &journal, &network, start, 12.5).unwrap();
        let (_, restored) = walk(&chunks, &journal, &network, end, -12.5).unwrap();
        assert!(
            point
                .position
                .iter()
                .zip(restored.position)
                .all(|(a, b)| (a - b).abs() < 1e-8)
        );
    }

    fn single_piece(piece: terra_core::rails::Piece) -> (ChunkStore, EditJournal) {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for cell in rails::cells(piece) {
            set(&mut chunks, cell, piece.material);
            journal.record(cell, piece.material).unwrap();
            journal
                .set_object_state(cell, Some(rails::object_state(piece, cell)))
                .unwrap();
        }
        (chunks, journal)
    }

    #[test]
    fn both_bogies_follow_a_real_free_curve_and_ramp_without_changing_the_rigid_wheelbase() {
        use terra_core::rails::{CurveSpec, Piece};
        let curve = CurveSpec::new([[8, 2, 0], [8, 2, 64], [40, 6, 128], [72, 10, 192]]).unwrap();
        let piece = Piece::new_curve([0; 3], curve).unwrap();
        let (chunks, journal) = single_piece(piece);
        let network = rails::network(&journal).unwrap();
        let (_, _, mut car) = fixture();
        car.motion.cursor = Some(RailCursor {
            anchor: piece.anchor,
            path: 0,
            distance: 8.0,
            forward: true,
        });
        let (initial_cursor, initial_pivot, wheelbase) =
            initialize(&chunks, &journal, &network, &car).unwrap();
        let front = walk(&chunks, &journal, &network, initial_cursor, 0.0)
            .unwrap()
            .1;
        let back = trailing(
            &chunks,
            &journal,
            &network,
            initial_cursor,
            front,
            wheelbase,
        )
        .unwrap();
        set_pose(&mut car, initial_pivot, front, back);
        let next = follow(&chunks, &journal, &network, &car, 1.0, 2.0).unwrap();
        let (cursor, pivot, wheelbase) = initialize(&chunks, &journal, &network, &next).unwrap();
        let front = walk(&chunks, &journal, &network, cursor, 0.0).unwrap().1;
        let back = trailing(&chunks, &journal, &network, cursor, front, wheelbase).unwrap();
        let world_front = next.world_point(pivot);
        let mut rear_pivot = pivot;
        rear_pivot[0] -= next.build_yaw().sin() * wheelbase;
        rear_pivot[2] -= next.build_yaw().cos() * wheelbase;
        let world_back = next.world_point(rear_pivot);
        for axis in 0..3 {
            assert!((world_front[axis] - front.position[axis]).abs() < 1e-8);
            assert!((world_back[axis] - back.position[axis]).abs() < 0.001);
        }
        assert!(next.motion.yaw > 0.05);
        assert!(next.motion.pitch < -0.01);
        assert_eq!(next.anchor, car.anchor);
        assert_eq!(next.parts, car.parts);
    }

    #[test]
    fn inactive_switch_branch_cannot_be_used_as_an_independent_route() {
        use terra_core::rails::{Piece, SWITCH_LEFT, SWITCH_RIGHT};
        for material in [SWITCH_LEFT, SWITCH_RIGHT] {
            for switched in [0, 4] {
                let piece = Piece::new([0; 3], material, switched).unwrap();
                let (chunks, journal) = single_piece(piece);
                let network = rails::network(&journal).unwrap();
                let active = piece.active_paths()[0];
                let cursor = RailCursor {
                    anchor: piece.anchor,
                    path: active.index,
                    distance: active.length() * 0.5,
                    forward: true,
                };
                assert!(walk(&chunks, &journal, &network, cursor, 0.5).is_ok());
                let inactive = piece
                    .paths()
                    .into_iter()
                    .find(|path| path.index != active.index)
                    .unwrap();
                assert_eq!(
                    walk(
                        &chunks,
                        &journal,
                        &network,
                        RailCursor {
                            path: inactive.index,
                            ..cursor
                        },
                        0.0
                    ),
                    Err(StopReason::InvalidTrack)
                );
            }
        }
    }

    #[test]
    fn a_stale_rear_route_or_saved_cursor_cannot_teleport_existing_rail_contacts() {
        let (chunks, journal, mut car) = fixture();
        let network = rails::network(&journal).unwrap();
        car.motion.cursor = Some(initialize(&chunks, &journal, &network, &car).unwrap().0);
        car.motion.translation[0] += 4.0;
        assert_eq!(
            follow(&chunks, &journal, &network, &car, 0.1, 1.0),
            Err(StopReason::InvalidTrack)
        );
        assert_eq!(car.motion.translation[0], 4.0);
    }

    #[test]
    fn swept_clearance_catches_a_thin_fence_between_otherwise_clear_endpoints() {
        use terra_core::{block_shapes::ShapeBox, trains::TrainBlockView};
        let (mut chunks, mut journal, mut car) = fixture();
        let index = car.parts.len() as u16;
        car.parts.push(TrainPart {
            offset: [1, 2, 3],
            material: 3,
            facing: 0,
            wood: Default::default(),
        });
        car.blocks.insert(
            index,
            TrainBlockView {
                boxes: vec![ShapeBox {
                    minimum: [0.0; 3],
                    maximum: [1.0; 3],
                    hinged: false,
                }],
                ..Default::default()
            },
        );
        set(&mut chunks, [1, 2, 4], 90);
        journal.record([1, 2, 4], 90).unwrap();
        let mut next = car.clone();
        next.motion.translation[2] += 1.8;
        let properties = Default::default();
        assert!(clear(&chunks, &journal, &properties, &car).is_ok());
        assert!(clear(&chunks, &journal, &properties, &next).is_ok());
        assert_eq!(
            clear_swept(&chunks, &journal, &properties, &car, &next),
            Err(StopReason::Obstructed)
        );
    }

    #[test]
    fn rotation_padding_uses_the_bogie_pivot_and_is_independent_of_the_model_origin() {
        let mut part = TrainPart {
            offset: [0, 1, 0],
            material: 3,
            facing: 0,
            wood: Default::default(),
        };
        let shape = terra_core::block_shapes::ShapeBox {
            minimum: [0.0; 3],
            maximum: [1.0; 3],
            hinged: false,
        };
        let pivot = [0.5, 1.0, 5.5];
        let radius = rotation_radius(&part, shape, pivot);
        assert!((radius - (0.5_f64.powi(2) + 1.0 + 5.5_f64.powi(2)).sqrt()).abs() < 1e-10);
        let shift = [7_i16, -2, 11];
        part.offset = std::array::from_fn(|i| part.offset[i] + shift[i]);
        let shifted_pivot = std::array::from_fn(|i| pivot[i] + f64::from(shift[i]));
        assert!((rotation_radius(&part, shape, shifted_pivot) - radius).abs() < 1e-10);
    }

    #[test]
    fn coupled_bodies_can_follow_forward_and_reverse_but_cannot_fold_through_each_other() {
        let (chunks, journal, first) = fixture();
        let network = rails::network(&journal).unwrap();
        let mut second = first.clone();
        second.id = 2;
        second.anchor[2] += 9;
        assert!(consist_clear(&[first.clone(), second.clone()]).is_ok());
        let next_a = follow(&chunks, &journal, &network, &first, 1.0, 1.0).unwrap();
        let next_b = follow(&chunks, &journal, &network, &second, 1.0, 1.0).unwrap();
        assert!(consist_clear(&[next_a.clone(), next_b.clone()]).is_ok());
        let back_a = follow(&chunks, &journal, &network, &next_a, -1.0, -1.0).unwrap();
        let back_b = follow(&chunks, &journal, &network, &next_b, -1.0, -1.0).unwrap();
        assert!(consist_clear(&[back_a, back_b]).is_ok());
        let curve = terra_core::rails::CurveSpec::new([
            [8, 2, 0],
            [8, 2, 128],
            [72, 6, 256],
            [136, 10, 384],
        ])
        .unwrap();
        let piece = terra_core::rails::Piece::new_curve([0; 3], curve).unwrap();
        let (curved_chunks, curved_journal) = single_piece(piece);
        let curved_network = rails::network(&curved_journal).unwrap();
        let mut curve_cars = [first.clone(), second.clone()];
        for (index, car) in curve_cars.iter_mut().enumerate() {
            car.motion.cursor = Some(RailCursor {
                anchor: piece.anchor,
                path: 0,
                distance: 8.0 + index as f64 * 9.0,
                forward: true,
            });
            let (cursor, pivot, wheelbase) =
                initialize(&curved_chunks, &curved_journal, &curved_network, car).unwrap();
            let front = walk(
                &curved_chunks,
                &curved_journal,
                &curved_network,
                cursor,
                0.0,
            )
            .unwrap()
            .1;
            let rear = trailing(
                &curved_chunks,
                &curved_journal,
                &curved_network,
                cursor,
                front,
                wheelbase,
            )
            .unwrap();
            set_pose(car, pivot, front, rear);
        }
        for meters in [0.2, -0.2] {
            for car in &mut curve_cars {
                *car = follow(
                    &curved_chunks,
                    &curved_journal,
                    &curved_network,
                    car,
                    meters,
                    meters,
                )
                .unwrap();
            }
            assert!(consist_clear(&curve_cars).is_ok());
        }
        second.motion.translation[2] = -8.0;
        assert_eq!(consist_clear(&[first, second]), Err(StopReason::Obstructed));
    }
}
