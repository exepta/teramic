use super::*;
use terra_core::{WoodSpecies, containers::ChestMerge};
use terra_world::edits::BlockObjectState;

fn chest_fixture(seam: bool, side: bool) -> (App, Peer, [i64; 3], [i64; 3]) {
    let (mut app, peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let old = [if seam { 15 } else { 7 }, 4, 8];
    let new = [old[0] + 1, 4, 8];
    let body = if side {
        [new[0] as f32 + 3.5, 5.0, 8.5]
    } else {
        [new[0] as f32 + 0.5, 6.0, 11.5]
    };
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::from_array(body);
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let section = terra_voxel::global_to_section(new).0;
        chunks.loaded.entry(section).or_insert_with(|| {
            let mut data = terra_voxel::BlockSection::empty(section).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    data.set_block([x, 3, z], 3).unwrap();
                }
            }
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            }
        });
        let (section, local) = terra_voxel::global_to_section(old);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 29)
            .unwrap();
    }
    let mut journal = app.world_mut().resource_mut::<EditJournal>();
    journal.record(old, 29).unwrap();
    journal
        .set_object_state(
            old,
            Some(BlockObjectState {
                wood: WoodSpecies::Spruce,
                ..Default::default()
            }),
        )
        .unwrap();
    let mut contents = Inventory::default();
    contents.insert(ItemKind::Block(3), 12);
    journal.set_container(old, contents).unwrap();
    let mut bag = Inventory::default();
    bag.insert(ItemKind::for_wood(29, WoodSpecies::Birch), 3);
    journal.gameplay_mut().inventories.insert(peer.uuid, bag);
    (app, peer, old, new)
}
fn command(app: &App, peer: &Peer, old: [i64; 3], new: [i64; 3], side: bool) -> EditCommand {
    let target = if side {
        [old[0] as f64 + 0.5, 4.5, 8.5]
    } else {
        [new[0] as f64 + 0.5, 4.0, 8.5]
    };
    EditCommand {
        request: 1,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(terra_voxel::global_to_section(new).0),
        look: aim(app, peer, target),
        source: Some(InventorySource {
            slot: 0,
            revision: app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
                .revision,
        }),
        action: EditAction::Block {
            position: new,
            expected: 0,
            material: 29,
        },
    }
}
fn apply_test(app: &mut App, peer: &Peer, command: EditCommand, merge: ChestMerge) -> EditOutcome {
    let mut state = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = state.get_mut(app.world_mut()).unwrap();
    apply_chest(command, peer, &mut world, merge)
}

#[test]
fn real_survival_chest_merge_preserves_old_contents_and_wood_and_uses_the_actual_held_new_chest() {
    for (side, merge) in [
        (false, ChestMerge::Auto),
        (true, ChestMerge::Target([7, 4, 8])),
    ] {
        let (mut app, peer, old, new) = chest_fixture(false, side);
        let old_bag = app
            .world()
            .resource::<EditJournal>()
            .container_at(old)
            .unwrap()
            .clone();
        let cmd = command(&app, &peer, old, new, side);
        assert_eq!(
            apply_test(&mut app, &peer, cmd, merge),
            EditOutcome::Applied
        );
        let journal = app.world().resource::<EditJournal>();
        let a = journal.object_state(old).unwrap();
        let b = journal.object_state(new).unwrap();
        assert_eq!((a.peer, b.peer), (Some(new), Some(old)));
        assert_eq!((a.facing, b.facing), (0, 0));
        assert_eq!((a.wood, b.wood), (WoodSpecies::Spruce, WoodSpecies::Birch));
        assert_eq!(journal.container_at(old), Some(&old_bag));
        assert_eq!(
            journal.gameplay().inventories[&peer.uuid]
                .count(ItemKind::for_wood(29, WoodSpecies::Birch)),
            2
        );
    }
}

#[test]
fn merging_across_a_foreign_claim_is_atomic_but_separate_placement_does_not_modify_the_foreign_chest()
 {
    for merge in [
        ChestMerge::Auto,
        ChestMerge::Target([15, 4, 8]),
        ChestMerge::Separate,
    ] {
        let side = matches!(merge, ChestMerge::Target(_));
        let (mut app, peer, old, new) = chest_fixture(true, side);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .kingdoms
            .found(PlayerUuid::new_v4(), [12, 4, 12], "Foreign")
            .unwrap();
        let before = app.world().resource::<EditJournal>().clone();
        let cmd = command(&app, &peer, old, new, side);
        let outcome = apply_test(&mut app, &peer, cmd, merge);
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.object_state(old), before.object_state(old));
        assert_eq!(journal.container_at(old), before.container_at(old));
        assert_eq!(
            journal.section_delta(terra_voxel::global_to_section(old).0),
            before.section_delta(terra_voxel::global_to_section(old).0)
        );
        if merge == ChestMerge::Separate {
            assert_eq!(outcome, EditOutcome::Applied);
            assert_eq!(journal.object_state(new).unwrap().peer, None);
            assert_eq!(
                journal.gameplay().inventories[&peer.uuid]
                    .count(ItemKind::for_wood(29, WoodSpecies::Birch)),
                2
            );
        } else {
            assert_eq!(outcome, EditOutcome::Invalid);
            assert_eq!(app.world().resource::<ChunkStore>().block_at(new), Some(0));
            assert_eq!(
                journal.gameplay().inventories[&peer.uuid],
                before.gameplay().inventories[&peer.uuid]
            );
            assert_eq!(
                journal.section_delta(terra_voxel::global_to_section(new).0),
                before.section_delta(terra_voxel::global_to_section(new).0)
            );
        }
    }
}

#[test]
fn forged_clicked_peer_stale_inventory_and_exhausted_old_peer_revision_never_partially_place_or_take_items()
 {
    for failure in 0..4 {
        let (mut app, peer, old, new) = chest_fixture(failure == 3, false);
        let mut cmd = command(&app, &peer, old, new, false);
        let merge = if failure == 0 {
            ChestMerge::Target(old)
        } else {
            ChestMerge::Auto
        };
        if failure == 1 {
            cmd.source.as_mut().unwrap().revision += 1;
        } else if failure == 2 {
            cmd.revision += 1;
        } else if failure == 3 {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            let mut delta = journal.section_delta(terra_voxel::global_to_section(old).0);
            delta.revision = u64::MAX;
            journal.import_delta(delta).unwrap();
        }
        let before = app.world().resource::<EditJournal>().clone();
        let result = apply_test(&mut app, &peer, cmd, merge);
        assert_eq!(
            result,
            if matches!(failure, 1 | 2) {
                EditOutcome::Conflict
            } else {
                EditOutcome::Invalid
            }
        );
        assert_eq!(app.world().resource::<ChunkStore>().block_at(new), Some(0));
        let journal = app.world().resource::<EditJournal>();
        for p in [old, new] {
            assert_eq!(
                journal.section_delta(terra_voxel::global_to_section(p).0),
                before.section_delta(terra_voxel::global_to_section(p).0)
            );
            assert_eq!(journal.container_at(p), before.container_at(p));
        }
        assert_eq!(
            journal.gameplay().inventories[&peer.uuid],
            before.gameplay().inventories[&peer.uuid]
        );
    }
}

#[test]
fn chest_specific_control_cannot_be_reused_to_place_other_materials() {
    let (mut app, peer, old, new) = chest_fixture(false, false);
    let mut cmd = command(&app, &peer, old, new, false);
    cmd.action = EditAction::Block {
        position: new,
        expected: 0,
        material: 3,
    };
    assert_eq!(
        apply_test(&mut app, &peer, cmd, ChestMerge::Separate),
        EditOutcome::Invalid
    );
    assert_eq!(app.world().resource::<ChunkStore>().block_at(new), Some(0));
}
