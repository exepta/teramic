//! Bounded TiltUI labels over the live map projection and the game camera.
use super::{fullmap::FullMapState, raster::Projection};
use bevy::prelude::*;
use terra_network::client::ClientSession;
use terra_storage::waypoints::{Waypoint, WaypointBook};
use terra_world::cartography::MapLayer;
use tilt_ui::html_shared;

const MAX_MAP_LABELS: usize = 32;
const MAX_WORLD_LABELS: usize = 24;

#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct WaypointLabel {
    pub id: String,
    pub name: String,
    pub distance: String,
    pub icon: String,
    pub icon_source: String,
    pub glyph_style: String,
    pub style: String,
    pub icon_style: String,
    pub text_style: String,
    pub pinned: bool,
    pub show_label: bool,
}

#[html_shared]
#[derive(Resource, Default, serde::Serialize, PartialEq)]
pub struct WorldWaypointView {
    pub points: Vec<WaypointLabel>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<WorldWaypointView>().add_systems(
        PostUpdate,
        world_labels.after(bevy::transform::TransformSystems::Propagate),
    );
}

pub(super) fn distance_label(distance: f64) -> String {
    if distance < 1000.0 {
        format!("{}m", distance.max(0.0).round() as u64)
    } else {
        let tenths = (distance / 100.0).round() as u64;
        if tenths.is_multiple_of(10) {
            format!("{}km", tenths / 10)
        } else {
            format!("{}.{:01}km", tenths / 10, tenths % 10)
        }
    }
}

pub(super) fn distance(point: &Waypoint, player: [f64; 3]) -> f64 {
    point
        .position
        .iter()
        .zip(player)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn label(point: &Waypoint, distance: f64, pinned: bool) -> WaypointLabel {
    let [r, g, b] = point.color;
    WaypointLabel {
        id: point.id.to_string(),
        name: point.name.clone(),
        distance: distance_label(distance),
        icon: point.icon.key().into(),
        icon_source: super::fullmap::icon_source(&point.icon),
        glyph_style: String::new(),
        style: String::new(),
        icon_style: format!("color: #{r:02x}{g:02x}{b:02x};"),
        text_style: String::new(),
        pinned,
        show_label: true,
    }
}

/// Pins an anchor inside the same circular/rectangular clip as the map shader.
fn circle_radius(size: f32) -> f32 {
    // Cardinal boxes are 18x20px at 43% radius, just outside the terrain clip.
    (size * 0.338).max(10.0) - 3.0
}

fn rectangular_bounds(size: Vec2) -> Rect {
    // Keep the full-map north indicator unobstructed as well as its frame.
    Rect::from_corners(Vec2::new(8.0, 28.0), size - Vec2::splat(8.0))
}

fn pin(point: Vec2, size: Vec2, circular: bool, icon_half: f32) -> (Vec2, bool) {
    let center = size * 0.5;
    let delta = point - center;
    if circular {
        let radius = (circle_radius(size.x) - icon_half).max(1.0);
        let distance = delta.length();
        if distance > radius {
            return (center + delta * (radius / distance), true);
        }
    } else {
        let bounds = rectangular_bounds(size);
        let mut amount = 1.0_f32;
        for axis in 0..2 {
            if delta[axis] > 0.0 {
                amount = amount.min((bounds.max[axis] - icon_half - center[axis]) / delta[axis]);
            } else if delta[axis] < 0.0 {
                amount = amount.min((bounds.min[axis] + icon_half - center[axis]) / delta[axis]);
            }
        }
        if amount < 1.0 {
            return (center + delta * amount.max(0.0), true);
        }
    }
    (point, false)
}

fn label_rect(anchor: Vec2, size: Vec2, circular: bool, icon_half: f32) -> Rect {
    let center = size * 0.5;
    let half = Vec2::new(
        92.0_f32.min(size.x * 0.42),
        if circular { 12.0 } else { 22.0 },
    ) * 0.5;
    let direction = anchor - center;
    let horizontal = Vec2::new(
        if direction.x >= 0.0 { -1.0 } else { 1.0 } * (half.x + icon_half + 4.0),
        0.0,
    );
    let vertical = Vec2::new(
        0.0,
        if direction.y >= 0.0 { -1.0 } else { 1.0 } * (half.y + icon_half + 4.0),
    );
    let candidates = if circular {
        [horizontal, -horizontal, vertical, -vertical]
    } else if direction.x.abs() >= direction.y.abs() {
        [horizontal, vertical, -horizontal, -vertical]
    } else {
        [vertical, horizontal, -vertical, -horizontal]
    };
    let bounds = rectangular_bounds(size);
    let fits = |rectangle: Rect| {
        if circular {
            [
                rectangle.min,
                rectangle.max,
                Vec2::new(rectangle.min.x, rectangle.max.y),
                Vec2::new(rectangle.max.x, rectangle.min.y),
            ]
            .iter()
            .all(|corner| corner.distance_squared(center) <= circle_radius(size.x).powi(2))
        } else {
            rectangle.min.cmpge(bounds.min).all() && rectangle.max.cmple(bounds.max).all()
        }
    };
    for offset in candidates {
        let rectangle = Rect::from_center_half_size(anchor + offset, half);
        if fits(rectangle) {
            return rectangle;
        }
    }
    let desired = anchor + candidates[0];
    let position = if circular {
        center
            + (desired - center).clamp_length_max((circle_radius(size.x) - half.length()).max(0.0))
    } else {
        desired.clamp(bounds.min + half, bounds.max - half)
    };
    Rect::from_center_half_size(position, half)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn map_labels(
    projection: &Projection,
    layer: MapLayer,
    level: i64,
    book: &WaypointBook,
    player: [f64; 3],
    selected: Option<u64>,
    display_size: Vec2,
) -> Vec<WaypointLabel> {
    let cave = match layer {
        MapLayer::Surface => None,
        MapLayer::Cave { level } => Some(level),
    };
    let mut points: Vec<_> = book
        .entries
        .iter()
        .filter(|point| point.cave_level == cave)
        .map(|point| (point, distance(point, player)))
        .collect();
    points.sort_by(|(a, da), (b, db)| {
        (Some(b.id) == selected)
            .cmp(&(Some(a.id) == selected))
            .then_with(|| da.total_cmp(db))
            .then_with(|| a.id.cmp(&b.id))
    });
    let size = projection.size() as f32;
    let icon_half = 16.0 * 0.9 * display_size.x / size * 0.5;
    let mut anchors = Vec::<Vec2>::new();
    points
        .into_iter()
        .filter_map(|(point, distance)| {
            let position =
                Vec2::from_array(projection.project(point.position.map(|v| v as f32), level))
                    * display_size
                    / size;
            let (position, pinned) =
                pin(position, display_size, projection.is_circular(), icon_half);
            // Nearest annotations win at a crowded edge instead of stacking labels.
            if pinned
                && anchors
                    .iter()
                    .any(|p| p.distance_squared(position) < 18.0_f32.powi(2))
            {
                return None;
            }
            anchors.push(position);
            let mut entry = label(point, distance, pinned);
            if projection.is_circular() {
                entry.distance.clear();
                entry.show_label = !pinned;
            }
            let scale = 16.0 * 0.9 * display_size.x / size / 14.0;
            entry
                .icon_style
                .push_str(&format!(" transform: scale({scale:.4});"));
            entry.style = format!(
                "left: {:.3}%; top: {:.3}%;",
                position.x / display_size.x * 100.0,
                position.y / display_size.y * 100.0
            );
            let rectangle = label_rect(position, display_size, projection.is_circular(), icon_half);
            entry.text_style = format!(
                "left: {:.2}px; top: {:.2}px; width: {:.2}px; max-width: {:.2}px; text-align: {};",
                rectangle.min.x - position.x,
                rectangle.min.y - position.y,
                rectangle.width(),
                rectangle.width(),
                if rectangle.center().x < position.x {
                    "right"
                } else {
                    "left"
                },
            );
            Some(entry)
        })
        .take(MAX_MAP_LABELS)
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn world_labels(
    state: Res<FullMapState>,
    session: Option<Res<ClientSession>>,
    menu: Option<Res<crate::menu::MenuState>>,
    scale: Option<Res<UiScale>>,
    cameras: Query<(&Camera, &GlobalTransform), With<terra_player::TerraCamera>>,
    bodies: Query<&Transform, With<terra_player::PlayerBody>>,
    mut view: ResMut<WorldWaypointView>,
) {
    let mut next = WorldWaypointView::default();
    if menu.as_ref().is_none_or(|m| m.world_hud_active())
        && session.as_ref().is_some_and(|s| s.ready && !s.dead())
        && let Some((camera, transform)) = cameras.iter().next()
    {
        let origin = bodies
            .iter()
            .next()
            .map_or(transform.translation(), |b| b.translation);
        let mut points: Vec<_> = state
            .waypoints
            .entries
            .iter()
            .filter(|point| point.show_in_world)
            .map(|point| (point, distance(point, origin.to_array().map(f64::from))))
            .collect();
        points.sort_by(|(a, da), (b, db)| da.total_cmp(db).then_with(|| a.id.cmp(&b.id)));
        let scale = scale.as_ref().map_or(1.0, |s| s.0).max(0.1);
        for (point, distance) in points.into_iter().take(MAX_WORLD_LABELS) {
            if let Ok(screen) = camera.world_to_viewport(
                transform,
                Vec3::from_array(point.position.map(|v| v as f32)) + Vec3::Y * 1.0,
            ) {
                let Some(size) = camera.logical_viewport_size() else {
                    continue;
                };
                if screen.x < 16.0
                    || screen.y < 16.0
                    || screen.x > size.x - 16.0
                    || screen.y > size.y - 40.0
                {
                    continue;
                }
                let mut entry = label(point, distance, false);
                let canvas = size / scale;
                let width = (canvas.x - 16.0).clamp(32.0, 260.0);
                let screen = (screen / scale).clamp(
                    Vec2::new(width * 0.5 + 4.0, 58.0_f32.min(canvas.y * 0.5)),
                    Vec2::new(
                        (canvas.x - width * 0.5 - 4.0).max(width * 0.5 + 4.0),
                        (canvas.y - 4.0).max(58.0_f32.min(canvas.y * 0.5)),
                    ),
                );
                entry.style = format!(
                    "left: {:.1}px; top: {:.1}px; width: {width:.1}px;",
                    screen.x, screen.y
                );
                entry.glyph_style = "width: 36px; height: 36px; min-width: 36px; max-width: 36px; min-height: 36px; max-height: 36px;".into();
                next.points.push(entry);
            }
        }
    }
    if *view != next {
        *view = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_units_keep_compact_kilometer_steps() {
        for (distance, expected) in [
            (0.0, "0m"),
            (42.2, "42m"),
            (999.0, "999m"),
            (1000.0, "1km"),
            (1099.0, "1.1km"),
            (1200.0, "1.2km"),
            (2000.0, "2km"),
            (10234.0, "10.2km"),
        ] {
            assert_eq!(distance_label(distance), expected);
        }
    }

    #[test]
    fn off_map_anchors_pin_to_both_clip_shapes_and_preserve_direction() {
        for circular in [false, true] {
            let center = Vec2::splat(160.0);
            let far = Vec2::new(-5000.0, 2100.0);
            let (p, pinned) = pin(far, Vec2::splat(320.0), circular, 7.2);
            assert!(pinned);
            assert!(
                (p - center)
                    .normalize()
                    .distance((far - center).normalize())
                    < 0.00001
            );
            assert!(p.cmpge(Vec2::ZERO).all() && p.cmplt(Vec2::splat(320.0)).all());
            assert_eq!(
                pin(center, Vec2::splat(320.0), circular, 7.2),
                (center, false)
            );
        }
    }

    #[test]
    fn labels_respect_layers_keep_selection_and_pin_far_points() {
        let mut book = WaypointBook::default();
        book.create("Home", [0.0, 80.0, 0.0], None, [255, 180, 80])
            .unwrap();
        let far = book
            .create("Far", [2400.0, 80.0, 1000.0], None, [80, 180, 255])
            .unwrap();
        book.create("Cave", [0.0, 0.0, 0.0], Some(0), [255; 3])
            .unwrap();
        let projection = Projection::new([0, 0], 192, 0.0, false, false);
        let labels = map_labels(
            &projection,
            MapLayer::Surface,
            80,
            &book,
            [0.0, 80.0, 0.0],
            Some(far),
            Vec2::splat(260.0),
        );
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].name, "Far");
        assert!(labels[0].pinned);
        assert!(labels[0].distance.is_empty());
        assert!(!labels[0].show_label);
        assert!(!labels[1].pinned);
        assert!(labels[1].distance.is_empty());
        assert!(labels[1].show_label);
        assert!(labels[0].icon_style.contains("scale(0.8357)"));
        let full = Projection::at([0.0; 2], 192, 0.0, false, true, 640, false);
        let labels = map_labels(
            &full,
            MapLayer::Surface,
            80,
            &book,
            [0.0, 80.0, 0.0],
            Some(far),
            Vec2::new(900.0, 600.0),
        );
        assert!(labels[0].pinned && labels[0].show_label);
        assert_eq!(labels[0].distance, "2.6km");
    }

    #[test]
    fn crowded_edge_pins_are_deduplicated_and_label_work_is_bounded() {
        let mut book = WaypointBook::default();
        for index in 0..200 {
            book.create(
                &format!("Point {index}"),
                [index as f64 * 40.0, 80.0, 3000.0],
                None,
                [255; 3],
            )
            .unwrap();
        }
        let projection = Projection::new([-100, -100], 192, 1.7, true, true);
        let labels = map_labels(
            &projection,
            MapLayer::Surface,
            80,
            &book,
            [-100.0, 80.0, -100.0],
            None,
            Vec2::splat(260.0),
        );
        assert!(labels.len() <= MAX_MAP_LABELS);
        assert!(labels.iter().all(|point| point.pinned));
        assert!(labels.len() < 12);
    }

    #[test]
    fn all_label_corners_stay_inside_the_map_in_small_and_large_layouts() {
        for (size, circular) in [
            (132.0, true),
            (260.0, true),
            (360.0, true),
            (96.0, false),
            (588.0, false),
        ] {
            let center = Vec2::splat(size * 0.5);
            let icon_half = 16.0 * 0.9 * size / if circular { 320.0 } else { 640.0 } * 0.5;
            for step in 0..72 {
                let angle = step as f32 * std::f32::consts::TAU / 72.0;
                for distance in [0.0, size * 0.1, size * 0.3, size * 3.0] {
                    let (anchor, _) = pin(
                        center + Vec2::from_angle(angle) * distance,
                        Vec2::splat(size),
                        circular,
                        icon_half,
                    );
                    let rect = label_rect(anchor, Vec2::splat(size), circular, icon_half);
                    let corners = [
                        rect.min,
                        rect.max,
                        Vec2::new(rect.min.x, rect.max.y),
                        Vec2::new(rect.max.x, rect.min.y),
                    ];
                    if circular {
                        assert!(
                            corners
                                .iter()
                                .all(|point| point.distance(center) <= circle_radius(size) + 0.001),
                            "{size} {angle} {rect:?}"
                        );
                    } else {
                        let bounds = rectangular_bounds(Vec2::splat(size));
                        assert!(rect.min.cmpge(bounds.min - Vec2::splat(0.001)).all());
                        assert!(rect.max.cmple(bounds.max + Vec2::splat(0.001)).all());
                    }
                }
            }
        }
    }
}
