use super::{PetMenuView, PetPortrait};
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    ui::widget::ImageNode,
};

#[derive(Resource, Default)]
struct PetArt {
    rings: [Handle<Image>; 3],
    plaques: [Handle<Image>; 4],
    heart: Handle<Image>,
    paw: Handle<Image>,
}

const RING_SIZE: usize = 1024;
const PLAQUE_WIDTH: usize = 512;
const PLAQUE_HEIGHT: usize = 224;

pub(super) fn install(app: &mut App) {
    app.init_resource::<PetArt>()
        .add_systems(Startup, prepare)
        .add_systems(PostUpdate, bind.before(bevy::ui::UiSystems::Prepare));
}

fn texture(width: u32, height: u32, pixels: Vec<u8>) -> Image {
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn ring(selected: usize) -> Vec<u8> {
    let mut pixels = vec![0; RING_SIZE * RING_SIZE * 4];
    let caption = [
        Vec2::new(346.0, 826.0),
        Vec2::new(678.0, 826.0),
        Vec2::new(716.0, 900.0),
        Vec2::new(686.0, 972.0),
        Vec2::new(338.0, 972.0),
        Vec2::new(308.0, 900.0),
    ];
    for y in 0..RING_SIZE {
        for x in 0..RING_SIZE {
            let point = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let p = point - Vec2::new(512.0, 550.0);
            let radius = p.length();
            let angle = p.x.atan2(-p.y);
            let grain = ((x.wrapping_mul(73) ^ y.wrapping_mul(131)) & 15) as f32;
            let mut color = [0; 4];
            paint(
                &mut color,
                [17, 21, 19, 247],
                (302.0 - radius).clamp(0.0, 1.0),
            );
            let side_selected = angle.abs() >= 0.76 && selected == if angle < 0.0 { 1 } else { 2 };
            frame(
                &mut color,
                (300.0 - radius).max(radius - 440.0),
                side_selected,
                grain,
                false,
            );

            // Etched diamonds and inset rivets stay in the shared frame, not separate UI entities.
            if (307.0..433.0).contains(&radius) && angle.abs() > 0.9 {
                let sector_angle = std::f32::consts::PI / 4.0;
                let local_angle = angle - (angle / sector_angle).round() * sector_angle;
                let etched = ((radius - 369.0).abs() + (local_angle * radius).abs() - 12.0).abs();
                paint(
                    &mut color,
                    [128, 110, 73, 34],
                    (1.5 - etched).clamp(0.0, 1.0),
                );
            }
            let seam = (angle.abs() - 0.76).abs() * radius;
            if (299.0..441.0).contains(&radius) {
                paint(
                    &mut color,
                    [173, 142, 87, 255],
                    (2.0 - seam).clamp(0.0, 1.0),
                );
            }
            let tick = std::f32::consts::PI / 6.0;
            let delta = angle - (angle / tick).round() * tick;
            let diamond = (radius - 438.0).abs() + (delta * radius).abs();
            paint(
                &mut color,
                [47, 37, 23, 255],
                (8.0 - diamond).clamp(0.0, 1.0),
            );
            paint(
                &mut color,
                [212, 176, 107, 255],
                (5.5 - diamond).clamp(0.0, 1.0),
            );

            let wedge = (190.0 - radius)
                .max(radius - 464.0)
                .max((angle.abs() - 0.76) * radius);
            frame(&mut color, wedge, selected == 0, grain, true);
            for (index, center, radius) in [
                (1, Vec2::new(132.0, 570.0), 115.0),
                (2, Vec2::new(892.0, 570.0), 115.0),
                (0, Vec2::new(512.0, 160.0), 110.0),
            ] {
                frame(
                    &mut color,
                    point.distance(center) - radius,
                    selected == index,
                    grain,
                    index == 0,
                );
            }
            frame(
                &mut color,
                polygon_distance(point, &caption),
                false,
                grain,
                false,
            );
            let bottom_diamond = (point.x - 512.0).abs() + (point.y - 991.0).abs();
            paint(
                &mut color,
                [79, 59, 29, 255],
                (17.0 - bottom_diamond).clamp(0.0, 1.0),
            );
            paint(
                &mut color,
                [222, 180, 99, 255],
                (12.0 - bottom_diamond).clamp(0.0, 1.0),
            );
            paint(
                &mut color,
                [255, 219, 139, 255],
                (6.0 - bottom_diamond).clamp(0.0, 1.0),
            );
            let selected_tip = (point.x - 512.0).abs() + (point.y - 363.0).abs();
            if selected == 0 {
                paint(
                    &mut color,
                    [255, 223, 137, 255],
                    (14.0 - selected_tip).clamp(0.0, 1.0),
                );
            }
            let index = (x + y * RING_SIZE) * 4;
            pixels[index..index + 4].copy_from_slice(&color);
        }
    }
    pixels
}

fn paint(pixel: &mut [u8; 4], color: [u8; 4], coverage: f32) {
    let alpha = f32::from(color[3]) / 255.0 * coverage;
    if alpha <= 0.0 {
        return;
    }
    let previous = f32::from(pixel[3]) / 255.0 * (1.0 - alpha);
    let total = alpha + previous;
    for channel in 0..3 {
        pixel[channel] =
            ((f32::from(color[channel]) * alpha + f32::from(pixel[channel]) * previous) / total)
                .round() as u8;
    }
    pixel[3] = (total * 255.0).round() as u8;
}

fn frame(pixel: &mut [u8; 4], distance: f32, selected: bool, grain: f32, glow: bool) {
    if distance > if glow && selected { 12.0 } else { 5.0 } {
        return;
    }
    if glow && selected {
        paint(
            pixel,
            [255, 190, 72, 75],
            (1.0 - distance.max(0.0) / 12.0).powi(2),
        );
    }
    paint(pixel, [4, 5, 5, 140], (4.0 - distance).clamp(0.0, 1.0));
    let light = (grain / 6.0) as u8;
    let color = if distance > -2.5 {
        if selected {
            [255, 222, 145, 255]
        } else {
            [181, 151, 98, 255]
        }
    } else if distance > -6.5 {
        [66, 56, 40, 255]
    } else if distance > -8.5 {
        if selected {
            [184, 141, 68, 255]
        } else {
            [115, 104, 78, 255]
        }
    } else if selected {
        [61 + light, 46 + light, 27 + light, 249]
    } else {
        [27 + light, 29 + light, 27 + light, 246]
    };
    paint(pixel, color, (0.75 - distance).clamp(0.0, 1.0));
}

fn polygon_distance(point: Vec2, vertices: &[Vec2]) -> f32 {
    let mut squared = f32::INFINITY;
    let mut inside = true;
    for (index, &a) in vertices.iter().enumerate() {
        let edge = vertices[(index + 1) % vertices.len()] - a;
        let delta = point - a;
        let nearest = delta - edge * (delta.dot(edge) / edge.length_squared()).clamp(0.0, 1.0);
        squared = squared.min(nearest.length_squared());
        inside &= edge.perp_dot(delta) >= 0.0;
    }
    squared.sqrt() * if inside { -1.0 } else { 1.0 }
}

fn plaque(direction: usize) -> Vec<u8> {
    let mut vertices = vec![
        Vec2::new(14.0, 8.0),
        Vec2::new(498.0, 8.0),
        Vec2::new(506.0, 20.0),
        Vec2::new(506.0, 174.0),
        Vec2::new(494.0, 188.0),
        Vec2::new(18.0, 188.0),
        Vec2::new(6.0, 174.0),
        Vec2::new(6.0, 20.0),
    ];
    if direction != 0 {
        for point in &mut vertices {
            point.y *= 1.12;
        }
    }
    let mut pixels = vec![0; PLAQUE_WIDTH * PLAQUE_HEIGHT * 4];
    for y in 0..PLAQUE_HEIGHT {
        for x in 0..PLAQUE_WIDTH {
            let mut point = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            if direction == 2 {
                point.x = PLAQUE_WIDTH as f32 - point.x;
            }
            if matches!(direction, 1 | 2) {
                point.x *= 1.13;
            }
            let mut color = [0; 4];
            frame(
                &mut color,
                polygon_distance(point, &vertices),
                false,
                (x ^ y) as f32 % 12.0,
                false,
            );
            let arrow = if direction == 0 {
                [
                    Vec2::new(232.0, 186.0),
                    Vec2::new(280.0, 186.0),
                    Vec2::new(256.0, 220.0),
                ]
            } else {
                [
                    Vec2::new(505.0, 83.0),
                    Vec2::new(570.0, 112.0),
                    Vec2::new(505.0, 141.0),
                ]
            };
            if direction < 3 {
                let distance = polygon_distance(point, &arrow);
                paint(
                    &mut color,
                    [98, 70, 33, 255],
                    (1.0 - distance).clamp(0.0, 1.0),
                );
                paint(
                    &mut color,
                    [255, 219, 128, 255],
                    (-2.0 - distance).clamp(0.0, 1.0),
                );
            }
            let index = (x + y * PLAQUE_WIDTH) * 4;
            pixels[index..index + 4].copy_from_slice(&color);
        }
    }
    pixels
}

fn heart() -> Vec<u8> {
    let mut data = vec![0; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let px = (x as f32 + 0.5 - 16.0) / 12.0;
            let py = (16.0 - y as f32 - 0.5) / 12.0;
            if (px * px + py * py - 1.0).powi(3) - px * px * py.powi(3) <= 0.0 {
                let i = (x + y * 32) * 4;
                data[i..i + 4].copy_from_slice(&[240, 78 + (py.max(0.0) * 35.0) as u8, 91, 255]);
            }
        }
    }
    data
}

fn prepare(world: &mut World) {
    let paw = tilt_ui::icon_image(world, tilt_ui::Icon::PawPrint, tilt_ui::IconSize::Px64)
        .and_then(|handle| {
            world
                .resource::<Assets<Image>>()
                .get(&handle)
                .and_then(filled_paw)
        });
    let mut images = world.resource_mut::<Assets<Image>>();
    let rings = std::array::from_fn(|selected| {
        images.add(texture(RING_SIZE as u32, RING_SIZE as u32, ring(selected)))
    });
    let plaques = std::array::from_fn(|direction| {
        images.add(texture(
            PLAQUE_WIDTH as u32,
            PLAQUE_HEIGHT as u32,
            plaque(direction),
        ))
    });
    let heart = images.add(texture(32, 32, heart()));
    let paw = paw.map(|image| images.add(image)).unwrap_or_default();
    world.insert_resource(PetArt {
        rings,
        plaques,
        heart,
        paw,
    });
}

fn filled_paw(source: &Image) -> Option<Image> {
    let width = source.width() as usize;
    let height = source.height() as usize;
    let mut pixels = source.data.clone()?;
    if width != 64 || height != 64 || pixels.len() != width * height * 4 {
        return None;
    }
    let mut outside = vec![false; width * height];
    let mut queue = std::collections::VecDeque::from([0_usize]);
    // Fill the closed regions of the existing TiltUI paw, preserving its actual geometry.
    while let Some(index) = queue.pop_front() {
        if outside[index] || pixels[index * 4 + 3] >= 16 {
            continue;
        }
        outside[index] = true;
        let x = index % width;
        let y = index / width;
        if x > 0 {
            queue.push_back(index - 1);
        }
        if x + 1 < width {
            queue.push_back(index + 1);
        }
        if y > 0 {
            queue.push_back(index - width);
        }
        if y + 1 < height {
            queue.push_back(index + width);
        }
    }
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        if !outside[index] && pixel[3] < 16 {
            pixel[3] = 255;
        }
        pixel[..3].copy_from_slice(&[255, 223, 147]);
    }
    Some(texture(width as u32, height as u32, pixels))
}

fn bind(
    view: Res<PetMenuView>,
    art: Res<PetArt>,
    portrait: Res<PetPortrait>,
    mut nodes: Query<(&tilt_ui::ImageMetadata, &mut ImageNode)>,
) {
    if !view.visible {
        return;
    }
    for (metadata, mut node) in &mut nodes {
        let desired = match metadata.alt.as_deref() {
            Some("Pet command ring") => art.rings[if view.follow {
                0
            } else if view.rest {
                1
            } else {
                2
            }]
            .clone(),
            Some("Pet health heart") => art.heart.clone(),
            Some("Pet command paw") => art.paw.clone(),
            Some("Pet follow plaque") => art.plaques[0].clone(),
            Some("Pet rest plaque") => art.plaques[1].clone(),
            Some("Pet none plaque") => art.plaques[2].clone(),
            Some("Pet compact plaque") => art.plaques[3].clone(),
            Some("Pet preview") => {
                if portrait.ready && portrait.id == Some(view.id) {
                    portrait.image.clone()
                } else {
                    Handle::default()
                }
            }
            _ => continue,
        };
        if node.image != desired {
            node.image = desired;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_shared_radial_images_have_a_quiet_center_and_distinct_selected_sectors() {
        let follow = ring(0);
        assert_eq!(follow.len(), RING_SIZE * RING_SIZE * 4);
        assert!(follow[(512 + 570 * RING_SIZE) * 4 + 3] > 240);
        assert!(follow.chunks_exact(4).filter(|p| p[3] != 0).count() > 300_000);
        assert_ne!(follow, ring(1));
        assert_ne!(follow, ring(2));
        assert!(heart().chunks_exact(4).any(|p| p[3] > 0));
    }

    #[test]
    fn reference_medallions_raised_wedge_and_name_plaque_are_bordered_and_separate() {
        let image = ring(0);
        let pixel = |x, y| &image[(x + y * RING_SIZE) * 4..(x + y * RING_SIZE) * 4 + 4];
        for (x, y) in [(512, 160), (132, 570), (892, 570), (512, 900)] {
            assert!(pixel(x, y)[3] > 240);
        }
        assert!(
            pixel(512, 300)[0] > pixel(512, 900)[0],
            "selected wedge has warm bronze depth"
        );
        assert!(
            pixel(512, 50)[0] > 200,
            "raised paw medallion has its own bright gold rim"
        );
        assert!(
            pixel(512, 991)[0] > 200,
            "bottom ornament finishes the full ring"
        );
        assert!(
            pixel(512, 410)[0] < 30,
            "quiet center does not duplicate the world animal behind its real portrait"
        );
        assert_ne!(plaque(0), plaque(1));
        assert_ne!(plaque(1), plaque(2));
        let compact = plaque(3);
        assert_eq!(compact.len(), PLAQUE_WIDTH * PLAQUE_HEIGHT * 4);
        assert_eq!(compact[(256 + 220 * PLAQUE_WIDTH) * 4 + 3], 0);
    }

    #[test]
    fn shared_frame_assets_are_prepared_once_and_hover_does_not_grow_the_asset_store() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<PetArt>()
            .init_resource::<PetMenuView>()
            .init_resource::<PetPortrait>()
            .add_systems(Startup, prepare)
            .add_systems(Update, bind);
        app.update();
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 10);
        let paw = &app.world().resource::<PetArt>().paw;
        let paw = app.world().resource::<Assets<Image>>().get(paw).unwrap();
        assert_eq!(paw.data.as_ref().unwrap()[(29 + 11 * 64) * 4 + 3], 255);
        let node = app
            .world_mut()
            .spawn((
                tilt_ui::ImageMetadata {
                    alt: Some("Pet command ring".into()),
                    ..default()
                },
                ImageNode::default(),
            ))
            .id();
        for selected in 0..120 {
            {
                let mut view = app.world_mut().resource_mut::<PetMenuView>();
                view.visible = selected % 4 != 0;
                view.follow = selected % 3 == 0;
                view.rest = selected % 3 == 1;
                view.free = selected % 3 == 2;
            }
            app.update();
            assert_eq!(app.world().resource::<Assets<Image>>().len(), 10);
            if selected % 4 != 0 {
                assert_eq!(
                    app.world().get::<ImageNode>(node).unwrap().image,
                    app.world().resource::<PetArt>().rings[selected % 3]
                );
            }
        }
    }
}
