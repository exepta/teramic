use super::*;
use terra_core::{
    WoodSpecies, train_parts,
    trains::{TrainPart, Vehicle, VehicleKind},
};

fn vehicle(owner: PlayerUuid) -> Vehicle {
    let vehicle = Vehicle {
        name: String::new(),
        id: 1,
        owner,
        kind: VehicleKind::Passenger,
        anchor: [4, 4, 4],
        parts: [
            ([0, 0, 0], train_parts::CHASSIS),
            ([0, -1, 0], train_parts::BOGIE),
            ([0, -1, 1], train_parts::BOGIE),
            ([0, 1, 1], train_parts::FRAME),
            ([0, 1, 0], train_parts::SEAT),
            ([0, 0, -1], train_parts::COUPLER),
        ]
        .into_iter()
        .map(|(offset, material)| TrainPart {
            offset,
            material,
            facing: 0,
            wood: WoodSpecies::Oak,
        })
        .collect(),
        cargo: Default::default(),
        blocks: Default::default(),
        payloads: Default::default(),
        couplings: [None; 2],
        revision: 1,

        motion: Default::default(),
        burn_seconds: 0.0,
    };
    assert!(vehicle.valid());
    vehicle
}

#[test]
fn generic_block_placement_cannot_fill_saved_assembled_vehicle_air_cells_or_spend_items() {
    let (mut app, peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let vehicle = vehicle(peer.uuid);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .insert(vehicle)
        .unwrap();
    let source = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let bag = journal
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap();
        assert!(bag.creative_set(
            0,
            ItemStack {
                item: ItemKind::Block(3),
                count: 2
            }
        ));
        InventorySource {
            slot: 0,
            revision: bag.revision,
        }
    };
    let command = EditCommand {
        request: 1,
        source: Some(source),
        look: aim(&app, &peer, [4.5, 4.0, 4.5]),
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section([4, 4, 4]).0),
        action: EditAction::Block {
            position: [4, 4, 4],
            expected: 0,
            material: 3,
        },
    };
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert_ne!(check(command, &peer, &world), EditOutcome::Applied);
    assert_ne!(
        apply(command, &peer, &mut world, false),
        EditOutcome::Applied
    );
    assert_eq!(world.journal.gameplay(), &before);
    assert_eq!(world.chunks.block_at([4, 4, 4]), Some(0));
}

#[test]
fn shared_furniture_environment_checks_reserved_secondary_cells_before_any_mutation() {
    let (app, peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let mut journal = app.world().resource::<EditJournal>().clone();
    let mut vehicle = vehicle(peer.uuid);
    vehicle.anchor = [5, 4, 4];
    journal.gameplay_mut().trains.insert(vehicle).unwrap();
    let plan = terra_world::placement::Layout {
        edits: vec![
            BlockEditRequest {
                position: [4, 4, 4],
                expected_material: 0,
                material: 28,
            },
            BlockEditRequest {
                position: [5, 4, 4],
                expected_material: 0,
                material: 28,
            },
        ],
        states: vec![],
        belts: vec![],
    };
    assert!(!terra_world::trains::reserved(&journal, [4, 4, 4]));
    assert!(terra_world::trains::reserved(&journal, [5, 4, 4]));
    assert!(
        terra_world::placement::environment(
            &plan,
            app.world().resource::<ChunkStore>(),
            &journal,
            true
        )
        .is_err()
    );
    assert_eq!(journal.block([4, 4, 4]), None);
}
