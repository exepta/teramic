use crate::{
    CameraPerspective, TargetBlock, TargetWorld,
    camera::{
        CaptureRequest, ControllerMode, PlayerControllerStatus, PlayerInputBlock, WalkIntent,
        capture_on_start, follow_player_camera, move_camera, read_walk_input, sync_controller,
        update_capture, walk_step,
    },
    interaction::{BlockPalette, edit_target_block, select_block, target_block},
};
use bevy::prelude::*;
use terra_world::streaming::StreamingSet;
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum PlayerSet {
    Movement,
    Targeting,
    Interaction,
}
pub struct TerraPlayerPlugin;
impl Plugin for TerraPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraPerspective>()
            .init_resource::<ControllerMode>()
            .init_resource::<PlayerInputBlock>()
            .init_resource::<PlayerControllerStatus>()
            .init_resource::<WalkIntent>()
            .add_message::<CaptureRequest>()
            .init_resource::<crate::CameraInputState>()
            .init_resource::<TargetBlock>()
            .init_resource::<TargetWorld>()
            .init_resource::<crate::interaction::TargetCreature>()
            .init_resource::<BlockPalette>()
            .init_resource::<crate::interaction::ActiveItem>()
            .init_resource::<crate::interaction::MiningVisual>()
            .init_resource::<crate::interaction::PlacementPreview>()
            .init_resource::<crate::interaction::InteractionMotion>()
            .add_message::<terra_network::client::MiningRequest>()
            .add_message::<terra_network::client::PropPlaceRequest>()
            .add_message::<terra_network::client::ConfirmedEdit>()
            .configure_sets(
                Update,
                (
                    PlayerSet::Movement,
                    PlayerSet::Targeting,
                    PlayerSet::Interaction,
                )
                    .chain()
                    .before(StreamingSet::Interest),
            )
            .add_systems(
                Update,
                (
                    sync_controller,
                    crate::camera::perspective::toggle,
                    follow_player_camera,
                    crate::camera::perspective::boom,
                )
                    .chain()
                    .in_set(PlayerSet::Movement)
                    .in_set(StreamingSet::Focus),
            )
            .add_systems(Startup, capture_on_start)
            .add_systems(
                RunFixedMainLoop,
                (update_capture, move_camera, read_walk_input)
                    .chain()
                    .in_set(bevy::app::RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            .add_systems(
                FixedUpdate,
                walk_step.after(avian3d::prelude::PhysicsSystems::Writeback),
            )
            .add_systems(
                Update,
                (target_block, crate::interaction::target_creature)
                    .chain()
                    .in_set(PlayerSet::Targeting),
            )
            .add_systems(
                Update,
                (select_block, edit_target_block)
                    .chain()
                    .in_set(PlayerSet::Interaction),
            );
        if std::env::args().any(|argument| {
            argument.starts_with("--smoke-test")
                && argument != "--smoke-test-physics"
                && !argument.starts_with("--smoke-test-network")
                && !argument.starts_with("--smoke-test-menu")
        }) {
            app.insert_resource(ControllerMode::Fly);
        }
    }
}
