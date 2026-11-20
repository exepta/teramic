//! Exercise the final indexed batch, not just individual face projections.
use super::*;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};

fn visible(batch: &MeshBatch, image: &Image, origin: Vec3, ray: Vec3) -> bool {
    batch.indices.chunks_exact(3).any(|indices| {
        let [a, b, c] =
            std::array::from_fn(|i| Vec3::from_array(batch.positions[indices[i] as usize]));
        let ab = b - a;
        let ac = c - a;
        let cross = ray.cross(ac);
        let determinant = ab.dot(cross);
        if determinant.abs() < 1e-7 {
            return false;
        }
        let delta = origin - a;
        let u = delta.dot(cross) / determinant;
        let q = delta.cross(ab);
        let v = ray.dot(q) / determinant;
        if u < 0.0 || v < 0.0 || u + v > 1.0 || ac.dot(q) / determinant <= 0.0 {
            return false;
        }
        let [ua, ub, uc] =
            std::array::from_fn(|i| Vec2::from_array(batch.uvs[indices[i] as usize]));
        let uv = ua + (ub - ua) * u + (uc - ua) * v;
        let x = (uv.x.clamp(0.0, 1.0) * (image.width() - 1) as f32) as usize;
        let y = (uv.y.clamp(0.0, 1.0) * (image.height() - 1) as f32) as usize;
        image.data.as_ref().unwrap()[(y * image.width() as usize + x) * 4 + 3] >= 128
    })
}

#[test]
fn final_indexed_crowns_keep_small_gaps_when_viewed_across_cube_corners() {
    const GRID: usize = 32;
    let block = BlockTemplate {
        surface: None,
        render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
        bevel_width: 0.0,
        faces: std::array::from_fn(|_| Vec::new()),
        face_materials: [0; 6],
        side_cap: None,
        details: Vec::new(),
        model: Vec::new(),
    };
    let image = Image::from_buffer(
        &std::fs::read(
            terra_core::asset_directory().join("terra/blocks/textures/foliage/oak_cluster.png"),
        )
        .unwrap(),
        ImageType::Extension("png"),
        CompressedImageFormats::empty(),
        true,
        ImageSampler::default(),
        bevy::asset::RenderAssetUsages::MAIN_WORLD,
    )
    .unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for x in 0..128 {
        let global = [x, 0, 0];
        let variant = variation(global, 0x004c_414d_494e) % 4;
        if !seen.insert(variant) {
            continue;
        }
        let mut batches = BTreeMap::new();
        append(
            &mut batches,
            &block,
            global,
            [0; 3],
            63,
            DetailLevel::Near,
            CrownStyle {
                material: 7,
                species: None,
                cluster: Some(crate::blocks::FoliageClusterMaterials {
                    leaves: 1,
                    needles: None,
                }),
            },
        )
        .unwrap();
        let batch = &batches[&1];
        for eye in [
            Vec3::new(2.3, 1.65, 2.6), // actual native close-up camera
            Vec3::new(-1.3, 1.65, 2.6),
            Vec3::new(2.3, 2.0, 2.6),
        ] {
            let forward = (Vec3::splat(0.5) - eye).normalize();
            let right = forward.cross(Vec3::Y).normalize();
            let up = right.cross(forward);
            let mut holes = [[0_usize; GRID + 1]; GRID + 1];
            let mut filled = 0;
            let mut largest_hole = 0;
            for y in 0..GRID {
                for x in 0..GRID {
                    let screen = (Vec2::new(x as f32 + 0.5, y as f32 + 0.5) / GRID as f32
                        - Vec2::splat(0.5))
                        * 0.24;
                    let ray = (forward + right * screen.x + up * screen.y).normalize();
                    if visible(batch, &image, eye, ray) {
                        filled += 1;
                    } else {
                        holes[y + 1][x + 1] =
                            1 + holes[y][x].min(holes[y][x + 1]).min(holes[y + 1][x]);
                        largest_hole = largest_hole.max(holes[y + 1][x + 1]);
                    }
                }
            }
            let coverage = filled as f32 / (GRID * GRID) as f32;
            assert!(coverage >= 0.90, "variant {variant}, eye {eye}: {coverage}");
            assert!(
                largest_hole <= GRID / 6,
                "variant {variant}, eye {eye}: detached crown corner ({largest_hole})"
            );
        }
        if seen.len() == 4 {
            break;
        }
    }
    assert_eq!(seen.len(), 4);
}
