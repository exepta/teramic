//! Webbing is a dodgeable ranged attack. The shooter stays attached to its
//! current surface throughout the wind-up and flight, fixing the launch point.
use super::*;
use bevy_math::DVec3;
use terra_core::creatures::spiders::{WEB_CAST_TIME, WEB_WINDUP, WebShot};

pub(super) fn begin(a: &mut Creature, center: [f64; 3], player: [f64; 3]) -> bool {
    let range = match a.kind {
        Kind::LargeSpider => 18.,
        Kind::MediumSpider => 14.,
        // The tiny surface spider only retaliates at very close range.
        _ => return false,
    };
    let d = distance(center, player);
    if a.timer > 0.
        || a.needs.search_in > 1.01
        || a.juvenile()
        || !(5.5_f64.powi(2)..=range * range).contains(&d)
    {
        return false;
    }
    // A wall spider's yaw encodes its contact face. Keep its feet and body on
    // that face while the net is fired sideways.
    if !matches!(a.behavior, Behavior::Climb | Behavior::Ceiling) {
        a.yaw = (center[0] - player[0])
            .atan2(center[2] - player[2])
            .rem_euclid(std::f64::consts::TAU) as f32;
        a.behavior = Behavior::Hunt;
    }
    a.velocity = [0.; 3];
    a.needs.goal = Some(NeedGoal {
        kind: NeedKind::AttackAim,
        position: player.map(|v| v.floor() as i64),
    });
    a.timer = WEB_CAST_TIME;
    true
}

pub(super) fn advance(
    a: &mut Creature,
    h: &Habitat,
    players: &[[f64; 3]],
    dt: f32,
) -> Option<usize> {
    let elapsed = WEB_CAST_TIME - a.timer - WEB_WINDUP;
    let mut hit = None;
    if let Some(shot) = WebShot::of(a) {
        if elapsed <= 0. {
            return None;
        }
        let from = shot.at(elapsed - dt);
        let to = shot.at(elapsed);
        let delta = to - from;
        let length = delta.length();
        let direction = delta.normalize_or_zero();
        // Capsule broad phase expanded by the spreading strands. Only the first
        // player intersected along this tick's travel can be ensnared.
        let first = players
            .iter()
            .enumerate()
            .filter(|(_, p)| super::super::navigation::valid_point(**p))
            .filter_map(|(i, p)| {
                let p = DVec3::from_array(*p);
                let half = DVec3::new(0.62, 1.04, 0.62);
                let body = terra_core::block_shapes::ShapeBox {
                    minimum: (p - half).to_array(),
                    maximum: (p + half).to_array(),
                    hinged: false,
                };
                body.ray(from.to_array(), direction.to_array(), length)
                    .map(|(t, _)| (i, t))
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b));
        let stop = first.map_or(to, |(_, t)| from + direction * t);
        let muzzle_clear = elapsed > dt
            || line_of_sight(
                h,
                terra_core::creatures::spiders::visual_center(a),
                shot.origin.to_array(),
            );
        if muzzle_clear && line_of_sight(h, from.to_array(), stop.to_array()) {
            if first.is_none() && elapsed < shot.flight_time {
                return None;
            }
            hit = first.map(|(index, _)| index);
        }
    }
    a.needs.goal = None;
    a.needs.search_in = 6.0;
    a.timer = 0.4;
    hit
}
