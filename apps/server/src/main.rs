mod console;
mod shutdown;
mod smoke_test;
use terra_core::{ClientConfig, Config, GameConfig, ServerConfig, WorldConfig, config_directory};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = config_directory();
    let mut server = ServerConfig::load(directory.join("server.toml"))?;
    let mut world = WorldConfig::load(directory.join("world.toml"))?;
    let mut bind_override = None;
    let mut advertised = None;
    let mut smoke = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--save-dir" => server.save_directory = Some(arguments.next().ok_or("--save-dir requires a path")?.into()),
            "--no-save" => server.save_directory = None,
            "--bind" => {
                bind_override = Some(
                    arguments
                        .next()
                        .ok_or("--bind requires an address")?
                        .parse::<std::net::SocketAddr>()?,
                )
            }
            "--advertise" => {
                advertised = Some(
                    arguments
                        .next()
                        .ok_or("--advertise requires an address")?
                        .parse::<std::net::SocketAddr>()?,
                )
            }
            "--token-out" => return Err("Join files are no longer needed. Clients connect by address using their UUID profile.".into()),
            "--smoke-test" => smoke = true,
            "--help" => {
                println!(
                    "terra-server [--bind IP:PORT] [--advertise IP:PORT] [--save-dir PATH | --no-save] [--smoke-test]\nTCP status/join uses the same port as UDP gameplay. LAN discovery: UDP 54545."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown server argument: {argument}").into()),
        }
    }
    server.validate()?;
    if advertised.is_some_and(|address| address.ip().is_unspecified() || address.port() == 0) {
        return Err("Advertised address must be reachable and have a nonzero port".into());
    }
    let bind = if smoke {
        ([127, 0, 0, 1], 0).into()
    } else {
        bind_override.unwrap_or(server.bind_address)
    };
    let save_path = (!smoke)
        .then_some(server.save_directory.as_ref())
        .flatten()
        .map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                directory.parent().unwrap_or(&directory).join(path)
            }
        });
    let persistence = if let Some(path) = save_path {
        world = terra_storage::DedicatedStore::resolve_world_config(&path, &world)
            .map_err(std::io::Error::other)?;
        Some(terra_storage::DedicatedStore::open(&path, &world).map_err(std::io::Error::other)?)
    } else {
        if world.ores.revision == 0 && !world.ores.enabled {
            world.ores = terra_core::OreGeneration::default();
        }
        world.geology_revision = 4;
        None
    };
    let world_id = world.id;
    let mut app = terra_server::headless_app(GameConfig {
        client: ClientConfig::default(),
        server,
        world,
    });
    if let Some((store, journal)) = persistence {
        terra_server::DedicatedPersistence::install(&mut app, store, journal, world_id);
    }
    app.insert_resource(terra_network::server::ServerEndpoint { bind, advertised });
    shutdown::install(&mut app)?;
    let _console = (!smoke).then(|| console::install(&mut app)).transpose()?;
    smoke_test::install(&mut app);
    if app.run().is_error() {
        return Err("Server exited with an error".into());
    }
    Ok(())
}
