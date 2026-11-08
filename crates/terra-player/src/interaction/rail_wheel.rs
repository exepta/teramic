//! Fresh target-based ownership of rail choice scrolling, before hotbar navigation.
use super::{ActiveItem, BlockPalette, PlacementPreview};
use bevy::{input::mouse::MouseWheel, prelude::*};
use terra_core::{
    GameAction, GameplayInput, ItemKind,
    rails::{BuildChoice, STRAIGHT},
};
use terra_voxel::blocks::BlockHit;
use terra_world::rails::ContextChoice;

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
        preview.cancel_rail_chain();
        if preview.rail_item != item {
            preview.rail_mode = 0;
        }
    }
    preview.rail_item = item.filter(|item| {
        item.block_material()
            .is_some_and(terra_core::rails::is_rail)
    });
    if !blocked && preview.rail_item.is_some() && input.just_pressed(GameAction::Use) {
        preview.cancel_rail_pending();
        if item.and_then(ItemKind::block_material) == Some(STRAIGHT) {
            preview.rail_mode = (preview.rail_mode + 1) % 3;
        }
        if preview.rail_mode == 2 {
            preview.cancel_rail_chain();
        }
    }
}

/// Track stretching has no preset alternatives. Let the ordinary hotbar own
/// the wheel, and never retain a former automatic four-block proposal.
pub(crate) fn update(mut context: ResMut<RailBuildContext>, mut wheel: MessageReader<MouseWheel>) {
    for _ in wheel.read() {}
    *context = Default::default();
}

pub(crate) fn retarget_needed(context: Res<RailBuildContext>) -> bool {
    context.hotbar_changed
}

#[cfg(test)]
#[path = "rail_wheel_tests.rs"]
mod tests;
