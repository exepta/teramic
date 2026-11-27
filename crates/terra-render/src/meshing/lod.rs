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

/// Leaf cards outside the close band keep their silhouette and paint. Non-oak
/// crown pieces retain their section-wide selection. Only octants containing
/// an exposed oak blade can change the oak mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FoldedLeaves {
    pub(crate) nearby: bool,
    occupied: u8,
    folded: u8,
}

impl FoldedLeaves {
    pub(crate) const FULL: Self = Self {
        nearby: true,
        occupied: u8::MAX,
        folded: u8::MAX,
    };

    pub(crate) fn at(self, cell: [u32; 3]) -> bool {
        self.folded & octant_bit(cell) != 0
    }

    pub(crate) fn at_focus(
        self,
        position: SectionPos,
        focus: [f64; 3],
        previous: Option<Self>,
        level: DetailLevel,
        quality: terra_core::BlockDetailQuality,
    ) -> Self {
        select_folded_leaves(position, focus, previous, level, quality, self.occupied)
    }
}

fn octant_bit(cell: [u32; 3]) -> u8 {
    1 << ((cell[0] >> 3) | ((cell[1] >> 3) << 1) | ((cell[2] >> 3) << 2))
}

/// A greedy face can cross the midpoint on both tangent axes. Its four
/// corners identify all affected octants without expanding it into cells.
pub(crate) fn foliage_octants(faces: &[terra_voxel::meshing::BlockFace]) -> Option<u8> {
    let mut occupied = None;
    for face in faces {
        if !terra_voxel::MaterialId::FOLIAGE.contains(&face.material) {
            continue;
        }
        let bits = occupied.get_or_insert(0);
        if face.material != terra_voxel::MaterialId::Leaves as u16 || face.occupied_neighbor {
            continue;
        }
        for u in [0, face.width - 1] {
            for v in [0, face.height - 1] {
                let mut cell = face.minimum;
                cell[face.u_axis] += u;
                cell[face.v_axis] += v;
                *bits |= octant_bit(cell);
            }
        }
        if *bits == u8::MAX {
            break;
        }
    }
    occupied
}

fn leaf_radius_squared(previous: Option<bool>) -> f64 {
    match previous {
        Some(true) => 36.0 * 36.0,
        Some(false) => 28.0 * 28.0,
        None => 32.0 * 32.0,
    }
}

pub(crate) fn select_folded_leaves(
    position: SectionPos,
    focus: [f64; 3],
    previous: Option<FoldedLeaves>,
    level: DetailLevel,
    quality: terra_core::BlockDetailQuality,
    occupied: u8,
) -> FoldedLeaves {
    let nearby = quality == terra_core::BlockDetailQuality::High
        && level == DetailLevel::Near
        && distance_squared(position, focus) < leaf_radius_squared(previous.map(|s| s.nearby));
    let mut choice = FoldedLeaves {
        nearby,
        occupied,
        folded: 0,
    };
    if !nearby || occupied == 0 {
        return choice;
    }

    let section = [position.x, position.y, position.z];
    let delta =
        std::array::from_fn::<_, 3, _>(|axis| focus[axis] - (section[axis] as f64 * 16.0 + 8.0));
    if delta.iter().map(|v| (v.abs() + 8.0).powi(2)).sum::<f64>() < 28.0 * 28.0 {
        choice.folded = occupied;
        return choice;
    }
    let distances =
        delta.map(|v| [-4.0, 4.0].map(|center| ((v - center).abs() - 4.0).max(0.0).powi(2)));
    let mut remaining = occupied;
    while remaining != 0 {
        let i = remaining.trailing_zeros() as usize;
        let bit = 1 << i;
        let prior = previous
            .filter(|s| s.occupied & bit != 0)
            .map(|s| s.folded & bit != 0);
        let d = distances[0][i & 1] + distances[1][(i >> 1) & 1] + distances[2][(i >> 2) & 1];
        if d < leaf_radius_squared(prior) {
            choice.folded |= bit;
        }
        remaining &= !bit;
    }
    choice
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
    fn folded_crowns_keep_player_quality_and_hysteresis_in_three_dimensions() {
        use terra_core::BlockDetailQuality::{High, Low, Medium};

        let section = SectionPos::new(-1, -1, -1);
        let near = DetailLevel::Near;
        let select = |y, prev, level, quality| {
            select_folded_leaves(section, [-8., y, -8.], prev, level, quality, u8::MAX)
        };
        let entered = select(31., None, near, High);
        assert!(entered.nearby && entered.at([12, 15, 12]));
        assert!(!entered.at([12, 0, 12]));
        let far = select(33., None, near, High);
        assert!(!far.nearby && !far.at([12, 15, 12]));
        assert!(select(35., Some(entered), near, High).at([12, 15, 12]));
        assert!(!select(29., Some(far), near, High).at([12, 15, 12]));
        // Moving through the 28–36 m band retains each half independently.
        assert!(!select(21., Some(entered), near, High).at([12, 0, 12]));
        assert!(select(19., Some(entered), near, High).at([12, 0, 12]));
        for quality in [Medium, Low] {
            assert!(!select(0., None, near, quality).nearby);
        }
        assert!(!select(0., None, DetailLevel::Middle, High).nearby);
    }

    #[test]
    fn folded_leaf_selection_preserves_near_diagonal_corners_and_ignores_empty_octants() {
        use terra_core::BlockDetailQuality::High;
        let section = SectionPos::new(-1, -1, -1);
        let select = |focus, prev, mask| {
            select_folded_leaves(section, focus, prev, DetailLevel::Near, High, mask)
        };
        let near = select([18.; 3], None, u8::MAX);
        assert!(near.at([15; 3]));
        assert!(!near.at([0; 3]));
        let unchanged = select([18.; 3], None, 1);
        assert_eq!(unchanged, select([16.; 3], Some(unchanged), 1));
        let changed = select([17.; 3], Some(unchanged), u8::MAX);
        assert!(
            changed.at([15; 3]),
            "newly exposed octant uses the 32 m initial band"
        );
    }

    #[test]
    fn folded_leaf_occupancy_counts_greedy_oak_faces_crossing_both_midplanes() {
        use terra_voxel::meshing::BlockFace;
        let face = BlockFace {
            minimum: [7, 1, 7],
            width: 2,
            height: 2,
            u_axis: 0,
            v_axis: 2,
            side: 3,
            material: 7,
            occupied_neighbor: false,
            edge_mask: 0,
        };
        assert_eq!(foliage_octants(&[face]), Some(0b0011_0011));
        assert_eq!(
            foliage_octants(&[BlockFace {
                occupied_neighbor: true,
                ..face
            }]),
            Some(0)
        );
        assert_eq!(
            foliage_octants(&[BlockFace {
                material: 8,
                ..face
            }]),
            Some(0)
        );
        assert_eq!(
            foliage_octants(&[BlockFace {
                material: 1,
                ..face
            }]),
            None
        );
    }

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
