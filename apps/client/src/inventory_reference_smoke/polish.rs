//! Focused native map-frame and hotkey alignment check, in the temporary world.
use super::*;

fn map_bounds(world: &mut World) -> Result<(), String> {
    let rect = |world: &World, entity| -> Result<Rect, String> {
        Ok(Rect::from_center_size(
            world
                .get::<bevy::ui::UiGlobalTransform>(entity)
                .ok_or("No map transform")?
                .translation,
            world
                .get::<ComputedNode>(entity)
                .ok_or("No map geometry")?
                .size(),
        ))
    };
    let root = element(world, "full-map-panel").ok_or("Map did not open")?;
    let frame = rect(world, root)?;
    let inset = 12.0
        / world
            .get::<ComputedNode>(root)
            .unwrap()
            .inverse_scale_factor();
    for id in [
        "fullmap-close",
        "fullmap-add",
        "full-map-viewport",
        "fullmap-in",
        "fullmap-out",
    ] {
        let child = element(world, id).ok_or_else(|| format!("Missing map control {id}"))?;
        let bounds = rect(world, child)?;
        if bounds.min.cmplt(frame.min + Vec2::splat(inset)).any()
            || bounds.max.cmpgt(frame.max - Vec2::splat(inset)).any()
        {
            return Err(format!(
                "Map control touches copper rail: {id}, {bounds:?}, {frame:?}"
            ));
        }
    }
    Ok(())
}
fn hotkeys(world: &mut World) -> Result<(), String> {
    let mut found = 0;
    for (classes, node) in world
        .query::<(&tilt_ui::ElementClasses, &Node)>()
        .iter(world)
    {
        if classes.classes.iter().any(|class| class == "hud-slot-key") {
            if node.justify_content != JustifyContent::Center
                || node.align_items != AlignItems::Center
            {
                return Err("Hotbar glyphs are not centered inside their labels".into());
            }
            found += 1;
        }
    }
    if found != 9 {
        return Err(format!("Expected nine hotkey labels, got {found}"));
    }
    Ok(())
}
pub(super) fn run(world: &mut World) {
    world.resource_scope(|world,mut state:Mut<Inspection>|{
        if state.stage==u8::MAX{return;}
        let result=(||->Result<bool,String>{
            if state.start.elapsed()>Duration::from_secs(150){return Err(format!("Polish timeout {}",state.stage));}
            if state.failed{return Err("Invalid screenshot".into());}
            if let Some(code)=state.key_release {
                if state.since.elapsed()>Duration::from_millis(180){
                    os_pointer(world,&[tilt_ui::serde_json::json!({"key":[code,0]})])?;
                    state.key_release=None;state.since=Instant::now();
                }
                return Ok(false);
            }
            if state.stage!=0 && state.since.elapsed()<Duration::from_millis(1200){return Ok(false);}
            match state.stage {
                0=>{
                    if !world.resource::<ClientSession>().ready || !world.resource::<terra_render::RenderStatus>().ready{return Ok(false);}
                    if world.resource::<MenuState>().page!=MenuPage::Playing {
                        world.resource_mut::<MenuState>().session_active=true;
                        world.write_message(MenuAction::Command("resume".into()));return Ok(false);
                    }
                    resize_reference(world,(1680,936),true);advance(&mut state,1);
                }
                1=>{
                    if !size_ready(world,(1680,936)){return Ok(false);}
                    let map=element(world,"mini-map").ok_or("Missing minimap")?;
                    pointer(world,map,None)?;hotkeys(world)?;shot(world,"hotbar-wide")?;advance(&mut state,2);
                }
                2=>{native_key(world,&mut state,50)?;advance(&mut state,3);}
                3=>{map_bounds(world)?;shot(world,"map-wide")?;advance(&mut state,8);}
                4=>{resize_reference(world,(1024,640),false);advance(&mut state,5);}
                5=>{if !size_ready(world,(1024,640)){return Ok(false);}map_bounds(world)?;shot(world,"map-compact")?;advance(&mut state,6);}
                6=>{native_key(world,&mut state,50)?;advance(&mut state,7);}
                7=>{if element(world,"full-map-panel").is_some(){return Err("Native M did not close map".into());}hotkeys(world)?;shot(world,"hotbar-compact")?;advance(&mut state,11);}
                8=>{world.write_message(terra_ui::minimap::fullmap::FullMapAction::Command{control:"fullmap-add".into(),id:None});advance(&mut state,9);}
                9=>{
                    let view=world.resource::<terra_ui::minimap::fullmap::FullMapView>();
                    if !view.editing || [&view.x,&view.y,&view.z].iter().any(|value|value.split_once(',').is_none_or(|(_,fraction)|fraction.len()!=2)) {return Err("Waypoint fields are not comma-separated two-decimal coordinates".into());}
                    shot(world,"waypoint-wide")?;advance(&mut state,10);
                }
                10=>{world.write_message(terra_ui::minimap::fullmap::FullMapAction::Command{control:"fullmap-cancel".into(),id:None});advance(&mut state,4);}
                _=>{if state.captured<5{return Ok(false);}info!("Native polish187 passed: map frame insets, nine centered hotkeys, two-decimal waypoint fields, native M, wide and compact");return Ok(true);}
            }
            Ok(false)
        })();
        match result {
            Ok(true)=>{world.write_message(AppExit::Success);state.stage=u8::MAX;}
            Ok(false)=>{}
            Err(error)=>{error!(%error,"Native polish failed");world.write_message(AppExit::error());state.stage=u8::MAX;}
        }
    });
}
