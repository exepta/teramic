//! Curve braking uses physical rail samples and a bounded forward stopping envelope.
use super::*;
const LATERAL_ACCELERATION: f64 = 2.5;
const BRAKING_ACCELERATION: f64 = 6.0;
fn radius(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let u: [f64; 3] = std::array::from_fn(|i| b[i] - a[i]);
    let v: [f64; 3] = std::array::from_fn(|i| c[i] - b[i]);
    let w: [f64; 3] = std::array::from_fn(|i| c[i] - a[i]);
    let cross = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let norm = |v: [f64; 3]| v.iter().map(|v| v * v).sum::<f64>().sqrt();
    let area = norm(cross);
    if area < 1e-9 {
        f64::INFINITY
    } else {
        norm(u) * norm(v) * norm(w) / (2.0 * area)
    }
}
pub(super) fn curve_limit(
    chunks: &ChunkStore,
    journal: &EditJournal,
    network: &RouteNetwork,
    car: &Vehicle,
    cursor: RailCursor,
    wheelbase: f64,
    direction: f64,
) -> Result<f64, StopReason> {
    let direction = if direction < 0.0 { -1.0 } else { 1.0 };
    let count = ((car.motion.speed_mps.powi(2) / (2.0 * BRAKING_ACCELERATION) + wheelbase + 8.0)
        .clamp(8.0, 256.0)
        / 2.0)
        .ceil() as usize;
    let mut positions = vec![
        sample(
            path(network, cursor).ok_or(StopReason::InvalidTrack)?,
            cursor,
        )
        .position,
    ];
    let mut next = cursor;
    let mut limit = f64::INFINITY;
    for step in 1..=count + 1 {
        let Ok((cursor, _)) = walk(chunks, journal, network, next, direction * 2.0) else {
            break;
        };
        next = cursor;
        positions.push(sample(path(network, next).ok_or(StopReason::InvalidTrack)?, next).position);
        if positions.len() < 3 {
            continue;
        }
        let last = positions.len() - 1;
        let radius = radius(positions[last - 2], positions[last - 1], positions[last]);
        let speed = (radius * LATERAL_ACCELERATION).sqrt().max(4.0);
        if step == 2
            && car.motion.speed_mps.abs() > 10.0
            && car.motion.speed_mps.abs() > speed * 1.5
        {
            return Err(StopReason::Derailed);
        }
        let remaining = (step as f64 - 2.0) * 2.0;
        limit = limit.min((speed * speed + 2.0 * BRAKING_ACCELERATION * remaining).sqrt());
    }
    Ok(limit)
}
/// Evaluate every car's live ballast once; callers take the minimum for the consist.
pub fn speed_limit(
    chunks: &ChunkStore,
    journal: &EditJournal,
    network: &RouteNetwork,
    car: &Vehicle,
    base_speed: f64,
    direction: f64,
) -> Result<f64, StopReason> {
    let (cursor, _, wheelbase) = initialize(chunks, journal, network, car)?;
    let bed = rails::ballast::speed_limit(chunks, journal, cursor.anchor, base_speed)
        .ok_or(StopReason::InvalidTrack)?;
    Ok(bed.min(curve_limit(
        chunks, journal, network, car, cursor, wheelbase, direction,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::train_motion::tests::{fixture, set};
    use terra_core::rails::{CurveSpec, Piece};

    #[test]
    fn a_real_bend_limits_speed_and_critical_overspeed_stops_before_moving() {
        let (mut chunks, mut journal, mut car) = fixture();
        // A separate quarter turn avoids the straight fixture's network.
        let piece = Piece::new_curve(
            [64, 0, 0],
            CurveSpec::new([[8, 2, 0], [8, 2, 176], [144, 2, 320], [320, 2, 320]]).unwrap(),
        )
        .unwrap();
        for cell in rails::cells(piece) {
            set(&mut chunks, cell, piece.material);
            journal.record(cell, piece.material).unwrap();
            journal
                .set_object_state(cell, Some(rails::object_state(piece, cell)))
                .unwrap();
        }
        let network = rails::network(&journal).unwrap();
        let cursor = RailCursor {
            anchor: piece.anchor,
            path: 0,
            distance: 12.0,
            forward: true,
        };
        let limit = curve_limit(&chunks, &journal, &network, &car, cursor, 4.0, 1.0).unwrap();
        assert!((4.0..10.0).contains(&limit), "{limit}");
        car.motion.speed_mps = 30.0;
        assert_eq!(
            curve_limit(&chunks, &journal, &network, &car, cursor, 4.0, 1.0),
            Err(StopReason::Derailed)
        );
        car.motion.speed_mps = 4.0;
        assert!(curve_limit(&chunks, &journal, &network, &car, cursor, 4.0, -1.0).is_ok());
    }

    #[test]
    fn consist_and_live_ballast_limits_apply_the_existing_bonus_only_once() {
        let (mut chunks, mut journal, mut car) = fixture();
        for z in (0..=32).step_by(4) {
            let piece = Piece::new([0, 0, z], terra_core::rails::STRAIGHT, 0).unwrap();
            for support in piece.supports().iter() {
                let cell = std::array::from_fn(|axis| {
                    piece.anchor[axis] + i64::from(support.support[axis])
                });
                set(&mut chunks, cell, 206);
                journal.record(cell, 206).unwrap();
            }
        }
        let graph = rails::network(&journal).unwrap();
        assert_eq!(
            speed_limit(&chunks, &journal, &graph, &car, 4.0, 1.0).unwrap(),
            6.0
        );
        car.motion.brake = false;
        car.motion.throttle = 1;
        car.motion.speed_mps = 6.0;
        let next = advance_with_speed_limit(&chunks, &journal, &graph, &car, 0.05, 6.0).unwrap();
        assert!(next.motion.speed_mps <= 6.0);
        let next = advance_with_speed_limit(&chunks, &journal, &graph, &car, 0.05, 4.0).unwrap();
        assert!(next.motion.speed_mps < 6.0);
    }
}
