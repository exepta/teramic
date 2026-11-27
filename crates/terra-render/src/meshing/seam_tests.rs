use super::{
    DetailLevel,
    builder::{MeshBatch, RenderOptions, compose_section_with_options},
};
use crate::blocks::{BlockTemplate, SideCap, TemplateSet, cube_faces, definitions::BlockLibrary};
use bevy::prelude::Vec3;
use terra_voxel::{
    SectionPos,
    meshing::{PADDED_VOLUME, mesh_section},
};

fn templates() -> TemplateSet {
    let directory = terra_core::asset_directory().join("terra/blocks");
    let library = BlockLibrary::load(&directory).expect("JSON definitions");
    let mut templates = crate::blocks::authored_terrain_templates();
    // This fixture fills complete solid cells. Shaped Dirt models imported by
    // the PBR fixture have their own construction/collision coverage tests.
    let authored_cubes = &templates.cube_lods;
    templates
        .blocks
        .retain(|id, _| authored_cubes.contains_key(id));
    let mut materials = std::collections::BTreeMap::new();
    for block in library.blocks {
        if block.render_kind != crate::blocks::definitions::BlockRenderKind::Cube {
            continue;
        }
        let face_materials = block.face_materials.expect("cube materials").map(|name| {
            let next = materials.len() + 7;
            *materials.entry(name).or_insert(next)
        });
        let side_cap = block.side_cap.map(|cap| {
            let next = materials.len() + 7;
            SideCap {
                material: *materials.entry(cap.material).or_insert(next),
                height: cap.height,
            }
        });
        let faces = cube_faces(block.bevel_width, face_materials, side_cap);
        templates.blocks.insert(
            block.id,
            BlockTemplate {
                surface: block
                    .surface
                    .map(crate::blocks::surface::SurfaceTemplate::new),
                model: Vec::new(),
                render_kind: block.render_kind,
                face_materials,
                side_cap,
                faces,
                bevel_width: block.bevel_width,
                details: Vec::new(),
            },
        );
    }
    templates
}

#[test]
fn cube_cores_close_every_exposed_face_and_relief_only_projects_into_open_cells() {
    let templates = templates();
    let mut plain_templates = self::templates();
    for block in plain_templates.blocks.values_mut() {
        block.surface = None;
    }
    for material in [1, 2, 3, 4, 5, 6, 10, 19, 42, 43] {
        for detail_level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            for mask in 1_u8..64 {
                let faces: Vec<_> = (0..6)
                    .filter(|side| mask & (1 << side) != 0)
                    .map(|side| {
                        let (u_axis, v_axis) = match side / 2 {
                            0 => (2, 1),
                            1 => (0, 2),
                            _ => (0, 1),
                        };
                        terra_voxel::meshing::BlockFace {
                            edge_mask: 15,
                            occupied_neighbor: false,
                            minimum: [0; 3],
                            width: 1,
                            height: 1,
                            u_axis,
                            v_axis,
                            side,
                            material,
                        }
                    })
                    .collect();
                let batches = compose_section_with_options(
                    &faces,
                    &templates,
                    SectionPos::new(-1, -1, -1),
                    &RenderOptions {
                        detail_level,
                        water_level: None,
                        terrain: None,
                        exposure: None,
                        tints: Default::default(),
                        conditions: Default::default(),
                    },
                )
                .expect("closed cube geometry");
                if detail_level == DetailLevel::Near {
                    let plain = compose_section_with_options(
                        &faces,
                        &plain_templates,
                        SectionPos::new(-1, -1, -1),
                        &RenderOptions {
                            detail_level,
                            water_level: None,
                            terrain: None,
                            exposure: None,
                            tints: Default::default(),
                            conditions: Default::default(),
                        },
                    )
                    .expect("reference shell");
                    for face in &faces {
                        let axis = face.side / 2;
                        let mut normal = Vec3::ZERO;
                        normal[axis] = if face.side.is_multiple_of(2) {
                            -1.0
                        } else {
                            1.0
                        };
                        for u in [
                            0.001, 0.005, 0.02, 0.05, 0.09, 0.5, 0.91, 0.95, 0.98, 0.995, 0.999,
                        ] {
                            for v in [
                                0.001, 0.005, 0.02, 0.05, 0.09, 0.5, 0.91, 0.95, 0.98, 0.995, 0.999,
                            ] {
                                let mut point = Vec3::ZERO;
                                point[axis] = (face.side % 2) as f32;
                                point[face.u_axis] = u;
                                point[face.v_axis] = v;
                                if let Some(reference) = first_hit(&plain, point + normal, -normal)
                                {
                                    let hit = first_hit(&batches, point + normal, -normal);
                                    assert!(
                                        hit.is_some_and(|d| d
                                            <= reference
                                                + match material {
                                                    1 => 0.0651,
                                                    2 | 19 => 0.0551,
                                                    3 | 42 | 43 => 0.0751,
                                                    _ => 0.0451,
                                                }),
                                        "missing border triangle: material={material}, mask={mask}, side={}, uv={u},{v}, reference={reference}, hit={hit:?}",
                                        face.side
                                    );
                                }
                            }
                        }
                    }
                }
                let additive = detail_level == DetailLevel::Near
                    && templates.block_details.micro_geometry
                    && templates.sculpted_blocks.contains_key(&material);
                for batch in &batches {
                    for point in &batch.positions {
                        let mut outside_axes = 0;
                        for (axis, &coordinate) in point.iter().enumerate() {
                            if !(0.0..=1.0).contains(&coordinate) {
                                outside_axes += 1;
                                let side = axis * 2 + usize::from(coordinate > 1.0);
                                assert!(
                                    additive && mask & (1 << side) != 0,
                                    "detail reaches an occupied neighbour: material={material}, point={point:?}, mask={mask}"
                                );
                            }
                        }
                        assert!(outside_axes <= 1, "detail crosses a diagonal neighbour");
                    }
                    assert!(
                        batch
                            .indices
                            .iter()
                            .all(|&i| (i as usize) < batch.positions.len())
                    );
                }
                // Test actual front faces: two-sided ray tests miss winding holes.
                for face in &faces {
                    let axis = face.side / 2;
                    let mut normal = Vec3::ZERO;
                    normal[axis] = if face.side.is_multiple_of(2) {
                        -1.0
                    } else {
                        1.0
                    };
                    for u in [0.12, 0.31, 0.5, 0.69, 0.88] {
                        for v in [0.12, 0.31, 0.5, 0.69, 0.88] {
                            let mut surface = Vec3::ZERO;
                            surface[axis] = (face.side % 2) as f32;
                            surface[face.u_axis] = u;
                            surface[face.v_axis] = v;
                            let hit = first_hit(&batches, surface + normal, -normal);
                            let maximum = if detail_level == DetailLevel::Near {
                                match material {
                                    1 => 1.0651,
                                    2 | 19 => 1.0551,
                                    3 | 42 | 43 => 1.0751,
                                    _ => 1.0451,
                                }
                            } else {
                                1.0001
                            };
                            assert!(
                                hit.is_some_and(|d| if additive {
                                    (0.75..=1.0001).contains(&d)
                                } else {
                                    (0.9999..=maximum).contains(&d)
                                }),
                                "open/deformed face: material={material}, LOD={detail_level:?}, mask={mask}, side={}, uv={u},{v}, hit={hit:?}",
                                face.side
                            );
                        }
                    }
                }
            }
        }
    }
}

fn terrain(templates: &TemplateSet, material: u16, stepped: bool) -> Vec<MeshBatch> {
    terrain_lods(templates, material, stepped, false)
}

#[test]
fn joined_cube_uvs_use_the_same_projection_as_greedy_lods() {
    let templates = templates();
    for material in [1, 3, 5] {
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            let top = terra_voxel::meshing::BlockFace {
                edge_mask: 15,
                occupied_neighbor: false,
                minimum: [0; 3],
                width: 16,
                height: 16,
                u_axis: 0,
                v_axis: 2,
                side: 3,
                material,
            };
            let batches = compose_section_with_options(
                &[top],
                &templates,
                SectionPos::new(-1, 0, -1),
                &RenderOptions {
                    detail_level: level,
                    water_level: None,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: Default::default(),
                },
            )
            .expect("projected flat terrain");
            for batch in batches {
                for ((position, uv), normal) in
                    batch.positions.iter().zip(&batch.uvs).zip(&batch.normals)
                {
                    if templates.sculpted_blocks.contains_key(&material) && normal[1].abs() < 0.5 {
                        // The steep sides of raised tiles have their own unwrapped UVs.
                        continue;
                    }
                    assert_eq!(*uv, [position[0], position[2]], "{material}: {level:?}");
                }
            }
        }
    }
}

fn terrain_lods(
    templates: &TemplateSet,
    material: u16,
    stepped: bool,
    mixed: bool,
) -> Vec<MeshBatch> {
    let mut output = Vec::new();
    // Four sections meet at global X/Z = 0, covering seams in negative regions too.
    for x in [-1, 0] {
        for z in [-1, 0] {
            let section = SectionPos { x, y: 0, z };
            let origin = section.origin().expect("origin");
            let mut padded = vec![0; PADDED_VOLUME];
            for pz in 0..18 {
                for py in 0..18 {
                    for px in 0..18 {
                        let global = [
                            origin[0] + px as i64 - 1,
                            py as i64 - 1,
                            origin[2] + pz as i64 - 1,
                        ];
                        let height = i64::from(stepped && global[0] < 0);
                        if global[1] <= height {
                            padded[px + 18 * (py + 18 * pz)] = material;
                        }
                    }
                }
            }
            let geometry = mesh_section(&padded).expect("culled section");
            let detail_level = if mixed && x == 0 {
                DetailLevel::Far
            } else {
                DetailLevel::Near
            };
            let mut batches = compose_section_with_options(
                &geometry.faces,
                templates,
                section,
                &RenderOptions {
                    detail_level,
                    water_level: None,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: Default::default(),
                },
            )
            .expect("section mesh");
            for batch in &mut batches {
                for point in &mut batch.positions {
                    for axis in 0..3 {
                        point[axis] += origin[axis] as f32;
                    }
                }
            }
            output.extend(batches);
        }
    }
    output
}

#[test]
fn mixed_lod_boundaries_close_flat_and_stepped_negative_section_edges() {
    let templates = templates();
    for material in [1, 2, 3, 4, 6] {
        for stepped in [false, true] {
            let batches = terrain_lods(&templates, material, stepped, true);
            for x in [-0.001, 0.0, 0.001] {
                for z in [-1.0, -0.001, 0.0, 0.001, 1.0] {
                    let distance = first_hit(&batches, Vec3::new(x, 4.0, z), -Vec3::Y)
                        .expect("LOD boundary must be closed");
                    assert!(distance <= 3.0001);
                }
            }
            if material == 1 && !stepped {
                // Look horizontally through the space between the coarse
                // walk plane and the raised cap on the neighboring section.
                for z in [-0.93, -0.53, -0.21] {
                    let hit = first_hit(&batches, Vec3::new(0.02, 1.001, z), -Vec3::X);
                    assert!(
                        hit.is_some_and(|d| (d - 0.02).abs() < 0.0001),
                        "open edge between raised grass and coarse ground at {z}: {hit:?}"
                    );
                    let interior = first_hit(&batches, Vec3::new(-0.98, 1.001, z), -Vec3::X);
                    assert!(
                        interior.is_none_or(|d| d > 0.1),
                        "hidden internal skirt at {z}"
                    );
                }
            }
        }
    }
}

fn first_hit(batches: &[MeshBatch], origin: Vec3, direction: Vec3) -> Option<f32> {
    let mut closest = f32::INFINITY;
    for batch in batches {
        for triangle in batch.indices.chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                .map(|i| Vec3::from_array(batch.positions[i as usize]));
            let ab = b - a;
            let ac = c - a;
            let p = direction.cross(ac);
            let determinant = ab.dot(p);
            if determinant <= 1e-7 {
                continue;
            }
            let t = origin - a;
            let u = t.dot(p) / determinant;
            let q = t.cross(ab);
            let v = direction.dot(q) / determinant;
            let distance = ac.dot(q) / determinant;
            if u >= -1e-5 && v >= -1e-5 && u + v <= 1.00001 && distance >= 0.0 {
                closest = closest.min(distance);
            }
        }
    }
    closest.is_finite().then_some(closest)
}

#[test]
fn solid_surfaces_close_flat_seams_and_four_section_corners() {
    let templates = templates();
    for &material in templates.blocks.keys() {
        // Foliage has authored gaps and overhanging sheets, not the exact
        // watertight collision-cell surface required of opaque terrain.
        // Its geometry, alpha and LOD contracts have separate regressions.
        if terra_voxel::MaterialId::FOLIAGE.contains(&material) {
            continue;
        }
        if terra_voxel::MaterialId::from_raw(material).is_some_and(|id| !id.is_solid()) {
            continue;
        }
        let mut batches = terrain(&templates, material, false);
        if templates
            .sculpted_blocks
            .get(&material)
            .is_some_and(|shape| shape.continuous_ground(3))
        {
            // The joined relief now spans those corners several millimetres
            // above ground. Ray-test its underlying unit-square support itself.
            for batch in &mut batches {
                batch.indices = batch
                    .indices
                    .chunks_exact(3)
                    .filter(|tri| tri.iter().all(|&i| batch.positions[i as usize][1] == 1.0))
                    .flatten()
                    .copied()
                    .collect();
            }
        }
        for x in [-1.001, -1.0, -0.999, -0.001, 0.0, 0.001, 0.999, 1.0, 1.001] {
            for z in [-1.001, -1.0, -0.001, 0.0, 0.001, 1.0, 1.001] {
                let hit = first_hit(&batches, Vec3::new(x, 3.0, z), -Vec3::Y)
                    .expect("No hole through solid terrain");
                assert!(
                    (hit - 2.0).abs() < 0.0001,
                    "material={material}, x={x}, z={z}, hit={hit}"
                );
            }
        }
    }
}

#[test]
fn all_materials_close_step_edges_at_section_boundaries() {
    let templates = templates();
    for &material in templates.blocks.keys() {
        if terra_voxel::MaterialId::from_raw(material).is_some_and(|id| !id.is_solid()) {
            continue;
        }
        let batches = terrain(&templates, material, true);
        for z in [-1.001, -1.0, -0.001, 0.0, 0.001, 1.0, 1.001] {
            for y in [1.001, 1.05, 1.25, 1.5, 1.75] {
                let hit = first_hit(&batches, Vec3::new(0.5, y, z), -Vec3::X)
                    .expect("No hole through a step side");
                assert!(hit <= 0.76, "material={material}, y={y}, z={z}, hit={hit}");
            }
            let hit = first_hit(&batches, Vec3::new(-0.001, 3.0, z), -Vec3::Y)
                .expect("No hole through top of a step");
            assert!(hit <= 1.26, "material={material}, z={z}, hit={hit}");
        }
    }
}

#[test]
fn fluid_source_has_a_real_animated_inventory_cube_but_no_solid_terrain_faces() {
    let templates = templates();
    let terrain = terrain(&templates, 27, false);
    assert!(terrain.is_empty());
    let batches = crate::previews::block_batches(
        27,
        &templates,
        &super::builder::RenderOptions {
            water_level: None,
            detail_level: super::DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        },
    )
    .unwrap();
    assert!(!batches.is_empty());
    for batch in batches {
        assert!(templates.water_material(batch.material));
        assert!(batch.colors.iter().all(|color| color[3] == 4.0));
        assert!(!batch.indices.is_empty());
    }
}

fn sparse_corner(
    templates: &TemplateSet,
    materials: [u16; 3],
    levels: [DetailLevel; 3],
) -> Vec<MeshBatch> {
    sparse_cells(
        templates,
        &[
            ([-1, 0, -1], materials[0], levels[0]),
            ([0, 0, -1], materials[1], levels[1]),
            ([-1, 0, 0], materials[2], levels[2]),
        ],
    )
}

fn sparse_cells(templates: &TemplateSet, cells: &[([i64; 3], u16, DetailLevel)]) -> Vec<MeshBatch> {
    let mut result = Vec::new();
    for &(cell, _, detail_level) in cells {
        let section = terra_voxel::global_to_section(cell).0;
        let origin = section.origin().unwrap();
        let mut padded = vec![0; PADDED_VOLUME];
        for pz in 0..18 {
            for py in 0..18 {
                for px in 0..18 {
                    let global = [
                        origin[0] + px as i64 - 1,
                        origin[1] + py as i64 - 1,
                        origin[2] + pz as i64 - 1,
                    ];
                    if let Some((_, material, _)) =
                        cells.iter().find(|(position, _, _)| *position == global)
                    {
                        padded[px + 18 * (py + 18 * pz)] = *material;
                    }
                }
            }
        }
        let geometry = mesh_section(&padded).unwrap();
        let mut batches = compose_section_with_options(
            &geometry.faces,
            templates,
            section,
            &RenderOptions {
                detail_level,
                water_level: None,
                terrain: None,
                exposure: None,
                tints: Default::default(),
                conditions: Default::default(),
            },
        )
        .unwrap();
        for batch in &mut batches {
            for point in &mut batch.positions {
                for axis in 0..3 {
                    point[axis] += origin[axis] as f32;
                }
            }
        }
        result.extend(batches);
    }
    result
}

#[test]
fn concave_three_cell_corners_do_not_expose_the_culled_interior() {
    let templates = templates();
    for material in 1..=10 {
        let batches = sparse_corner(&templates, [material; 3], [DetailLevel::Near; 3]);
        for offset in [-0.02, -0.005, 0.0, 0.005, 0.02] {
            let origin = Vec3::new(0.5 + offset, 0.995, 0.5);
            let hit = first_hit(&batches, origin, Vec3::new(-1.0, 0.0, -1.0).normalize());
            assert!(
                hit.is_some_and(|distance| distance < 0.8),
                "open concave corner: material={material}, offset={offset}, hit={hit:?}"
            );
        }
    }
}

#[test]
fn concave_corners_stay_closed_with_mixed_materials_and_lods() {
    let templates = templates();
    for materials in [[1, 2, 3], [7, 6, 8], [3, 9, 10]] {
        for a in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            for b in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
                for c in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
                    let batches = sparse_corner(&templates, materials, [a, b, c]);
                    for offset in [-0.005, 0.0, 0.005] {
                        let origin = Vec3::new(0.5 + offset, 0.995, 0.5);
                        let hit =
                            first_hit(&batches, origin, Vec3::new(-1.0, 0.0, -1.0).normalize());
                        assert!(
                            hit.is_some_and(|distance| distance < 0.8),
                            "open mixed corner: materials={materials:?}, levels={:?}, offset={offset}, hit={hit:?}",
                            [a, b, c]
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn rotated_three_and_four_section_corners_close_with_micro_foliage_and_far_neighbors() {
    let templates = templates();
    let levels = [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far];
    for materials in [[1, 2, 3, 4], [7, 8, 9, 6]] {
        for permutation in [[0, 1, 2], [1, 0, 2], [0, 2, 1]] {
            let rotate = |point: Vec3| Vec3::from_array(permutation.map(|axis| point[axis]));
            for shift in 0..3 {
                for full in [false, true] {
                    let cells: Vec<_> = [[-1, 0, -1], [0, 0, -1], [-1, 0, 0], [0, 0, 0]]
                        .into_iter()
                        .enumerate()
                        .take(if full { 4 } else { 3 })
                        .map(|(i, point)| {
                            (
                                permutation.map(|axis| point[axis]),
                                materials[i],
                                levels[(i + shift) % levels.len()],
                            )
                        })
                        .collect();
                    let batches = sparse_cells(&templates, &cells);
                    if full {
                        for x in [-0.005, 0.0, 0.005] {
                            for z in [-0.005, 0.0, 0.005] {
                                let hit = first_hit(
                                    &batches,
                                    rotate(Vec3::new(x, 2.0, z)),
                                    rotate(-Vec3::Y),
                                );
                                assert!(
                                    hit.is_some_and(|distance| distance <= 1.0001),
                                    "four-section seam: materials={materials:?}, axes={permutation:?}, shift={shift}, x={x}, z={z}, hit={hit:?}"
                                );
                            }
                        }
                    } else {
                        for height in [0.005, 0.995] {
                            for offset in [-0.005, 0.0, 0.005] {
                                let hit = first_hit(
                                    &batches,
                                    rotate(Vec3::new(0.5 + offset, height, 0.5)),
                                    rotate(Vec3::new(-1.0, 0.0, -1.0).normalize()),
                                );
                                assert!(
                                    hit.is_some_and(|distance| distance < 0.8),
                                    "three-section seam: materials={materials:?}, axes={permutation:?}, shift={shift}, y={height}, offset={offset}, hit={hit:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
