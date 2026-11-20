//! Project the actual cached geometry and shipped PNG alpha together. Counts
//! alone cannot detect a hollow ring of huge leaves around an empty face.
use super::*;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};

fn covered(triangles: &[Triangle], point: Vec2, image: &Image) -> bool {
    triangles.iter().any(|triangle| {
        let [a, b, c] = triangle.points.map(|p| p.truncate());
        let ab = b - a;
        let ac = c - a;
        let ap = point - a;
        let denominator = ab.perp_dot(ac);
        let u = ap.perp_dot(ac) / denominator;
        let v = ab.perp_dot(ap) / denominator;
        if u < 0.0 || v < 0.0 || u + v > 1.0 {
            return false;
        }
        let [ua, ub, uc] = triangle.uvs.map(Vec2::from_array);
        let uv = ua + (ub - ua) * u + (uc - ua) * v;
        let x = (uv.x.clamp(0.0, 1.0) * (image.width() - 1) as f32) as usize;
        let y = (uv.y.clamp(0.0, 1.0) * (image.height() - 1) as f32) as usize;
        image.data.as_ref().unwrap()[(y * image.width() as usize + x) * 4 + 3] >= 128
    })
}

#[test]
fn actual_sprays_are_dense_with_small_gaps_instead_of_an_empty_cube_center() {
    const GRID: usize = 64;
    for style in [
        "oak",
        "autumn",
        "birch",
        "apple",
        "cherry_pink",
        "cherry_white",
        "spirit_pink",
        "spirit_pearl",
        "spirit_lavender",
        "blood",
        "spruce",
    ] {
        let image = Image::from_buffer(
            &std::fs::read(
                terra_core::asset_directory()
                    .join(format!("terra/blocks/textures/foliage/{style}_cluster.png")),
            )
            .unwrap(),
            ImageType::Extension("png"),
            CompressedImageFormats::empty(),
            true,
            ImageSampler::default(),
            bevy::asset::RenderAssetUsages::MAIN_WORLD,
        )
        .unwrap();
        let family = match style {
            "birch" => 512,
            "spruce" => 1024,
            _ => 0,
        };
        for variant in 0..4 {
            let triangles = template(family + (1 << 5) * 4 + variant);
            let mut filled = 0;
            let mut holes = [[0_usize; GRID + 1]; GRID + 1];
            let mut largest_hole = 0;
            for y in 0..GRID {
                for x in 0..GRID {
                    let point = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) / GRID as f32;
                    if covered(triangles, point, &image) {
                        filled += 1;
                    } else {
                        holes[y + 1][x + 1] =
                            1 + holes[y][x].min(holes[y][x + 1]).min(holes[y + 1][x]);
                        largest_hole = largest_hole.max(holes[y + 1][x + 1]);
                    }
                }
            }
            let coverage = filled as f32 / (GRID * GRID) as f32;
            assert!(
                (0.70..0.985).contains(&coverage),
                "{style}/{variant} should remain dense with real gaps: {coverage}"
            );
            assert!(
                largest_hole <= GRID / 6,
                "{style}/{variant}: no bare hole wider than one sixth of a block ({largest_hole})"
            );
        }
    }
}
