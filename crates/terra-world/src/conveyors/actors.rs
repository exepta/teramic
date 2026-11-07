//! Shared conveyor contact sampling for authority, prediction and creature motors.
use super::*;

/// Feet in world space. Airborne bodies and unloaded/stopped belts contribute no motion.
pub fn surface_velocity(
    chunks: &ChunkStore,
    journal: &EditJournal,
    feet: [f64; 3],
    radius: f64,
) -> [f32; 3] {
    if !journal.has_kinetics()
        || feet.iter().any(|v| !v.is_finite())
        || !radius.is_finite()
        || !(0.0..=4.0).contains(&radius)
    {
        return [0.0; 3];
    }
    // A fixed five-point sole probe also catches bodies straddling two belt cells.
    let reach = radius.min(0.65) * 0.8;
    for [dx, dz] in [
        [0.0; 2],
        [reach, 0.0],
        [-reach, 0.0],
        [0.0, reach],
        [0.0, -reach],
    ] {
        let x = (feet[0] + dx).floor() as i64;
        let z = (feet[2] + dz).floor() as i64;
        let y = feet[1].floor() as i64;
        for y in [y, y - 1] {
            let Some(segment) = segment_at(journal, [x, y, z]) else {
                continue;
            };
            let Some(height) = segment.surface_height(feet) else {
                continue;
            };
            let normal = segment.span.normal();
            let tolerance = 0.08 + radius * f64::from(normal[0].abs() + normal[2].abs());
            if (feet[1] - height).abs() > tolerance || !segment.span.loaded(chunks, journal) {
                continue;
            }
            let axis = kinetic::axis(segment.span.facing);
            let center = segment.center();
            let side = (0..3)
                .map(|i| (feet[i] - center[i]) * axis[i] as f64)
                .sum::<f64>();
            if side.abs() > 0.32 + radius.min(0.65) {
                continue;
            }
            let p = segment.position();
            let state = journal.kinetic_state(p);
            if !state.running()
                || !state.rpm.is_finite()
                || state.rpm.abs() > f32::from(kinetic::MAX_RPM)
            {
                continue;
            }
            let Some(object) = journal.object_state(p) else {
                continue;
            };
            let facing = if journal.block(p) == Some(148) {
                kinetic::belt_facing(object.facing)
            } else {
                object.facing
            };
            let sign = (0..3)
                .map(|i| kinetic::axis(facing)[i] * axis[i])
                .sum::<i64>() as f32;
            let speed = belt_speed(state.rpm * sign) * segment.span.flow_sign();
            return segment.span.direction().map(|v| v * speed);
        }
    }
    [0.0; 3]
}

/// Additional swept locomotion, not an unconstrained transform offset.
pub fn carry_creature(
    animal: &mut terra_core::creatures::Creature,
    habitat: &crate::creatures::Habitat,
    dt: f32,
) {
    if !dt.is_finite() || !(0.0..=0.25).contains(&dt) || animal.velocity[1] > 0.1 {
        return;
    }
    if animal.kind.is_spider()
        && terra_core::creatures::spiders::surface_normal(animal) != [0, 1, 0]
    {
        return;
    }
    let size = animal.size();
    let velocity = surface_velocity(
        habitat.chunks,
        habitat.journal,
        animal.position,
        size[0].min(size[2]) * 0.5,
    );
    if velocity == [0.0; 3] {
        return;
    }
    let target = std::array::from_fn(|i| animal.position[i] + f64::from(velocity[i] * dt));
    if let Some(next) =
        habitat.travel_step_sized(animal.position, target, size, f64::from(animal.yaw))
    {
        animal.position = next;
    }
}
