//! Opt-in native work-panel test after a real founding, spawn and chest-placement ack.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_network::client::{ChatCommand, ClientChatCommands, ClientSession};
use terra_ui::{KingdomMenuAction, KingdomView, NpcWorkUiAction, NpcWorkView};

pub(crate) fn requested() -> bool {
    std::env::var("TERRA_NPC_WORK_UI_GALLERY").as_deref() == Ok("1")
}

#[derive(Resource)]
struct Gallery {
    step: u8,
    since: Instant,
    chest: crate::inventory_benchmark::chest::ChestFixture,
    npc: Option<u64>,
    keep: Option<[i64; 3]>,
    aligned: Option<u64>,
    captures: u8,
    pending: bool,
    failed: bool,
}

fn element(world: &mut World, id: &str) -> Option<Entity> {
    world
        .query::<(Entity, &tilt_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .find_map(|(entity, name, node)| {
            (name.0 == id && node.size().min_element() > 0.0).then_some(entity)
        })
}

fn bounds(world: &mut World) -> Result<(), String> {
    let dialog = element(world, "kingdom-dialog").ok_or("Missing kingdom dialog")?;
    for id in [
        "npc-work-panel",
        "npc-work-workers",
        "npc-work-editor",
        "npc-work-field-kind",
        "npc-work-field-priority",
        "npc-work-field-min-x",
        "npc-work-field-min-y",
        "npc-work-field-min-z",
        "npc-work-field-max-x",
        "npc-work-field-max-y",
        "npc-work-field-max-z",
        "npc-work-field-storage-x",
        "npc-work-field-storage-y",
        "npc-work-field-storage-z",
        "npc-work-assign",
        "npc-work-enabled",
    ] {
        let entity =
            element(world, id).ok_or_else(|| format!("Missing native work control {id}"))?;
        let bounds = |entity| -> Result<Rect, String> {
            let size = world
                .get::<ComputedNode>(entity)
                .ok_or("Missing node")?
                .size();
            let pose = world
                .get::<bevy::ui::UiGlobalTransform>(entity)
                .ok_or("Missing UI transform")?;
            Ok(Rect::from_center_size(
                pose.translation,
                size * pose.matrix2.x_axis.length(),
            ))
        };
        let outer = bounds(dialog)?;
        let inner = bounds(entity)?;
        if inner.min.cmplt(outer.min - Vec2::splat(2.0)).any()
            || inner.max.cmpgt(outer.max + Vec2::splat(2.0)).any()
        {
            return Err(format!(
                "Work control {id} escapes dialog: {inner:?}/{outer:?}"
            ));
        }
    }
    Ok(())
}

fn shot(world: &mut World, state: &mut Gallery, name: &str) -> Result<(), String> {
    bounds(world)?;
    let directory =
        PathBuf::from(std::env::var_os("TERRA_CAPTURE_DIR").ok_or("Set TERRA_CAPTURE_DIR")?);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Gallery>| {
                let colors: std::collections::HashSet<_> = event
                    .image
                    .data
                    .as_deref()
                    .unwrap_or_default()
                    .chunks_exact(4)
                    .step_by(113)
                    .map(|pixel| [pixel[0], pixel[1], pixel[2]])
                    .collect();
                state.failed |= colors.len() < 16;
                state.captures += 1;
                state.pending = false;
            },
        );
    state.pending = true;
    Ok(())
}

pub(crate) fn run(world: &mut World) -> Result<bool, String> {
    if !world.contains_resource::<Gallery>() {
        world.insert_resource(Gallery {
            step: 0,
            since: Instant::now(),
            chest: Default::default(),
            npc: None,
            keep: None,
            aligned: None,
            captures: 0,
            pending: false,
            failed: false,
        });
    }
    world.resource_scope(|world, mut state: Mut<Gallery>| {
        if let Some(keep) = state.keep { crate::kingdom_smoke::aim(world, keep); }
        if state.failed { return Err("NPC work UI screenshot is blank".into()); }
        if state.pending || state.since.elapsed() < Duration::from_millis(700) { return Ok(false); }
        match state.step {
            0 => {
                world.write_message(KingdomMenuAction::Close);
                world.resource_mut::<ClientChatCommands>().0.push_back(ChatCommand::Send("/spawn teramic::villager".into()));
            }
            1 => {
                if !state.chest.prepare(world)? { return Ok(false); }
                let session = world.resource::<ClientSession>();
                let keep = session.uuid.and_then(|uuid| session.kingdoms.active_for_member(uuid)).ok_or("Missing founded realm")?.location;
                state.keep = Some(keep);
                crate::kingdom_smoke::aim(world, keep);
                let session = world.resource::<ClientSession>();
                let Some(pose) = session.confirmed_state() else { return Ok(false) };
                let eye = Vec3::from_array(pose.position) + Vec3::Y * terra_player::PLAYER_EYE_OFFSET;
                let expected = (Vec3::from_array(keep.map(|v| v as f32 + 0.5)) - eye).normalize_or_zero();
                let actual = Quat::from_euler(EulerRot::YXZ, session.look[0], session.look[1], 0.0) * Vec3::NEG_Z;
                if actual.dot(expected) < 0.9999 { state.aligned = None; return Ok(false); }
                let initial = *state.aligned.get_or_insert(pose.ack);
                if pose.ack < initial.saturating_add(2) { return Ok(false); }
                world.write_message(KingdomMenuAction::Open(keep));
            }
            2 => {
                if !world.resource::<KingdomView>().visible { return Ok(false); }
                world.write_message(KingdomMenuAction::Tab("work".into()));
            }
            3 => {
                let Some(npc) = world.resource::<ClientSession>().work.workers.first().map(|worker| worker.npc) else { return Ok(false) };
                state.npc = Some(npc);
                world.write_message(NpcWorkUiAction::Select(npc));
            }
            4 => {
                if !world.resource::<NpcWorkView>().selected { return Ok(false); }
                world.write_message(NpcWorkUiAction::Field { field: "kind".into(), value: "collector".into() });
                for (axis, value) in ["x", "y", "z"].into_iter().zip(state.chest.position()) {
                    world.write_message(NpcWorkUiAction::Field { field: format!("storage-{axis}"), value: value.to_string() });
                }
            }
            5 => {
                if !world.resource::<NpcWorkView>().can_assign { return Err(format!("Native work draft invalid: {:?}", world.resource::<NpcWorkView>())); }
                world.write_message(NpcWorkUiAction::Assign);
            }
            6 => {
                if world.resource::<NpcWorkView>().pending { return Ok(false); }
                let view = world.resource::<NpcWorkView>();
                if !view.error.is_empty() { return Err(view.error.clone()); }
                if !view.has_order { return Ok(false); }
                for mut window in world.query::<&mut Window>().iter_mut(world) { window.resolution.set_physical_resolution(1680, 936); }
            }
            7 => {
                if world.resource::<ClientSession>().work.workers.iter().any(|worker|
                    Some(worker.npc) == state.npc && worker.status == terra_core::npc_jobs::WorkStatus::Waiting(terra_core::npc_jobs::WaitReason::MissingMaterial)
                ) { return Err("Collector rejected the real, freshly placed empty chest".into()); }
                shot(world, &mut state, "npc-work-wide")?;
            }
            8 => { world.write_message(NpcWorkUiAction::Enabled(false)); }
            9 => {
                let view = world.resource::<NpcWorkView>();
                if view.pending || view.enabled { return Ok(false); }
                if !view.error.is_empty() { return Err(view.error.clone()); }
                for mut window in world.query::<&mut Window>().iter_mut(world) { window.resolution.set_physical_resolution(1024, 768); }
            }
            10 => { shot(world, &mut state, "npc-work-compact-paused")?; }
            11 => { world.write_message(NpcWorkUiAction::Enabled(true)); }
            12 => {
                if world.resource::<NpcWorkView>().pending || !world.resource::<NpcWorkView>().enabled { return Ok(false); }
                for mut window in world.query::<&mut Window>().iter_mut(world) { window.resolution.set_physical_resolution(3840, 2160); }
            }
            13 => { shot(world, &mut state, "npc-work-4k")?; }
            _ => {
                if state.captures < 3 { return Ok(false); }
                info!(npc=?state.npc, "Native NPC work UI passed: real spawn, chest, order/pause/resume acknowledgements and three viewport captures");
                return Ok(true);
            }
        }
        state.step += 1;
        state.since = Instant::now();
        Ok(false)
    })
}
