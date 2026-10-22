use bevy::{app::AppExit, time::TimeUpdateStrategy};
use std::time::{Duration, Instant};
use terra_core::GameConfig;
use terra_network::connection::ConnectionStatus;
use terra_world::streaming::{ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain};

#[test]
fn dedicated_server_initializes_generates_and_exits_without_a_renderer() {
    let mut app = terra_server::headless_app(GameConfig::default());
    app.insert_resource(terra_network::server::ServerEndpoint {
        bind: ([127, 0, 0, 1], 0).into(),
        advertised: None,
    });
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    app.finish();
    app.cleanup();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.update();
        let generation = app.world().resource::<WorldGeneration>();
        if generation.status == GenerationStatus::Ready {
            break;
        }
        assert!(Instant::now() < deadline, "terrain worker timed out");
        assert!(!matches!(generation.status, GenerationStatus::Failed(_)));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(app.world().contains_resource::<WorldTerrain>());
    assert_eq!(
        *app.world().resource::<ConnectionStatus>(),
        ConnectionStatus::Unconfigured
    );
    let chunks = app.world().resource::<ChunkStore>();
    assert_eq!(chunks.loaded.len(), 27);
    assert!(chunks.loaded.values().all(|section| section.mesh.is_none()));
    assert!(
        chunks
            .loaded
            .values()
            .all(|section| section.water.is_none())
    );
    app.world_mut().write_message(AppExit::Success);
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

#[test]
fn integrated_server_owns_and_releases_a_real_loopback_socket() {
    let (host, token) = terra_server::IntegratedServer::start(GameConfig::default()).unwrap();
    assert_eq!(token.len(), 2048);
    assert!(host.address.ip().is_loopback());
    assert_ne!(host.address.port(), 0);
    assert!(std::net::UdpSocket::bind(host.address).is_err());
    let address = host.address;
    drop(host);
    assert!(std::net::UdpSocket::bind(address).is_ok());
}
