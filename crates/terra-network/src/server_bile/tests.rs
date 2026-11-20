use super::*;
use terra_core::{
    DamageCause, GameMode,
    creatures::{Creature, CreatureBehavior, CreatureKind},
};
fn player(app: &mut App, mode: GameMode, at: Vec3, health: f32) -> Entity {
    let mut peer = super::super::survival::tests::resting_peer(health);
    peer.mode = Some(mode);
    peer.body = app
        .world_mut()
        .spawn((
            ServerCharacter {
                link: Entity::PLACEHOLDER,
            },
            Transform::from_translation(at),
        ))
        .id();
    let transport_id = peer.body.to_bits();
    let entity = app
        .world_mut()
        .spawn((RemoteId(PeerId::Netcode(transport_id)), peer))
        .id();
    app.world_mut().entity_mut(entity).insert(Connected);
    entity
}
fn step(app: &mut App, seconds: f64) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f64(seconds));
    app.update();
}
#[test]
fn real_server_contacts_hurt_players_and_mobs_with_cooldown_immunity_and_one_death() {
    let mut chunks = ChunkStore::default();
    let p = terra_voxel::SectionPos::new(0, 0, 0);
    let mut data = terra_voxel::BlockSection::empty(p).unwrap();
    data.set_block([4, 1, 4], 440).unwrap();
    chunks.loaded.insert(
        p,
        terra_world::streaming::LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let mut app = App::new();
    app.insert_resource(Time::<()>::default())
        .init_resource::<GameConfig>()
        .init_resource::<EditJournal>()
        .init_resource::<riding::Mounts>()
        .insert_resource(chunks)
        .add_systems(Update, tick);
    let touch = Vec3::new(3.8, 1.9, 4.5);
    let a = player(&mut app, GameMode::Survival, touch, 100.0);
    let distant = player(
        &mut app,
        GameMode::Survival,
        Vec3::new(3.4, 1.9, 4.5),
        100.0,
    );
    let creative = player(&mut app, GameMode::Creative, touch, 100.0);
    let doomed = player(&mut app, GameMode::Survival, touch, 2.0);
    let protected = player(&mut app, GameMode::Survival, touch, 100.0);
    app.world_mut()
        .get_mut::<Peer>(protected)
        .unwrap()
        .survival
        .protection = 2.0;
    let kind = CreatureKind::Pig;
    let pig = Creature {
        id: 1,
        kind,
        home: [0, 0],
        position: [3.85, 1.0, 4.5],
        velocity: [0.; 3],
        yaw: 0.,
        health: kind.health(),
        behavior: CreatureBehavior::Rest,
        timer: 0.,
        age: 0.,
        variant: 0,
        needs: Default::default(),
    };
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .creatures
        .animals
        .insert(1, pig);
    step(&mut app, 0.1);
    assert_eq!(app.world().get::<Peer>(a).unwrap().vitals.health, 94.0);
    assert_eq!(
        app.world().get::<Peer>(a).unwrap().vitals.last_damage,
        Some(DamageCause::Acid)
    );
    for entity in [distant, creative, protected] {
        assert_eq!(
            app.world().get::<Peer>(entity).unwrap().vitals.health,
            100.0
        );
    }
    assert!(app.world().get::<Peer>(doomed).unwrap().vitals.dead());
    assert_eq!(
        app.world()
            .get::<Peer>(doomed)
            .unwrap()
            .survival
            .damage_epoch,
        1
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .animals[&1]
            .health,
        kind.health() - 6.0
    );
    step(&mut app, 0.1);
    assert_eq!(app.world().get::<Peer>(a).unwrap().vitals.health, 94.0);
    step(&mut app, 0.8);
    assert_eq!(app.world().get::<Peer>(a).unwrap().vitals.health, 88.0);
    assert_eq!(
        app.world()
            .get::<Peer>(doomed)
            .unwrap()
            .survival
            .damage_epoch,
        1
    );
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&p)
        .unwrap()
        .data
        .set_block([4, 1, 4], 0)
        .unwrap();
    step(&mut app, 1.0);
    assert_eq!(app.world().get::<Peer>(a).unwrap().vitals.health, 88.0);
}

#[test]
fn broadphase_includes_large_bodies_and_players_crossing_section_edges() {
    let sections = std::collections::HashSet::from([terra_voxel::SectionPos::new(1, 0, 0)]);
    assert!(near_acid(&sections, [15.8, 3., 2.], 2.));
    assert!(near_acid(&sections, [-4., 3., 2.], 24.));
    assert!(!near_acid(&sections, [-4., 3., 2.], 2.));
    assert!(!near_acid(&sections, [100., 3., 2.], 24.));
}
