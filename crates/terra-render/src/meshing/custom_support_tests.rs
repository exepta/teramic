use super::*;
use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};
use terra_world::{
    edits::{BlockObjectState, EditJournal},
    streaming::{ChunkStore, LoadedSection},
};

#[test]
fn real_funnel_snapshot_shares_band_support_with_physics_across_negative_vertical_section_borders()
{
    let owner = [-16, 0, -16];
    let below = [-16, -1, -16];
    let mut templates = TemplateSet::default();
    templates.logistics_support_materials.extend([1, 2]);
    let primitive = |material| PrimitiveTemplate {
        material,
        positions: vec![
            [0.08, -0.375, 0.08],
            [0.20, -0.375, 0.08],
            [0.08, -0.305, 0.08],
        ],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        tangents: Vec::new(),
        uvs: vec![[0.0; 2]; 3],
        indices: vec![0, 1, 2],
    };
    for id in [153, 157] {
        templates.blocks.insert(
            id,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: vec![primitive(0), primitive(1), primitive(2)],
            },
        );
    }
    for id in [153, 157] {
        for mode in [
            None,
            Some((0, 0)),
            Some((3, 0)),
            Some((148, 0)),
            Some((148, 56)),
        ] {
            let section = terra_voxel::global_to_section(owner).0;
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            let mut data = BlockSection::empty(section).unwrap();
            data.set_block([0; 3], id).unwrap();
            chunks.loaded.insert(
                section,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            journal.record(owner, id).unwrap();
            journal
                .set_object_state(owner, Some(BlockObjectState::default()))
                .unwrap();
            if let Some((material, facing)) = mode {
                let (section, local) = terra_voxel::global_to_section(below);
                let mut data = BlockSection::empty(section).unwrap();
                data.set_block(local, material).unwrap();
                chunks.loaded.insert(
                    section,
                    LoadedSection {
                        data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
                if material == 148 {
                    journal.record(below, 148).unwrap();
                    let end = [below[0], below[1], below[2] + 1];
                    journal
                        .set_object_state(
                            below,
                            Some(BlockObjectState {
                                facing,
                                peer: (facing >= 56).then_some(end),
                                ..Default::default()
                            }),
                        )
                        .unwrap();
                    if facing >= 56 {
                        let start = [below[0], below[1], below[2] - 1];
                        for (cell, peer) in [(start, below), (end, start)] {
                            journal.record(cell, 144).unwrap();
                            journal
                                .set_object_state(
                                    cell,
                                    Some(BlockObjectState {
                                        facing: terra_core::kinetic::belt_facing(facing),
                                        peer: Some(peer),
                                        ..Default::default()
                                    }),
                                )
                                .unwrap();
                            let (section, local) = terra_voxel::global_to_section(cell);
                            chunks
                                .loaded
                                .entry(section)
                                .or_insert_with(|| LoadedSection {
                                    data: BlockSection::empty(section).unwrap(),
                                    mesh: None,
                                    mesh_epoch: 0,
                                    water_epoch: 0,
                                    water: None,
                                    props: Vec::new(),
                                })
                                .data
                                .set_block(local, 144)
                                .unwrap();
                        }
                        let segment = terra_world::conveyors::segment_at(&journal, below).unwrap();
                        assert!(segment.span.linked);
                        assert!(segment.span.loaded(&chunks, &journal));
                        assert_eq!(segment.span.normal()[1], 0.0);
                    }
                }
            }
            let supported = matches!(mode, Some((3, 0)) | Some((148, 0)));
            assert_eq!(
                terra_world::funnels::supported_with(owner, &journal, |p| chunks.block_at(p)),
                supported
            );
            let snapshot = capture(&chunks.loaded[&section].data, &templates, &journal, |p| {
                chunks.block_at(p)
            })
            .unwrap();
            assert_eq!(snapshot.funnel_supports.contains(&[0; 3]), supported);
            let batches = compose(
                &BlockMesh::default(),
                &snapshot,
                &templates,
                section,
                &RenderOptions {
                    water_level: None,
                    detail_level: super::super::DetailLevel::Near,
                    terrain: None,
                    exposure: None,
                    conditions: Default::default(),
                    tints: Default::default(),
                },
            )
            .unwrap();
            assert_eq!(
                batches.len(),
                if supported { 3 } else { 1 },
                "only real loaded support enables feet: {mode:?}"
            );
        }
    }
}
