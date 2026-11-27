//! Territorial encounters. Targets and damage are authoritative; the replicated pose
//! carries the same locked aim and wind-up the client uses to show an attack.
use super::{Habitat, perception};
use bevy_math::DVec3;
use terra_core::{
    PlayerUuid,
    creatures::{
        Creature, CreatureBehavior as B, CreatureKind as K, NeedGoal, NeedKind, dragons as pose,
    },
};

/// Player UUIDs and persistent creature IDs occupy independent identity spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    Player(PlayerUuid),
    Mob(u64),
}

#[derive(Debug, Clone, Copy)]
pub struct Strike {
    pub dragon: u64,
    pub target: Target,
    pub damage: f32,
    pub effect: Option<terra_core::EffectKind>,
    pub impulse: [f32; 3],
}

mod sortie;
mod species;
use sortie::AttackRun;
mod upbringing;
pub(super) use upbringing::grow;
mod flight;
mod landing;
mod lineages;
mod patrol;
pub const ROAM_RADIUS: f64 = 500.0;
use patrol::Patrol;

pub struct Encounter {
    home: DVec3,
    keeper: Option<(DVec3, bool)>,
    pub target: Option<Target>,
    lost_for: f32,
    attack_number: u32,
    ground_casts: u8,
    attack_run: Option<AttackRun>,
    launch_retry: f32,
    cooldown: f32,
    airborne: bool,
    attack_pad: Option<(DVec3, DVec3, f64)>,
    prey_heading: Option<f32>,
    volley_hits: std::collections::BTreeSet<Target>,
    stopped_missiles: [bool; 3],
    flight_velocity: DVec3,
    landing: landing::Landing,
    last_sighting: Option<DVec3>,
    prey_velocity: DVec3,
    exit_course: Option<DVec3>,
    patrol: Option<Patrol>,
    swim_detour: Option<DVec3>,
    swim_retry: f64,
    course: DVec3,
    steer_time: f64,
}

impl Encounter {
    pub fn new(a: &Creature, nest: Option<([i64; 3], [i64; 2])>) -> Self {
        let home = nest.map_or_else(
            || DVec3::from_array(a.position),
            |(p, _)| DVec3::new(p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5),
        );
        Self {
            home,
            keeper: None,
            attack_pad: None,
            prey_heading: None,
            volley_hits: Default::default(),
            stopped_missiles: [false; 3],
            target: None,
            last_sighting: None,
            prey_velocity: DVec3::ZERO,
            exit_course: None,
            swim_detour: None,
            swim_retry: 0.,
            course: DVec3::ZERO,
            steer_time: 0.,
            lost_for: 0.0,
            attack_number: 0,
            ground_casts: 0,
            attack_run: None,
            launch_retry: 0.,
            cooldown: 1.0,
            patrol: nest.map(|(_, direction)| Patrol::new(direction, a, home)),
            landing: Default::default(),
            flight_velocity: DVec3::from_array(a.velocity.map(f64::from)),
            airborne: matches!(a.behavior, B::Fly | B::Leap | B::Land)
                || a.needs
                    .goal
                    .is_some_and(|g| g.kind == NeedKind::AirborneAttackAim),
        }
    }
    pub fn set_prey_heading(&mut self, yaw: Option<f32>) {
        self.prey_heading = yaw.filter(|yaw| yaw.is_finite());
    }
    pub fn step(
        &mut self,
        a: &mut Creature,
        h: &Habitat,
        dt: f32,
        targets: &[(Target, [f64; 3])],
        night: bool,
        friendly: bool,
    ) -> Vec<Strike> {
        let delta = f64::from(dt);
        a.age += delta;
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.launch_retry = (self.launch_retry - dt).max(0.);
        let old_timer = a.timer;
        a.timer = (a.timer - dt).max(0.0);
        a.velocity = [0.; 3];
        let from = DVec3::from_array(a.position);
        let eye = (from + DVec3::Y * a.size()[1] * 0.82).to_array();
        if !pose::flies(a.kind) && self.airborne {
            if a.kind == K::WaterDragon && h.wet(a.position) || on_ground(a, h) {
                self.airborne = false;
                set_pose(a, B::Rest);
            } else {
                // Bring airborne guardians from older saves safely back to their habitat.
                let falling = from - DVec3::Y * (6.0 * delta);
                if h.clear_swimmer_rotated(falling.to_array(), a.size(), f64::from(a.yaw)) {
                    translate(a, falling, delta);
                } else if let Some(point) =
                    h.supported_sized(a.position, a.size(), f64::from(a.yaw))
                {
                    translate(a, DVec3::from_array(point), delta);
                    self.airborne = false;
                }
                a.behavior = B::Land;
                return vec![];
            }
        }
        let engaged = self.target;
        let allowed = |p: [f64; 3]| {
            !friendly
                && !a.juvenile()
                && self.home.distance(DVec3::from_array(p))
                    < if self.patrol.is_some() {
                        ROAM_RADIUS
                    } else {
                        88.0
                    }
        };
        let current = targets.iter().find(|(id, p)| {
            Some(*id) == self.target && allowed(*p) && from.distance(DVec3::from_array(*p)) < 70.0
        });
        let mut seen = current.filter(|(_, p)| perception::visible(h, eye, *p));
        if seen.is_none() {
            self.lost_for += dt;
            if current.is_none() || self.lost_for > 2.4 {
                let radius = if a.kind == K::DarkDragon && !night && a.health >= a.kind.health() {
                    18.0
                } else {
                    terra_core::creatures::definitions::get(a.kind)
                        .combat
                        .unwrap()
                        .aggro_radius
                };
                seen = targets
                    .iter()
                    .filter(|(_, p)| allowed(*p) && from.distance(DVec3::from_array(*p)) < radius)
                    .filter(|(_, p)| perception::visible(h, eye, *p))
                    .min_by(|(left, p), (right, q)| {
                        matches!(left, Target::Mob(_))
                            .cmp(&matches!(right, Target::Mob(_)))
                            .then_with(|| {
                                from.distance_squared(DVec3::from_array(*p))
                                    .total_cmp(&from.distance_squared(DVec3::from_array(*q)))
                            })
                    });
                self.target = seen.map(|(id, _)| *id);
            }
        }
        if let Some((id, p)) = seen {
            let point = DVec3::from_array(*p);
            if Some(*id) != engaged || self.lost_for > 0. || self.last_sighting.is_none() {
                self.prey_velocity = DVec3::ZERO;
            } else if let Some(old) = self.last_sighting {
                let measured = (point - old) / delta.max(0.001);
                // Teleports are discontinuities, not leads for the next shot.
                self.prey_velocity = if measured.length_squared() < 18_f64.powi(2) {
                    self.prey_velocity.lerp(measured, 1. - (-5. * delta).exp())
                } else {
                    DVec3::ZERO
                };
            }
            self.last_sighting = Some(point);
            self.lost_for = 0.0;
        }
        if engaged != self.target {
            self.ground_casts = 0;
            self.attack_run = None;
            self.attack_pad = None;
            self.exit_course = None;
            self.steer_time = 0.;
            self.landing = Default::default();
            // Combat may interrupt takeoff or the final flare between animation ticks.
            if matches!(a.behavior, B::Land | B::Leap) {
                self.airborne = !on_ground(a, h);
            }
            a.needs.goal = None;
            if self.target.is_some() {
                set_pose(a, B::Emerge);
                return vec![];
            }
            self.last_sighting = None;
            self.prey_velocity = DVec3::ZERO;
            set_pose(a, if self.airborne { B::Fly } else { B::Wander });
            if let Some(patrol) = self.patrol.as_mut() {
                patrol.recall(self.airborne);
            }
        }
        if a.behavior == B::DragonBurrow {
            self.tunnel(a, h, delta);
            return vec![];
        }
        if pose::casting(a.behavior) && self.target.is_some() {
            let action = a.behavior;
            if self.airborne && matches!(action, B::Spit | B::Ink) {
                let forward = self.flight_velocity.with_y(0.0).normalize_or_zero();
                self.travel(a, h, from + forward * 7.0, delta, 5.0);
                a.behavior = action;
            }
            let impact = pose::duration(action) * (1. - pose::impact_phase(action));
            let form = pose::lineage(a.kind, a.variant);
            let strikes = if action == B::DragonVolley
                && let Some(form) = form
            {
                self.volley(a, h, form, targets, old_timer)
            } else if old_timer > impact && a.timer <= impact {
                let requires_footing = matches!(action, B::Tremor | B::Attack)
                    || matches!(action, B::Ink | B::DragonSurge) && a.kind != K::WaterDragon;
                if requires_footing && !on_ground(a, h) {
                    a.needs.goal = None;
                    set_pose(a, if self.airborne { B::Fly } else { B::Hunt });
                    self.cooldown = 0.7;
                    return vec![];
                }
                let hits = self.strikes(a, h, targets);
                if let Some(form) =
                    form.or((a.kind == K::DarkDragon).then_some(pose::Lineage::Shadow))
                {
                    lineages::resolve(a, h, form, self.airborne, targets, self.prey_heading);
                }
                hits
            } else {
                vec![]
            };
            if a.timer <= 0.0 {
                set_pose(a, if self.airborne { B::Fly } else { B::Hunt });
                a.needs.goal = None;
                if self.airborne {
                    if matches!(self.attack_run, Some(AttackRun::Pass)) {
                        self.attack_run = Some(AttackRun::Touchdown);
                    }
                    let yaw = f64::from(a.yaw);
                    let forward = DVec3::new(-yaw.sin(), 0., -yaw.cos());
                    let lateral = DVec3::new(-forward.z, 0., forward.x)
                        * if self.attack_number.is_multiple_of(2) {
                            1.
                        } else {
                            -1.
                        };
                    self.exit_course = Some(
                        DVec3::from_array(a.position)
                            + forward * 18.
                            + lateral * 7.
                            + DVec3::Y * 4.,
                    );
                }
                self.cooldown = if a.health < 0.4 * a.kind.health() {
                    1.15
                } else {
                    2.0
                };
            }
            return strikes;
        }
        if matches!(a.behavior, B::Leap | B::Land) {
            let rising = a.behavior == B::Leap;
            let phase = pose::phase(a.behavior, a.timer);
            let (s, c) = f64::from(a.yaw).sin_cos();
            let forward = DVec3::new(-s, 0., -c);
            // Weight shifts onto the hind legs before launch. The wings arrest the
            // descent before the feet settle and take the weight on landing.
            let lift = if rising {
                let push = ((phase - 0.18) / 0.28).clamp(0., 1.) as f64;
                (7.8 * push, 4.0 * push)
            } else {
                (-5.6 + 2.4 * f64::from(phase), 0.)
            };
            let floor = h.supported_sized(a.position, a.size(), f64::from(a.yaw));
            let mut next = from + (DVec3::Y * lift.0 + forward * lift.1) * delta;
            if !rising && let Some(footing) = floor {
                next.y = next.y.max(footing[1]);
            }
            let yaw = f64::from(a.yaw);
            let clear = if rising {
                flight::passage(h, a, from, next, yaw)
            } else {
                flight::flare_clear(a, h, from, next)
            };
            if clear {
                translate(a, next, delta);
            }
            let landed = !rising
                && floor.is_some_and(|p| {
                    a.position[1] <= p[1] + 0.10 && flight::clean_landing(a, h, p)
                });
            if !rising && !landed && floor.is_some_and(|p| a.position[1] <= p[1] + 0.1) {
                self.airborne = true;
                self.landing.reject();
                self.steer_time = 0.;
                self.flight_velocity = DVec3::ZERO;
                set_pose(a, B::Fly);
                return vec![];
            }
            if !rising && floor.is_none() && a.velocity != [0.; 3] {
                // Keep the flare until the legs can reach the ground. High-altitude
                // approaches take longer than the touchdown animation.
                a.timer = a.timer.max(pose::duration(B::Land) * 0.46);
            }
            if rising && a.timer <= 0.0 && !flight::passage(h, a, from, from + DVec3::Y, yaw) {
                self.airborne = floor.is_none();
                set_pose(a, if self.airborne { B::Fly } else { B::Wander });
                self.cooldown = 1.;
                return vec![];
            }
            if a.timer <= 0.0 && (rising || landed) {
                self.airborne = rising;
                self.landing = Default::default();
                self.flight_velocity = if rising {
                    forward * 5.0 + DVec3::Y * 2.0
                } else {
                    DVec3::ZERO
                };
                set_pose(a, if rising { B::Fly } else { B::Wander });
            } else if !rising && a.timer <= 0.0 && a.velocity == [0.; 3] && !landed {
                // Something entered the descent path, or the next section unloaded.
                self.airborne = true;
                self.landing.reject();
                self.steer_time = 0.;
                self.flight_velocity = DVec3::ZERO;
                set_pose(a, B::Fly);
            }
            return vec![];
        }
        if a.behavior == B::Emerge && a.timer > 0.0 {
            return vec![];
        }
        if friendly && a.needs.dragon.is_some() {
            self.care(a, h, dt, night);
            return vec![];
        }
        let Some((_, point)) = seen else {
            if self.target.is_some() {
                // Search the last observed position briefly without tracking a
                // player through rock. Keep the wings moving even after losing sight.
                if let Some(last) = self.last_sighting {
                    let goal = if self.airborne {
                        last + DVec3::Y * 9.
                    } else {
                        last
                    };
                    self.travel(a, h, goal, delta, if self.airborne { 8. } else { 2.4 });
                }
                return vec![];
            }
            if !pose::flies(a.kind) {
                self.territory(a, h, delta);
                return vec![];
            }
            if self.patrol.is_none() && !friendly && !a.juvenile() && flight::can_launch(a, h) {
                self.patrol = Some(Patrol::open_country(a, self.home));
            }
            if self.patrol.is_some() {
                self.patrol(a, h, delta, night);
                return vec![];
            }
            let to_home = from.distance(self.home);
            let cycle = (a.age + (a.id % 37) as f64).rem_euclid(68.0);
            if self.airborne {
                if (self.home - from).with_y(0.0).length() < 9.0
                    || self.landing.active_at(self.home)
                {
                    self.approach_pad(a, h, self.home, DVec3::NEG_Z, delta);
                } else {
                    self.travel(a, h, self.home + DVec3::Y * 4.0, delta, 6.0);
                }
            } else if to_home > 18.0 || (32.0..51.0).contains(&cycle) {
                let angle = (a.age * 0.11) + (a.id % 7) as f64;
                let goal = if to_home > 18.0 {
                    self.home
                } else {
                    self.home + DVec3::new(angle.sin() * 7.0, 0.0, angle.cos() * 7.0)
                };
                self.travel(a, h, goal, delta, 2.3);
            } else {
                a.behavior = if cycle < 17.0 && !(a.kind == K::DarkDragon && night) {
                    B::Sleep
                } else {
                    B::Rest
                };
            }
            return vec![];
        };
        let target = DVec3::from_array(*point);
        let dist = from.distance(target);
        if let Some((pad, direction, time)) = self.attack_pad {
            if self.airborne && time < 10. {
                self.attack_pad = Some((pad, direction, time + delta));
                self.approach_pad(a, h, pad, direction, delta);
                return vec![];
            }
            self.attack_pad = None;
            if self.airborne {
                // A moving target over an inaccessible cliff cannot pin the guardian in a holding pattern.
                self.attack_number = self.attack_number.wrapping_add(1);
                self.attack_run = None;
                self.cooldown = 0.8;
            }
        }
        if let Some(exit) = self.exit_course {
            if self.airborne && self.cooldown > 0.45 && from.distance_squared(exit) > 4. {
                self.travel(a, h, exit, delta, 13.);
                return vec![];
            }
            self.exit_course = None;
        }
        if pose::flies(a.kind)
            && !self.airborne
            && let Some(doorway) = self
                .patrol
                .as_ref()
                .and_then(|p| p.chase_waypoint(self.home, from, target))
        {
            self.travel(a, h, doorway, delta, a.kind.speed());
            return vec![];
        }
        if self.steer_attack_run(a, h, target, delta) {
            return vec![];
        }
        let swimming = a.kind == K::WaterDragon && h.wet(a.position);
        if swimming {
            Self::turn_swimmer(a, h, target, delta * 1.7);
        } else if !self.airborne {
            turn(a, target, delta * 2.7);
        }
        if self.cooldown <= 0.0
            && dist < 45.0
            && facing(a, target)
            && (!self.airborne
                || self.attack_run.is_some()
                || !self.attack_number.is_multiple_of(4)
                || dist > 24.0)
        {
            if a.kind == K::EarthDragon
                && self.attack_number % 4 == 1
                && dist > 6.
                && self.begin_tunnel(a, h, target)
            {
                self.attack_number = self.attack_number.wrapping_add(1);
                return vec![];
            }
            let action = pose::lineage(a.kind, a.variant).map_or_else(
                || self.choose(a.kind, dist),
                |form| {
                    lineages::choose(
                        form,
                        self.attack_number,
                        dist,
                        a.health < a.kind.health() * 0.92,
                    )
                },
            );
            let clustered = targets.iter().any(|(id, p)| {
                Some(*id) != self.target
                    && DVec3::from_array(*p).distance_squared(target) < 5.0_f64.powi(2)
            });
            let action = if clustered && self.attack_number % 3 == 1 {
                B::Ink
            } else {
                action
            };
            let action = if a.kind == K::DarkDragon && self.attack_number % 4 == 2 {
                B::DragonSurge
            } else {
                action
            };
            let action = match self.attack_run {
                Some(AttackRun::Pass) => B::Spit,
                Some(AttackRun::Touchdown) if !self.airborne && dist < 13. => B::Tremor,
                _ => action,
            };
            let in_water = a.kind == K::WaterDragon && h.wet(a.position);
            // The tidal geyser and whirlpool rise through water, the other species root these casts at the ground.
            let ground_spell = matches!(action, B::Tremor | B::Attack)
                || !in_water && matches!(action, B::Ink | B::DragonSurge);
            let ground_aim = matches!(action, B::Ink | B::DragonSurge) && ground_spell;
            let target_floor = ground_aim
                .then(|| h.paw_surface(target.to_array(), 3.0))
                .flatten();
            if ground_spell && self.airborne {
                let outward = (from - target).with_y(0.).normalize_or(DVec3::Z);
                let radius = (from - target).with_y(0.).length().clamp(7., 11.);
                let pad = target + outward * radius;
                self.attack_pad = Some((pad, outward, 0.));
                self.approach_pad(a, h, pad, outward, delta);
                return vec![];
            }
            let action = if ground_spell
                && (self.airborne || !on_ground(a, h) || (ground_aim && target_floor.is_none()))
            {
                B::Spit
            } else {
                action
            };
            if !self.airborne {
                self.ground_casts = self.ground_casts.saturating_add(1);
                self.attack_run = None;
            }
            self.volley_hits.clear();
            self.stopped_missiles = [false; 3];
            self.attack_number = self.attack_number.wrapping_add(1);
            let aim = if matches!(action, B::Tremor | B::Attack | B::DragonWard) {
                a.position
            } else if ground_aim
                && action != B::Spit
                && let Some(floor) = target_floor
            {
                target.with_y(floor).to_array()
            } else {
                // Read lateral motion before committing, then lock the warned
                // location. A mid-cast dodge still works for every bloodline.
                let lead = self.prey_velocity
                    * if action == B::DragonVolley {
                        0.62
                    } else {
                        0.38
                    };
                let aimed = target + lead.clamp_length_max(3.0);
                if perception::visible(h, eye, aimed.to_array()) {
                    aimed.to_array()
                } else {
                    *point
                }
            };
            a.needs.goal = Some(NeedGoal {
                kind: if self.airborne {
                    NeedKind::AirborneAttackAim
                } else {
                    NeedKind::AttackAim
                },
                position: aim.map(|v| v.floor() as i64),
            });
            set_pose(a, action);
        } else if !self.airborne
            && target.y > from.y + a.size()[1] + 2.
            && dist > 9.0
            && flight::can_launch(a, h)
        {
            set_pose(a, B::Leap);
        } else if self.airborne
            && self.attack_run.is_none()
            && self.attack_number.is_multiple_of(4)
            && dist < 25.0
            && h.supported_sized([from.x, target.y + 0.3, from.z], a.size(), f64::from(a.yaw))
                .is_some_and(|g| flight::clean_landing(a, h, g))
        {
            self.approach_pad(
                a,
                h,
                DVec3::new(from.x, target.y, from.z),
                DVec3::NEG_Z,
                delta,
            );
        } else {
            let offset = if self.airborne {
                let direction = (target - from).with_y(0.0).normalize_or_zero();
                let side = if self.attack_number % 4 < 2 { 1. } else { -1. };
                let (arc, height) = match a.kind {
                    K::EarthDragon => (7.0, 5.5),
                    K::WaterDragon => (13.0, 6.0),
                    _ => (14.0, 11.0),
                };
                // Circle out while recovering, then straighten the attack run:
                // an endless tangent orbit never points the mouth at the target.
                let bank = (self.cooldown / 1.2).clamp(0., 1.) as f64;
                DVec3::new(
                    -direction.z * arc * side * bank,
                    // Gain separation during recovery; descend into range and
                    // line up the head before committing to the next strike.
                    height + (9. + 0.20 * dist.min(40.)) * bank,
                    direction.x * arc * side * bank,
                )
            } else {
                DVec3::ZERO
            };
            if swimming && dist < 13. && self.cooldown > 0. {
                let outward = (from - target).with_y(0.).normalize_or(DVec3::Z);
                let side = if self.attack_number % 4 < 2 { 1. } else { -1. };
                let flank =
                    target + outward * 9. + DVec3::new(-outward.z, -0.3, outward.x) * side * 6.;
                self.travel(a, h, flank, delta, a.kind.speed());
            } else if dist > 9.0 || self.airborne {
                self.travel(
                    a,
                    h,
                    target + offset,
                    delta,
                    if self.airborne { 12.0 } else { a.kind.speed() },
                );
            } else {
                a.behavior = B::Hunt;
            }
        }
        vec![]
    }

    fn choose(&self, kind: K, distance: f64) -> B {
        match kind {
            K::EarthDragon if distance > 15.0 => B::Spit,
            K::EarthDragon if self.attack_number % 3 == 1 => B::Tremor,
            K::EarthDragon => B::Attack,
            _ if self.attack_number % 3 == 2 => B::Ink,
            _ if distance < 13.0 && !self.attack_number.is_multiple_of(3) => B::Tremor,
            _ => B::Spit,
        }
    }
    fn travel(&mut self, a: &mut Creature, h: &Habitat, goal: DVec3, dt: f64, speed: f64) {
        let from = DVec3::from_array(a.position);
        let direction = goal - from;
        if direction.length_squared() < 0.10 {
            return;
        }
        if self.airborne {
            self.fly(a, h, goal, dt, speed);
            return;
        }
        if a.kind == K::WaterDragon && self.dive(a, h, goal, dt, speed) {
            return;
        }
        self.walk(a, h, goal, dt, speed, true);
    }

    /// Returns true while walking or turning into an available route.
    fn walk(
        &mut self,
        a: &mut Creature,
        h: &Habitat,
        goal: DVec3,
        dt: f64,
        speed: f64,
        allow_launch: bool,
    ) -> bool {
        let from = DVec3::from_array(a.position);
        let direction = goal - from;
        turn(a, goal, dt * 2.0);
        let yaw = f64::from(a.yaw);
        let forward = DVec3::new(-yaw.sin(), 0., -yaw.cos());
        let horizontal = direction.with_y(0.);
        // Plant and pivot before setting off. Walking directly towards a target
        // behind the pelvis sent the animal sideways or backwards down stairs.
        let alignment = forward.dot(horizontal.normalize_or_zero());
        if alignment < 0.35 {
            a.velocity = [0.; 3];
            return horizontal.length_squared() > 0.01;
        }
        let step = horizontal.length().min(speed * dt * alignment);
        let next = from + forward * step;
        let scale = f64::from(pose::model_scale(a.kind, a.variant)) * a.growth_scale();
        let candidate = h
            .supported_sized(next.to_array(), a.size(), yaw)
            .filter(|&p| walkable_treads(h, p, yaw, scale));
        if let Some(next) = candidate {
            translate(a, DVec3::from_array(next), dt);
            a.behavior = if self.target.is_some() {
                B::Hunt
            } else {
                B::Wander
            };
            true
        } else if allow_launch && flight::can_launch(a, h) {
            set_pose(a, B::Leap);
            true
        } else {
            false
        }
    }
    fn strikes(&self, a: &Creature, h: &Habitat, targets: &[(Target, [f64; 3])]) -> Vec<Strike> {
        let Some(goal) = a.needs.goal else {
            return vec![];
        };
        let center = DVec3::from_array(a.position);
        let muzzle = DVec3::from_array(pose::muzzle(a));
        let aim = DVec3::from_array(goal.position.map(|v| v as f64 + 0.5));
        let damage = pose::cast_damage(a);
        let form = pose::lineage(a.kind, a.variant);
        if form.is_some()
            && matches!(a.behavior, B::DragonSurge | B::Ink)
            && !perception::visible(h, muzzle.to_array(), aim.to_array())
        {
            return vec![];
        }
        targets
            .iter()
            .filter_map(|(id, p)| {
                let p = DVec3::from_array(*p);
                let custom = form.and_then(|form| lineages::strike(a, form, center, aim, p));
                let (origin, hit, force) = if let Some(spell) = custom {
                    (spell.origin, spell.in_range, spell.push)
                } else {
                    match a.behavior {
                        B::Spit => {
                            let dir = (aim - muzzle).normalize_or_zero();
                            let ray = p - muzzle;
                            let along = ray.dot(dir);
                            (
                                muzzle,
                                (0.0..=52.0).contains(&along)
                                    && (ray - dir * along).length() < 1.8 + along * 0.047,
                                1.5,
                            )
                        }
                        B::Ink => (
                            aim + DVec3::Y * 1.5,
                            aim.distance(p) < 7.5,
                            if a.kind == K::WaterDragon { -7.0 } else { 2.0 },
                        ),
                        B::Tremor | B::Attack => (
                            center + DVec3::Y * 2.0,
                            (center - p).with_y(0.0).length() < 13.0
                                && (center.y - p.y).abs() < 7.0,
                            8.0,
                        ),
                        _ => return None,
                    }
                };
                if !hit || !perception::visible(h, origin.to_array(), p.to_array()) {
                    return None;
                }
                let dir = (p - origin).with_y(0.0).normalize_or_zero() * force;
                Some(Strike {
                    dragon: a.id,
                    target: *id,
                    damage: damage * if a.behavior == B::Ink { 0.65 } else { 1.0 },
                    effect: custom
                        .and_then(|spell| spell.effect)
                        .or(match (a.kind, a.behavior) {
                            (K::DarkDragon, B::Ink | B::Spit) => {
                                Some(terra_core::EffectKind::Rotten)
                            }
                            (K::FireDragon, B::Spit | B::Ink | B::Tremor) => {
                                Some(terra_core::EffectKind::OnFire)
                            }
                            (K::WaterDragon, B::Tremor) => Some(terra_core::EffectKind::Freeze),
                            (K::WaterDragon, B::Ink) => Some(terra_core::EffectKind::Slow),
                            _ => None,
                        }),
                    impulse: [
                        dir.x as f32,
                        if let Some(spell) = custom {
                            spell.lift
                        } else if a.behavior == B::Attack || a.behavior == B::Tremor {
                            5.0
                        } else {
                            1.0
                        },
                        dir.z as f32,
                    ],
                })
            })
            .collect()
    }
}

/// The broad collision box can still rest on the very last block behind the
/// dragon. Both axles need a reachable tread before committing a walking step.
fn walkable_treads(h: &Habitat, p: [f64; 3], yaw: f64, scale: f64) -> bool {
    use terra_core::creatures::dragon_pose::Joint;
    let (sin, cos) = yaw.sin_cos();
    [false, true].into_iter().all(|hind| {
        [-1, 1].into_iter().any(|side| {
            let paw = Joint::Paw { side, hind }.pivot();
            let x = f64::from(paw.x) * scale;
            let z = f64::from(paw.z - 0.65) * scale;
            let tread = [p[0] + cos * x + sin * z, p[1], p[2] - sin * x + cos * z];
            h.paw_surface(tread, (scale * 5.0).max(1.1)).is_some()
        })
    })
}

fn on_ground(a: &Creature, h: &Habitat) -> bool {
    h.supported_sized(a.position, a.size(), f64::from(a.yaw))
        .is_some_and(|floor| (a.position[1] - floor[1]).abs() < 0.18)
}

fn set_pose(a: &mut Creature, b: B) {
    a.behavior = b;
    a.timer = pose::duration(b);
}
fn translate(a: &mut Creature, p: DVec3, dt: f64) {
    a.velocity = ((p - DVec3::from_array(a.position)) / dt.max(0.001))
        .clamp_length_max(30.0)
        .as_vec3()
        .to_array();
    a.position = p.to_array();
}
fn turn(a: &mut Creature, p: DVec3, max: f64) {
    let d = DVec3::from_array(a.position) - p;
    if d.x * d.x + d.z * d.z < 0.0025 {
        return;
    }
    let desired = d.x.atan2(d.z);
    let yaw = f64::from(a.yaw);
    let diff = (desired - yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI;
    a.yaw = (yaw + diff.clamp(-max, max)).rem_euclid(std::f64::consts::TAU) as f32;
}
fn facing(a: &Creature, p: DVec3) -> bool {
    let d = (p - DVec3::from_array(a.position))
        .with_y(0.0)
        .normalize_or_zero();
    let (s, c) = f64::from(a.yaw).sin_cos();
    d.x * (-s) + d.z * (-c) > 0.75
}

#[cfg(test)]
mod tests;
