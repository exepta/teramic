//! Swimming, breach and fireball poses shared by the simulation, picking and renderer.
use super::{Creature, CreatureBehavior as B, NeedKind};
use bevy_math::{Quat, Vec3};

pub const SEGMENTS: u8 = 15;
pub const SWIM: f32 = 10.;
pub const BOLT_SPEED: f32 = 16.;
pub const RELEASE: f32 = 0.65;
pub const SPIT: f32 = 2.15;
pub const SLAM: f32 = 1.5;
pub const IMPACT: f32 = 0.82;
pub const RISE: f32 = 1.3;
pub const DIVE: f32 = 1.0;

fn ease(v: f32) -> f32 {
    let t = v.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
pub fn segment_radius(i: u8) -> f32 {
    0.73 - 0.48 * (f32::from(i) / f32::from(SEGMENTS - 1)).powf(1.4)
}
/// A wave runs from the head towards the tapered tail as each ring follows its neighbour.
fn spine(behavior: B, remaining: f32, age: f64, index: f32) -> Vec3 {
    let t = index / f32::from(SEGMENTS - 1);
    let along = index * 0.48;
    let neck = (1. - along / 3.4).clamp(0., 1.);
    let wave = ((age * 2.15).rem_euclid(std::f64::consts::TAU) as f32 - along * 1.16).sin();
    let swim = if behavior == B::Swim {
        ease((SWIM - remaining) / 0.8) * ease(remaining / 0.85)
    } else {
        0.
    };
    let extend = match behavior {
        B::Emerge => ease(1. - remaining / RISE),
        B::Retreat => ease(remaining / DIVE),
        B::Swim | B::Burrowed | B::Dead => 0.,
        _ => 1.,
    };
    let elapsed = SLAM - remaining;
    let slam = if behavior == B::Attack {
        ease((elapsed - 0.43) / 0.39) * (1. - ease((elapsed - 0.9) / 0.55))
    } else {
        0.
    };
    let bottom = -0.92;
    let breath = wave * neck * 0.08;
    // Pull the neck into an S before a strike; the muscular pulse then travels
    // back from the jaw through consecutive plates during release and recovery.
    let (coil, thrust, survey) = match behavior {
        B::Spit => {
            let t = SPIT - remaining;
            let load = ease(t / RELEASE) * (1. - ease((t - RELEASE) / 0.18));
            let recoil = ease((t - RELEASE) / 0.12) * (1. - ease((t - RELEASE - 0.12) / 0.55));
            (load * 0.34, recoil * 0.58, 0.)
        }
        B::Attack => (ease(elapsed / 0.42) * (1. - slam) * 0.47, -slam * 0.18, 0.),
        B::Hold => (0., 0., (age as f32 * 1.19).sin() * 0.25),
        _ => (0., 0., 0.),
    };
    let raised = extend * (3.15 * neck + breath + coil * neck - slam * 1.95 * neck.powi(2));
    Vec3::new(
        wave * (0.085 + 0.46 * t) * (swim + extend * (1. - neck))
            + survey * neck.powi(2)
            + coil * (along * 1.54).sin() * neck,
        bottom + raised + swim * (0.88 + 0.13 * wave),
        along - 1.10
            + extend * (0.65 * neck * (1. - neck) - slam * 1.08 * neck.powi(2))
            + (coil + thrust) * neck.powi(2),
    )
}

pub fn center(behavior: B, remaining: f32, age: f64, i: u8) -> Vec3 {
    spine(behavior, remaining, age, f32::from(i))
}

/// Orient each plate normal to the spine through lateral turns and vertical bends.
pub fn segment(behavior: B, remaining: f32, age: f64, i: u8) -> (Vec3, Quat) {
    let i = f32::from(i);
    let p = spine(behavior, remaining, age, i);
    let tangent =
        spine(behavior, remaining, age, i + 0.1) - spine(behavior, remaining, age, i - 0.1);
    let yaw = tangent.x.atan2(tangent.z);
    let pitch = (-tangent.y).atan2(tangent.x.hypot(tangent.z));
    (p, Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch))
}
/// The hot throat flares immediately before firing and snaps shut during recoil.
pub fn gape(behavior: B, remaining: f32, age: f32) -> f32 {
    match behavior {
        B::Spit => {
            let t = SPIT - remaining;
            ease(t / RELEASE) * (1. - ease((t - RELEASE - 0.12) / 0.49))
        }
        B::Attack => {
            let t = SLAM - remaining;
            ease(t / 0.50) * (1. - ease((t - 0.56) / 0.26))
        }
        B::Hold | B::Emerge => {
            (0.23 + (age * 2.15).sin() * 0.10)
                * if behavior == B::Emerge {
                    ease(1. - remaining / RISE)
                } else {
                    1.
                }
        }
        _ => 0.,
    }
}

pub fn mouth_socket(behavior: B, remaining: f32, age: f64) -> Vec3 {
    // Once released, the projectile keeps the mouth position at release,
    // including across snapshots taken during the recoil.
    let (remaining, age) = if behavior == B::Spit {
        (
            SPIT - RELEASE,
            age + f64::from(RELEASE - (SPIT - remaining)),
        )
    } else {
        (remaining, age)
    };
    let (center, rotation) = segment(behavior, remaining, age, 0);
    center + rotation * Vec3::new(0., 0., -0.44)
}
pub fn muzzle(a: &Creature) -> [f64; 3] {
    let (sin, cos) = f64::from(a.yaw).sin_cos();
    let socket = mouth_socket(a.behavior, a.timer, a.age).as_dvec3();
    [
        a.position[0] + cos * socket.x + sin * socket.z,
        a.position[1] + socket.y,
        a.position[2] + cos * socket.z - sin * socket.x,
    ]
}
pub fn bolt_direction(a: &Creature) -> Option<[f64; 3]> {
    let aim = a.needs.goal.filter(|g| g.kind == NeedKind::AttackAim)?;
    let from = muzzle(a);
    let d = std::array::from_fn::<_, 3, _>(|i| aim.position[i] as f64 + 0.5 - from[i]);
    let len = d.iter().map(|x| x * x).sum::<f64>().sqrt();
    (len > 0.01).then(|| d.map(|x| x / len))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jaw_telegraphs_attacks_and_release_socket_stays_fixed_through_recoil() {
        assert!(gape(B::Spit, SPIT, 70.) < 0.05);
        assert!(gape(B::Spit, SPIT - RELEASE, 70. + RELEASE) > 0.8);
        assert!(gape(B::Attack, SLAM - 0.5, 80.) > 0.9);
        assert!(gape(B::Attack, SLAM - IMPACT, 80.) < 0.07);
        let launch = mouth_socket(B::Spit, SPIT - RELEASE, 70. + f64::from(RELEASE));
        for i in 1..=25 {
            let elapsed = RELEASE + i as f32 * 0.05;
            let socket = mouth_socket(B::Spit, SPIT - elapsed, 70. + f64::from(elapsed));
            assert!(socket.distance(launch) < 0.0001);
        }
    }
    #[test]
    fn an_established_swim_exposes_a_continuous_tapered_back() {
        for frame in 0..60 {
            let age = f64::from(frame) / 20.;
            for i in 0..SEGMENTS {
                let (ring, orientation) = segment(B::Swim, SWIM * 0.5, age, i);
                assert!(ring.y + segment_radius(i) >= 0.075);
                assert!((orientation.length() - 1.).abs() < 0.001);
                if i > 0 {
                    let previous = center(B::Swim, SWIM * 0.5, age, i - 1);
                    assert!(previous.distance(ring) < 0.62);
                }
            }
        }
    }
}
