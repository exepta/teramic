use super::*;
use crate::server::rgb_filter_tests::{fixture, look, place};
use bevy::ecs::system::SystemState;
use terra_core::{ItemStack, brewing, campfire};
use terra_world::edits::BlockObjectState;

fn open(app: &mut App, peer: &mut Peer, station: [i64; 3]) {
    let revision =
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].revision;
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    super::super::access(
        InventoryCommand {
            revision,
            action: InventoryAction::OpenContainer { position: station },
        },
        peer,
        &mut MessageSender::default(),
        &mut system.get_mut(app.world_mut()).unwrap(),
    );
    assert!(
        peer.container.is_some(),
        "station must open through real shape LOS"
    );
}

#[test]
fn cauldron_recipe_selection_requires_an_open_visible_owner_and_resets_only_its_progress() {
    let (mut app, mut peer) = fixture();
    let station = [-1, 16, -1];
    place(
        &mut app,
        station,
        brewing::MATERIAL,
        Some(BlockObjectState::default()),
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_cauldron(
            station,
            brewing::CauldronState {
                water: 2,
                progress_seconds: 3.0,
                ..Default::default()
            },
        )
        .unwrap();
    look(
        &mut app,
        &mut peer,
        Vec3::new(-0.5, 17.4, 2.5),
        Vec3::new(-0.5, 16.6, -0.5),
    );
    let call = |app: &mut App, peer: &Peer, id| {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        set_recipe(
            peer,
            &mut system.get_mut(app.world_mut()).unwrap(),
            station,
            id,
        )
    };
    assert_eq!(
        call(&mut app, &peer, 6),
        InventoryOutcome::ContainerUnavailable
    );
    open(&mut app, &mut peer, station);
    assert!(peer.container.as_ref().unwrap().cauldron.is_some());
    assert_eq!(call(&mut app, &peer, 6), InventoryOutcome::Applied);
    let state = app
        .world()
        .resource::<EditJournal>()
        .cauldron_at(station)
        .unwrap();
    assert_eq!(
        (state.recipe, state.water, state.progress_seconds),
        (Some(6), 2, 0.0)
    );
    assert_eq!(
        call(&mut app, &peer, u16::MAX),
        InventoryOutcome::UnknownRecipe
    );
    let original = app.world().resource::<EditJournal>().gameplay().clone();
    let revision = original.inventories[&peer.uuid].revision;
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        super::super::access(
            InventoryCommand {
                revision: revision + 1,
                action: InventoryAction::SetBrewingRecipe { station, recipe: 1 },
            },
            &mut peer,
            &mut MessageSender::default(),
            &mut system.get_mut(app.world_mut()).unwrap(),
        );
    }
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .cauldron_at(station)
            .unwrap()
            .recipe,
        Some(6)
    );
    look(
        &mut app,
        &mut peer,
        Vec3::new(-0.5, 17.4, 12.5),
        Vec3::new(-0.5, 16.6, -0.5),
    );
    assert_eq!(
        call(&mut app, &peer, 1),
        InventoryOutcome::ContainerUnavailable
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories,
        original.inventories
    );
}

#[test]
fn campfire_ignition_spends_one_fuel_once_and_water_or_closed_access_cannot_ignite() {
    let (mut app, mut peer) = fixture();
    // A Rain command becomes snow in highlands; this ignition case needs real local rain.
    let mut config = app.world().resource::<GameConfig>().world.clone();
    config.biomes = false;
    config.rivers = false;
    config.landscape_revision = 0;
    config.base_height = 20.0;
    config.hill_amplitude = 0.0;
    config.mountain_amplitude = 0.0;
    app.insert_resource(terra_world::streaming::WorldTerrain(
        terra_world::TerraGenerator::new(config).unwrap(),
    ));
    let station = [4, 4, 4];
    place(&mut app, [4, 3, 4], 3, None);
    place(
        &mut app,
        station,
        campfire::MATERIAL,
        Some(BlockObjectState::default()),
    );
    let mut inventory = Inventory {
        slots: vec![ItemStack::EMPTY; campfire::SLOTS],
        ..Default::default()
    };
    inventory.slots[campfire::FUEL] = ItemStack {
        item: ItemKind::Stick,
        count: 2,
    };
    app.world_mut()
        .resource_mut::<EditJournal>()
        .commit_campfire(station, Default::default(), inventory)
        .unwrap();
    look(
        &mut app,
        &mut peer,
        Vec3::new(4.5, 5.2, 7.5),
        Vec3::new(4.5, 4.15, 4.5),
    );
    let call = |app: &mut App, peer: &Peer, lit| {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        set_lit(
            peer,
            &mut system.get_mut(app.world_mut()).unwrap(),
            station,
            lit,
        )
    };
    assert_eq!(
        call(&mut app, &peer, true),
        InventoryOutcome::ContainerUnavailable
    );
    open(&mut app, &mut peer, station);
    assert_eq!(call(&mut app, &peer, true), InventoryOutcome::Applied);
    let before = app
        .world()
        .resource::<EditJournal>()
        .container_at(station)
        .unwrap()
        .clone();
    assert_eq!(before.slot(campfire::FUEL).unwrap().count, 1);
    assert_eq!(call(&mut app, &peer, true), InventoryOutcome::Applied);
    assert_eq!(
        app.world().resource::<EditJournal>().container_at(station),
        Some(&before)
    );
    assert_eq!(call(&mut app, &peer, false), InventoryOutcome::Applied);
    assert!(
        !app.world()
            .resource::<EditJournal>()
            .campfire_at(station)
            .unwrap()
            .lit
    );
    for y in (16..=80).step_by(16) {
        place(&mut app, [4, y, 4], 0, None);
    }
    assert!(app.world().resource::<EditJournal>().weather.is_none());
    {
        let mut weather = app.world_mut().resource_mut::<terra_core::WorldWeather>();
        weather.set_pattern(terra_core::WeatherPattern::Rain);
        weather.advance(8.0, 164);
    }
    let chunks = app.world().resource::<ChunkStore>();
    assert!(terra_world::hearths::rain_exposed(
        chunks,
        app.world().resource::<EditJournal>(),
        station
    ));
    let local = terra_world::weather::SurfaceWeatherProbe::default().sample(
        app.world().resource::<terra_core::WorldWeather>(),
        &app.world()
            .resource::<terra_world::streaming::WorldTerrain>()
            .0,
        chunks,
        station.map(|value| value as f64 + 0.5),
        *app.world().resource::<terra_core::WorldClock>(),
    );
    assert!(local.rain > 0.2, "fixture needs actual rain: {local:?}");
    assert_eq!(call(&mut app, &peer, true), InventoryOutcome::Invalid);
    assert_eq!(
        app.world().resource::<EditJournal>().container_at(station),
        Some(&before)
    );
    {
        let mut weather = app.world_mut().resource_mut::<terra_core::WorldWeather>();
        weather.set_pattern(terra_core::WeatherPattern::Clear);
        weather.advance(8.0, 164);
    }
    place(&mut app, [5, 4, 4], 27, None);
    assert_eq!(call(&mut app, &peer, true), InventoryOutcome::Invalid);
    assert_eq!(
        app.world().resource::<EditJournal>().container_at(station),
        Some(&before)
    );
}

#[test]
fn filling_a_real_visible_water_source_conserves_bottles_and_rejects_full_inventory_or_air() {
    let (mut app, mut peer) = fixture();
    let water = [4, 4, 4];
    place(&mut app, water, 27, None);
    look(
        &mut app,
        &mut peer,
        Vec3::new(4.5, 5.6, 7.5),
        Vec3::new(4.5, 4.4, 4.5),
    );
    let mut inventory = Inventory::default();
    assert_eq!(inventory.insert(ItemKind::GlassBottle, 2), 0);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .insert(peer.uuid, inventory.clone());
    let call = |app: &mut App, peer: &Peer, inventory: &Inventory, position| {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        fill_bottle(
            peer,
            &mut system.get_mut(app.world_mut()).unwrap(),
            inventory,
            0,
            position,
        )
    };
    assert_eq!(
        call(&mut app, &peer, &inventory, water),
        InventoryOutcome::Applied
    );
    let next = app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].clone();
    assert_eq!(
        next.slots
            .iter()
            .filter(|s| s.valid())
            .map(|s| s.count)
            .sum::<u16>(),
        2
    );
    assert_eq!(
        next.slot(0),
        Some(ItemStack {
            item: ItemKind::GlassBottle,
            count: 1
        })
    );
    assert_eq!(
        next.slots
            .iter()
            .filter(|s| s.item == ItemKind::WaterBottle)
            .map(|s| s.count)
            .sum::<u16>(),
        1
    );
    assert_eq!(
        call(&mut app, &peer, &next, [4, 4, 5]),
        InventoryOutcome::Denied
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid],
        next
    );
    let mut full = Inventory {
        slots: vec![
            ItemStack {
                item: ItemKind::Block(3),
                count: 64
            };
            terra_core::INVENTORY_SLOTS
        ],
        ..Default::default()
    };
    full.slots[0] = ItemStack {
        item: ItemKind::GlassBottle,
        count: 2,
    };
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .insert(peer.uuid, full.clone());
    assert_eq!(
        call(&mut app, &peer, &full, water),
        InventoryOutcome::NoSpace
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid],
        full
    );
    full.slots[0].count = 1;
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .insert(peer.uuid, full.clone());
    assert_eq!(
        call(&mut app, &peer, &full, water),
        InventoryOutcome::Applied
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].slot(0),
        Some(ItemStack {
            item: ItemKind::WaterBottle,
            count: 1
        })
    );
    let mut overflow = Inventory {
        revision: u64::MAX,
        ..inventory
    };
    overflow.slots[0] = ItemStack {
        item: ItemKind::GlassBottle,
        count: 2,
    };
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .insert(peer.uuid, overflow.clone());
    assert_eq!(
        call(&mut app, &peer, &overflow, water),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid],
        overflow
    );
}
