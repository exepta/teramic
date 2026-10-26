use super::{PropDetail, PropMesh, PropPalette, geometry::add_box};
use bevy_math::{Quat, Vec3};
use std::{collections::BTreeMap, sync::OnceLock};

const SILK: [f32; 4] = [0.88, 0.81, 0.68, 1.0];

pub(super) fn template(
    variant: u8,
    detail: PropDetail,
    palette: &PropPalette,
) -> Result<Vec<PropMesh>, String> {
    if variant >= crate::props::webs::FIRST_CONSTRUCTION {
        return construction(variant, detail, palette);
    }
    let mut batches = BTreeMap::new();
    let steps = match detail {
        PropDetail::Near => 5,
        PropDetail::Middle => 3,
        PropDetail::Far => 2,
    };
    let width = if detail == PropDetail::Far {
        0.021
    } else {
        0.014
    } * (1.0 + f32::from(variant % 10) * 0.015);
    let material = palette.crop;
    let mut silk = |a: Vec3, b: Vec3, thick: f32| -> Result<(), String> {
        let delta = b - a;
        if delta.length_squared() < 0.000001 {
            return Ok(());
        }
        let old = batches
            .get(&material)
            .map_or(0, |mesh: &PropMesh| mesh.positions.len());
        add_box(
            &mut batches,
            material,
            (a + b) * 0.5,
            Vec3::new(thick, delta.length(), thick),
            Quat::from_rotation_arc(Vec3::Y, delta.normalize()),
        )?;
        let mesh = batches.get_mut(&material).unwrap();
        mesh.colors.resize(mesh.positions.len(), SILK);
        mesh.colors[old..].fill(SILK);
        Ok(())
    };
    let variant = variant % 10;
    if variant == 3 {
        silk(Vec3::ZERO, Vec3::new(0.0, -0.65, 0.0), width * 1.5)?;
        for ring in 0..6 {
            let y = -0.7 - ring as f32 * 0.20;
            let radius = [0.12, 0.28, 0.39, 0.34, 0.23, 0.09][ring];
            let next_radius = [0.28, 0.39, 0.34, 0.23, 0.09, 0.015][ring];
            for edge in 0..8 {
                let angle = edge as f32 * std::f32::consts::FRAC_PI_4;
                let next = angle + std::f32::consts::FRAC_PI_4;
                let a = Vec3::new(angle.cos() * radius, y, angle.sin() * radius);
                let b = Vec3::new(next.cos() * radius, y, next.sin() * radius);
                silk(a, b, width * 1.5)?;
                silk(
                    a,
                    Vec3::new(next.cos() * next_radius, y - 0.2, next.sin() * next_radius),
                    width,
                )?;
            }
        }
        return Ok(batches.into_values().collect());
    }
    let horizontal = variant == 5;
    let ceiling = variant == 2;
    let center = if horizontal {
        Vec3::new(0.0, 0.14, 0.0)
    } else {
        Vec3::new(0.0, -0.85, if variant == 1 { 0.5 } else { 0.08 })
    };
    let spokes = if matches!(variant, 6 | 8) { 12 } else { 8 };
    let anchor = |i: usize| {
        let angle = i as f32 / spokes as f32 * std::f32::consts::TAU;
        if horizontal {
            Vec3::new(angle.cos(), 0.02, angle.sin())
        } else if ceiling {
            Vec3::new(angle.cos(), -0.02, angle.sin() * 0.65)
        } else {
            Vec3::new(
                angle.cos(),
                (angle.sin() - 1.0) * 0.90,
                if variant == 1 {
                    angle.cos().abs() * 0.85
                } else {
                    0.02
                },
            )
        }
    };
    for spoke in 0..spokes {
        if variant == 7 && spoke % 3 == 0 {
            continue;
        }
        let a = anchor(spoke);
        silk(center, a, width * 1.25)?;
        for ring in 1..=steps {
            if variant == 7 && (ring + spoke) % 4 == 0 {
                continue;
            }
            let t = ring as f32 / (steps + 1) as f32;
            let b = anchor((spoke + 1) % spokes);
            let left = center.lerp(a, t);
            let right = center.lerp(b, t);
            let sag = if horizontal { 0.035 } else { 0.06 } * (1.0 - t);
            let middle = (left + right) * 0.5 - Vec3::Y * sag;
            silk(left, middle, width)?;
            silk(middle, right, width)?;
            if detail == PropDetail::Near && (spoke + ring) % 3 == 0 {
                silk(
                    middle,
                    middle - Vec3::Y * (0.06 + spoke as f32 * 0.009),
                    width * 0.8,
                )?;
            }
        }
    }
    if matches!(variant, 2 | 4 | 6 | 8 | 9) {
        for i in 0..if variant == 6 { 7 } else { 3 } {
            let x = i as f32 / if variant == 6 { 6.0 } else { 2.0 } * 1.8 - 0.9;
            let top = if ceiling {
                Vec3::new(x, -0.05, 0.0)
            } else {
                Vec3::new(x, -0.08, 0.03)
            };
            let bottom =
                top + Vec3::new(0.07 * (i as f32).sin(), -0.9 - (i % 3) as f32 * 0.35, 0.13);
            silk(top, bottom, width)?;
            if variant == 8 {
                for layer in 0..4 {
                    let y = bottom.y - layer as f32 * 0.10;
                    let radius = [0.10, 0.19, 0.16, 0.06][layer];
                    for edge in 0..6 {
                        let a = edge as f32 * std::f32::consts::TAU / 6.0;
                        let b = a + std::f32::consts::TAU / 6.0;
                        silk(
                            Vec3::new(bottom.x + a.cos() * radius, y, bottom.z + a.sin() * radius),
                            Vec3::new(bottom.x + b.cos() * radius, y, bottom.z + b.sin() * radius),
                            width * 2.0,
                        )?;
                    }
                }
            }
        }
    }
    Ok(batches.into_values().collect())
}

pub(super) fn bounds(variant: u8) -> Option<[[f32; 3]; 2]> {
    static BOUNDS: [OnceLock<Option<[[f32; 3]; 2]>>; crate::props::webs::VARIANTS] =
        [const { OnceLock::new() }; crate::props::webs::VARIANTS];
    *BOUNDS.get(usize::from(variant))?.get_or_init(|| {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for detail in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
            for mesh in template(variant, detail, &PropPalette::default()).ok()? {
                for point in mesh.positions {
                    let p = Vec3::from_array(point);
                    min = min.min(p);
                    max = max.max(p);
                }
            }
        }
        Some([min.to_array(), max.to_array()])
    })
}

fn construction(
    variant: u8,
    detail: PropDetail,
    palette: &PropPalette,
) -> Result<Vec<PropMesh>, String> {
    if usize::from(variant) >= crate::props::webs::VARIANTS {
        return Err("Unknown spider web construction".into());
    }
    let mut batches = BTreeMap::new();
    let mut error = None;
    crate::props::webs::visit_strands(
        variant,
        match detail {
            PropDetail::Near => 0,
            PropDetail::Middle => 1,
            PropDetail::Far => 2,
        },
        |a, b, thickness| {
            if error.is_some() {
                return;
            }
            let delta = b - a;
            if let Err(cause) = add_box(
                &mut batches,
                palette.crop,
                (a + b) * 0.5,
                Vec3::new(thickness, delta.length(), thickness),
                Quat::from_rotation_arc(Vec3::Y, delta.normalize_or_zero()),
            ) {
                error = Some(cause);
            }
        },
    );
    if let Some(error) = error {
        return Err(error);
    }
    for mesh in batches.values_mut() {
        mesh.colors = vec![SILK; mesh.positions.len()];
    }
    Ok(batches.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_web_constructions_are_batched_indexed_open_and_distinct() {
        let mut hashes = std::collections::HashSet::new();
        for variant in 0..10 {
            let mut counts = Vec::new();
            for detail in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
                let parts = template(variant, detail, &PropPalette::default()).unwrap();
                assert_eq!(parts.len(), 1);
                let mesh = &parts[0];
                assert!(
                    mesh.indices
                        .iter()
                        .all(|&i| (i as usize) < mesh.positions.len())
                );
                assert_eq!(mesh.colors.len(), mesh.positions.len());
                assert!(
                    mesh.positions
                        .iter()
                        .flatten()
                        .all(|v| v.is_finite() && v.abs() < 2.5)
                );
                assert!(mesh.positions.len() < 10_000);
                counts.push(mesh.positions.len());
                if detail == PropDetail::Near {
                    let fingerprint = mesh.positions.iter().flatten().fold(0_u64, |h, v| {
                        h.wrapping_mul(1099511628211) ^ u64::from(v.to_bits())
                    });
                    assert!(hashes.insert(fingerprint), "duplicate variant {variant}");
                }
            }
            assert!(counts[2] <= counts[0]);
            assert!(bounds(variant).is_some());
        }
    }

    #[test]
    fn spanning_networks_are_three_dimensional_connected_bounded_single_batches_at_every_lod() {
        let mut variants = std::collections::HashSet::new();
        for variant in crate::props::webs::FIRST_CONSTRUCTION..crate::props::webs::VARIANTS as u8 {
            let mut counts = Vec::new();
            for detail in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
                let meshes = template(variant, detail, &PropPalette::default()).unwrap();
                assert_eq!(meshes.len(), 1);
                let mesh = &meshes[0];
                assert!(mesh.positions.len() < 8_000);
                assert!(
                    mesh.indices
                        .iter()
                        .all(|&i| (i as usize) < mesh.positions.len())
                );
                assert_eq!(mesh.colors.len(), mesh.positions.len());
                let bounds = bounds(variant).unwrap();
                assert!(bounds[1][0] - bounds[0][0] >= 10.0);
                assert!(bounds[1][2] - bounds[0][2] > 0.5);
                assert!(bounds[1][1] - bounds[0][1] > 1.5);
                assert!(mesh.positions.iter().flatten().all(|p| p.is_finite()));
                counts.push(mesh.positions.len());
                if detail == PropDetail::Near {
                    let hash = mesh.positions.iter().flatten().fold(0_u64, |h, p| {
                        h.wrapping_mul(1099511628211) ^ u64::from(p.to_bits())
                    });
                    assert!(variants.insert(hash));
                }
            }
            assert!(counts[0] > counts[1] && counts[1] > counts[2]);
        }
    }
}
