//! Native UI projection gallery; authority is covered by the real UDP tests.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::time::Instant;
use terra_core::kingdom::*;
use terra_network::client::ClientSession;
use terra_ui::{
    menu::{MenuAction, MenuPage, MenuState},
    minimap::fullmap::{FullMapAction, FullMapView},
};

#[derive(Resource)]
struct Gallery {
    start: Instant,
    step: u8,
    since: Instant,
    captures: usize,
    id: Option<KingdomId>,
    ally: Option<KingdomId>,
    selected_chunks: usize,
}
pub(crate) fn install(app: &mut App) {
    app.insert_resource(Gallery {
        start: Instant::now(),
        since: Instant::now(),
        step: 0,
        captures: 0,
        id: None,
        ally: None,
        selected_chunks: 0,
    })
    .add_systems(PreUpdate, run.after(bevy::input::InputSystems));
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut g: Mut<Gallery>| {
        let result = (|| -> Result<bool, String> {
            if g.start.elapsed().as_secs() > 150 { return Err("Claims gallery timed out".into()); }
            if g.since.elapsed().as_secs_f32() < 1.5 { return Ok(false); }
            match g.step {
                0 => {
                    let session = world.resource::<ClientSession>();
                    if !session.ready || !world.resource::<terra_render::RenderStatus>().ready { return Ok(false); }
                    if world.resource::<MenuState>().page != MenuPage::Playing {
                        world.resource_mut::<MenuState>().session_active = true;
                        world.write_message(MenuAction::Command("resume".into())); return Ok(false);
                    }
                    let Some(pose) = session.confirmed_state() else { return Ok(false); };
                    let uuid = session.uuid.ok_or("Missing player identity")?;
                    let p = pose.position.map(|v| v.floor() as i64);
                    let chunk = claim_chunk(p);
                    let mut registry = KingdomRegistry::default();
                    let id = registry.found_settlement(uuid,"Player",p,"Northwatch",SettlementKind::Kingdom,KingdomHeraldry { color:[30,110,200],icon:KingdomIcon::Castle }).map_err(|e|e.to_string())?;
                    for (offset, actor) in [(1,ClaimActor::Player(uuid)),(2,ClaimActor::Bot { id:77,authorized_by:uuid })] {
                        registry.claim(id,[chunk[0]+offset,chunk[1]],ClaimKind::RealmTerritory,actor).map_err(|e|e.to_string())?;
                    }
                    let ally_owner=terra_core::PlayerUuid::from_u128(987654);
                    let ally=registry.found_settlement(ally_owner,"Visitor",[p[0]+64,p[1],p[2]],"Eastbank",SettlementKind::City,KingdomHeraldry { color:[190,70,40],icon:KingdomIcon::Anchor }).map_err(|e|e.to_string())?;
                    let enemy=registry.found_settlement(terra_core::PlayerUuid::from_u128(987655),"Guard",[p[0]+96,p[1],p[2]],"Ironhold",SettlementKind::Kingdom,KingdomHeraldry {color:[170,70,120],icon:KingdomIcon::Star}).map_err(|e|e.to_string())?;
                    registry.found_settlement(terra_core::PlayerUuid::from_u128(987656),"Traveler",[p[0]+128,p[1],p[2]],"Farplain",SettlementKind::Village,KingdomHeraldry {color:[170,155,90],icon:KingdomIcon::Wheat}).map_err(|e|e.to_string())?;
                    registry.set_relation(uuid,id,DiplomaticTarget::Player(ally),DiplomaticRelation::Allied).map_err(|e|e.to_string())?;
                    registry.set_relation(ally_owner,ally,DiplomaticTarget::Player(id),DiplomaticRelation::Allied).map_err(|e|e.to_string())?;
                    registry.set_relation(uuid,id,DiplomaticTarget::Player(enemy),DiplomaticRelation::Hostile).map_err(|e|e.to_string())?;
                    registry.set_relation(uuid,id,DiplomaticTarget::Bot(BotKingdom::Helteria),DiplomaticRelation::Allied).map_err(|e|e.to_string())?;
                    world.resource_mut::<ClientSession>().kingdoms = registry;
                    g.id=Some(id);
                    g.ally=Some(ally);
                    world.resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyM);
                }
                1 => {
                    world.resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::KeyM);
                    if !world.resource::<FullMapView>().visible { return Ok(false); }
                    world.write_message(FullMapAction::Command { control:"fullmap-diplomacy".into(),id:None });
                    let pose=world.resource::<ClientSession>().confirmed_state().ok_or("Missing player")?;
                    let p=pose.position.map(|v|v.floor() as i64);
                    world.resource_mut::<terra_ui::settlement_map::SettlementMap>().remember(SettlementDomain {id:u64::MAX-99,center:[p[0]-48,p[1],p[2]],bounds:[[p[0]-72,p[1]-5,p[2]-24],[p[0]-24,p[1]+30,p[2]+24]],kind:DomainKind::Village,native:Some(BotKingdom::Petora)});

                }
                2 | 4 | 6 => {
                    let view=world.resource::<FullMapView>();
                    if !view.visible || !view.diplomacy || view.diplomacy_legends.is_empty() { return Ok(false); }
                    if g.step==2 {
                        let relations=view.diplomacy_legends.iter().map(|realm|realm.relation.as_str()).collect::<std::collections::BTreeSet<_>>();
                        if relations!=std::collections::BTreeSet::from(["own","allied","neutral","hostile"]) {return Err(format!("Missing viewer relations in map legend: {relations:?}"));}
                        if !view.bot_realms.iter().any(|r|r.name=="Helteria"&&r.relation=="allied")||!view.bot_realms.iter().any(|r|r.name=="Petora"&&r.relation=="hostile") {return Err("Native realm relation labels are stale".into());}
                    } else if !view.diplomacy_legends.iter().any(|r|r.name=="Eastbank"&&r.relation=="hostile"&&r.color=="background-color: #be4628;") {return Err("Live relation changed the owner's map color or left a stale badge".into());}
                    let name=match g.step { 2=>"diplomacy-claims",4=>"diplomacy-live-color",_=>"diplomacy-compact" };
                    let folder=std::env::var_os("TERRA_CAPTURE_DIR").ok_or("Set TERRA_CAPTURE_DIR")?;
                    std::fs::create_dir_all(&folder).map_err(|e|e.to_string())?;
                    world.spawn(Screenshot::primary_window()).observe(save_to_disk(std::path::PathBuf::from(folder).join(format!("{name}.png"))))
                        .observe(|_: On<ScreenshotCaptured>,mut g:ResMut<Gallery>| { g.captures+=1; });
                }
                3 => {
                    let mut session=world.resource_mut::<ClientSession>();
                    let uuid=session.uuid.unwrap();
                    session.kingdoms.change_appearance(uuid,g.id.unwrap(),KingdomHeraldry { color:[65,160,75],icon:KingdomIcon::Tree }).map_err(|e|e.to_string())?;
                    session.kingdoms.set_relation(uuid,g.id.unwrap(),DiplomaticTarget::Player(g.ally.unwrap()),DiplomaticRelation::Hostile).map_err(|e|e.to_string())?;
                }
                5 => {
                    for mut window in world.query::<&mut Window>().iter_mut(world) { window.resolution.set_physical_resolution(1024,768); }
                }
                7 => {
                    for mut window in world.query::<&mut Window>().iter_mut(world) { window.resolution.set_physical_resolution(2560,1440); }
                    world.write_message(FullMapAction::Command {control:"fullmap-claim-protected".into(),id:None});
                }
                8 => { map_pointer(world,Vec2::new(0.51,0.51))?; world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left); }
                9 => { map_pointer(world,Vec2::new(0.60,0.60))?; }
                10 => { world.resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left); }
                11 => {
                    let view=world.resource::<FullMapView>();
                    if view.claim_draft_count=="0 chunks selected" || !view.claim_can_apply { return Err("Dragging did not stage a selection".into()); }
                    g.selected_chunks=view.claim_draft_count.split_whitespace().next().and_then(|s|s.parse().ok()).ok_or("Missing selection count")?;
                    let folder=std::env::var_os("TERRA_CAPTURE_DIR").ok_or("Set TERRA_CAPTURE_DIR")?;
                    world.spawn(Screenshot::primary_window()).observe(save_to_disk(std::path::PathBuf::from(folder).join("diplomacy-rectangle-preview.png")))
                        .observe(|_:On<ScreenshotCaptured>,mut g:ResMut<Gallery>|{g.captures+=1;});
                }
                12 => {world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);}
                13 => {world.resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Right);}
                14 => {
                    let selected=world.resource::<FullMapView>().claim_draft_count.split_whitespace().next().and_then(|s|s.parse::<usize>().ok()).ok_or("Missing remaining count")?;
                    if selected+1!=g.selected_chunks {return Err("Right-click did not unselect exactly one draft chunk".into());}
                    let folder=std::env::var_os("TERRA_CAPTURE_DIR").ok_or("Set TERRA_CAPTURE_DIR")?;
                    world.spawn(Screenshot::primary_window()).observe(save_to_disk(std::path::PathBuf::from(folder).join("diplomacy-rightclick-unselect.png")))
                        .observe(|_:On<ScreenshotCaptured>,mut g:ResMut<Gallery>|{g.captures+=1;});
                }
                15 => {world.write_message(FullMapAction::Command {control:"fullmap-claim-cancel".into(),id:None});}
                _ => {
                    if world.resource::<FullMapView>().claim_draft_count!="0 chunks selected" {return Err("Cancel did not clear the staged claims".into());}
                    if g.captures<5 { return Ok(false); }
                    info!("Native claims projection gallery passed: all realm colors, native village footprint, right-click unselect, borderless draft, compact layout; no authority mutation or OS input");
                    return Ok(true);
                }
            }
            g.step+=1; g.since=Instant::now(); Ok(false)
        })();
        match result { Ok(true)=>{world.write_message(AppExit::Success);},Ok(false)=>{},Err(e)=>{error!(%e,"Claims gallery failed");world.write_message(AppExit::error());} }
    });
}

fn map_pointer(world: &mut World, uv: Vec2) -> Result<(), String> {
    let point = world
        .query::<(
            &tilt_ui::ElementId,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
        .find(|(id, _, _)| id.0 == "full-map-viewport")
        .map(|(_, node, transform)| {
            transform.transform_point2((uv - Vec2::splat(0.5)) * node.size())
        })
        .ok_or("Missing map viewport")?;
    for mut window in world.query::<&mut Window>().iter_mut(world) {
        // Keep synthetic cursor/focus inside ECS; do not ask Winit to move the
        // operating-system pointer (unsupported for unlocked Wayland windows).
        let window = window.bypass_change_detection();
        window.focused = true;
        window.set_physical_cursor_position(Some(point.as_dvec2()));
    }
    Ok(())
}
