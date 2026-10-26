use super::*;
use terra_core::{
    ItemKind,
    creatures::{AnimalProduct, ecology::NestState},
};
use terra_world::{
    edits::EditJournal,
    props::{PropId, PropKind, WorldProp},
};

fn pump(host: &mut App, a: &mut App, b: &mut App) {
    for _ in 0..24 {
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn block(host: &mut App, position: [i64; 3], material: u16) {
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            let expected = chunks.block_at(position).unwrap();
            if expected != material {
                assert!(
                    terra_world::streaming::commit_block_edit(
                        &mut chunks,
                        &mut journal,
                        &terra_world::streaming::BlockEditRequest {
                            position,
                            expected_material: expected,
                            material
                        },
                        &Default::default()
                    )
                    .unwrap()
                );
            }
        });
}

#[test]
fn real_udp_honey_use_checks_a_real_loaded_visible_nest_and_commits_inventory_without_removing_it()
{
    let (mut host, mut a, mut b) = pair();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .natural_mob_limit = Some(0);
    let owner = a.world().resource::<client::ClientSession>().uuid.unwrap();
    let player = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let floor = (player[1] - terra_physics::character::PLAYER_HALF_HEIGHT).round() as i64 - 1;
    let position = [
        f64::from(player[0]),
        floor as f64 + 1.0,
        f64::from(player[2]) - 3.0,
    ];
    let support = [
        position[0].floor() as i64,
        floor,
        position[2].floor() as i64,
    ];
    let section = terra_voxel::global_to_section(support).0;
    wait(&mut host, &mut [&mut a, &mut b], |h, _| {
        h.world()
            .resource::<ChunkStore>()
            .block_at([support[0], floor + 3, support[2]])
            .is_some()
    });
    block(&mut host, support, 3);
    for y in 1..=3 {
        block(&mut host, [support[0], floor + y, support[2]], 0);
    }
    let nest = WorldProp {
        id: PropId(121125),
        kind: PropKind::BeeNest,
        position,
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section,
        removed: false,
    };
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            assert!(journal.place_prop(nest).unwrap());
            world
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&section)
                .unwrap()
                .props
                .push(nest);
            journal.gameplay_mut().creatures.ecology.nests.insert(
                nest.id.0,
                NestState {
                    position,
                    nectar: 345,
                    honey: 2,
                    progress: 0.75,
                },
            );
        });
    let eye = Vec3::from_array(player) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let direction =
        (Vec3::from_array(position.map(|v| v as f32)) + Vec3::Y * 0.25 - eye).normalize();
    let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
    let actual = terra_world::targeting::pick_world_with_context(
        host.world().resource::<ChunkStore>(),
        host.world().resource::<EditJournal>(),
        host.world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
        terra_world::props::PropPose::Rest,
    )
    .1;
    assert!(
        matches!(actual,Some(terra_world::targeting::WorldTarget::Prop(hit)) if hit.prop.id==nest.id)
    );
    let profile = terra_core::PlayerProfile::default();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .player_permissions
        .insert(
            profile.uuid,
            terra_core::PlayerPermissions {
                build: false,
                game_mode: false,
                flight: false,
                environment: false,
            },
        );
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut denied = profile_client(address, profile);
    wait(
        &mut host,
        &mut [&mut a, &mut b, &mut denied],
        |_, clients| clients[2].world().resource::<client::ClientSession>().ready,
    );
    assert!(
        !denied
            .world()
            .resource::<client::ClientSession>()
            .allow_build
    );
    send(
        &mut denied,
        protocol::session::ClientControl::HarvestHoney {
            section,
            id: nest.id,
            look,
        },
    );
    for _ in 0..24 {
        host.update();
        a.update();
        b.update();
        denied.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .ecology
            .nests[&nest.id.0]
            .honey,
        2
    );
    drop(denied);
    send(
        &mut a,
        protocol::session::ClientControl::HarvestHoney {
            section,
            id: PropId(991),
            look,
        },
    );
    pump(&mut host, &mut a, &mut b);
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .ecology
            .nests[&nest.id.0]
            .honey,
        2
    );
    let wall = [support[0], floor + 2, support[2] + 1];
    for dx in -1..=1 {
        for dy in -1..=1 {
            block(&mut host, [wall[0] + dx, wall[1] + dy, wall[2]], 3);
        }
    }
    std::thread::sleep(Duration::from_millis(460));
    send(
        &mut a,
        protocol::session::ClientControl::HarvestHoney {
            section,
            id: nest.id,
            look,
        },
    );
    pump(&mut host, &mut a, &mut b);
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .ecology
            .nests[&nest.id.0]
            .honey,
        2
    );
    for dx in -1..=1 {
        for dy in -1..=1 {
            block(&mut host, [wall[0] + dx, wall[1] + dy, wall[2]], 0);
        }
    }
    std::thread::sleep(Duration::from_millis(460));
    a.world_mut().resource_mut::<client::ClientSession>().look = look;
    a.world_mut()
        .resource_mut::<client::BeeNestActions>()
        .0
        .push_back((section, nest.id));
    let honey = ItemKind::Animal(AnimalProduct::Honey);
    wait(&mut host, &mut [&mut a, &mut b], |h, c| {
        h.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .ecology
            .nests[&nest.id.0]
            .honey
            == 1
            && c[0]
                .world()
                .resource::<client::ClientSession>()
                .inventory
                .count(honey)
                == 1
            && c[1]
                .world()
                .resource::<client::ReplicatedCreatures>()
                .ecology
                .nests
                .get(&nest.id.0)
                .is_some_and(|n| n.honey == 1)
    });
    let journal = host.world().resource::<EditJournal>();
    assert!(!journal.prop_removed(section, nest.id));
    assert_eq!(
        journal.gameplay().creatures.ecology.nests[&nest.id.0].nectar,
        345
    );
    assert_eq!(journal.gameplay().inventories[&owner].count(honey), 1);
    assert!(journal.gameplay().valid());
}
