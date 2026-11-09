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
        && path.piece.footprint().iter().all(|offset| {
            let cell =
                std::array::from_fn(|axis| path.piece.anchor[axis] + i64::from(offset[axis]));
            chunks.block_at(cell) == Some(path.piece.material)
        })
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
            (if path.piece.material == terra_core::rails::BUFFER {
                3.25
            } else {
                path.length()
            }) - cursor.distance
        } else {
            cursor.distance
        };
        if remaining <= available + 1e-9 {
            cursor.distance = (cursor.distance + remaining * if increasing { 1.0 } else { -1.0 })
                .clamp(0.0, path.length());
            return Ok((cursor, sample(path, cursor)));
        }
        if path.piece.material == terra_core::rails::BUFFER && increasing {
            return Err(StopReason::Buffer);
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
/// Keep the leading body and coupler behind the physical buffer, not just its axle.
fn leading_clearance(car: &Vehicle, pivot: [f64; 3], point: Sample, direction: f64) -> f64 {
    let origin = car.world_point(pivot);
    let bounds = car.model_bounds();
    let forward = point.tangent.map(|value| value * direction);
    (0..8)
        .map(|corner| {
            (0..3)
                .map(|axis| {
                    (bounds[usize::from(corner & (1 << axis) != 0)][axis] - origin[axis])
                        * forward[axis]
                })
                .sum::<f64>()
        })
        .fold(0.0, f64::max)
        + 0.05
}
/// Geometry-only wheelset layout; rebuilding rails never changes the local pivots.
fn wheelsets(car: &Vehicle) -> Result<([f64; 3], f64), StopReason> {
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
    Ok((pivot, wheelbase))
}
fn initialize(
    chunks: &ChunkStore,
    journal: &EditJournal,
    network: &RouteNetwork,
    car: &Vehicle,
) -> Result<(RailCursor, [f64; 3], f64), StopReason> {
    let (pivot, wheelbase) = wheelsets(car)?;
    let heading = car.build_yaw() + car.motion.yaw;
    let world_forward = [heading.sin(), 0.0, heading.cos()];
    if let Some(cursor) = car.motion.cursor
        && let Some(current) = path(network, cursor)
        && loaded(chunks, journal, current)
        && (0.0..=current.length()).contains(&cursor.distance)
    {
        let expected = car.world_point(pivot);
        let actual = sample(current, cursor);
        let error: f64 = (0..3)
            .map(|axis| (expected[axis] - actual.position[axis]).powi(2))
            .sum();
        if error < 0.05 * 0.05 {
            return Ok((cursor, pivot, wheelbase));
        }
    }
    // A removed module leaves the physical pose intact. Rebind to replacement
    // rails at that pose rather than retaining the retired module forever.
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
                    let aligned = point.tangent[0] * world_forward[0]
                        + point.tangent[2] * world_forward[2]
                        >= 0.0;
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
    let cursor = best
        .map(|(_, cursor)| cursor)
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
    let nose = leading_clearance(
        car,
        pivot,
        sample(
            path(network, cursor).ok_or(StopReason::InvalidTrack)?,
            cursor,
        ),
        meters.signum(),
    );
    let buffer_ahead = walk(
        chunks,
        journal,
        network,
        cursor,
        speed.signum() * (stopping + nose),
    ) == Err(StopReason::Buffer);
    if speed.abs() > 0.01
        && (buffer_ahead
            || walk(chunks, journal, network, cursor, speed.signum() * stopping).is_err())
    {
        next.motion.speed_mps =
            terra_core::train_motion::speed_step(car.motion.speed_mps, 0, true, powered, limit, dt);
        next.motion.brake = true;
        next.motion.stop_reason = if buffer_ahead {
            StopReason::Buffer
        } else {
            StopReason::EndOfTrack
        };
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
    if meters.abs() > 1e-10
        && walk(
            chunks,
            journal,
            network,
            cursor,
            meters.signum() * (meters.abs() + nose),
        ) == Err(StopReason::Buffer)
    {
        return Err(StopReason::Buffer);
    }
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
    let nose = leading_clearance(
        car,
        pivot,
        sample(
            path(network, cursor).ok_or(StopReason::InvalidTrack)?,
            cursor,
        ),
        meters.signum(),
    );
    if meters.abs() > 1e-10
        && walk(
            chunks,
            journal,
            network,
            cursor,
            meters.signum() * (meters.abs() + nose),
        ) == Err(StopReason::Buffer)
    {
        return Err(StopReason::Buffer);
    }
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
/// Rotate an entirely supported, uncoupled parked vehicle and its platform together.
pub fn turntable(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    network: &RouteNetwork,
    car: &Vehicle,
    clockwise: bool,
) -> Result<
    (
        crate::placement::Layout,
        Vehicle,
        Vec<terra_core::block_shapes::ShapeBox>,
    ),
    terra_core::trains::TrainError,
> {
    use terra_core::trains::TrainError;
    if !car.motion.stopped() || !car.motion.brake || car.couplings.iter().any(Option::is_some) {
        return Err(TrainError::Invalid);
    }
    let (cursor, _, wheelbase) =
        initialize(chunks, journal, network, car).map_err(|_| TrainError::Unsupported)?;
    let piece = *network.get(cursor.anchor).ok_or(TrainError::Unsupported)?;
    if piece.material != terra_core::rails::TURNTABLE
        || cursor.distance < wheelbase
        || cursor.distance > 25.0
    {
        return Err(TrainError::Unsupported);
    }
    let center = [
        piece.anchor[0] as f64 + 0.5,
        piece.anchor[1] as f64,
        piece.anchor[2] as f64 + 0.5,
    ];
    for index in 0..car.parts.len() {
        for shape in car.transformed_shapes(index) {
            for x in [shape.minimum[0], shape.maximum[0]] {
                for z in [shape.minimum[2], shape.maximum[2]] {
                    if (x - center[0]).hypot(z - center[2]) > terra_core::rails::TURNTABLE_RADIUS {
                        return Err(TrainError::Unsupported);
                    }
                }
            }
        }
    }
    let plan = rails::turntable_plan(chunks, journal, piece.anchor, clockwise)
        .map_err(|_| TrainError::Unsupported)?;
    let original = car.world_point(car.motion.pivot);
    let mut previous = car.clone();
    let mut swept = Vec::new();
    for step in 1..=18 {
        let angle = f64::from(step) / 18.0
            * std::f64::consts::FRAC_PI_2
            * if clockwise { 1.0 } else { -1.0 };
        let (sin, cos) = angle.sin_cos();
        let delta = [original[0] - center[0], original[2] - center[2]];
        let target = [
            center[0] + delta[0] * cos + delta[1] * sin,
            original[1],
            center[2] - delta[0] * sin + delta[1] * cos,
        ];
        let mut next = car.clone();
        next.motion.yaw = (car.motion.yaw + angle + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        for axis in 0..3 {
            next.motion.translation[axis] += target[axis] - original[axis];
        }
        // The rotating platform, rather than the bogie, is the arc pivot.
        // Bound the sagitta between samples even for thin corner obstacles.
        let padding = terra_core::rails::TURNTABLE_RADIUS
            * (std::f64::consts::FRAC_PI_2 / 18.0).powi(2)
            / 8.0;
        let mut probes = 0;
        for index in 0..next.parts.len() {
            for (before, after) in previous
                .transformed_shapes(index)
                .into_iter()
                .zip(next.transformed_shapes(index))
            {
                let shape = terra_core::block_shapes::ShapeBox {
                    minimum: std::array::from_fn(|axis| {
                        before.minimum[axis].min(after.minimum[axis])
                            - if axis == 1 { 0.0 } else { padding }
                    }),
                    maximum: std::array::from_fn(|axis| {
                        before.maximum[axis].max(after.maximum[axis])
                            + if axis == 1 { 0.0 } else { padding }
                    }),
                    hinged: false,
                };
                clear_shape(chunks, journal, properties, shape, &mut probes)
                    .map_err(|_| TrainError::Occupied)?;
                swept.push(shape);
            }
        }
        previous = next;
    }
    previous.motion.throttle = 0;
    previous.motion.cursor = Some(cursor);
    previous.motion.stop_reason = StopReason::Braking;
    previous.revision = car.revision.checked_add(1).ok_or(TrainError::Overflow)?;
    Ok((plan, previous, swept))
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
            name: String::new(),
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
    fn powered_train_brakes_with_its_whole_body_before_the_buffer_and_can_reverse() {
        let (mut chunks, mut journal, mut car) = fixture();
        let piece =
            terra_core::rails::Piece::new([0, 0, 36], terra_core::rails::BUFFER, 0).unwrap();
        for support in piece.supports().iter() {
            set(
                &mut chunks,
                std::array::from_fn(|axis| piece.anchor[axis] + i64::from(support.support[axis])),
                3,
            );
        }
        let plan = rails::plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
        rails::commit(&mut chunks, &mut journal, &plan).unwrap();
        let network = rails::network(&journal).unwrap();
        car.motion.throttle = 3;
        car.motion.brake = false;
        let mut buffer_seen = false;
        for _ in 0..1800 {
            match advance(&chunks, &journal, &network, &car, 1.0 / 60.0) {
                Ok(next) => {
                    buffer_seen |= next.motion.stop_reason == StopReason::Buffer;
                    car = next;
                }
                Err(StopReason::Buffer) => {
                    buffer_seen = true;
                    car.motion.speed_mps = 0.0;
                    car.motion.brake = true;
                }
                Err(reason) => panic!("unexpected stop: {reason:?}"),
            }
            assert!(
                car.model_bounds()[1][2] <= 39.25 + 1e-8,
                "the body must not pass the stop beam"
            );
            if buffer_seen && car.motion.stopped() {
                break;
            }
        }
        assert!(buffer_seen && car.motion.stopped());
        let before = car.motion.wheel_distance_m;
        car.motion.throttle = -1;
        car.motion.brake = false;
        for _ in 0..60 {
            car = advance(&chunks, &journal, &network, &car, 1.0 / 60.0).unwrap();
        }
        assert!(car.motion.wheel_distance_m < before);
    }
    #[test]
    fn buffer_stops_before_the_physical_beam_but_allows_retreat_in_every_facing() {
        for facing in 0..4 {
            let piece =
                terra_core::rails::Piece::new([0; 3], terra_core::rails::BUFFER, facing).unwrap();
            let (chunks, journal) = single_piece(piece);
            let network = rails::network(&journal).unwrap();
            let cursor = RailCursor {
                anchor: piece.anchor,
                path: 0,
                distance: 3.2,
                forward: true,
            };
            assert_eq!(
                walk(&chunks, &journal, &network, cursor, 0.2).unwrap_err(),
                StopReason::Buffer
            );
            let back = walk(&chunks, &journal, &network, cursor, -0.5).unwrap();
            assert!((back.0.distance - 2.7).abs() < 1e-8);
            assert!(walk(&chunks, &journal, &network, cursor, 100.0).is_err());
        }
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
        let (initial_pivot, wheelbase) = wheelsets(&car).unwrap();
        let initial_cursor = car.motion.cursor.unwrap();
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
            let (pivot, wheelbase) = wheelsets(car).unwrap();
            let cursor = car.motion.cursor.unwrap();
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
    #[test]
    fn removed_native_track_rebinds_to_new_starters_without_moving_the_body() {
        let (mut chunks, mut journal, car) = fixture();
        let network = rails::network(&journal).unwrap();
        let car = advance(&chunks, &journal, &network, &car, 0.0).unwrap();
        let old_cursor = car.motion.cursor.unwrap();
        let old = network.get(old_cursor.anchor).unwrap();
        journal.gameplay_mut().trains.insert(car.clone()).unwrap();
        let plan = rails::break_layout(&chunks, &journal, old.anchor).unwrap();
        rails::commit(&mut chunks, &mut journal, &plan).unwrap();
        assert!(
            advance(
                &chunks,
                &journal,
                &rails::network(&journal).unwrap(),
                &car,
                0.0
            )
            .is_err()
        );
        // Repair must also pass the real placement reservation checks with
        // the assembled car still registered above the removed module.
        let repair = rails::plan(
            &chunks,
            &journal,
            old.anchor,
            old.material,
            old.encoded_facing(),
        )
        .unwrap();
        rails::commit(&mut chunks, &mut journal, &repair).unwrap();
        let remove_again = rails::break_layout(&chunks, &journal, old.anchor).unwrap();
        rails::commit(&mut chunks, &mut journal, &remove_again).unwrap();
        for z in 0..4 {
            let piece = terra_core::rails::Piece::seed(
                [old.anchor[0], old.anchor[1], old.anchor[2] + z],
                0,
            )
            .unwrap();
            for cell in rails::cells(piece) {
                set(&mut chunks, cell, piece.material);
                journal.record(cell, piece.material).unwrap();
                journal
                    .set_object_state(cell, Some(rails::object_state(piece, cell)))
                    .unwrap();
            }
        }
        let recovered = advance(
            &chunks,
            &journal,
            &rails::network(&journal).unwrap(),
            &car,
            0.0,
        )
        .unwrap();
        assert_ne!(recovered.motion.cursor.unwrap().anchor, old_cursor.anchor);
        for index in 0..car.parts.len() {
            let before = car.world_cell(index).unwrap();
            let after = recovered.world_cell(index).unwrap();
            assert_eq!(before, after, "repair must not teleport any vehicle part");
        }
        let mut driving = recovered;
        driving.motion.brake = false;
        driving.motion.throttle = 1;
        let moved = advance(
            &chunks,
            &journal,
            &rails::network(&journal).unwrap(),
            &driving,
            1.0 / 60.0,
        )
        .unwrap();
        assert!(moved.motion.speed_mps > 0.0);
    }

    #[test]
    fn turntable_rotates_vehicle_and_route_together_and_rejects_a_swept_obstacle() {
        let (mut chunks, mut journal, car) = fixture();
        let old: Vec<_> = rails::network(&journal)
            .unwrap()
            .records()
            .copied()
            .collect();
        for piece in old {
            let plan = rails::break_layout(&chunks, &journal, piece.anchor).unwrap();
            rails::commit(&mut chunks, &mut journal, &plan).unwrap();
        }
        for x in -15..=15 {
            for y in -2..=8 {
                for z in -15..=15 {
                    set(&mut chunks, [x, y, z], if y == -1 { 3 } else { 0 });
                }
            }
        }
        let table = terra_core::rails::Piece::new([0; 3], terra_core::rails::TURNTABLE, 0).unwrap();
        let plan = rails::plan(&chunks, &journal, table.anchor, table.material, 0).unwrap();
        rails::commit(&mut chunks, &mut journal, &plan).unwrap();
        let network = rails::network(&journal).unwrap();
        let car = advance(&chunks, &journal, &network, &car, 0.0).unwrap();
        let original = car.world_point([0.5, 2.0, 3.5]);
        let (plan, turned, swept) =
            turntable(&chunks, &journal, &Default::default(), &network, &car, true).unwrap();
        assert!(!swept.is_empty());
        rails::commit(&mut chunks, &mut journal, &plan).unwrap();
        let network = rails::network(&journal).unwrap();
        assert_eq!(network.get([0; 3]).unwrap().facing, 1);
        let turned = advance(&chunks, &journal, &network, &turned, 0.0).unwrap();
        let (plan, restored, _) = turntable(
            &chunks,
            &journal,
            &Default::default(),
            &network,
            &turned,
            false,
        )
        .unwrap();
        rails::commit(&mut chunks, &mut journal, &plan).unwrap();
        let restored_point = restored.world_point([0.5, 2.0, 3.5]);
        assert!((0..3).all(|axis| (original[axis] - restored_point[axis]).abs() < 1e-6));
        // An obstacle at the final cab position must prevent the whole action.
        let cab = turned
            .parts
            .iter()
            .position(|part| part.material == train_parts::CAB)
            .unwrap();
        let cell = turned.world_cell(cab).unwrap();
        set(&mut chunks, cell, 3);
        let before = journal.rail_epoch();
        assert!(
            turntable(
                &chunks,
                &journal,
                &Default::default(),
                &rails::network(&journal).unwrap(),
                &restored,
                true
            )
            .is_err()
        );
        assert_eq!(journal.rail_epoch(), before);
        let mut moving = restored;
        moving.motion.brake = false;
        assert!(
            turntable(
                &chunks,
                &journal,
                &Default::default(),
                &rails::network(&journal).unwrap(),
                &moving,
                true
            )
            .is_err()
        );
    }
}
