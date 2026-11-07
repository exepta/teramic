use super::*;
use std::collections::HashMap;
#[test]
fn spruce_uses_its_own_compact_crown_without_extra_materials_or_unbounded_variants() {
    let spruce = species_template(6, 63, 0, 2).unwrap();
    let oak = species_template(6, 63, 0, 0).unwrap();
    let birch = species_template(6, 63, 0, 1).unwrap();
    let shape = |c: &CrownTemplate| {
        c.faces
            .iter()
            .map(|f| (f.minimum, f.side, f.width, f.height))
            .collect::<Vec<_>>()
    };
    assert_ne!(shape(&spruce), shape(&oak));
    assert_ne!(shape(&spruce), shape(&birch));
    assert!(spruce.faces.len() < 200);
    assert_eq!(CROWNS.len(), 1536);
}

type Point = [i32; 3];
type Edges = HashMap<[Point; 2], u32>;

fn edges(template: &CrownTemplate, offset: [i32; 3], edges: &mut Edges) {
    let lattice = |index: u32| {
        if template.resolution == 6 {
            index as i32
        } else {
            [0, 1, 3, 5, 6][index as usize]
        }
    };
    for face in &template.faces {
        let mut minimum = face.minimum.map(lattice);
        for axis in 0..3 {
            minimum[axis] += offset[axis];
        }
        if face.side % 2 == 1 {
            let axis = face.side / 2;
            minimum[axis] = lattice(face.minimum[axis] + 1) + offset[axis];
        }
        for u in
            0..lattice(face.minimum[face.u_axis] + face.width) - lattice(face.minimum[face.u_axis])
        {
            for v in 0..lattice(face.minimum[face.v_axis] + face.height)
                - lattice(face.minimum[face.v_axis])
            {
                let mut a = minimum;
                a[face.u_axis] += u;
                a[face.v_axis] += v;
                let mut b = a;
                b[face.u_axis] += 1;
                let mut c = b;
                c[face.v_axis] += 1;
                let mut d = a;
                d[face.v_axis] += 1;
                for (a, b) in [(a, b), (b, c), (c, d), (d, a)] {
                    let edge = if a < b { [a, b] } else { [b, a] };
                    *edges.entry(edge).or_default() += 1;
                }
            }
        }
    }
}

#[test]
fn crowns_are_closed_and_match_neighbor_interfaces_across_lods() {
    for birch in [false, true] {
        for resolution in [6, 4] {
            for variant in 0..4 {
                let isolated = crown_template(resolution, 63, variant, birch).unwrap();
                let mut incidence = Edges::new();
                edges(&isolated, [0; 3], &mut incidence);
                assert!(
                    incidence
                        .values()
                        .all(|count| *count >= 2 && count.is_multiple_of(2)),
                    "open edges: {:?}",
                    incidence
                        .iter()
                        .filter(|(_, count)| !count.is_multiple_of(2))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    for (left_resolution, right_resolution) in [(6, 6), (4, 4), (6, 4)] {
        for (left_birch, right_birch) in [(false, false), (true, true), (true, false)] {
            let left = crown_template(left_resolution, 63 & !2, 0, left_birch).unwrap();
            let right = crown_template(right_resolution, 63 & !1, 3, right_birch).unwrap();
            let mut incidence = Edges::new();
            edges(&left, [0; 3], &mut incidence);
            edges(&right, [6, 0, 0], &mut incidence);
            assert!(
                incidence
                    .values()
                    .all(|count| *count >= 2 && count.is_multiple_of(2)),
                "unclosed neighbor boundary for {left_resolution}/{right_resolution}"
            );
        }
    }
}

#[test]
fn birch_has_smaller_asymmetric_leaf_clusters_without_changing_cell_bounds() {
    let volume = |template: &CrownTemplate| {
        template
            .faces
            .iter()
            .map(|face| {
                let positive = !face.side.is_multiple_of(2);
                let axis = face.side / 2;
                let plane = coordinate(
                    face.minimum[axis] + u32::from(positive),
                    template.resolution,
                );
                let width = coordinate(face.minimum[face.u_axis] + face.width, template.resolution)
                    - coordinate(face.minimum[face.u_axis], template.resolution);
                let height =
                    coordinate(face.minimum[face.v_axis] + face.height, template.resolution)
                        - coordinate(face.minimum[face.v_axis], template.resolution);
                plane * width * height * if positive { 1.0 } else { -1.0 } / 3.0
            })
            .sum::<f32>()
    };
    for resolution in [6, 4] {
        let oak = crown_template(resolution, 63, 0, false).unwrap();
        let birch = crown_template(resolution, 63, 0, true).unwrap();
        assert!(volume(&birch) > 0.0);
        assert!(volume(&birch) < volume(&oak));
        let profile = |template: &CrownTemplate| {
            template
                .faces
                .iter()
                .map(|face| (face.minimum, face.width, face.height, face.side))
                .collect::<Vec<_>>()
        };
        assert_ne!(profile(&birch), profile(&oak));
    }
}

#[test]
fn cached_micro_crowns_have_bounded_geometry_and_no_faces_against_neighbors() {
    let templates: Vec<_> = (0..CROWNS.len())
        .map(|index| cached_crown(index).unwrap())
        .collect();
    assert_eq!(templates.len(), 1536);
    let maximum = templates
        .iter()
        .map(|template| template.faces.len())
        .max()
        .unwrap_or(0);
    assert!(maximum <= 100, "maximum {maximum} quads");
    for (index, template) in templates.iter().enumerate() {
        let exposure = (index / 4) % 64;
        for face in &template.faces {
            assert!(face.minimum.iter().all(|&v| v < template.resolution));
            assert!(face.minimum[face.u_axis] + face.width <= template.resolution);
            assert!(face.minimum[face.v_axis] + face.height <= template.resolution);
            let at_boundary = if face.side.is_multiple_of(2) {
                face.minimum[face.side / 2] == 0
            } else {
                face.minimum[face.side / 2] + 1 == template.resolution
            };
            if at_boundary {
                assert_ne!(exposure & (1 << face.side), 0);
            }
        }
    }
}

#[test]
fn leaf_micro_geometry_replaces_the_cube_for_all_existing_leaf_materials() {
    use super::super::builder::{RenderOptions, compose_section_with_options};
    use crate::blocks::TemplateSet;
    use crate::blocks::definitions::BlockRenderKind;
    for material in [7, 8, 9] {
        let block = BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::Cube,
            bevel_width: 0.0,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [usize::from(material); 6],
            side_cap: None,
            details: Vec::new(),
            model: Vec::new(),
        };
        let templates = TemplateSet {
            blocks: [(material, block)].into(),
            ..default()
        };
        let faces: Vec<_> = (0..6)
            .map(|side| BlockFace {
                edge_mask: 15,
                occupied_neighbor: false,
                minimum: [15; 3],
                width: 1,
                height: 1,
                u_axis: (side / 2 + 1) % 3,
                v_axis: (side / 2 + 2) % 3,
                side,
                material,
            })
            .collect();
        let options = RenderOptions {
            water_level: None,
            detail_level: DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        };
        let batches = compose_section_with_options(
            &faces,
            &templates,
            terra_voxel::SectionPos::new(-1, 0, -1),
            &options,
        )
        .unwrap();
        assert_eq!(batches.len(), 1);
        let batch = &batches[0];
        assert_eq!(batch.material, usize::from(material));
        assert_eq!(batch.foliage_owners.len(), 1);
        let owner = &batch.foliage_owners[0];
        assert_eq!(owner.cell, [15; 3]);
        assert_eq!(owner.material, material);
        assert_eq!(owner.vertices, 0..batch.positions.len());
        assert!(
            batch
                .positions
                .iter()
                .flatten()
                .all(|v| (15.0..=16.0).contains(v))
        );
        assert!(
            batch
                .positions
                .iter()
                .flatten()
                .any(|v| (v.fract() - 0.27).abs() < 1e-5)
        );
        for triangle in batch.indices.chunks_exact(3) {
            let [a, b, c] =
                std::array::from_fn(|i| Vec3::from_array(batch.positions[triangle[i] as usize]));
            assert!(
                (b - a)
                    .cross(c - a)
                    .dot(Vec3::from_array(batch.normals[triangle[0] as usize]))
                    > 0.0
            );
        }
    }
}

#[test]
fn crown_color_variation_keeps_the_area_weighted_lod_tint_unchanged() {
    for template in (0..CROWNS.len()).map(|index| cached_crown(index).unwrap()) {
        assert_eq!(template.tones.len(), template.faces.len());
        let mut deviation = 0.0;
        for (face, tone) in template.faces.iter().zip(&template.tones) {
            assert!(tone.abs() <= 0.12);
            let area = (coordinate(face.minimum[face.u_axis] + face.width, template.resolution)
                - coordinate(face.minimum[face.u_axis], template.resolution))
                * (coordinate(face.minimum[face.v_axis] + face.height, template.resolution)
                    - coordinate(face.minimum[face.v_axis], template.resolution));
            deviation += area * tone;
        }
        assert!(deviation.abs() < 1e-5);
    }
}
