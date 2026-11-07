use super::*;
use terra_core::ToolKind;
use terra_world::edits::BlockObjectState;

fn setup() -> (App, Peer, EditCommand) {
    let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
    equip_farm_tool(&mut app, &peer, ToolKind::Wrench);
    let position = [4, 4, 4];
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&terra_voxel::SectionPos::default())
        .unwrap()
        .data
        .set_block([4, 4, 4], 145)
        .unwrap();
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.record(position, 145).unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
    }
    let eye = Vec3::new(4.5, 4.55, 8.0);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let direction = (Vec3::new(4.5, 4.5, 4.5) - eye).normalize();
    let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
    peer.look = look;
    let journal = app.world().resource::<EditJournal>();
    let command = EditCommand {
        request: 1,
        look,
        revision: journal.revision(terra_voxel::SectionPos::default()),
        action: EditAction::RotateObject {
            position,
            expected: 145,
            facing: 0,
            reverse: false,
        },
        source: Some(InventorySource {
            slot: 0,
            revision: journal.gameplay().inventories[&peer.uuid].revision,
        }),
    };
    (app, peer, command)
}

#[test]
fn authoritative_wrench_rotates_with_real_pick_and_preserves_inventory() {
    let (mut app, peer, command) = setup();
    let inventory =
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].clone();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert_eq!(check(command, &peer, &world), EditOutcome::Applied);
    assert_eq!(
        apply(command, &peer, &mut world, false),
        EditOutcome::Applied
    );
    assert_ne!(world.journal.object_state([4, 4, 4]).unwrap().facing, 0);
    assert_eq!(world.chunks.block_at([4, 4, 4]), Some(145));
    assert_eq!(world.journal.gameplay().inventories[&peer.uuid], inventory);
    assert!(world.journal.gameplay().drops.is_empty());
    assert_eq!(
        apply(command, &peer, &mut world, false),
        EditOutcome::Conflict
    );
}

#[test]
fn wrench_cannot_bypass_permissions_item_revision_spectator_or_occlusion() {
    for invalid in 0..7 {
        let (mut app, mut peer, mut command) = setup();
        match invalid {
            0 => peer.permissions.build = false,
            1 => peer.mode = Some(GameMode::Spectator),
            2 => command.source = None,
            3 => command.source.as_mut().unwrap().revision += 1,
            4 => command.look[0] = f32::NAN,
            5 => peer.held_slot = Some(1),
            _ => {
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .get_mut(&terra_voxel::SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([4, 4, 6], 3)
                    .unwrap();
            }
        }
        let before = app
            .world()
            .resource::<EditJournal>()
            .section_delta(terra_voxel::SectionPos::default());
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_ne!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Applied,
            "case {invalid}"
        );
        assert_eq!(
            world
                .journal
                .section_delta(terra_voxel::SectionPos::default()),
            before
        );
    }
}
