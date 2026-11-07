//! Native responsive gallery reached only after the real Bergfried founding ack.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    ui::UiGlobalTransform,
};
use std::{path::PathBuf, time::Instant};
use terra_core::kingdom::{
    BotKingdom, DiplomaticRelation, DiplomaticTarget, KingdomIcon, KingdomRole,
};
use terra_network::client::ClientSession;
use terra_network::protocol::kingdom::{KingdomAction, KingdomOutcome};
use terra_ui::{ElementId, KingdomMenuAction, KingdomView};

pub(crate) fn requested() -> bool {
    std::env::var("TERRA_KINGDOM_UI_GALLERY").as_deref() == Ok("1")
}

#[derive(Resource)]
struct Gallery {
    step: u8,
    since: Instant,
    captures: usize,
    capture_pending: bool,
    failed: bool,
    selected: Option<KingdomIcon>,
    member: Option<terra_core::PlayerUuid>,
}
// A real second client exercises fresh membership and server acknowledgements.
// Its separate App remains on the native main thread; no replica is fabricated.
struct GalleryPeer {
    app: App,
    request: u64,
}
#[derive(Resource)]
struct PeerKeep([i64; 3]);
fn peer_prediction(
    mut commands: Commands,
    mut session: ResMut<ClientSession>,
    target: Res<PeerKeep>,
    simulation: terra_physics::character::CharacterSimulation,
    mut bodies: terra_physics::character::ControlledBodies,
) {
    if !session.ready {
        return;
    }
    let Some(confirmed) = session.confirmed_state() else {
        return;
    };
    if bodies.is_empty() {
        commands.spawn(terra_physics::character::body_bundle(Vec3::from_array(
            confirmed.position,
        )));
        return;
    }
    let eye = Vec3::from_array(confirmed.position) + Vec3::Y * terra_player::PLAYER_EYE_OFFSET;
    let direction = (Vec3::from_array(target.0.map(|v| v as f32 + 0.5)) - eye).normalize_or_zero();
    session.look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
    for (entity, mut transform, mut velocity, collider, mut grounded, _) in &mut bodies {
        let mut state = terra_physics::character::CharacterState {
            position: transform.translation,
            velocity: velocity.0,
            grounded: grounded.0,
        };
        session.predict(
            entity,
            collider,
            &mut state,
            Default::default(),
            &simulation,
        );
        transform.translation = state.position;
        velocity.0 = state.velocity;
        grounded.0 = state.grounded;
    }
}
fn create_peer(world: &mut World) -> Result<terra_core::PlayerUuid, String> {
    let address = world.resource::<terra_server::IntegratedServer>().address;
    let profile = terra_core::PlayerProfile {
        name: "F57 Visitor".into(),
        ..default()
    };
    let token = terra_network::browser::join(address, &profile)?;
    let mut config = world.resource::<terra_core::GameConfig>().clone();
    config.client.view_distance = 1;
    config.client.vertical_view_distance = 1;
    let keep = world
        .resource::<ClientSession>()
        .uuid
        .and_then(|id| {
            world
                .resource::<ClientSession>()
                .kingdoms
                .active_for_member(id)
        })
        .ok_or("Missing real founder")?
        .location;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(terra_core::TerraCorePlugin(config))
        .add_plugins((
            terra_world::TerraWorldPlugin {
                mode: terra_world::WorldMode::Client,
            },
            terra_physics::TerraPhysicsPlugin,
            terra_network::TerraNetworkPlugin(terra_network::connection::NetworkRole::Client),
        ))
        .insert_resource(terra_network::client::ClientLaunch { token })
        .insert_resource(profile.clone())
        .insert_resource(PeerKeep(keep))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 60.0),
        ))
        .add_systems(FixedUpdate, peer_prediction);
    app.finish();
    app.cleanup();
    let uuid = profile.uuid;
    world.insert_non_send(GalleryPeer { app, request: 0 });
    Ok(uuid)
}
fn peer_request(world: &mut World, action: KingdomAction) -> Result<(), String> {
    let mut peer = world.non_send_mut::<GalleryPeer>();
    let command = peer
        .app
        .world_mut()
        .resource_mut::<ClientSession>()
        .kingdom_request(action)
        .ok_or("Secondary request limit reached")?;
    peer.request = command.0.request;
    peer.app.world_mut().write_message(command);
    Ok(())
}
fn own_kingdom(world: &World) -> Result<terra_core::kingdom::KingdomId, String> {
    let session = world.resource::<ClientSession>();
    session
        .uuid
        .and_then(|uuid| session.kingdoms.active_for_member(uuid))
        .map(|k| k.id)
        .ok_or("Missing founder kingdom".into())
}
fn role_ack(world: &World, member: terra_core::PlayerUuid, role: KingdomRole) -> bool {
    let Ok(id) = own_kingdom(world) else {
        return false;
    };
    !world.resource::<KingdomView>().pending
        && world
            .resource::<ClientSession>()
            .kingdoms
            .role_for(id, member)
            == Some(role)
        && world
            .non_send::<GalleryPeer>()
            .app
            .world()
            .resource::<ClientSession>()
            .kingdoms
            .role_for(id, member)
            == Some(role)
}
fn element(world: &mut World, id: &str) -> Option<Entity> {
    world
        .query::<(Entity, &ElementId, &ComputedNode)>()
        .iter(world)
        .find_map(|(entity, name, node)| {
            (name.0 == id && node.size().min_element() > 0.0).then_some(entity)
        })
}
fn rect(world: &World, entity: Entity) -> Option<Rect> {
    let node = world.get::<ComputedNode>(entity)?;
    let pose = world.get::<UiGlobalTransform>(entity)?;
    let half = node.size() * 0.5;
    let (low, high) = [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(-half.x, half.y),
        Vec2::new(half.x, half.y),
    ]
    .into_iter()
    .map(|p| pose.transform_point2(p))
    .fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(low, high), p| (low.min(p), high.max(p)),
    );
    Some(Rect::from_corners(low, high))
}
fn within(inside: Rect, outside: Rect) -> bool {
    let epsilon = Vec2::splat(2.0);
    (inside.min + epsilon).cmpge(outside.min).all()
        && (inside.max - epsilon).cmple(outside.max).all()
}
fn belongs(world: &World, mut entity: Entity, root: Entity) -> bool {
    for _ in 0..48 {
        if entity == root {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}
fn bounds(world: &mut World, picker: bool) -> Result<(), String> {
    let id = if picker {
        "kingdom-icon-picker"
    } else {
        "kingdom-dialog"
    };
    let panel = element(world, id).ok_or_else(|| format!("Missing {id}"))?;
    let panel_rect = rect(world, panel).ok_or("Missing kingdom panel layout")?;
    let window = world
        .query::<&Window>()
        .iter(world)
        .next()
        .ok_or("Missing window")?;
    if !within(
        panel_rect,
        Rect::from_corners(Vec2::ZERO, window.resolution.physical_size().as_vec2()),
    ) {
        return Err(format!(
            "Kingdom panel escapes the viewport: {panel_rect:?}"
        ));
    }
    let texts = world
        .query::<(Entity, &Text)>()
        .iter(world)
        .map(|(e, t)| (e, t.0.clone()))
        .collect::<Vec<_>>();
    for (entity, text) in texts {
        if text.is_empty() || !belongs(world, entity, panel) {
            continue;
        }
        let Some(text_rect) = rect(world, entity) else {
            continue;
        };
        let mut parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
        for _ in 0..12 {
            let Some(ancestor) = parent else {
                break;
            };
            if world.get::<tilt_ui::TiltButton>(ancestor).is_some() {
                let button = rect(world, ancestor).ok_or("Missing kingdom button layout")?;
                if !within(text_rect, button) {
                    return Err(format!(
                        "Kingdom label escapes its button: {text:?}; {text_rect:?} / {button:?}"
                    ));
                }
                break;
            }
            if ancestor == panel {
                break;
            }
            parent = world.get::<ChildOf>(ancestor).map(ChildOf::parent);
        }
    }
    Ok(())
}
fn role_editor_visible(world: &mut World) -> Result<bool, String> {
    let editor =
        element(world, "kingdom-role-editor").ok_or("Missing actual member role editor")?;
    let scroll = element(world, "kingdom-scroll").ok_or("Missing kingdom scroll viewport")?;
    let editor_rect = rect(world, editor).ok_or("Missing role editor layout")?;
    let viewport = rect(world, scroll).ok_or("Missing scroll viewport layout")?;
    if within(editor_rect, viewport) {
        return Ok(true);
    }
    let mut position = world
        .get_mut::<bevy::ui::ScrollPosition>(scroll)
        .ok_or("Kingdom viewport cannot scroll")?;
    position.0.y += if editor_rect.max.y > viewport.max.y {
        128.0
    } else {
        -128.0
    };
    Ok(false)
}
fn scroll_to_top(world: &mut World) {
    if let Some(scroll) = element(world, "kingdom-scroll")
        && let Some(mut position) = world.get_mut::<bevy::ui::ScrollPosition>(scroll)
    {
        position.0.y = 0.0;
    }
}
fn colorpicker_theme(world: &mut World) -> Result<(), String> {
    let owner = element(world, "kingdom-color").ok_or("Missing real colorpicker")?;
    let popup = world
        .query::<(Entity, &tilt_ui::ControlPart, &BackgroundColor)>()
        .iter(world)
        .find_map(|(entity, part, background)| {
            (part.owner == owner && part.kind == tilt_ui::ControlPartKind::Popup)
                .then_some((entity, background.0.to_srgba()))
        })
        .ok_or("Missing rendered colorpicker popup")?;
    if popup.1.red > 0.5 || popup.1.green > 0.5 || popup.1.blue > 0.5 {
        return Err(format!(
            "Kingdom colorpicker popup has a light background: {:?}",
            popup.1
        ));
    }
    let panel = rect(world, popup.0).ok_or("Missing colorpicker popup layout")?;
    if panel.width() < 150.0 || panel.height() < 150.0 {
        return Err("Real colorpicker popup is clipped or hidden".into());
    }
    Ok(())
}
fn shot(world: &mut World, gallery: &mut Gallery, name: &str) -> Result<(), String> {
    let root = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .ok_or("Set TERRA_CAPTURE_DIR")?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(root.join(format!("{name}.png"))))
        .observe(|event: On<ScreenshotCaptured>, mut g: ResMut<Gallery>| {
            let colors = event
                .image
                .data
                .as_ref()
                .map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .step_by(113)
                        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
                        .collect::<std::collections::HashSet<_>>()
                })
                .unwrap_or_default();
            g.failed |= colors.len() < 16;
            g.captures += 1;
            g.capture_pending = false;
        });
    gallery.capture_pending = true;
    Ok(())
}

pub(crate) fn run(world: &mut World) -> Result<bool, String> {
    if !world.contains_resource::<Gallery>() {
        world.insert_resource(Gallery {
            step: 0,
            since: Instant::now(),
            captures: 0,
            capture_pending: false,
            failed: false,
            selected: None,
            member: None,
        });
    }
    if world.contains_non_send::<GalleryPeer>() {
        world.non_send_mut::<GalleryPeer>().app.update();
    }
    world.resource_scope(|world, mut g: Mut<Gallery>| {
        if g.failed { return Err("Kingdom gallery screenshot has no rendered UI".into()); }
        // Screenshot capture/readback is asynchronous. Do not mutate the scene
        // until its observer confirms the previous named state was captured.
        if g.capture_pending || g.since.elapsed().as_secs_f32() < 1.25 { return Ok(false); }
        match g.step {
            0 => {
                for mut window in world.query::<&mut Window>().iter_mut(world) { window.resolution.set_physical_resolution(2560,1440); }
            }
            1 => {
                bounds(world,false)?;
                shot(world,&mut g,"kingdom-ui-qhd")?;
            }
            2 => { world.write_message(KingdomMenuAction::OpenIcons); }
            3 => {
                let view=world.resource::<KingdomView>();
                if !view.icon_picker || view.icons.len() < 500 { return Ok(false); }
                bounds(world,true)?;
                shot(world,&mut g,"kingdom-ui-icons-qhd")?;
            }
            4 => { world.write_message(KingdomMenuAction::SearchIcons("shield".into())); }
            5 => {
                let view=world.resource::<KingdomView>();
                if view.icons.is_empty() || !view.icons.iter().all(|icon| icon.key.contains("shield")) { return Ok(false); }
                g.selected=Some(KingdomIcon::from_key(&view.icons[0].key).ok_or("Unregistered picker result")?);
                shot(world,&mut g,"kingdom-ui-icons-search")?;
            }
            6 => {
                world.write_message(KingdomMenuAction::Icon(g.selected.ok_or("Missing selected emblem")?));
            }
            7 => { world.write_message(KingdomMenuAction::SaveAppearance); }
            8 => {
                let view=world.resource::<KingdomView>();
                if view.pending { return Ok(false); }
                if !view.error.is_empty() { return Err(view.error.clone()); }
                let session=world.resource::<ClientSession>();
                if !session.uuid.and_then(|id| session.kingdoms.active_for_member(id)).is_some_and(|k| Some(k.heraldry.icon)==g.selected) {
                    return Err("Full catalog emblem did not survive the authoritative appearance ack".into());
                }
                let picker=element(world,"kingdom-color").ok_or("Missing real colorpicker")?;
                if !tilt_ui::set_color_picker_open(world,picker,true) { return Err("Could not open real colorpicker".into()); }
            }
            9 => {
                colorpicker_theme(world)?;
                shot(world,&mut g,"kingdom-ui-colorpicker")?;
            }
            10 => {
                let picker=element(world,"kingdom-color").ok_or("Missing colorpicker")?;
                tilt_ui::set_color_picker_open(world,picker,false);
            }
            11 => {
                for mut window in world.query::<&mut Window>().iter_mut(world) { window.resolution.set_physical_resolution(1024,768); }
            }
            12 => {
                bounds(world,false)?;
                shot(world,&mut g,"kingdom-ui-compact")?;
            }
            13 => { world.write_message(KingdomMenuAction::OpenIcons); }
            14 => {
                if !world.resource::<KingdomView>().icon_picker { return Ok(false); }
                bounds(world,true)?;
                shot(world,&mut g,"kingdom-ui-icons-compact")?;
            }
            15 => { world.write_message(KingdomMenuAction::CloseIcons); }
            16 => {
                for mut window in world.query::<&mut Window>().iter_mut(world) { window.resolution.set_physical_resolution(3840,2160); }
            }
            17 => {
                bounds(world,false)?;
                shot(world,&mut g,"kingdom-ui-4k")?;
            }
            18 => { world.write_message(KingdomMenuAction::OpenIcons); }
            19 => {
                if !world.resource::<KingdomView>().icon_picker { return Ok(false); }
                bounds(world,true)?;
                shot(world,&mut g,"kingdom-ui-icons-4k")?;
            }
            20 => { world.write_message(KingdomMenuAction::CloseIcons); }
            21 => { g.member=Some(create_peer(world)?);world.write_message(KingdomMenuAction::Tab("members".into())); }
            22 => {
                let session=world.non_send::<GalleryPeer>().app.world().resource::<ClientSession>();
                if !session.ready || !session.confirmed_state().is_some_and(|state|state.ack>0){return Ok(false);}
                let id=own_kingdom(world)?;peer_request(world,KingdomAction::Join{id})?;
            }
            23 => {
                let member=g.member.ok_or("Missing joined player")?;
                if !role_ack(world,member,KingdomRole::Visitor){return Ok(false);}
                if !world.resource::<KingdomView>().members.iter().any(|m|m.id==member.to_string()&&m.role=="Visitor"&&m.permissions=="No protected-land permissions"){return Err("Fresh member UI did not show Visitor capabilities".into());}
                bounds(world,false)?;shot(world,&mut g,"kingdom-rights-visitor")?;
            }
            24 => {world.write_message(KingdomMenuAction::EditRole(g.member.ok_or("Missing member")?));}
            25 => {
                if !role_editor_visible(world)? {g.since=Instant::now();return Ok(false);}
                bounds(world,false)?;shot(world,&mut g,"kingdom-role-choices")?;
            }
            26 => {world.write_message(KingdomMenuAction::SetRole{member:g.member.ok_or("Missing member")?,role:KingdomRole::Worker});}
            27 => {
                if !role_ack(world,g.member.ok_or("Missing member")?,KingdomRole::Worker){return Ok(false);}
                bounds(world,false)?;shot(world,&mut g,"kingdom-rights-worker")?;
            }
            28 => {scroll_to_top(world);world.write_message(KingdomMenuAction::Tab("diplomacy".into()));}
            29 => {
                if world.resource::<KingdomView>().diplomacy.len()<4{return Err("Missing NPC diplomacy targets".into());}
                bounds(world,false)?;shot(world,&mut g,"kingdom-diplomacy-neutral")?;
            }
            30 => {world.write_message(KingdomMenuAction::SetRelation{target:DiplomaticTarget::Bot(BotKingdom::Helteria),relation:DiplomaticRelation::Allied});}
            31 => {
                let view = world.resource::<KingdomView>();
                if view.pending { return Ok(false); }
                if !view.error.is_empty() { return Err(view.error.clone()); }
                if !view.diplomacy.iter().any(|r|r.key=="bot:helteria"&&r.relation_key=="allied"&&!r.pending_alliance){return Err("Helteria alliance was not confirmed by server".into());}
                bounds(world,false)?;shot(world,&mut g,"kingdom-diplomacy-allied")?;
            }
            32 => {world.write_message(KingdomMenuAction::SetRelation{target:DiplomaticTarget::Bot(BotKingdom::Lutera),relation:DiplomaticRelation::Allied});}
            33 => {
                let view = world.resource::<KingdomView>();
                if view.pending { return Ok(false); }
                if !view.error.is_empty() { return Err(view.error.clone()); }
                if !view.diplomacy.iter().any(|r|r.key=="bot:lutera"&&r.relation_key=="neutral"&&r.pending_alliance&&r.hint.contains("level 2")){return Err("Level 1 alliance offer must remain pending".into());}
                bounds(world,false)?;shot(world,&mut g,"kingdom-diplomacy-pending")?;
            }
            34 => {world.write_message(KingdomMenuAction::Tab("members".into()));world.write_message(KingdomMenuAction::SetRole{member:g.member.ok_or("Missing member")?,role:KingdomRole::Visitor});}
            35 => {if !role_ack(world,g.member.ok_or("Missing member")?,KingdomRole::Visitor){return Ok(false);}let id=own_kingdom(world)?;peer_request(world,KingdomAction::SetRole{id,member:g.member.ok_or("Missing member")?,role:KingdomRole::Officer})?;}
            36 => {
                let peer=world.non_send::<GalleryPeer>();let session=peer.app.world().resource::<ClientSession>();
                let Some(result)=session.last_kingdom_result.as_ref().filter(|r|r.request==peer.request)else{return Ok(false);};
                if result.outcome!=KingdomOutcome::Rejected(terra_core::kingdom::KingdomError::PermissionDenied){return Err(format!("Visitor escalation was not denied: {:?}",result.outcome));}
                bounds(world,false)?;shot(world,&mut g,"kingdom-rights-revoked")?;
            }
            37 => {
                // Retire the secondary link first, then reconnect the actual
                // rendered client with that authenticated Visitor profile.
                let mut peer=world.non_send_mut::<GalleryPeer>();
                terra_network::client::disconnect_session(peer.app.world_mut());peer.app.update();
            }
            38 => {
                let peer=world.remove_non_send::<GalleryPeer>().ok_or("Missing secondary link")?;
                let profile=peer.app.world().resource::<terra_core::PlayerProfile>().clone();drop(peer);
                let token=terra_network::browser::join(world.resource::<terra_server::IntegratedServer>().address,&profile)?;
                terra_network::client::disconnect_session(world);world.insert_resource(profile);
                terra_network::client::connect_session(world,token);
            }
            39 => {
                let session=world.resource::<ClientSession>();
                if !session.ready || session.uuid!=g.member || session.confirmed_state().is_none(){return Ok(false);}
                let keep=session.uuid.and_then(|uuid|session.kingdoms.active_for_member(uuid)).ok_or("Visitor membership was not replicated on reconnect")?.location;
                if world.resource::<terra_world::streaming::ChunkStore>().block_at(keep)!=Some(terra_core::castle::BERGFRIED){return Ok(false);}
                world.write_message(KingdomMenuAction::Open(keep));
            }
            40 => {
                let view=world.resource::<KingdomView>();if !view.visible||view.pending{return Ok(false);}
                if !view.error.is_empty(){return Err(view.error.clone());}
                if view.own_role!="Visitor"||view.manager_controls||view.founder_controls||view.members.iter().any(|m|m.can_edit_role||m.can_kick){return Err("Authenticated Visitor received management controls".into());}
                world.write_message(KingdomMenuAction::Tab("members".into()));
            }
            41 => {
                let view=world.resource::<KingdomView>();if !view.members_tab||view.capabilities.iter().any(|p|p.allowed){return Err("Authenticated Visitor must have no protected-land capabilities".into());}
                bounds(world,false)?;shot(world,&mut g,"kingdom-member-readonly")?;
            }
            42 => {world.write_message(KingdomMenuAction::Tab("diplomacy".into()));}
            43 => {
                if world.resource::<KingdomView>().diplomacy.iter().any(|r|r.can_neutral||r.can_ally||r.can_hostile)||element(world,"kingdom-relation-allied").is_some(){return Err("Readonly diplomacy exposed relation controls".into());}
                bounds(world,false)?;shot(world,&mut g,"kingdom-diplomacy-readonly")?;
            }
            _ => {
                if g.captures < 17 { return Ok(false); }
                info!(captures=g.captures,"Native Kingdom UI gallery passed: real founding/appearance ack, full icon search, themed colorpicker, QHD/1024x768/4K frames, real secondary Visitor join/Worker grant/revocation, rejected self-promotion, NPC allied/pending acknowledgements and authenticated Visitor reconnect/read-only controls");
                return Ok(true);
            }
        }
        g.step+=1; g.since=Instant::now(); Ok(false)
    })
}
