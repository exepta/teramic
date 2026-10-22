use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
use terra_player::{TargetWorld, TerraCamera, WorldTarget};
use terra_voxel::global_to_section;
use tilt_ui::html_shared;

#[html_shared]
#[derive(Resource, Default, serde::Serialize)]
pub struct DebugOverlay {
    pub visible: bool,
    pub fps: String,
    pub cpu: String,
    pub ram: String,
    pub chunk: String,
    pub xyz: String,
    pub block_name: String,
    pub biome: String,
    pub cave_visible: bool,
    pub cave_biome: String,
    pub cave_region: String,
    pub cave_conditions: String,
    pub cave_boundary: String,
    pub cave_connector: String,
    pub cave_local: String,
    pub cave_fluid: String,
    pub animal_visible: bool,
    pub animal_name: String,
    pub animal_health: String,
    pub animal_food: String,
    pub animal_water: String,
    pub animal_behavior: String,
}

pub fn update_debug_overlay(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    state: OverlayState,
    camera: Query<&Transform, With<TerraCamera>>,
    player: Query<&Transform, With<terra_player::PlayerBody>>,
    mut overlay: ResMut<DebugOverlay>,
    mut last_update: Local<f64>,
) {
    let allowed = state
            .menu
            .as_ref()
            .is_none_or(|menu| menu.world_hud_active());
    let visible = state.debug.enabled && allowed;
    let animal = state.creature_target.as_ref().and_then(|target| target.0)
        .and_then(|id| state.creatures.as_ref()?.creatures.get(&id))
        .map(|track| &track.current);
    let animal_visible = allowed && state.debug.creature_inspection && animal.is_some();
    if overlay.animal_visible != animal_visible {
        overlay.animal_visible = animal_visible;
    }
    if overlay.visible != visible {
        overlay.visible = visible;
    }
    if !visible && !overlay.animal_visible {
        return;
    }
    // Avoid formatting strings and invalidating HTML bindings on every render frame.
    if time.elapsed_secs_f64() - *last_update < 0.25 {
        return;
    }
    *last_update = time.elapsed_secs_f64();
    if let Some(animal) = animal.filter(|_| overlay.animal_visible) {
        overlay.animal_name = terra_core::creatures::definitions::get(animal.kind)
            .variant_title(animal.variant).into();
        overlay.animal_health = format!("{:.1} / {:.0}", animal.health, animal.kind.health());
        overlay.animal_food = if animal.kind.food() == 0.0 { "--".into() }
            else { format!("{:.1} / {:.0}", animal.needs.food, animal.kind.food()) };
        overlay.animal_water = if animal.kind.water() == 0.0 { "--".into() }
            else { format!("{:.1} / {:.0}", animal.needs.water, animal.kind.water()) };
        overlay.animal_behavior = match animal.behavior {
            terra_core::creatures::CreatureBehavior::Rest => "Ruht",
            terra_core::creatures::CreatureBehavior::Wander => "Laeuft",
            terra_core::creatures::CreatureBehavior::Graze => "Frisst",
            terra_core::creatures::CreatureBehavior::Flee => "Flieht",
            terra_core::creatures::CreatureBehavior::Swim => "Schwimmt",
            terra_core::creatures::CreatureBehavior::Stranded => "Gestrandet",
            terra_core::creatures::CreatureBehavior::Dead => "Tot",
            terra_core::creatures::CreatureBehavior::SeekFood => "Sucht Futter",
            terra_core::creatures::CreatureBehavior::SeekWater => "Sucht Wasser",
            terra_core::creatures::CreatureBehavior::Drink => "Trinkt",
            terra_core::creatures::CreatureBehavior::Shake => "Schuettelt sich",
            terra_core::creatures::CreatureBehavior::Fly => "Fliegt",
            terra_core::creatures::CreatureBehavior::Land => "Landet",
            terra_core::creatures::CreatureBehavior::Nectar => "Sammelt Nektar",
            terra_core::creatures::CreatureBehavior::Crawl => "Krabbelt",
            terra_core::creatures::CreatureBehavior::Sleep => "Schlaeft",
            terra_core::creatures::CreatureBehavior::InNest => "Im Nest",
            terra_core::creatures::CreatureBehavior::Leap => "Springt",
            terra_core::creatures::CreatureBehavior::Hunt => "Jagt Fische",
            terra_core::creatures::CreatureBehavior::Attack => "Greift an",
            terra_core::creatures::CreatureBehavior::Climb => "Klettert",
            terra_core::creatures::CreatureBehavior::Hold => "Haelt sich fest",
            terra_core::creatures::CreatureBehavior::Ink => "Versprueht Tinte",
        }.into();
        if let Some(traits) = animal.horse_traits() {
            overlay.animal_behavior = format!("{} / Ausdauer {} / Sprung {:.2} m", overlay.animal_behavior, traits.stamina, traits.jump_height);
        }
        if let Some(bee)=state.creatures.as_ref().and_then(|c|c.ecology.bees.get(&animal.id)) {
            overlay.animal_food=format!("Nektar {} / 100",bee.carried);
            overlay.animal_water=bee.flower.and_then(|id|state.creatures.as_ref().map(|c|format!("Bluete {} / 100",c.ecology.nectar(id)))).unwrap_or_else(||"Keine Bluete".into());
        }
    }
    overlay.block_name = match state.target.0 {
        Some(WorldTarget::Block(hit)) => state
            .chunks
            .block_at(hit.position)
            .map(|material| {
                state
                    .palette
                    .choices
                    .iter()
                    .find(|block| block.id == material)
                    .map(|block| block.name.clone())
                    .unwrap_or_else(|| format!("Block {material}"))
            })
            .unwrap_or_else(|| "--".into()),
        Some(WorldTarget::Prop(hit)) => match hit.prop.kind {
            terra_world::props::PropKind::Grass => "Grasbueschel",
            terra_world::props::PropKind::Flower => "Wildblumen",
            terra_world::props::PropKind::Fern => "Farn",
            terra_world::props::PropKind::Bush => "Busch",
            terra_world::props::PropKind::Mushroom => "Pilz",
            terra_world::props::PropKind::SmallPlant if hit.prop.variant >= 128 => "Kleiner Stein",
            terra_world::props::PropKind::SmallPlant => "Uferpflanze",
            terra_world::props::PropKind::BeeNest => "Bienennest",
            kind => terra_core::ResourcePropKind::from(kind).label(),
        }
        .into(),
        None => "--".into(),
    };
    overlay.fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .map(|fps| format!("{fps:.0}"))
        .unwrap_or_else(|| "--".into());
    overlay.cpu = state
        .process
        .cpu_percent
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "--".into());
    overlay.ram = state
        .process
        .resident_bytes
        .map(|bytes| format!("{:.0} MiB", bytes as f64 / 1048576.0))
        .unwrap_or_else(|| "--".into());
    if let Some(transform) = player.iter().next().or_else(|| camera.iter().next()) {
        let pos = transform.translation;
        let (chunk, _) = global_to_section([
            pos.x.floor() as i64,
            pos.y.floor() as i64,
            pos.z.floor() as i64,
        ]);
        overlay.chunk = format!("{}, {}, {}", chunk.x, chunk.y, chunk.z);
        overlay.xyz = format!("{:.1}  {:.1}  {:.1}", pos.x, pos.y, pos.z);
        overlay.cave_visible = false;
        if let Some(terrain) = &state.terrain {
            let p = [f64::from(pos.x), f64::from(pos.y), f64::from(pos.z)];
            overlay.cave_visible = terrain.0.config().cave_structure_revision >= 3
                && terrain.0.surface_height(p[0],p[2]) - p[1] > 5.0;
            if overlay.cave_visible {
                let cave = terrain.0.cave_ecology_sample(p,terrain.0.surface_column(p[0],p[2]));
                overlay.cave_biome = format!("{} / {}", cave.primary.label(), cave.secondary.map_or("--", |b| b.label()));
                overlay.cave_region = format!("{:016x}", cave.region_id);
                overlay.cave_conditions = format!("{:.0}% / {:.0}%", cave.blend*100.0, cave.moisture*100.0);
                overlay.cave_boundary = format!("{:.1} m", cave.boundary_distance);
                overlay.cave_connector = if terrain.0.cave_connector_at(p) { "Connector" } else { "Room / Rock" }.into();
                let temperature = terrain.0.biome_at(p[0], p[2]).temperature;
                let (props, minerals) = state.chunks.loaded.get(&chunk).map_or((0, 0), |s| {
                    (s.props.len(), s.data.blocks().iter().filter(|&&m| matches!(m, 51..=53)).count())
                });
                overlay.cave_local = format!("{temperature:.2} / {props} / {minerals}");
                overlay.cave_fluid = state.fluid.as_ref().filter(|_| {
                    state.access.as_ref().is_none_or(|a| a.authoritative())
                }).map_or_else(
                    || "Replica".into(),
                    |d| format!("{} / {:.2} ms", d.visited, d.elapsed_ms),
                );
            }
        }
        overlay.biome = state.terrain.as_ref().map_or_else(
            || "--".into(),
            |terrain| {
                use terra_world::generation::biomes::BiomeKind;
                match terrain
                    .0
                    .biome_at(f64::from(pos.x), f64::from(pos.z))
                    .weights
                    .dominant()
                {
                    BiomeKind::Ocean => "Ocean",
                    BiomeKind::Plains => "Plains",
                    BiomeKind::Forest => "Forest",
                    BiomeKind::Desert => "Desert",
                    BiomeKind::Highland => "Highland",
                    BiomeKind::MushroomPark => "Mushroom Park",
                    BiomeKind::ClifftedDesert => "Cliffted Desert",
                }
                .into()
            },
        );
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct OverlayState<'w> {
    debug: Res<'w, terra_core::DebugSettings>,
    menu: Option<Res<'w, crate::menu::MenuState>>,
    palette: Res<'w, terra_player::interaction::BlockPalette>,
    target: Res<'w, TargetWorld>,
    process: Res<'w, crate::AppProcessStats>,
    chunks: Res<'w, terra_world::streaming::ChunkStore>,
    terrain: Option<Res<'w, terra_world::streaming::WorldTerrain>>,
    fluid: Option<Res<'w, terra_world::water::FluidDiagnostics>>,
    access: Option<Res<'w, terra_world::streaming::WorldAccess>>,
    creature_target: Option<Res<'w, terra_player::interaction::TargetCreature>>,
    creatures: Option<Res<'w, terra_network::client::ReplicatedCreatures>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_player::interaction::{BlockChoice, BlockPalette};
    use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit};
    use terra_world::streaming::{ChunkStore, LoadedSection};
    #[test]
    fn targeted_animal_vitals_render_without_general_debug_and_hide_with_target_loss() {
        use terra_core::creatures::*;
        use terra_network::client::{CreatureTrack, ReplicatedCreatures};
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(300)))
            .init_resource::<DiagnosticsStore>().init_resource::<DebugOverlay>()
            .insert_resource(terra_core::DebugSettings { creature_inspection: true, ..default() })
            .init_resource::<crate::AppProcessStats>().init_resource::<ChunkStore>()
            .init_resource::<TargetWorld>().init_resource::<BlockPalette>()
            .init_resource::<terra_player::interaction::TargetCreature>()
            .init_resource::<ReplicatedCreatures>().add_systems(Update, update_debug_overlay);
        let id = creature_id([0,0],0).unwrap();
        let animal = Creature { id, home: [0,0], kind: CreatureKind::Sheep,
            position: [0.0,3.0,0.0], velocity: [0.0;3], yaw: 0.0, health: 7.0,
            behavior: CreatureBehavior::Drink, timer: 2.0, age: 1.0, variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Sheep) };
        app.world_mut().resource_mut::<ReplicatedCreatures>().creatures.insert(id,
            CreatureTrack { previous: animal.clone(), current: animal, received: std::time::Instant::now(), interval: 0.1 });
        app.world_mut().resource_mut::<terra_player::interaction::TargetCreature>().0 = Some(id);
        app.update(); app.update();
        let view = app.world().resource::<DebugOverlay>();
        assert!(!view.visible && view.animal_visible);
        assert_eq!((&*view.animal_name, &*view.animal_health, &*view.animal_food, &*view.animal_water), (definitions::get(CreatureKind::Sheep).variant_title(0), "7.0 / 7", "35.0 / 35", "25.0 / 25"));
        assert_eq!(view.animal_behavior, "Trinkt");
        for (kind, behavior, label) in [
            (CreatureKind::Squid, CreatureBehavior::Hold, "Haelt sich fest"),
            (CreatureKind::Squid, CreatureBehavior::Ink, "Versprueht Tinte"),
            (CreatureKind::MoonJelly, CreatureBehavior::Swim, "Schwimmt"),
            (CreatureKind::FireJelly, CreatureBehavior::Swim, "Schwimmt"),
            (CreatureKind::VoidJelly, CreatureBehavior::Swim, "Schwimmt"),
            (CreatureKind::SunJelly, CreatureBehavior::Swim, "Schwimmt"),
            (CreatureKind::CrystalJelly, CreatureBehavior::Swim, "Schwimmt"),
            (CreatureKind::ReefShark, CreatureBehavior::Attack, "Greift an"),
            (CreatureKind::HammerheadShark, CreatureBehavior::Hunt, "Jagt Fische"),
            (CreatureKind::WhaleShark, CreatureBehavior::Attack, "Greift an"),
        ] {
            {
                let mut animals = app.world_mut().resource_mut::<ReplicatedCreatures>();
                let animal = &mut animals.creatures.get_mut(&id).unwrap().current;
                animal.kind = kind;
                animal.behavior = behavior;
                animal.health = kind.health();
                animal.needs = CreatureNeeds::full(kind);
            }
            app.update();
            let view = app.world().resource::<DebugOverlay>();
            assert!(view.animal_visible && !view.visible);
            assert_eq!(view.animal_name, definitions::get(kind).variant_title(0));
            assert_eq!(view.animal_behavior, label);
            assert_eq!(view.animal_health, format!("{:.1} / {:.0}", kind.health(), kind.health()));
        }
        {
            let mut animals = app.world_mut().resource_mut::<ReplicatedCreatures>();
            let track = animals.creatures.get_mut(&id).unwrap();
            track.current.kind = CreatureKind::Fish;
            track.current.health = 3.0;
            track.current.needs = CreatureNeeds::full(CreatureKind::Fish);
        }
        app.update();
        let view = app.world().resource::<DebugOverlay>();
        assert_eq!((&*view.animal_food, &*view.animal_water), ("--", "--"));
        app.world_mut().resource_mut::<terra_player::interaction::TargetCreature>().0 = None;
        app.update();
        assert!(!app.world().resource::<DebugOverlay>().animal_visible);
    }
    #[test]
    fn hud_uses_target_material_and_player_position_with_offline_camera_fallback() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(FrameTimeDiagnosticsPlugin::default())
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs_f64(1.0 / 60.0),
            ))
            .init_resource::<DebugOverlay>()
            .insert_resource(terra_core::DebugSettings {
                enabled: true,
                ..default()
            })
            .init_resource::<crate::AppProcessStats>()
            .init_resource::<ChunkStore>()
            .init_resource::<TargetWorld>()
            .insert_resource(BlockPalette {
                choices: vec![
                    BlockChoice {
                        id: 1,
                        name: "Meadow Grass".into(),
                        identifier: "teramic::grass".into(),
                        description: String::new(),
                    },
                    BlockChoice {
                        id: 3,
                        name: "Stone".into(),
                        identifier: "teramic::stone".into(),
                        description: String::new(),
                    },
                ],
                selected: 1,
                prop_descriptions: default(),
                ..default()
            })
            .add_systems(Update, update_debug_overlay);
        let position = SectionPos::new(-1, 0, -1);
        let mut data = BlockSection::empty(position).unwrap();
        data.set_block([15, 0, 15], 1).unwrap();
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                props: Vec::new(),
                water: None,
            },
        );
        app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(BlockHit {
            position: [-1, 0, -1],
            normal: [0, 1, 0],
            distance: 2.0,
        }));
        app.world_mut()
            .spawn((TerraCamera, Transform::from_xyz(-0.5, 2.0, -0.5)));
        for _ in 0..30 {
            app.update();
        }
        let overlay = app.world().resource::<DebugOverlay>();
        assert_eq!(overlay.block_name, "Meadow Grass");
        assert_eq!(overlay.chunk, "-1, 0, -1");
        assert_eq!(overlay.xyz, "-0.5  2.0  -0.5");
        assert!(overlay.visible);
        app.world_mut()
            .resource_mut::<terra_core::DebugSettings>()
            .enabled = false;
        app.update();
        assert!(!app.world().resource::<DebugOverlay>().visible);
        app.world_mut()
            .resource_mut::<terra_core::DebugSettings>()
            .enabled = true;
        app.world_mut().resource_mut::<TargetWorld>().0 = None;
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(app.world().resource::<DebugOverlay>().block_name, "--");

        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let camera_biome = terrain.biome_at(-0.5, -0.5).weights.dominant();
        let player_position = (-16..=16)
            .flat_map(|x| (-16..=16).map(move |z| (x, z)))
            .map(|(x, z)| Vec3::new(x as f32 * 512.0 + 0.5, 7.0, z as f32 * 512.0 + 0.5))
            .find(|position| {
                terrain
                    .biome_at(f64::from(position.x), f64::from(position.z))
                    .weights
                    .dominant()
                    != camera_biome
            })
            .expect("fixture needs distinct camera and player biomes");
        let player_biome = format!(
            "{:?}",
            terrain
                .biome_at(f64::from(player_position.x), f64::from(player_position.z))
                .weights
                .dominant()
        );
        app.insert_resource(terra_world::streaming::WorldTerrain(terrain));
        app.world_mut().spawn((
            terra_player::PlayerBody,
            Transform::from_translation(player_position),
        ));
        for _ in 0..30 {
            app.update();
        }
        let overlay = app.world().resource::<DebugOverlay>();
        assert_eq!(overlay.biome, player_biome);
        assert_eq!(
            overlay.xyz,
            format!(
                "{:.1}  {:.1}  {:.1}",
                player_position.x, player_position.y, player_position.z
            )
        );
        let (section, _) =
            global_to_section(player_position.to_array().map(|value| value.floor() as i64));
        assert_eq!(
            overlay.chunk,
            format!("{}, {}, {}", section.x, section.y, section.z)
        );

        app.init_resource::<crate::inventory::InventoryState>();
        app.world_mut()
            .resource_mut::<crate::inventory::InventoryState>()
            .open = true;
        app.update();
        assert!(app.world().resource::<DebugOverlay>().visible);
        assert!(app.world().resource::<terra_core::DebugSettings>().enabled);
        app.world_mut()
            .resource_mut::<crate::inventory::InventoryState>()
            .open = false;
        app.update();
        assert!(app.world().resource::<DebugOverlay>().visible);

        app.init_resource::<crate::SettingsView>();
        app.world_mut().resource_mut::<crate::SettingsView>().open = true;
        app.update();
        assert!(app.world().resource::<DebugOverlay>().visible);
        app.world_mut().resource_mut::<crate::SettingsView>().open = false;
        app.insert_resource(crate::menu::MenuState {
            page: crate::menu::MenuPage::Pause,
            session_active: true,
            ..default()
        });
        app.update();
        assert!(app.world().resource::<DebugOverlay>().visible);
        app.world_mut()
            .resource_mut::<crate::menu::MenuState>()
            .page = crate::menu::MenuPage::Playing;
        app.update();
        assert!(app.world().resource::<DebugOverlay>().visible);
        assert!(app.world().resource::<terra_core::DebugSettings>().enabled);
    }
}
