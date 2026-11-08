//! Foley uses the material's recorded contacts; only confirmed edits produce impacts.
use super::{
    clips,
    scene::{AudioWorld, Soundscape},
    source::{Category, Cue},
};
use bevy::prelude::*;
use terra_network::{
    client::ConfirmedEdit,
    protocol::session::{EditAction, EditOutcome},
};

pub(super) fn update(
    world: AudioWorld,
    sounds: Res<Soundscape>,
    mut edits: MessageReader<ConfirmedEdit>,
    mut landings: MessageReader<crate::footsteps::Landing>,
    mut variation: Local<clips::ContactVariation>,
) {
    let Some(body) = world
        .bodies
        .iter()
        .next()
        .filter(|_| world.access.ready() && sounds.entity.is_some())
    else {
        edits.clear();
        landings.clear();
        return;
    };
    let listener = body.translation;
    let right = world.cameras.iter().next().map_or(Vec3::X, |c| *c.right());
    let mut emit = |p: Vec3, material, gain, pitch, category| {
        let distance = p.distance(listener);
        if distance > 24.0 {
            return;
        }
        let pan = (p - listener)
            .normalize_or_zero()
            .dot(right)
            .clamp(-1.0, 1.0);
        let take = variation.next(material, false);
        let level = gain * take.level * (1.0 - distance / 24.0).powi(2);
        sounds.controls.enqueue(Cue {
            category,
            sound: take.sound,
            gains: [level * (0.7 - pan * 0.3), level * (0.7 + pan * 0.3)],
            pitch,
            reverb: sounds.room.cave * 0.65,
        });
    };
    for landing in landings.read().take(8) {
        let heavy = landing.fall > 2.0;
        emit(
            landing.position,
            landing.material,
            if heavy { 0.65 } else { 0.30 },
            if heavy { 0.80 } else { 0.92 },
            Category::Footsteps,
        );
    }
    for edit in edits
        .read()
        .filter(|edit| edit.outcome == EditOutcome::Applied)
        .take(12)
    {
        if matches!(edit.action, EditAction::HarvestTree { .. }) {
            emit(listener, 7, 0.22, 1.12, Category::Environment);
        }
        if let EditAction::ToggleObject {
            position,
            expected: terra_core::rails::SWITCH_STAND,
            ..
        } = edit.action
        {
            emit(
                Vec3::from_array(position.map(|value| value as f32 + 0.5)),
                terra_core::rails::SWITCH_STAND,
                0.30,
                1.08,
                Category::Environment,
            );
        }
        if let EditAction::ConnectRail { position, .. }
        | EditAction::BuildRail { position, .. }
        | EditAction::SeedRail { position, .. }
        | EditAction::StretchRail { position, .. } = edit.action
        {
            emit(
                Vec3::from_array(position.map(|value| value as f32 + 0.5)),
                terra_core::rails::STRAIGHT,
                0.36,
                0.95,
                Category::Environment,
            );
        }
        if let EditAction::Block {
            position,
            material,
            expected,
        } = edit.action
        {
            let removed = material == 0;
            let material = if removed { expected } else { material };
            if matches!(material, 0 | 14 | 27) {
                continue;
            }
            emit(
                Vec3::from_array(position.map(|v| v as f32 + 0.5)),
                material,
                if removed { 0.48 } else { 0.36 },
                if removed { 0.84 } else { 0.95 },
                Category::Environment,
            );
        }
    }
    edits.clear();
    landings.clear();
}
