use super::*;
use terra_core::WoodSpecies;
use terra_world::{conveyors, edits::BlockObjectState};

fn place_shaft(app: &mut App, position: [i64; 3]) {
    let (section, local) = global_to_section(position);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 144)
        .unwrap();
    let mut journal = app.world_mut().resource_mut::<EditJournal>();
    journal.record(position, 144).unwrap();
    journal
        .set_object_state(
            position,
            Some(BlockObjectState {
                wood: WoodSpecies::Spruce,
                ..Default::default()
            }),
        )
        .unwrap();
}

#[test]
fn authority_connects_first_and_last_of_four_adjacent_shafts_then_returns_belt_and_mined_shaft() {
    let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let cells = [[7, 6, 5], [7, 6, 6], [7, 6, 7], [7, 6, 8]];
    for position in cells {
        place_shaft(&mut app, position);
    }
    let item = ItemKind::TintedBlock {
        material: 148,
        wood: WoodSpecies::Cherry,
        color: [71, 93, 184],
    };
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .get_mut(&peer.uuid)
        .unwrap()
        .creative_set(0, ItemStack { item, count: 1 });
    peer.look = [0.0; 2];
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(7.5, 6.5, 11.5) - Vec3::Y * PLAYER_EYE_OFFSET;
    let journal = app.world().resource::<EditJournal>();
    let revision = journal.revision(global_to_section(cells[0]).0);
    let command = EditCommand {
        request: 1,
        revision,
        look: peer.look,
        source: Some(InventorySource {
            slot: 0,
            revision: journal.gameplay().inventories[&peer.uuid].revision,
        }),
        action: EditAction::ConnectConveyor {
            start: cells[0],
            end: cells[3],
            start_revision: revision,
            item,
        },
    };
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(check(command, &peer, &world), EditOutcome::Applied);
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Applied
        );
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(item),
            0
        );
        let span = conveyors::span_at(&world.journal, cells[1]).unwrap();
        assert_eq!((span.start, span.end, span.length), (cells[0], cells[3], 2));
        assert_eq!(conveyors::item(span, &world.journal), item);
        for position in cells {
            assert_eq!(conveyors::span_at(&world.journal, position), Some(span));
        }
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Conflict
        );
    }
    let shaft_item = ItemKind::for_wood(144, WoodSpecies::Spruce);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .get_mut(&peer.uuid)
        .unwrap()
        .creative_set(
            0,
            ItemStack {
                item: ItemKind::Tool(terra_core::ToolKind::IronPickaxe),
                count: 1,
            },
        );
    peer.look = [std::f32::consts::FRAC_PI_2, 0.0];
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(10.5, 6.5, 6.5) - Vec3::Y * PLAYER_EYE_OFFSET;
    let command = EditCommand {
        request: 2,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(cells[1]).0),
        look: peer.look,
        source: None,
        action: EditAction::Block {
            position: cells[1],
            expected: 144,
            material: 0,
        },
    };
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert_eq!(
        apply(command, &peer, &mut world, true),
        EditOutcome::Applied
    );
    for expected in [item, shaft_item] {
        assert_eq!(
            world
                .journal
                .gameplay()
                .drops
                .iter()
                .filter(|drop| drop.stack.item == expected)
                .map(|drop| drop.stack.count)
                .sum::<u16>(),
            1
        );
    }
    assert_eq!(world.chunks.block_at(cells[1]), Some(0));
    for position in [cells[0], cells[2], cells[3]] {
        assert_eq!(world.chunks.block_at(position), Some(144));
        assert_eq!(world.journal.object_state(position).unwrap().peer, None);
        assert_eq!(
            world.journal.object_state(position).unwrap().wood,
            WoodSpecies::Spruce
        );
    }
}

#[test]
fn authority_inserts_shafts_at_controller_and_last_remaining_ribbon_without_losing_owned_belt() {
    let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let start = [7, 6, 5];
    let end = [7, 6, 8];
    for position in [start, end] {
        place_shaft(&mut app, position);
    }
    let item = ItemKind::TintedBlock {
        material: 148,
        wood: WoodSpecies::Cherry,
        color: [71, 93, 184],
    };
    let span = {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let plan = conveyors::layout(&world.chunks, &world.journal, start, end, item).unwrap();
        let span = conveyors::proposed_span(&plan).unwrap();
        assert!(conveyors::commit(&mut world.chunks, &mut world.journal, &plan).unwrap());
        span
    };
    let shaft_item = ItemKind::for_wood(144, WoodSpecies::Birch);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .get_mut(&peer.uuid)
        .unwrap()
        .creative_set(
            0,
            ItemStack {
                item: shaft_item,
                count: 2,
            },
        );
    for (index, position) in span.cells().enumerate() {
        peer.look = [0.0, -1.4];
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation =
            Vec3::new(7.5, 8.5, position[2] as f32 + 0.5) - Vec3::Y * PLAYER_EYE_OFFSET;
        let journal = app.world().resource::<EditJournal>();
        let command = EditCommand {
            request: index as u64 + 1,
            revision: journal.revision(global_to_section(position).0),
            look: peer.look,
            source: Some(InventorySource {
                slot: 0,
                revision: journal.gameplay().inventories[&peer.uuid].revision,
            }),
            action: EditAction::Block {
                position,
                expected: 148,
                material: 144,
            },
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        peer.permissions.build = false;
        assert_eq!(check(command, &peer, &world), EditOutcome::Invalid);
        peer.permissions.build = true;
        assert_eq!(check(command, &peer, &world), EditOutcome::Applied);
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Applied
        );
        assert_eq!(world.chunks.block_at(position), Some(144));
        assert_eq!(
            world.journal.object_state(position).unwrap().wood,
            WoodSpecies::Birch
        );
        assert_eq!(conveyors::span_at(&world.journal, position), Some(span));
        assert_eq!(conveyors::item(span, &world.journal), item);
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(shaft_item),
            1 - index as u16
        );
        assert!(world.journal.gameplay().drops.is_empty());
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Conflict
        );
    }
}

#[test]
fn mining_a_ribbon_without_local_tint_returns_the_owner_item_after_controller_shaft_insertion() {
    let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let start = [7, 6, 5];
    let end = [7, 6, 9];
    for position in [start, end] {
        place_shaft(&mut app, position);
    }
    let item = ItemKind::TintedBlock {
        material: 148,
        wood: WoodSpecies::Cherry,
        color: [17, 95, 213],
    };
    let span = {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let plan = conveyors::layout(&world.chunks, &world.journal, start, end, item).unwrap();
        let span = conveyors::proposed_span(&plan).unwrap();
        assert!(conveyors::commit(&mut world.chunks, &mut world.journal, &plan).unwrap());
        let insertion = conveyors::insert_shaft_layout(
            &world.chunks,
            &world.journal,
            span.controller,
            ItemKind::for_wood(144, WoodSpecies::Birch),
        )
        .unwrap();
        assert!(conveyors::commit(&mut world.chunks, &mut world.journal, &insertion).unwrap());
        span
    };
    let target = [7, 6, 7];
    assert_eq!(app.world().resource::<EditJournal>().tint_at(target), None);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .get_mut(&peer.uuid)
        .unwrap()
        .creative_set(
            0,
            ItemStack {
                item: ItemKind::Tool(terra_core::ToolKind::IronPickaxe),
                count: 1,
            },
        );
    peer.look = [0.0, -1.4];
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(7.5, 8.5, 7.5) - Vec3::Y * PLAYER_EYE_OFFSET;
    let command = EditCommand {
        request: 1,
        source: None,
        look: peer.look,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(target).0),
        action: EditAction::Block {
            position: target,
            expected: 148,
            material: 0,
        },
    };
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert_eq!(
        apply(command, &peer, &mut world, true),
        EditOutcome::Applied
    );
    let drops = &world.journal.gameplay().drops;
    assert_eq!(drops.len(), 1);
    assert_eq!(drops[0].stack, ItemStack { item, count: 1 });
    assert_eq!(world.chunks.block_at(span.controller), Some(144));
    assert_eq!(
        world.journal.object_state(span.controller).unwrap().wood,
        WoodSpecies::Birch
    );
    assert_eq!(
        world.journal.object_state(span.controller).unwrap().peer,
        None
    );
    assert!(world.journal.belt_owner(span.controller).is_none());
}
