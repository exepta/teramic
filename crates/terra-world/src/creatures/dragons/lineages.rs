//! The large bloodlines telegraph their spells against a locked world-space aim.
use super::*;
use pose::Lineage;

pub(super) fn choose(form: Lineage, step: u32, distance: f64, injured: bool) -> B {
    use B::*;
    let pattern = match form {
        Lineage::Shadow => [Spit, DragonVolley, DragonSurge, Tremor, Ink, DragonWard],
        Lineage::Tidal => [Spit, DragonVolley, Ink, DragonSurge, Tremor, DragonWard],
        Lineage::Stone => [DragonVolley, Ink, DragonSurge, Attack, Tremor, DragonWard],
    };
    match pattern[step as usize % pattern.len()] {
        Attack | Tremor if distance > 17. => DragonVolley,
        DragonWard if !injured => DragonVolley,
        other => other,
    }
}

pub(super) fn resolve(
    a: &mut Creature,
    h: &Habitat,
    form: Lineage,
    airborne: bool,
    targets: &[(Target, [f64; 3])],
    prey_yaw: Option<f32>,
) {
    if a.behavior == B::DragonWard && form == Lineage::Tidal {
        a.health = (a.health + 0.035 * a.kind.health()).min(a.kind.health());
    }
    if a.behavior != B::DragonSurge || form != Lineage::Shadow {
        return;
    }
    let Some(goal) = a.needs.goal else { return };
    let aim = DVec3::from_array(goal.position.map(|v| v as f64 + 0.5));
    let from = DVec3::from_array(a.position);
    let forward = (aim - from).with_y(0.).normalize_or_zero();
    let behind = prey_yaw.map_or(forward, |yaw| {
        let (s, c) = f64::from(yaw).sin_cos();
        DVec3::new(s, 0., c)
    });
    let side = DVec3::new(-behind.z, 0., behind.x);
    // The gate opens behind the locked target. Two nearby rearward offsets handle a narrow ledge.
    // The dragon never materialises inside unloaded
    // terrain, a player or the cave wall. Failed landings simply fizzle.
    for sign in [0., 1., -1.] {
        let flank = aim + behind * 9. + side * sign * 5.;
        let yaw = (flank.x - aim.x).atan2(flank.z - aim.z);
        let destination = if airborne {
            let pos = flank.with_y(from.y.max(aim.y + 7.));
            flight::passage(h, a, pos, pos, yaw).then_some(pos.to_array())
        } else {
            [0., -1., 1., -2., 2., -3., 3.]
                .into_iter()
                .find_map(|offset| {
                    h.ground_sized((flank + DVec3::Y * offset).to_array(), a.size(), yaw)
                })
        };
        if let Some(p) = destination
            && targets.iter().all(|(_, pos)| {
                let delta = DVec3::from_array(*pos) - DVec3::from_array(p);
                let size = a.size();
                delta.y < -1.9
                    || delta.y > size[1]
                    || delta.with_y(0.).length() > 0.5 * size[0].hypot(size[2]) + 0.4
            })
            && perception::visible(
                h,
                (from + DVec3::Y * 3.).to_array(),
                [p[0], p[1] + 3., p[2]],
            )
        {
            a.position = p;
            a.yaw = yaw.rem_euclid(std::f64::consts::TAU) as f32;
            break;
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct SpellImpact {
    pub origin: DVec3,
    pub in_range: bool,
    pub push: f64,
    pub lift: f32,
    pub effect: Option<terra_core::EffectKind>,
}

/// Local radial/target spells return their actual impulse and debuff. Breath
/// and the travelling missiles use separate trajectory tests.
pub(super) fn strike(
    a: &Creature,
    form: Lineage,
    center: DVec3,
    aim: DVec3,
    p: DVec3,
) -> Option<SpellImpact> {
    use terra_core::EffectKind::*;
    let local = p - center;
    let near = p - aim;
    let radius = |v: DVec3, r: f64, height: f64| v.with_y(0.).length() < r && v.y.abs() < height;
    let (origin, in_range, push, lift, effect) = match (form, a.behavior) {
        (_, B::DragonWard) | (Lineage::Shadow, B::DragonSurge) => return None,
        (Lineage::Tidal, B::DragonSurge) => (aim, radius(near, 4.7, 9.), 2., 11., Some(Slow)),
        (Lineage::Stone, B::DragonSurge) => (aim, radius(near, 7., 5.), 2., 1., Some(Slow)),
        (Lineage::Tidal, B::Ink) => (aim, radius(near, 8., 6.), -10., 1., Some(Slow)),
        (Lineage::Shadow | Lineage::Stone, B::Ink) => {
            let end = (aim - center).with_y(0.);
            let t = (local.with_y(0.).dot(end) / end.length_squared().max(0.001)).clamp(0., 1.);
            let point = center.lerp(aim, t);
            (
                point + DVec3::Y,
                radius(p - point, 3.2, 4.5),
                4.,
                5.,
                Some(if form == Lineage::Shadow {
                    Rotten
                } else {
                    Bleeding
                }),
            )
        }
        (_, B::Tremor | B::Attack) => (center + DVec3::Y, radius(local, 15.5, 6.5), 10., 5.5, None),
        _ => return None,
    };
    Some(SpellImpact {
        origin,
        in_range,
        push,
        lift,
        effect,
    })
}

impl Encounter {
    pub(super) fn volley(
        &mut self,
        a: &Creature,
        h: &Habitat,
        form: Lineage,
        targets: &[(Target, [f64; 3])],
        previous: f32,
    ) -> Vec<Strike> {
        let Some(goal) = a.needs.goal else {
            return vec![];
        };
        let progress = |timer| ((pose::phase(a.behavior, timer) - 0.30) / 0.42).clamp(0., 1.);
        let (start, end) = (progress(previous), progress(a.timer));
        if end <= start {
            return vec![];
        }
        let muzzle = DVec3::from_array(pose::muzzle(a));
        let aim = DVec3::from_array(goal.position.map(|v| v as f64 + 0.5));
        let mut hits = vec![];
        for index in 0..3u8 {
            let stopped = &mut self.stopped_missiles[index as usize];
            if *stopped {
                continue;
            }
            let point = |t| {
                muzzle
                    + pose::missile_path(
                        form,
                        index,
                        t,
                        bevy_math::Vec3::ZERO,
                        (aim - muzzle).as_vec3(),
                    )
                    .as_dvec3()
            };
            let steps = ((end - start) * 24.).ceil().max(1.) as u32;
            let mut prev = point(start);
            for n in 1..=steps {
                let next = point(start + (end - start) * n as f32 / steps as f32);
                if !perception::visible(h, prev.to_array(), next.to_array()) {
                    *stopped = true;
                    break;
                }
                let path = next - prev;
                for &(target, at) in targets {
                    let p = DVec3::from_array(at);
                    let u = ((p - prev).dot(path) / path.length_squared().max(1e-8)).clamp(0., 1.);
                    let nearest = prev + path * u;
                    if p.distance_squared(nearest) < 2.6
                        && perception::visible(h, nearest.to_array(), at)
                        && self.volley_hits.insert(target)
                    {
                        let knock = path.with_y(0.).normalize_or_zero() * 3.;
                        hits.push(Strike {
                            dragon: a.id,
                            target,
                            damage: pose::cast_damage(a) * 0.82,
                            effect: Some(match form {
                                Lineage::Shadow => terra_core::EffectKind::Rotten,
                                Lineage::Tidal => terra_core::EffectKind::Slow,
                                Lineage::Stone => terra_core::EffectKind::Bleeding,
                            }),
                            impulse: [knock.x as f32, 2., knock.z as f32],
                        });
                    }
                }
                prev = next;
            }
        }
        hits
    }
}
