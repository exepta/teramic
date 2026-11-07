//! Decorative water still has cube geometry. Its interface with an immersed
//! model is internal water, and must not become a translucent air-box wall.
use super::*;

impl ModelSnapshot {
    pub(crate) fn water_contacts_changed(
        &self,
        section: SectionPos,
        journal: &terra_world::edits::EditJournal,
    ) -> bool {
        let Ok(origin) = section.origin() else {
            return false;
        };
        self.water_contacts.iter().any(|(cell, &wet)| {
            let global = std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis]));
            wet != journal.fluid(global).is_some_and(|water| water.amount == 8)
        })
    }
    pub(crate) fn refresh_water_contacts(
        &mut self,
        section: SectionPos,
        journal: &terra_world::edits::EditJournal,
    ) {
        let Ok(origin) = section.origin() else {
            return;
        };
        for (cell, wet) in &mut self.water_contacts {
            let global = std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis]));
            *wet = journal.fluid(global).is_some_and(|water| water.amount == 8);
        }
    }
}

pub(super) fn visible_faces(
    faces: Vec<BlockFace>,
    contacts: &HashMap<[i32; 3], bool>,
) -> Vec<BlockFace> {
    if !contacts.values().any(|wet| *wet) {
        return faces;
    }
    let mut visible = Vec::with_capacity(faces.len());
    for face in faces {
        if face.material != MaterialId::SolidWater as u16 {
            visible.push(face);
            continue;
        }
        let mut cells = Vec::with_capacity((face.width * face.height) as usize);
        for u in 0..face.width {
            for v in 0..face.height {
                let mut cell = face.minimum;
                cell[face.u_axis] += u;
                cell[face.v_axis] += v;
                let mut neighbor = cell.map(|v| v as i32);
                neighbor[face.side / 2] += if face.side % 2 == 0 { -1 } else { 1 };
                if contacts.get(&neighbor) != Some(&true) {
                    cells.push(BlockFace {
                        minimum: cell,
                        width: 1,
                        height: 1,
                        ..face
                    });
                }
            }
        }
        if cells.len() == (face.width * face.height) as usize {
            visible.push(face);
        } else {
            visible.extend(cells);
        }
    }
    visible
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flowing_water_voxel_faces_are_never_restored_around_real_pipe_models_or_border_models() {
        use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
        use terra_core::plumbing;
        use terra_world::edits::{BlockObjectState, EditJournal};
        const WATER: usize = 9001;
        let primitive = crate::blocks::PrimitiveTemplate {
            positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            normals: vec![[0., 0., 1.]; 3],
            tangents: vec![],
            uvs: vec![[0.; 2]; 3],
            indices: vec![0, 1, 2],
            material: WATER,
        };
        for (material, path) in [
            (plumbing::WOOD_PIPE, "terra/blocks/models/wood_pipe.gltf"),
            (plumbing::STONE_PIPE, "terra/blocks/models/stone_pipe.gltf"),
            (plumbing::IRON_PIPE, "terra/blocks/models/iron_pipe.gltf"),
            (plumbing::GLASS_PIPE, "terra/blocks/models/glass_pipe.gltf"),
        ] {
            let mut templates = TemplateSet::default();
            templates.blocks.insert(
                27,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::Cube,
                    bevel_width: 0.,
                    faces: std::array::from_fn(|_| vec![primitive.clone()]),
                    face_materials: [WATER; 6],
                    side_cap: None,
                    details: vec![],
                    model: vec![],
                },
            );
            templates.blocks.insert(
                material,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.,
                    faces: std::array::from_fn(|_| vec![]),
                    face_materials: [0; 6],
                    side_cap: None,
                    details: vec![],
                    model: crate::blocks::test_static_scene(path, material),
                },
            );
            for p in [
                [8, 7, 8],
                [-1, 7, 8],
                [16, 7, 8],
                [8, -1, 8],
                [8, 16, 8],
                [8, 7, -1],
                [8, 7, 16],
            ] {
                for direction in plumbing::DIRECTIONS {
                    let q = std::array::from_fn(|i| p[i] + direction[i]);
                    if !q.iter().all(|v| (0..16).contains(v)) {
                        continue;
                    }
                    let mut section = BlockSection::empty(SectionPos::default()).unwrap();
                    section.set_block(q.map(|v| v as usize), 27).unwrap();
                    let mut journal = EditJournal::default();
                    journal.record(p, material).unwrap();
                    journal.record(q, 27).unwrap();
                    journal
                        .set_fluid(
                            p,
                            terra_world::water::FluidCell {
                                amount: 8,
                                source: true,
                                ..Default::default()
                            },
                        )
                        .unwrap();
                    journal
                        .set_object_state(
                            p,
                            Some(BlockObjectState {
                                facing: 4,
                                ..Default::default()
                            }),
                        )
                        .unwrap();
                    if p.iter().all(|v| (0..16).contains(v)) {
                        section.set_block(p.map(|v| v as usize), material).unwrap();
                    }
                    let snapshot = capture(&section, &templates, &journal, |cell| {
                        Some(if cell == p {
                            material
                        } else if cell == q {
                            27
                        } else {
                            0
                        })
                    })
                    .unwrap();
                    let batches = compose(
                        &BlockMesh::default(),
                        &snapshot,
                        &templates,
                        SectionPos::default(),
                        &RenderOptions {
                            water_level: None,
                            detail_level: super::super::super::DetailLevel::Near,
                            terrain: None,
                            exposure: None,
                            conditions: Default::default(),
                            tints: Default::default(),
                        },
                    )
                    .unwrap();
                    assert!(
                        !batches.iter().any(|batch| batch.material == WATER),
                        "extra cubical water faces around pipe {material} at {p:?} beside {q:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn only_internal_water_contacts_are_removed_including_section_border_models() {
        let face = BlockFace {
            minimum: [0, 4, 4],
            width: 3,
            height: 1,
            u_axis: 1,
            v_axis: 2,
            side: 0,
            material: 14,
            occupied_neighbor: false,
            edge_mask: 15,
        };
        let wet = HashMap::from([([-1, 5, 4], true)]);
        let remaining = visible_faces(vec![face], &wet);
        assert_eq!(remaining.len(), 2);
        assert_eq!(
            remaining
                .iter()
                .map(|face| face.minimum[1])
                .collect::<Vec<_>>(),
            [4, 6]
        );
        let dry = visible_faces(vec![face], &HashMap::from([([-1, 5, 4], false)]));
        assert_eq!(dry.len(), 1);
        assert_eq!(dry[0].minimum, face.minimum);
        assert_eq!(dry[0].width, face.width);
        let stone = BlockFace {
            material: 3,
            ..face
        };
        let untouched = visible_faces(vec![stone], &wet);
        assert_eq!(untouched.len(), 1);
        assert_eq!(untouched[0].material, 3);
        assert_eq!(untouched[0].width, 3);
    }
}
