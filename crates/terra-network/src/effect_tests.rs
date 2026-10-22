use super::*;
use crate::client::{
    ChatCommand, ClientChat, ClientChatCommands, ClientSession, ReplicatedPlayers,
};
use terra_core::{EffectKind, GameMode, PlayerProfile};
use terra_world::edits::EditJournal;

#[derive(Resource, Default)]
struct TamperedHello {
    enabled: bool,
    count: usize,
}

fn tamper_effect_catalog(
    mut receivers: Query<
        &mut lightyear::prelude::MessageReceiver<protocol::session::ClientControl>,
    >,
    mut tampered: ResMut<TamperedHello>,
) {
    if !tampered.enabled {
        return;
    }
    for mut receiver in &mut receivers {
        receiver.retain_messages(|message| {
            if let protocol::session::ClientControl::Hello { effect_catalog, .. } = message {
                *effect_catalog ^= 1;
                tampered.count += 1;
            }
            true
        });
    }
}

#[test]
fn different_effect_catalogs_are_rejected_before_accepting_a_real_udp_player() {
    let mut host = server();
    host.insert_resource(TamperedHello {
        enabled: true,
        count: 0,
    })
    .add_systems(
        PreUpdate,
        tamper_effect_catalog.after(lightyear::prelude::MessageSystems::Receive),
    );
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let profile = PlayerProfile::default();
    let uuid = profile.uuid;
    let mut client = profile_client(address, profile);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        host.update();
        client.update();
        if let Some(error) = &client.world().resource::<ClientSession>().error {
            assert!(error.contains("status effect definitions"), "{error}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "mismatched catalog was not rejected"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(host.world().resource::<TamperedHello>().count, 1);
    assert!(
        host.world()
            .resource::<server::ConnectedProfiles>()
            .0
            .is_empty()
    );
    assert!(
        !host
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .player_stats
            .contains_key(&uuid)
    );
    assert!(!client.world().resource::<ClientSession>().ready);
    host.world_mut().resource_mut::<TamperedHello>().enabled = false;
    let mut compatible = profile_client(address, PlayerProfile::default());
    wait(&mut host, &mut [&mut compatible], |_, clients| {
        clients[0].world().resource::<ClientSession>().ready
    });
    assert_eq!(
        host.world().resource::<server::ConnectedProfiles>().0.len(),
        1
    );
}

fn effect_command(app: &mut App, command: &str) {
    app.world_mut()
        .resource_mut::<ClientChatCommands>()
        .0
        .push_back(ChatCommand::Send(command.into()));
}

#[test]
fn status_effects_use_authority_permissions_and_replicate_to_owner_and_other_client() {
    let mut host = server();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Survival;
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let profile = PlayerProfile {
        name: "EffectsAlice".into(),
        ..Default::default()
    };
    let uuid = profile.uuid;
    let mut alice = profile_client(address, profile);
    let mut bob = profile_client(address, PlayerProfile::default());
    wait(&mut host, &mut [&mut alice, &mut bob], |_, clients| {
        clients
            .iter()
            .all(|client| client.world().resource::<ClientSession>().ready)
    });
    let player = alice.world().resource::<ClientSession>().player.unwrap();
    effect_command(&mut alice, "/effect give speed 10");
    wait(&mut host, &mut [&mut alice, &mut bob], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|entry| entry.text.contains("requires Creative"))
    });
    assert!(
        !host
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .player_stats[&uuid]
            .survival
            .effects
            .contains(EffectKind::Speed)
    );
    assert!(
        host.world_mut()
            .resource_mut::<server::ServerConsole>()
            .submit("as EffectsAlice effect give speed 10 2".into())
    );
    wait(&mut host, &mut [&mut alice, &mut bob], |host, clients| {
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .player_stats[&uuid]
            .survival
            .effects
            .get(EffectKind::Speed)
            .is_some_and(|effect| effect.level == 2)
            && clients[0]
                .world()
                .resource::<ClientSession>()
                .stats
                .survival
                .effects
                .get(EffectKind::Speed)
                .is_some_and(|effect| effect.level == 2)
            && clients[1]
                .world()
                .resource::<ReplicatedPlayers>()
                .players
                .get(&player)
                .is_some_and(|track| track.current.survival.effects.contains(EffectKind::Speed))
    });
    assert!(
        host.world_mut()
            .resource_mut::<server::ServerConsole>()
            .submit("as EffectsAlice effect clear speed".into())
    );
    wait(&mut host, &mut [&mut alice, &mut bob], |host, clients| {
        !host
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .player_stats[&uuid]
            .survival
            .effects
            .contains(EffectKind::Speed)
            && !clients[0]
                .world()
                .resource::<ClientSession>()
                .stats
                .survival
                .effects
                .contains(EffectKind::Speed)
            && clients[1]
                .world()
                .resource::<ReplicatedPlayers>()
                .players
                .get(&player)
                .is_some_and(|track| !track.current.survival.effects.contains(EffectKind::Speed))
    });
    effect_command(&mut alice, "/effect give poison 10");
    wait(&mut host, &mut [&mut alice, &mut bob], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .filter(|entry| entry.text.contains("requires Creative"))
            .count()
            >= 2
    });
    assert!(
        !alice
            .world()
            .resource::<ClientSession>()
            .stats
            .survival
            .effects
            .contains(EffectKind::Poison)
    );
}
