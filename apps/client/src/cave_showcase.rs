//! Opt-in handcrafted lighting fixture. It never changes the production generator or saves.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{CaveQuality, GraphicsConfig};
use terra_voxel::{
    SectionPos,
    blocks::BlockSection,
    meshing::{PADDED_VOLUME, mesh_section},
};
use terra_world::streaming::{
    ChunkFoci, ChunkFocus, ChunkStore, GenerationStatus, LoadedSection, WorldGeneration,
};

// The floor cell is Y=-21, so its upward-facing surface is Y=-20.
const PORTAL_GROUND_TARGET: Vec3 = Vec3::new(3.5, -19.995, 35.5);
const PORTAL_SKY_TARGET: Vec3 = Vec3::new(80.0, 20.0, 35.0);
const LOD_SHADOW_GROUND_TARGET: Vec3 = Vec3::new(-28.5, -30.995, 10.5);

fn lod_shadow_fixture() -> bool {
    static ENABLED: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("TERRA_CAVE_LOD_SHADOW_INSPECT").is_some());
    *ENABLED
}

fn lod_shadow_block([_, y, _]: [i64; 3]) -> u16 {
    // Infinite overburden has no camera-visible top face. Its underside must
    // still block the sun when the actual caster section reaches another LOD.
    if y >= 0 || y <= -32 { 3 } else { 0 }
}

fn lod_shadow_caster(world: &mut World) -> Option<(SectionPos, Vec3)> {
    let layer = bevy::camera::visibility::RenderLayers::default();
    let direction = world
        .query::<(
            &DirectionalLight,
            &Transform,
            Option<&bevy::camera::visibility::RenderLayers>,
        )>()
        .iter(world)
        .filter(|(light, _, layers)| {
            light.illuminance > 1.0
                && light.shadow_maps_enabled
                && layers.is_none_or(|layers| layers.intersects(&layer))
        })
        .max_by(|(a, ..), (b, ..)| a.illuminance.total_cmp(&b.illuminance))
        .map(|(_, transform, _)| transform.rotation * Vec3::Z)?;
    // Terrain lighting may clamp the celestial elevation. Trace the actual
    // light transform, not the unclamped sun drawn by the sky renderer.
    let chunks = world.resource::<ChunkStore>();
    let hit = terra_voxel::blocks::raycast_blocks(
        LOD_SHADOW_GROUND_TARGET.as_dvec3().to_array(),
        direction.as_dvec3().to_array(),
        128.0,
        |cell| chunks.block_at(cell),
    )?;
    (hit.position[1] == 0).then_some((terra_voxel::global_to_section(hit.position).0, direction))
}

#[derive(Resource)]
struct Showcase {
    start: Instant,
    entered: Instant,
    built: bool,
    stage: usize,
    captured: usize,
    requested: bool,
    frames: Vec<f64>,
    contact_time: f32,
    torch_definition: Option<terra_render::blocks::definitions::BlockLight>,
    portal_luminance: Option<(f32, f32)>,
    lod_shadow_luminance: Option<f32>,
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-caves") {
        return;
    }
    let now = Instant::now();
    let lighting_only = std::env::var_os("TERRA_CAVE_LIGHT_INSPECT").is_some();
    let portal_only = std::env::var_os("TERRA_CAVE_PORTAL_INSPECT").is_some();
    let initial_stage = if lod_shadow_fixture() {
        22
    } else if portal_only {
        18
    } else if lighting_only {
        14
    } else {
        0
    };
    let torch_definition = terra_render::blocks::definitions::BlockLibrary::load(
        &terra_core::asset_directory().join("terra/blocks"),
    )
    .ok()
    .and_then(|library| {
        library
            .blocks
            .into_iter()
            .find(|block| block.id == 40)?
            .light
    });
    app.insert_resource(Showcase {
        start: now,
        entered: now,
        built: false,
        stage: initial_stage,
        captured: initial_stage,
        requested: false,
        frames: Vec::new(),
        contact_time: 0.0,
        torch_definition,
        portal_luminance: None,
        lod_shadow_luminance: None,
    })
    .init_resource::<ChunkFoci>()
    .insert_resource(GraphicsConfig::default())
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .insert_resource(terra_player::ControllerMode::Fly)
    .add_systems(
        Update,
        run.after(terra_player::PlayerSet::Movement)
            .before(terra_world::streaming::StreamingSet::Interest),
    );
}

fn eye(stage: usize) -> Transform {
    if stage >= 22 {
        Transform::from_xyz(-28.5, -27.0, 5.5).looking_at(Vec3::new(-28.5, -29.0, 12.0), Vec3::Y)
    } else if stage == 18 {
        Transform::from_xyz(-19.0, -17.0, 35.0).looking_at(Vec3::new(4.0, -16.5, 35.0), Vec3::Y)
    } else if stage == 19 {
        // Keep both world-space photometry targets inside the same frustum.
        // This lip is 1/8 enclosed under the continuous entrance kernel.
        Transform::from_xyz(-6.0, -17.0, 35.0).looking_at(Vec3::new(16.0, -16.5, 35.0), Vec3::Y)
    } else if stage == 20 {
        eye(14)
    } else if stage == 21 {
        eye(16)
    } else if stage >= 16 {
        Transform::from_xyz(-10.2, -26.3, -40.6).looking_at(Vec3::new(-7.3, -27.0, -39.2), Vec3::Y)
    } else if stage >= 14 {
        Transform::from_xyz(-5.8, -24.4, -36.7).looking_at(Vec3::new(-8.1, -26.8, -40.4), Vec3::Y)
    } else if stage == 13 {
        Transform::from_xyz(17.0, 30.0, 19.0).looking_at(Vec3::new(6.0, 19.0, 1.5), Vec3::Y)
    } else if stage == 12 {
        Transform::from_xyz(-1.5, 20.5, 3.0).looking_at(Vec3::new(-3.3, 19.6, 0.4), Vec3::Y)
    } else if stage == 10 {
        Transform::from_xyz(0.0, 65.0, 58.0).looking_at(Vec3::new(0.5, 19.8, 0.5), Vec3::Y)
    } else if stage >= 8 {
        Transform::from_xyz(4.5, 21.3, 6.0).looking_at(Vec3::new(0.5, 19.8, 0.5), Vec3::Y)
    } else if stage == 7 {
        Transform::from_xyz(-2.0, -31.6, 4.5).looking_at(Vec3::new(-4.0, -32.3, 0.0), Vec3::Y)
    } else if stage == 5 {
        Transform::from_xyz(12.5, -29.1, 5.5).looking_at(Vec3::new(11.5, -29.25, 4.5), Vec3::Y)
    } else if stage == 6 {
        Transform::from_xyz(6.4, -28.1, -13.4).looking_at(Vec3::new(5.45, -28.3, -14.5), Vec3::Y)
    } else if stage == 4 {
        Transform::from_xyz(0.5, -25.0, -27.0).looking_at(Vec3::new(0.5, -25.0, -43.0), Vec3::Y)
    } else {
        Transform::from_xyz(11.0, -26.0, 18.0).looking_at(Vec3::new(-2.0, -25.0, -7.0), Vec3::Y)
    }
}

fn block([x, y, z]: [i64; 3]) -> u16 {
    if lod_shadow_fixture() {
        return lod_shadow_block([x, y, z]);
    }
    let xf = x as f64 + 0.5;
    let yf = y as f64 + 0.5;
    let zf = z as f64 + 0.5;
    let relief =
        (xf * 0.29).sin() * (zf * 0.21).cos() * 0.06 + (yf * 0.51 + zf * 0.13).sin() * 0.025;
    let chamber =
        (xf / 24.0).powi(2) + ((yf + 24.0) / 14.0).powi(2) + (zf / 28.0).powi(2) < 1.0 + relief;
    let tunnel = x.abs() <= 2 && (-28..=-23).contains(&y) && (-43..=-20).contains(&z);
    // Follow the 10:00 sun direction through the overburden, not a roofed vertical well.
    let shaft = y >= -25
        && (xf - (-8.0 + (yf + 25.0) * 0.64)).abs() < 4.0
        && (zf - (-15.0 + (yf + 25.0) * 0.484)).abs() < 4.0;
    let mut material = if chamber || tunnel || shaft || y > 18 {
        0
    } else {
        3
    };
    // A closed static lake in this fixture, not a claim of multi-interval natural hydrology.
    if ((xf + 4.0) / 9.0).powi(2) + (zf / 12.0).powi(2) < 1.0 && y <= -33 {
        material = if y == -33 { 14 } else { 3 };
    }
    for (a, b) in [(-13, -7), (9, -12), (-7, 13), (17, 3)] {
        if x == a && z == b && (-36..=-33).contains(&y) {
            material = 37;
        }
    }
    if [(-12, -30, 8), (11, -30, 4), (5, -29, -15)].contains(&(x, y, z)) {
        material = 40;
    }
    if [x, y, z] == [0, 19, 0] {
        material = 40;
    }
    if [[-4, 19, 0], [-3, 19, 0]].contains(&[x, y, z]) {
        material = 28;
    }
    for (tx, ty, tz) in [(-12, -30, 8), (11, -30, 4), (5, -29, -15)] {
        if x == tx && z == tz && y < ty {
            material = 3;
        }
    }
    if x == 4 && z == -15 && (-30..=-27).contains(&y) {
        material = 3;
    }
    if x <= -19 && (-29..=-25).contains(&y) && (-3..=1).contains(&z) {
        material = if x == -19 { 34 } else { 3 };
    }
    if x >= 15 && (-28..=-25).contains(&y) && (-14..=-11).contains(&z) {
        material = if x == 15 { 35 } else { 3 };
    }
    if y == -13 && [(-9, 2), (5, 8), (12, -6)].contains(&(x, z)) {
        material = 37;
    }
    // A close, fully enclosed alcove exposes camera/torch EV mismatches that
    // the wide, paused chamber views do not. Stages 14/15 share this geometry.
    if (-12..=-4).contains(&x) && (-29..=-20).contains(&y) && (-44..=-35).contains(&z) {
        let interior =
            (-11..=-5).contains(&x) && (-28..=-21).contains(&y) && (-43..=-36).contains(&z);
        material = if interior { 0 } else { 3 };
        if z <= -42 && y <= -27 + (x + 12).rem_euclid(3) {
            material = 3;
        }
        if [x, y, z] == [-8, -28, -40] {
            material = 3;
        } else if [x, y, z] == [-8, -27, -40] {
            material = 40;
        }
        if material == 0 && (-11..=-9).contains(&x) && (-43..=-41).contains(&z) && y <= -26 {
            material = 27;
        }
    }
    // A roofed entrance and its genuinely open courtyard share the same floor
    // material. The portal regression compares those exact sunlit world points
    // from an enclosed camera and from outside, not two unrelated screenshots.
    if x >= -25 && z >= 27 && y >= -22 {
        material = if y <= -21 { 3 } else { 0 };
        if x <= -12 && y <= 18 {
            let room =
                (-24..=-12).contains(&x) && (-20..=-12).contains(&y) && (29..=43).contains(&z);
            material = if room { 0 } else { 3 };
        }
    }
    material
}

fn build(world: &mut World) -> Result<(), String> {
    if !lod_shadow_fixture() {
        let mut journal = world.resource_mut::<terra_world::edits::EditJournal>();
        journal
            .record([5, -29, -15], 40)
            .map_err(|e| e.to_string())?;
        // The fixture replaces generated sections after lamp discovery may
        // already have cached them. Invalidate its new alcove source normally.
        journal
            .record([-8, -27, -40], 40)
            .map_err(|e| e.to_string())?;
        // Real fluid occupancy makes the cave-diving check exercise the normal
        // underwater probe, not a forced presentation blend or solid-water cube.
        for x in -11..=-9 {
            for z in -43..=-41 {
                for y in -28..=-26 {
                    let p = [x, y, z];
                    if block(p) == 27 {
                        journal
                            .set_fluid(
                                p,
                                terra_world::water::FluidCell {
                                    amount: terra_world::water::FLUID_CAPACITY,
                                    source: true,
                                    flow: [0; 2],
                                },
                            )
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        journal.set_object_state(
            [5, -29, -15],
            Some(terra_world::edits::BlockObjectState {
                facing: 4,
                ..Default::default()
            }),
        )?;
        for (p, peer, secondary) in [
            ([-4, 19, 0], [-3, 19, 0], false),
            ([-3, 19, 0], [-4, 19, 0], true),
        ] {
            journal.record(p, 28).map_err(|e| e.to_string())?;
            journal.set_object_state(
                p,
                Some(terra_world::edits::BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    ..default()
                }),
            )?;
        }
    }
    let mut sections = Vec::new();
    for z in -3..=2 {
        for y in -3..=3 {
            for x in -2..=1 {
                let position = SectionPos::new(x, y, z);
                let origin = position.origin().map_err(|e| e.to_string())?;
                let mut padded = Vec::with_capacity(PADDED_VOLUME);
                for z in -1..=16 {
                    for y in -1..=16 {
                        for x in -1..=16 {
                            padded.push(block([origin[0] + x, origin[1] + y, origin[2] + z]));
                        }
                    }
                }
                let mut mesh = mesh_section(&padded).map_err(|e| e.to_string())?;
                mesh.mineral_growth =
                    terra_world::ore_shapes::salt_growth_snapshot(&mesh.faces, origin, |p| {
                        Some(block(p))
                    });
                // Deliberate fixture anchors bypass only the natural rarity roll,
                // not clearance or the real shared stalactite/stalagmite renderer.
                for face in &mesh.faces {
                    if face.material != 37 || !matches!(face.side, 2 | 3) {
                        continue;
                    }
                    let cell = face.minimum.map(|v| v as u8);
                    let p = std::array::from_fn::<_, 3, _>(|i| origin[i] + i64::from(cell[i]));
                    let sign = if face.side == 2 { -1 } else { 1 };
                    let free = (1..=14)
                        .take_while(|d| block([p[0], p[1] + d * sign, p[2]]) == 0)
                        .count();
                    let length = (free as f32 - 0.1).min(if face.side == 2 { 8.0 } else { 3.0 });
                    if length >= 2.0 {
                        mesh.mineral_growth
                            .retain(|(c, s, _)| *c != cell || usize::from(*s) != face.side);
                        mesh.mineral_growth.push((cell, face.side as u8, length));
                    }
                }
                let mut blocks = Vec::with_capacity(4096);
                for z in 0..16 {
                    for y in 0..16 {
                        for x in 0..16 {
                            blocks.push(block([origin[0] + x, origin[1] + y, origin[2] + z]));
                        }
                    }
                }
                let data =
                    BlockSection::from_blocks(position, 1, blocks).map_err(|e| e.to_string())?;
                sections.push((
                    position,
                    LoadedSection {
                        data,
                        mesh: Some(mesh),
                        mesh_epoch: world
                            .resource::<terra_world::edits::EditJournal>()
                            .mesh_epoch(position),
                        water_epoch: world
                            .resource::<terra_world::edits::EditJournal>()
                            .water_mesh_epoch(position),
                        water: (position == SectionPos::new(-1, -2, -3)).then(|| {
                            terra_world::water::WaterPatch {
                                positions: vec![
                                    [5.0, 7.0, 5.0],
                                    [5.0, 7.0, 8.0],
                                    [8.0, 7.0, 8.0],
                                    [8.0, 7.0, 5.0],
                                ],
                                normals: vec![[0.0, 1.0, 0.0]; 4],
                                depth_shore: vec![[2.5, 0.0]; 4],
                                flow: vec![[0.0; 2]; 4],
                                surface_effects: vec![[0.0; 4]; 4],
                                wave_weights: vec![0.0; 4],
                                indices: vec![0, 1, 2, 0, 2, 3],
                            }
                        }),
                        props: Vec::new(),
                    },
                ));
            }
        }
    }
    let positions: Vec<_> = sections.iter().map(|(p, _)| *p).collect();
    terra_render::meshing::forget_section_snapshots(world, &positions);
    // Replace only this fixture's prior roots. Keep the real journal epoch;
    // the upload path correctly refuses fabricated/inconsistent worker epochs.
    for (position, _) in &sections {
        if let Some(old) = world
            .resource_mut::<terra_render::RenderedSections>()
            .sections
            .remove(position)
        {
            world.despawn(old.entity);
            for mesh in old.meshes {
                world.resource_mut::<Assets<Mesh>>().remove(mesh.id());
            }
        }
    }
    info!(
        sections = sections.len(),
        "Handcrafted cave fixture installed without modifying world generation"
    );
    world.resource_mut::<ChunkStore>().loaded.extend(sections);
    Ok(())
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Showcase>| {
        let lighting_only=std::env::var_os("TERRA_CAVE_LIGHT_INSPECT").is_some();
        let portal_only=std::env::var_os("TERRA_CAVE_PORTAL_INSPECT").is_some();
        let lod_shadow_only=lod_shadow_fixture();
        let stages=if lod_shadow_only {25}else if portal_only {22}else if lighting_only {18}else if std::env::var_os("TERRA_TORCH_INSPECT").is_some(){16}else{8};
        if state.stage>=stages {return;}
        if state.start.elapsed()>Duration::from_secs(180) || world.resource::<terra_render::RenderStatus>().error.is_some() {
            error!("Cave showcase failed"); state.stage=8; world.write_message(AppExit::error()); return;
        }
        let camera=eye(state.stage);
        for mut transform in world.query_filtered::<&mut Transform,With<terra_player::TerraCamera>>().iter_mut(world) {*transform=camera;}
        // Keep the authored room and the moving camera resident. Reloading
        // authored sections would restore procedural terrain, not this room.
        let focus=camera.translation.to_array().map(f64::from);
        world.resource_mut::<ChunkFocus>().0=focus;
        world.resource_mut::<ChunkFoci>().0=Some(vec![[0.0;3],focus]);
        if !state.built {
            if world.resource::<WorldGeneration>().status!=GenerationStatus::Ready {return;}
            if let Err(error)=build(world) {error!(%error); state.stage=8; world.write_message(AppExit::error());return;}
            state.built=true; state.entered=Instant::now(); return;
        }
        // The fixture has its own roof/opening. Procedural mountains outside
        // its bounds must not cast a second, unrelated roof over this test.
        let outside:Vec<_>=world.resource::<terra_render::RenderedSections>().sections.iter()
            .filter(|(p,_)| !(-2..=1).contains(&p.x)|| !(-3..=3).contains(&p.y)|| !(-3..=2).contains(&p.z))
            .map(|(_,s)|s.entity).collect();
        for entity in outside {
            if let Some(mut visibility)=world.get_mut::<Visibility>(entity)
                && *visibility!=Visibility::Hidden {
                *visibility=Visibility::Hidden;
            }
        }
        let quality=match state.stage {0=>CaveQuality::Low,2=>CaveQuality::High,3=>CaveQuality::Ultra,_=>CaveQuality::Medium};
        if state.stage>=14 {
            let phase=if lod_shadow_only {8.0/24.0}else if state.stage==17 {0.0}else if state.stage==15 {0.5 + f64::from(world.resource::<Time>().elapsed_secs())*0.000001}else{0.5};
            let mut clock=world.resource_mut::<terra_core::WorldClock>();
            if clock.phase!=phase {clock.phase=phase;}
            let paused=state.stage!=15;
            if clock.paused!=paused {clock.paused=paused;}
        } else if state.stage>=8 {
            let mut clock=world.resource_mut::<terra_core::WorldClock>();
            if clock.phase!=0.0 {clock.phase=0.0;}
            if !clock.paused {clock.paused=true;}
        }
        if state.stage==11 && !world.contains_resource::<terra_network::client::ReplicatedPlayers>() {
            use terra_network::{client::{ReplicatedPlayers,RemoteTrack},protocol::session::PlayerState};
            let pose=PlayerState{player:terra_core::PlayerId(777),position:[3.0,20.0,1.0],held_torch:true,held_item:Some(terra_core::ItemKind::Block(40)),..default()};
            let mut players=ReplicatedPlayers::default();
            players.players.insert(pose.player,RemoteTrack{previous:pose,current:pose,received:Instant::now(),interval:0.05});
            world.insert_resource(players);
        }
        let now=world.resource::<Time>().elapsed_secs();
        if state.stage==7 && now-state.contact_time>0.25 {
            use terra_core::footsteps::{WaterMotion,WaterMotionKind};
            state.contact_time=now;
            for kind in [WaterMotionKind::Splash,WaterMotionKind::Wake,WaterMotionKind::Bubble] {
                world.write_message(WaterMotion{position:Vec3::new(-4.0,if kind==WaterMotionKind::Bubble{-32.65}else{-32.02},0.0),velocity:Vec3::new(1.0,-6.0,0.0),strength:1.2,kind});
            }
        }
        if state.stage==14 && now-state.contact_time>0.25 {
            state.contact_time=now;
            world.write_message(terra_core::footsteps::WaterMotion {
                position:Vec3::new(-10.2,-24.98,-41.6),
                velocity:Vec3::new(1.2,0.0,1.4),strength:0.6,
                kind:terra_core::footsteps::WaterMotionKind::Wake,
            });
        }
        if world.resource::<GraphicsConfig>().cave_quality!=quality { world.resource_mut::<GraphicsConfig>().cave_quality=quality; }
        if lod_shadow_only {
            let (near,far)=match state.stage {22=>(96.0,160.0),23=>(16.0,96.0),_=>(16.0,20.0)};
            let mut config=world.resource_mut::<terra_core::GameConfig>();
            if config.client.lod_near_distance!=near || config.client.lod_far_distance!=far || config.client.lod_hysteresis!=0.0 {
                config.client.lod_near_distance=near;
                config.client.lod_far_distance=far;
                config.client.lod_hysteresis=0.0;
            }
        }
        if !world.resource::<terra_render::RenderStatus>().ready || state.entered.elapsed()<Duration::from_secs(4) {return;}
        if lod_shadow_only {
            let expected=match state.stage {22=>terra_render::meshing::DetailLevel::Near,23=>terra_render::meshing::DetailLevel::Middle,_=>terra_render::meshing::DetailLevel::Far};
            let Some((caster,_))=lod_shadow_caster(world) else {
                error!("Cave LOD fixture has no active sunlight ray hitting its actual ceiling");
                state.stage=stages;world.write_message(AppExit::error());return;
            };
            if world.resource::<terra_render::RenderedSections>().sections.get(&caster).is_none_or(|s|s.detail_level!=expected) {
                state.frames.clear();
                return;
            }
        }
        if !world.resource::<terra_render::TorchLightingStatus>().ready {
            error!("Native Bevy PBR source did not accept the guarded torch attenuation extension");
            state.stage=stages; world.write_message(AppExit::error()); return;
        }
        if !state.requested {
            state.frames.push(f64::from(world.resource::<Time>().delta_secs())*1000.0);
            let frame_count=if lighting_only || portal_only || lod_shadow_only {60}else{180};
            if state.frames.len()<frame_count {return;}
            if lod_shadow_only {
                let (caster,sun_direction)=lod_shadow_caster(world).expect("active ceiling caster was verified before sampling");
                let (root,level)={let section=&world.resource::<terra_render::RenderedSections>().sections[&caster];(section.entity,section.detail_level)};
                let children=world.get::<Children>(root).map_or_else(Vec::new,|children|children.iter().collect::<Vec<_>>());
                let layer=bevy::camera::visibility::RenderLayers::default();
                let mut shared=0;
                let mut standard=0;
                let mut proxies=0;
                let mut mesh_count=0;
                for child in children {
                    let Some(mesh)=world.get::<Mesh3d>(child) else{continue;};
                    if !world.resource::<Assets<Mesh>>().contains(mesh.0.id()) {continue;}
                    mesh_count+=1;
                    if world.get::<bevy::camera::visibility::RenderLayers>(child).is_some_and(|layers|!layers.intersects(&layer)) {
                        proxies+=1;
                    } else if terra_render::meshing::uses_shared_surface_material(world,child) {
                        shared+=1;
                    } else if world.get::<MeshMaterial3d<StandardMaterial>>(child).is_some() {
                        standard+=1;
                    }
                }
                let active_suns=world.query::<(&DirectionalLight,Option<&bevy::camera::visibility::RenderLayers>)>().iter(world)
                    .filter(|(light,layers)|light.illuminance>1.0 && light.shadow_maps_enabled && layers.is_none_or(|layers|layers.intersects(&layer))).count();
                let reference=std::env::var("TERRA_CAVE_LOD_SHADOW_REFERENCE").is_ok_and(|value|value=="1");
                let material_matches=if reference {standard>0 && shared==0}else{shared>0 && standard==0};
                let visible=world.get::<InheritedVisibility>(root).is_some_and(|visibility|visibility.get());
                info!(stage=state.stage,?caster,?sun_direction,?level,mesh_count,shared,standard,proxies,active_suns,visible,reference,
                    "Native cave LOD actual caster material and live sunlight verified");
                if !material_matches || active_suns==0 || !visible || mesh_count==0 {
                    error!("Cave LOD fixture did not render the expected real ceiling material with active sunlight");
                    state.stage=stages;world.write_message(AppExit::error());return;
                }
            }
            if state.stage==1 {
                let chunks=world.resource::<ChunkStore>();
                let mismatches:Vec<_>=chunks.loaded.iter().filter(|(p,_)|(-2..=1).contains(&p.x)&&(-3..=3).contains(&p.y)&&(-3..=2).contains(&p.z)).filter_map(|(p,s)|{
                    let origin=p.origin().ok()?;
                    let count=s.data.blocks().iter().enumerate().filter(|(i,m)|**m!=block([origin[0]+(*i%16) as i64,origin[1]+(*i/16%16) as i64,origin[2]+(*i/256) as i64])).count();
                    (count>0).then_some((*p,count))
                }).collect();
                info!(?mismatches,"Cave fixture source integrity");
                for pixel in [[800.0,540.0],[960.0,540.0]] {
                    let local=Vec3::new((pixel[0]/1920.0*2.0-1.0)*(65.0_f32.to_radians()*0.5).tan()*1920.0/1080.0,(1.0-pixel[1]/1080.0*2.0)*(65.0_f32.to_radians()*0.5).tan(),-1.0).normalize();
                    let ray=(camera.rotation*local).to_array().map(f64::from);
                    let hit=terra_voxel::blocks::raycast_blocks(camera.translation.to_array().map(f64::from),ray,128.0,|p|chunks.block_at(p));
                    info!(?pixel,?hit,"Cave fixture center-ray geometry probe");
                    if let Some(hit)=hit {
                        let owner=terra_voxel::global_to_section(hit.position).0;
                        let root=world.resource::<terra_render::RenderedSections>().sections.get(&owner).map(|r|(r.entity,r.mesh_epoch,r.meshes.len()));
                        let visibility=root.and_then(|(entity,_,_)|world.get::<Visibility>(entity)).copied();
                        info!(?owner,?root,?visibility,epoch=world.resource::<terra_world::edits::EditJournal>().mesh_epoch(owner),"Cave fixture render-root integrity");
                    }
                }
            }
            let folder=PathBuf::from(std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_|"/tmp/terra-caves".into()));
            if let Err(error)=std::fs::create_dir_all(&folder) {error!(%error);world.write_message(AppExit::error());state.stage=8;return;}
            let mut frames=state.frames.clone(); frames.sort_by(f64::total_cmp);
            let mean=frames.iter().sum::<f64>()/frames.len() as f64;
            let lights=world.query::<&PointLight>().iter(world).count();
            let volumes=world.query::<&bevy::light::FogVolume>().iter(world).count();
            let ssao=world.query::<&bevy::pbr::ScreenSpaceAmbientOcclusion>().iter(world).count();
            if quality==CaveQuality::Ultra && volumes==0 {
                error!("Ultra cave fixture did not activate its volumetric path");world.write_message(AppExit::error());state.stage=8;return;
            }
            let exposure=world.query_filtered::<&bevy::camera::Exposure,With<terra_player::TerraCamera>>().iter(world).next().map_or(0.0,|e|e.ev100);
            if matches!(state.stage,14..=17|20|21) {
                let blend=world.resource::<terra_render::CaveState>().blend;
                let intensities:Vec<_>=world.query::<(&PointLight,&Transform)>().iter(world)
                    .filter(|(_,t)|t.translation.distance_squared(Vec3::new(-7.5,-26.02,-39.5))<1.0)
                    .map(|(l,_)|l.intensity).collect();
                let lamp_gain=2.0_f32.powf(exposure-6.0);
                let Some(definition)=state.torch_definition else {
                    error!("Cave fixture could not load the actual torch light definition");
                    state.stage=stages;world.write_message(AppExit::error());return;
                };
                let normalized=definition.intensity
                    * world.resource::<terra_render::TorchLightingStatus>().flux(definition)
                    * (1.0+(definition.cave_intensity-1.0)*blend);
                let expected=(normalized*(1.0-definition.flicker)-0.01)..=(normalized*(1.0+definition.flicker)+0.01);
                if blend<0.99 || (exposure-6.451_563).abs()>0.01 || intensities.len()!=1 || !expected.contains(&(intensities[0]/lamp_gain)) {
                    error!(stage=state.stage,blend,exposure,?intensities,?expected,"Deep-cave torch inherited outdoor exposure gain");
                    state.stage=stages;world.write_message(AppExit::error());return;
                }
                info!(stage=state.stage,?intensities,"Deep-cave torch uses final camera exposure with live world clock");
                let world_layers=bevy::camera::visibility::RenderLayers::default();
                let solar:Vec<_>=world.query::<(&DirectionalLight,Option<&bevy::camera::visibility::RenderLayers>)>().iter(world)
                    .filter(|(_,layers)|layers.is_none_or(|layers|layers.intersects(&world_layers)))
                    .map(|(light,_)|(light.illuminance,light.shadow_maps_enabled)).collect();
                if solar.len()!=2 || solar.iter().any(|(lux,shadows)|*lux>0.01 && !shadows) {
                    error!(stage=state.stage,?solar,"Cave roof must cast shadows without extinguishing the outdoor sun");
                    state.stage=stages;world.write_message(AppExit::error());return;
                }
                if matches!(state.stage,16|17|21) && world.resource::<terra_render::UnderwaterState>().blend<0.99 {
                    error!(stage=state.stage,"Cave-diving fixture did not activate actual underwater optics");
                    state.stage=stages;world.write_message(AppExit::error());return;
                }
            }
            if state.stage==10 && !world.resource::<terra_render::TorchSources>().0.iter().any(|p|p.distance_squared(Vec3::new(0.5,19.98,0.5))<0.1) {
                error!("Loaded torch emitter disappeared beyond 50m");world.write_message(AppExit::error());state.stage=stages;return;
            }
            if state.stage==11 && !world.query::<&Name>().iter(world).any(|n|n.as_str()=="Carried torch light") {
                error!("Replicated held torch did not create a light");world.write_message(AppExit::error());state.stage=stages;return;
            }
            if state.stage==12 && world.query::<&PointLight>().iter(world).filter(|l|(l.range-4.0).abs()<0.01 && !l.shadow_maps_enabled).count()!=1 {
                error!("Workbench must have exactly one four-meter shadowless lantern");world.write_message(AppExit::error());state.stage=stages;return;
            }
            info!(stage=state.stage,?quality,fps=1000.0/mean,p95_ms=frames[frames.len()*95/100],p99_ms=frames[frames.len()*99/100],lights,volumes,ssao,exposure,underwater=world.resource::<terra_render::UnderwaterState>().blend,blend=world.resource::<terra_render::CaveState>().blend,"Cave showcase measured frame sample");
            let stage=state.stage;
            let probes=if stage==8 || stage==13 {ground_probes(world)}else{Vec::new()};
            let portal=portal_probe(world,stage);
            world.spawn((Screenshot::primary_window(), GroundProbes(probes), CaveExposureProbe(stage),portal))
                .observe(save_to_disk(folder.join(format!("cave-{stage}-{quality:?}.png"))))
                .observe(measure_ground)
                .observe(measure_cave_exposure)
                .observe(measure_portal)
                .observe(|_:On<ScreenshotCaptured>,mut state:ResMut<Showcase>|state.captured+=1);
            state.requested=true;
        } else if state.captured>state.stage {
            state.stage+=1; state.requested=false; state.entered=Instant::now(); state.frames.clear();
            if state.stage==stages {
                if lod_shadow_only && std::env::var("TERRA_CAVE_LOD_SHADOW_REFERENCE").is_ok_and(|value|value=="1") {
                    info!(stages,"Cave ceiling LOD reference captured; not a passing light-isolation validation");
                } else {
                    info!(stages,"Cave showcase passed: quality levels, floor/wall torches, optional night/retention/carried-light views and live-clock exposure regression");
                }
                world.write_message(AppExit::Success);
            }
        }
    });
}

#[derive(Component)]
struct GroundProbes(Vec<(f32, Vec2)>);

#[derive(Component)]
struct CaveExposureProbe(usize);

#[derive(Component)]
struct PortalProbe {
    stage: usize,
    ground: Option<Vec2>,
    sky: Option<Vec2>,
}

fn portal_probe(world: &mut World, stage: usize) -> PortalProbe {
    let mut probe = PortalProbe {
        stage,
        ground: None,
        sky: None,
    };
    if !matches!(stage, 18 | 19 | 22..=24) {
        return probe;
    }
    let scale = world
        .query::<&Window>()
        .iter(world)
        .next()
        .map_or(1.0, Window::scale_factor);
    let mut cameras =
        world.query_filtered::<(&Camera, &GlobalTransform), With<terra_player::TerraCamera>>();
    if let Some((camera, transform)) = cameras.iter(world).next() {
        probe.ground = camera
            .world_to_viewport(
                transform,
                if stage >= 22 {
                    LOD_SHADOW_GROUND_TARGET
                } else {
                    PORTAL_GROUND_TARGET
                },
            )
            .ok()
            .map(|p| p * scale);
        if stage < 22 {
            probe.sky = camera
                .world_to_viewport(transform, PORTAL_SKY_TARGET)
                .ok()
                .map(|p| p * scale);
        }
    }
    probe
}

fn measure_portal(
    event: On<ScreenshotCaptured>,
    probes: Query<&PortalProbe>,
    cave: Res<terra_render::CaveState>,
    mut state: ResMut<Showcase>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(probe) = probes.get(event.entity) else {
        return;
    };
    if !matches!(probe.stage, 18 | 19 | 22..=24) {
        return;
    }
    let sample = |pixel: Option<Vec2>| {
        let pixel = pixel?;
        let mut total = 0.0;
        let mut count = 0;
        for dy in -3..=3 {
            for dx in -3..=3 {
                let x = pixel.x.round() as i32 + dx;
                let y = pixel.y.round() as i32 + dy;
                if x < 0 || y < 0 {
                    continue;
                }
                if let Ok(color) = event.image.get_color_at(x as u32, y as u32) {
                    let c = color.to_linear();
                    total += c.red * 0.2126 + c.green * 0.7152 + c.blue * 0.0722;
                    count += 1;
                }
            }
        }
        (count > 0).then_some(total / count as f32)
    };
    if probe.stage >= 22 {
        let ground = sample(probe.ground);
        if probe.stage == 22 {
            state.lod_shadow_luminance = ground;
        }
        let ceiling_shadow = ground.is_some_and(|current| current <= 0.015);
        let reference =
            std::env::var("TERRA_CAVE_LOD_SHADOW_REFERENCE").is_ok_and(|value| value == "1");
        info!(
            stage = probe.stage,
            blend = cave.blend,
            ?ground,
            baseline = state.lod_shadow_luminance,
            ceiling_shadow,
            reference,
            "Native cave ceiling LOD shadow photometry (same receiver and real caster section)"
        );
        if cave.blend < 0.99 || ground.is_none() || (!ceiling_shadow && !reference) {
            error!(
                "Closed cave ceiling lost its physical directional shadow after a terrain LOD change"
            );
            state.stage = usize::MAX;
            exit.write(AppExit::error());
        }
        return;
    }
    let measurements = sample(probe.ground).zip(sample(probe.sky));
    let enclosed = probe.stage == 18;
    let position_matches = if enclosed {
        cave.blend > 0.99
    } else {
        (cave.blend - 0.125).abs() < 0.01
    };
    let visible = measurements.is_some_and(|(ground, sky)| ground > 0.01 && sky > 0.03);
    let comparison = if enclosed {
        true
    } else {
        state.portal_luminance.zip(measurements).is_some_and(
            |((inside_ground, inside_sky), (outside_ground, outside_sky))| {
                inside_ground >= outside_ground * 0.40 && inside_sky >= outside_sky * 0.40
            },
        )
    };
    info!(
        stage = probe.stage,
        blend = cave.blend,
        ?measurements,
        ?position_matches,
        ?comparison,
        "Native cave-entrance daylight photometry (same world-space outdoor targets)"
    );
    if !position_matches || !visible || !comparison {
        error!("Cave entrance lost its sunlit outdoor terrain or visible daytime sky");
        state.stage = usize::MAX;
        exit.write(AppExit::error());
    } else if enclosed {
        state.portal_luminance = measurements;
    }
}

fn measure_cave_exposure(
    event: On<ScreenshotCaptured>,
    probes: Query<&CaveExposureProbe>,
    mut state: ResMut<Showcase>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(probe) = probes.get(event.entity) else {
        return;
    };
    if probe.0 < 14 || matches!(probe.0, 18 | 19 | 22..=24) {
        return;
    }
    let size = event.image.size();
    let mut luminances = Vec::new();
    for y in (size.y / 10..size.y * 9 / 10).step_by(4) {
        for x in (size.x / 10..size.x * 9 / 10).step_by(4) {
            if let Ok(color) = event.image.get_color_at(x, y) {
                let color = color.to_linear();
                luminances.push(color.red * 0.2126 + color.green * 0.7152 + color.blue * 0.0722);
            }
        }
    }
    if luminances.is_empty() {
        error!("Deep-cave screenshot has no readable pixels");
        state.stage = usize::MAX;
        exit.write(AppExit::error());
        return;
    }
    luminances.sort_by(f32::total_cmp);
    let mean = luminances.iter().sum::<f32>() / luminances.len() as f32;
    let p95 = luminances[luminances.len() * 95 / 100];
    let washed_out_fraction =
        luminances.iter().filter(|v| **v > 0.70).count() as f32 / luminances.len() as f32;
    let black_fraction =
        luminances.iter().filter(|v| **v < 0.00001).count() as f32 / luminances.len() as f32;
    info!(
        stage = probe.0,
        mean,
        p95,
        washed_out_fraction,
        black_fraction,
        "Native enclosed-cave torch photometry (tonemapped capture)"
    );
    if mean < 0.002 || washed_out_fraction > 0.06 || black_fraction > 0.75 {
        error!("Enclosed cave is blank or its stone walls are overexposed");
        state.stage = usize::MAX;
        exit.write(AppExit::error());
    }
}

fn ground_probes(world: &mut World) -> Vec<(f32, Vec2)> {
    let scale = world
        .query::<&Window>()
        .iter(world)
        .next()
        .map_or(1.0, Window::scale_factor);
    let mut cameras =
        world.query_filtered::<(&Camera, &GlobalTransform), With<terra_player::TerraCamera>>();
    let Some((camera, transform)) = cameras.iter(world).next() else {
        return Vec::new();
    };
    [1.0, 3.0, 6.0, 10.0, 16.0]
        .into_iter()
        .filter_map(|distance| {
            camera
                .world_to_viewport(transform, Vec3::new(0.5 + distance, 19.005, 0.5))
                .ok()
                .map(|pixel| (distance, pixel * scale))
        })
        .collect()
}

fn measure_ground(event: On<ScreenshotCaptured>, probes: Query<&GroundProbes>) {
    let Ok(probes) = probes.get(event.entity) else {
        return;
    };
    for &(distance, pixel) in &probes.0 {
        let mut sum = 0.0;
        let mut count = 0;
        for dy in -3..=3 {
            for dx in -3..=3 {
                let x = pixel.x.round() as i32 + dx;
                let y = pixel.y.round() as i32 + dy;
                if x < 0 || y < 0 {
                    continue;
                }
                if let Ok(color) = event.image.get_color_at(x as u32, y as u32) {
                    let color = color.to_linear();
                    sum += color.red * 0.2126 + color.green * 0.7152 + color.blue * 0.0722;
                    count += 1;
                }
            }
        }
        if count > 0 {
            info!(
                distance,
                pixel_x = pixel.x,
                pixel_y = pixel.y,
                linear_luminance = sum / count as f32,
                samples = count,
                "Native torch ground photometry (tonemapped capture)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_contains_openings_emitting_blocks_water_and_dark_tunnels() {
        assert_eq!(block([19, 18, 6]), 0);
        assert_eq!(block([-12, -30, 8]), 40);
        assert_eq!(block([4, -29, -15]), 3);
        assert_eq!(block([-19, -26, 0]), 34);
        assert_eq!(block([-4, -33, 0]), 14);
        assert_eq!(block([0, -25, -40]), 0);
        assert_eq!(block([3, -25, -40]), 3);
        assert_eq!(block([0, -25, -44]), 3);
        assert_eq!(block([-8, -27, -40]), 40);
        assert_eq!(block([-8, -28, -40]), 3);
        assert_eq!(block([-8, -20, -40]), 3);
        assert_eq!(block([-8, -25, -40]), 0);
    }

    #[test]
    fn daylight_portal_fixture_has_real_roof_opening_and_shared_sunlit_ground() {
        let inside = eye(18).translation.as_dvec3().to_array();
        let outside = eye(19).translation.as_dvec3().to_array();
        assert_eq!(
            terra_world::enclosure::sample(inside, 18.0, |p| Some(block(p))).cave,
            1.0
        );
        assert_eq!(
            terra_world::enclosure::sample(outside, 18.0, |p| Some(block(p))).cave,
            0.125
        );
        let daylight =
            terra_world::enclosure::sample([-3.0, -17.0, 35.0], 18.0, |p| Some(block(p)));
        assert_eq!(daylight.cave, 0.0);
        assert_eq!(block([-19, -11, 35]), 3);
        assert_eq!(block([3, -21, 35]), 3);
        assert_eq!(block([3, 18, 35]), 0);
        assert_eq!(block([3, 63, 35]), 0);
        assert_eq!(
            block(PORTAL_GROUND_TARGET.floor().as_i64vec3().to_array()),
            0,
            "The projected target must be above the floor, never inside it"
        );
        let target = PORTAL_GROUND_TARGET.as_dvec3();
        for position in [inside, outside] {
            let direction = (target - bevy::math::DVec3::from_array(position))
                .normalize()
                .to_array();
            let hit =
                terra_voxel::blocks::raycast_blocks(position, direction, 64.0, |p| Some(block(p)))
                    .unwrap();
            assert_eq!(hit.position, [3, -21, 35]);
            let sky_direction = (PORTAL_SKY_TARGET.as_dvec3()
                - bevy::math::DVec3::from_array(position))
            .normalize()
            .to_array();
            assert!(
                terra_voxel::blocks::raycast_blocks(position, sky_direction, 64.0, |p| {
                    Some(block(p))
                })
                .is_none(),
                "The sky sample must pass through the real opening"
            );
        }
    }

    #[test]
    fn lod_ceiling_fixture_uses_a_real_distant_underside_without_an_exterior_top_face() {
        use terra_core::Config;
        let receiver = LOD_SHADOW_GROUND_TARGET.as_dvec3();
        let rotation = (8.0_f64 / 24.0 - 0.25) * std::f64::consts::TAU;
        // 08:00 terrain lighting uses its documented 45-degree minimum;
        // runtime acceptance traces the real light transform independently.
        let horizontal =
            bevy::math::DVec2::new(rotation.cos(), rotation.sin() * 0.4358899).normalize();
        let height = std::f64::consts::FRAC_1_SQRT_2;
        let direction =
            bevy::math::DVec3::new(horizontal.x * height, height, horizontal.y * height);
        let hit = terra_voxel::blocks::raycast_blocks(
            receiver.to_array(),
            direction.to_array(),
            128.0,
            |p| Some(lod_shadow_block(p)),
        )
        .unwrap();
        assert_eq!(hit.position[1], 0);
        assert_eq!(
            terra_voxel::global_to_section(hit.position).0,
            SectionPos::new(0, 0, 1)
        );
        let camera = eye(22).translation.as_dvec3().to_array();
        let mut config = terra_core::ClientConfig {
            lod_hysteresis: 0.0,
            ..default()
        };
        for (near, far, expected) in [
            (96.0, 160.0, terra_render::meshing::DetailLevel::Near),
            (16.0, 96.0, terra_render::meshing::DetailLevel::Middle),
            (16.0, 20.0, terra_render::meshing::DetailLevel::Far),
        ] {
            config.lod_near_distance = near;
            config.lod_far_distance = far;
            assert!(config.validate().is_ok());
            // Public render output is asserted natively; here the geometric
            // distance is clearly outside the configured near/far intervals.
            let dx = (0.0 - camera[0]).max(0.0);
            let dz = (16.0 - camera[2]).max(0.0);
            let distance = (dx * dx + dz * dz).sqrt();
            let actual = if distance < f64::from(near) * 1.5 {
                terra_render::meshing::DetailLevel::Near
            } else if distance < f64::from(far) * 1.5 {
                terra_render::meshing::DetailLevel::Middle
            } else {
                terra_render::meshing::DetailLevel::Far
            };
            assert_eq!(actual, expected);
        }
        for y in 0..=64 {
            assert_eq!(lod_shadow_block([hit.position[0], y, hit.position[2]]), 3);
        }
    }
}
