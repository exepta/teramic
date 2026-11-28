//! Explicit model/animation inspection, separate from authoritative network tests.
mod dragon;
mod forest;
mod golem;
mod poultry;
mod sandworm;
mod savanna;
mod spirit;
mod swamp;
mod volcanic;
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::creatures::*;
use terra_network::client::{CreatureTrack, ReplicatedCreatures};

#[derive(Resource)]
struct Inspection {
    start: Instant,
    requested: usize,
    captured: usize,
    last: Instant,
    ready_frames: usize,
    failed: bool,
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-creatures") {
        return;
    }
    app.init_resource::<ReplicatedCreatures>()
        .insert_resource(Inspection {
            start: Instant::now(),
            requested: 0,
            captured: 0,
            last: Instant::now(),
            ready_frames: 0,
            failed: false,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .before(terra_player::PlayerSet::Targeting),
        );
}
fn setup(
    mut creatures: ResMut<ReplicatedCreatures>,
    mut journal: ResMut<terra_world::edits::EditJournal>,
) {
    if std::env::var_os("TERRA_EYE_INSPECT").is_some()
        || std::env::var_os("TERRA_SPIDER_MODEL_INSPECT").is_some()
    {
        return;
    }
    if std::env::var_os("TERRA_SWAMP_RESIDENTS_INSPECT").is_some() {
        swamp::setup(&mut creatures, &mut journal);
        return;
    }
    if std::env::var_os("TERRA_SAVANNA_INSPECT").is_some() {
        savanna::setup(&mut creatures, &mut journal);
        return;
    }
    if std::env::var_os("TERRA_POULTRY_INSPECT").is_some() {
        poultry::setup(&mut creatures, &mut journal);
        return;
    }
    if std::env::var_os("TERRA_VOLCANIC_INSPECT").is_some() {
        volcanic::setup(&mut creatures, &mut journal);
        return;
    }
    if std::env::var_os("TERRA_DRAGON_INSPECT").is_some() {
        dragon::setup(&mut creatures);
        return;
    }
    if std::env::var_os("TERRA_SANDWORM_INSPECT").is_some() {
        sandworm::setup(&mut creatures, &mut journal);
        return;
    }
    if std::env::var_os("TERRA_GOLEM_INSPECT").is_some() {
        golem::setup(&mut creatures, &mut journal);
        return;
    }
    if std::env::var_os("TERRA_FOREST_INSPECT").is_some() {
        forest::setup(&mut creatures);
        return;
    }
    if std::env::var_os("TERRA_SPIRIT_INSPECT").is_some() {
        spirit::setup(&mut creatures);
        return;
    }
    if std::env::var_os("TERRA_SMOKE_VILLAGERS").is_some() {
        setup_villagers(&mut creatures, &mut journal);
        return;
    }
    if std::env::var_os("TERRA_HORSE_INSPECT").is_some() {
        for variant in 0..16 {
            let home = [i64::from(variant), 0];
            let Some(id) = creature_id(home, 0) else {
                continue;
            };
            let animal = Creature {
                id,
                kind: CreatureKind::Horse,
                home,
                position: [
                    4.0 + f64::from(variant % 4) * 3.8,
                    32.0,
                    14.0 + f64::from(variant / 4) * 3.8,
                ],
                velocity: [0.0; 3],
                yaw: 0.42,
                health: 20.0,
                behavior: CreatureBehavior::Rest,
                timer: 1.0,
                age: 0.0,
                variant,
                needs: CreatureNeeds::full(CreatureKind::Horse),
            };
            creatures.creatures.insert(
                id,
                CreatureTrack {
                    interval: 0.1,
                    previous: animal.clone(),
                    current: animal,
                    received: Instant::now(),
                    playback: Default::default(),
                },
            );
        }
        return;
    }
    let aquatic = std::env::var_os("TERRA_AQUATIC_INSPECT").is_some();
    for (i, kind) in CreatureKind::ALL
        .into_iter()
        .filter(|kind| {
            if aquatic {
                kind.is_aquatic() && *kind != CreatureKind::Fish
            } else {
                matches!(
                    kind,
                    CreatureKind::Sheep
                        | CreatureKind::Cow
                        | CreatureKind::Pig
                        | CreatureKind::Fish
                        | CreatureKind::Bee
                        | CreatureKind::Bear
                )
            }
        })
        .enumerate()
    {
        let home = [i as i64, 0];
        let Some(id) = (if kind == CreatureKind::Bee {
            bee_id(home, 0)
        } else {
            creature_id(home, 0)
        }) else {
            continue;
        };
        let position = [
            if kind == CreatureKind::Bear {
                19.4
            } else {
                3.5 + i as f64 * 2.6
            },
            32.0 + if aquatic {
                0.4
            } else if kind == CreatureKind::Fish {
                0.7
            } else {
                0.0
            },
            24.0,
        ];
        let animal = Creature {
            id,
            home,
            kind,
            position,
            velocity: [0.9, 0.0, 0.0],
            yaw: std::f32::consts::PI,
            health: kind.health(),
            behavior: if kind == CreatureKind::Bee {
                CreatureBehavior::Fly
            } else if kind.is_aquatic() {
                CreatureBehavior::Swim
            } else {
                CreatureBehavior::Wander
            },
            timer: 3.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        };
        creatures.creatures.insert(
            id,
            CreatureTrack {
                interval: 0.1,
                previous: animal.clone(),
                current: animal,
                received: Instant::now(),
                playback: Default::default(),
            },
        );
    }
    if let Some(mut river) = creatures
        .creatures
        .values()
        .find(|a| a.current.kind == CreatureKind::Fish)
        .cloned()
        && let Some(id) = creature_id([CreatureKind::ALL.len() as i64, 0], 0)
    {
        river.current.id = id;
        river.current.home = [CreatureKind::ALL.len() as i64, 0];
        river.current.variant = 2;
        river.current.position = [16.5, 32.7, 24.0];
        river.previous = river.current.clone();
        creatures.creatures.insert(id, river);
    }
}
fn run(world: &mut World) {
    if std::env::var_os("TERRA_EYE_INSPECT").is_some()
        || std::env::var_os("TERRA_SPIDER_MODEL_INSPECT").is_some()
    {
        return;
    }
    if std::env::var_os("TERRA_SWAMP_RESIDENTS_INSPECT").is_some() {
        swamp::run(world);
        return;
    }
    if std::env::var_os("TERRA_SAVANNA_INSPECT").is_some() {
        savanna::run(world);
        return;
    }
    if std::env::var_os("TERRA_POULTRY_INSPECT").is_some() {
        poultry::run(world);
        return;
    }
    if std::env::var_os("TERRA_VOLCANIC_INSPECT").is_some() {
        volcanic::run(world);
        return;
    }
    if std::env::var_os("TERRA_DRAGON_INSPECT").is_some() {
        dragon::run(world);
        return;
    }
    if std::env::var_os("TERRA_SANDWORM_INSPECT").is_some() {
        sandworm::run(world);
        return;
    }
    if std::env::var_os("TERRA_GOLEM_INSPECT").is_some() {
        golem::run(world);
        return;
    }
    if std::env::var_os("TERRA_FOREST_INSPECT").is_some() {
        forest::run(world);
        return;
    }
    if std::env::var_os("TERRA_SPIRIT_INSPECT").is_some() {
        spirit::run(world);
        return;
    }
    if std::env::var_os("TERRA_SMOKE_VILLAGERS").is_some() {
        villagers_run(world);
        return;
    }
    if std::env::var_os("TERRA_HORSE_INSPECT").is_some() {
        horse_run(world);
        return;
    }
    let aquatic = std::env::var_os("TERRA_AQUATIC_INSPECT").is_some();
    let life = std::env::var_os("TERRA_LIFE_INSPECT").is_some();
    let inspect = std::env::var_os("TERRA_CREATURE_INSPECT").is_some();
    let bee = std::env::var_os("TERRA_BEE_INSPECT").is_some();
    let bear = std::env::var_os("TERRA_BEAR_INSPECT").is_some();
    let stage = world.resource::<Inspection>().captured;
    if aquatic {
        for track in world
            .resource_mut::<ReplicatedCreatures>()
            .creatures
            .values_mut()
        {
            track.current.behavior = match stage {
                9 if track.current.kind == CreatureKind::Squid => CreatureBehavior::Hold,
                10 if track.current.kind == CreatureKind::Squid => CreatureBehavior::Ink,
                11 if track.current.kind.is_jelly() => CreatureBehavior::Rest,
                12 if track.current.kind.is_shark() => CreatureBehavior::Attack,
                _ => CreatureBehavior::Swim,
            };
            track.current.velocity = if matches!(
                track.current.behavior,
                CreatureBehavior::Rest | CreatureBehavior::Hold
            ) {
                [0.0; 3]
            } else {
                [0.0, 0.05, 0.65]
            };
            track.previous = track.current.clone();
        }
    }
    if !aquatic && (bear || stage >= 8) && !life && !inspect && !bee {
        let pose = if bear { stage } else { stage - 8 };
        for track in world
            .resource_mut::<ReplicatedCreatures>()
            .creatures
            .values_mut()
        {
            if track.current.kind != CreatureKind::Bear {
                continue;
            }
            track.current.behavior = match pose {
                0 => CreatureBehavior::Rest,
                1 => CreatureBehavior::Hunt,
                2 => CreatureBehavior::Attack,
                3 => CreatureBehavior::Sleep,
                _ => CreatureBehavior::Climb,
            };
            track.current.velocity = match pose {
                1 => [1.5, 0.0, 0.0],
                4 => [0.0, 0.6, 0.0],
                _ => [0.0; 3],
            };
            track.current.timer = if pose == 2 { 0.04 } else { 3.0 };
            track.current.position[1] = if pose == 4 { 32.6 } else { 32.0 };
            track.previous = track.current.clone();
        }
    }
    if life {
        let stage = world.resource::<Inspection>().captured;
        for track in world
            .resource_mut::<ReplicatedCreatures>()
            .creatures
            .values_mut()
        {
            track.current.behavior = if track.current.kind == CreatureKind::Bee {
                CreatureBehavior::Nectar
            } else if stage > 0 && track.current.kind != CreatureKind::Fish {
                CreatureBehavior::Sleep
            } else {
                track.current.behavior
            };
            track.current.velocity = [0.0; 3];
        }
    }
    if !aquatic && !life && !inspect && !bee && !bear && stage == 7 {
        for track in world
            .resource_mut::<ReplicatedCreatures>()
            .creatures
            .values_mut()
        {
            if track.current.kind == CreatureKind::Bee {
                track.current.behavior = CreatureBehavior::Nectar;
            } else if track.current.kind != CreatureKind::Fish {
                track.current.behavior = CreatureBehavior::Sleep;
            }
            track.current.velocity = [0.0; 3];
        }
    }
    let eye = if aquatic {
        aquatic_camera(world, stage)
    } else if bear || (!life && !inspect && !bee && stage >= 8) {
        detail_camera(world, 8)
    } else if life && world.resource::<Inspection>().captured == 2 {
        Transform::from_xyz(17.2, 33.2, 25.3).looking_at(Vec3::new(16.5, 32.87, 24.0), Vec3::Y)
    } else if (life && world.resource::<Inspection>().captured == 0) || bee {
        Transform::from_xyz(14.25, 32.20, 24.45).looking_at(Vec3::new(13.9, 32.06, 24.0), Vec3::Y)
    } else if inspect {
        world
            .resource_mut::<terra_core::DebugSettings>()
            .creature_inspection = true;
        Transform::from_xyz(6.3, 33.5, 27.5).looking_at(Vec3::new(6.1, 33.0, 24.0), Vec3::Y)
    } else if (1..=6).contains(&stage) {
        detail_camera(world, stage)
    } else {
        Transform::from_xyz(11.0, 36.5, 36.5).looking_at(Vec3::new(10.5, 32.8, 24.0), Vec3::Y)
    };
    if inspect || life {
        for mut body in world
            .query_filtered::<&mut Transform, With<terra_player::PlayerBody>>()
            .iter_mut(world)
        {
            body.translation =
                eye.translation - Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
        }
    }
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = eye;
    }
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 = [8.0, 33.0, 24.0];
    let status = world.resource::<terra_render::RenderStatus>();
    let ready = status.ready
        && world
            .resource::<terra_world::streaming::WorldGeneration>()
            .status
            == terra_world::streaming::GenerationStatus::Ready;
    {
        let mut state = world.resource_mut::<Inspection>();
        state.ready_frames = if ready { state.ready_frames + 1 } else { 0 };
    }
    let state = world.resource::<Inspection>();
    if state.failed {
        world.write_message(AppExit::error());
        return;
    }
    if state.start.elapsed() > Duration::from_secs(90) {
        error!("Creature model inspection timeout");
        world.write_message(AppExit::error());
        return;
    }
    if state.captured
        == if aquatic {
            13
        } else if life {
            3
        } else if inspect || bee {
            2
        } else if bear {
            5
        } else {
            13
        }
    {
        if aquatic {
            info!(
                "Aquatic creature model inspection passed: squid, five named jellies, three sharks, shared meshes, swim/hold/ink/rest/bite poses; model-only fixture, not an AI/swimming integration test"
            );
        } else {
            info!(
                "Creature model inspection passed: six species, two fish variants, shared meshes, closeups and bear rest/hunt/attack/sleep/climb poses; model-only fixture"
            );
        }
        world.write_message(AppExit::Success);
        return;
    }
    if !ready
        || state.ready_frames < 120
        || state.start.elapsed() < Duration::from_secs(8)
        || state.requested != state.captured
        || state.last.elapsed() < Duration::from_secs(2)
    {
        return;
    }
    if inspect && !world.resource::<terra_ui::DebugOverlay>().animal_visible {
        return;
    }
    if inspect
        && !world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0 == terra_core::creatures::definitions::get(CreatureKind::Cow).title)
    {
        return;
    }
    let count = world
        .query::<&terra_render::creatures::CreatureAvatar>()
        .iter(world)
        .count();
    if count != if aquatic { 9 } else { 7 } {
        return;
    }
    let folder = PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/terra-creatures".into()),
    );
    if let Err(error) = std::fs::create_dir_all(&folder) {
        error!(%error);
        world.write_message(AppExit::error());
        return;
    }
    let stage = world.resource::<Inspection>().requested;
    world.resource_mut::<Inspection>().requested += 1;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("creatures-{stage}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                let nonblank = event.image.data.as_ref().is_some_and(|data| {
                    let samples = data.chunks_exact(4).step_by(16);
                    let count = samples.len();
                    samples.filter(|p| p[..3].iter().any(|v| *v > 16)).count() > count / 100
                });
                if nonblank {
                    state.captured += 1;
                    state.last = Instant::now();
                } else {
                    state.failed = true;
                    error!("Creature screenshot is blank; refusing a visual-test pass");
                }
            },
        );
}

// Representative civilian professions, each inspected with both immutable identity sexes.
const VILLAGER_GALLERY_ROLES: [u8; 10] = [
    villagers::LUMBERJACK,
    villagers::MINER,
    1, // Farmer.
    villagers::CRAFTSMAN,
    villagers::ENGINEER,
    villagers::BUTCHER,
    villagers::SELLER,
    5, // Bishop.
    6, // Archer artisan; carries a stick outside work poses.
    villagers::UNEMPLOYED,
];
const VILLAGER_GALLERY_COUNT: usize = VILLAGER_GALLERY_ROLES.len() * 2;

fn villager_gallery_position(slot: usize, sleeping: bool) -> [f64; 3] {
    [
        3.5 + (slot % 10) as f64 * 2.0,
        if sleeping { 32.79 } else { 32.0 },
        (if sleeping { 14.1 } else { 23.5 }) + (slot / 10) as f64 * 4.0,
    ]
}

fn setup_villagers(
    creatures: &mut ReplicatedCreatures,
    journal: &mut terra_world::edits::EditJournal,
) {
    use terra_world::edits::BlockObjectState;
    for (slot, (role, sex)) in VILLAGER_GALLERY_ROLES
        .into_iter()
        .flat_map(|role| [CreatureSex::Male, CreatureSex::Female].map(|sex| (role, sex)))
        .enumerate()
    {
        // Home is only the fixture slot here; select a real stable identity for each sex.
        let home = [slot as i64, 0];
        let Some(id) = (0..villagers::MAX_RESIDENTS)
            .filter_map(|resident| villagers::id(home, resident))
            .find(|id| CreatureSex::from_spawn_id(*id) == sex)
        else {
            error!(role, ?sex, "No identity for villager inspection fixture");
            continue;
        };
        let animal = Creature {
            id,
            kind: CreatureKind::Villager,
            home,
            position: villager_gallery_position(slot, false),
            velocity: [0.0; 3],
            yaw: std::f32::consts::PI,
            health: CreatureKind::Villager.health(),
            behavior: CreatureBehavior::Rest,
            timer: 1.0,
            age: 0.0,
            variant: role,
            needs: CreatureNeeds::full(CreatureKind::Villager),
        };
        creatures.creatures.insert(
            id,
            CreatureTrack {
                interval: 0.1,
                previous: animal.clone(),
                current: animal,
                received: Instant::now(),
                playback: Default::default(),
            },
        );
        let sleeping_position = villager_gallery_position(slot, true);
        let foot = sleeping_position.map(|coordinate| coordinate.floor() as i64);
        let head = [foot[0], foot[1], foot[2] + 1];
        for (position, peer, secondary) in [(foot, head, false), (head, foot, true)] {
            journal.record(position, 71).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        facing: 0,
                        peer: Some(peer),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
    }
}

fn villagers_run(world: &mut World) {
    let stage = world.resource::<Inspection>().captured;
    let stride_time = world.resource::<Time>().elapsed_secs_f64() * 1.15;
    let state = world.resource::<Inspection>();
    if state.failed || state.start.elapsed() > Duration::from_secs(90) {
        error!("Villager gallery failed or timed out");
        world.write_message(AppExit::error());
        return;
    }
    if stage >= 5 {
        // Offscreen item/portrait cameras also own screenshot readback channels.
        if state.last.elapsed() < Duration::from_millis(250)
            || world
                .resource::<terra_render::RenderBlockPreviews>()
                .pending
                != 0
            || !world.resource::<terra_render::PlayerPreviewImage>().ready
        {
            return;
        }
        info!(
            "Villager gallery passed: ten representative professions with male and female models (20 residents), two visible everyday tools, eight action tools, idle, closeup, walking, bed-aligned sleep without held tools and action poses; rendering fixture, not an authoritative AI test"
        );
        world.write_message(AppExit::Success);
        return;
    }
    for track in world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .values_mut()
    {
        let slot = track.current.home[0] as usize;
        track.current.behavior = match stage {
            2 => CreatureBehavior::Wander,
            3 => CreatureBehavior::Sleep,
            4 => CreatureBehavior::Attack,
            _ => CreatureBehavior::Rest,
        };
        track.current.velocity = if stage == 2 {
            [0.0, 0.0, (stride_time.cos() * 0.65 * 1.15) as f32]
        } else {
            [0.0; 3]
        };
        track.current.position = villager_gallery_position(slot, stage == 3);
        if stage == 2 {
            track.current.position[2] += stride_time.sin() * 0.65;
        }
        track.current.yaw = if stage == 3 {
            0.0
        } else {
            if stage == 2 && stride_time.cos() < 0.0 {
                0.0
            } else {
                std::f32::consts::PI
            }
        };
        track.previous = track.current.clone();
    }
    let eye = match stage {
        // Close-up of the male/female lumberjacks and miners, including their working tools.
        1 | 4 => {
            Transform::from_xyz(6.5, 35.3, 31.5).looking_at(Vec3::new(6.5, 33.0, 23.5), Vec3::Y)
        }
        3 => Transform::from_xyz(12.5, 43.0, 34.5).looking_at(Vec3::new(12.5, 32.7, 16.1), Vec3::Y),
        _ => Transform::from_xyz(12.5, 40.5, 46.0).looking_at(Vec3::new(12.5, 33.0, 25.5), Vec3::Y),
    };
    for mut body in world
        .query_filtered::<&mut Transform, With<terra_player::PlayerBody>>()
        .iter_mut(world)
    {
        body.translation = eye.translation - Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    }
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = eye;
    }
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 = [12.5, 33.0, 21.0];
    let ready = world.resource::<terra_render::RenderStatus>().ready
        && world
            .resource::<terra_world::streaming::WorldGeneration>()
            .status
            == terra_world::streaming::GenerationStatus::Ready;
    let count = world
        .query::<&terra_render::creatures::CreatureAvatar>()
        .iter(world)
        .count();
    let held: Vec<_> = world
        .query::<(&terra_render::creatures::VillagerHeldItem, &ChildOf)>()
        .iter(world)
        .map(|(held, parent)| (held.arm, parent.parent()))
        .collect();
    // Work-only tools are stowed in idle/walking poses; sleep hides every hand item.
    let expected_held = VILLAGER_GALLERY_ROLES
        .into_iter()
        .filter(|role| match stage {
            3 => false,
            4 => matches!(*role, 1..=4 | 6 | 7 | 9 | 14),
            _ => matches!(*role, 2..=4 | 6 | 7),
        })
        .count()
        * 2;
    let held_ready = held.len() == expected_held
        && held.iter().all(|(arm, parent)| arm == parent)
        && held
            .iter()
            .map(|(arm, _)| arm)
            .collect::<std::collections::HashSet<_>>()
            .len()
            == expected_held;
    let state = world.resource::<Inspection>();
    if !ready
        || count != VILLAGER_GALLERY_COUNT
        || !held_ready
        || state.start.elapsed() < Duration::from_secs(8)
        || state.requested != state.captured
        || state.last.elapsed() < Duration::from_secs(2)
    {
        return;
    }
    let folder = PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/terra-villagers".into()),
    );
    if let Err(error) = std::fs::create_dir_all(&folder) {
        error!(%error, "Cannot create villager capture directory");
        world.write_message(AppExit::error());
        return;
    }
    world.resource_mut::<Inspection>().requested += 1;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("villagers-{stage}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                let nonblank = event.image.data.as_ref().is_some_and(|data| {
                    data.chunks_exact(4)
                        .step_by(16)
                        .filter(|pixel| pixel[..3].iter().any(|value| *value > 20))
                        .count()
                        > 1000
                });
                if nonblank {
                    state.captured += 1;
                    state.last = Instant::now();
                } else {
                    state.failed = true;
                }
            },
        );
}

fn horse_run(world: &mut World) {
    let stage = world.resource::<Inspection>().captured;
    if stage >= 4 {
        info!(
            "Horse inspection passed: sixteen coats, front overview, face, side walking and side sleeping poses; shared real creature meshes"
        );
        world.write_message(AppExit::Success);
        return;
    }
    if world.resource::<Inspection>().start.elapsed() > Duration::from_secs(120) {
        error!("Horse gallery timed out");
        world.write_message(AppExit::error());
        return;
    }
    let eye = match stage {
        0 => Transform::from_xyz(24.0, 44.0, 1.0).looking_at(Vec3::new(9.7, 32.8, 19.7), Vec3::Y),
        1 => {
            Transform::from_xyz(10.0, 33.65, 10.5).looking_at(Vec3::new(11.6, 34.1, 14.0), Vec3::Y)
        }
        2 => {
            Transform::from_xyz(15.1, 33.55, 12.4).looking_at(Vec3::new(11.6, 34.1, 14.0), Vec3::Y)
        }
        _ => {
            Transform::from_xyz(15.1, 33.55, 12.4).looking_at(Vec3::new(11.6, 33.65, 14.0), Vec3::Y)
        }
    };
    for mut body in world
        .query_filtered::<&mut Transform, With<terra_player::PlayerBody>>()
        .iter_mut(world)
    {
        body.translation = eye.translation - Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    }
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = eye;
    }
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 = [10.0, 33.0, 20.0];
    for track in world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .values_mut()
    {
        track.current.behavior = match stage {
            2 => CreatureBehavior::Wander,
            3 => CreatureBehavior::Sleep,
            _ => CreatureBehavior::Rest,
        };
        track.current.velocity = if stage == 2 {
            [0.0, 0.0, 1.6]
        } else {
            [0.0; 3]
        };
        track.previous = track.current.clone();
    }
    let ready = world.resource::<terra_render::RenderStatus>().ready
        && world
            .resource::<terra_world::streaming::WorldGeneration>()
            .status
            == terra_world::streaming::GenerationStatus::Ready;
    let count = world
        .query::<&terra_render::creatures::CreatureAvatar>()
        .iter(world)
        .count();
    if !ready || count != 16 {
        return;
    }
    let state = world.resource::<Inspection>();
    if state.failed {
        world.write_message(AppExit::error());
        return;
    }
    if state.start.elapsed() < Duration::from_secs(8)
        || state.requested != state.captured
        || state.last.elapsed() < Duration::from_secs(3)
    {
        return;
    }
    let folder = PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/terra-horses".into()),
    );
    if std::fs::create_dir_all(&folder).is_err() {
        world.write_message(AppExit::error());
        return;
    }
    world.resource_mut::<Inspection>().requested += 1;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("horses-{stage}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                let good = event.image.data.as_ref().is_some_and(|data| {
                    data.chunks_exact(4)
                        .step_by(16)
                        .filter(|p| p[..3].iter().any(|v| *v > 20))
                        .count()
                        > 1000
                });
                if good {
                    state.captured += 1;
                    state.last = Instant::now();
                } else {
                    state.failed = true;
                }
            },
        );
}

fn detail_camera(world: &World, stage: usize) -> Transform {
    let (kind, variant) = match stage {
        1 => (CreatureKind::Sheep, 0),
        2 => (CreatureKind::Cow, 0),
        3 => (CreatureKind::Pig, 0),
        4 => (CreatureKind::Fish, 0),
        5 => (CreatureKind::Bee, 0),
        8 => (CreatureKind::Bear, 0),
        _ => (CreatureKind::Fish, 2),
    };
    let position = world
        .resource::<ReplicatedCreatures>()
        .creatures
        .values()
        .find(|track| track.current.kind == kind && track.current.variant == variant)
        .map(|track| Vec3::from_array(track.current.position.map(|v| v as f32)))
        .unwrap_or(Vec3::new(8.0, 32.0, 24.0));
    let (eye, target) = match kind {
        CreatureKind::Bee => (Vec3::new(0.17, 0.16, 0.26), Vec3::new(0.0, 0.055, 0.0)),
        CreatureKind::Fish => (Vec3::new(0.65, 0.48, 0.72), Vec3::new(0.0, 0.17, 0.02)),
        CreatureKind::Cow => (Vec3::new(1.35, 1.65, 2.30), Vec3::new(0.0, 0.83, 0.0)),
        CreatureKind::Sheep => (Vec3::new(1.2, 1.35, 1.95), Vec3::new(0.0, 0.69, 0.0)),
        CreatureKind::Pig => (Vec3::new(1.0, 1.1, 1.65), Vec3::new(0.0, 0.47, 0.0)),
        CreatureKind::Bear => (Vec3::new(1.95, 1.65, 2.5), Vec3::new(0.0, 0.67, 0.0)),
        _ => (Vec3::new(1.5, 1.5, 2.5), Vec3::new(0.0, 0.5, 0.0)),
    };
    Transform::from_translation(position + eye).looking_at(position + target, Vec3::Y)
}

fn aquatic_camera(world: &World, stage: usize) -> Transform {
    let kinds = [
        CreatureKind::Squid,
        CreatureKind::MoonJelly,
        CreatureKind::FireJelly,
        CreatureKind::VoidJelly,
        CreatureKind::SunJelly,
        CreatureKind::CrystalJelly,
        CreatureKind::ReefShark,
        CreatureKind::HammerheadShark,
        CreatureKind::WhaleShark,
    ];
    let kind = match stage {
        9 | 10 => CreatureKind::Squid,
        11 => CreatureKind::CrystalJelly,
        12 => CreatureKind::ReefShark,
        _ => kinds[stage.min(kinds.len() - 1)],
    };
    let creature = world
        .resource::<ReplicatedCreatures>()
        .creatures
        .values()
        .find(|track| track.current.kind == kind);
    let (position, size) = creature.map_or((Vec3::new(8.0, 32.4, 24.0), Vec3::ONE), |track| {
        (
            Vec3::from_array(track.current.position.map(|v| v as f32)),
            Vec3::from_array(track.current.size().map(|v| v as f32)),
        )
    });
    let distance = size.max_element().max(0.7);
    let target = position + Vec3::Y * size.y * 0.48;
    Transform::from_translation(
        target + Vec3::new(distance * 0.75, distance * 0.43, distance * 1.25),
    )
    .looking_at(target, Vec3::Y)
}
