use super::{B, Creature, DVec3, Encounter, Habitat, NeedKind, flight, set_pose, turn};
use crate::creatures::needs;
use terra_core::creatures::{
    breeding,
    dragon_life::{DragonLife, DragonStage},
};

pub(in crate::creatures) fn grow(a: &mut Creature, habitat: &Habitat, days: f64) {
    if !a.kind.is_dragon() || !breeding::is_birth_id(a.id) {
        return;
    }
    let config = habitat.terrain.config();
    let mut life = a.needs.dragon.unwrap_or_else(|| DragonLife::legacy(a));
    let previous = life.months;
    life.advance(days, config.seasons);
    a.needs.dragon = Some(life);
    if life.months > previous && !habitat.clear_swimmer_rotated(a.position, a.size(), a.yaw.into())
    {
        a.needs.dragon.as_mut().unwrap().months = previous;
    }
}

impl Encounter {
    pub fn set_keeper(&mut self, position: Option<[f64; 3]>, follow: bool) {
        self.keeper = position.map(|p| (DVec3::from_array(p), follow));
    }

    pub(super) fn care(&mut self, a: &mut Creature, h: &Habitat, dt: f32, night: bool) {
        let from = DVec3::from_array(a.position);
        let life = a.needs.dragon.unwrap();
        let stage = a.dragon_stage();
        let nest = life.nest.map_or(self.home, |p| {
            DVec3::new(p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5)
        });
        a.needs.food = (a.needs.food - dt * 0.003).max(0.);
        a.needs.water = (a.needs.water - dt * 0.005).max(0.);
        let near_keeper = self.keeper.is_some_and(|(p, _)| from.distance(p) < 9.);
        a.needs.search_in = (a.needs.search_in - dt).max(0.);
        let search = a.needs.search_in <= 0.;
        if search {
            a.needs.search_in = 5.;
            let [x, y, z] = a.position.map(|v| v.floor() as i64);
            let head = y + a.size()[1].ceil() as i64;
            a.needs.dragon.as_mut().unwrap().sheltered = from.distance(nest) <= 4.
                && life.nest.is_some_and(|[nx, ny, nz]| {
                    terra_core::creatures::dragon_eggs::supports_nest(
                        a.kind,
                        h.block([nx, ny - 1, nz]),
                    )
                })
                || (head..=head + 12).any(|roof| {
                    h.block([x, roof, z])
                        .and_then(terra_voxel::MaterialId::from_raw)
                        .is_some_and(|m| m.is_solid())
                });
        }
        if near_keeper {
            a.needs.dragon.as_mut().unwrap().affection = (life.affection + dt * 0.008).min(100.);
        }
        if !self.airborne && a.behavior == B::Graze && a.timer > 0. {
            return;
        }
        if (a.needs.water < a.kind.water() * 0.75
            || a.behavior == B::Drink && a.needs.water < a.kind.water())
            && !self.airborne
        {
            if a.needs.goal.is_none() && search {
                a.needs.goal = needs::find_water(a, h, 6);
            }
            if let Some(water) = a.needs.goal.filter(|g| g.kind == NeedKind::Water) {
                let point = DVec3::from_array(water.position.map(|v| v as f64 + 0.5));
                if !needs::water_at(h, water.position) {
                    a.needs.goal = None;
                } else if from.distance(point) < a.size()[2] * 0.5 + 2. {
                    turn(a, point, f64::from(dt) * 2.);
                    a.behavior = B::Drink;
                    a.needs.water = (a.needs.water + dt * 2.5).min(a.kind.water());
                    return;
                } else {
                    self.travel(a, h, point, dt.into(), 2.5 * a.movement_scale());
                    return;
                }
            }
        } else {
            a.needs.goal = None;
        }

        let leader = self.keeper.filter(|(_, follow)| *follow).map(|(p, _)| p);
        let cycle = (a.age + (a.id % 29) as f64).rem_euclid(58.);
        if self.airborne {
            if night
                || cycle > 48.
                || leader.is_none()
                || leader.is_some_and(|p| from.distance(p) < 8.)
            {
                self.approach_pad(a, h, leader.unwrap_or(nest), DVec3::NEG_Z, dt.into());
            } else if let Some(p) = leader {
                self.travel(
                    a,
                    h,
                    p + DVec3::Y * 6.,
                    dt.into(),
                    6. + f64::from(life.training) * 0.05,
                );
            }
            return;
        }
        if let Some(p) = leader
            && from.distance(p) > a.size()[2] * 0.6 + 3.
        {
            if stage.can_fly()
                && (p.y - from.y > 5. || from.distance(p) > 24.)
                && flight::can_launch(a, h)
            {
                set_pose(a, B::Leap);
            } else {
                self.travel(
                    a,
                    h,
                    p,
                    dt.into(),
                    (3.6 + f64::from(life.training) * 0.015) * a.movement_scale(),
                );
            }
            if a.velocity[0].hypot(a.velocity[2]) > 0.25 {
                a.needs.dragon.as_mut().unwrap().training = (life.training + dt * 0.05).min(100.);
            }
        } else if night
            && !life.sheltered
            && leader.is_none()
            && from.distance(nest) > 4.
            && from.distance(nest) < 64.
        {
            self.travel(a, h, nest, dt.into(), 2.2 * a.movement_scale());
        } else if night || stage == DragonStage::Baby && cycle < 14. {
            a.behavior = B::Sleep;
        } else if stage == DragonStage::Adult && cycle < 1.4 && self.cooldown <= 0. {
            set_pose(a, B::Emerge);
            self.cooldown = 12.;
        } else {
            a.behavior = B::Rest;
        }
    }
}
