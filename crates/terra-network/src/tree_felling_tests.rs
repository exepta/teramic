use super::*;
use crate::protocol::session::{MiningIntent, MiningTarget};
use terra_core::{GameMode, PlayerProfile};
use terra_world::{
    edits::EditJournal,
    streaming::{StreamingSettings, WorldTerrain},
};

fn picked(host: &App, eye: Vec3, look: [f32; 2]) -> Option<terra_world::targeting::WorldTarget> {
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let journal = host.world().resource::<EditJournal>();
    let season = journal
        .clock
        .unwrap_or_default()
        .season(host.world().resource::<GameConfig>().world.seasons);
    terra_world::targeting::pick_world_with_context(
        host.world().resource::<ChunkStore>(),
        journal,
        host.world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
        terra_world::props::PropPose::WeatherSwept {
            frame: terra_world::props::weather_wind_frame(
                host.world().resource::<terra_core::WorldWeather>().wind(),
            ),
        }
        .with_season(season),
    )
    .1
}

fn base_look(host: &App, foot: [i64; 3], eye: Vec3) -> Option<[f32; 2]> {
    for target_y in [0.65, 0.85, 0.35, 0.15] {
        for [x, z] in [
            [0.5, 0.5],
            [0.25, 0.5],
            [0.75, 0.5],
            [0.5, 0.25],
            [0.5, 0.75],
        ] {
            let target = Vec3::new(
                foot[0] as f32 + x,
                foot[1] as f32 + target_y,
                foot[2] as f32 + z,
            );
            let direction = (target - eye).normalize();
            let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
            if matches!(picked(host, eye, look), Some(terra_world::targeting::WorldTarget::Block(hit)) if hit.position == foot)
            {
                return Some(look);
            }
        }
    }
    None
}

fn wait_for_progress(host: &mut App, client: &mut App, intent: MiningIntent) {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut heartbeat = Instant::now();
    loop {
        if Instant::now() >= heartbeat {
            client
                .world_mut()
                .write_message(client::MiningRequest(intent));
            heartbeat = Instant::now() + Duration::from_millis(50);
        }
        host.update();
        client.update();
        let session = client.world().resource::<client::ClientSession>();
        assert!(session.error.is_none(), "{:?}", session.error);
        if session.mining.is_some() {
            return;
        }
        if Instant::now() >= deadline {
            let MiningTarget::Block { position, .. } = intent.target.unwrap() else {
                panic!("tree fixture requires a block target")
            };
            let chunks = host.world().resource::<ChunkStore>();
            let journal = host.world().resource::<EditJournal>();
            let eye = Vec3::from_array(session.confirmed_state().unwrap().position)
                + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
            panic!(
                "natural tree mining did not start: foot={position:?}, eye={eye:?}, look={:?}, selected={:?}, eligible={}, plan_ready={}, block={:?}, edit={:?}, confirmed={:?}",
                intent.look,
                picked(host, eye, intent.look),
                terra_world::tree_felling::eligible(chunks, journal, position),
                terra_world::tree_felling::Plan::prepare(chunks, journal, position).is_some(),
                chunks.block_at(position),
                session.last_edit,
                session.confirmed_state(),
            );
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn candidates(host: &App) -> Vec<([i64; 3], [f32; 3], [f32; 2])> {
    let chunks = host.world().resource::<ChunkStore>();
    let journal = host.world().resource::<EditJournal>();
    let properties = host
        .world()
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>();
    let mut result = Vec::new();
    for (&section, loaded) in &chunks.loaded {
        for (index, &material) in loaded.data.blocks().iter().enumerate() {
            if !matches!(material, 6 | 10 | 36 | 134) {
                continue;
            }
            let cell = section
                .global_block([index % 16, (index / 16) % 16, index / 256])
                .unwrap();
            if !terra_world::tree_felling::eligible(chunks, journal, cell) {
                continue;
            }
            // Cardinal roots can screen a thick natural trunk. Inspect diagonal
            // approaches too, and aim at its actual lowest visible base row.
            for [dx, dz] in [
                [4.5, 0.0],
                [-4.5, 0.0],
                [0.0, 4.5],
                [0.0, -4.5],
                [3.2, 3.2],
                [-3.2, 3.2],
                [3.2, -3.2],
                [-3.2, -3.2],
            ] {
                let feet = Vec3::new(
                    cell[0] as f32 + 0.5 + dx,
                    cell[1] as f32,
                    cell[2] as f32 + 0.5 + dz,
                );
                let body = feet + Vec3::Y * (terra_physics::character::PLAYER_HALF_HEIGHT + 0.02);
                let eye = body + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
                if !terra_physics::character::space_clear_with_shapes(
                    chunks, journal, properties, body,
                ) {
                    continue;
                }
                for target_y in [0.15, 0.35, 0.65, 0.85] {
                    let mut target = Vec3::from_array(cell.map(|v| v as f32 + 0.5));
                    target.y = cell[1] as f32 + target_y;
                    let direction = (target - eye).normalize();
                    let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
                    let Some(terra_world::targeting::WorldTarget::Block(hit)) =
                        picked(host, eye, look)
                    else {
                        continue;
                    };
                    if hit.position != cell
                        || !terra_world::tree_felling::eligible(chunks, journal, hit.position)
                        || chunks
                            .block_at([feet.x.floor() as i64, cell[1] - 1, feet.z.floor() as i64])
                            .is_none_or(|m| {
                                !terra_voxel::MaterialId::from_raw(m)
                                    .is_some_and(|m| m.is_natural_ground())
                            })
                    {
                        continue;
                    }
                    result.push((hit.position, feet.to_array(), look));
                    break;
                }
                if result.last().is_some_and(|entry| entry.0 == cell) {
                    break;
                }
            }
        }
    }
    result.sort_unstable_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.map(f32::to_bits).cmp(&b.1.map(f32::to_bits)))
            .then_with(|| a.2.map(f32::to_bits).cmp(&b.2.map(f32::to_bits)))
    });
    result
}

#[test]
fn udp_whole_tree_felling_has_exact_duration_is_cancellable_and_replicates_bounded_drops() {
    whole_tree_felling(GameMode::Survival, terra_core::ToolKind::WoodAxe);
}

#[test]
fn creative_axe_whole_tree_felling_is_timed_cancellable_and_never_creates_free_wood_drops() {
    whole_tree_felling(GameMode::Creative, terra_core::ToolKind::GoldAxe);
}

fn whole_tree_felling(mode: GameMode, tool: terra_core::ToolKind) {
    let mut host = base(
        WorldMode::DedicatedServer,
        connection::NetworkRole::DedicatedServer,
    );
    {
        let mut config = host.world_mut().resource_mut::<GameConfig>();
        config.server.game_mode = mode;
        config.world.trees = true;
        config.world.tree_density = 1.0;
        config.world.cave_structure_revision = 0;
        config.world.rivers = false;
        config.world.base_height = 12.0;
        config.world.sea_level = 0.0;
    }
    let terrain =
        terra_world::TerraGenerator::new(host.world().resource::<GameConfig>().world.clone())
            .unwrap();
    host.insert_resource(WorldTerrain(terrain));
    {
        let mut settings = host.world_mut().resource_mut::<StreamingSettings>();
        settings.radius = 2;
    }
    host.insert_resource(server::ServerEndpoint {
        bind: ([127, 0, 0, 1], 0).into(),
        advertised: None,
    });
    host.finish();
    host.cleanup();
    host.update();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let profile = PlayerProfile {
        name: "Feller".into(),
        ..Default::default()
    };
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .entry(profile.uuid)
        .or_default()
        .insert(terra_core::ItemKind::Tool(tool), 1);
    let mut client = profile_client(address, profile);
    client
        .world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(0);
    #[derive(Resource, Default)]
    struct Collapses(Vec<terra_world::tree_felling::Collapse>);
    client.init_resource::<Collapses>().add_systems(
        Update,
        |mut events: MessageReader<client::TreeCollapseEvent>, mut received: ResMut<Collapses>| {
            received
                .0
                .extend(events.read().map(|event| event.0.clone()));
        },
    );
    wait(&mut host, &mut [&mut client], |_, clients| {
        clients[0].world().resource::<client::ClientSession>().ready
    });
    let spawn = client.world().resource::<client::ClientSession>().spawn;
    client
        .world_mut()
        .spawn(terra_physics::character::body_bundle(spawn));
    let started = Instant::now();
    let reported = std::cell::Cell::new(false);
    wait(&mut host, &mut [&mut client], |host, _| {
        let sites = candidates(host);
        if sites.is_empty()
            && started.elapsed() > Duration::from_secs(13)
            && !reported.replace(true)
        {
            let chunks = host.world().resource::<ChunkStore>();
            let journal = host.world().resource::<EditJournal>();
            let mut wood = 0;
            let mut eligible = Vec::new();
            for (&section, loaded) in &chunks.loaded {
                for (index, &material) in loaded.data.blocks().iter().enumerate() {
                    if !matches!(material, 6 | 10 | 36 | 134) {
                        continue;
                    }
                    wood += 1;
                    let cell = section
                        .global_block([index % 16, (index / 16) % 16, index / 256])
                        .unwrap();
                    if terra_world::tree_felling::eligible(chunks, journal, cell)
                        && eligible.len() < 12
                    {
                        eligible.push(cell);
                    }
                }
            }
            eprintln!(
                "felling site diagnostic: loaded={}, wood={}, eligible={eligible:?}, terrain_trees={}, server_error={:?}",
                chunks.loaded.len(),
                wood,
                host.world().resource::<WorldTerrain>().0.config().trees,
                host.world().resource::<server::ServerRuntime>().error
            );
        }
        !sites.is_empty()
    });
    let (foot, feet, _) = candidates(&host)[0];
    assert!(
        host.world_mut()
            .resource_mut::<server::ServerConsole>()
            .submit(format!("tp Feller {} {} {}", feet[0], feet[1], feet[2]))
    );
    let teleport_started = Instant::now();
    let teleport_reported = std::cell::Cell::new(false);
    wait(&mut host, &mut [&mut client], |host, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        if teleport_started.elapsed() > Duration::from_secs(13) && !teleport_reported.replace(true)
        {
            eprintln!(
                "felling teleport diagnostic: destination={feet:?}, foot={foot:?}, ready={}, teleporting={}, confirmed={:?}, console={:?}",
                session.ready,
                session.teleporting,
                session
                    .confirmed_state()
                    .map(|s| (s.generation, s.position, s.grounded)),
                host.world().resource::<server::ServerConsole>().output
            );
        }
        session.ready
            && !session.teleporting
            && session
                .confirmed_state()
                .is_some_and(|state| state.generation > 0 && state.grounded)
    });
    let material = host
        .world()
        .resource::<ChunkStore>()
        .block_at(foot)
        .unwrap();
    let confirmed = client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye = Vec3::from_array(confirmed.position)
        + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let look = base_look(&host, foot, eye).unwrap_or_else(|| {
        panic!("natural tree base is not visible after real teleport: foot={foot:?}, eye={eye:?}")
    });
    let intent = MiningIntent {
        target: Some(MiningTarget::Block {
            position: foot,
            expected: material,
        }),
        look,
        fell_tree: true,
    };
    client
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = look;
    wait_for_progress(&mut host, &mut client, intent);
    let (cells, count) = {
        let plan = terra_world::tree_felling::Plan::prepare(
            host.world().resource::<ChunkStore>(),
            host.world().resource::<EditJournal>(),
            foot,
        )
        .unwrap();
        (plan.wood_positions().collect::<Vec<_>>(), plan.wood_count())
    };
    assert!(
        client
            .world()
            .resource::<client::ClientSession>()
            .mining
            .is_some_and(|p| p.duration
                == terra_world::tree_felling::duration(
                    count,
                    Some(terra_core::ItemKind::Tool(tool))
                )
                && p.progress < 1.0)
    );
    assert_eq!(host.world().resource::<EditJournal>().block(foot), None);
    client
        .world_mut()
        .write_message(client::MiningRequest(MiningIntent {
            target: None,
            fell_tree: false,
            ..intent
        }));
    wait(&mut host, &mut [&mut client], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .mining
            .is_none()
    });
    assert!(!server::elapse_tree_mining_timer_for_test(host.world_mut()));
    assert!(
        client.world().resource::<Collapses>().0.is_empty(),
        "cancelled mining must not animate a falling tree"
    );
    wait_for_progress(&mut host, &mut client, intent);
    // The real duration was asserted above; do not spend several minutes waiting
    // for a large procedural tree in a UDP regression.
    assert!(server::elapse_tree_mining_timer_for_test(host.world_mut()));
    client
        .world_mut()
        .write_message(client::MiningRequest(intent));
    wait(&mut host, &mut [&mut client], |host, _| {
        host.world()
            .resource::<terra_world::tree_felling::Queue>()
            .is_empty()
            && cells
                .iter()
                .all(|p| host.world().resource::<EditJournal>().block(*p) == Some(0))
    });
    let uuid = client
        .world()
        .resource::<client::ClientSession>()
        .uuid
        .unwrap();
    let journal = host.world().resource::<EditJournal>();
    let loose: usize = journal
        .gameplay()
        .drops
        .iter()
        .filter(|d| d.stack.item == terra_core::ItemKind::Block(material))
        .map(|d| usize::from(d.stack.count))
        .sum();
    let picked = journal.gameplay().inventories.get(&uuid).map_or(0, |bag| {
        usize::from(bag.count(terra_core::ItemKind::Block(material)))
    });
    assert_eq!(
        loose + picked,
        if mode == GameMode::Survival { count } else { 0 }
    );
    assert!(journal.gameplay().valid());
    wait(&mut host, &mut [&mut client], |_, clients| {
        clients[0].world().resource::<EditJournal>().block(foot) == Some(0)
    });
    let collapses = &client.world().resource::<Collapses>().0;
    assert_eq!(
        collapses.len(),
        1,
        "one accepted tree cut produces exactly one cosmetic snapshot"
    );
    assert!(collapses[0].valid());
    assert_eq!(collapses[0].foot, foot);
    assert_eq!(
        collapses[0]
            .sections
            .iter()
            .map(|s| s.wood.len())
            .sum::<usize>(),
        count
    );
}
