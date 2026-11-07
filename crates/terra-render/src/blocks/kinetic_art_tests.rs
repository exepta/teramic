use super::*;

#[test]
fn conveyor_frames_leave_the_real_shaft_and_bearing_center_visible() {
    let model = load_static_scene("terra/blocks/models/conveyor.gltf", 148);
    assert_eq!(model.len(), 4, "no extra materials or per-detail meshes");
    let wood: Vec<_> = model
        .iter()
        .filter(|primitive| primitive.material == 2)
        .cloned()
        .collect();
    let frame: Vec<_> = model
        .iter()
        .filter(|primitive| primitive.material >= 2)
        .cloned()
        .collect();
    for sample in 0..32 {
        let angle = sample as f32 * std::f32::consts::TAU / 32.0;
        assert!(
            !axial_surface(&wood, 0.132, angle),
            "wood cannot cap the actual shaft envelope"
        );
        assert!(
            !axial_surface(&frame, 0.10, angle),
            "the metal bearings need a real opening"
        );
    }
    assert!(
        frame
            .iter()
            .flat_map(|p| &p.positions)
            .all(|p| p[1] <= terra_core::kinetic::CONVEYOR_TOP + 1e-6)
    );
    assert!(model.iter().map(|p| p.positions.len()).sum::<usize>() <= 1400);
}

fn axial_surface(model: &[PrimitiveTemplate], radius: f32, angle: f32) -> bool {
    let y = 0.5 + angle.cos() * radius;
    let z = 0.5 + angle.sin() * radius;
    model.iter().any(|primitive| {
        primitive.indices.chunks_exact(3).any(|indices| {
            let [a, b, c] = [indices[0], indices[1], indices[2]]
                .map(|index| primitive.positions[index as usize]);
            let by = b[1] - a[1];
            let bz = b[2] - a[2];
            let cy = c[1] - a[1];
            let cz = c[2] - a[2];
            let determinant = by * cz - bz * cy;
            if determinant.abs() < 1e-9 {
                return false;
            }
            let u = ((y - a[1]) * cz - (z - a[2]) * cy) / determinant;
            let v = (by * (z - a[2]) - bz * (y - a[1])) / determinant;
            let x = a[0] + u * (b[0] - a[0]) + v * (c[0] - a[0]);
            u >= -1e-5 && v >= -1e-5 && u + v <= 1.00001 && (-1e-5..=1.00001).contains(&x)
        })
    })
}

#[test]
fn actual_gears_have_thick_open_spokes_and_mechanically_matching_eight_sixteen_teeth() {
    for (file, small, large) in [
        ("wooden", 141, 149),
        ("stone", 142, 150),
        ("iron", 143, 151),
    ] {
        for (suffix, id, radius, teeth) in
            [("gear", small, 0.55, 8), ("large_gear", large, 1.05, 16)]
        {
            let path: &'static str = match (file, suffix) {
                ("wooden", "gear") => "terra/blocks/models/wooden_gear.gltf",
                ("wooden", _) => "terra/blocks/models/wooden_large_gear.gltf",
                ("stone", "gear") => "terra/blocks/models/stone_gear.gltf",
                ("stone", _) => "terra/blocks/models/stone_large_gear.gltf",
                ("iron", "gear") => "terra/blocks/models/iron_gear.gltf",
                ("iron", _) => "terra/blocks/models/iron_large_gear.gltf",
                _ => unreachable!(),
            };
            let model = load_static_scene(path, id);
            assert_eq!(model.len(), 2, "one shared body and one shared metal group");
            assert!(
                model
                    .iter()
                    .map(|primitive| primitive.positions.len())
                    .sum::<usize>()
                    <= 8192
            );
            let body_radii: &[f32] = if teeth == 8 {
                &[0.22, 0.28, 0.32]
            } else {
                &[0.35, 0.55, 0.75]
            };
            for &r in body_radii {
                let mut solid = 0;
                for sample in 0..32 {
                    let angle = (sample as f32 + 0.31) * std::f32::consts::TAU / 32.0;
                    solid += usize::from(axial_surface(&model, r, angle));
                }
                assert!(
                    (1..=24).contains(&solid),
                    "{file} {suffix}: real open windows and structural spokes at radius {r}, hits {solid}/32"
                );
            }
            let outer: Vec<_> = (0..128)
                .map(|sample| {
                    axial_surface(
                        &model,
                        radius - 0.02,
                        (sample as f32 + 0.19) * std::f32::consts::TAU / 128.0,
                    )
                })
                .collect();
            let lobes = (0..outer.len())
                .filter(|&i| outer[i] && !outer[(i + outer.len() - 1) % outer.len()])
                .count();
            assert_eq!(
                lobes, teeth,
                "{file} {suffix}: tooth pitch must match2:1 gearing"
            );
            let body = model
                .iter()
                .find(|primitive| primitive.material == 0)
                .unwrap();
            let broad: Vec<_> = body
                .positions
                .iter()
                .filter(|point| {
                    let r = (point[1] - 0.5).hypot(point[2] - 0.5);
                    r > (if teeth == 8 { 0.20 } else { 0.30 }) && r < radius - 0.10
                })
                .collect();
            assert!(broad.iter().any(|point| point[0] < 0.37));
            assert!(broad.iter().any(|point| point[0] > 0.63));
        }
    }
}

#[test]
fn water_wheel_has_twelve_inward_open_scoop_floors_within_one_shared_geometry_budget() {
    let model = load_static_scene("terra/blocks/models/water_wheel.gltf", 147);
    assert_eq!(model.len(), 2);
    assert!(model.iter().map(|p| p.positions.len()).sum::<usize>() <= 8192);
    let mut cups = [false; 12];
    for primitive in &model {
        for (point, normal) in primitive.positions.iter().zip(&primitive.normals) {
            let radial = Vec3::new(0.0, point[1] - 0.5, point[2] - 0.5);
            let radius = radial.length();
            if (1.435..1.445).contains(&radius)
                && Vec3::from_array(*normal).dot(radial.normalize()) < -0.98
            {
                let turn = radial.z.atan2(radial.y) / std::f32::consts::TAU;
                let cup = (turn * 12.0).round().rem_euclid(12.0) as usize;
                cups[cup] = true;
                assert!((0.14999..=0.85001).contains(&point[0]));
            }
        }
    }
    assert!(cups.into_iter().all(|present| present));
}

#[test]
fn actual_shafts_have_fitted_octagonal_boards_end_straps_and_unchanged_axial_envelopes() {
    for (path, id) in [
        ("terra/blocks/models/wooden_shaft.gltf", 144),
        ("terra/blocks/models/stone_shaft.gltf", 145),
        ("terra/blocks/models/iron_shaft.gltf", 146),
    ] {
        let model = load_static_scene(path, id);
        assert_eq!(model.len(), 2);
        assert!(
            model
                .iter()
                .map(|primitive| primitive.positions.len())
                .sum::<usize>()
                <= 1536
        );
        let body = model
            .iter()
            .find(|primitive| primitive.material == 0)
            .unwrap();
        let metal = model
            .iter()
            .find(|primitive| primitive.material == 1)
            .unwrap();
        assert!(
            body.positions
                .iter()
                .any(|p| (p[1] - 0.5).hypot(p[2] - 0.5) > 0.105)
        );
        assert!(body.positions.iter().any(|p| p[0] == 0.0));
        assert!(body.positions.iter().any(|p| p[0] == 1.0));
        assert!(metal.positions.iter().any(|p| p[0] < 0.05));
        assert!(metal.positions.iter().any(|p| p[0] > 0.95));
        assert!(metal.positions.iter().all(|p| p[0] < 0.15 || p[0] > 0.85));
        for point in model.iter().flat_map(|primitive| &primitive.positions) {
            assert!((0.0..=1.0).contains(&point[0]));
            assert!((point[1] - 0.5).abs() <= 0.13201);
            assert!((point[2] - 0.5).abs() <= 0.13201);
        }
    }
    let directory = terra_core::asset_directory().join("terra/blocks");
    for name in ["wooden_gear", "wooden_large_gear", "wooden_shaft"] {
        let definition: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(directory.join(format!("{name}.json"))).unwrap(),
        )
        .unwrap();
        let material = if name.ends_with("shaft") {
            "kinetic_shaft_wooden"
        } else if name.contains("large") {
            "kinetic_gear_large_wooden"
        } else {
            "kinetic_gear_wooden"
        };
        assert_eq!(
            definition["textures"][material],
            "textures/machinery-oak-hd.png"
        );
        let document: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(directory.join(format!("models/{name}.gltf"))).unwrap(),
        )
        .unwrap();
        assert_eq!(document["materials"][0]["name"], material);
        assert!(
            document["materials"][1]["pbrMetallicRoughness"]["metallicFactor"]
                .as_f64()
                .unwrap()
                >= 0.6
        );
    }
}
