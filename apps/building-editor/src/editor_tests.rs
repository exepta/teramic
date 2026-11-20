use super::*;
use terra_voxel::{BlockSection, SectionPos, global_to_section};
use terra_world::streaming::{ChunkFocus, LoadedSection, SectionRemoved};

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Document>()
        .init_resource::<Editor>()
        .init_resource::<crate::workspace::WorkspacePlatform>()
        .init_resource::<ChunkStore>()
        .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .init_resource::<PlayerInputBlock>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<bevy::input_focus::InputFocus>()
        .insert_resource(ChunkFocus([0.0; 3]))
        .add_message::<CaptureRequest>()
        .add_message::<SectionRemoved>()
        .add_message::<AppExit>();
    let journal = crate::scene::journal(
        app.world().resource::<Document>(),
        crate::workspace::WorkspacePlatform::default(),
    )
    .unwrap();
    for y in -1..=0 {
        for z in -1..=0 {
            for x in -1..=0 {
                let p = SectionPos::new(x, y, z);
                let mut data = BlockSection::empty(p).unwrap();
                for (global, material) in journal.snapshot(p).unwrap() {
                    let (section, local) = global_to_section(global);
                    if section == p {
                        data.set_block(local, material).unwrap();
                    }
                }
                app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
                    p,
                    LoadedSection {
                        data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
        }
    }
    app.insert_resource(journal);
    app
}
#[test]
fn real_local_block_commit_and_removal_update_voxels_and_document() {
    let mut app = app();
    let plan = terra_world::placement::layout(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        [0, -1, 0],
        0,
        43,
        [0.0; 2],
        None,
    )
    .unwrap();
    apply(app.world_mut(), plan).unwrap();
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([0, -1, 0]),
        Some(43)
    );
    assert_eq!(app.world().resource::<Document>().cells.len(), 1);
    assert!(app.world().resource::<Document>().dirty());
    let template = app.world().resource::<Document>().template().unwrap();
    assert_eq!(
        template.steps[0].phase,
        terra_core::building_jobs::BuildPhase::Floor
    );
    assert_eq!(template.materials.iter().map(|s| s.count).sum::<u16>(), 1);
    remove(app.world_mut(), [0, -1, 0]).unwrap();
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([0, -1, 0]),
        Some(0)
    );
    assert!(app.world().resource::<Document>().cells.is_empty());
    assert_eq!(
        app.world()
            .resource::<ChunkStore>()
            .block_at([0, crate::workspace::PLATFORM_Y, 0]),
        Some(crate::workspace::PLATFORM_MATERIAL)
    );
    assert!(remove(app.world_mut(), [0, crate::workspace::PLATFORM_Y, 0]).is_err());
}

#[test]
fn technical_support_and_side_extensions_cannot_be_authored_or_removed() {
    use crate::workspace::{PLATFORM_MATERIAL, PLATFORM_Y};
    let mut app = app();
    app.world_mut().spawn((
        TerraCamera,
        Transform::from_xyz(7.5, PLATFORM_Y as f32 + 0.5, 0.5)
            .looking_at(Vec3::new(4.5, PLATFORM_Y as f32 + 0.5, 0.5), Vec3::Y),
    ));
    let hit = terra_voxel::blocks::BlockHit {
        position: [4, PLATFORM_Y, 0],
        normal: [1, 0, 0],
        distance: 3.0,
    };
    assert!(
        place(app.world_mut(), hit)
            .unwrap_err()
            .contains("protected")
    );
    assert!(remove(app.world_mut(), [0, PLATFORM_Y, 0]).is_err());
    let plan = Layout {
        edits: vec![
            BlockEditRequest {
                position: [0, -2, 0],
                expected_material: 0,
                material: 16,
            },
            BlockEditRequest {
                position: [0, PLATFORM_Y, 0],
                expected_material: PLATFORM_MATERIAL,
                material: 0,
            },
        ],
        states: Vec::new(),
        belts: Vec::new(),
    };
    assert!(apply(app.world_mut(), plan).is_err());
    let document = app.world().resource::<Document>();
    assert!(document.cells.is_empty());
    assert_eq!(document.revision, 0);
    assert_eq!(app.world().resource::<EditJournal>().edited_blocks(), 81);
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([0, -2, 0]),
        Some(0)
    );
    assert_eq!(
        app.world()
            .resource::<ChunkStore>()
            .block_at([0, PLATFORM_Y, 0]),
        Some(PLATFORM_MATERIAL)
    );
    assert_eq!(
        app.world()
            .resource::<ChunkStore>()
            .block_at([5, PLATFORM_Y, 0]),
        Some(0)
    );
}

#[test]
fn resizing_from_settings_preserves_authored_foundations_camera_and_export() {
    use crate::workspace::{PLATFORM_MATERIAL, PLATFORM_SIZES, PLATFORM_Y, WorkspacePlatform};
    let mut app = app();
    for (position, material) in [([0, -2, 0], 43), ([0, -1, 0], 16)] {
        let plan = terra_world::placement::layout(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            position,
            0,
            material,
            [0.0; 2],
            None,
        )
        .unwrap();
        apply(app.world_mut(), plan).unwrap();
    }
    let before = app.world().resource::<Document>().clone();
    let template = before.template().unwrap();
    let camera_pose = Transform::from_xyz(10.0, 7.0, 13.0).looking_at(Vec3::ZERO, Vec3::Y);
    let camera = app.world_mut().spawn((TerraCamera, camera_pose)).id();
    app.init_resource::<GameConfig>()
        .init_resource::<terra_core::GraphicsConfig>();
    app.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .view_distance = 2;
    for size in PLATFORM_SIZES {
        change(app.world_mut(), "platform-size", &size.to_string()).unwrap();
        let platform = *app.world().resource::<WorkspacePlatform>();
        assert_eq!(platform.size(), size);
        let document = app.world().resource::<Document>();
        assert_eq!(document.cells, before.cells);
        assert_eq!(document.revision, before.revision);
        assert_eq!(document.dirty(), before.dirty());
        assert_eq!(document.template().unwrap(), template);
        assert_eq!(*app.world().get::<Transform>(camera).unwrap(), camera_pose);
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(
            journal.edited_blocks(),
            usize::from(size) * usize::from(size) + 2
        );
        assert_eq!(
            journal.block([platform.half_size(), PLATFORM_Y, 0]),
            Some(PLATFORM_MATERIAL)
        );
        assert_eq!(
            journal.block([platform.half_size() + 1, PLATFORM_Y, 0]),
            None
        );
        assert_eq!(
            app.world().resource::<GameConfig>().client.view_distance,
            platform.view_distance()
        );
    }
    for invalid in ["0", "10", "65535", "-1", "not-a-size"] {
        assert!(change(app.world_mut(), "platform-size", invalid).is_err());
        assert_eq!(app.world().resource::<WorkspacePlatform>().size(), 33);
        assert_eq!(
            app.world().resource::<Document>().template().unwrap(),
            template
        );
    }
    let directory =
        std::env::temp_dir().join(format!("terra-editor-support-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("support-excluded.json");
    crate::document::save_template(&path, &template, true).unwrap();
    let restored = terra_core::building_jobs::template::BuildingTemplate::load(&path).unwrap();
    assert_eq!(restored, template);
    assert_eq!(
        restored
            .steps
            .iter()
            .map(|step| step.cells.len())
            .sum::<usize>(),
        2
    );
    assert!(
        restored
            .steps
            .iter()
            .flat_map(|step| &step.cells)
            .all(|cell| cell.position[1] > PLATFORM_Y)
    );
    assert_eq!(
        restored
            .materials
            .iter()
            .map(|stack| stack.count)
            .sum::<u16>(),
        2
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn platform_size_survives_model_import_and_new_document() {
    use crate::workspace::{PLATFORM_MATERIAL, PLATFORM_Y, WorkspacePlatform};
    let mut app = app();
    change(app.world_mut(), "platform-size", "17").unwrap();
    command_action(app.world_mut(), "default", "").unwrap();
    assert_eq!(app.world().resource::<WorkspacePlatform>().size(), 17);
    assert!(!app.world().resource::<Document>().cells.is_empty());
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .block([8, PLATFORM_Y, 8]),
        Some(PLATFORM_MATERIAL)
    );
    assert!(
        app.world()
            .resource::<Document>()
            .cells
            .keys()
            .all(|p| p[1] > PLATFORM_Y)
    );
    command_action(app.world_mut(), "new", "").unwrap();
    assert_eq!(app.world().resource::<WorkspacePlatform>().size(), 17);
    assert!(app.world().resource::<Document>().cells.is_empty());
    assert_eq!(
        app.world().resource::<EditJournal>().edited_blocks(),
        17 * 17
    );
    let shifted = Document {
        anchor: [-32, 20, -48],
        ..Default::default()
    };
    crate::scene::install_document(app.world_mut(), shifted).unwrap();
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .block([-40, 20 + PLATFORM_Y, -56]),
        Some(PLATFORM_MATERIAL)
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .block([-41, 20 + PLATFORM_Y, -56]),
        None
    );
    assert!(!app.world().resource::<Document>().dirty());
}
#[test]
fn native_oak_and_spruce_doors_use_real_pairs_and_one_paid_item() {
    for material in [65, 67] {
        let mut app = app();
        for (p, material) in [([0, -1, 0], 43), ([0, 0, 0], material)] {
            let plan = terra_world::placement::layout(
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
                p,
                0,
                material,
                [0.0; 2],
                None,
            )
            .unwrap();
            apply(app.world_mut(), plan).unwrap();
        }
        let document = app.world().resource::<Document>();
        let first = document.cells.get(&[0, 0, 0]).unwrap();
        let peer = first.state.peer.unwrap();
        assert_eq!(peer, [0, 1, 0]);
        assert!(document.cells.get(&peer).unwrap().state.secondary);
        let template = document.template().unwrap();
        assert_eq!(
            template
                .materials
                .iter()
                .find(|s| s.item == ItemKind::Block(material))
                .unwrap()
                .count,
            1
        );
        let imported = Document::from_template(template, None).unwrap();
        let restored =
            crate::scene::journal(&imported, crate::workspace::WorkspacePlatform::default())
                .unwrap();
        assert_eq!(imported.cells, document.cells);
        assert_eq!(restored.object_state(peer).unwrap().peer, Some([0, 0, 0]));
    }
}
#[test]
fn model_foundations_remain_above_the_protected_support_surface() {
    let mut app = app();
    let foundation = terra_world::placement::layout(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        [1, -2, 1],
        0,
        43,
        [0.0; 2],
        None,
    )
    .unwrap();
    apply(app.world_mut(), foundation).unwrap();
    let plan = terra_world::placement::layout(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        [0, -1, 0],
        0,
        16,
        [0.0; 2],
        None,
    )
    .unwrap();
    apply(app.world_mut(), plan).unwrap();
    let document = app.world().resource::<Document>();
    assert_eq!(document.cells.len(), 2);
    assert!(document.cells.contains_key(&[1, -2, 1]));
    assert_eq!(
        document.template().unwrap().steps[0].phase,
        terra_core::building_jobs::BuildPhase::Foundation
    );
}
#[test]
fn native_spruce_basin_placement_roundtrips_its_real_wood_state() {
    let mut app = app();
    let foundation = terra_world::placement::layout(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        [0, -2, 0],
        0,
        43,
        [0.0; 2],
        None,
    )
    .unwrap();
    apply(app.world_mut(), foundation).unwrap();
    let plan = terra_world::placement::resolve_with_chest(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        terra_voxel::blocks::BlockHit {
            position: [0, -2, 0],
            normal: [0, 1, 0],
            distance: 2.0,
        },
        ItemKind::Block(terra_core::plumbing::BASIN),
        ([0.5, 2.0, 0.5], [0.0, -1.0, 0.0]),
        [0.0; 2],
        false,
        terra_core::containers::ChestMerge::Auto,
    )
    .unwrap();
    apply(app.world_mut(), plan).unwrap();
    let document = app.world().resource::<Document>();
    assert_eq!(
        document.cells[&[0, -1, 0]].state.wood,
        terra_core::WoodSpecies::Spruce
    );
    let template = document.template().unwrap();
    assert_eq!(
        template
            .materials
            .iter()
            .find(|stack| stack.item == ItemKind::Block(169)),
        Some(&terra_core::ItemStack {
            item: ItemKind::Block(169),
            count: 1
        })
    );
    let restored = Document::from_template(template, None).unwrap();
    assert_eq!(restored.cells, document.cells);
    let journal =
        crate::scene::journal(&restored, crate::workspace::WorkspacePlatform::default()).unwrap();
    assert_eq!(
        journal.object_state([0, -1, 0]).unwrap().wood,
        terra_core::WoodSpecies::Spruce
    );
}
#[test]
fn typing_e_in_catalog_search_does_not_close_the_item_catalog() {
    let mut app = app();
    app.world_mut().resource_mut::<Editor>().panel = Panel::Items;
    let input = app
        .world_mut()
        .spawn(tilt_ui::ElementId("editor-search".into()))
        .id();
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocus>()
        .set(input, bevy::input_focus::FocusCause::Navigated);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyE);
    keyboard(app.world_mut());
    assert!(app.world().resource::<Editor>().panel == Panel::Items);
}
#[test]
fn original_filename_and_id_are_preserved_and_save_as_gets_a_new_identity() {
    let mut app = app();
    let directory =
        std::env::temp_dir().join(format!("terra-editor-identity-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let original = directory.join("residential.json");
    let mut document = Document {
        id: "permanent-house-id".into(),
        path: Some(original.clone()),
        ..Default::default()
    };
    document.cells.insert(
        [0, 0, 0],
        AuthoredCell {
            material: 43,
            state: Default::default(),
        },
    );
    crate::document::save_template(&original, &document.template().unwrap(), true).unwrap();
    crate::scene::install_document(app.world_mut(), document).unwrap();
    assert_eq!(app.world().resource::<Editor>().filename, "residential");
    save(app.world_mut(), false).unwrap();
    assert!(app.world().resource::<Editor>().panel == Panel::Overwrite);
    save(app.world_mut(), true).unwrap();
    assert_eq!(
        terra_core::building_jobs::template::BuildingTemplate::load(&original)
            .unwrap()
            .id,
        "permanent-house-id"
    );
    app.world_mut().resource_mut::<Editor>().filename = "second-house".into();
    save(app.world_mut(), false).unwrap();
    assert_eq!(
        terra_core::building_jobs::template::BuildingTemplate::load(
            &directory.join("second-house.json")
        )
        .unwrap()
        .id,
        "second-house"
    );
    assert_eq!(
        terra_core::building_jobs::template::BuildingTemplate::load(&original)
            .unwrap()
            .id,
        "permanent-house-id"
    );
    let mut target =
        terra_core::building_jobs::template::BuildingTemplate::load(&original).unwrap();
    target.id = "stable-target-house".into();
    let existing_target = directory.join("authored-home.json");
    crate::document::save_template(&existing_target, &target, false).unwrap();
    app.world_mut().resource_mut::<Editor>().filename = "authored-home".into();
    save(app.world_mut(), false).unwrap();
    assert!(app.world().resource::<Editor>().panel == Panel::Overwrite);
    save(app.world_mut(), true).unwrap();
    assert_eq!(
        terra_core::building_jobs::template::BuildingTemplate::load(&existing_target)
            .unwrap()
            .id,
        "stable-target-house"
    );
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn shifted_anchor_moves_real_camera_and_stream_focus_to_the_imported_building() {
    let mut app = app();
    let camera = app
        .world_mut()
        .spawn((TerraCamera, Transform::IDENTITY))
        .id();
    let document = Document {
        anchor: [104, 50, -88],
        ..Default::default()
    };
    crate::scene::install_document(app.world_mut(), document).unwrap();
    let transform = app.world().get::<Transform>(camera).unwrap();
    let baseline =
        crate::scene::initial_camera(&Document::default(), std::f32::consts::FRAC_PI_4, 1.6);
    assert!(
        (transform.translation - baseline.translation - Vec3::new(104.0, 50.0, -88.0)).length()
            < 0.0001
    );
    assert_eq!(
        app.world().resource::<ChunkFocus>().0,
        transform.translation.to_array().map(f64::from)
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .block([104, 50 + crate::workspace::PLATFORM_Y, -88]),
        Some(43)
    );
}
#[test]
fn editor_styles_and_static_icon_sources_use_real_runtime_parsers() {
    tilt_ui::tilt_ui_css::parse_stylesheet(include_str!("../ui/styles/editor.css")).unwrap();
    tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
        "../ui/components/building-editor-options/building-editor-options.component.css"
    ))
    .unwrap();
    for html in [
        include_str!("../ui/components/building-editor-hud/building-editor-hud.component.html"),
        include_str!("../ui/components/building-editor-items/building-editor-items.component.html"),
        include_str!("../ui/components/building-editor-files/building-editor-files.component.html"),
    ] {
        for icon in html.split("<icon ").skip(1) {
            let attrs = icon.split('>').next().unwrap();
            let name = attrs
                .split("name=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let size = attrs
                .split("size=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            assert!(
                tilt_ui::tilt_ui_icons::parse_source(&format!("tilt-icon:{name}@{size}")).is_some()
            );
        }
    }
}

#[test]
fn imported_house_aabb_fits_real_perspective_below_the_editor_toolbar() {
    let template = terra_core::building_jobs::template::BuildingTemplate::load(
        &terra_core::asset_directory()
            .join(terra_core::building_jobs::template::DEFAULT_HOUSE_PATH),
    )
    .unwrap();
    let document = Document::from_template(template, None).unwrap();
    let (min, max) = crate::scene::document_bounds(&document);
    for aspect in [0.8, 1.6, 2.0] {
        for fov in [45.0_f32, 90.0] {
            let camera = crate::scene::initial_camera(&document, fov.to_radians(), aspect);
            let projection = PerspectiveProjection {
                fov: fov.to_radians(),
                aspect_ratio: aspect,
                ..Default::default()
            };
            let clip = bevy::camera::CameraProjection::get_clip_from_view(&projection)
                * camera.to_matrix().inverse();
            for corner in 0..8 {
                let p = Vec3::new(
                    if corner & 1 == 0 { min.x } else { max.x },
                    if corner & 2 == 0 { min.y } else { max.y },
                    if corner & 4 == 0 { min.z } else { max.z },
                );
                let projected = clip * p.extend(1.0);
                let ndc = projected.truncate() / projected.w;
                assert!(
                    projected.w > 0.0 && ndc.x.abs() <= 0.881 && (-0.901..=0.551).contains(&ndc.y),
                    "AABB outside usable viewport: {ndc:?}"
                );
            }
        }
    }
}
