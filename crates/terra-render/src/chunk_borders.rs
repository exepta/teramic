//! Session-local diagnostics: a bounded lattice, rebuilt only after a section change.
use bevy::{
    gizmos::{
        AppGizmoBuilder,
        config::{GizmoConfig, GizmoConfigGroup},
    },
    prelude::*,
};
use terra_core::DebugSettings;
use terra_voxel::{SectionPos, global_to_section};
use terra_world::streaming::ChunkFocus;

/// Chunk diagnostics have their own pixel width; selection and mob gizmos stay unchanged.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct ChunkBorderGizmos;

const WIDTH_MULTIPLIER: f32 = 1.35 * 1.30;
const VERTICAL_COLOR: Color = Color::srgb(1.0, 0.48, 0.06);
const HORIZONTAL_COLOR: Color = Color::srgba(1.0, 0.70, 0.10, 0.65);
const CURRENT_COLOR: Color = Color::srgb(1.0, 0.90, 0.24);

fn border_config() -> GizmoConfig {
    let mut config = GizmoConfig::default();
    config.line.width *= WIDTH_MULTIPLIER;
    config
}

#[derive(Resource, Default)]
struct Grid {
    section: Option<SectionPos>,
    lines: Vec<(Vec3, Vec3, Color)>,
}

pub(crate) fn install(app: &mut App) {
    app.insert_gizmo_config(ChunkBorderGizmos, border_config())
        .init_resource::<Grid>()
        .add_systems(PostUpdate, draw);
}

fn draw(
    settings: Option<Res<DebugSettings>>,
    focus: Res<ChunkFocus>,
    mut grid: ResMut<Grid>,
    mut gizmos: Gizmos<ChunkBorderGizmos>,
) {
    if !settings.is_some_and(|s| s.chunk_borders) || !focus.0.iter().all(|v| v.is_finite()) {
        return;
    }
    let section = global_to_section(focus.0.map(|v| v.floor() as i64)).0;
    if grid.section != Some(section) {
        grid.lines = lattice(section);
        grid.section = Some(section);
    }
    for &(start, end, color) in &grid.lines {
        gizmos.line(start, end, color);
    }
}

fn lattice(section: SectionPos) -> Vec<(Vec3, Vec3, Color)> {
    let Ok(origin) = section.origin() else {
        return Vec::new();
    };
    let base = Vec3::from_array(origin.map(|v| v as f32));
    let mut lines = Vec::with_capacity(300);
    // These are the world's existing inclusive voxel bounds, not new terrain limits.
    for x in -1..=2 {
        for z in -1..=2 {
            let p = base + Vec3::new(x as f32 * 16.0, 0.0, z as f32 * 16.0);
            lines.push((
                Vec3::new(p.x, -128.0, p.z),
                Vec3::new(p.x, 357.0, p.z),
                VERTICAL_COLOR,
            ));
        }
    }
    for y in (-128..=352).step_by(16) {
        for edge in -1..=2 {
            let n = edge as f32 * 16.0;
            lines.push((
                Vec3::new(base.x - 16.0, y as f32, base.z + n),
                Vec3::new(base.x + 32.0, y as f32, base.z + n),
                HORIZONTAL_COLOR,
            ));
            lines.push((
                Vec3::new(base.x + n, y as f32, base.z - 16.0),
                Vec3::new(base.x + n, y as f32, base.z + 32.0),
                HORIZONTAL_COLOR,
            ));
        }
    }
    for corner in 0..8 {
        let point = |bits: usize| {
            base + Vec3::new(
                if bits & 1 == 0 { 0.0 } else { 16.0 },
                if bits & 2 == 0 { 0.0 } else { 16.0 },
                if bits & 4 == 0 { 0.0 } else { 16.0 },
            )
        };
        for axis in [1, 2, 4] {
            if corner & axis == 0 {
                lines.push((point(corner), point(corner | axis), CURRENT_COLOR));
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::gizmos::{
        GizmoAsset,
        config::{DefaultGizmoConfigGroup, GizmoConfigStore},
    };

    #[test]
    fn chunk_width_adds_thirty_percent_to_previous_width_without_changing_other_gizmo_groups() {
        let mut app = App::new();
        app.init_resource::<Assets<GizmoAsset>>()
            .init_gizmo_group::<DefaultGizmoConfigGroup>();
        {
            let mut configs = app.world_mut().resource_mut::<GizmoConfigStore>();
            let (other, _) = configs.config_mut::<DefaultGizmoConfigGroup>();
            other.line.width = 7.0;
            other.enabled = false;
        }
        install(&mut app);
        let configs = app.world().resource::<GizmoConfigStore>();
        let (chunk, _) = configs.config::<ChunkBorderGizmos>();
        assert!(
            (chunk.line.width - GizmoConfig::default().line.width * 1.35 * 1.30).abs() < 0.000_001
        );
        assert!((chunk.line.width - 3.51).abs() < 0.000_001);
        assert!(
            !chunk.line.perspective,
            "thickness is stable in screen pixels"
        );
        let (other, _) = configs.config::<DefaultGizmoConfigGroup>();
        assert_eq!(other.line.width, 7.0);
        assert!(!other.enabled);
    }

    #[test]
    fn all_chunk_lines_use_warm_orange_amber_or_yellow_with_distinct_current_edges() {
        let lines = lattice(SectionPos::new(-2, -1, -3));
        for (_, _, color) in &lines {
            let color = color.to_srgba();
            assert_eq!(color.red, 1.0);
            assert!(color.green > color.blue && color.blue <= 0.24);
            assert!(color.green >= 0.48 && color.green <= 0.90);
        }
        for (color, expected) in [
            (VERTICAL_COLOR, 16),
            (HORIZONTAL_COLOR, 248),
            (CURRENT_COLOR, 12),
        ] {
            assert_eq!(
                lines
                    .iter()
                    .filter(|(_, _, actual)| *actual == color)
                    .count(),
                expected
            );
        }
        assert!(CURRENT_COLOR.to_srgba().green > VERTICAL_COLOR.to_srgba().green);
    }
    #[test]
    fn negative_positions_have_true_vertical_and_horizontal_section_edges_with_bounded_cost() {
        for position in [[-0.1, -0.1, -16.1], [16.0, 32.0, 48.0]] {
            let section = global_to_section(position.map(|v: f64| v.floor() as i64)).0;
            let lines = lattice(section);
            assert_eq!(lines.len(), 276);
            assert!(
                lines
                    .iter()
                    .all(|(a, b, _)| a.is_finite() && b.is_finite() && a != b)
            );
            assert_eq!(lines.iter().filter(|(a, b, _)| a.y != b.y).count(), 20);
            assert!(lines.iter().any(|(a, b, _)| a.y == -128.0 && b.y == 357.0));
            let origin = section.origin().unwrap();
            assert!(
                lines
                    .iter()
                    .any(|(a, b, _)| a.y == origin[1] as f32 && a.y == b.y)
            );
        }
    }
}
