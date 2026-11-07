use super::*;
use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};
use terra_core::rails;

fn template() -> BlockTemplate {
    BlockTemplate {
        surface: None,
        render_kind: BlockRenderKind::CustomMesh,
        bevel_width: 0.0,
        faces: std::array::from_fn(|_| Vec::new()),
        face_materials: [0; 6],
        side_cap: None,
        details: Vec::new(),
        model: (0..5)
            .map(|material| PrimitiveTemplate {
                positions: Vec::new(),
                normals: Vec::new(),
                tangents: Vec::new(),
                uvs: Vec::new(),
                indices: Vec::new(),
                material,
            })
            .collect(),
    }
}

fn area(batch: &MeshBatch) -> f64 {
    batch
        .indices
        .chunks_exact(3)
        .map(|triangle| {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                .map(|index| Vec3::from_array(batch.positions[index as usize]));
            f64::from((b - a).cross(c - a).length()) * 0.5
        })
        .sum()
}

fn overlap_area(first: [[f64; 2]; 3], second: [[f64; 2]; 3]) -> f64 {
    let cross = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| {
        (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
    };
    for axis in 0..2 {
        let a_min = first.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min);
        let a_max = first
            .iter()
            .map(|p| p[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        let b_min = second.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min);
        let b_max = second
            .iter()
            .map(|p| p[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        if a_max.min(b_max) - a_min.max(b_min) <= 1.0e-9 {
            return 0.0;
        }
    }
    let direction = cross(second[0], second[1], second[2]).signum();
    let mut polygon = first.to_vec();
    for edge in 0..3 {
        if polygon.is_empty() {
            return 0.0;
        }
        let a = second[edge];
        let b = second[(edge + 1) % 3];
        let mut output = Vec::new();
        let mut previous = *polygon.last().unwrap();
        let mut before = cross(a, b, previous) * direction;
        for current in polygon {
            let after = cross(a, b, current) * direction;
            if (after >= -1.0e-12) != (before >= -1.0e-12) {
                let t = before / (before - after);
                output.push(std::array::from_fn(|axis| {
                    previous[axis] + (current[axis] - previous[axis]) * t
                }));
            }
            if after >= -1.0e-12 {
                output.push(current);
            }
            previous = current;
            before = after;
        }
        polygon = output;
    }
    (0..polygon.len())
        .map(|index| {
            let a = polygon[index];
            let b = polygon[(index + 1) % polygon.len()];
            a[0] * b[1] - a[1] * b[0]
        })
        .sum::<f64>()
        .abs()
        * 0.5
}

#[test]
fn actual_frog_and_crossing_triangles_have_zero_coplanar_overlap_across_all_material_roles() {
    for (material, facing) in [
        (rails::SWITCH_LEFT, 0),
        (rails::SWITCH_LEFT, 4),
        (rails::SWITCH_RIGHT, 0),
        (rails::SWITCH_RIGHT, 4),
        (rails::CROSSING, 0),
    ] {
        let piece = Piece::new([0; 3], material, facing).unwrap();
        let rendered = profile(piece, &template()).unwrap();
        let mut planes = BTreeMap::<[i64; 4], Vec<[[f64; 2]; 3]>>::new();
        let mut overlaps = 0;
        for batch in rendered.full.values() {
            for triangle in batch.indices.chunks_exact(3) {
                let points = [triangle[0], triangle[1], triangle[2]]
                    .map(|index| Vec3::from_array(batch.positions[index as usize]));
                let normal = Vec3::from_array(batch.normals[triangle[0] as usize]);
                let key = [normal.x, normal.y, normal.z, normal.dot(points[0])]
                    .map(|value| (f64::from(value) * 100_000.0).round() as i64);
                let axis = (0..3)
                    .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
                    .unwrap();
                let projected = points.map(|point| {
                    [
                        f64::from(point[(axis + 1) % 3]),
                        f64::from(point[(axis + 2) % 3]),
                    ]
                });
                let earlier = planes.entry(key).or_default();
                for old in earlier.iter() {
                    // Shared cut edges acquire at most a few1e-8 square meters
                    // of roundoff after conversion to the actual float32 mesh.
                    if overlap_area(*old, projected) > 1.0e-7 {
                        overlaps += 1;
                    }
                }
                earlier.push(projected);
            }
        }
        assert_eq!(
            overlaps, 0,
            "coplanar rail triangles must not flicker: {material}/{facing}"
        );
    }
}

#[test]
fn every_yaw_and_switch_pose_partitions_without_holes_or_duplicate_surfaces() {
    for material in rails::MATERIALS {
        for facing in 0..if matches!(material, rails::SWITCH_LEFT | rails::SWITCH_RIGHT) {
            8
        } else {
            4
        } {
            let piece = Piece::from_facing([0; 3], material, facing).unwrap();
            let rendered = profile(piece, &template()).unwrap();
            let whole = rendered.full.values().map(area).sum::<f64>();
            let sliced = rendered
                .cells
                .values()
                .flat_map(BTreeMap::values)
                .map(area)
                .sum::<f64>();
            assert!(whole > 1.0);
            assert!(
                (whole - sliced).abs() < whole * 0.00002 + 0.00001,
                "duplicated/lost rail surface {material}/{facing}: whole={whole}, sliced={sliced}"
            );
            for (&cell, batches) in &rendered.cells {
                assert!(piece.footprint().contains(&cell));
                for batch in batches.values() {
                    assert_eq!(batch.positions.len(), batch.normals.len());
                    assert_eq!(batch.positions.len(), batch.tangents.len());
                    for point in &batch.positions {
                        assert!((0..3).all(|axis| point[axis] >= cell[axis] as f32 - 0.00001
                            && point[axis] <= cell[axis] as f32 + 1.00001));
                        assert!(
                            piece.cell_boxes(cell).iter().any(|shape| {
                                (0..3).all(|axis| {
                                    f64::from(point[axis]) - f64::from(cell[axis])
                                        >= shape.minimum[axis] - 0.00001
                                        && f64::from(point[axis]) - f64::from(cell[axis])
                                            <= shape.maximum[axis] + 0.00001
                                })
                            }),
                            "visible rail outside the shared picking/collision profile"
                        );
                    }
                    for triangle in batch.indices.chunks_exact(3) {
                        let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                            .map(|index| Vec3::from_array(batch.positions[index as usize]));
                        let normal = Vec3::from_array(batch.normals[triangle[0] as usize]);
                        assert!(
                            (b - a).cross(c - a).normalize().dot(normal) > 0.9999,
                            "ramp shear needs geometric face normals, not basis normals"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn real_switch_steel_tongues_and_red_lever_move_but_sleepers_stay_fixed() {
    for material in [rails::SWITCH_LEFT, rails::SWITCH_RIGHT] {
        let main = profile(Piece::new([0; 3], material, 0).unwrap(), &template()).unwrap();
        let branch = profile(Piece::new([0; 3], material, 4).unwrap(), &template()).unwrap();
        assert_eq!(main.full[&0].positions, branch.full[&0].positions);
        assert_ne!(main.full[&1].positions, branch.full[&1].positions);
        assert_ne!(main.full[&2].positions, branch.full[&2].positions);
        assert_ne!(main.full[&4].positions, branch.full[&4].positions);
    }
}

#[test]
fn cell_slices_render_once_across_negative_horizontal_and_vertical_section_edges() {
    use terra_voxel::{BlockSection, SectionPos, global_to_section, meshing::BlockMesh};
    use terra_world::edits::{BlockObjectState, EditJournal};
    let anchor = [-17, 15, -17];
    for material in [
        rails::STRAIGHT,
        rails::CURVE_LARGE,
        rails::SLOPE,
        rails::SWITCH_LEFT,
        rails::CROSSING,
    ] {
        let piece = Piece::new(anchor, material, 1).unwrap();
        let mut templates = TemplateSet::default();
        templates.blocks.insert(material, template());
        let mut sections = BTreeMap::<SectionPos, BlockSection>::new();
        let mut journal = EditJournal::default();
        let mut occupied = BTreeMap::new();
        for &offset in piece.footprint() {
            let cell = std::array::from_fn(|axis| anchor[axis] + i64::from(offset[axis]));
            let (section, local) = global_to_section(cell);
            sections
                .entry(section)
                .or_insert_with(|| BlockSection::empty(section).unwrap())
                .set_block(local, material)
                .unwrap();
            occupied.insert(cell, material);
            journal.record(cell, material).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(BlockObjectState {
                        peer: Some(anchor),
                        secondary: cell != anchor,
                        facing: 1,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let mut composed_area = 0.0;
        for (&position, section) in &sections {
            let snapshot = super::super::capture(section, &templates, &journal, |cell| {
                Some(occupied.get(&cell).copied().unwrap_or(0))
            })
            .unwrap();
            let batches = super::super::compose(
                &BlockMesh::default(),
                &snapshot,
                &templates,
                position,
                &crate::meshing::builder::RenderOptions {
                    water_level: None,
                    detail_level: crate::meshing::DetailLevel::Near,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: Default::default(),
                },
            )
            .unwrap();
            for batch in &batches {
                assert!(
                    batch
                        .positions
                        .iter()
                        .all(|point| point.iter().all(|v| (-0.00001..=16.00001).contains(v)))
                );
                composed_area += area(batch);
            }
        }
        let whole = profile(Piece::new([0; 3], material, 1).unwrap(), &template())
            .unwrap()
            .full
            .values()
            .map(area)
            .sum::<f64>();
        assert!(
            (whole - composed_area).abs() < whole * 0.00003 + 0.00001,
            "secondary cells cannot duplicate full assemblies or depend on a loaded anchor section"
        );
    }
}

#[test]
fn native_gltf_and_exported_profile_are_exactly_the_core_profile_not_a_second_curve_formula() {
    let assets = terra_core::asset_directory();
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(assets.join("terra/blocks/models/rail_profiles.json")).unwrap(),
    )
    .unwrap();
    let provenance = manifest["core_source_sha256"].as_str().unwrap();
    assert_eq!(provenance.len(), 64);
    assert!(provenance.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let mut source_hash = 0xcbf29ce484222325_u64;
    for (path, source) in [
        (
            "crates/terra-core/src/rails.rs",
            include_bytes!("../../../terra-core/src/rails.rs").as_slice(),
        ),
        (
            "crates/terra-core/src/rails/geometry.rs",
            include_bytes!("../../../terra-core/src/rails/geometry.rs").as_slice(),
        ),
        (
            "crates/terra-core/src/rails/surfaces.rs",
            include_bytes!("../../../terra-core/src/rails/surfaces.rs").as_slice(),
        ),
    ] {
        for byte in path.bytes().chain([0]).chain(source.iter().copied()) {
            source_hash = (source_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(
        manifest["core_source_fnv64"].as_str(),
        Some(format!("{source_hash:016x}").as_str()),
        "rail profiles must be regenerated after the production geometry changes"
    );
    for exported in manifest["profiles"].as_array().unwrap() {
        let material = exported["material"].as_u64().unwrap() as u16;
        let facing = exported["facing"].as_u64().unwrap() as u8;
        let piece = Piece::new([0; 3], material, facing).unwrap();
        let beams = exported["beams"].as_array().unwrap();
        assert_eq!(piece.local_geometry().len(), beams.len());
        for (beam, exported) in piece.local_geometry().iter().zip(beams) {
            let corners: [[f64; 3]; 8] =
                serde_json::from_value(exported["corners"].clone()).unwrap();
            for (actual, expected) in corners
                .into_iter()
                .flatten()
                .zip(beam.corners().into_iter().flatten())
            {
                assert!(
                    (actual - expected).abs() < 1.0e-12,
                    "stale rail asset export"
                );
            }
        }
        let surfaces = exported["surfaces"].as_array().unwrap();
        assert_eq!(surfaces.len(), piece.local_surfaces().len());
        for (surface, exported) in piece.local_surfaces().iter().zip(surfaces) {
            assert_eq!(surface.beam, exported["beam"].as_u64().unwrap() as usize);
            let vertices: Vec<[f64; 3]> =
                serde_json::from_value(exported["points"].clone()).unwrap();
            assert_eq!(vertices.len(), surface.points.len());
            for (actual, expected) in vertices
                .into_iter()
                .flatten()
                .zip(surface.points.iter().flatten().copied())
            {
                assert!(
                    (actual - expected).abs() < 1.0e-12,
                    "stale disjoint rail surface export"
                );
            }
        }
        if facing != 0 {
            continue;
        }
        let kind = piece.kind().unwrap();
        let path: &'static str =
            Box::leak(format!("terra/blocks/models/{}.gltf", kind.key()).into_boxed_str());
        let model = crate::blocks::test_static_scene(path, material);
        let source: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(assets.join(path)).unwrap()).unwrap();
        assert_eq!(
            source["extras"]["core_source_sha256"].as_str(),
            Some(provenance)
        );
        for (index, name) in ["rail_0_wood", "rail_1_steel", "rail_2_head", "rail_3_bolt"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(source["materials"][index]["name"].as_str(), Some(name));
        }
        let native_area = model
            .iter()
            .map(|part| {
                part.indices
                    .chunks_exact(3)
                    .map(|triangle| {
                        let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                            .map(|index| Vec3::from_array(part.positions[index as usize]));
                        f64::from((b - a).cross(c - a).length()) * 0.5
                    })
                    .sum::<f64>()
            })
            .sum::<f64>();
        let runtime_area = profile(piece, &template())
            .unwrap()
            .full
            .values()
            .map(area)
            .sum::<f64>();
        assert!((native_area - runtime_area).abs() < runtime_area * 0.00001);
        let definition: crate::blocks::definitions::BlockDefinition = serde_json::from_str(
            &std::fs::read_to_string(assets.join(format!("terra/blocks/{}.json", kind.key())))
                .unwrap(),
        )
        .unwrap();
        definition.validate().unwrap();
        assert!(definition.gameplay.struct_block);
        assert_eq!(definition.creative_category.as_deref(), Some("building"));
        let mut preview_templates = TemplateSet::default();
        preview_templates.blocks.insert(
            material,
            BlockTemplate {
                model,
                ..template()
            },
        );
        let preview = crate::previews::block_batches(
            material,
            &preview_templates,
            &crate::meshing::builder::RenderOptions {
                water_level: None,
                detail_level: crate::meshing::DetailLevel::Near,
                terrain: None,
                exposure: None,
                tints: Default::default(),
                conditions: Default::default(),
            },
        )
        .unwrap();
        let (low, high) = preview
            .iter()
            .flat_map(|batch| &batch.positions)
            .map(|point| Vec3::from_array(*point))
            .fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(low, high), point| (low.min(point), high.max(point)),
            );
        assert!(low.min_element() >= -0.00001 && high.max_element() <= 1.00001);
        assert!(
            ((high - low).max_element() - 1.0).abs() < 0.00001,
            "icons/drops must fit the entire assembly rather than crop a one-cell fragment"
        );
    }
}
