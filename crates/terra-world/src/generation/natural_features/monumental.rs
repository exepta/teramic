//! Ecology ten: sweeping sky arches and curved, split mountain claws.
//! Connected voxel spines leave large real openings and individually rooted feet.
use super::Cell;
use std::collections::BTreeSet;

pub(super) fn shape(seed: u32) -> Vec<Cell> {
    let mut cells = BTreeSet::new();
    let arch = seed & 1 == 0;
    let height = if arch {
        18.0 + f64::from((seed >> 3) % 6)
    } else {
        21.0 + f64::from((seed >> 3) % 5)
    };
    let lean = 1.35 + f64::from((seed >> 12) % 5) * 0.10;
    for step in 0..=160 {
        let t = step as f64 / 160.0;
        let wave = (std::f64::consts::PI * t).sin();
        let (spine, radius) = if arch {
            (
                [
                    -9.0 + 18.0 * t + wave * lean,
                    wave.powf(0.84) * height,
                    (t * std::f64::consts::TAU).sin() * 2.8,
                ],
                1.85 + 0.38 * (t * 8.0 + f64::from(seed % 5)).sin() + (1.0 - wave) * 0.50,
            )
        } else {
            (
                [
                    (t * 1.9).sin() * 7.0 * t,
                    t * height,
                    (t * 2.8).sin() * 3.4 * t,
                ],
                3.1 * (1.0 - t).powf(0.72) + 0.58,
            )
        };
        stamp(&mut cells, spine, radius);
    }
    if !arch {
        // The unequal counter-claw joins the main root, opening a tall notch
        // rather than producing two regular upright pillars.
        for step in 0..=112 {
            let t = step as f64 / 112.0;
            stamp(
                &mut cells,
                [
                    -(t * 1.65).sin() * 6.4 * t,
                    t * height * 0.73,
                    (t * 2.5).sin() * 3.5,
                ],
                2.05 * (1.0 - t).powf(0.8) + 0.55,
            );
        }
    }
    cells.into_iter().collect()
}

fn stamp(cells: &mut BTreeSet<Cell>, spine: [f64; 3], radius: f64) {
    let base = spine.map(|value| value.floor() as i64);
    let reach = radius.ceil() as i64;
    for z in -reach..=reach {
        for y in -reach..=reach {
            for x in -reach..=reach {
                let p = [base[0] + x, base[1] + y, base[2] + z];
                if p[1] >= 0
                    && (0..3)
                        .map(|axis| (p[axis] as f64 - spine[axis]).powi(2))
                        .sum::<f64>()
                        <= radius * radius
                {
                    cells.insert(p);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn monumental_silhouettes_are_connected_rooted_open_and_bounded() {
        for seed in 0..64 {
            let cells: BTreeSet<_> = shape(seed).into_iter().collect();
            let root = *cells.first().unwrap();
            let mut seen = BTreeSet::from([root]);
            let mut queue = VecDeque::from([root]);
            while let Some(cell) = queue.pop_front() {
                for axis in 0..3 {
                    for offset in [-1, 1] {
                        let mut neighbor = cell;
                        neighbor[axis] += offset;
                        if cells.contains(&neighbor) && seen.insert(neighbor) {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
            assert_eq!(seen.len(), cells.len(), "detached voxel in seed {seed}");
            assert!(cells.iter().any(|p| p[1] == 0));
            assert!((18..=25).contains(&cells.iter().map(|p| p[1]).max().unwrap()));
            assert!(cells.len() < 4500, "unbounded formation in seed {seed}");
            assert!(cells.iter().all(|p| p[0].abs() <= 13 && p[2].abs() <= 13));
            if seed & 1 == 0 {
                assert!(cells.iter().any(|p| p[0] <= -9 && p[1] == 0));
                assert!(cells.iter().any(|p| p[0] >= 9 && p[1] == 0));
                for y in 1..=8 {
                    assert!(!cells.contains(&[0, y, 0]), "closed arch in seed {seed}");
                }
                let width = cells.iter().map(|p| p[0]).max().unwrap()
                    - cells.iter().map(|p| p[0]).min().unwrap();
                assert!(width >= 20);
            } else {
                assert!(cells.iter().any(|p| p[0] >= 5 && p[1] >= 18));
                assert!(cells.iter().any(|p| p[0] <= -5 && p[1] >= 12));
            }
            assert_eq!(cells.into_iter().collect::<Vec<_>>(), shape(seed));
        }
    }
}
