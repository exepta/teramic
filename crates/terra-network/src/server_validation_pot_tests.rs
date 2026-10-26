use super::*;

fn pot_fixture() -> (App, Peer, [i64; 3]) {
    let (mut app, peer, support) = fixture(ResourcePropKind::Flower, false);
    let position = [support[0], support[1] + 1, support[2]];
    let (owner, local) = terra_voxel::global_to_section(position);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, terra_core::flower_pot::MATERIAL)
        .unwrap();
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal
            .record(position, terra_core::flower_pot::MATERIAL)
            .unwrap();
        journal
            .set_object_state(position, Some(Default::default()))
            .unwrap();
        let mut inventory = Inventory::default();
        inventory.insert(ItemKind::Flower(terra_core::beds::DyeColor::Red), 2);
        journal
            .gameplay_mut()
            .inventories
            .insert(peer.uuid, inventory);
    }
    (app, peer, position)
}

fn command(app: &App, peer: &Peer, position: [i64; 3]) -> EditCommand {
    let journal = app.world().resource::<EditJournal>();
    EditCommand {
        request: 1,
        revision: journal.revision(terra_voxel::global_to_section(position).0),
        look: aim(app, peer, position.map(|v| v as f64 + 0.35)),
        action: EditAction::ToggleObject {
            position,
            expected: terra_core::flower_pot::MATERIAL,
            facing: journal.object_state(position).unwrap().facing,
        },
        source: peer.held_slot.map(|slot| InventorySource {
            slot,
            revision: journal.gameplay().inventories[&peer.uuid].revision,
        }),
    }
}

#[test]
fn empty_hand_on_an_empty_pot_is_an_acknowledged_noop_without_revision_or_inventory_changes() {
    for held in [Some(1), None] {
        let (mut app, mut peer, position) = pot_fixture();
        peer.held_slot = held;
        let request = command(&app, &peer, position);
        let owner = terra_voxel::global_to_section(position).0;
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let before = world.journal.section_delta(owner);
        let gameplay = world.journal.gameplay().clone();
        let mesh_epoch = world.journal.mesh_epoch(owner);
        let water_epoch = world.journal.water_mesh_epoch(owner);
        for _ in 0..2 {
            assert_eq!(
                apply(request, &peer, &mut world, false),
                EditOutcome::Applied
            );
            assert_eq!(world.journal.section_delta(owner), before);
            assert_eq!(world.journal.gameplay(), &gameplay);
            assert_eq!(world.journal.mesh_epoch(owner), mesh_epoch);
            assert_eq!(world.journal.water_mesh_epoch(owner), water_epoch);
        }
        let mut invalid_source = request;
        invalid_source.source = Some(InventorySource {
            slot: if held == Some(1) { 0 } else { 1 },
            revision: gameplay.inventories[&peer.uuid].revision,
        });
        assert_eq!(
            apply(invalid_source, &peer, &mut world, false),
            EditOutcome::Invalid
        );
        assert_eq!(world.journal.gameplay(), &gameplay);
        let mut stale = request;
        stale.revision += 1;
        assert_eq!(
            apply(stale, &peer, &mut world, false),
            EditOutcome::Conflict
        );
    }
}

#[test]
fn flower_pot_uses_authoritative_ray_inventory_revision_and_exact_once_take_return_and_mining_drops()
 {
    let (mut app, mut peer, position) = pot_fixture();
    let insert = command(&app, &peer, position);
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(
            apply(insert, &peer, &mut world, false),
            EditOutcome::Applied
        );
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid]
                .count(ItemKind::Flower(terra_core::beds::DyeColor::Red)),
            1
        );
        assert_eq!(
            apply(insert, &peer, &mut world, false),
            EditOutcome::Conflict
        );
        assert!(
            world
                .chunks
                .loaded
                .values()
                .all(|section| section.props.is_empty())
        );
    }
    peer.held_slot = Some(1);
    let remove = command(&app, &peer, position);
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(
            apply(remove, &peer, &mut world, false),
            EditOutcome::Applied
        );
        assert_eq!(world.journal.object_state(position).unwrap().facing, 0);
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid]
                .count(ItemKind::Flower(terra_core::beds::DyeColor::Red)),
            2
        );
    }
    peer.held_slot = Some(0);
    let insert = command(&app, &peer, position);
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(
            apply(insert, &peer, &mut world, false),
            EditOutcome::Applied
        );
    }
    let mut mine = command(&app, &peer, position);
    mine.action = EditAction::Block {
        position,
        expected: 131,
        material: 0,
    };
    mine.source = None;
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert_eq!(apply(mine, &peer, &mut world, true), EditOutcome::Applied);
    assert_eq!(world.chunks.block_at(position), Some(0));
    assert_eq!(world.journal.gameplay().drops.len(), 2);
    assert!(
        world
            .journal
            .gameplay()
            .drops
            .iter()
            .any(|drop| drop.stack.item == ItemKind::Block(131) && drop.stack.count == 1)
    );
    assert!(
        world
            .journal
            .gameplay()
            .drops
            .iter()
            .any(
                |drop| drop.stack.item == ItemKind::Flower(terra_core::beds::DyeColor::Red)
                    && drop.stack.count == 1
            )
    );
    assert_eq!(apply(mine, &peer, &mut world, true), EditOutcome::Conflict);
    assert_eq!(world.journal.gameplay().drops.len(), 2);
}

#[test]
fn flower_pot_cannot_be_changed_through_walls_without_build_permission_or_with_stale_held_items() {
    let (mut app, mut peer, position) = pot_fixture();
    let valid = command(&app, &peer, position);
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(check(valid, &peer, &world), EditOutcome::Applied);
        let mut stale = valid;
        stale.source.as_mut().unwrap().revision += 1;
        assert_eq!(apply(stale, &peer, &mut world, false), EditOutcome::Invalid);
        assert_eq!(world.journal.object_state(position).unwrap().facing, 0);
        peer.permissions.build = false;
        assert_eq!(check(valid, &peer, &world), EditOutcome::Invalid);
        peer.permissions.build = true;
        world
            .chunks
            .loaded
            .get_mut(&terra_voxel::global_to_section(position).0)
            .unwrap()
            .data
            .set_block([4, 5, 6], 3)
            .unwrap();
        assert_eq!(check(valid, &peer, &world), EditOutcome::Occluded);
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid]
                .count(ItemKind::Flower(terra_core::beds::DyeColor::Red)),
            2
        );
    }
}
