//! The guardian walks through the mouth before unfolding its wings. A sortie
//! has two distant waypoints, with a terrain-following approach to the same pad.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Leg {
    Rest,
    WalkOut,
    Outbound,
    Crosswind,
    Approach,
    EnterMouth,
    WalkIn,
}

#[derive(Debug)]
pub(super) struct Patrol {
    forward: DVec3,
    runway: f64,
    leg: Leg,
    seconds: f64,
    pad: DVec3,
    first: DVec3,
    second: DVec3,
    sorties: u64,
    previous_distance: f64,
    stalled: f64,
    cruise_level: f64,
    survey_after: f64,
    detour: Option<DVec3>,
    detour_after: f64,
}

impl Patrol {
    pub(super) fn new(direction: [i64; 2], a: &Creature, home: DVec3) -> Self {
        let forward = DVec3::new(direction[0] as f64, 0., direction[1] as f64);
        Self {
            forward,
            runway: 54.,
            leg: if matches!(a.behavior, B::Fly | B::Land | B::Leap) {
                Leg::Approach
            } else if DVec3::from_array(a.position).distance(home) > 9. {
                Leg::EnterMouth
            } else {
                Leg::Rest
            },
            seconds: 9. + (a.id % 13) as f64,
            pad: home + forward * 54.,
            first: home,
            second: home,
            sorties: 0,
            previous_distance: f64::INFINITY,
            stalled: 0.,
            cruise_level: home.y + 12.,
            survey_after: 0.,
            detour: None,
            detour_after: 0.,
        }
    }

    pub(super) fn open_country(a: &Creature, home: DVec3) -> Self {
        let mut patrol = Self::new([0, -1], a, home);
        let (s, c) = f64::from(a.yaw).sin_cos();
        patrol.forward = DVec3::new(-s, 0., -c);
        patrol.runway = 0.;
        patrol.pad = home;
        patrol
    }
    fn plot(&mut self, home: DVec3, id: u64) {
        let seed = crate::generation::seed::layer_seed(id, self.sorties);
        self.sorties += 1;
        let distance = ROAM_RADIUS - f64::from((seed % 353) as u16);
        let angle = (f64::from(((seed >> 12) % 1000) as u16) / 999.0 - 0.5) * 1.22;
        let cross = if seed & 1 == 0 { 0.49 } else { -0.49 };
        let side = DVec3::new(-self.forward.z, 0., self.forward.x);
        let waypoint = |a: f64, r: f64| home + (self.forward * a.cos() + side * a.sin()) * r;
        self.first = waypoint(angle, distance);
        self.second = waypoint(angle + cross, distance * 0.84);
        self.leg = Leg::Outbound;
        self.detour = None;
        self.previous_distance = f64::INFINITY;
        self.stalled = 0.;
        self.survey_after = 0.;
    }

    fn cruise(
        &mut self,
        h: &Habitat,
        from: DVec3,
        waypoint: DVec3,
        dt: f64,
        headroom: f64,
    ) -> DVec3 {
        self.survey_after -= dt;
        if self.survey_after <= 0. {
            let heading = (waypoint - from).with_y(0.).normalize_or_zero();
            self.cruise_level = [0., 14., 35.]
                .into_iter()
                .map(|d| {
                    let p = from + heading * d;
                    h.chunks
                        .surface_column_at(p.x.floor() as i64, p.z.floor() as i64)
                        .map_or_else(
                            || h.terrain.surface_height(p.x, p.z),
                            |c| c.height.max(c.water_level.unwrap_or(c.height)),
                        )
                })
                .fold(f64::NEG_INFINITY, f64::max)
                + headroom;
            self.survey_after = 0.85;
        }
        let horizontal = (waypoint - from).with_y(0.).clamp_length_max(23.);
        (from + horizontal).with_y(self.cruise_level)
    }

    pub(super) fn chase_waypoint(&self, home: DVec3, from: DVec3, target: DVec3) -> Option<DVec3> {
        let inside = (from - home).dot(self.forward) < 28.;
        let outside = (target - home).dot(self.forward) > 34.;
        (inside && outside).then_some(home + self.forward * 35.)
    }
    pub(super) fn recall(&mut self, airborne: bool) {
        self.leg = if airborne {
            Leg::Approach
        } else {
            Leg::EnterMouth
        };
        self.detour = None;
        self.detour_after = 0.;
        self.survey_after = 0.;
    }
}

impl Encounter {
    pub(super) fn patrol(&mut self, a: &mut Creature, h: &Habitat, dt: f64, night: bool) {
        let mut patrol = self.patrol.take().expect("a natural roost");
        let from = DVec3::from_array(a.position);
        let pad_distance = (from - patrol.pad).with_y(0.).length();
        let height = match a.kind {
            K::DarkDragon => 60.,
            K::WaterDragon => 42.,
            _ => 50.,
        };
        match patrol.leg {
            Leg::Rest => {
                patrol.seconds -= dt;
                if patrol.seconds <= 0. {
                    patrol.leg = Leg::WalkOut;
                    patrol.pad = self.home + patrol.forward * patrol.runway;
                }
                a.behavior = if patrol.seconds > 15. && !(a.kind == K::DarkDragon && night) {
                    B::Sleep
                } else {
                    B::Rest
                };
            }
            Leg::WalkOut => {
                if pad_distance > 2. {
                    self.travel(a, h, patrol.pad, dt, 4.1);
                } else if self.airborne || flight::can_launch(a, h) {
                    patrol.pad.y = from.y;
                    patrol.runway = (from - self.home).dot(patrol.forward);
                    patrol.plot(self.home, a.id.wrapping_add((a.age / 128.) as u64));
                    if !self.airborne {
                        set_pose(a, B::Leap);
                    }
                } else if (patrol.pad - self.home).with_y(0.).length() < 75. {
                    patrol.pad += patrol.forward * 4.;
                } else {
                    patrol.recall(false);
                }
            }
            Leg::Outbound | Leg::Crosswind => {
                let waypoint = if patrol.leg == Leg::Outbound {
                    patrol.first
                } else {
                    patrol.second
                };
                let remaining = (waypoint - from).with_y(0.).length();
                if remaining < patrol.previous_distance - 0.75 {
                    patrol.previous_distance = remaining;
                    patrol.stalled = 0.;
                } else {
                    patrol.stalled += dt;
                }
                if patrol.stalled > 5.5 || remaining < 2.5 {
                    patrol.leg = if remaining < 2.5 && patrol.leg == Leg::Outbound {
                        Leg::Crosswind
                    } else {
                        Leg::Approach
                    };
                    patrol.previous_distance = f64::INFINITY;
                    patrol.stalled = 0.;
                    patrol.survey_after = 0.;
                } else {
                    let goal = patrol.cruise(h, from, waypoint, dt, height);
                    self.travel(a, h, goal, dt, 16.5);
                }
            }
            Leg::Approach => {
                if !self.airborne {
                    patrol.recall(false);
                } else if pad_distance > 32. && !self.landing.active_at(patrol.pad) {
                    let pad = patrol.pad;
                    let approach_height = (13. + (pad_distance - 32.) * 0.45).min(height);
                    let goal = patrol.cruise(h, from, pad, dt, approach_height);
                    self.travel(a, h, goal, dt, 14.);
                } else {
                    self.approach_pad(a, h, patrol.pad, patrol.forward, dt);
                }
            }
            Leg::EnterMouth | Leg::WalkIn => {
                let delta = from - self.home;
                let depth = delta.dot(patrol.forward);
                let lateral = (delta - patrol.forward * depth).with_y(0.);
                let mouth = self.home + patrol.forward * 38.;
                // A restored creature can be directly above the nest on its
                // roof. Horizontal proximity alone does not put it inside.
                let runway_height = self.home.y
                    + (patrol.pad.y - self.home.y) * (depth / patrol.runway.max(1.)).clamp(0., 1.);
                let corridor = (-5.0..=40.).contains(&depth)
                    && lateral.length() < 5.
                    && (from.y - runway_height).abs() < 5.;
                if self.airborne {
                    patrol.recall(true);
                } else if delta.with_y(0.).length() < 2.4 && delta.y.abs() < 2.5 {
                    patrol.leg = Leg::Rest;
                    patrol.seconds = 44. + (a.id % 23) as f64;
                    patrol.detour = None;
                } else {
                    if patrol.leg == Leg::EnterMouth && (corridor || patrol.runway == 0.) {
                        patrol.leg = Leg::WalkIn;
                        patrol.detour = None;
                    }
                    let goal = if patrol.leg == Leg::WalkIn {
                        self.home
                    } else {
                        mouth
                    };
                    if patrol
                        .detour
                        .is_some_and(|p| (from - p).with_y(0.).length() < 0.60)
                    {
                        patrol.detour = None;
                    }
                    // Retain a usable waypoint until reached. Expiring it mid-pivot
                    // used to alternate the head between the obstruction and the route.
                    let advancing = self.walk(a, h, patrol.detour.unwrap_or(goal), dt, 4.6, false);
                    if !advancing && a.age >= patrol.detour_after {
                        // Only a blocked walker pays for a bounded local route.
                        // The ground graph uses this variant's collision volume.
                        patrol.detour_after = a.age + 1.5;
                        patrol.detour = crate::creatures::navigation::ground_waypoint(
                            a,
                            h,
                            goal.to_array(),
                            true,
                            None,
                        )
                        .map(DVec3::from_array);
                        // If the nest is below a sealed roof or a cliff, take
                        // the outside approach and enter with folded wings.
                        if patrol.detour.is_none()
                            && patrol.leg == Leg::EnterMouth
                            && !corridor
                            && flight::can_launch(a, h)
                        {
                            patrol.recall(true);
                            set_pose(a, B::Leap);
                        }
                    }
                }
            }
        }
        self.patrol = Some(patrol);
    }
}
