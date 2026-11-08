use super::*;
use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
use terra_world::edits::BlockObjectState;

fn templates() -> TemplateSet {
    let mut templates = TemplateSet::default();
    let mut slots = BTreeMap::<String, usize>::new();
    for (material, path) in [
        (210, "terra/blocks/models/train_chassis.gltf"),
        (211, "terra/blocks/models/train_bogie.gltf"),
        (212, "terra/blocks/models/train_frame.gltf"),
        (213, "terra/blocks/models/train_coupler.gltf"),
        (214, "terra/blocks/models/train_cab.gltf"),
        (215, "terra/blocks/models/train_seat.gltf"),
        (216, "terra/blocks/models/train_cargo.gltf"),
    ] {
        let source: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(terra_core::asset_directory().join(path)).unwrap(),
        )
        .unwrap();
        let mut model = crate::blocks::test_static_scene(path, material);
        for primitive in &mut model {
            let name = source["materials"][primitive.material]["name"]
                .as_str()
                .unwrap();
            let next = slots.len();
            primitive.material = *slots.entry(name.to_owned()).or_insert(next);
            if name == "oak" {
                templates.wood_materials.insert(primitive.material);
            }
        }
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
                model,
            },
        );
    }
    assert!(!templates.wood_materials.is_empty());
    assert!(
        slots.len() <= 7,
        "authored train palette unexpectedly expanded"
    );
    templates
}

#[test]
fn assembled_geometry_matches_actual_placed_models_in_every_pose_and_wood() {
    let templates = templates();
    let parts: Vec<_> = (210..=216)
        .flat_map(|material| {
            (0..4).map(move |facing| TrainPart {
                material,
                facing,
                offset: [i16::from(facing) * 2, 1, (material - 210) as i16 * 2],
                wood: if facing % 2 == 0 {
                    terra_core::WoodSpecies::Birch
                } else {
                    terra_core::WoodSpecies::Cherry
                },
            })
        })
        .collect();
    let merged = geometry(
        &parts,
        &Default::default(),
        &templates,
        [0; 3],
        &Default::default(),
    )
    .unwrap();
    assert!(!merged.is_empty() && merged.len() <= 7);
    type AuthoredBatch = (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>);
    let mut expected = BTreeMap::<usize, AuthoredBatch>::new();
    let mut wood_colors = BTreeSet::new();
    for part in &parts {
        let state = BlockObjectState {
            facing: part.facing,
            wood: part.wood,
            ..default()
        };
        for batch in crate::meshing::custom::model_geometry(
            part.material,
            state,
            part.wood,
            [None; 6],
            [0; 6],
            &templates,
        )
        .unwrap()
        {
            let target = expected.entry(batch.material).or_default();
            let base = target.0.len() as u32;
            target.0.extend(batch.positions.iter().map(|point| {
                std::array::from_fn(|axis| point[axis] + f32::from(part.offset[axis]))
            }));
            target.1.extend(batch.normals);
            target.2.extend(batch.uvs);
            target
                .3
                .extend(batch.indices.into_iter().map(|index| base + index));
            if templates.wood_materials.contains(&batch.material) {
                assert!(batch.colors.iter().all(|color| *color == part.wood.tint()));
                wood_colors.insert(part.wood.tint().map(f32::to_bits));
            }
        }
    }
    assert_eq!(
        wood_colors.len(),
        2,
        "wood variant tints survive material merging"
    );
    assert_eq!(merged.len(), expected.len());
    for batch in merged {
        let target = expected.remove(&batch.material).unwrap();
        assert_eq!(
            (
                batch.positions.clone(),
                batch.normals.clone(),
                batch.uvs.clone(),
                batch.indices.clone()
            ),
            target
        );
        assert!(batch.tangents.is_empty() || batch.tangents.len() == batch.positions.len());
        assert!(batch.colors.is_empty() || batch.colors.len() == batch.positions.len());
        assert!(!batch.positions.is_empty() && !batch.indices.is_empty());
    }
}

#[test]
fn vehicle_geometry_key_ignores_cargo_revisions_and_tracks_authored_pose() {
    let part = TrainPart {
        offset: [0; 3],
        material: 210,
        facing: 0,
        wood: terra_core::WoodSpecies::Oak,
    };
    let key = GeometryKey {
        anchor: [-16, 12, 31],
        parts: vec![part],
        blocks: Default::default(),
    };
    assert!(key.matches([-16, 12, 31], &[part], &Default::default()));
    assert!(!key.matches([-15, 12, 31], &[part], &Default::default()));
    for changed in [
        TrainPart { facing: 1, ..part },
        TrainPart {
            offset: [1, 0, 0],
            ..part
        },
        TrainPart {
            wood: terra_core::WoodSpecies::Spruce,
            ..part
        },
    ] {
        assert!(!key.matches(key.anchor, &[changed], &Default::default()));
    }
}

#[test]
fn retired_train_mesh_assets_are_released_without_a_render_app() {
    let mut assets = Assets::<Mesh>::default();
    let batch = MeshBatch {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0; 2]; 3],
        indices: vec![0, 1, 2],
        ..default()
    };
    let handles: Vec<_> = (0..7)
        .map(|_| assets.add(crate::previews::batch_mesh(&batch)))
        .collect();
    assert_eq!(assets.len(), 7);
    release_meshes(&mut assets, handles);
    assert!(assets.is_empty());
}

#[test]
fn assembled_root_uses_the_same_fractional_origin_as_authoritative_models() {
    let vehicle = terra_core::trains::VehicleView {
        id: 1,
        owner: terra_core::PlayerUuid::from_u128(1),
        kind: terra_core::trains::VehicleKind::Passenger,
        anchor: [-16, 12, 31],
        parts: Vec::new(),
        couplings: [None; 2],
        revision: 1,
        mass_kg: 0,
        blocks: Default::default(),
    };
    assert_eq!(
        model_translation(vehicle.anchor).to_array(),
        vehicle.model_origin().map(|value| value as f32)
    );
    assert!(model_translation(vehicle.anchor).y < vehicle.anchor[1] as f32);
}

fn block_template(material: usize, model: Vec<crate::blocks::PrimitiveTemplate>) -> BlockTemplate {
    BlockTemplate {
        surface: None,
        render_kind: if model.is_empty() {
            BlockRenderKind::Cube
        } else {
            BlockRenderKind::CustomMesh
        },
        bevel_width: 0.0,
        faces: crate::blocks::cube::cube_faces(0.0, [material; 6], None),
        face_materials: [material; 6],
        side_cap: None,
        details: Vec::new(),
        model,
    }
}

#[test]
fn arbitrary_cubes_keep_their_surface_and_dye_instead_of_becoming_invisible() {
    let mut templates = templates();
    templates
        .blocks
        .insert(103, block_template(100, Vec::new()));
    let part = TrainPart {
        material: 103,
        facing: 0,
        offset: [3, 2, -1],
        wood: Default::default(),
    };
    let tint = [42, 172, 190];
    let blocks = [(
        0,
        TrainBlockView {
            tint: Some(tint),
            ..default()
        },
    )]
    .into();
    let batches = geometry(&[part], &blocks, &templates, [0; 3], &Default::default()).unwrap();
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];
    assert_eq!(batch.material, 100);
    assert_eq!(batch.indices.len(), 36);
    assert_eq!(batch.colors.len(), batch.positions.len());
    assert!(
        batch
            .colors
            .iter()
            .all(|color| *color == crate::dye_tint::rgb(tint))
    );
    for axis in 0..3 {
        let low = batch
            .positions
            .iter()
            .map(|p| p[axis])
            .min_by(f32::total_cmp)
            .unwrap();
        let high = batch
            .positions
            .iter()
            .map(|p| p[axis])
            .max_by(f32::total_cmp)
            .unwrap();
        assert_eq!(low, f32::from(part.offset[axis]));
        assert_eq!(high, f32::from(part.offset[axis]) + 1.0);
    }
}

#[test]
fn side_and_top_slabs_keep_their_non_yaw_poses_and_future_custom_models_are_accepted() {
    let mut templates = templates();
    let slab = crate::blocks::cube::box_model(terra_core::construction::boxes(82), 0.0, [100; 6]);
    templates.blocks.insert(82, block_template(100, slab));
    let future = crate::blocks::cube::box_model(
        &[terra_core::block_shapes::ShapeBox {
            minimum: [0.2, 0.1, 0.3],
            maximum: [0.8, 0.9, 0.7],
            hinged: false,
        }],
        0.0,
        [101; 6],
    );
    templates.blocks.insert(60_000, block_template(101, future));
    for facing in [4, 5, 6, 7, 8] {
        let part = TrainPart {
            material: 82,
            facing,
            offset: [0; 3],
            wood: Default::default(),
        };
        let bounds = terra_core::construction::slab_bounds(facing);
        let batches = geometry(
            &[part],
            &Default::default(),
            &templates,
            [0; 3],
            &Default::default(),
        )
        .unwrap();
        for axis in 0..3 {
            let points = || {
                batches
                    .iter()
                    .flat_map(|batch| batch.positions.iter().map(move |p| p[axis]))
            };
            assert!(
                (points().min_by(f32::total_cmp).unwrap() - bounds.minimum[axis] as f32).abs()
                    < 1e-5
            );
            assert!(
                (points().max_by(f32::total_cmp).unwrap() - bounds.maximum[axis] as f32).abs()
                    < 1e-5
            );
        }
    }
    let future = TrainPart {
        material: 60_000,
        facing: 1,
        offset: [0; 3],
        wood: Default::default(),
    };
    assert!(
        !geometry(
            &[future],
            &Default::default(),
            &templates,
            [0; 3],
            &Default::default()
        )
        .unwrap()[0]
            .positions
            .is_empty()
    );
}

#[test]
fn double_chest_retains_both_wood_colors_and_emits_its_shared_body_once() {
    let mut templates = templates();
    let mut chest = crate::blocks::test_static_scene("terra/blocks/models/chest.gltf", 29);
    for primitive in &mut chest {
        primitive.material = 100;
    }
    templates.wood_materials.insert(100);
    templates.blocks.insert(29, block_template(100, chest));
    let first = TrainPart {
        material: 29,
        facing: 0,
        offset: [0; 3],
        wood: terra_core::WoodSpecies::Birch,
    };
    let parts = [
        first,
        TrainPart {
            offset: [1, 0, 0],
            wood: terra_core::WoodSpecies::Cherry,
            ..first
        },
    ];
    let blocks = [
        (
            0,
            TrainBlockView {
                peer: Some([1, 0, 0]),
                ..default()
            },
        ),
        (
            1,
            TrainBlockView {
                peer: Some([-1, 0, 0]),
                secondary: true,
                ..default()
            },
        ),
    ]
    .into();
    let batches = geometry(&parts, &blocks, &templates, [0; 3], &Default::default()).unwrap();
    let batch = &batches[0];
    assert!(batch.colors.contains(&parts[0].wood.tint()));
    assert!(batch.colors.contains(&parts[1].wood.tint()));
    let expected = crate::meshing::custom::vehicle_model_geometry(
        &first,
        blocks.get(&0),
        parts[1].wood,
        [None; 6],
        &templates,
        [0; 3],
    )
    .unwrap();
    assert_eq!(batch.indices.len(), expected[0].indices.len());
}

#[test]
fn water_refilling_a_removed_source_cell_does_not_hide_the_mounted_model() {
    let part = TrainPart {
        offset: [0; 3],
        material: 163,
        facing: 0,
        wood: Default::default(),
    };
    let cell = [0, 2, 0];
    let (owner, local) = terra_voxel::global_to_section(cell);
    let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
    data.set_block(local, 27).unwrap();
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        owner,
        terra_world::streaming::LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let mut journal = EditJournal::default();
    journal.record(cell, 27).unwrap();
    let mut rendered = RenderedSections::default();
    rendered
        .block_revisions
        .insert(owner, journal.revision(owner));
    assert!(terrain_retired(cell, &[part], &chunks, &journal, &rendered));
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, part.material)
        .unwrap();
    assert!(!terrain_retired(
        cell,
        &[part],
        &chunks,
        &journal,
        &rendered
    ));
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, 27)
        .unwrap();
    rendered.block_revisions.clear();
    assert!(!terrain_retired(
        cell,
        &[part],
        &chunks,
        &journal,
        &rendered
    ));
}
