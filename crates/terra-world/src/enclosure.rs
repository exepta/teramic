//! Bounded loaded-cell probes shared by cave lighting and acoustic classification.
#[derive(Debug, Default, Clone, Copy)]
pub struct Enclosure {
    pub cave: f32,
    pub ceiling: f32,
    /// False when a roof ray encountered an unloaded section.
    pub complete: bool,
}

pub fn sample(
    position: [f64; 3],
    surface_height: f64,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Enclosure {
    if !position.iter().all(|v| v.is_finite())
        || !surface_height.is_finite()
        || surface_height <= position[1] + 1.0
    {
        return Enclosure {
            complete: true,
            ..Default::default()
        };
    }
    let origin = position.map(|v| v.floor() as i64);
    if block(origin).is_none() {
        return Enclosure::default();
    }
    let mut coverage = 0.0;
    let mut distance = 0.0;
    let mut complete = true;
    // Include high chambers while keeping probes within the world's vertical span.
    let roof_steps =
        (surface_height.ceil().min(356.0) + 2.0 - origin[1] as f64).clamp(1.0, 486.0) as i64;
    // A fixed world-space quadratic kernel gives continuous coverage across a
    // mouth. A camera-relative integer fan jumped whenever it crossed a cell.
    let axis = |value: f64| {
        let grid = value / 4.0;
        let base = grid.floor() as i64;
        let t = (grid - grid.floor()) as f32;
        [
            ((base - 1) * 4, 0.5 * (1.0 - t).powi(2)),
            (base * 4, 0.5 + t - t * t),
            ((base + 1) * 4, 0.5 * t * t),
        ]
    };
    for (x, wx) in axis(position[0]) {
        for (z, wz) in axis(position[2]) {
            let weight = wx * wz;
            if weight <= 0.0 {
                continue;
            }
            for y in 1..=roof_steps {
                let Some(material) = block([x, origin[1] + y, z]) else {
                    complete = false;
                    continue;
                };
                if terra_voxel::MaterialId::from_raw(material)
                    .is_some_and(terra_voxel::MaterialId::is_opaque)
                {
                    coverage += weight;
                    distance += y as f32 * weight;
                    break;
                }
            }
        }
    }
    Enclosure {
        complete,
        cave: {
            let depth = ((surface_height - position[1] - 1.0) / 6.0).clamp(0.0, 1.0) as f32;
            coverage.clamp(0.0, 1.0) * depth * depth * (3.0 - 2.0 * depth)
        },
        ceiling: if coverage == 0.0 {
            0.0
        } else {
            distance / coverage
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mouth_coverage_changes_continuously_in_both_directions() {
        let roof = |p: [i64; 3]| Some(if p[0] < 0 && p[1] == 20 { 3 } else { 0 });
        let mut previous = sample([-8.0, 0.0, 0.0], 40.0, roof).cave;
        assert_eq!(previous, 1.0);
        for step in -799..=800 {
            let x = f64::from(step) / 100.0;
            let next = sample([x, 0.0, 0.0], 40.0, roof);
            assert!(next.complete);
            assert!(next.cave <= previous + 0.00001);
            assert!((next.cave - previous).abs() < 0.006);
            previous = next.cave;
        }
        assert_eq!(previous, 0.0);
    }

    #[test]
    fn trees_and_missing_chunks_are_not_caves_but_tall_loaded_roofs_are() {
        assert_eq!(sample([0.5, 3.0, 0.5], 50.0, |_| None).cave, 0.0);
        assert_eq!(sample([0.5, 3.0, 0.5], 50.0, |_| Some(7)).cave, 0.0);
        for transparent in [14, 15, 27, 28, 29, 40] {
            assert_eq!(
                sample([0.5, 3.0, 0.5], 50.0, |_| Some(transparent)).cave,
                0.0
            );
        }
        let cave = sample([0.5, 3.0, 0.5], 60.0, |p| {
            Some(if p[1] == 43 { 3 } else { 0 })
        });
        assert_eq!(cave.cave, 1.0);
        assert_eq!(cave.ceiling, 40.0);
        assert!(cave.complete);
        assert!(!sample([0.5, 3.0, 0.5], 60.0, |p| (p[1] < 10).then_some(0)).complete);
    }

    #[test]
    fn broad_deep_chambers_find_their_roof_without_unbounded_or_unloaded_probes() {
        let cave = sample([0.5, -80.0, 0.5], 120.0, |p| {
            Some(if p[1] == 90 { 3 } else { 0 })
        });
        assert_eq!(cave.cave, 1.0);
        assert_eq!(cave.ceiling, 170.0);
        assert!(cave.complete);
        let mut probes = 0;
        let open = sample([0.5, -128.0, 0.5], 100_000.0, |_| {
            probes += 1;
            Some(0)
        });
        assert_eq!(open.cave, 0.0);
        assert!(probes <= 9 * 486 + 1);
        let missing = sample([0.5, -80.0, 0.5], 120.0, |p| (p[1] < -48).then_some(0));
        assert_eq!(missing.cave, 0.0);
        assert!(!missing.complete);
        let separated = sample([0.5, -80.0, 0.5], 120.0, |p| match p[1] {
            -80..=-48 => Some(0),
            100..=120 => Some(3),
            _ => None,
        });
        assert_eq!(separated.cave, 1.0);
        assert_eq!(separated.ceiling, 180.0);
        assert!(!separated.complete);
        assert_eq!(sample([0.0; 3], f64::INFINITY, |_| Some(3)).cave, 0.0);
    }
}
