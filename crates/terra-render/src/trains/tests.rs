use super::*;
use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};

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
    let merged = geometry(&parts, &templates).unwrap();
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
    };
    assert!(key.matches([-16, 12, 31], &[part]));
    assert!(!key.matches([-15, 12, 31], &[part]));
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
        assert!(!key.matches(key.anchor, &[changed]));
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
    };
    assert_eq!(
        model_translation(vehicle.anchor).to_array(),
        vehicle.model_origin().map(|value| value as f32)
    );
    assert!(model_translation(vehicle.anchor).y < vehicle.anchor[1] as f32);
}
