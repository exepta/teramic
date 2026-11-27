//! Conservative swept flight clearance. Wings, neck and tail extend well beyond
//! the walking collider. Empty loaded sections let long sky probes finish cheaply.
use super::*;
use terra_voxel::{MaterialId, SectionPos, global_to_section};

fn empty(h: &Habitat, low: DVec3, high: DVec3) -> bool {
    let min = low.floor().as_i64vec3().to_array();
    let max = high.floor().as_i64vec3().to_array();
    let (lo, _) = global_to_section(min);
    let (hi, _) = global_to_section(max);
    for z in lo.z..=hi.z {
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let section = SectionPos::new(x, y, z);
                let Some(loaded) = h.chunks.loaded.get(&section) else {
                    return false;
                };
                if h.chunks.voxel_base_pending(section) {
                    return false;
                }
                let edited = h.journal.revision(section) != 0;
                if loaded.data.is_empty() && !edited {
                    continue;
                }
                let origin = [16 * x, 16 * y, 16 * z];
                let start =
                    std::array::from_fn::<_, 3, _>(|i| (min[i] - origin[i]).max(0) as usize);
                let end = std::array::from_fn::<_, 3, _>(|i| (max[i] - origin[i]).min(15) as usize);
                for k in start[2]..=end[2] {
                    for j in start[1]..=end[1] {
                        let base = 16 * (j + 16 * k);
                        let row = &loaded.data.blocks()[base + start[0]..=base + end[0]];
                        if row.iter().enumerate().any(|(i, &base)| {
                            let id = if edited {
                                h.journal
                                    .block([
                                        origin[0] + (start[0] + i) as i64,
                                        origin[1] + j as i64,
                                        origin[2] + k as i64,
                                    ])
                                    .unwrap_or(base)
                            } else {
                                base
                            };
                            id != 0 && MaterialId::from_raw(id).is_none_or(|m| m.is_solid())
                        }) {
                            return false;
                        }
                    }
                }
            }
        }
    }
    true
}

/// Three overlapping bands span the feet, the axial skeleton, and outstretched
/// wings. The sweep includes the turn between the old and proposed headings.
pub(super) fn passage(h: &Habitat, a: &Creature, start: DVec3, end: DVec3, yaw: f64) -> bool {
    let scale = f64::from(pose::model_scale(a.kind, a.variant)) * a.growth_scale();
    let wingspan = 1.82 + 16.98 * f64::from(pose::wing_scale(a.kind, a.variant));
    let mid = f64::from(a.yaw) + angle_delta(f64::from(a.yaw), yaw) * 0.5;
    for (width, front, back, low, high) in [
        (2.7, 4.0, 3.7, 0.08, 3.4),
        (2.8, 10.1, 17.2, 3.3, 10.1),
        (wingspan, 2.0, 10.5, 3.6, 13.0),
    ] {
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        for heading in [f64::from(a.yaw), mid, yaw] {
            let (s, c) = heading.sin_cos();
            for x in [-width, width] {
                for z in [-front, back] {
                    let offset = DVec3::new(x * c + z * s, 0., z * c - x * s) * scale;
                    lo = lo.min(offset);
                    hi = hi.max(offset);
                }
            }
        }
        lo.y = low * scale;
        hi.y = high * scale;
        if !empty(h, start.min(end) + lo, end.max(start) + hi) {
            return false;
        }
    }
    true
}

pub(super) fn can_launch(a: &Creature, h: &Habitat) -> bool {
    if !pose::flies(a.kind) || !a.dragon_stage().can_fly() {
        return false;
    }
    let from = DVec3::from_array(a.position);
    let yaw = f64::from(a.yaw);
    let fwd = DVec3::new(-yaw.sin(), 0., -yaw.cos());
    passage(h, a, from, from + DVec3::Y * 8.0 + fwd * 4.0, yaw)
}

pub(super) fn clean_landing(a: &Creature, h: &Habitat, ground: [f64; 3]) -> bool {
    let p = DVec3::from_array(ground);
    let support = (p - DVec3::Y * 0.1).floor().as_i64vec3().to_array();
    if !h
        .block(support)
        .and_then(MaterialId::from_raw)
        .is_some_and(|m| {
            m.is_natural_ground()
                || matches!(
                    m,
                    MaterialId::SaltStone
                        | MaterialId::WetSaltStone
                        | MaterialId::DeepSaltStone
                        | MaterialId::StrawBlock
                )
        })
        || h.wet_footprint(ground, a.size(), f64::from(a.yaw))
    {
        return false;
    }
    let scale = f64::from(pose::model_scale(a.kind, a.variant)) * a.growth_scale();
    let (sin, cos) = f64::from(a.yaw).sin_cos();
    // All four feet need a reachable patch at touchdown. A lone column under
    // the centre of the large walking collider cannot support a landing.
    for hind in [false, true] {
        for side in [-1, 1] {
            let paw = terra_core::creatures::dragon_pose::Joint::Paw { side, hind }.pivot();
            let x = f64::from(paw.x) * scale;
            let z = f64::from(paw.z) * scale;
            let sample = [p.x + cos * x + sin * z, p.y, p.z - sin * x + cos * z];
            if h.paw_surface(sample, (scale * 1.9).max(1.1)).is_none() {
                return false;
            }
        }
    }
    flare_clear(a, h, p, p + DVec3::Y * 1.25)
}

/// When the wing fingers fold into the body on final approach, test the body
/// and folded shoulders along the descent. An extended flight envelope here
/// rejects ordinary slopes several metres beyond all four planted feet.
pub(super) fn flare_clear(a: &Creature, h: &Habitat, from: DVec3, to: DVec3) -> bool {
    let size = a.size();
    let yaw = f64::from(a.yaw);
    let scale = f64::from(pose::model_scale(a.kind, a.variant)) * a.growth_scale();
    let shoulder = DVec3::new(3.6, 0., 4.0) * scale;
    [0., 0.5, 1.].into_iter().all(|t| {
        let p = from.lerp(to, t);
        h.clear_swimmer_rotated(p.to_array(), size, yaw)
            && empty(
                h,
                p - shoulder + DVec3::Y * 3.4 * scale,
                p + shoulder + DVec3::Y * 9.4 * scale,
            )
    })
}

impl Encounter {
    pub(super) fn fly(&mut self, a: &mut Creature, h: &Habitat, goal: DVec3, dt: f64, speed: f64) {
        let from = DVec3::from_array(a.position);
        let aim = (goal - from).clamp_length_max(speed);
        let desired = aim.normalize_or_zero();
        let side = DVec3::new(-desired.z, 0., desired.x).normalize_or_zero()
            * if a.id.is_multiple_of(2) { 1. } else { -1. };
        self.steer_time -= dt;
        if self.steer_time <= 0. {
            // Probe braking distance before committing. Keep the chosen detour
            // briefly so an isolated tree cannot cause alternating left/right turns.
            let reach = (2.8 + self.flight_velocity.length_squared() / 19.).max(5.);
            let detour = [
                desired,
                (desired + DVec3::Y * 1.15).normalize_or_zero(),
                (desired + side * 1.45 + DVec3::Y * 0.15).normalize_or_zero(),
                (desired - side * 1.45 + DVec3::Y * 0.15).normalize_or_zero(),
                DVec3::Y,
                side,
                -side,
                -desired,
            ];
            self.course = detour
                .into_iter()
                .find(|d| {
                    d.length_squared() > 0.5
                        && passage(h, a, from, from + *d * reach, f64::from(a.yaw))
                })
                .unwrap_or(DVec3::ZERO);
            self.steer_time = if self.course.dot(desired) > 0.98 {
                0.25
            } else {
                0.85
            };
        }
        // Steer the body into the chosen corridor before accelerating through
        // it. A reversal makes a braked, banked turn instead of a sideways slide.
        let old_heading = f64::from(a.yaw);
        let flat = self.course.with_y(0.);
        let heading = if flat.length_squared() > 0.015 {
            let desired_heading = (-flat.x).atan2(-flat.z);
            let turn_rate = (1.65 - self.flight_velocity.length() * 0.035).max(0.85);
            old_heading
                + angle_delta(old_heading, desired_heading).clamp(-turn_rate * dt, turn_rate * dt)
        } else {
            old_heading
        };
        let forward = DVec3::new(-heading.sin(), 0., -heading.cos());
        let alignment = forward.dot(flat.normalize_or_zero()).max(0.);
        let wish = (forward * flat.length() * alignment + DVec3::Y * self.course.y) * aim.length();
        // Lift and forward thrust pulse after the visible power stroke. Lose
        // the impulse while braking over a pad or waiting for loaded airspace.
        let [lift, thrust] = terra_core::creatures::dragon_pose::wing_impulse(
            a,
            self.flight_velocity.as_vec3().to_array(),
        );
        let work = (aim.length() / 9.).clamp(0., 1.) * self.course.length();
        let wish = (wish
            + (DVec3::Y * f64::from(lift) + forward * f64::from(thrust) * alignment) * work)
            .clamp_length_max(speed);
        let velocity =
            self.flight_velocity + (wish - self.flight_velocity).clamp_length_max(12.5 * dt);
        let next = from + velocity * dt;
        let radial = |p: DVec3| (p - self.home).with_y(0.).length_squared();
        let clear_heading = if passage(h, a, from, next, heading) {
            Some(heading)
        } else if heading != f64::from(a.yaw) && passage(h, a, from, next, f64::from(a.yaw)) {
            Some(f64::from(a.yaw))
        } else {
            None
        };
        if let Some(heading) = clear_heading
            && (radial(next) <= ROAM_RADIUS.powi(2) || radial(next) < radial(from))
        {
            a.yaw = heading.rem_euclid(std::f64::consts::TAU) as f32;
            self.flight_velocity = velocity;
            translate(a, next, dt);
        } else {
            // Cancel momentum into the obstacle; replan from the last safe pose.
            self.flight_velocity = DVec3::ZERO;
            self.steer_time = 0.;
        }
        a.behavior = B::Fly;
    }
}
fn angle_delta(from: f64, to: f64) -> f64 {
    (to - from + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}
