//! Movement in open water and through a broad bank of natural soil. Burrowing
//! uses the surface as its replicated anchor, like the sandworm's visible wake.
use super::*;
use terra_voxel::MaterialId;

impl Encounter {
    /// Turning a long body broadside near rock can be as obstructed as advancing.
    pub(super) fn turn_swimmer(a: &mut Creature, h: &Habitat, goal: DVec3, angle: f64) {
        let prior = a.yaw;
        turn(a, goal, angle);
        if a.yaw != prior && !h.clear_swimmer_rotated(a.position, a.size(), a.yaw.into()) {
            a.yaw = prior;
        }
    }

    pub(super) fn dive(
        &mut self,
        a: &mut Creature,
        h: &Habitat,
        goal: DVec3,
        dt: f64,
        speed: f64,
    ) -> bool {
        let from = DVec3::from_array(a.position);
        let size = a.size();
        let Some(surface) = h.water_surface((from + DVec3::Y * 0.15).to_array()) else {
            return false;
        };
        a.behavior = B::Swim;
        // An older or disturbed swimmer can straddle the waterline. Requiring its
        // crown to be wet on the very next tick would trap it at that height.
        if !h.wet((from + DVec3::Y * (size[1] - 0.14)).to_array()) {
            let sink = from - DVec3::Y * (3.4 * dt);
            if h.wet((sink + DVec3::Y * 0.12).to_array())
                && h.clear_swimmer_rotated(sink.to_array(), size, a.yaw.into())
            {
                translate(a, sink, dt);
                return true;
            }
            // The bed is too shallow to sink here. Follow the deepest neighbouring
            // channel while keeping the belly immersed and the hull clear.
            let heading = (goal - from).with_y(0.).normalize_or(DVec3::NEG_Z);
            let side = DVec3::new(-heading.z, 0., heading.x);
            let mut best = None;
            for (forward, lateral) in [
                (1., 0.),
                (0.7, 0.7),
                (0.7, -0.7),
                (0., 1.),
                (0., -1.),
                (-1., 0.),
            ] {
                let dir = heading * forward + side * lateral;
                let next = from + dir * (speed * dt);
                let probe = from + dir * 3.;
                if !h.wet((probe + DVec3::Y * 0.12).to_array())
                    || !h.wet((next + DVec3::Y * 0.12).to_array())
                    || !h.clear_swimmer_rotated(next.to_array(), size, a.yaw.into())
                {
                    continue;
                }
                let depth = (1..=6)
                    .take_while(|d| h.wet((probe - DVec3::Y * f64::from(*d)).to_array()))
                    .count();
                if best.is_none_or(|(old, _)| depth > old) {
                    best = Some((depth, next));
                }
            }
            if let Some((_, next)) = best {
                Self::turn_swimmer(a, h, next, dt * 1.3);
                if h.clear_swimmer_rotated(next.to_array(), size, a.yaw.into()) {
                    translate(a, next, dt);
                }
            }
            return true;
        }
        let goal = goal.with_y(goal.y.min(surface - size[1] - 0.32));
        let detour = self.swim_detour.filter(|p| p.distance_squared(from) > 0.5);
        self.swim_detour = detour;
        let destination = detour.unwrap_or(goal);
        let to = destination - from;
        if to.length_squared() < 0.02 {
            return true;
        }
        Self::turn_swimmer(a, h, destination, dt * 1.65);
        let yaw = f64::from(a.yaw);
        let clear = |p: DVec3| {
            h.wet((p + DVec3::Y * 0.12).to_array())
                && h.wet((p + DVec3::Y * (size[1] - 0.14)).to_array())
                && h.clear_swimmer_rotated(p.to_array(), size, yaw)
        };
        let steer = |offset: DVec3| {
            let xz = offset.with_y(0.).clamp_length_max(speed * dt);
            xz + DVec3::Y * offset.y.clamp(-3.4 * dt, 3.4 * dt)
        };
        let step = steer(to);
        // Reserve space ahead. If the floor rises steeply the dragon should
        // start climbing or turn aside while it still has turning room.
        let ahead = from + step.normalize_or_zero() * to.length().min(1.65);
        if clear(from + step) && clear(ahead) {
            translate(a, from + step, dt);
            return true;
        }
        self.swim_detour = None;
        self.swim_retry -= dt;
        if self.swim_retry > 0. {
            return true;
        }
        let heading = (goal - from).with_y(0.).normalize_or(DVec3::NEG_Z);
        let port =
            DVec3::new(-heading.z, 0., heading.x) * if a.id.is_multiple_of(2) { 1. } else { -1. };
        // Sweep each candidate with the real hull. Keep the bend for several
        // metres so the next update cannot turn straight back into the reef.
        for (forward, lateral, ascent) in [
            (1., 0., 0.65),
            (1., 0., -0.65),
            (0.72, 0.72, 0.),
            (0.72, -0.72, 0.),
            (0., 1., 0.),
            (0., -1., 0.),
            (0., 0., 1.),
            (0., 0., -1.),
            (-0.54, 0.85, 0.),
            (-0.54, -0.85, 0.),
            (-1., 0., 0.),
        ] {
            let direction = (heading * forward + port * lateral + DVec3::Y * ascent).normalize();
            let mut distance = 0.;
            for i in 1..=6 {
                if !clear(from + direction * f64::from(i)) {
                    break;
                }
                distance = f64::from(i);
            }
            if distance >= 3. {
                let offset = direction * distance;
                let next = from + steer(offset);
                if clear(next) {
                    self.swim_detour = Some(from + offset);
                    translate(a, next, dt);
                    break;
                }
            }
        }
        if self.swim_detour.is_none() {
            self.swim_retry = 0.45;
        }
        true
    }
    pub(super) fn territory(&mut self, a: &mut Creature, h: &Habitat, dt: f64) {
        let phase = (a.age + (a.id % 61) as f64).rem_euclid(76.);
        let from = DVec3::from_array(a.position);
        let water = a.kind == K::WaterDragon;
        let resting = phase < if water { 3. } else { 18. };
        let goal = if resting || from.distance_squared(self.home) > 55. * 55. {
            self.home
        } else {
            let bearing = phase * 0.075 + (a.id % 12) as f64;
            self.home
                + DVec3::new(
                    bearing.sin() * 21.,
                    if water {
                        3. + (phase * 0.15).sin() * 3.
                    } else {
                        0.
                    },
                    bearing.cos() * 18.,
                )
        };
        if (goal - from).length_squared() < 1. && resting {
            a.behavior = if water { B::Swim } else { B::Rest };
            return;
        }
        if water && self.dive(a, h, goal, dt, 4.8) {
            return;
        }
        if !water
            && self.cooldown <= 0.
            && phase > 40.
            && phase < 45.
            && self.begin_tunnel(a, h, goal)
        {
            return;
        }
        self.walk(a, h, goal, dt, 2.8, false);
    }
    fn bed(a: &Creature, h: &Habitat, point: DVec3) -> Option<DVec3> {
        let mut midpoint = None;
        let radius = (a.size()[0] * 0.64).max(2.);
        for (x, z) in [(0., 0.), (-1., -0.7), (-1., 0.7), (1., 0.7), (1., -0.7)] {
            let p = point + DVec3::new(x * radius, 0., z * radius);
            let top = h.paw_surface(p.to_array(), 2.6)?;
            if (top - point.y).abs() > 2.2 || h.wet(p.with_y(top + 0.12).to_array()) {
                return None;
            }
            let floor = p.with_y(top - 0.1).floor().as_i64vec3().to_array();
            for down in [0, 1] {
                let cell = [floor[0], floor[1] - down, floor[2]];
                let material = MaterialId::from_raw(h.block(cell)?)?;
                if h.journal.block(cell).is_some()
                    || !(material.is_natural_ground()
                        || matches!(material, MaterialId::Stone | MaterialId::Sandstone))
                {
                    return None;
                }
            }
            if x == 0. {
                midpoint = Some(point.with_y(top));
            }
        }
        midpoint
    }
    pub(super) fn begin_tunnel(&mut self, a: &mut Creature, h: &Habitat, target: DVec3) -> bool {
        let from = DVec3::from_array(a.position);
        if from.distance_squared(target) < 36. || Self::bed(a, h, from).is_none() {
            return false;
        }
        a.needs.goal = Some(NeedGoal {
            kind: NeedKind::AttackAim,
            position: target.floor().as_i64vec3().to_array(),
        });
        set_pose(a, B::DragonBurrow);
        true
    }
    pub(super) fn tunnel(&mut self, a: &mut Creature, h: &Habitat, dt: f64) {
        let phase = pose::phase(B::DragonBurrow, a.timer);
        let current = DVec3::from_array(a.position);
        if let Some(goal) = a.needs.goal {
            let target = DVec3::from_array(goal.position.map(|v| v as f64 + 0.5));
            let dir = (target - current).with_y(0.);
            if (0.24..0.72).contains(&phase) && dir.length_squared() > 9. {
                turn(a, target, dt * 2.);
                let next = current + dir.clamp_length_max(9. * dt);
                if let Some(exit) = Self::bed(a, h, next)
                    .filter(|p| h.clear_swimmer_rotated(p.to_array(), a.size(), f64::from(a.yaw)))
                {
                    translate(a, exit, dt);
                } else {
                    a.timer = pose::duration(B::DragonBurrow) * 0.25;
                }
            }
        }
        if a.timer <= 0. {
            set_pose(a, B::Hunt);
            a.needs.goal = None;
            self.cooldown = 0.9;
        }
    }
}
