use super::*;

fn bounds(mesh: &MeshBatch, transform: impl Fn(Vec3) -> Vec3) -> (Vec3, Vec3) {
    mesh.positions.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(min, max), &p| {
            let p = transform(Vec3::from_array(p));
            (min.min(p), max.max(p))
        },
    )
}

#[test]
fn all_tools_are_detailed_bounded_three_dimensional_shared_solids() {
    let mut total_vertices = 0;
    for item in ToolKind::ALL
        .map(ItemKind::Tool)
        .into_iter()
        .chain([ItemKind::Stick])
    {
        let models = batches(item, 42).unwrap();
        assert_eq!(
            models.len(),
            1,
            "One cached material draw per tool: {item:?}"
        );
        let mesh = &models[0];
        assert_eq!(mesh.material, 42);
        let minimum = if profile(item).is_some_and(|(shape, _)| shape == Shape::Spoon) {
            256
        } else {
            1_000
        };
        assert!(mesh.positions.len() >= minimum, "{item:?}");
        assert!(
            mesh.positions.len() <= 16_000,
            "{item:?}: {} vertices",
            mesh.positions.len()
        );
        assert_eq!(mesh.positions.len(), mesh.colors.len());
        assert_eq!(mesh.positions.len(), mesh.normals.len());
        assert_eq!(mesh.positions.len(), mesh.uvs.len());
        assert_eq!(mesh.indices.len() % 3, 0);
        let (min, max) = bounds(mesh, |p| p);
        assert!(
            min.cmpge(Vec3::splat(-1e-6)).all() && max.cmple(Vec3::splat(1.0 + 1e-6)).all(),
            "{item:?}: {min:?} .. {max:?}"
        );
        assert!(max.z - min.z > 0.07, "No flat icon geometry: {item:?}");
        for triangle in mesh.indices.chunks_exact(3) {
            let vertices = [triangle[0], triangle[1], triangle[2]].map(|i| {
                assert!(
                    (i as usize) < mesh.positions.len(),
                    "{item:?}: invalid index"
                );
                Vec3::from_array(mesh.positions[i as usize])
            });
            let normal = Vec3::from_array(mesh.normals[triangle[0] as usize]);
            let cross = (vertices[1] - vertices[0]).cross(vertices[2] - vertices[0]);
            assert!(
                cross.length_squared() > 1e-15,
                "{item:?}: degenerate triangle {vertices:?}"
            );
            assert!(
                normal.is_finite() && (normal.length() - 1.0).abs() < 1e-4,
                "{item:?}: {normal:?}"
            );
            assert!(cross.dot(normal) > 0.0, "{item:?}: inverted winding");
        }
        total_vertices += mesh.positions.len();
    }
    assert!(
        total_vertices < 260_000,
        "Bounded shared tool cache, not per-microvoxel assets: {total_vertices}"
    );
}

#[test]
fn tar_bucket_shows_dark_tar_instead_of_the_water_surface() {
    let tar = batches(ItemKind::Tool(ToolKind::TarBucket), 0)
        .unwrap()
        .remove(0);
    let water = batches(ItemKind::Tool(ToolKind::WaterBucket), 0)
        .unwrap()
        .remove(0);
    assert_eq!(tar.positions, water.positions);
    assert_eq!(tar.normals, water.normals);
    let changed: Vec<_> = tar
        .colors
        .iter()
        .zip(&water.colors)
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed.len(), 36);
    for (tar, water) in changed {
        assert!(tar[0] < 0.05 && tar[1] < 0.05 && tar[2] < 0.05);
        // Mesh colors are linear and face shaded; compare hue and contrast,
        // rather than applying an sRGB brightness threshold to every face.
        assert!(water[2] > water[1] && water[1] > water[0]);
        assert!(water[2] > tar[2] * 8.0);
    }
}

#[test]
fn all_spoons_share_a_real_concave_bowl_but_keep_their_material_palette() {
    let mut colors = std::collections::HashSet::new();
    for tool in [
        ToolKind::WoodSpoon,
        ToolKind::StoneSpoon,
        ToolKind::GoldSpoon,
    ] {
        let item = ItemKind::Tool(tool);
        let mesh = batches(item, 42).unwrap().remove(0);
        let (minimum, maximum) = bounds(&mesh, |p| p);
        assert!(minimum.y.abs() < 1e-6 && (maximum.y - 1.0).abs() < 1e-6);
        assert!(mesh.positions.contains(&[0.5, 0.80, 0.405]));
        assert!(mesh.positions.contains(&[0.5, 0.80, 0.38]));
        assert!(mesh.positions.iter().any(|p| p[1] > 0.75 && p[2] >= 0.55));
        assert!(colors.insert(mesh.colors[0].map(f32::to_bits)));
        assert_eq!(world_length(item), Some(0.75));
        assert!(grip(item).is_some());
    }
}

#[test]
fn real_equipped_dimensions_and_handles_match_every_material_row() {
    for tool in ToolKind::ALL {
        let item = ItemKind::Tool(tool);
        let mesh = batches(item, 0).unwrap().remove(0);
        let (min, max) = bounds(&mesh, |p| cache_vertex(item, p));
        let shape = profile(item).unwrap().0;
        let expected = world_length(item).unwrap();
        assert!(
            (max.y - min.y - expected).abs() < 1e-5
                || matches!(shape, Shape::Bucket(_)) && (max.y - min.y) <= expected,
            "{tool:?}: {min:?} .. {max:?}"
        );
        assert!(min.y.abs() < expected && max.y.abs() < expected);
        let grip = grip(item).unwrap();
        if matches!(shape, Shape::Bucket(_)) {
            assert!(
                grip.y > max.y - 0.06 && grip.y < max.y,
                "bucket grip is on its carrying bow"
            );
        } else if shape == Shape::WateringCan {
            assert!(grip.y > min.y + expected * 0.80 && grip.y < max.y);
            assert!((grip.x + expected * 0.15).abs() < 1e-6);
        } else {
            assert!(
                grip.y > min.y + 0.08 && grip.y < min.y + expected * 0.25,
                "{tool:?}: hand is not on the lower handle"
            );
            assert_eq!(grip.x, 0.0);
        }
        assert_eq!(grip.z, 0.0);
        assert!(
            max.z - min.z >= 0.10,
            "{tool:?}: needs a real socket/haft depth"
        );
    }
}

#[test]
fn farming_models_have_distinct_functional_profiles_for_all_five_tiers() {
    let families = [
        [
            ToolKind::WoodWateringCan,
            ToolKind::IronWateringCan,
            ToolKind::GoldWateringCan,
            ToolKind::DiamondWateringCan,
            ToolKind::RubyWateringCan,
        ],
        [
            ToolKind::WoodHoe,
            ToolKind::IronHoe,
            ToolKind::GoldHoe,
            ToolKind::DiamondHoe,
            ToolKind::RubyHoe,
        ],
        [
            ToolKind::WoodScythe,
            ToolKind::IronScythe,
            ToolKind::GoldScythe,
            ToolKind::DiamondScythe,
            ToolKind::RubyScythe,
        ],
    ];
    for (family, tools) in families.into_iter().enumerate() {
        let mut palettes = std::collections::BTreeSet::new();
        for tool in tools {
            let item = ItemKind::Tool(tool);
            let mesh = batches(item, 0).unwrap().remove(0);
            assert!(
                palettes.insert(
                    Palette::for_material(profile(item).unwrap().1)
                        .body
                        .map(f32::to_bits)
                )
            );
            if family == 0 {
                assert!(
                    mesh.positions.iter().any(|p| p[0] > 0.88 && p[1] > 0.60),
                    "Raised +X watering rose"
                );
                assert!(
                    mesh.positions.iter().any(|p| p[1] > 0.95),
                    "Top carrying bow"
                );
                assert!(
                    mesh.positions.iter().any(|p| p[0] < 0.08),
                    "Open side handle"
                );
            } else {
                assert!(
                    mesh.positions.iter().any(|p| p[0] > 0.90 && p[1] < 0.80),
                    "Working edge curves toward +X"
                );
                assert!(
                    mesh.positions
                        .iter()
                        .filter(|p| p[0] < 0.37 && p[1] > 0.75)
                        .count()
                        < 8,
                    "Not a symmetrical double axe"
                );
            }
        }
    }
}

#[test]
fn six_materials_have_distinct_facets_without_per_material_draws_or_emission() {
    let swords = [
        ToolKind::WoodSword,
        ToolKind::StoneSword,
        ToolKind::IronSword,
        ToolKind::GoldSword,
        ToolKind::DiamondSword,
        ToolKind::RubySword,
    ];
    let mut colors = std::collections::BTreeSet::new();
    for tool in swords {
        let item = ItemKind::Tool(tool);
        let a = batches(item, 17).unwrap().remove(0);
        let b = batches(item, 17).unwrap().remove(0);
        assert_eq!(a.positions, b.positions, "Stable cached geometry: {tool:?}");
        assert_eq!(a.colors, b.colors);
        let p = Palette::for_material(profile(item).unwrap().1);
        assert!(
            colors.insert(p.body.map(f32::to_bits)),
            "Distinct material palette"
        );
        let palette: std::collections::BTreeSet<_> = a
            .colors
            .iter()
            .map(|color| color.map(f32::to_bits))
            .collect();
        assert!(
            palette.len() >= 9,
            "{tool:?}: wrapped wood, leather, body, facets, edges and socket must remain distinct"
        );
        assert!(a.colors.iter().all(|c| c[3] == 1.0));
        assert_eq!(a.material, 17);
    }
}

#[test]
fn broad_double_axe_and_curved_pick_silhouettes_are_not_rectangular_plates() {
    for tool in [
        ToolKind::WoodAxe,
        ToolKind::StoneAxe,
        ToolKind::IronAxe,
        ToolKind::GoldAxe,
        ToolKind::DiamondAxe,
        ToolKind::RubyAxe,
    ] {
        let mesh = batches(ItemKind::Tool(tool), 0).unwrap().remove(0);
        let (min, max) = bounds(&mesh, |p| p);
        assert!(max.x - min.x > 0.70);
        for x in [0.125, 0.875] {
            assert!(
                mesh.positions
                    .iter()
                    .any(|p| (p[0] - x).abs() < 1e-5 && p[1] > 0.8)
            );
        }
        assert!(mesh.positions.iter().any(|p| p[0] < 0.20 && p[1] < 0.70));
        assert!(mesh.positions.iter().any(|p| p[0] > 0.80 && p[1] < 0.70));
    }
    let mesh = batches(ItemKind::Tool(ToolKind::IronPickaxe), 0)
        .unwrap()
        .remove(0);
    for x in [0.066, 0.934] {
        assert!(
            mesh.positions
                .iter()
                .any(|p| (p[0] - x).abs() < 1e-5 && (p[1] - 0.72).abs() < 1e-5)
        );
    }
    assert!(
        mesh.positions
            .iter()
            .any(|p| (p[1] - 1.0).abs() < 1e-6 && p[0] > 0.4 && p[0] < 0.6)
    );
}

#[test]
fn axe_and_pick_heads_have_large_connected_sloped_facets_and_a_thick_backbone() {
    for tool in ToolKind::ALL {
        let item = ItemKind::Tool(tool);
        if !matches!(profile(item).unwrap().0, Shape::Axe | Shape::Pickaxe) {
            continue;
        }
        let mesh = batches(item, 0).unwrap().remove(0);
        let (min, max) = mesh
            .positions
            .iter()
            .copied()
            .map(Vec3::from_array)
            .filter(|p| p.y > 0.70 && (p.x - 0.5).abs() > 0.13)
            .fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(a, b), p| (a.min(p), b.max(p)),
            );
        assert!(
            max.z - min.z > 0.18,
            "{tool:?}: head outside its socket must have actual ridge depth: {min:?}..{max:?}"
        );
        let sloped = mesh
            .indices
            .chunks_exact(3)
            .filter(|triangle| {
                let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                    .map(|i| Vec3::from_array(mesh.positions[i as usize]));
                let n = Vec3::from_array(mesh.normals[triangle[0] as usize]);
                [a, b, c]
                    .iter()
                    .all(|p| p.y > 0.65 && (p.x - 0.5).abs() > 0.065)
                    && (b - a).cross(c - a).length() > 0.001
                    && n.z.abs() > 0.2
                    && n.z.abs() < 0.99
            })
            .count();
        assert!(
            sloped >= 8,
            "{tool:?}: use coherent large sloped facets, not flat caps plus tiny patches: {sloped}"
        );
        assert!(
            mesh.positions.len() < 8_000,
            "A thicker head must not add a microgeometry cache: {tool:?}"
        );
    }
}

#[test]
fn sharpened_head_edges_have_exactly_one_sidewall_and_no_coplanar_rim_overlay() {
    for tool in ToolKind::ALL {
        let item = ItemKind::Tool(tool);
        let (start, end) = match profile(item).unwrap().0 {
            Shape::Axe => (Vec3::new(0.125, 0.85, 0.509), Vec3::new(0.168, 1.0, 0.509)),
            Shape::Pickaxe => (Vec3::new(0.066, 0.72, 0.509), Vec3::new(0.13, 0.856, 0.509)),
            _ => continue,
        };
        let mesh = batches(item, 0).unwrap().remove(0);
        for reflected in [false, true] {
            let mut point = start.lerp(end, 0.37);
            if reflected {
                point.x = 1.0 - point.x;
            }
            let covering = mesh
                .indices
                .chunks_exact(3)
                .filter(|triangle| {
                    let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                        .map(|i| Vec3::from_array(mesh.positions[i as usize]));
                    let ab = b - a;
                    let ac = c - a;
                    let ap = point - a;
                    let normal = ab.cross(ac);
                    if normal.length_squared() < 1e-16
                        || ap.dot(normal).abs() > normal.length() * 1e-6
                    {
                        return false;
                    }
                    let aa = ab.dot(ab);
                    let bb = ab.dot(ac);
                    let cc = ac.dot(ac);
                    let denominator = aa * cc - bb * bb;
                    let u = (cc * ap.dot(ab) - bb * ap.dot(ac)) / denominator;
                    let v = (aa * ap.dot(ac) - bb * ap.dot(ab)) / denominator;
                    u > 1e-5 && v > 1e-5 && u + v < 1.0 - 1e-5
                })
                .count();
            assert_eq!(
                covering, 1,
                "{tool:?}, reflected={reflected}: duplicate sidewalls would z-fight at {point:?}"
            );
        }
    }
}

#[test]
fn inventory_camera_fits_all_tool_geometry_without_special_case_assets() {
    let camera = Transform::from_xyz(2.9, 2.5, 3.5).looking_at(Vec3::splat(0.5), Vec3::Y);
    for tool in ToolKind::ALL {
        let mesh = batches(ItemKind::Tool(tool), 0).unwrap().remove(0);
        for p in mesh.positions {
            let local = camera.rotation.inverse() * (Vec3::from_array(p) - camera.translation);
            assert!(
                local.x.abs() < 1.95 * 0.5 && local.y.abs() < 1.95 * 0.5,
                "{tool:?}: preview clips {local:?}"
            );
            assert!(local.z < 0.0);
        }
    }
}

#[test]
fn wrench_is_a_meter_long_open_forging_with_a_wooden_grip() {
    let item = ItemKind::Tool(ToolKind::Wrench);
    let mesh = batches(item, 73).unwrap().remove(0);
    assert_eq!(mesh.material, 73);
    assert_eq!(world_length(item), Some(1.0));
    let (min, max) = bounds(&mesh, |p| cache_vertex(item, p));
    assert!((max.y - min.y - 1.0).abs() < 1e-5);
    assert!((max.z - min.z - 0.184).abs() < 1e-5);
    assert!(
        grip(item)
            .unwrap()
            .abs_diff_eq(Vec3::new(0.0, -0.30, 0.0), 1e-6)
    );
    assert!(mesh.colors.iter().any(|c| c[0] > c[2] * 2.0));
    assert!(mesh.colors.iter().any(|c| c[2] > c[0]));

    let covers = |point: Vec2| {
        mesh.indices.chunks_exact(3).any(|indices| {
            let [a, b, c] = [indices[0], indices[1], indices[2]]
                .map(|i| Vec3::from_array(mesh.positions[i as usize]).truncate());
            let ab = b - a;
            let ac = c - a;
            let denominator = ab.perp_dot(ac);
            if denominator.abs() < 1e-8 {
                return false;
            }
            let ap = point - a;
            let u = ap.perp_dot(ac) / denominator;
            let v = ab.perp_dot(ap) / denominator;
            u >= 0.0 && v >= 0.0 && u + v <= 1.0
        })
    };
    for y in [0.91, 0.95, 0.99] {
        assert!(!covers(Vec2::new(0.5, y)), "the jaw mouth must stay open");
    }
    assert!(covers(Vec2::new(0.65, 0.86)), "the right jaw must be solid");
    assert!(covers(Vec2::new(0.35, 0.86)), "the left jaw must be solid");
}
