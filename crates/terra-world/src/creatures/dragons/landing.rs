//! Survey loaded treads around the requested touchdown point. Keep flying a
//! holding pattern while the ground is obstructed or still streaming in.
use super::*;

#[derive(Default)]
pub(super) struct Landing {
    anchor: Option<DVec3>,
    site: Option<DVec3>,
    next_probe: f64,
    cursor: usize,
    best_distance: f64,
    stalled: f64,
    hold_angle: f64,
}

impl Landing {
    pub(super) fn active_at(&self, center: DVec3) -> bool {
        self.anchor
            .is_some_and(|p| p.distance_squared(center) <= 36.)
    }

    fn sample(a: &Creature, h: &Habitat, site: DVec3) -> Option<DVec3> {
        let x = site.x.floor() as i64;
        let z = site.z.floor() as i64;
        let surface = h
            .chunks
            .surface_column_at(x, z)
            .map_or_else(|| h.terrain.surface_height(site.x, site.z), |c| c.height);
        // The runway can be excavated below the original terrain. Probe both
        // the known pad elevation and the loaded terrain around its shoulders.
        [site.y, surface].into_iter().find_map(|level| {
            let top = h.paw_surface([site.x, level + 3., site.z], 7.)?;
            let feet = [site.x, top, site.z];
            flight::clean_landing(a, h, feet).then_some(DVec3::from_array(feet))
        })
    }

    pub(super) fn reject(&mut self) {
        self.site = None;
        self.stalled = 0.;
        self.best_distance = f64::INFINITY;
        self.next_probe = 0.6;
    }
}

impl Encounter {
    pub(super) fn approach_pad(
        &mut self,
        a: &mut Creature,
        h: &Habitat,
        center: DVec3,
        forward: DVec3,
        dt: f64,
    ) {
        let from = DVec3::from_array(a.position);
        let search = &mut self.landing;
        let side = DVec3::new(-forward.z, 0., forward.x);
        if search
            .anchor
            .is_none_or(|p| p.distance_squared(center) > 36.)
        {
            let radial = (from - center).with_y(0.);
            *search = Landing {
                anchor: Some(center),
                hold_angle: radial.dot(side).atan2(radial.dot(forward)),
                best_distance: f64::INFINITY,
                ..Default::default()
            };
        }
        let center = search.anchor.unwrap();
        search.next_probe -= dt;
        if search.next_probe <= 0. {
            // Revalidate at the current heading and after any world edits.
            if let Some(site) = search.site
                && !flight::clean_landing(a, h, site.to_array())
            {
                search.reject();
            }
            if search.site.is_none() {
                // Sample a progressively wider fan on the outer side of the
                // entrance. Survey time is bounded even over open water.
                for _ in 0..4 {
                    let i = search.cursor;
                    search.cursor = (i + 1) % 29;
                    let offset = if i == 0 {
                        DVec3::ZERO
                    } else {
                        let bearing = [0., 1., -1., 2., -2., 3., -3.][(i - 1) % 7]
                            * std::f64::consts::FRAC_PI_6;
                        let radius = [8., 16., 25., 36.][(i - 1) / 7];
                        let (s, c) = f64::sin_cos(bearing);
                        (forward * c + side * s) * radius
                    };
                    if let Some(site) = Landing::sample(a, h, center + offset) {
                        search.site = Some(site);
                        search.best_distance = f64::INFINITY;
                        search.stalled = 0.;
                        break;
                    }
                }
            }
            search.next_probe = 1.2;
        }
        if let Some(site) = search.site {
            let remaining = from.distance(site);
            if remaining < search.best_distance - 0.65 {
                search.best_distance = remaining;
                search.stalled = 0.;
            } else {
                search.stalled += dt;
            }
            if search.stalled < 6. {
                let horizontal = (from - site).with_y(0.).length();
                // Commit only over the actual support under the feet. A pad
                // beside the dragon cannot catch it if it is still over water.
                if horizontal < 2.2 && (from.y - site.y).abs() < 7.5 {
                    let below = [from.x, site.y, from.z];
                    if flight::clean_landing(a, h, below) {
                        self.flight_velocity = DVec3::ZERO;
                        self.steer_time = 0.;
                        set_pose(a, B::Land);
                        return;
                    }
                }
                let height = (horizontal * 0.42 + 1.4).min(18.);
                self.travel(a, h, site + DVec3::Y * height, dt, 9.);
                return;
            }
            search.reject();
        }
        search.hold_angle = (search.hold_angle + dt * 0.34).rem_euclid(std::f64::consts::TAU);
        let (s, c) = (search.hold_angle + 0.65).sin_cos();
        let hold = center + forward * (11. + 16. * c) + side * (19. * s);
        let terrain = h.terrain.surface_height(hold.x, hold.z);
        self.travel(a, h, hold.with_y(terrain.max(center.y) + 14.), dt, 9.);
    }
}
