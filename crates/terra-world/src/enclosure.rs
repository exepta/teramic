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
        || surface_height <= position[1] + 3.0
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
    let mut roofs = 0;
    let mut distance = 0.0;
    let mut complete = true;
    // Include high chambers while keeping probes within the world's vertical span.
    let roof_steps =
        (surface_height.ceil().min(356.0) + 2.0 - origin[1] as f64).clamp(1.0, 486.0) as i64;
    for x in [-2, 0, 2] {
        for z in [-2, 0, 2] {
            for y in 1..=roof_steps {
                let Some(material) = block([origin[0] + x, origin[1] + y, origin[2] + z]) else {
                    complete = false;
                    continue;
                };
                if terra_voxel::MaterialId::from_raw(material)
                    .is_some_and(terra_voxel::MaterialId::is_opaque)
                {
                    roofs += 1;
                    distance += y as f32;
                    break;
                }
            }
        }
    }
    Enclosure {
        complete,
        cave: (roofs as f32 / 8.0).min(1.0),
        ceiling: if roofs == 0 {
            0.0
        } else {
            distance / roofs as f32
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
