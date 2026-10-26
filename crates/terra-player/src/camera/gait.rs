//! Presentation-only footsteps; never move the controller or rotate the aim ray.
use bevy::prelude::*;
use std::f32::consts::TAU;

// Match the existing local-player contact cadence (one contact every 1.25 metres).
const STEP_DISTANCE: f32 = 1.25;
const WALK_SPEED: f32 = terra_physics::character::WALK_SPEED;

#[derive(Component, Debug, Default)]
pub(crate) struct WalkCameraMotion {
    body: Option<Entity>,
    previous: Option<Vec3>,
    phase: f32,
    amplitude: f32,
    grounded: bool,
    fall_speed: f32,
    landing_offset: f32,
    landing_velocity: f32,
    step_previous: Option<(Entity, Vec3)>,
    step_offset: f32,
}

pub(crate) struct GaitFrame {
    pub body: Entity,
    pub position: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
    pub enabled: bool,
    pub moving: bool,
    pub delta: f32,
}

impl WalkCameraMotion {
    pub(crate) fn reset_step(&mut self, body: Entity, position: Vec3) {
        self.step_previous = Some((body, position));
        self.step_offset = 0.0;
    }

    pub(crate) fn step_height(
        &mut self,
        body: Entity,
        position: Vec3,
        grounded: bool,
        dt: f32,
    ) -> f32 {
        let previous = self.step_previous.replace((body, position));
        if !dt.is_finite() || !(0.0..=0.1).contains(&dt) || !position.is_finite() {
            self.step_offset = 0.0;
            return 0.0;
        }
        if let Some((old_body, old)) = previous {
            let travel = position - old;
            if body != old_body || travel.length_squared() > 1.0 {
                self.step_offset = 0.0;
            } else if grounded && travel.y > 0.0 && travel.y <= 0.55 {
                self.step_offset = (self.step_offset - travel.y).max(-0.55);
            }
        }
        self.step_offset *= (-16.0 * dt).exp();
        self.step_offset
    }

    pub(crate) fn offset(&mut self, frame: GaitFrame) -> Vec2 {
        let previous = self.previous.replace(frame.position);
        let same_body = self.body.replace(frame.body) == Some(frame.body);
        let delta = frame.delta;
        if !frame.enabled
            || !same_body
            || !frame.position.is_finite()
            || !frame.velocity.is_finite()
            || !delta.is_finite()
            || !(0.0..=0.1).contains(&delta)
        {
            self.reset(frame.grounded);
            return Vec2::ZERO;
        }
        let Some(previous) = previous else {
            self.reset(frame.grounded);
            return Vec2::ZERO;
        };
        if !previous.is_finite() {
            self.reset(frame.grounded);
            return Vec2::ZERO;
        }
        let travel = frame.position - previous;
        // Network teleports and large corrections are not footsteps or landings.
        if travel.length_squared() > (16.0 * delta + 0.2).powi(2) {
            self.reset(frame.grounded);
            return Vec2::ZERO;
        }
        if delta <= f32::EPSILON {
            return self.current_offset();
        }
        if !frame.grounded {
            self.fall_speed = self
                .fall_speed
                .max(-frame.velocity.y)
                .max(-travel.y / delta);
            self.phase = 0.0;
            self.amplitude = 0.0;
            self.landing_offset = 0.0;
            self.landing_velocity = 0.0;
            self.grounded = false;
            return Vec2::ZERO;
        }
        if !self.grounded {
            self.landing_velocity -= ((self.fall_speed - 1.4) * 0.055).clamp(0.0, 0.6);
            self.fall_speed = 0.0;
        }
        self.grounded = true;
        let distance = travel.xz().length();
        let speed = distance / delta;
        let walking = frame.moving && speed > 0.15;
        let target_amplitude = if walking {
            (speed / WALK_SPEED).clamp(0.0, 1.28)
        } else {
            0.0
        };
        let response = if walking { 14.0 } else { 22.0 };
        self.amplitude += (target_amplitude - self.amplitude) * (1.0 - (-response * delta).exp());
        if walking {
            self.phase = (self.phase + distance * TAU / (STEP_DISTANCE * 2.0)) % TAU;
        }
        // Exact critically damped spring: no overshoot or frame-rate-dependent Euler step.
        let frequency = 18.0;
        let impulse = self.landing_velocity + frequency * self.landing_offset;
        let retention = (-frequency * delta).exp();
        self.landing_offset = (self.landing_offset + impulse * delta) * retention;
        self.landing_velocity = (self.landing_velocity - frequency * impulse * delta) * retention;
        self.current_offset()
    }

    fn current_offset(&self) -> Vec2 {
        Vec2::new(
            self.phase.sin() * 0.018 * self.amplitude,
            -(self.phase * 2.0).cos() * 0.036 * self.amplitude + self.landing_offset,
        )
    }

    fn reset(&mut self, grounded: bool) {
        self.phase = 0.0;
        self.amplitude = 0.0;
        self.grounded = grounded;
        self.fall_speed = 0.0;
        self.landing_offset = 0.0;
        self.landing_velocity = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_stair_rise_is_smoothed_at_all_render_rates_without_following_teleports() {
        for hz in [30, 60, 144, 240] {
            let dt = 1.0 / hz as f32;
            let mut camera = WalkCameraMotion::default();
            let body = Entity::PLACEHOLDER;
            camera.step_height(body, Vec3::ZERO, true, dt);
            let offset = camera.step_height(body, Vec3::Y * (1.0 / 3.0), true, dt);
            assert!(offset < -0.19 && offset > -1.0 / 3.0);
            for _ in 0..hz {
                camera.step_height(body, Vec3::Y * (1.0 / 3.0), true, dt);
            }
            assert!(camera.step_offset.abs() < 0.0001);
            assert_eq!(camera.step_height(body, Vec3::Y * 30.0, true, dt), 0.0);
        }
    }

    fn frame(position: Vec3, delta: f32) -> GaitFrame {
        GaitFrame {
            body: Entity::PLACEHOLDER,
            position,
            velocity: Vec3::X * WALK_SPEED,
            grounded: true,
            enabled: true,
            moving: true,
            delta,
        }
    }

    #[test]
    fn cadence_depends_on_real_distance_at_all_render_rates() {
        let mut results = Vec::new();
        for hz in [30, 60, 144, 240] {
            let delta = 1.0 / hz as f32;
            let mut motion = WalkCameraMotion::default();
            motion.offset(frame(Vec3::ZERO, delta));
            let mut offset = Vec2::ZERO;
            for tick in 1..=hz * 2 {
                offset = motion.offset(frame(Vec3::X * (tick as f32 * delta * WALK_SPEED), delta));
            }
            results.push(offset);
        }
        for offset in &results[1..] {
            assert!(offset.distance(results[0]) < 0.00003, "{results:?}");
        }
    }

    #[test]
    fn pushing_a_wall_does_not_advance_steps_and_stops_softly() {
        let delta = 1.0 / 60.0;
        let mut motion = WalkCameraMotion::default();
        motion.offset(frame(Vec3::ZERO, delta));
        for tick in 1..=60 {
            motion.offset(frame(Vec3::X * (tick as f32 * delta * WALK_SPEED), delta));
        }
        let phase = motion.phase;
        let before = motion.current_offset().length();
        let after = motion.offset(frame(Vec3::X * WALK_SPEED, delta)).length();
        assert!(after < before && after > 0.0);
        for _ in 0..60 {
            motion.offset(frame(Vec3::X * WALK_SPEED, delta));
        }
        assert_eq!(motion.phase, phase);
        assert!(motion.current_offset().length() < 0.00001);
    }

    #[test]
    fn mounting_resets_walk_step_smoothing_without_a_later_camera_height_kick() {
        let body = Entity::PLACEHOLDER;
        let mut motion = WalkCameraMotion::default();
        motion.step_height(body, Vec3::ZERO, true, 1.0 / 60.0);
        assert!(motion.step_height(body, Vec3::Y * 0.5, true, 1.0 / 60.0) < -0.1);
        motion.reset_step(body, Vec3::Y * 2.0);
        assert_eq!(
            motion.step_height(body, Vec3::Y * 2.0, false, 1.0 / 60.0),
            0.0
        );
    }

    #[test]
    fn sprint_increases_stride_amplitude_without_time_driven_idle_motion() {
        let mut walk = WalkCameraMotion::default();
        let mut sprint = WalkCameraMotion::default();
        let delta = 1.0 / 60.0;
        for tick in 0..=60 {
            walk.offset(frame(Vec3::X * (tick as f32 * delta * WALK_SPEED), delta));
            sprint.offset(frame(
                Vec3::X * (tick as f32 * delta * terra_physics::character::SPRINT_SPEED),
                delta,
            ));
        }
        assert!(sprint.amplitude > walk.amplitude * 1.2);
        assert!(sprint.amplitude <= 1.28);
        let mut idle = WalkCameraMotion::default();
        for _ in 0..120 {
            assert_eq!(idle.offset(frame(Vec3::ZERO, delta)), Vec2::ZERO);
        }
    }

    #[test]
    fn airborne_and_disabled_views_have_no_bob_and_teleports_reset() {
        let mut motion = WalkCameraMotion::default();
        let delta = 1.0 / 60.0;
        for tick in 0..10 {
            motion.offset(frame(Vec3::X * (tick as f32 * delta * WALK_SPEED), delta));
        }
        let mut air = frame(Vec3::new(1.0, 0.2, 0.0), delta);
        air.grounded = false;
        assert_eq!(motion.offset(air), Vec2::ZERO);
        let mut disabled = frame(Vec3::X, delta);
        disabled.enabled = false;
        assert_eq!(motion.offset(disabled), Vec2::ZERO);
        assert_eq!(motion.offset(frame(Vec3::X * 200.0, delta)), Vec2::ZERO);
        assert_eq!(motion.offset(frame(Vec3::X * 200.0, delta)), Vec2::ZERO);
    }

    #[test]
    fn landings_compress_and_settle_without_adding_physics_motion() {
        let mut motion = WalkCameraMotion::default();
        let delta = 1.0 / 60.0;
        motion.offset(frame(Vec3::Y, delta));
        for tick in 1..=5 {
            let mut air = frame(Vec3::Y * (1.0 - tick as f32 * delta * 5.0), delta);
            air.grounded = false;
            air.velocity = Vec3::NEG_Y * 5.0;
            motion.offset(air);
        }
        let mut landed = frame(Vec3::Y * (1.0 - 5.0 * delta * 5.0), delta);
        landed.moving = false;
        let offset = motion.offset(landed);
        assert!(offset.y < 0.0 && offset.y > -0.025, "{offset:?}");
        for _ in 0..120 {
            let mut stopped = frame(Vec3::Y * (1.0 - 5.0 * delta * 5.0), delta);
            stopped.moving = false;
            motion.offset(stopped);
        }
        assert!(motion.current_offset().length() < 0.00001);
    }

    #[test]
    fn frame_gaps_and_invalid_positions_cannot_accumulate_bob() {
        let mut motion = WalkCameraMotion::default();
        assert_eq!(motion.offset(frame(Vec3::ZERO, 0.3)), Vec2::ZERO);
        assert_eq!(
            motion.offset(frame(Vec3::splat(f32::NAN), 0.016)),
            Vec2::ZERO
        );
        assert_eq!(motion.offset(frame(Vec3::X, 0.016)), Vec2::ZERO);
    }
}
