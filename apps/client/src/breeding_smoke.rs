//! Short isolated real-UDP reproduction of extra-feed breeding and physical maturation.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    CropItem, GameConfig, GameMode, ItemKind, PlayerProfile,
    creatures::{
        Creature, CreatureBehavior, CreatureKind, CreatureNeeds,
        breeding::{self, AnimalFamily},
        taming::AnimalBond,
    },
};
use terra_network::client::{ClientSession, CreatureActions, ReplicatedCreatures};

const A: u64 = (1 << 62) + 115;
const B: u64 = (1 << 62) + 120;
const MATURING: u64 = (1 << 60) + 1;
const NEWBORN: u64 = MATURING + 1;
const WHEAT: ItemKind = ItemKind::Crop(CropItem::Wheat);

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Creative;
    config.server.natural_mob_limit = Some(0);
    config.world.biomes = false;
    config.world.landscape_revision = 0;
    config.world.trees = false;
    config.world.caves = false;
    config.world.rivers = false;
    config.world.water = false;
    config.world.base_height = 16.0;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    let path = std::env::var_os("TERRA_BREEDING_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("terra-breeding-{}", std::process::id()))
        });
    if path.exists() {
        return Err("Breeding fixture requires a fresh isolated save".into());
    }
    let profile = PlayerProfile::load_for_client(
        PlayerProfile::selected_path(&terra_core::menu_config_directory())
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut journal = terra_world::edits::EditJournal::default();
    let terrain =
        terra_world::TerraGenerator::new(config.world.clone()).map_err(|e| e.to_string())?;
    let ground = terra_world::spawn::find_land_spawn(&terrain, &journal, [8, 24])?.ground;
    let state = journal.gameplay_mut();
    if terra_core::creatures::CreatureSex::from_spawn_id(A)
        == terra_core::creatures::CreatureSex::from_spawn_id(B)
    {
        return Err("Prepared breeding parents must have opposite sex".into());
    }
    for (id, dx, dz) in [(A, -0.9, -2.7), (B, 1.9, -2.7), (MATURING, 3.5, -1.0)] {
        let kind = CreatureKind::Cow;
        let position = [
            ground[0] as f64 + dx,
            ground[1] as f64 + 1.0,
            ground[2] as f64 + dz,
        ];
        state.creatures.animals.insert(
            id,
            Creature {
                id,
                kind,
                home: [
                    (position[0].floor() as i64).div_euclid(32),
                    (position[2].floor() as i64).div_euclid(32),
                ],
                position,
                velocity: [0.0; 3],
                yaw: std::f32::consts::PI,
                health: kind.health(),
                behavior: CreatureBehavior::Rest,
                timer: 60.0,
                age: if id == MATURING {
                    breeding::GROWTH_SECONDS - 30.0
                } else {
                    10.0
                },
                variant: 0,
                needs: CreatureNeeds::full(kind),
            },
        );
        state.creatures.owners.insert(id, profile.uuid);
        state
            .animal_bonds
            .insert(id, AnimalBond::owned(profile.uuid));
    }
    state.animal_families.insert(
        MATURING,
        AnimalFamily {
            parents: Some([A, B]),
            active_age: breeding::GROWTH_SECONDS - 30.0,
            ready_age: breeding::GROWTH_SECONDS,
            willing_until_age: 0.0,
        },
    );
    state.next_animal_birth = 1;
    state
        .inventories
        .entry(profile.uuid)
        .or_default()
        .insert(WHEAT, 6);
    if !state.valid() {
        return Err("Invalid prepared breeding population".into());
    }
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: u8,
    captures: u8,
    blank: bool,
    selected: bool,
    drained: u8,
    young_age: f64,
    young_scale: f64,
}
impl Inspection {
    fn target(&self) -> u64 {
        target_for_stage(self.stage)
    }
    fn advance(&mut self, stage: u8) {
        self.stage = stage;
        self.since = Instant::now();
        info!(stage, "Native breeding stage");
    }
}
fn target_for_stage(stage: u8) -> u64 {
    match stage {
        0 | 1 => A,
        2 | 3 => B,
        4 | 5 => NEWBORN,
        _ => MATURING,
    }
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-breeding") {
        return;
    }
    app.insert_resource(Inspection {
        start: Instant::now(),
        since: Instant::now(),
        stage: 0,
        captures: 0,
        blank: false,
        selected: false,
        drained: 0,
        young_age: 0.0,
        young_scale: 0.0,
    })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .insert_resource(terra_player::ControllerMode::Walk)
    .add_systems(PostStartup, setup)
    .add_systems(PreUpdate, aim.after(bevy::input::InputSystems))
    .add_systems(Last, run);
}
fn setup(world: &mut World) {
    let mut menu = world.resource_mut::<terra_ui::menu::MenuState>();
    menu.page = terra_ui::menu::MenuPage::Playing;
    menu.session_active = true;
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world
        .resource_mut::<terra_core::GameplayConfig>()
        .minimap
        .enabled = false;
    world.write_message(terra_player::CaptureRequest(true));
}
fn aim(world: &mut World) {
    world
        .resource_mut::<bevy::input::mouse::AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    for mut window in world
        .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
        .iter_mut(world)
    {
        window.bypass_change_detection().focused = true;
    }
    world
        .resource_mut::<terra_player::CameraInputState>()
        .captured = true;
    let id = world.resource::<Inspection>().target();
    let point = world
        .resource::<ReplicatedCreatures>()
        .creatures
        .get(&id)
        .map(|t| {
            Vec3::from_array(t.current.position.map(|v| v as f32))
                + Vec3::Y * t.current.size()[1] as f32 * 0.5
        });
    let Some(point) = point else { return };
    let eye = world
        .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
        .iter(world)
        .next()
        .map(|body| body.translation + Vec3::Y * terra_player::PLAYER_EYE_OFFSET);
    for (mut transform, mut look) in world.query_filtered::<(&mut Transform, &mut terra_player::CameraLook), With<terra_player::TerraCamera>>().iter_mut(world) {
        let rotation = Transform::from_translation(eye.unwrap_or(transform.translation)).looking_at(point, Vec3::Y).rotation;
        transform.rotation = rotation;
        if !look.rotation().abs_diff_eq(rotation, 0.000_01) { *look = terra_player::CameraLook::default(); }
    }
}
fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .ok_or("Set TERRA_CAPTURE_DIR")?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("breeding-{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                state.blank |= event.image.data.as_ref().is_none_or(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .step_by(16)
                        .filter(|p| p[..3].iter().any(|v| *v > 30))
                        .count()
                        < 1_000
                });
                state.captures += 1;
            },
        );
    Ok(())
}
fn aim_ready(world: &mut World, id: u64) -> bool {
    let Some(point) = world
        .resource::<ReplicatedCreatures>()
        .creatures
        .get(&id)
        .map(|track| {
            Vec3::from_array(track.current.position.map(|v| v as f32))
                + Vec3::Y * track.current.size()[1] as f32 * 0.5
        })
    else {
        return false;
    };
    let Some(eye) = world
        .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
        .iter(world)
        .next()
        .map(|body| body.translation + Vec3::Y * terra_player::PLAYER_EYE_OFFSET)
    else {
        return false;
    };
    let direction = (point - eye).normalize_or_zero();
    let local = world
        .query_filtered::<&terra_player::CameraLook, With<terra_player::TerraCamera>>()
        .iter(world)
        .next()
        .is_some_and(|look| (look.rotation() * Vec3::NEG_Z).dot(direction) >= 0.999_5);
    let look = world.resource::<ClientSession>().look;
    local
        && (Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z).dot(direction)
            >= 0.999_5
}
fn hud_matches(world: &mut World, id: u64, growing: bool) -> bool {
    let Some(sex) = world
        .resource::<ReplicatedCreatures>()
        .creatures
        .get(&id)
        .map(|track| track.current.sex().label())
    else {
        return false;
    };
    let view = world.resource::<terra_ui::TargetView>();
    if !view.visible
        || !view.mob
        || world.resource::<terra_player::TargetCreature>().0 != Some(id)
        || !view
            .stats
            .iter()
            .any(|r| r.id == "animal-owner" && r.text == "Du")
        || !view
            .stats
            .iter()
            .any(|r| r.id == "animal-sex" && r.text == sex)
        || growing
            != view
                .stats
                .iter()
                .any(|r| r.id == "animal-growth" && r.bar && r.value < 100.0)
    {
        return false;
    }
    let card = world
        .query::<(Entity, &tilt_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .find(|(_, id, node)| id.0 == "target-card" && !node.is_empty())
        .map(|(e, _, _)| e);
    let Some(card) = card else { return false };
    let mut pending = vec![card];
    let mut labels = Vec::new();
    while let Some(entity) = pending.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
        if let Some(text) = world.get::<Text>(entity) {
            labels.push(text.0.trim().to_owned());
        }
    }
    labels.iter().any(|s| s == "Du")
        && labels.iter().any(|s| s == sex)
        && labels.iter().any(|s| s == "Wachstum") == growing
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        if state.start.elapsed() > Duration::from_secs(90) {
            error!(stage = state.stage, "Native breeding timed out");
            world.write_message(AppExit::error());
            return;
        }
        if let Err(error) = step(world, &mut state) {
            error!(%error, stage = state.stage, "Native breeding failed");
            world.write_message(AppExit::error());
        }
    });
}
fn step(world: &mut World, state: &mut Inspection) -> Result<(), String> {
    if state.blank {
        return Err("Blank breeding screenshot".into());
    }
    let aimed = aim_ready(world, state.target());
    let parent_hud_ready = state.stage == 1 && hud_matches(world, A, false);
    let session = world.resource::<ClientSession>();
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    if !session.ready
        || session.teleporting
        || !world.resource::<terra_render::RenderStatus>().ready
    {
        return Ok(());
    }
    if !state.selected {
        world.write_message(terra_ui::InventoryAction::Hotbar(0));
        state.selected = true;
        return Ok(());
    }
    let inventory = session.inventory.count(WHEAT);
    let owner = session.uuid;
    let animals = world.resource::<ReplicatedCreatures>();
    if matches!(state.stage, 1 | 3) && state.since.elapsed() > Duration::from_secs(5) {
        return Err(format!(
            "Authoritative feed/birth acknowledgement timed out: stage {}, wheat {} expected {}, inventory revision {}, target {:?}, aimed {}, session look {:?}, parent A {:?}, parent B {:?}, newborn {}",
            state.stage,
            inventory,
            if state.stage == 1 { 5 } else { 4 },
            session.inventory.revision,
            world.resource::<terra_player::TargetCreature>().0,
            aimed,
            session.look,
            animals.families.get(&A),
            animals.families.get(&B),
            animals.creatures.contains_key(&NEWBORN),
        ));
    }
    match state.stage {
        0 if inventory == 6
            && aimed
            && world.resource::<terra_player::TargetCreature>().0 == Some(A)
            && world.resource::<terra_player::interaction::ActiveItem>().0 == Some(WHEAT) =>
        {
            world
                .resource_mut::<CreatureActions>()
                .0
                .push_back((A, true));
            state.advance(1);
        }
        1 if inventory == 5
            && animals.families.get(&A).is_some_and(|f| f.willing)
            && parent_hud_ready =>
        {
            capture(world, "parents")?;
            state.advance(2);
        }
        2 if inventory == 5
            && state.since.elapsed() >= Duration::from_millis(500)
            && aimed
            && world.resource::<terra_player::TargetCreature>().0 == Some(B) =>
        {
            world
                .resource_mut::<CreatureActions>()
                .0
                .push_back((B, true));
            state.advance(3);
        }
        3 if inventory == 4 && animals.creatures.contains_key(&NEWBORN) => {
            state.advance(4);
        }
        4 => {
            let Some(child) = animals.creatures.get(&NEWBORN).map(|t| &t.current) else {
                return Ok(());
            };
            if inventory != 4 {
                return Ok(());
            }
            if !child.juvenile()
                || child.growth_scale() > 0.5
                || animals
                    .families
                    .get(&NEWBORN)
                    .is_none_or(|f| f.parents != Some([A, B]))
                || animals.bonds.get(&NEWBORN).is_none_or(|b| b.owner != owner)
                || [A, B].into_iter().any(|id| {
                    animals
                        .families
                        .get(&id)
                        .is_none_or(|f| f.cooldown_seconds < 290.0 || f.willing)
                })
            {
                return Err(
                    "Actual breeding ownership/growth/parent cooldown snapshot was incorrect"
                        .into(),
                );
            }
            if animals
                .creatures
                .get(&A)
                .zip(animals.creatures.get(&B))
                .is_none_or(|(a, b)| a.current.sex() == b.current.sex())
            {
                return Err("Native breeding bypassed opposite-sex requirement".into());
            }
            if !hud_matches(world, NEWBORN, true) {
                return Ok(());
            }
            capture(world, "newborn")?;
            state.advance(5);
        }
        5 if state.since.elapsed() > Duration::from_secs(2) && state.captures >= 2 => {
            if inventory != 4
                || animals
                    .creatures
                    .values()
                    .filter(|t| breeding::is_birth_id(t.current.id))
                    .count()
                    != 2
            {
                return Err("Breeding reproduced again or consumed unexpected food".into());
            }
            capture(world, "stable-newborn")?;
            state.advance(6);
        }
        6 if state.captures >= 3 => {
            let Some(child) = animals.creatures.get(&MATURING).map(|t| &t.current) else {
                return Ok(());
            };
            if !child.juvenile() {
                return Err("Maturing fixture reached adulthood before observation".into());
            }
            state.young_age = child.age;
            state.young_scale = child.growth_scale();
            if !hud_matches(world, MATURING, true) {
                return Ok(());
            }
            capture(world, "growing")?;
            state.advance(7);
        }
        7 => {
            let Some(child) = animals.creatures.get(&MATURING).map(|t| &t.current) else {
                return Ok(());
            };
            if child.age < state.young_age || child.growth_scale() < state.young_scale {
                return Err("Growth rewound".into());
            }
            if child.juvenile() {
                return Ok(());
            }
            if child.growth_scale() != 1.0 || !hud_matches(world, MATURING, false) {
                return Ok(());
            }
            capture(world, "adult")?;
            state.advance(8);
        }
        8 if state.captures == 5 && state.since.elapsed() > Duration::from_millis(600) => {
            if inventory != 4 {
                return Err("Breeding consumed unexpected feed".into());
            }
            let pending = world.query::<&Screenshot>().iter(world).count();
            state.drained = if pending == 0 {
                state.drained.saturating_add(1)
            } else {
                0
            };
            if state.drained >= 2 {
                info!(
                    captures = state.captures,
                    food_consumed = 2,
                    "Native F06 passed: real owner feeds, single birth, parents/UUID/cooldown replication and physical maturation"
                );
                world.write_message(AppExit::Success);
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_interactions_hold_the_same_aim_until_authoritative_acknowledgement() {
        assert_ne!(
            terra_core::creatures::CreatureSex::from_spawn_id(A),
            terra_core::creatures::CreatureSex::from_spawn_id(B)
        );
        assert_eq!(target_for_stage(0), A);
        assert_eq!(target_for_stage(1), A);
        assert_eq!(target_for_stage(2), B);
        assert_eq!(target_for_stage(3), B);
        assert_eq!(target_for_stage(4), NEWBORN);
        assert_eq!(target_for_stage(5), NEWBORN);
    }
}
