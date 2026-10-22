//! Short native F01 checks against real integrated-server commands and replicated vitals.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{EffectKind, GameConfig, GameMode, PlayerProfile, PlayerStats, SurvivalVitals};

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Survival;
    config.server.natural_mob_limit = Some(0);
    config.world.biomes = false;
    config.world.trees = false;
    config.world.caves = false;
    config.world.rivers = false;
    config.world.water = false;
    config.world.base_height = 16.0;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    let path = std::env::temp_dir().join(format!("terra-status-effects-{}", std::process::id()));
    if path.exists() {
        return Err("Status-effect fixture needs a fresh save directory".into());
    }
    let profile_path = PlayerProfile::selected_path(&terra_core::menu_config_directory())
        .map_err(|error| error.to_string())?;
    let profile =
        PlayerProfile::load_for_client(profile_path).map_err(|error| error.to_string())?;
    let mut permissions = config.server.permissions_for(profile.uuid);
    permissions.environment = true;
    config
        .server
        .player_permissions
        .insert(profile.uuid, permissions);
    let mut journal = terra_world::edits::EditJournal::default();
    journal.update_player_stats(
        profile.uuid,
        PlayerStats {
            survival: SurvivalVitals {
                health: 92.0,
                hunger: 50.0,
                ..default()
            },
            ..default()
        },
    );
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: usize,
    configured: bool,
    pending: bool,
    baseline_nodes: Option<usize>,
    commands: VecDeque<String>,
    next_command: Instant,
}

const STAGES: [&str; 8] = [
    "poison-1280",
    "freeze-1280",
    "regeneration-1280",
    "all-1280",
    "clear-1280",
    "all-1920",
    "all-compact",
    "clear-compact",
];

fn effects_for_stage(stage: usize) -> &'static [EffectKind] {
    match stage {
        0 => &[EffectKind::Poison],
        1 => &[EffectKind::Freeze],
        2 => &[EffectKind::Regeneration],
        3 | 5 | 6 => &EffectKind::ALL,
        _ => &[],
    }
}

fn requested_effects(stage: usize) -> Vec<(EffectKind, u8)> {
    let mut effects = effects_for_stage(stage).to_vec();
    let all = effects.len() == EffectKind::ALL.len();
    if all {
        // Protect the real player while the rate-limited command sequence arrives.
        effects.sort_by_key(|effect| match effect {
            EffectKind::Regeneration => 0,
            EffectKind::FireResistance => 1,
            EffectKind::FreezeResistance => 2,
            EffectKind::ShieldPower => 3,
            _ => 4 + effect.index(),
        });
    }
    effects
        .into_iter()
        .map(|effect| {
            (
                effect,
                if all && effect == EffectKind::Regeneration {
                    3
                } else {
                    1
                },
            )
        })
        .collect()
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-effects") {
        return;
    }
    app.insert_resource(terra_player::ControllerMode::Walk)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Inspection {
            start: Instant::now(),
            since: Instant::now(),
            stage: 0,
            configured: false,
            pending: false,
            baseline_nodes: None,
            commands: VecDeque::new(),
            next_command: Instant::now(),
        })
        .add_systems(PostStartup, setup)
        .add_systems(PreUpdate, focus.after(bevy::input::InputSystems))
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

fn focus(mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>) {
    for mut window in &mut windows {
        window.bypass_change_detection().focused = true;
    }
}

fn configure(world: &mut World, stage: usize) {
    let (width, height) = match stage {
        5 => (1920, 1080),
        6 | 7 => (360, 800),
        _ => (1280, 800),
    };
    for mut window in world
        .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
        .iter_mut(world)
    {
        window.resolution.set(width as f32, height as f32);
    }
    let mut state = world.resource_mut::<Inspection>();
    // Respect the real server's one-command-per-second token bucket.
    if stage != 6 {
        state.commands.push_back("/effect clear".into());
        for (effect, level) in requested_effects(stage) {
            state
                .commands
                .push_back(format!("/effect give {} 120 {level}", effect.identifier()));
        }
    }
    state.next_command = Instant::now();
    state.configured = true;
    state.since = Instant::now();
}

fn check(world: &mut World, stage: usize) -> Result<(), String> {
    let expected = effects_for_stage(stage);
    let vitals = world
        .resource::<terra_network::client::ClientSession>()
        .stats
        .survival;
    let actual: Vec<_> = vitals.active_effects().map(|effect| effect.kind).collect();
    if vitals.health <= 0.0
        || actual.len() != expected.len()
        || expected.iter().any(|effect| !actual.contains(effect))
    {
        return Err(format!(
            "Wrong authoritative effects: expected {expected:?}, received {actual:?}, health={}",
            vitals.health
        ));
    }
    let view = world.resource::<terra_ui::StatusEffectView>();
    if view.cards.len() + view.overflow != expected.len()
        || view
            .cards
            .iter()
            .any(|card| !expected.iter().any(|effect| effect.identifier() == card.id))
    {
        return Err(format!(
            "Wrong HUD effect cards: cards={}, overflow={}, expected={}",
            view.cards.len(),
            view.overflow,
            expected.len()
        ));
    }
    let card_count = view.cards.len();
    let has_overflow = view.overflow > 0;
    let rendered = world.resource::<terra_ui::VitalRenderStatus>();
    if !rendered.bound
        || (stage == 0 && !rendered.poison)
        || (stage == 1 && !rendered.freeze)
        || (stage == 2 && !rendered.regeneration)
        || (expected.is_empty() && (rendered.poison || rendered.freeze || rendered.regeneration))
    {
        return Err("Wrong native vital material effect flags".into());
    }
    let viewport = world
        .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
        .single(world)
        .map_err(|error| error.to_string())?
        .physical_size()
        .as_vec2();
    let cards: Vec<_> = world
        .query::<(
            &tilt_ui::ElementClasses,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
        .filter(|(classes, _, _)| {
            classes
                .classes
                .iter()
                .any(|class| class == "status-effect-card")
        })
        .map(|(_, node, transform)| Rect::from_center_size(transform.translation, node.size()))
        .collect();
    if cards.len() != card_count + usize::from(has_overflow) {
        return Err(format!(
            "HUD card binding mismatch: {} real nodes for {} cards and overflow={has_overflow}",
            cards.len(),
            card_count
        ));
    }
    for card in &cards {
        if card.size().min_element() < 10.0
            || card.min.cmplt(Vec2::splat(-1.0)).any()
            || card.max.cmpgt(viewport + Vec2::ONE).any()
        {
            return Err(format!(
                "Effect card outside viewport: {card:?}, {viewport:?}"
            ));
        }
    }
    if viewport.x < 640.0 {
        let target = world
            .query::<(
                &terra_ui::ElementId,
                &ComputedNode,
                &bevy::ui::UiGlobalTransform,
            )>()
            .iter(world)
            .find(|(id, node, _)| id.0 == "target-card" && !node.is_empty())
            .map(|(_, node, transform)| Rect::from_center_size(transform.translation, node.size()));
        if target.is_some_and(|target| cards.iter().any(|card| !card.intersect(target).is_empty()))
        {
            return Err("Compact target card overlaps an effect card".into());
        }
    }
    let icon_nodes: Vec<_> = world
        .query::<(&tilt_ui::ElementClasses, &bevy::ui::widget::ImageNode)>()
        .iter(world)
        .filter(|(classes, _)| {
            classes
                .classes
                .iter()
                .any(|class| class == "status-effect-icon")
        })
        .map(|(_, image)| (image.image.clone(), image.rect))
        .collect();
    if icon_nodes.len() != card_count {
        return Err("Effect icons did not bind to the actual card tree".into());
    }
    for (handle, rect) in icon_nodes {
        let image = world
            .resource::<Assets<Image>>()
            .get(&handle)
            .ok_or("Missing effect icon atlas")?;
        let rect = rect.ok_or("Missing effect icon atlas rectangle")?;
        if rect.min.cmplt(Vec2::ZERO).any()
            || rect.max.cmpgt(image.size().as_vec2()).any()
            || rect.size().min_element() < 1.0
        {
            return Err("Invalid effect atlas rectangle".into());
        }
    }
    let nodes = world.query::<&Node>().iter(world).count();
    let baseline = world.resource::<Inspection>().baseline_nodes;
    if baseline.is_some_and(|baseline| nodes > baseline + 400) {
        return Err(format!(
            "Unbounded status HUD node growth: {baseline:?} -> {nodes}"
        ));
    }
    if baseline.is_none() {
        world.resource_mut::<Inspection>().baseline_nodes = Some(nodes);
    }
    info!(
        stage = STAGES[stage],
        effects = actual.len(),
        cards = cards.len(),
        nodes,
        health = vitals.health,
        ?viewport,
        "Native F01 authoritative HUD check passed"
    );
    Ok(())
}

fn run(world: &mut World) {
    if world.resource::<Inspection>().start.elapsed() > Duration::from_secs(120) {
        error!("Status-effect fixture timed out");
        world.write_message(AppExit::error());
        return;
    }
    let stage = world.resource::<Inspection>().stage;
    if stage == STAGES.len() {
        let previews = world.resource::<terra_render::RenderBlockPreviews>();
        let portrait = world.resource::<terra_render::PlayerPreviewImage>();
        if previews.error.is_some() || portrait.error.is_some() {
            error!("Status-effect fixture encountered a preview readback error");
            world.write_message(AppExit::error());
        } else if previews.pending == 0
            && portrait.ready
            && world.resource::<Inspection>().since.elapsed() > Duration::from_secs(1)
        {
            info!(
                "Native F01 passed: real UDP effects, 13 cards/overflow, atlas icons, clear and three viewports"
            );
            world.write_message(AppExit::Success);
        }
        return;
    }
    let session = world.resource::<terra_network::client::ClientSession>();
    if !session.ready
        || session.teleporting
        || !world.resource::<terra_render::RenderStatus>().ready
    {
        return;
    }
    if !world.resource::<Inspection>().configured {
        configure(world, stage);
        return;
    }
    let command = {
        let mut state = world.resource_mut::<Inspection>();
        if Instant::now() >= state.next_command {
            let command = state.commands.pop_front();
            if command.is_some() {
                state.next_command = Instant::now() + Duration::from_millis(1100);
                state.since = Instant::now();
            }
            command
        } else {
            None
        }
    };
    if let Some(command) = command {
        world
            .resource_mut::<terra_network::client::ClientChatCommands>()
            .0
            .push_back(terra_network::client::ChatCommand::Send(command));
    }
    if !world.resource::<Inspection>().commands.is_empty() {
        return;
    }
    if world.resource::<Inspection>().pending
        || world.resource::<Inspection>().since.elapsed() < Duration::from_millis(1800)
    {
        return;
    }
    if let Err(error) = check(world, stage) {
        error!(%error, stage, "Native F01 check failed");
        world.write_message(AppExit::error());
        return;
    }
    let folder = PathBuf::from(
        std::env::var_os("TERRA_CAPTURE_DIR").unwrap_or_else(|| "/tmp/terra-status-effects".into()),
    );
    if let Err(error) = std::fs::create_dir_all(&folder) {
        error!(%error);
        world.write_message(AppExit::error());
        return;
    }
    world.resource_mut::<Inspection>().pending = true;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(
            folder.join(format!("status-{}.png", STAGES[stage])),
        ))
        .observe(
            |event: On<ScreenshotCaptured>,
             mut state: ResMut<Inspection>,
             mut exit: MessageWriter<AppExit>| {
                let lit = event.image.data.as_ref().is_some_and(|data| {
                    data.chunks_exact(4)
                        .step_by(16)
                        .filter(|pixel| pixel[..3].iter().any(|&value| value > 30))
                        .count()
                        > 1000
                });
                if !lit {
                    error!("Blank status-effect capture");
                    exit.write(AppExit::error());
                    return;
                }
                state.stage += 1;
                state.configured = false;
                state.pending = false;
                state.since = Instant::now();
            },
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_stages_include_each_effect_and_end_cleared() {
        assert_eq!(effects_for_stage(0), &[EffectKind::Poison]);
        assert_eq!(effects_for_stage(1), &[EffectKind::Freeze]);
        assert_eq!(effects_for_stage(2), &[EffectKind::Regeneration]);
        for stage in [3, 5, 6] {
            assert_eq!(effects_for_stage(stage), &EffectKind::ALL);
        }
        assert!(effects_for_stage(4).is_empty());
        assert!(effects_for_stage(STAGES.len() - 1).is_empty());
    }

    #[test]
    fn all_effect_commands_protect_the_player_before_applying_damage() {
        for stage in [3, 5] {
            let effects = requested_effects(stage);
            assert_eq!(effects.len(), EffectKind::ALL.len());
            assert_eq!(
                &effects[..4],
                &[
                    (EffectKind::Regeneration, 3),
                    (EffectKind::FireResistance, 1),
                    (EffectKind::FreezeResistance, 1),
                    (EffectKind::ShieldPower, 1),
                ]
            );
            for kind in EffectKind::ALL {
                assert_eq!(
                    effects.iter().filter(|(effect, _)| *effect == kind).count(),
                    1
                );
            }
        }
        assert_eq!(requested_effects(0), vec![(EffectKind::Poison, 1)]);
        assert_eq!(requested_effects(1), vec![(EffectKind::Freeze, 1)]);
        assert_eq!(requested_effects(2), vec![(EffectKind::Regeneration, 1)]);
    }
}
