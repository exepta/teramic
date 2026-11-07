use super::*;
use std::collections::BTreeSet;
use terra_core::rails::{self, Piece};
use terra_voxel::{BlockSection, SectionPos, global_to_section};

#[path = "server_rail_connections_tests.rs"]
mod connections;

fn set_block(app: &mut App, cell: [i64; 3], material: u16) {
    let (owner, local) = global_to_section(cell);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .entry(owner)
        .or_insert_with(|| LoadedSection {
            data: BlockSection::empty(owner).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        })
        .data
        .set_block(local, material)
        .unwrap();
}
fn rail_fixture(material: u16, anchor: [i64; 3]) -> (App, Peer, Piece, EditCommand) {
    let (mut app, peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let piece = Piece::new(anchor, material, 2).unwrap();
    for cell in terra_world::rails::cells(piece) {
        set_block(&mut app, cell, 0);
    }
    for support in piece.supports().iter() {
        set_block(
            &mut app,
            std::array::from_fn(|axis| anchor[axis] + i64::from(support.support[axis])),
            3,
        );
    }
    let eye = Vec3::new(
        anchor[0] as f32 + 0.5,
        anchor[1] as f32 + 2.0,
        anchor[2] as f32 + 3.5,
    );
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let look = aim(
        &app,
        &peer,
        [
            anchor[0] as f64 + 0.5,
            anchor[1] as f64,
            anchor[2] as f64 + 0.5,
        ],
    );
    let mut journal = app.world_mut().resource_mut::<EditJournal>();
    let bag = journal
        .gameplay_mut()
        .inventories
        .get_mut(&peer.uuid)
        .unwrap();
    assert!(bag.creative_set(
        0,
        ItemStack {
            item: ItemKind::Block(rails::STRAIGHT),
            count: terra_world::rails::piece_cost(piece) + 1
        }
    ));
    let source = Some(InventorySource {
        slot: 0,
        revision: bag.revision,
    });
    let command = EditCommand {
        request: 1,
        revision: journal.revision(global_to_section(anchor).0),
        look,
        source,
        action: EditAction::Block {
            position: anchor,
            expected: 0,
            material,
        },
    };
    (app, peer, piece, command)
}
fn run(app: &mut App, peer: &Peer, command: EditCommand, timed: bool) -> EditOutcome {
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    apply(command, peer, &mut world, timed)
}
#[derive(Debug, PartialEq)]
struct Snapshot {
    gameplay: terra_core::GameplayState,
    deltas: Vec<terra_world::edits::SectionDelta>,
    blocks: Vec<([i64; 3], Option<u16>)>,
    epochs: Vec<(SectionPos, u64, u64)>,
}
fn snapshot(app: &App, piece: Piece) -> Snapshot {
    let chunks = app.world().resource::<ChunkStore>();
    let journal = app.world().resource::<EditJournal>();
    let sections = chunks.loaded.keys().copied().collect::<BTreeSet<_>>();
    Snapshot {
        gameplay: journal.gameplay().clone(),
        deltas: sections
            .iter()
            .map(|section| journal.section_delta(*section))
            .collect(),
        blocks: terra_world::rails::cells(piece)
            .into_iter()
            .map(|cell| (cell, chunks.block_at(cell)))
            .collect(),
        epochs: sections
            .iter()
            .map(|section| {
                (
                    *section,
                    chunks.loaded[section].mesh_epoch,
                    chunks.loaded[section].water_epoch,
                )
            })
            .collect(),
    }
}
fn target_look(app: &App, peer: &Peer, cell: [i64; 3], material: u16) -> [f32; 2] {
    let journal = app.world().resource::<EditJournal>();
    let state = journal.object_state(cell).unwrap();
    for bounds in terra_world::rails::cell_boxes(cell, material, state).iter() {
        let target = std::array::from_fn(|axis| {
            cell[axis] as f64 + (bounds.minimum[axis] + bounds.maximum[axis]) * 0.5
        });
        let look = aim(app, peer, target);
        let eye = app.world().get::<Transform>(peer.body).unwrap().translation
            + Vec3::Y * PLAYER_EYE_OFFSET;
        let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
        let (_, hit) = terra_world::targeting::pick_world_with_context(
            app.world().resource::<ChunkStore>(),
            journal,
            app.world()
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
            eye.to_array().map(f64::from),
            direction.to_array().map(f64::from),
            PropPose::Rest,
        );
        if matches!(hit, Some(WorldTarget::Block(hit)) if hit.position == cell && hit.distance <= 4.5)
        {
            return look;
        }
    }
    panic!("rail slice must be genuinely visible: {cell:?}")
}
fn removal(app: &App, peer: &Peer, piece: Piece, cell: [i64; 3]) -> EditCommand {
    EditCommand {
        request: 2,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(cell).0),
        look: target_look(app, peer, cell, piece.material),
        source: None,
        action: EditAction::Block {
            position: cell,
            expected: piece.material,
            material: 0,
        },
    }
}

#[test]
fn survival_places_one_module_and_secondary_removal_returns_one_item() {
    let (mut app, peer, piece, command) = rail_fixture(rails::STRAIGHT, [8, 4, 10]);
    assert!(piece.footprint().len() > 1);
    assert_eq!(run(&mut app, &peer, command, false), EditOutcome::Applied);
    let journal = app.world().resource::<EditJournal>();
    assert!(terra_world::rails::complete(journal, piece));
    assert_eq!(
        journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(piece.material)),
        1
    );
    assert_eq!(journal.gameplay().player_stats[&peer.uuid].blocks_placed, 1);
    let secondary = [piece.anchor[0] + 1, piece.anchor[1], piece.anchor[2]];
    assert!(journal.object_state(secondary).unwrap().secondary);
    let remove = removal(&app, &peer, piece, secondary);
    let before = snapshot(&app, piece);
    assert_eq!(run(&mut app, &peer, remove, false), EditOutcome::Invalid);
    assert_eq!(
        snapshot(&app, piece),
        before,
        "survival mining must finish its timer"
    );
    assert_eq!(run(&mut app, &peer, remove, true), EditOutcome::Applied);
    let journal = app.world().resource::<EditJournal>();
    assert!(
        terra_world::rails::cells(piece)
            .into_iter()
            .all(|cell| journal.block(cell) == Some(0) && journal.object_state(cell).is_none())
    );
    assert_eq!(journal.gameplay().drops.len(), 1);
    assert_eq!(
        journal.gameplay().drops[0].stack,
        ItemStack {
            item: ItemKind::Block(piece.material),
            count: 1
        }
    );
    assert_eq!(journal.gameplay().player_stats[&peer.uuid].blocks_broken, 1);
}

#[test]
fn stale_sources_revisions_missing_items_and_overflow_publish_no_partial_module() {
    for failure in [
        "section",
        "source",
        "missing_source",
        "missing_item",
        "inventory_overflow",
        "section_overflow",
        "gameplay_overflow",
    ] {
        let (mut app, peer, piece, mut command) = rail_fixture(rails::STRAIGHT, [8, 4, 10]);
        match failure {
            "section" => command.revision += 1,
            "source" => command.source.as_mut().unwrap().revision += 1,
            "missing_source" => command.source = None,
            "missing_item" => {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .gameplay_mut()
                    .inventories
                    .get_mut(&peer.uuid)
                    .unwrap()
                    .slots
                    .clear();
            }
            "inventory_overflow" => {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .gameplay_mut()
                    .inventories
                    .get_mut(&peer.uuid)
                    .unwrap()
                    .revision = u64::MAX;
                command.source.as_mut().unwrap().revision = u64::MAX;
            }
            "section_overflow" => {
                let owner = global_to_section(piece.anchor).0;
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .import_delta(terra_world::edits::SectionDelta {
                        position: owner,
                        revision: u64::MAX,
                        ..Default::default()
                    })
                    .unwrap();
                command.revision = u64::MAX;
            }
            "gameplay_overflow" => {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .gameplay_mut()
                    .revision = u64::MAX
            }
            _ => unreachable!(),
        }
        let before = snapshot(&app, piece);
        assert_ne!(
            run(&mut app, &peer, command, false),
            EditOutcome::Applied,
            "{failure}"
        );
        assert_eq!(snapshot(&app, piece), before, "{failure}");
    }
}

#[test]
fn a_foreign_claim_under_a_secondary_cell_blocks_the_entire_placement() {
    let (mut app, peer, piece, command) = rail_fixture(rails::STRAIGHT, [15, 4, 10]);
    let stranger = PlayerUuid::from_u128(6202);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .kingdoms
        .found(stranger, [16, 10, 0], "Other bank")
        .unwrap();
    assert_ne!(
        terra_core::kingdom::claim_chunk(piece.anchor),
        terra_core::kingdom::claim_chunk([16, 4, 10])
    );
    let before = snapshot(&app, piece);
    assert_eq!(run(&mut app, &peer, command, false), EditOutcome::Invalid);
    assert_eq!(snapshot(&app, piece), before);
}

#[test]
fn full_drop_capacity_rejects_secondary_removal_before_any_cell_changes() {
    let (mut app, peer, piece, command) = rail_fixture(rails::STRAIGHT, [8, 4, 10]);
    assert_eq!(run(&mut app, &peer, command, false), EditOutcome::Applied);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        for _ in 0..terra_core::MAX_WORLD_DROPS {
            assert!(
                journal
                    .gameplay_mut()
                    .drop_one([100.0; 3], ItemKind::Block(3))
            );
        }
    }
    let remove = removal(&app, &peer, piece, [9, 4, 10]);
    let before = snapshot(&app, piece);
    assert_eq!(run(&mut app, &peer, remove, true), EditOutcome::RateLimited);
    assert_eq!(snapshot(&app, piece), before);
}

#[test]
fn switches_require_the_correct_opcode_facing_and_real_ray_then_update_every_slice() {
    let (mut app, peer, piece, command) = rail_fixture(rails::SWITCH_RIGHT, [8, 4, 10]);
    assert_eq!(run(&mut app, &peer, command, false), EditOutcome::Applied);
    let toggle = EditCommand {
        request: 2,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(piece.anchor).0),
        look: target_look(&app, &peer, piece.anchor, piece.material),
        source: None,
        action: EditAction::ToggleObject {
            position: piece.anchor,
            expected: piece.material,
            facing: 2,
            closing: false,
        },
    };
    for failure in ["closing", "facing", "ray"] {
        let mut invalid = toggle;
        match failure {
            "closing" => {
                if let EditAction::ToggleObject { closing, .. } = &mut invalid.action {
                    *closing = true;
                }
            }
            "facing" => {
                if let EditAction::ToggleObject { facing, .. } = &mut invalid.action {
                    *facing = 6;
                }
            }
            "ray" => invalid.look = [std::f32::consts::PI, 0.0],
            _ => unreachable!(),
        }
        let before = snapshot(&app, piece);
        assert_ne!(
            run(&mut app, &peer, invalid, false),
            EditOutcome::Applied,
            "{failure}"
        );
        assert_eq!(snapshot(&app, piece), before, "{failure}");
    }
    assert_eq!(run(&mut app, &peer, toggle, false), EditOutcome::Applied);
    let journal = app.world().resource::<EditJournal>();
    assert!(
        terra_world::rails::cells(piece)
            .into_iter()
            .all(|cell| journal.object_state(cell).unwrap().facing == 6)
    );
    assert!(terra_world::rails::complete(
        journal,
        Piece::new(piece.anchor, piece.material, 6).unwrap()
    ));
    let (mut plain, peer, piece, command) = rail_fixture(rails::STRAIGHT, [8, 4, 10]);
    assert_eq!(run(&mut plain, &peer, command, false), EditOutcome::Applied);
    let invalid = EditCommand {
        request: 2,
        revision: plain
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(piece.anchor).0),
        look: target_look(&plain, &peer, piece.anchor, piece.material),
        source: None,
        action: EditAction::ToggleObject {
            position: piece.anchor,
            expected: piece.material,
            facing: 2,
            closing: false,
        },
    };
    let before = snapshot(&plain, piece);
    assert_eq!(run(&mut plain, &peer, invalid, false), EditOutcome::Invalid);
    assert_eq!(snapshot(&plain, piece), before);
}

#[test]
fn ordinary_block_mining_cannot_remove_a_real_rail_support() {
    let (mut app, peer, piece, command) = rail_fixture(rails::STRAIGHT, [8, 4, 10]);
    assert_eq!(run(&mut app, &peer, command, false), EditOutcome::Applied);
    let support = [8, 3, 10];
    let eye = Vec3::new(9.0, 2.0, 10.5);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let look = aim(&app, &peer, [8.5, 3.0, 10.5]);
    let command = EditCommand {
        request: 2,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(support).0),
        look,
        source: None,
        action: EditAction::Block {
            position: support,
            expected: 3,
            material: 0,
        },
    };
    assert!(!terra_world::rails::support_edit_allowed(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        support,
        0
    ));
    let before = snapshot(&app, piece);
    assert_eq!(run(&mut app, &peer, command, true), EditOutcome::Invalid);
    assert_eq!(snapshot(&app, piece), before);
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at(support),
        Some(3)
    );
}

fn fill_stats(app: &mut App, peer: &Peer, include_actor: bool) {
    let mut journal = app.world_mut().resource_mut::<EditJournal>();
    let stats = &mut journal.gameplay_mut().player_stats;
    stats.clear();
    if include_actor {
        stats.insert(peer.uuid, Default::default());
    }
    for serial in 1_u128..=4097 {
        if stats.len() == 4096 {
            break;
        }
        let uuid = PlayerUuid::from_u128(62_000 + serial);
        if uuid != peer.uuid {
            stats.insert(uuid, Default::default());
        }
    }
    assert_eq!(stats.len(), 4096);
    assert_eq!(stats.contains_key(&peer.uuid), include_actor);
    assert!(journal.gameplay().valid());
}

#[test]
fn stats_capacity_is_reserved_before_placement_or_removal_and_existing_stats_remain_usable() {
    let (mut app, peer, piece, command) = rail_fixture(rails::STRAIGHT, [8, 4, 10]);
    fill_stats(&mut app, &peer, false);
    let before = snapshot(&app, piece);
    assert_eq!(
        run(&mut app, &peer, command, false),
        EditOutcome::RateLimited
    );
    assert_eq!(snapshot(&app, piece), before);
    assert!(app.world().resource::<EditJournal>().gameplay().valid());

    // A full table still permits updating an actor's existing saved entry.
    fill_stats(&mut app, &peer, true);
    assert_eq!(run(&mut app, &peer, command, false), EditOutcome::Applied);
    let remove = removal(&app, &peer, piece, [9, 4, 10]);
    fill_stats(&mut app, &peer, false);
    let before = snapshot(&app, piece);
    assert_eq!(run(&mut app, &peer, remove, true), EditOutcome::RateLimited);
    assert_eq!(snapshot(&app, piece), before);
    assert!(app.world().resource::<EditJournal>().gameplay().valid());

    fill_stats(&mut app, &peer, true);
    assert_eq!(run(&mut app, &peer, remove, true), EditOutcome::Applied);
    let gameplay = app.world().resource::<EditJournal>().gameplay();
    assert_eq!(gameplay.player_stats.len(), 4096);
    assert_eq!(gameplay.player_stats[&peer.uuid].blocks_broken, 1);
    assert_eq!(gameplay.drops.len(), 1);
    assert!(gameplay.valid());
}

#[test]
fn a_switch_does_not_allocate_stats_when_the_valid_save_table_is_full() {
    let (mut app, peer, piece, command) = rail_fixture(rails::SWITCH_RIGHT, [8, 4, 10]);
    assert_eq!(run(&mut app, &peer, command, false), EditOutcome::Applied);
    fill_stats(&mut app, &peer, false);
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    let toggle = EditCommand {
        request: 2,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(piece.anchor).0),
        look: target_look(&app, &peer, piece.anchor, piece.material),
        source: None,
        action: EditAction::ToggleObject {
            position: piece.anchor,
            expected: piece.material,
            facing: piece.encoded_facing(),
            closing: false,
        },
    };
    assert_eq!(run(&mut app, &peer, toggle, false), EditOutcome::Applied);
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(journal.gameplay(), &before);
    assert!(journal.gameplay().valid());
    assert!(
        terra_world::rails::cells(piece)
            .into_iter()
            .all(|cell| journal.object_state(cell).unwrap().facing == 6)
    );
}
