use super::*;
use crate::blocks::{BlockTemplate, TemplateSet, definitions::BlockRenderKind};
use terra_core::castle::{self, connections};

fn templates(material: u16) -> TemplateSet {
    let id = MaterialId::from_raw(material).unwrap();
    let name = id.identifier().strip_prefix("teramic::").unwrap();
    let path: &'static str = Box::leak(format!("terra/blocks/models/{name}.gltf").into_boxed_str());
    let geometry = crate::blocks::test_static_scene(path, material);
    assert_eq!(
        geometry.len(),
        connections::MODULE_COUNT * connections::MODULE_PRIMITIVES
    );
    let mut templates = TemplateSet::default();
    templates.blocks.insert(
        material,
        BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::CustomMesh,
            bevel_width: 0.0,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [0; 6],
            side_cap: None,
            details: Vec::new(),
            model: geometry,
        },
    );
    templates
}

#[test]
fn authored_end_straight_corner_t_and_cross_meshes_match_physics_and_placement_ghosts() {
    for material in [
        castle::BERGFRIED_WALL,
        castle::BATTLEMENT_STRAIGHT,
        castle::BATTLEMENT_CORNER,
        castle::BATTLEMENT_PILLAR,
    ] {
        let templates = templates(material);
        for facing in 0..24 {
            for mask in [1_u8, 3, 5, 7, 15] {
                let mut neighbors = [None; 6];
                for port in 0..4 {
                    if mask & (1 << port) != 0 {
                        neighbors[connections::port_side(port, facing)] =
                            Some(castle::BATTLEMENT_CORNER);
                    }
                }
                let states = [facing; 6];
                let batches = model_geometry(
                    material,
                    terra_world::edits::BlockObjectState {
                        facing,
                        ..Default::default()
                    },
                    Default::default(),
                    neighbors,
                    states,
                    &templates,
                )
                .unwrap();
                let selected = connections::modules(material, mask);
                let source = &templates.blocks[&material].model;
                let expected: usize = selected
                    .iter()
                    .map(|module| {
                        source[module * connections::MODULE_PRIMITIVES
                            ..(module + 1) * connections::MODULE_PRIMITIVES]
                            .iter()
                            .map(|p| p.positions.len())
                            .sum::<usize>()
                    })
                    .sum();
                assert_eq!(
                    batches.iter().map(|b| b.positions.len()).sum::<usize>(),
                    expected,
                    "wrong module selection {material}/{facing}/{mask}"
                );
                let solids = connections::placed_boxes(
                    material,
                    facing,
                    std::array::from_fn(|side| neighbors[side].map(|id| (id, states[side]))),
                );
                for batch in batches {
                    for point in batch.positions {
                        assert!(
                            solids.iter().any(|b| (0..3)
                                .all(|axis| f64::from(point[axis]) >= b.minimum[axis] - 1e-6
                                    && f64::from(point[axis]) <= b.maximum[axis] + 1e-6)),
                            "unpickable authored masonry {material}/{facing}/{mask}/{point:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn connected_profiles_use_bounded_keys_and_refresh_after_neighbor_removal_or_rotation() {
    let material = castle::BATTLEMENT_CORNER;
    let templates = templates(material);
    let source = &templates.blocks[&material].model;
    let mut profiles = super::castle::Profiles::default();
    let mut model = ModelCell {
        cell: [15, 15, 15],
        material,
        state: Default::default(),
        peer_wood: Default::default(),
        neighbors: [None; 6],
        neighbor_facing: [0; 6],
    };
    let isolated = profiles.get(&model, source).unwrap().into_owned();
    model.neighbors[1] = Some(castle::BATTLEMENT_STRAIGHT);
    model.neighbors[5] = Some(castle::BERGFRIED_WALL);
    let joined = profiles.get(&model, source).unwrap().into_owned();
    assert_ne!(
        joined.iter().map(|p| &p.positions).collect::<Vec<_>>(),
        isolated.iter().map(|p| &p.positions).collect::<Vec<_>>()
    );
    model.neighbor_facing[1] = 8; // Different support plane removes this port.
    let turned = profiles.get(&model, source).unwrap().into_owned();
    assert_ne!(
        turned.iter().map(|p| &p.positions).collect::<Vec<_>>(),
        joined.iter().map(|p| &p.positions).collect::<Vec<_>>()
    );
    model.neighbors = [None; 6];
    let removed = profiles.get(&model, source).unwrap().into_owned();
    assert_eq!(
        removed.iter().map(|p| &p.positions).collect::<Vec<_>>(),
        isolated.iter().map(|p| &p.positions).collect::<Vec<_>>()
    );
}

#[test]
fn authored_pillar_stacks_select_only_outer_crowns_and_feet_with_live_vertical_cache_keys() {
    let material = castle::BATTLEMENT_PILLAR;
    let templates = templates(material);
    let source = &templates.blocks[&material].model;
    let mut cache = super::castle::Profiles::default();
    for facing in 0..24 {
        let up = terra_core::construction::stair_basis(facing)[1];
        let axis = up.iter().position(|v| *v != 0).unwrap();
        let top = axis * 2 + usize::from(up[axis] > 0);
        let mut model = ModelCell {
            cell: [15, 15, 15],
            material,
            state: terra_world::edits::BlockObjectState {
                facing,
                ..Default::default()
            },
            peer_wood: Default::default(),
            neighbors: [None; 6],
            neighbor_facing: [facing; 6],
        };
        for key in [0_u8, 16, 32, 48, 18, 34, 55, 15] {
            model.neighbors = [None; 6];
            for port in 0..4 {
                if key & (1 << port) != 0 {
                    model.neighbors[connections::port_side(port, facing)] = Some(material);
                }
            }
            if key & connections::STACK_ABOVE != 0 {
                model.neighbors[top] = Some(material);
            }
            if key & connections::STACK_BELOW != 0 {
                model.neighbors[top ^ 1] = Some(material);
            }
            let actual = cache.get(&model, source).unwrap();
            let selected = connections::modules(material, key);
            let expected = selected
                .iter()
                .flat_map(|module| {
                    &source[module * connections::MODULE_PRIMITIVES
                        ..(module + 1) * connections::MODULE_PRIMITIVES]
                })
                .map(|p| &p.positions)
                .collect::<Vec<_>>();
            assert_eq!(
                actual.iter().map(|p| &p.positions).collect::<Vec<_>>(),
                expected
            );
            let batches = model_geometry(
                material,
                model.state,
                Default::default(),
                model.neighbors,
                model.neighbor_facing,
                &templates,
            )
            .unwrap();
            let solids = connections::placed_boxes(
                material,
                facing,
                std::array::from_fn(|side| model.neighbors[side].map(|id| (id, facing))),
            );
            for point in batches.iter().flat_map(|b| &b.positions) {
                assert!(
                    solids.iter().any(|b| (0..3)
                        .all(|axis| f64::from(point[axis]) >= b.minimum[axis] - 1e-6
                            && f64::from(point[axis]) <= b.maximum[axis] + 1e-6)),
                    "pillar mesh outside matching collider {facing}/{key}/{point:?}"
                );
            }
        }
    }
}
