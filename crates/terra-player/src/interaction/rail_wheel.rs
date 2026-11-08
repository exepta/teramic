//! Fresh target-based ownership of rail choice scrolling, before hotbar navigation.
use super::{ActiveItem, BlockPalette, PlacementPreview, TargetWorld, WorldTarget};
use bevy::{ecs::system::SystemParam, input::mouse::MouseWheel, prelude::*};
use terra_core::{
    GameAction, GameplayInput, ItemKind,
    rails::{BuildChoice, STRAIGHT},
};
use terra_voxel::blocks::BlockHit;
use terra_world::{edits::EditJournal, rails::ContextChoice, streaming::ChunkStore};

#[derive(Resource)]
pub struct RailBuildContext {
    pub choices: Vec<ContextChoice>,
    pub selected: BuildChoice,
    pub captures_wheel: bool,
    pub active: bool,
    pub hotbar_changed: bool,
    pub hit: Option<BlockHit>,
    pub item: Option<ItemKind>,
}
impl Default for RailBuildContext {
    fn default() -> Self {
        Self {
            choices: Vec::new(),
            selected: BuildChoice::Automatic,
            captures_wheel: false,
            active: false,
            hotbar_changed: false,
            hit: None,
            item: None,
        }
    }
}
impl RailBuildContext {
    /// Navigation invalidates the old item's proposal and requests same-frame targeting.
    pub fn mark_hotbar_changed(&mut self) {
        *self = Self {
            hotbar_changed: true,
            ..Default::default()
        };
    }

    fn set_choices(
        &mut self,
        hit: BlockHit,
        item: ItemKind,
        choices: Vec<ContextChoice>,
        movement: f32,
        aimed_at_rail: bool,
    ) {
        let same_dock = self.item == Some(item)
            && self.choices.first().and_then(|choice| choice.source)
                == choices.first().and_then(|choice| choice.source);
        let previous = self.selected;
        self.active = choices.iter().any(|choice| {
            choice
                .layout
                .as_ref()
                .is_some_and(|layout| !layout.edits.is_empty())
        });
        let meaningful = choices
            .iter()
            .filter_map(|choice| {
                choice
                    .layout
                    .as_ref()
                    .filter(|layout| !layout.edits.is_empty())
            })
            .collect::<Vec<_>>();
        self.captures_wheel = aimed_at_rail
            && choices.len() == 2
            && meaningful.len() == 2
            && !same_geometry(meaningful[0], meaningful[1]);
        self.selected = if aimed_at_rail
            && same_dock
            && choices.iter().any(|choice| {
                choice.choice == previous
                    && choice
                        .layout
                        .as_ref()
                        .is_some_and(|layout| !layout.edits.is_empty())
            }) {
            previous
        } else {
            BuildChoice::Automatic
        };
        if self.captures_wheel && movement.is_finite() && movement.abs() > f32::EPSILON {
            // World choices are preferred-first, matching Automatic. With exactly
            // two alternatives either wheel direction selects the other candidate.
            let current = choices
                .iter()
                .position(|choice| choice.choice == self.selected)
                .unwrap_or(0);
            self.selected = choices[(current + 1) % choices.len()].choice;
        }
        self.hit = self.active.then_some(hit);
        self.item = Some(item);
        self.choices = choices;
    }
}

fn same_geometry(
    first: &terra_world::placement::Layout,
    second: &terra_world::placement::Layout,
) -> bool {
    first.edits.len() == second.edits.len()
        && first
            .edits
            .iter()
            .zip(&second.edits)
            .all(|(first, second)| {
                first.position == second.position && first.material == second.material
            })
        && first.states == second.states
}

/// Resolve the current item's mode before context planning and wheel routing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_intent(
    input: GameplayInput,
    mut preview: ResMut<PlacementPreview>,
    active: Option<Res<ActiveItem>>,
    palette: Res<BlockPalette>,
    capture: Res<crate::CameraInputState>,
    blocked: Res<crate::PlayerInputBlock>,
    session: Option<Res<terra_network::client::ClientSession>>,
) {
    let item = active.as_ref().and_then(|active| active.0).or_else(|| {
        session
            .is_none()
            .then(|| palette.selected())
            .flatten()
            .map(|choice| ItemKind::Block(choice.id))
    });
    let blocked = !capture.capture_ready()
        || blocked.0
        || session.as_ref().is_some_and(|session| {
            !session.ready
                || !session.allow_build
                || session.error.is_some()
                || session.game_mode == terra_core::GameMode::Spectator
        });
    if blocked || preview.rail_item != item {
        preview.rail_start = None;
        if preview.rail_item != item {
            preview.rail_mode = 0;
        }
    }
    preview.rail_item = item.filter(|item| {
        item.block_material()
            .is_some_and(terra_core::rails::is_rail)
    });
    if !blocked && preview.rail_item.is_some() && input.just_pressed(GameAction::Use) {
        if item.and_then(ItemKind::block_material) == Some(STRAIGHT) {
            preview.rail_mode = (preview.rail_mode + 1) % 3;
        }
        preview.rail_start = None;
    }
}

#[derive(SystemParam)]
pub(crate) struct Scene<'w, 's> {
    target: Res<'w, TargetWorld>,
    capture: Res<'w, crate::CameraInputState>,
    blocked: Res<'w, crate::PlayerInputBlock>,
    preview: ResMut<'w, PlacementPreview>,
    active: Res<'w, ActiveItem>,
    palette: Res<'w, BlockPalette>,
    session: Option<Res<'w, terra_network::client::ClientSession>>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    perspective: Option<Res<'w, crate::CameraPerspective>>,
    cameras:
        Query<'w, 's, (&'static Transform, &'static crate::CameraLook), With<crate::TerraCamera>>,
    bodies:
        Query<'w, 's, &'static Transform, (With<crate::PlayerBody>, Without<crate::TerraCamera>)>,
}

pub(crate) fn update(
    mut context: ResMut<RailBuildContext>,
    mut wheel: MessageReader<MouseWheel>,
    input: GameplayInput,
    mut scene: Scene,
) {
    let movement: f32 = wheel.read().map(|event| event.y).sum();
    context.hotbar_changed = false;
    let item = scene.active.0.or_else(|| {
        scene
            .session
            .is_none()
            .then(|| scene.palette.selected())
            .flatten()
            .map(|choice| ItemKind::Block(choice.id))
    });
    // Interaction clears the previous item's intent later in this frame. A
    // numeric hotbar change must not let that stale intent suppress fresh aim.
    let same_item = scene.preview.rail_item == item;
    let mode = if same_item {
        scene.preview.rail_mode
    } else {
        0
    };
    let eligible = scene.capture.capture_ready()
        && !scene.blocked.0
        && !input.pressed(GameAction::Break)
        && (!same_item || (mode < 2 && scene.preview.rail_start.is_none()))
        && item.and_then(ItemKind::block_material) == Some(STRAIGHT)
        && scene.session.as_ref().is_none_or(|session| {
            session.ready
                && session.allow_build
                && session.error.is_none()
                && session.game_mode != terra_core::GameMode::Spectator
        });
    let Some(WorldTarget::Block(hit)) = scene.target.0.filter(|_| eligible) else {
        *context = Default::default();
        return;
    };
    if !hit.distance.is_finite() || !(0.0..=4.5).contains(&hit.distance) {
        *context = Default::default();
        return;
    }
    let Some((camera, look)) = scene.cameras.iter().next() else {
        *context = Default::default();
        return;
    };
    let origin = if scene
        .perspective
        .as_deref()
        .is_some_and(|mode| mode.is_third_person())
    {
        scene
            .bodies
            .iter()
            .next()
            .map_or(camera.translation, |body| {
                body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET
            })
    } else {
        camera.translation
    };
    let rotation = look.rotation();
    let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
    let item = item.expect("eligible rail item");
    let choices = terra_world::rails::context_choices(
        &scene.chunks,
        &scene.journal,
        hit,
        (
            origin.to_array().map(f64::from),
            (rotation * Vec3::NEG_Z).to_array().map(f64::from),
        ),
        [yaw, pitch],
        item.wood_species().unwrap_or_default(),
    );
    let aimed_at_rail = scene
        .chunks
        .block_at(hit.position)
        .is_some_and(terra_core::rails::is_rail);
    let use_pressed = input.just_pressed(GameAction::Use);
    if use_pressed {
        context.selected = BuildChoice::Automatic;
    }
    context.set_choices(
        hit,
        item,
        choices,
        if use_pressed { 0.0 } else { movement },
        aimed_at_rail,
    );
    if mode == 1 && (use_pressed || context.selected == BuildChoice::Automatic) {
        context.selected = BuildChoice::Curve;
    }
    if !use_pressed
        && context.captures_wheel
        && movement.is_finite()
        && movement.abs() > f32::EPSILON
    {
        scene.preview.rail_mode = u8::from(context.selected == BuildChoice::Curve);
    }
}

pub(crate) fn retarget_needed(context: Res<RailBuildContext>) -> bool {
    context.hotbar_changed
}

#[cfg(test)]
#[path = "rail_wheel_tests.rs"]
mod tests;
