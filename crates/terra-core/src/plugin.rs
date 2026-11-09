use crate::GameConfig;

pub struct TerraCorePlugin(pub GameConfig);
impl bevy::prelude::Plugin for TerraCorePlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(bevy::prelude::PreStartup, crate::initial_ticks::install);
        match crate::block_properties::BlockPropertiesCatalog::load(
            &crate::asset_directory().join("terra/blocks"),
        ) {
            Ok(catalog) => {
                app.insert_resource(catalog);
            }
            Err(error) => {
                bevy::log::error!(%error, "Cannot load block mechanics");
                app.init_resource::<crate::block_properties::BlockPropertiesCatalog>();
            }
        }
        let mut recipes = crate::crafting::RecipeBook::builtins().clone();
        let mut assets = crate::crafting::RecipeBook::default();
        match assets.load_directory(&crate::asset_directory().join("crafting")) {
            Ok(()) if assets.iter().next().is_some() => recipes = assets,
            Err(error) => recipes.error = Some(error),
            _ => {}
        }
        if let Err(error) = recipes.load_directory(&crate::config_directory().join("recipes")) {
            recipes.error = Some(error);
        }
        app.insert_resource(recipes);
        app.insert_resource(crate::WorldClock::new(self.0.world.time))
            .insert_resource(crate::WorldWeather::new(self.0.world.seed.0))
            .add_message::<crate::footsteps::Footstep>()
            .add_message::<crate::footsteps::WaterMotion>()
            .add_message::<crate::footsteps::UiSound>()
            .insert_resource(self.0.clone())
            .init_resource::<crate::DebugSettings>()
            .init_resource::<crate::HudVisibility>();
    }
}
