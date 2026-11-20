//! Opt-in native acceptance path through the actual standalone TiltUI document.
use crate::{
    document::{Document, PLATFORM_Y},
    editor::{Editor, Panel},
};
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Resource)]
struct Smoke {
    stage: u8,
    since: Instant,
    started: Instant,
    output: PathBuf,
    pending: bool,
    captures: usize,
    release_mouse: Option<MouseButton>,
    initial_scene_since: Option<Instant>,
    saved_template: Option<terra_core::building_jobs::template::BuildingTemplate>,
}
fn emit(world: &mut World, id: &str, value: Option<&str>) -> Result<(), String> {
    let entity = world
        .query::<(Entity, &tilt_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .find_map(|(entity, name, node)| {
            (name.0 == id && node.size().min_element() > 0.0).then_some(entity)
        })
        .ok_or_else(|| format!("Real editor control missing: {id}"))?;
    world.write_message(tilt_ui::HtmlEvent {
        target: entity,
        kind: if value.is_some() { "change" } else { "click" },
        value: value.map(str::to_owned),
        submitter: None,
        data: Default::default(),
        form_data: None,
        handler: if value.is_some() {
            "editor_change"
        } else {
            "editor_command"
        }
        .into(),
    });
    Ok(())
}

fn platform_choice(world: &mut World) -> Result<tilt_ui::ChoiceBoxParts, String> {
    world
        .query::<(&tilt_ui::ElementId, &tilt_ui::ChoiceBoxParts)>()
        .iter(world)
        .find_map(|(id, parts)| (id.0 == "editor-platform-size").then_some(*parts))
        .ok_or("Missing real platform dropdown".into())
}

fn choose_platform(world: &mut World, value: &str) -> Result<bool, String> {
    let control = world
        .query::<(Entity, &tilt_ui::ElementId)>()
        .iter(world)
        .find_map(|(entity, id)| (id.0 == "editor-platform-size").then_some(entity))
        .ok_or("Missing platform control")?;
    let parts = platform_choice(world)?;
    if !parts.open {
        world.write_message(tilt_ui::ControlActivated { entity: control });
        return Ok(false);
    }
    let option = world
        .query::<(Entity, &tilt_ui::OptionData)>()
        .iter(world)
        .find_map(|(entity, data)| {
            let mut parent = entity;
            for _ in 0..32 {
                if parent == parts.popup {
                    return (data.value == value).then_some(entity);
                }
                parent = world.get::<ChildOf>(parent)?.parent();
            }
            None
        })
        .ok_or("Platform option is not inside the real dropdown popup")?;
    world.write_message(tilt_ui::ControlActivated { entity: option });
    Ok(true)
}
fn mouse(
    world: &mut World,
    button: MouseButton,
    state: bevy::input::ButtonState,
) -> Result<(), String> {
    let window = world
        .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("Missing actual editor window")?;
    world.write_message(bevy::input::mouse::MouseButtonInput {
        button,
        state,
        window,
    });
    Ok(())
}
fn aim(world: &mut World, target: Vec3) {
    let transform = Transform::from_xyz(0.5, 1.5, 4.5).looking_at(target, Vec3::Y);
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = transform;
    }
}
fn catalog_images_ready(world: &World) -> bool {
    let keys = world
        .resource::<crate::interface::EditorView>()
        .items
        .iter()
        .take(12)
        .map(|item| item.preview.as_str())
        .collect::<Vec<_>>();
    if keys.len() < 12 {
        return false;
    }
    let blocks = world.resource::<terra_render::RenderBlockPreviews>();
    let items = world.resource::<terra_render::RenderItemPreviews>();
    let props = world.resource::<terra_render::RenderPropPreviews>();
    let available = blocks
        .images
        .keys()
        .map(|&id| terra_ui::item_preview_key(terra_core::ItemKind::Block(id)))
        .chain(
            items
                .images
                .keys()
                .map(|&item| terra_ui::item_preview_key(item)),
        )
        .chain(
            props
                .images
                .keys()
                .map(|&kind| terra_ui::item_preview_key(terra_core::ItemKind::Prop(kind))),
        )
        .collect::<std::collections::HashSet<_>>();
    keys.into_iter().all(|key| available.contains(key))
}
fn shot(world: &mut World, smoke: &mut Smoke, name: &str) {
    smoke.pending = true;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(smoke.output.join(format!("{name}.png"))))
        .observe(|_: On<ScreenshotCaptured>, mut smoke: ResMut<Smoke>| {
            smoke.pending = false;
            smoke.captures += 1;
        });
}
pub fn run(world: &mut World) {
    if !std::env::args().any(|a| a == "--editor-acceptance") {
        return;
    }
    if !world.contains_resource::<Smoke>() {
        let output = std::env::var_os("TERRA_EDITOR_SMOKE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "terra-building-editor-smoke-{}",
                    std::process::id()
                ))
            });
        if let Err(error) = std::fs::create_dir_all(&output) {
            error!(%error);
            world.write_message(AppExit::error());
            return;
        }
        world.insert_resource(Smoke {
            stage: 0,
            since: Instant::now(),
            started: Instant::now(),
            output,
            pending: false,
            captures: 0,
            release_mouse: None,
            initial_scene_since: None,
            saved_template: None,
        });
    }
    let result = world.resource_scope(|world, mut smoke: Mut<Smoke>| -> Result<(), String> {
        if let Some(button) = smoke.release_mouse.take() {
            mouse(world, button, bevy::input::ButtonState::Released)?;
        }
        if smoke.started.elapsed() > Duration::from_secs(150) {
            return Err(format!("Editor acceptance timed out at {}", smoke.stage));
        }
        if smoke.pending || smoke.since.elapsed() < Duration::from_millis(900) {
            return Ok(());
        }
        match smoke.stage {
            0 => {
                let transitions=world.resource::<terra_render::ChunkAnimationStatus>();
                if world.resource::<terra_world::streaming::ChunkStore>().block_at([0,PLATFORM_Y,0]) != Some(43)
                    || !world.resource::<terra_render::RenderStatus>().ready
                    || !world.resource::<terra_render::RenderBlockPreviews>().images.contains_key(&43)
                    || transitions.active!=0 || transitions.queued_in_view!=0 || transitions.waiting_for_draw!=0 {
                    smoke.initial_scene_since=None;
                    return Ok(());
                }
                // The stone preview requires a completed real GPU readback;
                // settled chunk transitions prevent photographing a fade proxy.
                if smoke.initial_scene_since.get_or_insert_with(Instant::now).elapsed()<Duration::from_millis(500) { return Ok(()); }
                if !world.resource::<Document>().cells.is_empty() { return Err("Editor startup is not an empty authored document".into()); }
                shot(world,&mut smoke,"editor-empty-platform");
            }
            1 => {
                aim(world,Vec3::new(0.5,PLATFORM_Y as f32+0.99,0.5));
                world.write_message(terra_player::CaptureRequest(true));
            }
            2 => {
                if !world.resource::<terra_player::CameraInputState>().capture_ready() { return Ok(()); }
                if !world.resource::<terra_player::TargetBlock>().0.is_some_and(|hit|hit.position==[0,PLATFORM_Y,0]) {
                    return Err(format!("Normal platform ray missed: {:?}",world.resource::<terra_player::TargetBlock>().0));
                }
                mouse(world,MouseButton::Right,bevy::input::ButtonState::Pressed)?;
                smoke.release_mouse=Some(MouseButton::Right);
            }
            3 => {
                if world.resource::<terra_world::streaming::ChunkStore>().block_at([0,PLATFORM_Y+1,0])!=Some(16)
                    || world.resource::<terra_world::edits::EditJournal>().block([0,PLATFORM_Y+1,0])!=Some(16)
                    || world.resource::<Document>().cells.get(&[0,PLATFORM_Y+1,0]).is_none_or(|cell|cell.material!=16) {
                    return Err("Real editor right click did not place the selected voxel".into());
                }
                shot(world,&mut smoke,"editor-placed-block");
                aim(world,Vec3::new(0.5,PLATFORM_Y as f32+1.99,0.5));
            }
            4 => {
                if !world.resource::<terra_player::TargetBlock>().0.is_some_and(|hit|hit.position==[0,PLATFORM_Y+1,0]) {
                    return Err(format!("Normal authored-block ray missed: {:?}",world.resource::<terra_player::TargetBlock>().0));
                }
                mouse(world,MouseButton::Left,bevy::input::ButtonState::Pressed)?;
                smoke.release_mouse=Some(MouseButton::Left);
            }
            5 => {
                if !world.resource::<Document>().cells.is_empty()
                    || world.resource::<terra_world::streaming::ChunkStore>().block_at([0,PLATFORM_Y+1,0])!=Some(0)
                    || world.resource::<terra_world::streaming::ChunkStore>().block_at([0,PLATFORM_Y,0])!=Some(43) {
                    return Err("Real editor left click did not remove only the authored block".into());
                }
                shot(world,&mut smoke,"editor-removed-block");
            }
            6 => emit(world,"editor-items",None)?,
            7 => {
                if !catalog_images_ready(world) { return Ok(()); }
                shot(world,&mut smoke,"editor-real-inventory-catalog");
            }
            8 => emit(world,"editor-resume",None)?,
            9 => emit(world,"editor-menu",None)?,
            10 => emit(world,"editor-load",None)?,
            11 => {
                if world.resource::<Editor>().panel == Panel::Discard {
                    emit(world,"editor-discard",None)?;
                    smoke.since=Instant::now();
                    return Ok(());
                }
                if world.resource::<Editor>().panel != Panel::Load {
                    return Err("Real discard confirmation did not open the JSON browser".into());
                }
                emit(world,"editor-default",None)?;
            }
            12 => {
                if world.resource::<Document>().cells.len()<100 { return Err("Real default JSON house was not imported".into()); }
                world.resource::<Document>().template()?;
                shot(world,&mut smoke,"editor-loaded-house");
            }
            13 => emit(world,"editor-menu",None)?,
            14 => emit(world,"editor-save",None)?,
            15 => {
                let path=smoke.output.to_string_lossy().into_owned();
                emit(world,"editor-directory",Some(&path))?;
                emit(world,"editor-filename",Some("editor-acceptance"))?;
                emit(world,"editor-name",Some("Editor acceptance house"))?;
            }
            16 => {
                let editor = world.resource::<Editor>();
                let view = world.resource::<crate::interface::EditorView>();
                if editor.filename != "editor-acceptance" || editor.name != "Editor acceptance house"
                    || view.filename != editor.filename || view.name != editor.name
                    || view.directory != smoke.output.to_string_lossy() { return Ok(()); }
                shot(world,&mut smoke,"editor-save-browser");
            }
            17 => emit(world,"editor-write",None)?,
            18 => {
                let template=terra_core::building_jobs::template::BuildingTemplate::load(&smoke.output.join("editor-acceptance.json"))?;
                if template.display_name!="Editor acceptance house" || template.steps.is_empty() { return Err("Real JSON save is incomplete".into()); }
                smoke.saved_template=Some(template);
                emit(world,"editor-save",None)?;
            }
            19 => emit(world,"editor-write",None)?,
            20 => {
                if world.resource::<Editor>().panel!=Panel::Overwrite { return Err("Save did not request explicit overwrite confirmation".into()); }
                shot(world,&mut smoke,"editor-overwrite-confirmation");
            }
            21 => emit(world,"editor-overwrite",None)?,
            22 => emit(world,"editor-options",None)?,
            23 => emit(world,"editor-language",Some("en"))?,
            24 => {
                if !world.resource::<Editor>().english { return Err("Real editor language option did not apply".into()); }
                shot(world,&mut smoke,"editor-options-english");
            }
            25 => {
                if !choose_platform(world,"17")? { return Ok(()); }
            }
            26 => {
                if world.resource::<crate::workspace::WorkspacePlatform>().size()!=17
                    || world.resource::<terra_world::streaming::ChunkStore>().block_at([8,PLATFORM_Y,8])!=Some(crate::workspace::PLATFORM_MATERIAL)
                    || !world.resource::<terra_render::RenderStatus>().ready { return Ok(()); }
                let choice=platform_choice(world)?;
                if world.get::<Text>(choice.value).is_none_or(|text|text.0!="17 × 17") {
                    return Err("Platform dropdown does not display its selected size".into());
                }
                if world.resource::<Document>().dirty()
                    || Some(world.resource::<Document>().template()?)!=smoke.saved_template {
                    return Err("Changing the support size altered the authored model".into());
                }
                shot(world,&mut smoke,"editor-expanded-platform-options");
            }
            27 => emit(world,"editor-menu",None)?,
            28 => emit(world,"editor-resume",None)?,
            29 => {
                let transitions=world.resource::<terra_render::ChunkAnimationStatus>();
                if transitions.active!=0 || transitions.queued_in_view!=0 || transitions.waiting_for_draw!=0 { return Ok(()); }
                shot(world,&mut smoke,"editor-expanded-platform");
            }
            30 => emit(world,"editor-menu",None)?,
            31 => emit(world,"editor-save",None)?,
            32 => emit(world,"editor-write",None)?,
            33 => emit(world,"editor-overwrite",None)?,
            34 => {
                let template=terra_core::building_jobs::template::BuildingTemplate::load(&smoke.output.join("editor-acceptance.json"))?;
                if Some(template)!=smoke.saved_template {
                    return Err("Resized support leaked into the saved model or its material list".into());
                }
                if smoke.captures!=10 { return Err("Editor native captures incomplete".into()); }
                info!(size=17,"Standalone building editor protected platform resize passed: actual settings handler, real enlarged support, unchanged model and exact JSON save/reload without support cells");
                info!(captures=smoke.captures,"Standalone building editor acceptance passed: empty platform, actual right/left mouse voxel edits, real inventory previews, real JSON import, exact save/reload, explicit overwrite confirmation, live English options and protected support resizing");
                world.write_message(AppExit::Success);
            }
            _ => {}
        }
        smoke.stage+=1;
        smoke.since=Instant::now();
        Ok(())
    });
    if let Err(error) = result {
        error!(%error,"Standalone editor acceptance failed");
        world.write_message(AppExit::error());
    }
}
