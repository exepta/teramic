mod appearance_smoke;
mod avatar_smoke;
mod biome_smoke;
mod bootstrap;
mod botanical_smoke;
mod breeding_smoke;
mod building_smoke;
mod camera_smoke;
mod cave_showcase;
mod cave_structures_smoke;
mod chat_smoke;
mod crafting_smoke;
mod creature_audio;
mod creature_smoke;
mod crusher_audio;
mod crusher_smoke;
mod custom_smoke;
mod daylight_smoke;
mod diagnostic_report;
mod drop_smoke;
mod ecology_smoke;
mod equipment_smoke;
mod farming_smoke;
mod flower_wither_smoke;
mod fluid_smoke;
mod fluid_source_smoke;
mod foliage_smoke;
mod food_smoke;
mod footsteps;
mod furnace_smoke;
mod geology_smoke;
mod hand_smoke;
mod hud_smoke;
mod hydrology_smoke;
mod inventory_benchmark;
mod inventory_smoke;
mod loading_smoke;
mod materials_smoke;
mod menu_smoke;
mod minimap_smoke;
mod mob_health_smoke;
mod network_smoke;
mod ore_smoke;
mod performance_edits;
mod performance_route;
mod performance_soak;
mod pet_commands_smoke;
mod physics_smoke;
mod reef_smoke;
mod riding_smoke;
mod season_smoke;
mod sessions;
mod settings_smoke;
mod smoke_test;
mod spider_smoke;
mod stalker_smoke;
mod status_effect_smoke;
mod surface_smoke;
mod survival_smoke;
mod systems_smoke;
mod taming_smoke;
mod track_smoke;
mod tree_collapse_smoke;
mod ui_audio;
mod village_smoke;
mod water_audio;
mod water_contacts;
mod waterfall_smoke;
mod web_constructions_smoke;
mod window_gallery_smoke;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|argument| argument == "--help") {
        println!(
            "terra-client [--connect IP:PORT] [--profile NAME] [--gender=f|m]\nDefault: TERAMIC main menu. Persistent local identity: configs/profile.toml.\n--profile NAME selects configs/profiles/NAME.toml with its own persistent UUID for local multi-client testing.\n--gender=f or --gender=m selects and remembers the female or male avatar for that profile.\nAlternatively override the identity file with TERRA_PROFILE_FILE. Duplicate UUID protection remains enabled."
        );
        return Ok(());
    }
    let exit = bootstrap::build_app()?.run();
    if exit.is_error() {
        return Err("Client exited with an error".into());
    }
    Ok(())
}
