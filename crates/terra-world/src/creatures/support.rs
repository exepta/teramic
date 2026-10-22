//! Swept gravity is independent of the stricter four-foot walking probe.
use super::{Creature, Habitat};

pub(super) fn settle(animal: &mut Creature, habitat: &Habitat, dt: f32) {
    if let Some(ground) =
        habitat.ground_rotated(animal.position, animal.kind, f64::from(animal.yaw))
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

/// Feet remain below the actual fluid surface; buoyancy never uses a water
/// block's upper face as a walkable floor. The same resolver serves all mammals.
pub(super) fn swimming(animal: &Creature, habitat: &Habitat) -> bool {
    let mut feet = animal.position;
    feet[1] -= 0.08;
    habitat.wet(animal.position) || habitat.wet(feet)
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
    let amount = (speed * f64::from(dt)).min(length);
    let mut next = [
        before[0] + dx / length * amount,
        before[1],
        before[2] + dz / length * amount,
    ];
    if let Some(shore) = habitat.ground_rotated(next, animal.kind, f64::from(animal.yaw)) {
        animal.position = shore;
        animal.behavior = super::Behavior::Wander;
    } else {
        let mut submerged = before;
        submerged[1] -= 0.08;
        let surface = habitat.water_surface(submerged);
        // Keep large mammals' feet within a walkable step of the bank while
        // retaining an immersed lower torso and a clear breathing head.
        let immersion = (animal.kind.size()[1] * 0.68).min(0.9);
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
    (animal.position[0] - before[0]).hypot(animal.position[2] - before[2]) > 0.001
}
