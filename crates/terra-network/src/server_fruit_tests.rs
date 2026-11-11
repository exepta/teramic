use super::*;
use bevy::ecs::system::SystemState;
use terra_core::{
    FruitState, Inventory, ItemStack, TreeRecord,
    kingdom::{ClaimActor, ClaimKind, KingdomRole},
};
use terra_voxel::{BlockSection, SectionPos, global_to_section};
use terra_world::{
    props::WorldProp,
    streaming::{LoadedSection, WorldTerrain},
};

fn fixture() -> (App, Peer, EditCommand) {
    let mut app = App::new();
    let config = GameConfig::default();
    app.insert_resource(WorldTerrain(
        terra_world::TerraGenerator::new(config.world.clone()).unwrap(),
    ))
    .insert_resource(terra_core::WorldClock {
        day: 11,
        ..Default::default()
    })
    .init_resource::<terra_core::WorldWeather>()
    .insert_resource(config)
    .init_resource::<terra_core::crafting::RecipeBook>()
    .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
    .init_resource::<ChunkStore>()
    .init_resource::<EditJournal>()
    .init_resource::<ConnectedProfiles>()
    .init_resource::<riding::Mounts>();
    for x in 0..=1 {
        let position = SectionPos::new(x, 0, 0);
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data: BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
    }
    for (position, material) in [([14, 1, 8], 6), ([16, 4, 8], 195)] {
        let (section, local) = global_to_section(position);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, material)
            .unwrap();
    }
    let prop = WorldProp {
        id: terra_world::fruit::prop_id(17, 0),
        kind: PropKind::AppleFruit,
        position: [16.5, 3.575, 8.5],
        rotation: 0.,
        scale: 1.,
        variant: 0,
        section: SectionPos::new(1, 0, 0),
        removed: false,
    };
    assert!(prop.valid());
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&prop.section)
        .unwrap()
        .props
        .push(prop);
    let actor = terra_core::PlayerUuid::new_v4();
    let tree = TreeRecord {
        id: 17,
        root: [14, 1, 8],
        species: TreeSpecies::Apple,
        seed: 17,
        planted_by: None,
        stage: TreeStage::Mature,
        growth_seconds: 0.,
        wood: vec![],
        leaves: vec![],
        bounds: None,
        fruit: Some(FruitState {
            phase: FruitPhase::Ripe,
            remaining: 8,
            picked: 0,
            cycle_day: 0,
            last_harvest_day: None,
            revision: 4,
        }),
        revision: 4,
    };
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        assert!(journal.gameplay_mut().trees.insert(tree));
        journal
            .gameplay_mut()
            .inventories
            .insert(actor, Inventory::default());
    }
    let body = app
        .world_mut()
        .spawn((
            ServerCharacter {
                link: Entity::PLACEHOLDER,
            },
            Transform::from_xyz(
                16.5,
                3.775 - terra_physics::character::PLAYER_EYE_OFFSET,
                11.5,
            ),
        ))
        .id();
    let now = Instant::now();
    let peer = Peer {
        farming: default(),
        crank: default(),
        chest_cursor: None,
        creature_action_at: now,
        container: None,
        held_slot: None,
        mode: Some(terra_core::GameMode::Creative),
        generation: 0,
        chat: default(),
        uuid: actor,
        gender: default(),
        appearance: Default::default(),
        mining: None,
        consumption: None,
        gameplay_sent: None,
        permissions: terra_core::PlayerPermissions {
            build: true,
            game_mode: false,
            flight: false,
            environment: false,
        },
        id: PlayerId(1),
        transport_id: 1,
        body,
        accepted: true,
        synced: true,
        view: 1,
        known: default(),
        inputs: default(),
        received: 0,
        ack: 0,
        look: [0.0; 2],
        last_input: now,
        joined: now,
        last_request: 0,
        edits_left: 20.0,
        rate_time: now,
        spawned: true,
        restored_spawn: false,
        restore_location: None,
        last_movement: default(),
        vitals: default(),
        equipment: default(),
        survival: default(),
        stats_sent: None,
    };
    let command = EditCommand {
        request: 1,
        revision: 4,
        look: [0., 0.],
        source: None,
        action: EditAction::HarvestTree {
            id: 17,
            fruit_revision: 4,
            slot: 0,
        },
    };
    (app, peer, command)
}
fn apply(app: &mut App, peer: &Peer, command: EditCommand) -> EditOutcome {
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    harvest(command, peer, &mut world)
}
#[test]
fn actual_harvest_commits_inventory_and_persistent_slot_atomically_once() {
    let (mut app, peer, command) = fixture();
    assert_eq!(apply(&mut app, &peer, command), EditOutcome::Applied);
    let game = app.world().resource::<EditJournal>().gameplay();
    assert_eq!(game.inventories[&peer.uuid].count(ItemKind::Apple), 1);
    let fruit = game.trees.get(17).unwrap().fruit.unwrap();
    assert_eq!(
        (fruit.remaining, fruit.picked, fruit.last_harvest_day),
        (7, 1, Some(11))
    );
    let before = game.clone();
    assert_eq!(apply(&mut app, &peer, command), EditOutcome::Conflict);
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
}
#[test]
fn full_inventory_occlusion_stale_revision_and_overflow_leave_both_halves_unchanged() {
    for mode in 0..6 {
        let (mut app, peer, mut command) = fixture();
        match mode {
            0 => {
                let bag = app
                    .world_mut()
                    .resource_mut::<EditJournal>()
                    .into_inner()
                    .gameplay_mut()
                    .inventories
                    .get_mut(&peer.uuid)
                    .unwrap();
                bag.slots = vec![
                    ItemStack {
                        item: ItemKind::Block(3),
                        count: terra_core::STACK_LIMIT
                    };
                    terra_core::INVENTORY_SLOTS
                ];
            }
            1 => {
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .get_mut(&SectionPos::new(1, 0, 0))
                    .unwrap()
                    .data
                    .set_block([0, 3, 10], 3)
                    .unwrap();
            }
            2 => {
                command.action = EditAction::HarvestTree {
                    id: 17,
                    fruit_revision: 3,
                    slot: 0,
                };
            }
            3 => {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .gameplay_mut()
                    .revision = u64::MAX;
            }
            4 => {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .gameplay_mut()
                    .trees
                    .revision = u64::MAX;
            }
            _ => {
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .get_mut(&SectionPos::new(1, 0, 0))
                    .unwrap()
                    .data
                    .set_block([0, 4, 8], 0)
                    .unwrap();
            }
        }
        let before = app.world().resource::<EditJournal>().gameplay().clone();
        assert_ne!(
            apply(&mut app, &peer, command),
            EditOutcome::Applied,
            "scenario {mode}"
        );
        assert_eq!(
            app.world().resource::<EditJournal>().gameplay(),
            &before,
            "scenario {mode}"
        );
    }
}
#[test]
fn harvest_checks_inventory_rights_at_both_root_and_cross_chunk_fruit() {
    for seat in [[1, 1, 1], [16, 1, 1]] {
        let (mut app, peer, command) = fixture();
        let owner = terra_core::PlayerUuid::new_v4();
        let realm = {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            let registry = &mut journal.gameplay_mut().kingdoms;
            let realm = registry.found(owner, seat, "Orchard").unwrap();
            registry
                .claim(
                    realm,
                    [seat[0].div_euclid(16), 0],
                    ClaimKind::ProtectedBuilding,
                    ClaimActor::Player(owner),
                )
                .ok();
            realm
        };
        let before = app.world().resource::<EditJournal>().gameplay().clone();
        assert_eq!(apply(&mut app, &peer, command), EditOutcome::Invalid);
        assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            let registry = &mut journal.gameplay_mut().kingdoms;
            registry.join(peer.uuid, realm).unwrap();
            registry
                .set_role(owner, realm, peer.uuid, KingdomRole::Worker)
                .unwrap();
        }
        assert_eq!(apply(&mut app, &peer, command), EditOutcome::Applied);
    }
}
#[test]
fn removing_and_replacing_natural_root_without_a_fruit_tick_never_revives_harvest() {
    let (mut app, peer, command) = fixture();
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.record([14, 1, 8], 0).unwrap();
        journal.record([14, 1, 8], 6).unwrap();
    }
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([14, 1, 8]),
        Some(6)
    );
    assert_eq!(apply(&mut app, &peer, command), EditOutcome::Conflict);
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
            .count(ItemKind::Apple),
        0
    );
}
