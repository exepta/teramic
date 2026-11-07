//! Continue an enclosing full-block wall to the actual round pipe boundary.
use super::*;
use bevy::math::Vec3;
use terra_core::plumbing as core;

#[derive(Clone, Debug)]
pub(super) struct Surround {
    mask: u8,
    hidden: u8,
    material: u16,
}

fn touches_water(journal: &terra_world::edits::EditJournal, position: [i64; 3]) -> bool {
    std::iter::once(position)
        .chain(core::DIRECTIONS.map(|d| std::array::from_fn(|axis| position[axis] + d[axis])))
        .any(|p| {
            journal
                .fluid(p)
                .is_some_and(|water| water.amount > 0 && (p[1] >= position[1] || water.amount == 8))
        })
}

fn infer(
    position: [i64; 3],
    material: u16,
    facing: u8,
    templates: &TemplateSet,
    journal: &terra_world::edits::EditJournal,
    block_at: &mut impl FnMut([i64; 3]) -> Option<u16>,
) -> Option<Surround> {
    if !core::is_pipe(material) {
        return None;
    }
    // Waterlogged pipes occupy their actual round body, never an inferred
    // replacement cube of nearby bank/wall material.
    if touches_water(journal, position) {
        return None;
    }
    let mask =
        terra_world::plumbing::connection_mask(journal, position, material, facing, &mut *block_at);
    let full: [Option<u16>; 6] = std::array::from_fn(|side| {
        let cell = std::array::from_fn(|axis| position[axis] + core::DIRECTIONS[side][axis]);
        block_at(cell).filter(|id| {
            terra_voxel::MaterialId::from_raw(*id).is_some_and(terra_voxel::MaterialId::is_opaque)
                && templates.blocks.get(id).is_some_and(|block| {
                    block.model.is_empty()
                        && !matches!(
                            block.render_kind,
                            crate::blocks::definitions::BlockRenderKind::Boxes
                                | crate::blocks::definitions::BlockRenderKind::Segment
                        )
                        && block.faces.iter().all(|faces| !faces.is_empty())
                })
        })
    });
    // Match chimney walls: at least three surrounding opaque full faces.
    // End supports along the actual pipe ports never create a floating surround.
    let supports: Vec<_> = full
        .iter()
        .enumerate()
        .filter_map(|(side, id)| (mask & (1 << side) == 0).then_some(*id).flatten())
        .collect();
    if supports.len() < 3 {
        return None;
    }
    let mut counts = BTreeMap::new();
    for id in supports {
        *counts.entry(id).or_insert(0usize) += 1;
    }
    let material = counts
        .into_iter()
        .max_by_key(|&(id, count)| (count, std::cmp::Reverse(id)))?
        .0;
    let hidden = full.iter().enumerate().fold(0, |mask, (side, id)| {
        mask | (u8::from(id.is_some()) << side)
    });
    Some(Surround {
        mask,
        hidden,
        material,
    })
}

// A cross/T junction can have fewer than three bare full-block neighbors.
// Already enclosed adjacent pipe cells carry that same wall material through
// their ports. Only direct, independently enclosed neighbors can seed this
// second pass; a floating pipe network never manufactures its own wall.
fn infer_connected(
    position: [i64; 3],
    material: u16,
    facing: u8,
    templates: &TemplateSet,
    journal: &terra_world::edits::EditJournal,
    block_at: &mut impl FnMut([i64; 3]) -> Option<u16>,
) -> Option<Surround> {
    if touches_water(journal, position) {
        return None;
    }
    if let Some(surround) = infer(
        position,
        material,
        facing,
        templates,
        journal,
        &mut *block_at,
    ) {
        return Some(surround);
    }
    if !core::is_pipe(material) {
        return None;
    }
    let mask =
        terra_world::plumbing::connection_mask(journal, position, material, facing, &mut *block_at);
    let mut hosts = Vec::new();
    let mut hidden = 0;
    let mut fitted = 0;
    for side in 0..6 {
        let neighbor = std::array::from_fn(|axis| position[axis] + core::DIRECTIONS[side][axis]);
        let Some(id) = block_at(neighbor) else {
            continue;
        };
        let ordinary = terra_voxel::MaterialId::from_raw(id)
            .is_some_and(terra_voxel::MaterialId::is_opaque)
            && templates.blocks.get(&id).is_some_and(|block| {
                block.model.is_empty()
                    && !matches!(
                        block.render_kind,
                        crate::blocks::definitions::BlockRenderKind::Boxes
                            | crate::blocks::definitions::BlockRenderKind::Segment
                    )
                    && block.faces.iter().all(|face| !face.is_empty())
            });
        if ordinary {
            hidden |= 1 << side;
            if mask & (1 << side) == 0 {
                hosts.push(id);
            }
        } else if let Some(enclosed) = infer(
            neighbor,
            id,
            journal.object_state(neighbor).unwrap_or_default().facing,
            templates,
            journal,
            &mut *block_at,
        ) {
            hosts.push(enclosed.material);
            hidden |= 1 << side;
            fitted += 1;
        }
    }
    if fitted == 0 || hosts.len() < 3 {
        return None;
    }
    let mut counts = BTreeMap::new();
    for id in hosts {
        *counts.entry(id).or_insert(0usize) += 1;
    }
    let material = counts
        .into_iter()
        .max_by_key(|&(id, count)| (count, std::cmp::Reverse(id)))?
        .0;
    Some(Surround {
        mask,
        hidden,
        material,
    })
}

pub(super) fn capture(
    model: &ModelCell,
    position: [i64; 3],
    templates: &TemplateSet,
    journal: &terra_world::edits::EditJournal,
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
) -> Option<Surround> {
    if touches_water(journal, position) {
        return None;
    }
    let mut surround = infer_connected(
        position,
        model.material,
        model.state.facing,
        templates,
        journal,
        &mut block_at,
    )?;
    for side in 0..6 {
        let neighbor = std::array::from_fn(|axis| position[axis] + core::DIRECTIONS[side][axis]);
        let Some(material) = block_at(neighbor) else {
            continue;
        };
        let facing = journal.object_state(neighbor).unwrap_or_default().facing;
        if infer_connected(
            neighbor,
            material,
            facing,
            templates,
            journal,
            &mut block_at,
        )
        .is_some()
        {
            surround.hidden |= 1 << side;
        }
    }
    Some(surround)
}

fn quad(mesh: &mut crate::blocks::PrimitiveTemplate, points: [Vec3; 4], side: usize) {
    let axis = side / 2;
    let (u, v) = match axis {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    let normal = Vec3::AXES[axis] * if side.is_multiple_of(2) { -1. } else { 1. };
    let base = mesh.positions.len() as u32;
    mesh.positions.extend(points.map(|p| p.to_array()));
    mesh.normals.extend([normal.to_array(); 4]);
    mesh.uvs.extend(points.map(|p| [p[u], p[v]]));
    mesh.tangents
        .extend([crate::blocks::projected_tangent(normal, side); 4]);
    for [a, mut b, mut c] in [[0, 1, 2], [0, 2, 3]] {
        if (points[b] - points[a])
            .cross(points[c] - points[a])
            .dot(normal)
            < 0.
        {
            std::mem::swap(&mut b, &mut c);
        }
        mesh.indices
            .extend([base + a as u32, base + b as u32, base + c as u32]);
    }
}

fn profile(surround: &Surround, materials: [usize; 6]) -> Vec<crate::blocks::PrimitiveTemplate> {
    let mut faces = Vec::new();
    for (side, &material) in materials.iter().enumerate() {
        if surround.hidden & (1 << side) != 0 {
            continue;
        }
        let axis = side / 2;
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let mut mesh = crate::blocks::PrimitiveTemplate {
            material,
            positions: Vec::new(),
            normals: Vec::new(),
            tangents: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
        };
        let point = |uv: [f32; 2]| {
            let mut p = Vec3::ZERO;
            p[axis] = (side % 2) as f32;
            p[u] = uv[0];
            p[v] = uv[1];
            p
        };
        if surround.mask & (1 << side) != 0 {
            // Sixteen sectors meet the exact pipe vertices. Radial rays hit the
            // square perimeter, filling every corner without a square cutout.
            for sector in 0..16 {
                let radial = [sector, sector + 1].map(|i| {
                    let angle = i as f32 * std::f32::consts::TAU / 16.;
                    [angle.cos(), angle.sin()]
                });
                let inner = radial.map(|r| point(r.map(|x| 0.5 + x * 0.15)));
                let outer = radial.map(|r| {
                    let scale = 0.5 / r[0].abs().max(r[1].abs());
                    point(r.map(|x| 0.5 + x * scale))
                });
                quad(&mut mesh, [inner[0], outer[0], outer[1], inner[1]], side);
            }
        } else {
            quad(
                &mut mesh,
                [[0., 0.], [1., 0.], [1., 1.], [0., 1.]].map(point),
                side,
            );
        }
        faces.push(mesh);
    }
    // Opaque terrain behind transparent glass needs its real tunnel wall too.
    // Back-to-back winding gives each interface exactly one visible owner.
    let mut inside = plumbing::pipe::surround_boundary(surround.mask, materials[0]);
    for normal in &mut inside.normals {
        *normal = normal.map(|v| -v);
    }
    for tangent in &mut inside.tangents {
        tangent[3] = -tangent[3];
    }
    for face in inside.indices.chunks_exact_mut(3) {
        face.swap(1, 2);
    }
    faces.push(inside);
    faces
}

pub(super) fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    model: &ModelCell,
    templates: &TemplateSet,
    surround: &Surround,
) -> Result<(), String> {
    let Some(template) = templates.blocks.get(&surround.material) else {
        return Ok(());
    };
    for primitive in profile(surround, template.face_materials) {
        let batch = batches
            .entry(primitive.material)
            .or_insert_with(|| MeshBatch {
                material: primitive.material,
                ..Default::default()
            });
        let base =
            u32::try_from(batch.positions.len()).map_err(|_| "Pipe surround index overflow")?;
        let offset = Vec3::from_array(model.cell.map(|v| v as f32));
        batch.positions.extend(
            primitive
                .positions
                .iter()
                .map(|&p| (Vec3::from_array(p) + offset).to_array()),
        );
        batch.normals.extend(primitive.normals);
        batch.uvs.extend(primitive.uvs);
        batch.append_tangents(base as usize, primitive.tangents.into_iter());
        batch
            .indices
            .extend(primitive.indices.into_iter().map(|i| base + i));
        if !batch.colors.is_empty() {
            batch.colors.resize(batch.positions.len(), [1.; 4]);
        }
        if !batch.surface_water.is_empty() {
            batch.surface_water.resize(batch.positions.len(), [0.; 2]);
        }
        if !batch.rooted_wind.is_empty() {
            batch.rooted_wind.resize(batch.positions.len(), [0.; 2]);
        }
        if !batch.growth_anchors.is_empty() {
            batch.growth_anchors.resize(batch.positions.len(), [0.; 3]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
    use terra_world::edits::{BlockObjectState, EditJournal};

    #[test]
    fn waterlogged_pipes_never_receive_solid_wall_infill() {
        let templates = templates();
        for material in [
            core::WOOD_PIPE,
            core::STONE_PIPE,
            core::IRON_PIPE,
            core::GLASS_PIPE,
        ] {
            let mut journal = EditJournal::default();
            journal
                .set_fluid(
                    [0; 3],
                    terra_world::water::FluidCell {
                        amount: 8,
                        source: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert!(
                capture(&model(material, 4), [0; 3], &templates, &journal, |_| Some(
                    16
                ))
                .is_none()
            );
        }
    }

    #[test]
    fn a_dry_pipe_at_the_water_plane_must_not_cover_the_water_below_with_infill() {
        let templates = templates();
        for material in [
            core::WOOD_PIPE,
            core::STONE_PIPE,
            core::IRON_PIPE,
            core::GLASS_PIPE,
        ] {
            let mut journal = EditJournal::default();
            journal
                .set_fluid(
                    [0, -1, 0],
                    terra_world::water::FluidCell {
                        amount: 8,
                        source: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert!(
                capture(&model(material, 4), [0; 3], &templates, &journal, |_| Some(
                    16
                ))
                .is_none()
            );
            // Water strictly below the contact plane leaves the dry surround available.
            journal
                .set_fluid(
                    [0, -1, 0],
                    terra_world::water::FluidCell {
                        amount: 4,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert!(
                capture(&model(material, 4), [0; 3], &templates, &journal, |_| Some(
                    16
                ))
                .is_some()
            );
        }
    }
    fn templates() -> TemplateSet {
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            16,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::Cube,
                bevel_width: 0.,
                faces: crate::blocks::cube_faces(0., [10, 11, 12, 13, 14, 15], None),
                face_materials: [10, 11, 12, 13, 14, 15],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
        for id in [
            core::WOOD_PIPE,
            core::STONE_PIPE,
            core::IRON_PIPE,
            core::GLASS_PIPE,
        ] {
            templates.blocks.insert(
                id,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [99; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model: vec![plumbing::pipe::body(3, 99, 3)],
                },
            );
        }
        templates
    }
    fn model(material: u16, facing: u8) -> ModelCell {
        ModelCell {
            cell: [0; 3],
            material,
            state: BlockObjectState {
                facing,
                ..Default::default()
            },
            peer_wood: Default::default(),
            neighbors: [None; 6],
            neighbor_facing: [0; 6],
        }
    }
    fn area(mesh: &crate::blocks::PrimitiveTemplate) -> f32 {
        mesh.indices
            .chunks_exact(3)
            .map(|t| {
                let [a, b, c] =
                    [t[0], t[1], t[2]].map(|i| Vec3::from_array(mesh.positions[i as usize]));
                (b - a).cross(c - a).length() * 0.5
            })
            .sum()
    }
    fn covers(mesh: &crate::blocks::PrimitiveTemplate, side: usize, uv: [f32; 2]) -> bool {
        let u = (side / 2 + 1) % 3;
        let v = (side / 2 + 2) % 3;
        mesh.indices.chunks_exact(3).any(|t| {
            let p = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize]);
            let cross = |a: [f32; 3], b: [f32; 3]| {
                (b[u] - a[u]) * (uv[1] - a[v]) - (b[v] - a[v]) * (uv[0] - a[u])
            };
            let d = [cross(p[0], p[1]), cross(p[1], p[2]), cross(p[2], p[0])];
            d.iter().all(|&x| x >= -1e-7) || d.iter().all(|&x| x <= 1e-7)
        })
    }
    #[test]
    fn all_round_port_faces_fill_the_square_corners_without_covering_the_pipe() {
        let hole = 16. * 0.15_f32.powi(2) * (std::f32::consts::TAU / 16.).sin() * 0.5;
        for mask in 1u8..64 {
            if mask.count_ones() < 2 {
                continue;
            }
            let parts = profile(
                &Surround {
                    mask,
                    hidden: 0,
                    material: 16,
                },
                [0, 1, 2, 3, 4, 5],
            );
            for (side, face) in parts[..6].iter().enumerate() {
                let port = mask & (1 << side) != 0;
                assert!(
                    (area(face) - (1. - if port { hole } else { 0. })).abs() < 1e-6,
                    "mask{mask}, side{side}"
                );
                assert_eq!(covers(face, side, [0.5, 0.5]), !port);
                // This lies outside the real circle but inside the former square gap.
                assert!(covers(face, side, [0.61, 0.61]));
                for (normal, tangent) in face.normals.iter().zip(&face.tangents) {
                    let n = Vec3::from_array(*normal);
                    let t = Vec3::new(tangent[0], tangent[1], tangent[2]);
                    assert!((t.length() - 1.).abs() < 1e-6 && n.dot(t).abs() < 1e-6);
                }
                if port {
                    let axis = side / 2;
                    let mut angles = std::collections::BTreeSet::new();
                    for p in &face.positions {
                        let mut radial = Vec3::from_array(*p) - Vec3::splat(0.5);
                        radial[axis] = 0.;
                        if (radial.length() - 0.15).abs() < 1e-6 {
                            angles.insert(radial.to_array().map(|v| (v * 100000.).round() as i32));
                        }
                    }
                    assert_eq!(
                        angles.len(),
                        16,
                        "port contour must match all sixteen pipe vertices"
                    );
                }
            }
        }
    }
    #[test]
    fn authored_full_block_face_groups_can_supply_the_surround_material() {
        let mut templates = templates();
        let mut host = templates.blocks.remove(&16).unwrap();
        host.render_kind = BlockRenderKind::CustomMesh;
        templates.blocks.insert(1, host);
        let journal = EditJournal::default();
        let surround = capture(
            &model(core::IRON_PIPE, 4),
            [0; 3],
            &templates,
            &journal,
            |p| Some(if p[1] == 0 && p != [0; 3] { 1 } else { 0 }),
        )
        .unwrap();
        assert_eq!(surround.material, 1);
    }

    #[test]
    fn no_surround_is_invented_for_floor_only_or_two_walls() {
        let templates = templates();
        let journal = EditJournal::default();
        let p = [0; 3];
        for material in [
            core::WOOD_PIPE,
            core::STONE_PIPE,
            core::IRON_PIPE,
            core::GLASS_PIPE,
        ] {
            for facing in 0..6 {
                let axis = terra_core::kinetic::axis(facing)
                    .iter()
                    .position(|&v| v != 0)
                    .unwrap();
                let supports: Vec<_> = core::DIRECTIONS
                    .into_iter()
                    .filter(|d| d[axis] == 0)
                    .collect();
                for count in 0..=4 {
                    let result = capture(&model(material, facing), p, &templates, &journal, |q| {
                        Some(if supports[..count].contains(&q) {
                            16
                        } else {
                            0
                        })
                    });
                    assert_eq!(
                        result.is_some(),
                        count >= 3,
                        "material{material}, facing{facing}, count{count}"
                    );
                }
            }
        }
    }
    #[test]
    fn elbow_and_t_junctions_keep_exactly_their_real_connected_ports() {
        let templates = templates();
        for material in [
            core::WOOD_PIPE,
            core::STONE_PIPE,
            core::IRON_PIPE,
            core::GLASS_PIPE,
        ] {
            for mask in 1u8..64 {
                if !(2..=3).contains(&mask.count_ones()) {
                    continue;
                }
                let p = [0; 3];
                let mut cells = BTreeMap::from([(p, material)]);
                for (side, d) in core::DIRECTIONS.into_iter().enumerate() {
                    cells.insert(
                        d,
                        if mask & (1 << side) != 0 {
                            material
                        } else {
                            16
                        },
                    );
                }
                let mut journal = EditJournal::default();
                for (&cell, &id) in &cells {
                    journal.record(cell, id).unwrap();
                }
                let surround = capture(&model(material, 0), p, &templates, &journal, |q| {
                    Some(cells.get(&q).copied().unwrap_or(0))
                })
                .unwrap();
                assert_eq!(surround.mask, mask);
                assert_eq!(surround.hidden, 63 & !mask);
            }
        }
    }
    #[test]
    fn four_way_wall_junction_inherits_only_independently_enclosed_pipe_neighbors() {
        let templates = templates();
        let p = [0; 3];
        let mask = 0b110011u8;
        let mut cells = BTreeMap::from([(p, core::GLASS_PIPE), ([0, -1, 0], 16), ([0, 1, 0], 16)]);
        for (side, d) in core::DIRECTIONS
            .into_iter()
            .enumerate()
            .filter(|(side, _)| mask & (1 << side) != 0)
        {
            cells.insert(d, core::WOOD_PIPE);
            for normal in core::DIRECTIONS
                .into_iter()
                .filter(|normal| normal[side / 2] == 0)
            {
                let q = std::array::from_fn(|axis| d[axis] + normal[axis]);
                cells.entry(q).or_insert(16);
            }
        }
        let journal = EditJournal::default();
        let fitted = capture(&model(core::GLASS_PIPE, 0), p, &templates, &journal, |q| {
            Some(cells.get(&q).copied().unwrap_or(0))
        })
        .unwrap();
        assert_eq!(fitted.mask, mask);
        assert_eq!(fitted.hidden, 63);
        assert_eq!(fitted.material, 16);
        cells.retain(|&q, id| q == p || *id != 16);
        assert!(
            capture(&model(core::GLASS_PIPE, 0), p, &templates, &journal, |q| {
                Some(cells.get(&q).copied().unwrap_or(0))
            })
            .is_none()
        );
    }

    #[test]
    fn every_material_and_native_axis_captures_the_same_fitted_surround_across_section_borders() {
        let templates = templates();
        for material in [
            core::WOOD_PIPE,
            core::STONE_PIPE,
            core::IRON_PIPE,
            core::GLASS_PIPE,
        ] {
            for facing in 0..6 {
                let axis = terra_core::kinetic::axis(facing)
                    .iter()
                    .position(|&v| v != 0)
                    .unwrap();
                for border_axis in 0..3 {
                    for edge in [-17, -16, -1, 0, 15, 16] {
                        let mut p = [5, 5, 5];
                        p[border_axis] = edge;
                        let mut cells = BTreeMap::from([(p, material)]);
                        for d in core::DIRECTIONS.into_iter().filter(|d| d[axis] == 0) {
                            cells.insert(std::array::from_fn(|i| p[i] + d[i]), 16);
                        }
                        let mut journal = EditJournal::default();
                        journal.record(p, material).unwrap();
                        journal
                            .set_object_state(
                                p,
                                Some(BlockObjectState {
                                    facing,
                                    ..Default::default()
                                }),
                            )
                            .unwrap();
                        let (owner, local) = terra_voxel::global_to_section(p);
                        let mut section = BlockSection::empty(owner).unwrap();
                        section.set_block(local, material).unwrap();
                        let snapshot = super::super::capture(&section, &templates, &journal, |q| {
                            Some(cells.get(&q).copied().unwrap_or(0))
                        })
                        .unwrap();
                        let key = local.map(|v| v as i32);
                        let fitted = snapshot
                            .pipe_surround
                            .get(&key)
                            .expect("capture must sample the real neighboring section");
                        assert_eq!(fitted.mask, 3 << (axis * 2));
                        assert_eq!(fitted.hidden, 63 & !(3 << (axis * 2)));
                        assert_eq!(fitted.material, 16);
                        let mut batches = BTreeMap::new();
                        append(&mut batches, &snapshot.models[0], &templates, fitted).unwrap();
                        assert!(!batches.is_empty());
                        for batch in batches.values() {
                            assert_eq!(batch.positions.len(), batch.normals.len());
                            assert_eq!(batch.positions.len(), batch.uvs.len());
                            assert_eq!(batch.positions.len(), batch.tangents.len());
                            assert!(
                                batch
                                    .indices
                                    .iter()
                                    .all(|&i| (i as usize) < batch.positions.len())
                            );
                            assert!(batch.positions.iter().all(|p| {
                                (0..3).all(|i| {
                                    p[i] >= key[i] as f32 - 1e-6
                                        && p[i] <= key[i] as f32 + 1. + 1e-6
                                })
                            }));
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn adjacent_fitted_pipes_do_not_duplicate_their_shared_plane_at_any_section_edge() {
        let templates = templates();
        for axis in 0..3 {
            for edge in [-17, -1, 15] {
                let mut a = [5, 5, 5];
                a[axis] = edge;
                let mut b = a;
                b[axis] += 1;
                let mut cells = BTreeMap::from([(a, core::WOOD_PIPE), (b, core::GLASS_PIPE)]);
                for p in [a, b] {
                    for d in core::DIRECTIONS.into_iter().filter(|d| d[axis] == 0) {
                        cells.insert(std::array::from_fn(|i| p[i] + d[i]), 16);
                    }
                }
                let mut journal = EditJournal::default();
                let facing = [0, 4, 1][axis];
                for p in [a, b] {
                    journal.record(p, cells[&p]).unwrap();
                    journal
                        .set_object_state(
                            p,
                            Some(BlockObjectState {
                                facing,
                                ..Default::default()
                            }),
                        )
                        .unwrap();
                }
                for (p, shared_side) in [(a, axis * 2 + 1), (b, axis * 2)] {
                    let fitted = capture(&model(cells[&p], facing), p, &templates, &journal, |q| {
                        Some(cells.get(&q).copied().unwrap_or(0))
                    })
                    .unwrap();
                    assert!(fitted.hidden & (1 << shared_side) != 0);
                    for part in profile(&fitted, [0; 6]) {
                        assert!(
                            !part.indices.chunks_exact(3).any(|t| t.iter().all(|&i| (part
                                .positions[i as usize][axis]
                                - (shared_side % 2) as f32)
                                .abs()
                                < 1e-6)),
                            "duplicate shared plane {axis} {edge}"
                        );
                    }
                }
            }
        }
    }
}
