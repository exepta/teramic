use terra_core::ClientConfig;
use terra_voxel::SectionPos;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetailLevel {
    Near,
    Middle,
    Far,
}

pub(crate) fn distance_squared(position: SectionPos, focus: [f64; 3]) -> f64 {
    let dy = ((position.y as f64 * 16.0 + 8.0 - focus[1]).abs() - 8.0).max(0.0);
    horizontal_distance_squared(position, focus) + dy * dy
}

fn horizontal_distance_squared(position: SectionPos, focus: [f64; 3]) -> f64 {
    let dx = ((position.x as f64 * 16.0 + 8.0 - focus[0]).abs() - 8.0).max(0.0);
    let dz = ((position.z as f64 * 16.0 + 8.0 - focus[2]).abs() - 8.0).max(0.0);
    dx * dx + dz * dz
}

pub(crate) fn select_level(
    position: SectionPos,
    focus: [f64; 3],
    previous: Option<DetailLevel>,
    config: &ClientConfig,
) -> DetailLevel {
    // Preserve saved relative LOD preferences; extend only block geometry by 50%.
    let scale = if legacy_benchmark_lod() { 1.0 } else { 1.5 };
    select_distances(
        horizontal_distance_squared(position, focus).sqrt(),
        previous,
        config.lod_near_distance * scale,
        config.lod_far_distance * scale,
        config.lod_hysteresis * scale,
    )
}

fn legacy_benchmark_lod() -> bool {
    static LEGACY: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::var("TERRA_BENCH_LEGACY_LOD").is_ok_and(|value| value == "1")
            && std::env::args().any(|arg| arg.starts_with("--smoke-test"))
    });
    *LEGACY
}

/// Sub-centimeter inset walls become subpixel long before terrain LOD changes.
/// Preserve cube outlines/materials, and retain hysteresis for this visual-only layer.
#[cfg(test)]
pub(crate) fn select_micro_detail(
    position: SectionPos,
    focus: [f64; 3],
    previous: Option<bool>,
) -> bool {
    select_configured_micro_detail(
        position,
        focus,
        previous,
        terra_core::BlockDetailConfig::default(),
    )
}

pub(crate) fn select_configured_micro_detail(
    position: SectionPos,
    focus: [f64; 3],
    previous: Option<bool>,
    config: terra_core::BlockDetailConfig,
) -> bool {
    if !config.micro_geometry {
        return false;
    }
    static FULL_BENCH: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::var("TERRA_BENCH_FULL_MICRO").is_ok_and(|value| value == "1")
            && std::env::args().any(|arg| arg.starts_with("--smoke-test"))
    });
    if *FULL_BENCH {
        return true;
    }
    let scale = if legacy_benchmark_lod() { 1.0 } else { 1.5 }
        * f64::from(config.micro_geometry_distance)
        / 48.0;
    distance_squared(position, focus)
        < match previous {
            Some(true) => (36.0_f64 * scale).powi(2),
            Some(false) => (28.0_f64 * scale).powi(2),
            None => (32.0_f64 * scale).powi(2),
        }
}

pub(crate) fn select_prop_level(
    position: SectionPos,
    focus: [f64; 3],
    previous: Option<DetailLevel>,
    config: &ClientConfig,
) -> DetailLevel {
    select_distances(
        distance_squared(position, focus).sqrt(),
        previous,
        config.prop_lod_near_distance,
        config.prop_lod_far_distance,
        config.lod_hysteresis,
    )
}

fn select_distances(
    distance: f64,
    previous: Option<DetailLevel>,
    near: f32,
    far: f32,
    margin: f32,
) -> DetailLevel {
    let near = f64::from(near);
    let far = f64::from(far);
    let margin = f64::from(margin);
    let near = near
        + match previous {
            Some(DetailLevel::Near) => margin,
            Some(_) => -margin,
            None => 0.0,
        };
    let far = far
        + match previous {
            Some(DetailLevel::Far) => -margin,
            Some(_) => margin,
            None => 0.0,
        };
    if distance < near {
        DetailLevel::Near
    } else if distance < far {
        DetailLevel::Middle
    } else {
        DetailLevel::Far
    }
}

#[cfg(test)]
pub(crate) fn detail_level(position: SectionPos, focus: [f64; 3]) -> DetailLevel {
    select_level(position, focus, None, &ClientConfig::default())
}

#[cfg(test)]
mod micro_tests {
    use super::*;
    #[test]
    fn vertical_distance_limits_detail_without_changing_terrain_column_lod() {
        let section = SectionPos::new(-1, -1, -1);
        let config = ClientConfig::default();
        for focus in [[-8.0, 180.0, -8.0], [-8.0, -196.0, -8.0]] {
            assert_eq!(distance_squared(section, focus), 180.0 * 180.0);
            assert!(!select_micro_detail(section, focus, Some(true)));
            assert_eq!(
                select_prop_level(section, focus, None, &config),
                DetailLevel::Far
            );
            assert_eq!(
                select_level(section, focus, None, &config),
                DetailLevel::Near
            );
        }
        assert_eq!(distance_squared(section, [-8.0, -8.0, -8.0]), 0.0);
        assert!(select_micro_detail(section, [-8.0, -8.0, -8.0], None));
        assert_eq!(
            select_prop_level(section, [-8.0, -8.0, -8.0], None, &config),
            DetailLevel::Near
        );
    }

    #[test]
    fn diagonal_detail_uses_nearest_section_corner_and_keeps_hysteresis() {
        let section = SectionPos::new(-1, -1, -1);
        assert_eq!(distance_squared(section, [30.0, 40.0, -8.0]), 2500.0);
        assert!(select_micro_detail(section, [30.0, 40.0, -8.0], Some(true)));
        assert!(!select_micro_detail(
            section,
            [30.0, 40.0, -8.0],
            Some(false)
        ));
    }
    #[test]
    fn mushroom_geometry_lods_outlive_the_micro_relief_range() {
        let section = SectionPos::new(-1, 0, -1);
        let config = ClientConfig::default();
        for (distance, level) in [
            (96.0, DetailLevel::Near),
            (180.0, DetailLevel::Middle),
            (288.0, DetailLevel::Far),
        ] {
            let focus = [distance, 0.0, -8.0];
            assert!(!select_micro_detail(section, focus, None));
            assert_eq!(select_level(section, focus, None, &config), level);
        }
    }

    #[test]
    fn micro_detail_hysteresis_is_independent_of_terrain_lod_and_handles_negative_cells() {
        let section = SectionPos::new(-1, 0, -1);
        let config = ClientConfig::default();
        assert!(select_micro_detail(section, [49.5, 0.0, -8.0], Some(true)));
        assert!(!select_micro_detail(
            section,
            [49.5, 0.0, -8.0],
            Some(false)
        ));
        assert_eq!(
            select_level(section, [33.0, 0.0, -8.0], None, &config),
            DetailLevel::Near
        );
        assert!(!select_micro_detail(section, [55.5, 0.0, -8.0], Some(true)));
        assert!(select_micro_detail(section, [40.5, 0.0, -8.0], Some(false)));
    }
}
