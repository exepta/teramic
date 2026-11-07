use super::*;

#[test]
fn real_lantern_assets_emit_one_mount_only_and_keep_a_warm_emissive_physical_light() {
    use crate::meshing::{builder::RenderOptions, custom};
    use terra_core::lantern::{CHAIN, LANTERN};
    use terra_voxel::{BlockSection, SectionPos, meshing::BlockMesh};
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    let definition = library.blocks.iter().find(|b| b.id == LANTERN).unwrap();
    definition.validate().unwrap();
    let light = definition.light.unwrap();
    assert!(light.intensity > 1000. && light.range >= 20.);
    assert!(light.color[0] > light.color[1] && light.color[1] > light.color[2]);
    assert!(!light.shadows, "The glass emitter must not shadow itself");
    let model = load_static_scene("terra/blocks/models/lantern.gltf", LANTERN);
    let source: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            terra_core::asset_directory().join("terra/blocks/models/lantern.gltf"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut templates = TemplateSet::default();
    for (slot, material) in source["materials"].as_array().unwrap().iter().enumerate() {
        let name = material["name"].as_str().unwrap();
        let variant = name
            .strip_prefix("lantern_")
            .unwrap()
            .split('_')
            .next()
            .unwrap()
            .parse::<u8>()
            .unwrap();
        templates.lantern_variants.insert(slot, variant);
        if name.ends_with("_glass") || name.ends_with("_heart") {
            assert!(material["emissiveFactor"][0].as_f64().unwrap() > 0.9);
        } else {
            assert!(material.get("emissiveFactor").is_none());
        }
    }
    templates.blocks.insert(
        LANTERN,
        BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::CustomMesh,
            bevel_width: 0.,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [0; 6],
            side_cap: None,
            details: Vec::new(),
            model: model.clone(),
        },
    );
    for facing in [0, 4, 5, 6, 7, 8] {
        let hulls = terra_core::lantern::boxes(LANTERN, facing);
        for point in model
            .iter()
            .filter(|primitive| {
                templates.lantern_variants[&primitive.material]
                    == terra_core::lantern::variant(facing)
            })
            .flat_map(|primitive| &primitive.positions)
        {
            let point = terra_core::lantern::rotate_point(point.map(f64::from), facing);
            assert!(
                hulls
                    .iter()
                    .any(|shape| (0..3).all(|i| point[i] >= shape.minimum[i] - 1e-6
                        && point[i] <= shape.maximum[i] + 1e-6)),
                "Unpickable authored lantern detail {point:?} at pose{facing}"
            );
        }
        let owner = SectionPos::new(-1, 0, -1);
        let p = [-8, 8, -8];
        let mut section = BlockSection::empty(owner).unwrap();
        section.set_block([8, 8, 8], LANTERN).unwrap();
        let mut journal = terra_world::edits::EditJournal::default();
        journal.record(p, LANTERN).unwrap();
        journal
            .set_object_state(
                p,
                Some(terra_world::edits::BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        let snapshot = custom::capture(&section, &templates, &journal, |q| {
            Some(if q == p { LANTERN } else { 0 })
        })
        .unwrap();
        let parts = custom::compose(
            &BlockMesh::default(),
            &snapshot,
            &templates,
            owner,
            &RenderOptions {
                water_level: None,
                detail_level: crate::meshing::DetailLevel::Near,
                terrain: None,
                exposure: None,
                tints: Default::default(),
                conditions: Default::default(),
            },
        )
        .unwrap();
        let expected = model
            .iter()
            .filter(|primitive| {
                templates.lantern_variants[&primitive.material]
                    == terra_core::lantern::variant(facing)
            })
            .map(|p| p.positions.len())
            .sum::<usize>();
        assert_eq!(
            parts.iter().map(|p| p.positions.len()).sum::<usize>(),
            expected
        );
        assert!(expected > 500);
        assert!(
            parts
                .iter()
                .flat_map(|p| &p.positions)
                .all(|p| (0..3).all(|i| (8.0 - 1e-5..=9.0 + 1e-5).contains(&p[i])))
        );
    }
    let chain = load_static_scene("terra/blocks/models/chain.gltf", CHAIN);
    assert!(chain.iter().map(|p| p.positions.len()).sum::<usize>() > 500);
    assert!(
        chain
            .iter()
            .flat_map(|p| &p.positions)
            .all(|p| p.iter().all(|v| (0.0..=1.0).contains(v)))
    );
}
