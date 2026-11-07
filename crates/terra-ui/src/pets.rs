//! Owner-only pet commands over the existing replicated animal population.
mod art;
mod layout;
#[cfg(test)]
mod tests;

use bevy::prelude::*;
use serde::Serialize;
use terra_core::{GameAction, GameMode, creatures::taming::CompanionMode};
use terra_network::client::{ClientSession, PetCommands, ReplicatedCreatures};
use tilt_ui::html_shared;

#[derive(Resource, Default)]
pub(crate) struct PetMenuState {
    id: Option<u64>,
    hovered: Option<CompanionMode>,
    pending: Option<(CompanionMode, f64)>,
    resume_pending: bool,
    pub consumed_escape: bool,
    error: String,
}
impl PetMenuState {
    pub(crate) fn blocks_input(&self) -> bool {
        self.id.is_some() || self.resume_pending
    }
}

#[derive(Message)]
pub enum PetMenuAction {
    Select(CompanionMode),
    Hover(CompanionMode),
    Close,
}

#[derive(Clone, Serialize, PartialEq)]
pub struct PetHeart {
    pub id: u8,
    pub style: String,
}

#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct PetMenuView {
    pub visible: bool,
    pub id: u64,
    pub title: String,
    pub health: String,
    pub hearts: Vec<PetHeart>,
    pub follow: bool,
    pub rest: bool,
    pub free: bool,
    pub pending: bool,
    pub error: String,
    pub style: String,
    pub compact: bool,
    pub tiny: bool,
    pub art_style: String,
    pub portrait_style: String,
    pub follow_style: String,
    pub rest_style: String,
    pub none_style: String,
    pub caption_style: String,
    pub follow_info_style: String,
    pub rest_info_style: String,
    pub none_info_style: String,
}

/// A single retained render target is filled by terra-render using its real mob meshes.
#[derive(Resource, Default)]
pub struct PetPortrait {
    pub image: Handle<Image>,
    pub id: Option<u64>,
    pub ready: bool,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<PetMenuState>()
        .init_resource::<PetMenuView>()
        .init_resource::<PetPortrait>()
        .add_message::<PetMenuAction>()
        .add_systems(PreUpdate, opening_frame.after(bevy::input::InputSystems))
        .add_systems(
            Update,
            update
                .before(crate::menu::MenuInputSet)
                .before(terra_player::PlayerSet::Movement),
        );
    art::install(app);
}

fn action(world: &World, action: GameAction) -> bool {
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(
            || action.default_binding(),
            |controls| controls.binding(action),
        );
    world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|keys| {
            binding.just_pressed(keys, world.get_resource::<ButtonInput<MouseButton>>())
        })
}

fn sneaking(world: &World) -> bool {
    let action = GameAction::FlyDown;
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(
            || action.default_binding(),
            |controls| controls.binding(action),
        );
    world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|keys| {
            binding.pressed(keys, world.get_resource::<ButtonInput<MouseButton>>())
                || binding == action.default_binding() && keys.pressed(KeyCode::ShiftRight)
        })
}

fn playing(world: &World) -> bool {
    world.get_resource::<ClientSession>().is_some_and(|s| {
        s.ready
            && s.error.is_none()
            && !s.dead()
            && !s.teleporting
            && s.game_mode != GameMode::Spectator
            && s.mounted.is_none()
    }) && world
        .get_resource::<crate::menu::MenuState>()
        .is_none_or(|m| m.page == crate::menu::MenuPage::Playing)
}

fn unoccupied(world: &World) -> bool {
    !world
        .get_resource::<crate::kingdom::KingdomMenuState>()
        .is_some_and(|k| k.blocks_input())
        && !world
            .get_resource::<crate::inventory::InventoryState>()
            .is_some_and(|s| s.blocks_input())
        && !world
            .get_resource::<crate::chat::ChatState>()
            .is_some_and(|s| s.blocks_input())
        && !world
            .get_resource::<crate::minimap::fullmap::FullMapState>()
            .is_some_and(|s| s.blocks_input())
        && !world
            .get_resource::<crate::SettingsView>()
            .is_some_and(|s| s.open)
}

fn opening_frame(world: &mut World) {
    world.resource_mut::<PetMenuState>().consumed_escape = false;
    if world.resource::<PetMenuState>().resume_pending
        && world
            .get_resource::<ButtonInput<KeyCode>>()
            .is_none_or(|keys| keys.get_pressed().next().is_none())
        && world
            .get_resource::<ButtonInput<MouseButton>>()
            .is_none_or(|buttons| buttons.get_pressed().next().is_none())
    {
        world.resource_mut::<PetMenuState>().resume_pending = false;
        if playing(world) && unoccupied(world) {
            world.resource_mut::<terra_player::PlayerInputBlock>().0 = false;
            world.write_message(terra_player::CaptureRequest(true));
        }
    }
    if !playing(world)
        || !unoccupied(world)
        || !sneaking(world)
        || !action(world, GameAction::Place)
        || world.resource::<PetMenuState>().blocks_input()
        || !world
            .get_resource::<terra_player::CameraInputState>()
            .is_some_and(|c| c.captured)
    {
        return;
    }
    let Some(id) = world
        .get_resource::<terra_player::TargetCreature>()
        .and_then(|t| t.0)
    else {
        return;
    };
    let Some(session) = world.get_resource::<ClientSession>() else {
        return;
    };
    let Some(owner) = session.uuid else { return };
    let Some(pets) = world.get_resource::<ReplicatedCreatures>() else {
        return;
    };
    if pets.bonds.get(&id).is_none_or(|b| b.owner != Some(owner))
        || pets.creatures.get(&id).is_none_or(|a| {
            !a.current.health.is_finite()
                || a.current.health <= 0.0
                || !terra_core::creatures::taming::tameable(a.current.kind)
        })
    {
        return;
    }
    world.resource_mut::<PetMenuState>().id = Some(id);
    world.resource_mut::<PetMenuState>().hovered = None;
    world.resource_mut::<PetMenuState>().error.clear();
    world.resource_mut::<terra_player::PlayerInputBlock>().0 = true;
    world.write_message(terra_player::CaptureRequest(false));
}

fn close(world: &mut World, resume: bool) {
    let mut state = world.resource_mut::<PetMenuState>();
    state.id = None;
    state.pending = None;
    state.hovered = None;
    state.error.clear();
    state.resume_pending = resume;
}

fn update(world: &mut World) {
    let actions: Vec<_> = world
        .resource_mut::<Messages<PetMenuAction>>()
        .drain()
        .collect();
    let Some(id) = world.resource::<PetMenuState>().id else {
        if world.resource::<PetMenuView>().visible {
            *world.resource_mut::<PetMenuView>() = default();
        }
        return;
    };
    let valid = playing(world) && unoccupied(world);
    let snapshot = world
        .get_resource::<ReplicatedCreatures>()
        .and_then(|pets| {
            let session = world.get_resource::<ClientSession>()?;
            let animal = pets.creatures.get(&id)?.current.clone();
            let bond = pets.bonds.get(&id)?;
            let player = session.confirmed_state().map_or_else(
                || session.spawn.to_array().map(f64::from),
                |state| state.position.map(f64::from),
            );
            (session.uuid.is_some()
                && bond.owner == session.uuid
                && animal.health > 0.0
                && terra_world::creatures::distance(player, animal.position) <= 36.0)
                .then_some((animal, bond.mode))
        });
    if !valid || snapshot.is_none() {
        close(world, valid);
        *world.resource_mut::<PetMenuView>() = default();
        return;
    }
    let (animal, mode) = snapshot.unwrap();
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |t| t.elapsed_secs_f64());
    if action(world, GameAction::Pause) {
        close(world, true);
        world.resource_mut::<PetMenuState>().consumed_escape = true;
    } else {
        for action in actions {
            match action {
                PetMenuAction::Close => {
                    close(world, true);
                    break;
                }
                PetMenuAction::Hover(mode) => {
                    world.resource_mut::<PetMenuState>().hovered = Some(mode)
                }
                PetMenuAction::Select(request)
                    if world.resource::<PetMenuState>().pending.is_none() =>
                {
                    if request == mode {
                        close(world, true);
                        break;
                    }
                    if let Some(mut queue) = world.get_resource_mut::<PetCommands>()
                        && queue.0.len() < 2
                    {
                        queue.0.push_back((id, request));
                        world.resource_mut::<PetMenuState>().pending = Some((request, now));
                        world.resource_mut::<PetMenuState>().error.clear();
                    }
                }
                _ => {}
            }
        }
    }
    if let Some((wanted, since)) = world.resource::<PetMenuState>().pending {
        if wanted == mode {
            close(world, true);
        } else if now - since >= 3.0 {
            let mut state = world.resource_mut::<PetMenuState>();
            state.pending = None;
            state.error = "Command could not be confirmed".into();
        }
    }
    if world.resource::<PetMenuState>().id.is_none() {
        *world.resource_mut::<PetMenuView>() = default();
        return;
    }
    let highlight = world.resource::<PetMenuState>().hovered.unwrap_or(mode);
    let pending = world.resource::<PetMenuState>().pending.is_some();
    let error = world.resource::<PetMenuState>().error.clone();
    let (canvas, viewport) = world
        .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .map_or(
            (Vec2::new(1280.0, 720.0), Vec2::new(1280.0, 720.0)),
            |window| {
                (
                    window.size(),
                    Vec2::new(
                        window.physical_width() as f32,
                        window.physical_height() as f32,
                    ),
                )
            },
        );
    let ui_scale = world
        .get_resource::<UiScale>()
        .map_or(1.0, |s| s.0)
        .max(0.1);
    let layout = layout::Layout::new(canvas / ui_scale, viewport);
    let definition = terra_core::creatures::definitions::get(animal.kind);
    let full_title = definition.variant_title(animal.variant);
    let view = PetMenuView {
        visible: true,
        id,
        title: if layout.compact {
            &definition.title
        } else {
            full_title
        }
        .into(),
        health: format!(
            "{full_title}: {} / {}",
            animal.health.ceil() as u16,
            animal.kind.health() as u16
        ),
        hearts: (0..10)
            .map(|i| PetHeart {
                id: i,
                style: format!(
                    "opacity: {};",
                    if animal.health / animal.kind.health() * 10.0 > f32::from(i) {
                        1.0
                    } else {
                        0.18
                    }
                ),
            })
            .collect(),
        follow: highlight == CompanionMode::Follow,
        rest: highlight == CompanionMode::Wait,
        free: highlight == CompanionMode::None,
        pending,
        error,
        style: layout.style(),
        compact: layout.compact,
        tiny: layout.tiny,
        art_style: layout.art.style(),
        portrait_style: layout.portrait.style(),
        follow_style: layout.follow.style(),
        rest_style: layout.rest.style(),
        none_style: layout.free.style(),
        caption_style: layout.caption.style(),
        follow_info_style: layout.follow_info.style(),
        rest_info_style: layout.rest_info.style(),
        none_info_style: layout.none_info.style(),
    };
    if *world.resource::<PetMenuView>() != view {
        *world.resource_mut::<PetMenuView>() = view;
    }
    world.resource_mut::<terra_player::PlayerInputBlock>().0 = true;
}
