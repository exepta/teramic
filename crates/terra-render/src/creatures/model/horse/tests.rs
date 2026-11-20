use super::*;
use bevy::mesh::VertexAttributeValues;

fn positions(mesh: &Mesh) -> &[[f32; 3]] {
    let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        panic!("positions");
    };
    p
}

#[test]
fn horse_has_a_broad_chest_tapering_curved_neck_and_long_muzzle_with_original_pivots() {
    let parts = parts(0).unwrap();
    let body = &parts
        .iter()
        .find(|(p, _, _)| matches!(p, Part::Body))
        .unwrap()
        .2;
    let cross_width = |low: f32, high: f32, z: std::ops::Range<f32>| {
        positions(body)
            .iter()
            .filter(|p| p[1] >= low && p[1] <= high && z.contains(&p[2]))
            .map(|p| p[0].abs())
            .fold(0.0_f32, f32::max)
            * 2.0
    };
    let chest = cross_width(0.9, 1.3, -0.7..-0.25);
    let neck = cross_width(1.56, 1.73, -0.85..-0.35);
    assert!((0.57..0.595).contains(&chest), "chest {chest}");
    assert!(neck > 0.24 && neck < chest * 0.80, "neck {neck}");
    let (_, pivot, head) = parts
        .iter()
        .find(|(p, _, _)| matches!(p, Part::Head))
        .unwrap();
    assert_eq!(*pivot, HEAD);
    let tip = positions(head)
        .iter()
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min);
    assert!(tip < -0.49, "long muzzle {tip}");
    let top = positions(head)
        .iter()
        .map(|p| p[1] + pivot.y)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!((1.92..1.96).contains(&top));
    let leg_pivots: Vec<_> = parts
        .iter()
        .filter(|(p, _, _)| matches!(p, Part::Leg(_)))
        .map(|(_, p, _)| *p)
        .collect();
    assert_eq!(
        leg_pivots,
        [
            Vec3::new(-0.175, 0.68, -0.46),
            Vec3::new(0.175, 0.68, -0.46),
            Vec3::new(-0.175, 0.68, 0.46),
            Vec3::new(0.175, 0.68, 0.46)
        ]
    );
}

#[test]
fn chestnut_uses_three_subtle_coherent_tones_without_microtile_noise() {
    let mut tones = std::collections::BTreeSet::new();
    for y in 0..6 {
        for z in 0..14 {
            let color = coat(2, [0.274, 0.72 + y as f32 * 0.10, -0.65 + z as f32 * 0.10]);
            tones.insert(color.map(f32::to_bits));
            assert!(
                color[1] / color[0] > 0.55,
                "warm brown, not saturated dark red"
            );
            assert!((0.93..1.07).contains(&(color[0] / 0.69)));
        }
    }
    assert_eq!(tones.len(), 3);
}
