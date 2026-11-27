use super::{MeshBatch, ModelCell};
use bevy::math::Vec3;
use std::collections::BTreeMap;
use terra_core::quartz_prisms::{self, Slice};

pub(super) fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    model: &ModelCell,
    slice: &Slice,
    primitives: &[crate::blocks::PrimitiveTemplate],
) {
    let offset = Vec3::from_array(model.cell.map(|v| v as f32));
    let d = usize::from(quartz_prisms::diameter(model.material));
    let code = usize::from(model.state.facing - 4);
    let crystal_cell = Vec3::new((code % d) as f32, offset.y, ((code / d) % d) as f32);
    for face in &slice.faces {
        if face.boundary.is_some_and(|side| {
            model.neighbors[side] == Some(model.material)
                && quartz_prisms::joins(
                    model.material,
                    model.state.facing,
                    side,
                    model.neighbor_facing[side],
                )
        }) {
            continue;
        }
        let shade = if face.boundary.is_some() {
            1
        } else {
            [0, 1, 0, 2, 1, 2][face.facet]
        };
        let slot = primitives[shade % primitives.len()].material;
        let batch = batches.entry(slot).or_insert_with(|| MeshBatch {
            material: slot,
            ..Default::default()
        });
        let start = batch.positions.len() as u32;
        let normal = face.normal.to_array();
        let tone = if face.boundary.is_some() {
            0.77
        } else {
            [1.0, 0.85, 0.96, 0.78, 0.83, 0.98][face.facet]
        };
        let tangent = Vec3::new(-normal[2], 0., normal[0]).normalize_or(Vec3::X);
        for vertex in &face.vertices {
            batch.positions.push((offset + *vertex).to_array());
            batch.normals.push(normal);
            let p = crystal_cell + *vertex;
            batch.uvs.push(if normal[1] != 0. {
                [p.x * 0.53, p.z * 0.53]
            } else {
                [
                    p.dot(tangent) * 0.53 + face.facet as f32 * 0.125,
                    p.y * 0.25 + p.x * 0.045,
                ]
            });
            batch
                .colors
                .push([tone, tone * (0.90 + 0.1 * tone), tone, 1.]);
        }
        for i in 1..face.vertices.len() as u32 - 1 {
            batch.indices.extend([start, start + i, start + i + 1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::PrimitiveTemplate;

    #[test]
    fn unsubscribed_natural_outcrop_uses_its_crystal_slices_across_the_section_edge() {
        use super::super::capture_prepared;
        use crate::blocks::{BlockTemplate, TemplateSet, definitions::BlockRenderKind};
        use terra_voxel::{BlockSection, global_to_section};
        use terra_world::{SectionGenerator, TerraGenerator, edits::EditJournal};

        // The outcrop crosses the negative X section edge at x = -1360.
        let section = global_to_section([-1360, 57, 562]).0;
        let generator = SectionGenerator::new(TerraGenerator::new(Default::default()).unwrap());
        let column = generator
            .prepare_column([section.x, section.z], true)
            .unwrap();
        let section_data = generator
            .build_prepared(section, 0, &Default::default(), &column)
            .unwrap();
        let data: &BlockSection = &section_data.data;
        let objects: BTreeMap<_, _> = column
            .model_objects_in_halo(section)
            .filter(|o| quartz_prisms::is_quartz(o.material))
            .map(|o| (o.position, o.state.unwrap().facing))
            .collect();
        assert!(objects.keys().any(|&p| global_to_section(p).0 != section));
        let mut templates = TemplateSet::default();
        for id in 770..=773 {
            let slots = [usize::from(id); 6];
            templates.blocks.insert(
                id,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.,
                    faces: Default::default(),
                    face_materials: slots,
                    side_cap: None,
                    details: Vec::new(),
                    model: crate::blocks::cube::box_model(
                        &[terra_core::block_shapes::ShapeBox {
                            minimum: [0.; 3],
                            maximum: [1.; 3],
                            hinged: false,
                        }],
                        0.,
                        slots,
                    ),
                },
            );
        }
        let snapshot = capture_prepared(
            data,
            &templates,
            &EditJournal::default(),
            Some(&column),
            &Default::default(),
            |_| None,
        )
        .unwrap();
        assert!(snapshot.models.len() > 20);
        let origin = section.origin().unwrap();
        for model in &snapshot.models {
            let p = std::array::from_fn(|i| origin[i] + i64::from(model.cell[i]));
            assert_eq!(model.state.facing, objects[&p]);
            for side in 0..6 {
                let mut neighbor = p;
                neighbor[side / 2] += if side % 2 == 0 { -1 } else { 1 };
                if let Some(&facing) = objects.get(&neighbor) {
                    assert_eq!(model.neighbor_facing[side], facing);
                }
            }
        }
    }

    #[test]
    fn crystal_shaft_hides_its_internal_cube_and_mining_opens_exactly_one_cut() {
        let material = terra_voxel::MaterialId::MassiveAmethyst as u16;
        let facing = quartz_prisms::code(material, 2, 2, 0);
        let mut model = ModelCell {
            cell: [0; 3],
            material,
            state: terra_world::edits::BlockObjectState {
                facing,
                ..Default::default()
            },
            peer_wood: Default::default(),
            neighbors: [Some(material); 6],
            neighbor_facing: [
                quartz_prisms::code(material, 1, 2, 0),
                quartz_prisms::code(material, 3, 2, 0),
                facing,
                facing,
                quartz_prisms::code(material, 2, 1, 0),
                quartz_prisms::code(material, 2, 3, 0),
            ],
        };
        let primitive = PrimitiveTemplate {
            positions: vec![],
            normals: vec![],
            tangents: vec![],
            uvs: vec![],
            indices: vec![],
            material: 9,
        };
        let templates = [primitive];
        let mut batches = BTreeMap::new();
        let shape = quartz_prisms::slice(material, facing).unwrap();
        append(&mut batches, &model, shape, &templates);
        assert!(batches.is_empty());
        model.neighbors[1] = None;
        append(&mut batches, &model, shape, &templates);
        let cut = &batches[&9];
        assert_eq!(cut.indices.len(), 6);
        assert!(cut.positions.iter().all(|p| (p[0] - 1.).abs() < 1e-4));
        assert!(cut.normals.iter().all(|n| n[0] > 0.999));
    }
}
