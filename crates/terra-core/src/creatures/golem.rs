//! Stone guardian action timing shared by simulation and mesh animation.
use super::{CreatureBehavior as B, CreatureKind as K, definitions};
/// Distance between consecutive sole contacts in the rendered gait and Foley.
pub const STRIDE: f32 = 0.92;
pub const WAKE_SECONDS: f32 = 2.1;
pub const SLAM_SECONDS: f32 = 3.15;
pub const SLAM_IMPACT: f32 = 0.56;
pub const SLAM_RADIUS: f64 = 5.0;
pub fn duration(action: B) -> f32 {
    match action {
        B::Shake => 0.52,
        B::Emerge => WAKE_SECONDS,
        B::Tremor => SLAM_SECONDS,
        B::Attack => definitions::get(K::StoneGolem).combat.unwrap().cooldown,
        _ => 0.,
    }
}
pub fn phase(action: B, remaining: f32) -> f32 {
    let total = duration(action);
    if total > 0. {
        (1. - remaining / total).clamp(0., 1.)
    } else {
        0.
    }
}

/// Player blows wake a sleeping guardian immediately. Its committed strike can finish.
pub fn hit(a: &mut super::Creature) {
    match a.behavior {
        B::Sleep | B::Rest => {
            a.behavior = B::Emerge;
            a.timer = WAKE_SECONDS;
        }
        B::Emerge | B::Attack | B::Tremor | B::Shake => {}
        _ => {
            a.behavior = B::Shake;
            a.timer = duration(B::Shake);
        }
    }
}
