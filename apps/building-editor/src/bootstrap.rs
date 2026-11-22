use bevy::{prelude::*, window::PresentMode};
use terra_core::{ControlsConfig, GameConfig, GameplayConfig, GraphicsConfig, TerraCorePlugin};

pub fn app() -> Result<App, Box<dyn std::error::Error>> {
    let mut config = GameConfig::default();
    config.client.width = 1440;
    config.client.height = 900;
    config.client.view_distance = 2;
    config.client.vertical_view_distance = 2;
    config.client.fog_distance = 0.0;
    config.client.fly_speed = 8.0;
    config.world.base_height = -100_000.0;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.sea_level = -100_000.0;
    config.world.biomes = false;
    config.world.caves = false;
    config.world.trees = false;
    config.world.water = false;
    config.world.rivers = false;
    config.world.landscape_revision = 0;
    config.world.ecology_revision = 0;
    config.world.geology_revision = 0;
    config.world.structures_revision = 0;
    config.world.cave_structure_revision = 0;
    config.world.structures.set_all(false);
    let mut app = App::new();
    // TiltUI registers its asset source before AssetPlugin. Its local build()
    // explicitly requires this order; finish() installs runtime systems after
    // DefaultPlugins has initialized AssetServer and the Bevy UI resources.
    app.add_plugins(
        tilt_ui::TiltUiPlugin::new(crate::tilt_ui_component_catalog())
            .with_source_root(terra_core::installation_root().join("apps/building-editor/ui"))
            .without_camera(),
    );
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: terra_core::asset_directory().to_string_lossy().into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                close_when_requested: false,
                primary_window: Some(Window {
                    title: "TERAMIC · Building Editor".into(),
                    resolution: (1440, 900).into(),
                    present_mode: PresentMode::AutoVsync,
                    ..default()
                }),
                ..default()
            }),
    );
    app.add_plugins(terra_ui::MenuLayoutPlugin);
    let graphics = GraphicsConfig::from_client(&config.client);
    app.insert_resource(graphics)
        .insert_resource(GameplayConfig::default())
        .insert_resource(ControlsConfig::default())
        .add_plugins(TerraCorePlugin(config))
        .add_plugins(terra_world::TerraWorldPlugin {
            mode: terra_world::WorldMode::Client,
        })
        .add_plugins(terra_physics::TerraPhysicsPlugin)
        .add_plugins(terra_player::TerraPlayerPlugin)
        .add_plugins(terra_render::TerraRenderPlugin)
        .insert_resource(terra_player::ControllerMode::Fly)
        // The editor owns atomic object-aware authoring transactions. The shared
        // controller still provides capture, movement, targeting and catalog assets.
        .insert_resource(terra_player::WorldInteractionBlock(true))
        .insert_resource(terra_world::streaming::ChunkFocus([10.0, 7.0, 13.0]))
        .add_message::<terra_network::client::InventoryRequest>()
        .add_message::<terra_network::client::TreeHarvestRequest>()
        .init_resource::<crate::document::Document>()
        .init_resource::<crate::workspace::WorkspacePlatform>()
        .init_resource::<crate::editor::Editor>()
        .init_resource::<crate::interface::EditorView>()
        .add_message::<crate::editor::EditorAction>()
        .add_systems(PreStartup, crate::scene::seed)
        .add_systems(PostStartup, crate::scene::camera)
        .add_systems(
            PreUpdate,
            (crate::editor::keyboard, crate::editor::close_requested)
                .after(bevy::input::InputSystems),
        )
        .add_systems(
            Update,
            (
                crate::editor::actions,
                crate::editor::interact,
                crate::interface::project,
            )
                .chain()
                .after(terra_player::PlayerSet::Interaction)
                .before(terra_world::streaming::StreamingSet::Interest),
        )
        .add_systems(Update, crate::scene::outline)
        .add_systems(
            PostUpdate,
            crate::interface::bind_previews.before(bevy::ui::UiSystems::Prepare),
        )
        .add_systems(Update, crate::smoke::run);
    Ok(app)
}
