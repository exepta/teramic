use super::*;
use terra_core::{ToolKind, farming::FarmingAction, roads};

const P: [i64; 3] = [4, 3, 4];

fn setup(material: u16, tool: ToolKind) -> (App, Peer, EditCommand) {
    let (mut app, peer, _) = fixture(ResourcePropKind::SmallStones, false);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&terra_voxel::SectionPos::default())
        .unwrap()
        .data
        .set_block([4, 3, 4], material)
        .unwrap();
    equip_farm_tool(&mut app, &peer, tool);
    let action = if tool == ToolKind::TarBucket {
        FarmingAction::PourTar
    } else {
        FarmingAction::MakePath
    };
    let look = aim(&app, &peer, [4.5, 4.0, 4.5]);
    let command = farm_command(&app, &peer, P, action, look);
    (app, peer, command)
}

fn apply_farm(app: &mut App, peer: &mut Peer, command: EditCommand) -> EditOutcome {
    let mut sender = lightyear::prelude::MessageSender::<ServerControl>::default();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    super::super::super::farm_tools::apply(command, peer, &mut sender, &mut world)
}

#[test]
fn shovels_prepare_all_grass_sand_gravel_and_earth_variants_without_consuming_the_tool() {
    for material in [1, 2, 4, 42, 306, 307, 400, 401] {
        for tool in [ToolKind::WoodShovel, ToolKind::StoneShovel] {
            let (mut app, mut peer, command) = setup(material, tool);
            let before =
                app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].clone();
            assert_eq!(
                apply_farm(&mut app, &mut peer, command),
                EditOutcome::Applied,
                "{material}, {tool:?}"
            );
            assert_eq!(app.world().resource::<ChunkStore>().block_at(P), Some(102));
            assert_eq!(
                app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid],
                before
            );
            peer.farming.reset_cooldown();
            assert_eq!(
                apply_farm(&mut app, &mut peer, command),
                EditOutcome::Conflict
            );
        }
    }
}

#[test]
fn tar_bucket_coats_only_prepared_paths_returns_one_empty_bucket_and_cannot_repeat() {
    let (mut app, mut peer, command) = setup(102, ToolKind::TarBucket);
    assert_eq!(
        apply_farm(&mut app, &mut peer, command),
        EditOutcome::Applied
    );
    let journal = app.world().resource::<EditJournal>();
    let bag = &journal.gameplay().inventories[&peer.uuid];
    assert_eq!(bag.count(ItemKind::Tool(ToolKind::TarBucket)), 0);
    assert_eq!(bag.count(ItemKind::Tool(ToolKind::Bucket)), 1);
    assert_eq!(
        roads::tar_amount(journal.object_state(P).unwrap().facing),
        8
    );
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at(P),
        Some(roads::WET_TAR)
    );
    let mut replica = EditJournal::default();
    replica
        .import_delta(journal.section_delta(terra_voxel::SectionPos::default()))
        .unwrap();
    assert_eq!(replica.object_state(P), journal.object_state(P));
    peer.farming.reset_cooldown();
    assert_eq!(
        apply_farm(&mut app, &mut peer, command),
        EditOutcome::Conflict
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
            .count(ItemKind::Tool(ToolKind::Bucket)),
        1
    );
}

#[test]
fn road_actions_reject_wrong_soil_obstruction_side_hits_permissions_and_stale_inventory() {
    for pouring in [false, true] {
        let tool = if pouring {
            ToolKind::TarBucket
        } else {
            ToolKind::WoodShovel
        };
        let valid = if pouring { 102 } else { 1 };
        for failure in 0..8 {
            let material = if failure == 0 { 3 } else { valid };
            let (mut app, mut peer, mut command) = setup(material, tool);
            match failure {
                1 => {
                    command.source.as_mut().unwrap().revision += 1;
                }
                2 => {
                    command.revision += 1;
                }
                3 => {
                    peer.permissions.build = false;
                }
                4 => {
                    app.world_mut()
                        .resource_mut::<ChunkStore>()
                        .loaded
                        .get_mut(&terra_voxel::SectionPos::default())
                        .unwrap()
                        .data
                        .set_block([4, 4, 4], 3)
                        .unwrap();
                }
                5 => {
                    app.world_mut()
                        .get_mut::<Transform>(peer.body)
                        .unwrap()
                        .translation = Vec3::new(4.5, 3.5 - PLAYER_EYE_OFFSET, 8.5);
                    command.look = aim(&app, &peer, [4.5, 3.5, 4.5]);
                }
                6 => {
                    command.action = EditAction::Farm {
                        position: P,
                        action: if pouring {
                            FarmingAction::MakePath
                        } else {
                            FarmingAction::PourTar
                        },
                    };
                }
                7 => {
                    peer.held_slot = None;
                }
                _ => {}
            }
            let before =
                app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].clone();
            assert_ne!(
                apply_farm(&mut app, &mut peer, command),
                EditOutcome::Applied,
                "pour={pouring}, failure={failure}"
            );
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(P),
                Some(material)
            );
            assert_eq!(
                app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid],
                before
            );
        }
    }
}

#[test]
fn creative_tar_preserves_the_filled_bucket_and_other_players_cannot_be_embedded() {
    let (mut app, mut peer, command) = setup(102, ToolKind::TarBucket);
    peer.mode = Some(GameMode::Creative);
    assert_eq!(
        apply_farm(&mut app, &mut peer, command),
        EditOutcome::Applied
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
            .count(ItemKind::Tool(ToolKind::TarBucket)),
        1
    );
    let (mut app, mut peer, command) = setup(102, ToolKind::TarBucket);
    app.world_mut().spawn((
        Transform::from_xyz(4.5, 4.5, 4.5),
        crate::server::ServerCharacter {
            link: Entity::PLACEHOLDER,
        },
    ));
    assert_eq!(
        apply_farm(&mut app, &mut peer, command),
        EditOutcome::Invalid
    );
    assert_eq!(app.world().resource::<ChunkStore>().block_at(P), Some(102));
}

#[test]
fn coated_paths_mine_to_path_without_internal_items_and_direct_tar_blocks_are_rejected() {
    for material in [roads::WET_TAR, roads::DRIED_TAR] {
        let drops = resource_stacks(
            EditAction::Block {
                position: P,
                expected: material,
                material: 0,
            },
            &ChunkStore::default(),
            &EditJournal::default(),
        );
        assert_eq!(
            drops,
            vec![(
                P.map(|v| v as f64 + 0.5),
                ItemStack {
                    item: ItemKind::Block(102),
                    count: 1
                }
            )]
        );
        let (mut app, peer, mut command) = setup(102, ToolKind::TarBucket);
        command.action = EditAction::Block {
            position: [4, 4, 4],
            expected: 0,
            material,
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(check(command, &peer, &world), EditOutcome::Invalid);
    }
}

#[test]
fn bile_bucket_authority_spills_collects_one_source_and_rejects_stale_transactions() {
    let (mut app, mut peer, _) = setup(3, ToolKind::BileBucket);
    let cell = [4, 4, 4];
    let look = aim(&app, &peer, [4.5, 4.0, 4.5]);
    let pour = farm_command(&app, &peer, cell, FarmingAction::EmptyBileBucket, look);
    assert_eq!(apply_farm(&mut app, &mut peer, pour), EditOutcome::Applied);
    let journal = app.world().resource::<EditJournal>();
    assert!(journal.object_state(cell).unwrap().liquid.unwrap().source);
    assert_eq!(
        journal.gameplay().inventories[&peer.uuid].count(ItemKind::Tool(ToolKind::Bucket)),
        1
    );
    peer.farming.reset_cooldown();
    assert_eq!(apply_farm(&mut app, &mut peer, pour), EditOutcome::Conflict);
    let look = aim(&app, &peer, [4.5, 4.5, 4.5]);
    let collect = farm_command(&app, &peer, cell, FarmingAction::FillBileBucket, look);
    assert_eq!(
        apply_farm(&mut app, &mut peer, collect),
        EditOutcome::Applied
    );
    assert_eq!(app.world().resource::<ChunkStore>().block_at(cell), Some(0));
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
            .count(ItemKind::Tool(ToolKind::BileBucket)),
        1
    );
}
