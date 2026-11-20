//! Identical voxel-based traction for client replay and authoritative movement.
use super::{ChunkStore, EditJournal, PLAYER_HALF_HEIGHT, PLAYER_RADIUS};
use bevy::prelude::*;

pub(super) fn horizontal_velocity(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Vec3,
    velocity: Vec3,
    desired: Vec3,
    grounded: bool,
    dt: f32,
) -> Vec3 {
    if !grounded {
        return desired;
    }
    let feet = position - Vec3::Y * (PLAYER_HALF_HEIGHT + 0.06);
    let r = PLAYER_RADIUS * 0.6;
    let coverage = [[0., 0.], [-r, -r], [r, -r], [-r, r], [r, r]]
        .into_iter()
        .filter(|[x, z]| {
            let p = (feet + Vec3::new(*x, 0., *z))
                .floor()
                .as_i64vec3()
                .to_array();
            journal.block(p).or_else(|| chunks.block_at(p)) == Some(49)
        })
        .count() as f32
        / 5.;
    if coverage == 0. {
        return desired;
    }
    let ice_drag = if desired.length_squared() > 0.001 {
        1.8
    } else {
        0.65
    };
    let response = 1. - (-(30. * (1. - coverage) + ice_drag * coverage) * dt).exp();
    Vec3::new(
        velocity.x + (desired.x - velocity.x) * response,
        desired.y,
        velocity.z + (desired.z - velocity.z) * response,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ice_retains_momentum_across_negative_seams_and_stone_brakes_normally() {
        let chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for x in -2..=1 {
            for z in -2..=1 {
                journal.record([x, 0, z], 49).unwrap();
            }
        }
        let position = Vec3::new(0., 1.9, 0.);
        let drive = |velocity, desired, dt| {
            horizontal_velocity(&chunks, &journal, position, velocity, desired, true, dt)
        };
        let mut replay = Vec3::X * 6.;
        for _ in 0..60 {
            replay = drive(replay, Vec3::ZERO, 1. / 60.);
        }
        assert!(
            replay.x > 3. && replay.x < 3.2,
            "ice must glide on input release: {replay}"
        );
        assert!((replay.x - drive(Vec3::X * 6., Vec3::ZERO, 1.).x).abs() < 0.0001);
        let reversing = drive(Vec3::X * 6., -Vec3::X * 6., 1. / 60.);
        assert!(
            reversing.x > 5.5,
            "turning cannot discard existing momentum"
        );
        assert_eq!(
            horizontal_velocity(&chunks, &journal, position, replay, Vec3::ZERO, false, 0.1),
            Vec3::ZERO
        );
        for x in -2..=1 {
            for z in -2..=1 {
                journal.record([x, 0, z], 3).unwrap();
            }
        }
        assert_eq!(
            horizontal_velocity(&chunks, &journal, position, replay, Vec3::ZERO, true, 0.1),
            Vec3::ZERO
        );
    }
}
