//! Shared fluid response for rigid bodies and the bounded loose-item simulation.
use avian3d::prelude::*;
use bevy::{ecs::system::SystemParam, prelude::*};
use terra_world::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, WorldTerrain},
    water::{CURRENT_RESPONSE, FluidMotion, fluid_motion_at},
};

pub(crate) fn install(app: &mut App) {
    app.add_systems(
        FixedUpdate,
        advect_dynamic
            .after(PhysicsSystems::Prepare)
            .before(PhysicsSystems::StepSimulation),
    );
}

pub fn body_motion(
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
    low: Vec3,
    high: Vec3,
) -> FluidMotion {
    if !low.is_finite() || !high.is_finite() {
        return FluidMotion {
            velocity: [0.0; 3],
            immersion: 0.0,
        };
    }
    let center = (low + high) * 0.5;
    let feet = Vec3::new(
        center.x,
        low.y + ((high.y - low.y) * 0.2).clamp(0.01, 0.05),
        center.z,
    );
    let feet_motion = fluid_motion_at(terrain, chunks, journal, feet.to_array().map(f64::from));
    let center_motion = fluid_motion_at(terrain, chunks, journal, center.to_array().map(f64::from));
    let weight = feet_motion.immersion + center_motion.immersion;
    let velocity = if weight > 0.0 {
        (Vec3::from_array(feet_motion.velocity) * feet_motion.immersion
            + Vec3::from_array(center_motion.velocity) * center_motion.immersion)
            / weight
    } else {
        Vec3::ZERO
    };
    FluidMotion {
        velocity: velocity.to_array(),
        immersion: (weight * 0.5).clamp(0.0, 1.0),
    }
}

pub fn advection(motion: FluidMotion) -> Vec3 {
    Vec3::from_array(motion.velocity) * CURRENT_RESPONSE * motion.immersion.clamp(0.0, 1.0)
}

pub fn drift_velocity(velocity: Vec3, motion: FluidMotion, dt: f32) -> Vec3 {
    let flow = Vec3::from_array(motion.velocity);
    if motion.immersion <= 0.0 || flow.length_squared() < 0.0001 {
        return velocity;
    }
    let blend = 1.0 - (-4.0 * dt.clamp(0.0, 0.1) * motion.immersion.clamp(0.0, 1.0)).exp();
    let target = flow * CURRENT_RESPONSE;
    let mut next = velocity;
    if flow.x.abs() + flow.z.abs() > 0.001 {
        next.x += (target.x - next.x) * blend;
        next.z += (target.z - next.z) * blend;
    }
    if flow.y.abs() > 0.001 {
        next.y += (target.y - next.y) * blend;
    }
    next
}

#[derive(SystemParam)]
struct CurrentWorld<'w> {
    terrain: Option<Res<'w, WorldTerrain>>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    time: Res<'w, Time<Fixed>>,
}

type Bodies<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static RigidBody,
        &'static ColliderAabb,
        &'static mut LinearVelocity,
        Has<Sleeping>,
        Option<&'static mut SleepTimer>,
    ),
    Without<crate::character::CharacterBody>,
>;

fn advect_dynamic(mut commands: Commands, world: CurrentWorld, mut bodies: Bodies) {
    let Some(terrain) = world.terrain else {
        return;
    };
    for (entity, body, bounds, mut velocity, sleeping, timer) in &mut bodies {
        if *body != RigidBody::Dynamic {
            continue;
        }
        let motion = body_motion(
            &terrain.0,
            &world.chunks,
            &world.journal,
            bounds.min,
            bounds.max,
        );
        let next = drift_velocity(velocity.0, motion, world.time.delta_secs());
        if next.distance_squared(velocity.0) <= 1e-10 {
            continue;
        }
        velocity.0 = next;
        if sleeping {
            commands.entity(entity).remove::<Sleeping>();
        }
        if let Some(mut timer) = timer {
            timer.0 = 0.0;
        }
    }
}

#[cfg(test)]
mod tests;
