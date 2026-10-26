//! Swept gravity is independent of the stricter four-foot walking probe.
use super::{Creature, Habitat};

pub(super) fn settle(animal: &mut Creature, habitat: &Habitat, dt: f32) {
    if !animal.kind.is_aquatic()
        && !animal.kind.is_spider()
        && animal.kind != super::Kind::Bee
        && !habitat.clear_swimmer_rotated(animal.position, animal.size(), f64::from(animal.yaw))
        && recover_overlap(animal, habitat)
    {
        return;
    }
    if let Some(ground) =
        habitat.supported_sized(animal.position, animal.size(), f64::from(animal.yaw))
        && ground[1] <= animal.position[1] + 0.001
    {
        animal.position = ground;
        animal.velocity[1] = 0.0;
        return;
    }
    let velocity = (animal.velocity[1].min(0.0) - 12.0 * dt).max(-16.0);
    let distance = f64::from(-velocity * dt);
    let steps = (distance / 0.125).ceil().max(1.0) as usize;
    // The swimming probe deliberately starts 2 cm above the feet. Gravity
    // needs the actual lower face, otherwise edge contacts sink by that margin.
    let size = animal.size();
    let yaw = f64::from(animal.yaw);
    let clear = |p: [f64; 3]| {
        habitat.clear_swimmer_rotated([p[0], p[1] - 0.02 + 0.000001, p[2]], size, yaw)
    };
    for _ in 0..steps {
        let mut next = animal.position;
        next[1] -= distance / steps as f64;
        if clear(next) {
            animal.position = next;
        } else {
            if !clear(animal.position) {
                // An already penetrated side contact has no valid upper
                // endpoint for a floor sweep. Keep falling intent instead of
                // turning it into a static overlap that climbs onto the ledge.
                animal.velocity[1] = velocity;
                return;
            }
            // Resolve the last fraction instead of leaving the body hovering
            // above a floor that the next complete step would intersect.
            let (mut low, mut high) = (next[1], animal.position[1]);
            for _ in 0..12 {
                next[1] = (low + high) * 0.5;
                if clear(next) {
                    high = next[1];
                } else {
                    low = next[1];
                }
            }
            animal.position[1] = if (high - high.round()).abs() < 0.0001 {
                high.round()
            } else {
                high
            };
            animal.velocity[1] = 0.0;
            return;
        }
        if habitat.wet(animal.position) {
            break;
        }
    }
    animal.velocity[1] = velocity;
}

fn body_bounds(position: [f64; 3], size: [f64; 3], yaw: f64) -> ([f64; 3], [f64; 3]) {
    let (sin, cos) = yaw.sin_cos();
    let x = (size[0] * cos.abs() + size[2] * sin.abs()) * 0.5;
    let z = (size[0] * sin.abs() + size[2] * cos.abs()) * 0.5;
    (
        [position[0] - x, position[1] + 0.02, position[2] - z],
        [
            position[0] + x - 0.000001,
            position[1] + size[1] - 0.02,
            position[2] + z - 0.000001,
        ],
    )
}

fn overlapping_shapes(
    habitat: &Habitat,
    position: [f64; 3],
    size: [f64; 3],
    yaw: f64,
) -> Option<Vec<terra_core::block_shapes::ShapeBox>> {
    let (lower, upper) = body_bounds(position, size, yaw);
    let min = lower.map(|v| v.floor() as i64);
    let max = upper.map(|v| v.floor() as i64);
    if (0..3).map(|i| max[i] - min[i] + 1).product::<i64>() > 256 {
        return None;
    }
    let mut shapes = Vec::new();
    for x in min[0]..=max[0] {
        for y in min[1]..=max[1] {
            for z in min[2]..=max[2] {
                let cell = [x, y, z];
                habitat.block(cell)?;
                if !habitat.solid(cell) {
                    continue;
                }
                let mut collect = |shape| {
                    if Habitat::shape_obstructs(shape, lower, upper, (position, size, yaw)) {
                        shapes.push(shape);
                    }
                };
                if !habitat.boxes(cell, &mut collect) {
                    collect(terra_core::block_shapes::ShapeBox {
                        minimum: cell.map(|v| v as f64),
                        maximum: cell.map(|v| v as f64 + 1.0),
                        hinged: false,
                    });
                }
                if shapes.len() > 16 {
                    return None;
                }
            }
        }
    }
    Some(shapes)
}

fn overlap_volume(
    shape: terra_core::block_shapes::ShapeBox,
    lower: [f64; 3],
    upper: [f64; 3],
) -> f64 {
    (0..3)
        .map(|i| (upper[i].min(shape.maximum[i]) - lower[i].max(shape.minimum[i])).max(0.0))
        .product()
}

/// Only remove a genuine local penetration. Every sampled pose remains loaded,
/// cannot acquire a new collision, and monotonically leaves the initial shapes.
/// Encased bodies and unknown sections fail closed instead of crossing a wall.
fn recover_overlap(animal: &mut Creature, habitat: &Habitat) -> bool {
    let start = animal.position;
    let size = animal.size();
    let yaw = f64::from(animal.yaw);
    let falling = animal.velocity[1] < -0.001;
    let Some(initial) = overlapping_shapes(habitat, start, size, yaw).filter(|s| !s.is_empty())
    else {
        return false;
    };
    let (lower, upper) = body_bounds(start, size, yaw);
    let mut candidates = Vec::with_capacity(initial.len() * 6);
    for shape in &initial {
        for axis in 0..3 {
            if falling && axis == 1 {
                continue;
            }
            let positive = if axis == 1 {
                shape.maximum[1] - start[1] + 0.00001
            } else {
                shape.maximum[axis] - lower[axis] + 0.00001
            };
            for delta in [shape.minimum[axis] - upper[axis] - 0.00001, positive] {
                if delta.abs() > 1.001 {
                    continue;
                }
                let mut probe = start;
                probe[axis] += delta;
                let next = habitat
                    .supported_sized(probe, size, yaw)
                    .filter(|next| !falling || next[1] <= start[1] + 0.001)
                    .or_else(|| {
                        (habitat.clear_swimmer_rotated(probe, size, yaw)
                            && (falling || habitat.wet_footprint(probe, size, yaw)))
                        .then_some(probe)
                    });
                if let Some(next) = next
                    && super::distance(start, next) <= 1.001_f64.powi(2)
                    && habitat.clear_swimmer_rotated(next, size, yaw)
                {
                    candidates.push(next);
                }
            }
        }
    }
    candidates
        .sort_unstable_by(|a, b| super::distance(start, *a).total_cmp(&super::distance(start, *b)));
    candidates.dedup();
    for next in candidates.into_iter().take(24) {
        let steps = (super::distance(start, next).sqrt() / 0.05).ceil().max(1.0) as usize;
        let mut volumes: Vec<_> = initial
            .iter()
            .map(|shape| overlap_volume(*shape, lower, upper))
            .collect();
        let safe = (1..=steps).all(|step| {
            let t = step as f64 / steps as f64;
            let p = std::array::from_fn(|i| start[i] + (next[i] - start[i]) * t);
            let Some(collisions) = overlapping_shapes(habitat, p, size, yaw) else {
                return false;
            };
            if collisions.iter().any(|shape| {
                !initial
                    .iter()
                    .any(|old| old.minimum == shape.minimum && old.maximum == shape.maximum)
            }) {
                return false;
            }
            let (lower, upper) = body_bounds(p, size, yaw);
            initial.iter().zip(&mut volumes).all(|(shape, previous)| {
                let volume = if Habitat::shape_obstructs(*shape, lower, upper, (p, size, yaw)) {
                    overlap_volume(*shape, lower, upper)
                } else {
                    0.0
                };
                let shrinking = volume <= *previous + 0.0000001;
                *previous = volume;
                shrinking
            })
        });
        if safe {
            animal.position = next;
            animal.velocity = [0.0, if falling { animal.velocity[1] } else { 0.0 }, 0.0];
            return true;
        }
    }
    false
}

/// Feet remain below the actual fluid surface; buoyancy never uses a water
/// block's upper face as a walkable floor. The same resolver serves all mammals.
pub(super) fn swimming(animal: &Creature, habitat: &Habitat) -> bool {
    habitat.wet_footprint(animal.position, animal.size(), f64::from(animal.yaw))
}

pub(super) fn swim_toward(
    animal: &mut Creature,
    habitat: &Habitat,
    target: [f64; 3],
    dt: f32,
    speed: f64,
) -> bool {
    let before = animal.position;
    let dx = target[0] - before[0];
    let dz = target[2] - before[2];
    let length = dx.hypot(dz).max(0.001);
    let amount = (speed * animal.movement_scale() * f64::from(dt)).min(length);
    let mut next = [
        before[0] + dx / length * amount,
        before[1],
        before[2] + dz / length * amount,
    ];
    let mut stepped_ashore = false;
    if let Some(shore) = habitat.ground_sized(next, animal.size(), f64::from(animal.yaw)) {
        animal.position = shore;
        animal.behavior = super::Behavior::Wander;
        stepped_ashore = true;
    } else {
        let mut submerged = before;
        submerged[1] -= 0.08;
        let surface = habitat.water_surface(submerged);
        // Keep large mammals' feet within a walkable step of the bank while
        // retaining an immersed lower torso and a clear breathing head.
        let immersion = (animal.size()[1] * 0.68).min(0.9);
        let desired = surface.map_or(1.8, |surface| {
            ((surface - immersion - before[1]) * 3.0).clamp(-1.8, 1.8)
        }) as f32;
        let vertical =
            animal.velocity[1] + (desired - animal.velocity[1]) * (1.0 - (-dt * 6.0).exp());
        next[1] += f64::from(vertical * dt);
        let flow =
            crate::water::fluid_motion_at(habitat.terrain, habitat.chunks, habitat.journal, before);
        for (value, velocity) in next.iter_mut().zip(flow.velocity) {
            *value += f64::from(velocity * 0.4 * dt);
        }
        if habitat.clear_swimmer_rotated(next, animal.size(), f64::from(animal.yaw)) {
            animal.position = next;
        } else {
            // A bank may block horizontal swimming while vertical buoyancy is free.
            let vertical_only = [before[0], next[1], before[2]];
            if habitat.clear_swimmer_rotated(vertical_only, animal.size(), f64::from(animal.yaw)) {
                animal.position = vertical_only;
            }
        }
        animal.behavior = super::Behavior::Swim;
    }
    animal.velocity =
        std::array::from_fn(|i| ((animal.position[i] - before[i]) / f64::from(dt)) as f32);
    if stepped_ashore {
        // A swept bank step is ground support, not a ballistic upward impulse.
        animal.velocity[1] = 0.0;
    }
    (animal.position[0] - before[0]).hypot(animal.position[2] - before[2]) > 0.001
}
