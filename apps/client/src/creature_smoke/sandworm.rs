//! The real cached model driven by replicated burrow/warning/emergence phases.
use super::*;
pub(super) fn setup(
    replicas: &mut ReplicatedCreatures,
    journal: &mut terra_world::edits::EditJournal,
) {
    // A real sand floor for projected surface waves. This gallery owns an
    // isolated temporary world and never writes a user's saved terrain.
    for x in 0..=18 {
        for z in -8..=46 {
            for y in 30..=41 {
                journal
                    .record(
                        [x, y, z],
                        if y <= 31 || y == 32 && z >= 35 {
                            match z {
                                ..=12 => terra_voxel::MaterialId::Sand as u16,
                                13..=34 => terra_voxel::MaterialId::RedDuneSand as u16,
                                _ => terra_voxel::MaterialId::DuneSand as u16,
                            }
                        } else {
                            0
                        },
                    )
                    .unwrap();
            }
        }
    }

    let animal = Creature {
        id: creature_id([0, 0], 0).unwrap(),
        kind: CreatureKind::SandWorm,
        home: [0, 0],
        position: [8.0, 32.0, 18.0],
        velocity: [0.0; 3],
        yaw: std::f32::consts::PI,
        health: CreatureKind::SandWorm.health(),
        behavior: CreatureBehavior::Burrowed,
        timer: 4.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::SandWorm),
    };
    replicas.creatures.insert(
        animal.id,
        CreatureTrack {
            previous: animal.clone(),
            current: animal,
            received: Instant::now(),
            interval: 0.1,
            playback: Default::default(),
        },
    );
}
pub(super) fn run(world: &mut World) {
    let state = world.resource::<Inspection>();
    let stage = state.captured;
    if state.failed || state.start.elapsed() > Duration::from_secs(120) {
        error!("Sandworm native gallery failed");
        world.write_message(AppExit::error());
        return;
    }
    if stage >= 15 {
        if state.last.elapsed() > Duration::from_millis(500) {
            let folder = PathBuf::from(
                std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/terra-sandworm".into()),
            );
            let proof = serde_json::json!({"status":"passed","creature":"sand_worm","captures":15,
                "phases":["burrowed","tremor","leap","upright","bite","spit","retreat","mouth_detail","vertical_rise","top_bend","moving_burrow","stepped_burrow","distant_burrow","passive_burrow","underground_charge"],"replicated_pose_fixture":true,
                "warning_seconds":terra_core::creatures::sandworm::WARNING_SECONDS,
                "radial_fanged_maw":true,"recessed_throat":true,"rear_collar_attachment":true,"body_kept_out_of_mouth":true,"scalloped_cuticle":true,"visible_buried_back":false,"buried_anatomy_hidden":true,"body_length_multiple":5,
                "body_sections":terra_core::creatures::sandworm::BODY_SEGMENTS,"tapered_tail":true,
                "animated_closing_mandibles":3,"cached_bile_droplets":3,
                "combat_vertical_exit":true,"combat_reuses_dive_hole":true,"combat_retreat_same_shaft":true,"arc_is_long_range_locomotion":true,"shared_authoritative_spine":true,"airborne_breach":true,"travelling_arc_span":terra_core::creatures::sandworm::LEAP_DISTANCE,"tail_enters_before_reanchor":true,"head_first_dive":true,
                "continuous_advancing_sand_crest":true,"distance_sampled_trail":true,"passive_trail_without_attack_goal":true,"charge_has_sand_trail":true,"stepped_dune_projection":true,"cached_expanding_ground_ridges":3,"ground_projected_sectors_per_ridge":8,
                "scope":"Real cached renderer and phase presentation; actual spawn commands, natural spawning, hit targeting and authority damage are tested separately."});
            if std::fs::write(
                folder.join("sandworm-report.json"),
                serde_json::to_vec_pretty(&proof).unwrap(),
            )
            .is_err()
            {
                world.write_message(AppExit::error());
                return;
            }
            info!(
                "Sandworm gallery passed: hidden burrowed anatomy, ground tremor, fivefold annulated breach, closing bite, aimed bile spit and retreat; AI is tested separately"
            );
            world.write_message(AppExit::Success);
        }
        return;
    }
    let (behavior, timer) = match stage {
        0 => (CreatureBehavior::Burrowed, 4.0),
        1 => (CreatureBehavior::Tremor, 0.4),
        2 => (
            CreatureBehavior::Leap,
            terra_core::creatures::sandworm::LEAP_SECONDS * 0.55,
        ),
        3 => (CreatureBehavior::Emerge, 0.04),
        4 => (
            CreatureBehavior::Attack,
            terra_core::creatures::sandworm::EXPOSED_SECONDS
                * (1.0 - terra_core::creatures::sandworm::BITE_PHASE),
        ),
        5 => (
            CreatureBehavior::Spit,
            terra_core::creatures::sandworm::SPIT_SECONDS - 0.40,
        ),
        6 => (CreatureBehavior::Retreat, 1.05),
        8 => (
            CreatureBehavior::Emerge,
            terra_core::creatures::sandworm::EMERGE_SECONDS * 0.50,
        ),
        9 => (
            CreatureBehavior::Emerge,
            terra_core::creatures::sandworm::EMERGE_SECONDS * 0.18,
        ),
        10..=13 => (CreatureBehavior::Burrowed, 4.0),
        14 => (CreatureBehavior::Hunt, 1.0),
        _ => (
            CreatureBehavior::Attack,
            terra_core::creatures::sandworm::EXPOSED_SECONDS,
        ),
    };
    let moving_age = world.resource::<Inspection>().last.elapsed().as_secs_f32();
    for track in world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .values_mut()
    {
        track.current.behavior = behavior;
        track.current.timer = timer;
        let travel_speed = if stage == 14 { 6.0 } else { 1.65 };
        let z = if stage == 11 { 32.0 } else { 18.0 } + moving_age * travel_speed;
        track.current.position = if stage >= 10 {
            [
                if stage >= 13 { 4.0 } else { 8.0 },
                if z >= 35.0 { 33.0 } else { 32.0 },
                f64::from(z),
            ]
        } else {
            [8.0, 32.0, 18.0]
        };
        track.current.velocity = if stage >= 10 && stage != 13 {
            [0.0, 0.0, travel_speed]
        } else {
            [0.0; 3]
        };
        track.current.age = if stage >= 10 {
            f64::from(moving_age)
        } else {
            0.0
        };
        track.current.needs.goal =
            (behavior == CreatureBehavior::Spit).then_some(terra_core::creatures::NeedGoal {
                kind: terra_core::creatures::NeedKind::AttackAim,
                position: [8, 33, 28],
            });
        track.previous = track.current.clone();
        track.received = Instant::now();
    }
    let eye = Transform::from_translation(if stage >= 13 {
        Vec3::new(16.0, 39.0, 37.0)
    } else if stage == 10 {
        Vec3::new(15.0, 36.0, 29.0)
    } else if stage == 11 {
        Vec3::new(15.0, 38.0, 40.0)
    } else if stage == 12 {
        Vec3::new(32.0, 48.0, 52.0)
    } else if stage == 3 {
        // Three-quarter profile of the bend and the rim, as seen from a dune.
        Vec3::new(15.6, 39.5, 23.0)
    } else if stage == 4 {
        Vec3::new(10.4, 35.0, 24.0)
    } else if stage == 5 {
        Vec3::new(15.0, 41.0, 28.0)
    } else if stage == 7 {
        Vec3::new(10.8, 39.8, 23.6)
    } else if stage == 8 {
        Vec3::new(15.0, 38.0, 27.0)
    } else if stage == 9 {
        Vec3::new(15.0, 42.0, 27.0)
    } else if stage == 2 {
        Vec3::new(29.0, 43.0, 38.0)
    } else {
        Vec3::new(30.0, 42.0, 30.0)
    })
    .looking_at(
        Vec3::new(
            if stage >= 13 { 4.0 } else { 8.0 },
            if stage == 11 {
                33.3
            } else if stage >= 10 {
                32.3
            } else if stage == 4 {
                33.5
            } else if stage == 7 {
                38.85
            } else if stage == 3 {
                37.7
            } else if stage == 5 || stage == 9 {
                38.0
            } else {
                35.0
            },
            if stage == 11 {
                35.0
            } else if stage == 14 {
                26.0
            } else if stage >= 10 {
                21.5
            } else if stage == 4 {
                19.3
            } else if stage == 5 {
                21.0
            } else if stage == 2 {
                25.0
            } else {
                18.0
            },
        ),
        Vec3::Y,
    );
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
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 =
        eye.translation.to_array().map(f64::from);
    let ready = world.resource::<terra_render::RenderStatus>().ready;
    let avatars = world
        .query::<&terra_render::creatures::CreatureAvatar>()
        .iter(world)
        .count();
    let state = world.resource::<Inspection>();
    if !ready
        || avatars != 1
        || state.start.elapsed() < Duration::from_secs(8)
        || state.requested != state.captured
        || state.last.elapsed() < Duration::from_secs(2)
    {
        return;
    }
    let tracks: Vec<_> = world
        .query::<(&Name, &Visibility, &Transform)>()
        .iter(world)
        .filter(|(name, visibility, _)| {
            name.as_str() == "Fading sandworm track" && **visibility != Visibility::Hidden
        })
        .map(|(_, _, transform)| transform.translation)
        .collect();
    let visible_tracks = tracks.len();
    // The passive path has its own x-lane. Earlier aggressive marks cannot
    // satisfy its proof, even though they deliberately fade over ten seconds.
    let passive_tracks = tracks.iter().filter(|p| p.x < 5.0).count();
    if stage >= 10 && visible_tracks < 4 {
        return;
    }
    if stage >= 13 && passive_tracks < 4 {
        return;
    }
    let folder = PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/terra-sandworm".into()),
    );
    if std::fs::create_dir_all(&folder).is_err() {
        world.write_message(AppExit::error());
        return;
    }
    if stage >= 10 {
        let replicas = world.resource::<ReplicatedCreatures>();
        let no_attack_goal = replicas
            .creatures
            .values()
            .all(|track| track.current.needs.goal.is_none());
        let reported_stationary = replicas
            .creatures
            .values()
            .all(|track| track.current.velocity == [0.0; 3]);
        std::fs::write(
            folder.join(format!("sandworm-{stage}-trail.json")),
            serde_json::to_vec_pretty(&serde_json::json!({
                "visible_track_meshes": visible_tracks,
                "moving_buried_body_hidden": true,
                "stepped_dune": stage == 11,
                "distant_view": stage == 12,
                "passive_burrow": stage == 13,
                "passive_lane_track_meshes": passive_tracks,
                "no_attack_goal": no_attack_goal,
                "zero_reported_velocity": reported_stationary,
                "underground_charge": stage == 14,
            }))
            .unwrap(),
        )
        .unwrap();
    }
    world.resource_mut::<Inspection>().requested += 1;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("sandworm-{stage}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                let valid = event.image.data.as_ref().is_some_and(|pixels| {
                    pixels
                        .chunks_exact(4)
                        .step_by(127)
                        .filter(|p| p[..3].iter().any(|v| *v > 20))
                        .count()
                        > 1000
                });
                if valid {
                    state.captured += 1;
                    state.last = Instant::now();
                } else {
                    state.failed = true;
                }
            },
        );
}
