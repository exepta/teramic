use super::*;
use bevy::ecs::message::Messages;
use terra_core::{ToolKind, farming::FarmingAction};
use terra_voxel::SectionPos;

#[test]
fn right_click_requests_shovel_or_tar_actions_with_the_actual_top_hit_and_no_speculative_edit() {
    for (tool, material, expected) in [
        (ToolKind::WoodShovel, 1, true),
        (ToolKind::StoneShovel, 2, true),
        (ToolKind::WoodShovel, 4, true),
        (ToolKind::WoodShovel, 42, true),
        (ToolKind::WoodShovel, 3, false),
        (ToolKind::TarBucket, 102, true),
        (ToolKind::TarBucket, 1, false),
        (ToolKind::TarBucket, terra_core::roads::WET_TAR, false),
    ] {
        for mode in [GameMode::Survival, GameMode::Creative, GameMode::Spectator] {
            let mut app = tests::app();
            tests::session(&mut app, mode, false);
            let item = ItemKind::Tool(tool);
            app.insert_resource(terra_world::edits::EditJournal::default())
                .insert_resource(terra_core::block_properties::BlockPropertiesCatalog::default())
                .insert_resource(super::super::ActiveItem(Some(item)))
                .insert_resource(terra_network::client::SelectedInventorySlot(Some(0)));
            let mut controls = terra_core::ControlsConfig::default();
            controls.bindings.insert(
                Action::Place,
                terra_core::InputBinding::Mouse(MouseButton::Right),
            );
            app.insert_resource(controls);
            let origin = Vec3::new(4.5, 6.0, 8.5);
            let direction = (Vec3::new(4.5, 4.0, 4.5) - origin).normalize();
            let rotation = Quat::from_euler(
                EulerRot::YXZ,
                (-direction.x).atan2(-direction.z),
                direction.y.asin(),
                0.0,
            );
            let mut look = crate::CameraLook::default();
            look.aim(rotation);
            app.world_mut().spawn((
                crate::TerraCamera,
                Transform::from_translation(origin),
                look,
            ));
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([4, 3, 4], material)
                .unwrap();
            app.world_mut()
                .resource_mut::<terra_network::client::ClientSession>()
                .inventory
                .insert(item, 1);
            let before = app
                .world()
                .resource::<terra_network::client::ClientSession>()
                .inventory
                .clone();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Right);
            app.update();
            let requests: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<FarmToolRequest>>()
                .drain()
                .collect();
            let should_request = expected && mode != GameMode::Spectator;
            assert_eq!(
                requests.len(),
                usize::from(should_request),
                "{tool:?} material{material} {mode:?}"
            );
            if let Some(request) = requests.first() {
                assert_eq!(request.position, [4, 3, 4]);
                assert_eq!(
                    request.action,
                    if tool == ToolKind::TarBucket {
                        FarmingAction::PourTar
                    } else {
                        FarmingAction::MakePath
                    }
                );
            }
            assert!(
                app.world()
                    .resource::<Messages<BlockEditRequest>>()
                    .is_empty()
            );
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at([4, 3, 4]),
                Some(material)
            );
            assert_eq!(
                app.world()
                    .resource::<terra_network::client::ClientSession>()
                    .inventory,
                before
            );
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
            app.update();
            assert!(
                app.world()
                    .resource::<Messages<FarmToolRequest>>()
                    .is_empty()
            );
        }
    }
}

#[test]
fn tar_bucket_uses_the_vessel_pour_pose_with_a_full_reserve() {
    let mut motion = super::super::VesselMotion::default();
    motion.begin_frame(
        Some(ItemKind::Tool(ToolKind::TarBucket)),
        (0, 0),
        true,
        true,
    );
    assert_eq!(motion.fill, 1.0);
    assert_eq!(motion.item, Some(ItemKind::Tool(ToolKind::TarBucket)));
    motion.pour([4, 3, 4]);
    assert!(motion.pouring);
    assert_eq!(motion.target, Some([4, 3, 4]));
}
