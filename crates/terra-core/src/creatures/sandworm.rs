//! Replicated worm phases use the existing saved behavior/timer contract.
use super::{Creature, CreatureBehavior as B, CreatureKind};

pub const WARNING_SECONDS: f32 = 1.8;
pub const LEAP_SECONDS: f32 = 5.0;
pub const LEAP_RADIUS: f32 = 7.0;
pub const LEAP_DISTANCE: f32 = LEAP_RADIUS * 2.0;
pub const EMERGE_SECONDS: f32 = 1.35;
pub const EXPOSED_SECONDS: f32 = 4.2;
pub const RETREAT_SECONDS: f32 = 1.5;
pub const BURROW_SECONDS: f32 = 7.0;
/// An announced, straight underground rush, represented by the existing Hunt phase.
pub const CHARGE_SECONDS: f32 = 1.6;
pub const CHARGE_PACE: f64 = 2.7;
pub const CHARGE_DAMAGE: f32 = 8.0;
pub const KNOCKUP_PHASE: f32 = 0.38;
pub const KNOCKUP_SPEED: f32 = 11.0;
pub const KNOCKUP_DAMAGE: f32 = 4.0;
pub const BODY_SEGMENTS: usize = 24;
pub const HEAD_SEGMENT: usize = BODY_SEGMENTS;
pub const NECK_ROOT_SEGMENT: usize = 14;
pub const NECK_LINK: f32 = 1.32;
/// The exposed worm keeps its maw on the rising arc of the neck until it strikes.
const REARED_PITCH: f32 = 0.54;
pub const THROAT_BACKPLANE: f32 = 0.315;
pub const HEAD_TARGET_OFFSET: f32 = 0.10;
pub const HEAD_TARGET_SIZE: [f32; 3] = [1.94, 1.94, 1.80];
pub const BITE_PHASE: f32 = 0.20;
pub const SPIT_PHASE: f32 = 0.66;
pub const SPIT_SECONDS: f32 = 2.6;
pub const SPIT_SPEED: f32 = 20.0;
pub const SPIT_RANGE: f64 = 40.0;
pub const SPIT_UPWARD: f32 = 1.4;
pub const SPIT_GRAVITY: f32 = 10.0;

/// Circular mandibles visibly open, then close together at the authority bite.
pub fn bite_closure(progress: f32) -> f32 {
    let distance = (progress - BITE_PHASE).abs();
    // Ease both the heavy forward strike and its recovery. The old squared
    // triangle changed velocity abruptly at contact, snapping the long neck.
    smooth(1.0 - distance / 0.16)
}

pub fn spit_time(progress: f32) -> Option<f32> {
    let time = (progress - SPIT_PHASE) * EXPOSED_SECONDS;
    (0.0..=SPIT_SECONDS).contains(&time).then_some(time)
}

pub fn spit_offset(time: f32) -> [f32; 3] {
    [
        0.0,
        SPIT_UPWARD * time - 0.5 * SPIT_GRAVITY * time * time,
        -SPIT_SPEED * time,
    ]
}

pub fn phase_progress(behavior: B, timer: f32) -> f32 {
    let duration = match behavior {
        B::Leap => LEAP_SECONDS,
        B::Tremor => WARNING_SECONDS,
        B::Emerge => EMERGE_SECONDS,
        B::Attack => EXPOSED_SECONDS,
        B::Retreat => RETREAT_SECONDS,
        B::Spit => SPIT_SECONDS,
        B::Hunt => CHARGE_SECONDS,
        _ => return 0.0,
    };
    if timer.is_finite() {
        (1.0 - timer / duration).clamp(0.0, 1.0)
    } else {
        0.0
    }
}
pub fn emergence(a: &Creature) -> f32 {
    if a.kind != CreatureKind::SandWorm {
        return 1.0;
    }
    let t = phase_progress(a.behavior, a.timer);
    let smooth = t * t * (3.0 - 2.0 * t);
    match a.behavior {
        B::Emerge => smooth,
        B::Leap => (t * std::f32::consts::PI).sin().max(0.0),
        B::Attack | B::Spit | B::Dead => 1.0,
        B::Retreat => 1.0 - smooth,
        _ => 0.0,
    }
}
pub fn warning(a: &Creature) -> f32 {
    if a.kind == CreatureKind::SandWorm && a.behavior == B::Tremor {
        0.25 + phase_progress(a.behavior, a.timer) * 0.75
    } else if a.kind == CreatureKind::SandWorm && a.behavior == B::Hunt {
        0.8
    } else {
        0.0
    }
}

/// One shared spine drives cached rigid meshes, hit targets and health labels.
/// The stored location stays fixed throughout the announced eruption, then
/// follows authoritative crawling once its opening bite has finished.
#[derive(Clone, Copy, Debug)]
pub struct SegmentPose {
    pub center: [f32; 3],
    pub pitch: f32,
}

/// Flat annulated body remains thick almost to the rounded tail. Compared with
/// the previous 4.6m model the longitudinal footprint is approximately23m.
pub const SPINE_CENTERS: [[f32; 3]; BODY_SEGMENTS] = spine_centers();
const fn spine_centers() -> [[f32; 3]; BODY_SEGMENTS] {
    let mut out = [[0.0; 3]; BODY_SEGMENTS];
    let mut i = 0;
    while i < 15 {
        out[i] = [0.0, -1.40, 4.90 + (14 - i) as f32 * 1.20];
        i += 1;
    }
    out[15] = [0.0, -0.90, 3.70];
    out[16] = [0.0, -0.30, 2.50];
    out[17] = [0.0, 0.60, 1.30];
    out[18] = [0.0, 1.50, 0.60];
    out[19] = [0.0, 2.40, 0.20];
    out[20] = [0.0, 3.30, 0.10];
    out[21] = [0.0, 4.20, 0.10];
    out[22] = [0.0, 5.10, 0.10];
    out[23] = [0.0, 6.00, 0.10];
    out
}
pub const SPINE_PITCHES: [f32; BODY_SEGMENTS] = [
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.40, 0.55, 0.75,
    0.95, 1.25, 1.50, 1.50, 1.25, 0.65,
];
pub fn segment_radius(index: usize) -> f32 {
    match index {
        0 => 0.12,
        1 => 0.42,
        2 => 0.74,
        3 => 0.93,
        19 | 21 => 1.04,
        20 => 1.08,
        23 => 0.94,
        _ => 1.0,
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Combat eruptions rise vertically from their stored hole, then bend at the
/// top. Retreat follows the same shaft back underground. Traveling leaps use a
/// separate head-first arc; the long tail never becomes a rigid airborne lever.
pub fn spine_pose(behavior: B, progress: f32, segment: usize) -> SegmentPose {
    let progress = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if matches!(behavior, B::Burrowed | B::Tremor) {
        return raw_spine_pose(behavior, progress, segment);
    }
    if segment > NECK_ROOT_SEGMENT && segment < HEAD_SEGMENT {
        if behavior == B::Leap {
            if segment == HEAD_SEGMENT - 1 {
                let head = raw_spine_pose(behavior, progress, HEAD_SEGMENT);
                return from_head(head, [0.0, 0.0, NECK_LINK], 0.0);
            }
        } else {
            return neck_pose(behavior, progress, segment);
        }
    }
    raw_spine_pose(behavior, progress, segment)
}

fn from_head(head: SegmentPose, local: [f32; 3], pitch: f32) -> SegmentPose {
    let (sin, cos) = head.pitch.sin_cos();
    SegmentPose {
        center: [
            head.center[0] + local[0],
            head.center[1] + local[1] * cos - local[2] * sin,
            head.center[2] + local[1] * sin + local[2] * cos,
        ],
        pitch: head.pitch + pitch,
    }
}

/// A single flexible neck joins the buried shoulder to the rear of the head.
/// Arc-length samples preserve annular spacing while a bite folds the neck
/// behind the collar instead of compressing several rings through the mouth.
fn neck_pose(behavior: B, progress: f32, segment: usize) -> SegmentPose {
    const STEPS: usize = 16;
    let head = raw_spine_pose(behavior, progress, HEAD_SEGMENT);
    let root = raw_spine_pose(behavior, progress, NECK_ROOT_SEGMENT);
    let (sin, cos) = head.pitch.sin_cos();
    let dy = root.center[1] - head.center[1];
    let dz = root.center[2] - head.center[2];
    let root_y = dy * cos + dz * sin;
    let root_z = -dy * sin + dz * cos;
    let distance = root_y.hypot(root_z - NECK_LINK);
    let compression = (9.0 - distance).max(0.0);
    let bulge = 0.45 + compression * 0.25;
    // Spread the turn across several links when fully reared. A short last
    // handle kinks the spine at the collar even with exact tangent matching.
    let collar_handle = bulge.max(distance * 0.39);
    let coil = smooth(compression / 3.0);
    let root_control = root_z * 0.65 * (1.0 - coil) + (root_z + bulge) * coil;
    let controls = [
        [0.0, root_y, root_z],
        [0.0, root_y * 0.55, root_control.max(NECK_LINK + bulge)],
        [0.0, 0.0, NECK_LINK + collar_handle],
        [0.0, 0.0, NECK_LINK],
    ];
    let curve = |t: f32| -> [f32; 3] {
        let u = 1.0 - t;
        std::array::from_fn(|i| {
            controls[0][i] * u.powi(3)
                + 3.0 * controls[1][i] * u * u * t
                + 3.0 * controls[2][i] * u * t * t
                + controls[3][i] * t.powi(3)
        })
    };
    let mut lengths = [0.0; STEPS + 1];
    let mut previous = curve(0.0);
    for i in 1..=STEPS {
        let point = curve(i as f32 / STEPS as f32);
        lengths[i] = lengths[i - 1] + (point[1] - previous[1]).hypot(point[2] - previous[2]);
        previous = point;
    }
    let fraction =
        (segment - NECK_ROOT_SEGMENT) as f32 / (BODY_SEGMENTS - 1 - NECK_ROOT_SEGMENT) as f32;
    let wanted = lengths[STEPS] * fraction;
    let end = (1..=STEPS).find(|&i| lengths[i] >= wanted).unwrap_or(STEPS);
    let blend =
        ((wanted - lengths[end - 1]) / (lengths[end] - lengths[end - 1]).max(1e-6)).clamp(0.0, 1.0);
    let t = (end as f32 - 1.0 + blend) / STEPS as f32;
    let u = 1.0 - t;
    let tangent: [f32; 3] = std::array::from_fn(|i| {
        3.0 * u * u * (controls[1][i] - controls[0][i])
            + 6.0 * u * t * (controls[2][i] - controls[1][i])
            + 3.0 * t * t * (controls[3][i] - controls[2][i])
    });
    from_head(head, curve(t), tangent[1].atan2(-tangent[2]))
}

/// Resting combat anatomy emerges around the saved ground anchor. The tail
/// keeps its full buried footprint while its shoulder meets that exact hole.
fn upright_rest(segment: usize) -> SegmentPose {
    if segment >= HEAD_SEGMENT {
        return SegmentPose {
            center: [0.0, 6.85, -0.75],
            pitch: REARED_PITCH,
        };
    }
    let mut center = SPINE_CENTERS[segment];
    center[2] -= SPINE_CENTERS[NECK_ROOT_SEGMENT][2] * segment.min(NECK_ROOT_SEGMENT) as f32
        / NECK_ROOT_SEGMENT as f32;
    SegmentPose {
        center,
        pitch: SPINE_PITCHES[segment],
    }
}

/// A piston-like rise precedes the upper-neck bend. While the mouth clears the
/// sand its centre and direction stay on the hole's vertical axis.
fn emerging_head(progress: f32) -> SegmentPose {
    let rise = smooth(progress / 0.78);
    let bend = smooth((progress - 0.68) / 0.32);
    SegmentPose {
        center: [0.0, -3.5 + 10.35 * rise, -0.75 * bend],
        pitch: std::f32::consts::FRAC_PI_2 * (1.0 - bend) + REARED_PITCH * bend,
    }
}

fn raw_spine_pose(behavior: B, progress: f32, segment: usize) -> SegmentPose {
    if behavior == B::Leap {
        return leap_pose(progress, segment);
    }
    if behavior == B::Retreat {
        // Withdraw tail first along the same vertical shaft. Turning the head
        // down into its own upright neck made the body pass through the mouth.
        return raw_spine_pose(B::Emerge, 1.0 - progress, segment);
    }
    if matches!(behavior, B::Burrowed | B::Tremor) {
        return SegmentPose {
            center: [
                0.0,
                -3.5 - (HEAD_SEGMENT - segment.min(HEAD_SEGMENT)) as f32 * 0.96,
                0.0,
            ],
            pitch: std::f32::consts::FRAC_PI_2,
        };
    }
    let mut rest = upright_rest(segment);
    match behavior {
        B::Emerge => {
            let head = emerging_head(progress);
            if segment == HEAD_SEGMENT {
                return head;
            }
            // The shoulder remains behind the rising head, rather than making
            // the emerging collar fold back through a shallower body.
            rest.center[1] += (head.center[1] - 8.0 - SPINE_CENTERS[NECK_ROOT_SEGMENT][1]).min(0.0);
            rest
        }
        B::Attack | B::Spit => {
            if segment < 15 {
                return rest;
            }
            let bite = if behavior == B::Attack {
                bite_closure(progress)
            } else {
                0.0
            };
            let prepare = (1.0 - ((progress - (SPIT_PHASE - 0.07)) / 0.09).abs()).clamp(0.0, 1.0);
            let neck = ((segment as f32 - 15.0) / 9.0).clamp(0.0, 1.0).powi(2);
            rest.center[1] -= 5.35 * neck * bite;
            // Preserve the real bite's ground target while the resting head
            // now leans slightly forward from the vertical shoulder.
            rest.center[2] -= 0.55 * neck * bite;
            rest.pitch += -(0.42 + REARED_PITCH) * neck * bite + 0.10 * neck * prepare;
            rest
        }
        B::Dead => {
            let neck = ((segment as f32 - 15.0) / 9.0).clamp(0.0, 1.0);
            rest.center[1] -= 0.16 + neck * 1.3;
            rest.pitch -= neck * 0.65;
            rest
        }
        _ => SegmentPose {
            center: [0.0, -3.5, 0.0],
            pitch: std::f32::consts::FRAC_PI_2,
        },
    }
}

/// Arc-length travel joins an upward exit to a separate downward entry. Rings
/// sample the same curve behind the head, including hidden underground ends.
fn leap_pose(progress: f32, segment: usize) -> SegmentPose {
    let arc = std::f32::consts::PI * LEAP_RADIUS;
    let lag = if segment >= HEAD_SEGMENT {
        0.0
    } else {
        NECK_LINK + (HEAD_SEGMENT - 1 - segment) as f32 * 0.96
    };
    let tail_lag = NECK_LINK + (HEAD_SEGMENT - 1) as f32 * 0.96;
    let head = -2.0 + progress * (arc + tail_lag + 4.0);
    let distance = head - lag;
    if distance < 0.0 {
        return SegmentPose {
            center: [0.0, distance - 1.25, 0.0],
            pitch: std::f32::consts::FRAC_PI_2,
        };
    }
    if distance > arc {
        return SegmentPose {
            center: [0.0, -1.25 - (distance - arc), -LEAP_DISTANCE],
            pitch: -std::f32::consts::FRAC_PI_2,
        };
    }
    let angle = distance / LEAP_RADIUS;
    SegmentPose {
        center: [
            0.0,
            angle.sin() * LEAP_RADIUS - 1.25,
            -LEAP_RADIUS * (1.0 - angle.cos()),
        ],
        pitch: std::f32::consts::FRAC_PI_2 - angle,
    }
}

pub fn mouth_offset(head: SegmentPose) -> [f32; 3] {
    from_head(head, [0.0, 0.0, -0.62], 0.0).center
}

pub fn mouth_position(a: &Creature) -> [f64; 3] {
    let local = mouth_offset(pose(a, HEAD_SEGMENT));
    let (sin, cos) = f64::from(a.yaw).sin_cos();
    [
        a.position[0] + f64::from(local[0]) * cos + f64::from(local[2]) * sin,
        a.position[1] + f64::from(local[1]),
        a.position[2] - f64::from(local[0]) * sin + f64::from(local[2]) * cos,
    ]
}

pub fn committed(behavior: B) -> bool {
    matches!(
        behavior,
        B::Tremor | B::Hunt | B::Leap | B::Emerge | B::Attack | B::Spit | B::Retreat
    )
}

pub fn leap_destination(a: &Creature) -> [f64; 3] {
    let (sin, cos) = f64::from(a.yaw).sin_cos();
    [
        a.position[0] - sin * f64::from(LEAP_DISTANCE),
        a.position[1],
        a.position[2] - cos * f64::from(LEAP_DISTANCE),
    ]
}

/// The rear collar contributes to the head target without moving the actual
/// mouth pivot used by authority and the projectile origin.
pub fn target_center(pose: SegmentPose, segment: usize) -> [f32; 3] {
    if segment == HEAD_SEGMENT {
        from_head(pose, [0.0, 0.0, HEAD_TARGET_OFFSET], 0.0).center
    } else {
        pose.center
    }
}
pub fn target_size(segment: usize) -> [f32; 3] {
    if segment == HEAD_SEGMENT {
        HEAD_TARGET_SIZE
    } else {
        let r = segment_radius(segment);
        [1.84 * r, 1.46 * r, 1.68]
    }
}
fn anatomy_top(pose: SegmentPose, segment: usize) -> f32 {
    let (sin, cos) = pose.pitch.sin_cos();
    let size = target_size(segment);
    target_center(pose, segment)[1] + size[1] * 0.5 * cos.abs() + size[2] * 0.5 * sin.abs()
}
/// Fully submerged rings are absent even for a camera below the terrain.
pub fn anatomy_visible(behavior: B, pose: SegmentPose, segment: usize) -> bool {
    !matches!(behavior, B::Burrowed | B::Tremor | B::Hunt) && anatomy_top(pose, segment) > 0.0
}

pub fn pose(a: &Creature, segment: usize) -> SegmentPose {
    spine_pose_at(
        a.behavior,
        phase_progress(a.behavior, a.timer),
        a.age as f32,
        segment,
    )
}

pub fn spine_pose_at(behavior: B, progress: f32, clock: f32, segment: usize) -> SegmentPose {
    let mut pose = spine_pose(behavior, progress, segment);
    if segment < BODY_SEGMENTS && clock.is_finite() {
        let phase = segment as f32 * 0.62;
        pose.center[1] +=
            ((clock * 2.7 - phase).sin() - (-phase).sin()) * 0.025 * segment_radius(segment);
    }
    pose
}

/// Launch coordinates are shared with the cached visible projectile path.
pub fn spit_launch(a: &Creature) -> ([f64; 3], [f64; 3]) {
    let local = mouth_offset(spine_pose(B::Spit, 0.0, HEAD_SEGMENT));
    let (sin, cos) = f64::from(a.yaw).sin_cos();
    let origin = [
        a.position[0] + f64::from(local[0]) * cos + f64::from(local[2]) * sin,
        a.position[1] + f64::from(local[1]),
        a.position[2] - f64::from(local[0]) * sin + f64::from(local[2]) * cos,
    ];
    (
        origin,
        [
            -sin * f64::from(SPIT_SPEED),
            f64::from(SPIT_UPWARD),
            -cos * f64::from(SPIT_SPEED),
        ],
    )
}

/// A world-space aim recorded by authority, shared with the visual projectile.
/// Cell centers give sub-block accuracy without altering saved Creature layout.
pub fn aimed_spit_launch(a: &Creature) -> Option<([f64; 3], [f64; 3])> {
    let aim = a
        .needs
        .goal
        .filter(|g| g.kind == super::NeedKind::AttackAim)?;
    let target = aim.position.map(|p| p as f64 + 0.5);
    let (origin, _) = spit_launch(a);
    let delta = std::array::from_fn::<_, 3, _>(|i| target[i] - origin[i]);
    let horizontal = delta[0].hypot(delta[2]);
    if !(0.2..=SPIT_RANGE).contains(&horizontal) {
        return None;
    }
    let flight = (horizontal / f64::from(SPIT_SPEED)).clamp(0.25, f64::from(SPIT_SECONDS));
    let velocity = [
        delta[0] / flight,
        delta[1] / flight + 0.5 * f64::from(SPIT_GRAVITY) * flight,
        delta[2] / flight,
    ];
    velocity
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 32.0)
        .then_some((origin, velocity))
}

pub fn visible_height(a: &Creature) -> f32 {
    if matches!(a.behavior, B::Burrowed | B::Tremor | B::Hunt) {
        return 0.0;
    }
    (0..=HEAD_SEGMENT)
        .map(|i| anatomy_top(pose(a, i), i))
        .fold(0.35, f32::max)
}

/// Damage must not teleport an airborne or diving animal into its upright
/// attack pose. Resting armor can still be provoked into an announced eruption.
pub fn hit_reaction(a: &mut Creature, attacker: [f64; 3]) {
    if a.kind != CreatureKind::SandWorm || committed(a.behavior) {
        return;
    }
    a.behavior = B::Tremor;
    a.timer = 0.65;
    if attacker.iter().all(|v| v.is_finite()) {
        a.yaw = ((a.position[0] - attacker[0]).atan2(a.position[2] - attacker[2])) as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_attack_eruption_shoots_vertically_from_its_hole_before_only_the_top_bends() {
        let mut last_y = f32::MIN;
        for step in 0..=100 {
            let progress = step as f32 / 100.;
            let head = spine_pose(B::Emerge, progress, HEAD_SEGMENT);
            assert!(head.center[1] >= last_y);
            last_y = head.center[1];
            assert_eq!(head.center[0], 0.);
            assert!((-0.75001..=0.00001).contains(&head.center[2]));
            if progress <= 0.68 {
                assert!(head.center[2].abs() < 0.00001);
                assert!((head.pitch - std::f32::consts::FRAC_PI_2).abs() < 0.00001);
                for i in NECK_ROOT_SEGMENT..HEAD_SEGMENT {
                    let ring = spine_pose(B::Emerge, progress, i);
                    assert!(
                        ring.center[0].abs() < 0.00001 && ring.center[2].abs() < 0.00001,
                        "the exposed shaft must rise over the actual hole: {progress}, {i}, {ring:?}"
                    );
                }
            }
        }
        let shoulder = spine_pose(B::Attack, 0., NECK_ROOT_SEGMENT);
        assert_eq!(shoulder.center[2], 0.);
        let head = spine_pose(B::Attack, 0., HEAD_SEGMENT);
        assert!(head.center[2].abs() < 0.8 && head.center[1] > 6.8);
    }

    #[test]
    fn the_reared_maw_continues_the_upper_neck_arc() {
        let head = spine_pose(B::Attack, 0.0, HEAD_SEGMENT);
        assert!((0.42..0.70).contains(&head.pitch));
        for segment in 20..HEAD_SEGMENT {
            let a = spine_pose(B::Attack, 0.0, segment);
            let b = spine_pose(B::Attack, 0.0, segment + 1);
            assert!(
                (a.pitch - b.pitch).abs() < 0.48,
                "sharp kink behind head at {segment}"
            );
        }
    }

    #[test]
    fn the_diving_mouth_and_the_next_eruption_cross_the_same_ground_point() {
        let mut down = false;
        let mut up = false;
        for i in 0..=2000 {
            let p = i as f32 / 2000.;
            for phase in [B::Retreat, B::Emerge] {
                let head = spine_pose(phase, p, HEAD_SEGMENT);
                let y = head.center[1] + head.pitch.sin() * 0.62;
                if y.abs() < 0.04 {
                    let z = head.center[2] - head.pitch.cos() * 0.62;
                    assert!(
                        head.center[0].abs() < 0.00001 && z.abs() < 0.00001,
                        "entry/exit must share the stored hole: {phase:?}, {p}, {z}"
                    );
                    if phase == B::Retreat {
                        down = true;
                    } else {
                        up = true;
                    }
                }
            }
        }
        assert!(
            down && up,
            "both actual mouth trajectories must cross the surface"
        );
    }

    #[test]
    fn phases_preserve_old_ids_and_have_smooth_bounded_transitions() {
        assert_eq!(CreatureKind::ThornBrute as u8, 29);
        assert_eq!(CreatureKind::SandWorm as u8, 30);
        assert_eq!(B::Descend as u8, 24);
        assert_eq!(B::Burrowed as u8, 25);
        assert_eq!(phase_progress(B::Tremor, WARNING_SECONDS), 0.0);
        assert_eq!(phase_progress(B::Tremor, 0.0), 1.0);
        assert_eq!(phase_progress(B::Emerge, f32::NAN), 0.0);
    }

    #[test]
    fn leap_travels_between_two_holes_and_waits_for_the_whole_tail_to_enter() {
        let total =
            std::f32::consts::PI * LEAP_RADIUS + NECK_LINK + (HEAD_SEGMENT - 1) as f32 * 0.96 + 4.0;
        let at = |q| (q + 2.0) / total;
        let rising = spine_pose(B::Leap, at(3.0), HEAD_SEGMENT);
        let apex = spine_pose(
            B::Leap,
            at(std::f32::consts::FRAC_PI_2 * LEAP_RADIUS),
            HEAD_SEGMENT,
        );
        let diving = spine_pose(
            B::Leap,
            at(std::f32::consts::PI * LEAP_RADIUS - 3.0),
            HEAD_SEGMENT,
        );
        assert!(rising.pitch > 0.8 && diving.pitch < -0.8);
        assert!(apex.center[1] > 5.7 && (apex.center[2] + LEAP_RADIUS).abs() < 0.001);
        assert!(diving.center[2] < rising.center[2] - 12.0);
        let head_entered = at(std::f32::consts::PI * LEAP_RADIUS + 2.0);
        assert!(!anatomy_visible(
            B::Leap,
            spine_pose(B::Leap, head_entered, HEAD_SEGMENT),
            HEAD_SEGMENT
        ));
        assert!((0..HEAD_SEGMENT).any(|i| anatomy_visible(
            B::Leap,
            spine_pose(B::Leap, head_entered, i),
            i
        )));
        for i in 0..=HEAD_SEGMENT {
            assert!(
                !anatomy_visible(B::Leap, spine_pose(B::Leap, 0.999, i), i),
                "the entire body must be hidden before the root moves"
            );
            for step in 0..=100 {
                let ring = spine_pose(B::Leap, step as f32 / 100.0, i);
                assert!(ring.center.iter().all(|v| v.is_finite()) && ring.pitch.is_finite());
                if i > 0 {
                    let previous = spine_pose(B::Leap, step as f32 / 100.0, i - 1);
                    let gap = ring
                        .center
                        .into_iter()
                        .zip(previous.center)
                        .map(|(a, b)| (a - b).powi(2))
                        .sum::<f32>()
                        .sqrt();
                    assert!(
                        gap <= if i == HEAD_SEGMENT {
                            NECK_LINK + 0.001
                        } else {
                            1.02
                        },
                        "all rings follow one continuous curve: {i}, {gap}"
                    );
                }
            }
        }
    }

    #[test]
    fn upright_fighting_pose_joins_emergence_then_retracts_completely() {
        assert!(spine_pose(B::Attack, 0.0, HEAD_SEGMENT).center[1] > 6.8);
        assert!(
            spine_pose(B::Attack, BITE_PHASE, HEAD_SEGMENT).center[1] < 1.51,
            "the jaws lunge down to prey"
        );
        for i in 0..=HEAD_SEGMENT {
            let emergence = spine_pose(B::Emerge, 1.0, i);
            let attack = spine_pose(B::Attack, 0.0, i);
            assert!((emergence.center[1] - attack.center[1]).abs() < 0.001);
            assert!((emergence.pitch - attack.pitch).abs() < 0.001);
            let approaching = spine_pose(B::Emerge, 0.999, i);
            assert!((approaching.pitch - attack.pitch).abs() < 0.002);
            assert!(
                approaching
                    .center
                    .into_iter()
                    .zip(attack.center)
                    .all(|(a, b)| (a - b).abs() < 0.002)
            );
            assert!(!anatomy_visible(
                B::Retreat,
                spine_pose(B::Retreat, 1.0, i),
                i
            ));
        }
        for i in 0..=NECK_ROOT_SEGMENT {
            assert!(
                !anatomy_visible(B::Attack, spine_pose(B::Attack, BITE_PHASE, i), i),
                "buried rear armor stays hidden"
            );
        }
    }

    #[test]
    fn annulated_body_is_five_times_the_previous_length_with_overlapping_round_tail() {
        assert_eq!(BODY_SEGMENTS, 24);
        let footprint = SPINE_CENTERS[0][2] + 0.84 - (-0.62 - 0.60);
        assert!((23.0..24.0).contains(&footprint));
        assert!(footprint >= 4.6 * 5.0);
        assert!(segment_radius(0) < 0.15);
        for i in 0..4 {
            assert!(segment_radius(i) < segment_radius(i + 1));
        }
        for pair in SPINE_CENTERS[..18].windows(2) {
            assert!(
                (pair[0][2] - pair[1][2]).abs() < 1.58,
                "adjacentannuli overlap"
            );
        }
        assert_eq!(bite_closure(BITE_PHASE), 1.0);
        assert_eq!(bite_closure(0.0), 0.0);
    }

    #[test]
    fn the_heavy_bite_eases_into_contact_and_recovers_without_a_velocity_snap() {
        let epsilon = 0.0001;
        assert_eq!(bite_closure(BITE_PHASE), 1.0);
        assert_eq!(bite_closure(BITE_PHASE - 0.16), 0.0);
        assert_eq!(bite_closure(BITE_PHASE + 0.16), 0.0);
        let left = (bite_closure(BITE_PHASE) - bite_closure(BITE_PHASE - epsilon)) / epsilon;
        let right = (bite_closure(BITE_PHASE + epsilon) - bite_closure(BITE_PHASE)) / epsilon;
        assert!(left.abs() < 0.05 && right.abs() < 0.05);
        for step in 0..100 {
            let before = BITE_PHASE - 0.16 + step as f32 * 0.0016;
            assert!(bite_closure(before + 0.0016) >= bite_closure(before));
            let after = BITE_PHASE + step as f32 * 0.0016;
            assert!(bite_closure(after + 0.0016) <= bite_closure(after));
        }
    }

    #[test]
    fn locked_aim_reaches_the_enemy_cell_and_survives_saved_state_roundtrip() {
        let mut a = Creature {
            id: super::super::creature_id([0, 0], 0).unwrap(),
            kind: CreatureKind::SandWorm,
            home: [0, 0],
            position: [0.0; 3],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 42.0,
            behavior: B::Attack,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: super::super::CreatureNeeds::full(CreatureKind::SandWorm),
        };
        a.behavior = B::Spit;
        a.timer = SPIT_SECONDS;
        a.yaw = 0.0;
        a.needs.goal = Some(super::super::NeedGoal {
            kind: super::super::NeedKind::AttackAim,
            position: [5, 1, -9],
        });
        let (origin, velocity) = aimed_spit_launch(&a).unwrap();
        let target = [5.5, 1.5, -8.5];
        let t = (target[0] - origin[0]).hypot(target[2] - origin[2]) / f64::from(SPIT_SPEED);
        let impact = std::array::from_fn::<_, 3, _>(|i| {
            origin[i] + velocity[i] * t
                - if i == 1 {
                    0.5 * f64::from(SPIT_GRAVITY) * t * t
                } else {
                    0.0
                }
        });
        assert!(
            impact
                .into_iter()
                .zip(target)
                .all(|(a, b)| (a - b).abs() < 1e-6)
        );
        assert_eq!(
            a,
            serde_json::from_slice::<Creature>(&serde_json::to_vec(&a).unwrap()).unwrap()
        );
        a.needs.goal = None;
        assert!(aimed_spit_launch(&a).is_none());
    }

    #[test]
    fn hitting_an_airborne_or_diving_worm_does_not_reset_its_spine() {
        let mut a = Creature {
            id: 1,
            kind: CreatureKind::SandWorm,
            home: [0, 0],
            position: [0.0; 3],
            velocity: [0.0; 3],
            yaw: 0.7,
            health: 20.0,
            behavior: B::Emerge,
            timer: EMERGE_SECONDS * 0.5,
            age: 2.0,
            variant: 0,
            needs: super::super::CreatureNeeds::full(CreatureKind::SandWorm),
        };
        for (behavior, timer) in [
            (B::Leap, LEAP_SECONDS * 0.5),
            (B::Attack, EXPOSED_SECONDS * 0.5),
            (B::Emerge, EMERGE_SECONDS * 0.5),
            (B::Retreat, RETREAT_SECONDS * 0.5),
            (B::Spit, SPIT_SECONDS * 0.5),
        ] {
            a.behavior = behavior;
            a.timer = timer;
            let before = a.clone();
            hit_reaction(&mut a, [2.0, 1.0, 2.0]);
            assert_eq!(a, before);
        }
        a.behavior = B::Burrowed;
        hit_reaction(&mut a, [2.0, 1.0, 2.0]);
        assert_eq!(a.behavior, B::Tremor);
        assert_eq!(a.timer, 0.65);
    }
    #[test]
    fn a_ground_victim_is_inside_the_visible_bite_and_outside_the_upright_jaws() {
        let mut a = Creature {
            id: 1,
            kind: CreatureKind::SandWorm,
            home: [0, 0],
            position: [0.0; 3],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 20.0,
            behavior: B::Attack,
            timer: EXPOSED_SECONDS,
            age: 0.0,
            variant: 0,
            needs: super::super::CreatureNeeds::full(CreatureKind::SandWorm),
        };
        let victim = [0.0, 0.9, -1.8];
        let distance = |p: [f64; 3]| {
            p.into_iter()
                .zip(victim)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                .sqrt()
        };
        assert!(distance(mouth_position(&a)) > 5.0);
        a.timer = EXPOSED_SECONDS * (1.0 - BITE_PHASE);
        assert!(distance(mouth_position(&a)) < 0.5);
    }
    #[test]
    fn articulated_neck_meets_the_back_of_the_collar_through_every_phase() {
        for behavior in [B::Leap, B::Emerge, B::Attack, B::Spit, B::Retreat] {
            for progress in (0..=100)
                .map(|i| i as f32 / 100.0)
                .chain([BITE_PHASE, SPIT_PHASE])
            {
                let head = spine_pose(behavior, progress, HEAD_SEGMENT);
                let neck = spine_pose(behavior, progress, HEAD_SEGMENT - 1);
                let (sin, cos) = head.pitch.sin_cos();
                let dy = neck.center[1] - head.center[1];
                let dz = neck.center[2] - head.center[2];
                assert!(
                    (dy * cos + dz * sin).abs() < 0.0001,
                    "the neck attaches on the head axis"
                );
                assert!((-dy * sin + dz * cos - NECK_LINK).abs() < 0.0001);
                assert!(
                    (head.pitch - neck.pitch).abs() < 0.0001,
                    "no detached or kinked collar"
                );
                for i in NECK_ROOT_SEGMENT + 1..HEAD_SEGMENT {
                    let a = spine_pose(behavior, progress, i - 1);
                    let b = spine_pose(behavior, progress, i);
                    let gap = (a.center[1] - b.center[1]).hypot(a.center[2] - b.center[2]);
                    assert!(
                        gap < 1.65,
                        "neck rings must overlap: {behavior:?}, {progress}, {i}, {gap}"
                    );
                }
            }
        }
    }

    #[test]
    fn no_visible_body_target_blocks_the_open_throat_or_bite_aperture() {
        use super::super::hitboxes::Hitbox;
        for behavior in [B::Leap, B::Emerge, B::Attack, B::Spit, B::Retreat] {
            for progress in (0..=100)
                .map(|i| i as f32 / 100.0)
                .chain([BITE_PHASE, SPIT_PHASE])
            {
                let head = spine_pose(behavior, progress, HEAD_SEGMENT);
                if !anatomy_visible(behavior, head, HEAD_SEGMENT) {
                    continue;
                }
                let (sin, cos) = head.pitch.sin_cos();
                for i in 0..BODY_SEGMENTS {
                    let ring = spine_pose(behavior, progress, i);
                    if !anatomy_visible(behavior, ring, i) {
                        continue;
                    }
                    let dy = ring.center[1] - head.center[1];
                    let dz = ring.center[2] - head.center[2];
                    let (rs, rc) = (ring.pitch - head.pitch).sin_cos();
                    let box_ = Hitbox {
                        center: [
                            0.0,
                            f64::from(dy * cos + dz * sin),
                            f64::from(-dy * sin + dz * cos),
                        ],
                        size: target_size(i).map(f64::from),
                        axes: [
                            [1.0, 0.0, 0.0],
                            [0.0, f64::from(rc), f64::from(rs)],
                            [0.0, -f64::from(rs), f64::from(rc)],
                        ],
                    };
                    for [x, y] in [[0.0, 0.0], [0.3, 0.0], [-0.3, 0.0], [0.0, 0.3], [0.0, -0.3]] {
                        if let Some(hit) = box_.ray_distance(
                            [x, y, -2.0],
                            [0.0, 0.0, 1.0],
                            2.0 + f64::from(THROAT_BACKPLANE),
                        ) {
                            panic!(
                                "body target enters the visible mouth: {behavior:?}, phase={progress}, ring={i}, depth={hit}"
                            );
                        }
                    }
                }
            }
        }
    }
}
