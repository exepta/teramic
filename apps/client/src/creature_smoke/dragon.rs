//! Native gallery of the shared gameplay meshes and replicated boss poses.
use super::*;

pub(super) fn setup(animals: &mut ReplicatedCreatures) {
    let kind = CreatureKind::DarkDragon;
    let animal = Creature {
        id: terra_core::creatures::breeding::next_birth_id(&Default::default(), 0).unwrap(),
        kind,
        home: [0, 0],
        position: [8., 32., 16.],
        velocity: [0.; 3],
        yaw: 0.,
        health: kind.health(),
        behavior: CreatureBehavior::Rest,
        timer: 1.,
        age: terra_core::creatures::breeding::GROWTH_SECONDS,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    };
    animals.creatures.insert(
        animal.id,
        CreatureTrack {
            current: animal.clone(),
            previous: animal,
            interval: 0.1,
            received: Instant::now(),
            playback: Default::default(),
        },
    );
}

pub(super) fn run(world: &mut World) {
    use CreatureBehavior as B;
    use CreatureKind as K;
    let poses = [
        (K::DarkDragon, 0, B::Rest, "dark-rest"),
        (K::DarkDragon, 1, B::Emerge, "elder-roar"),
        (K::DarkDragon, 2, B::Ink, "shadow-rune"),
        (K::EarthDragon, 0, B::Rest, "earth-rest"),
        (K::EarthDragon, 0, B::Leap, "earth-takeoff"),
        (K::EarthDragon, 0, B::Tremor, "earth-impact"),
        (K::WaterDragon, 0, B::Fly, "water-flight"),
        (K::WaterDragon, 0, B::Fly, "water-glide"),
        (K::WaterDragon, 0, B::Fly, "water-bank"),
        (K::WaterDragon, 0, B::Land, "water-landing"),
        (K::EarthDragon, 0, B::Wander, "earth-stride"),
        (K::WaterDragon, 1, B::Spit, "ice-breath"),
        (K::DarkDragon, 0, B::Spit, "shadow-breath"),
        (K::EarthDragon, 0, B::Spit, "earth-breath"),
        (K::WaterDragon, 2, B::Swim, "deep-swim"),
        (K::EarthDragon, 0, B::Sleep, "earth-sleep"),
        (K::DarkDragon, 0, B::Dead, "dark-fall"),
        (K::DarkDragon, 3, B::Ink, "ruin-fissure"),
        (K::DarkDragon, 4, B::DragonSurge, "wyrm-phase"),
        (K::DarkDragon, 5, B::DragonVolley, "void-orbs"),
        (K::WaterDragon, 3, B::Tremor, "tidal-wave"),
        (K::WaterDragon, 4, B::DragonWard, "deepwater-barrier"),
        (K::WaterDragon, 5, B::DragonSurge, "icewater-geyser"),
        (K::EarthDragon, 1, B::DragonVolley, "mountain-boulders"),
        (K::EarthDragon, 2, B::DragonWard, "granite-shield"),
        (K::EarthDragon, 3, B::Ink, "magma-fissure"),
        (K::WaterDragon, 0, B::Rest, "water-baby"),
        (K::WaterDragon, 0, B::Graze, "water-young"),
        (K::WaterDragon, 0, B::Fly, "water-adolescent"),
        (K::WaterDragon, 0, B::Wander, "water-young-adult"),
        (K::WaterDragon, 0, B::Emerge, "water-grown"),
    ];
    let state = world.resource::<Inspection>();
    if state.failed || state.start.elapsed() > Duration::from_secs(150) {
        error!("Dragon gallery failed or timed out");
        world.write_message(AppExit::error());
        return;
    }
    let index = state.captured;
    if index >= poses.len() {
        if state.last.elapsed() > Duration::from_millis(500) {
            info!("Dragon gallery passed: all sixteen adult designs and five growth stages");
            world.write_message(AppExit::Success);
        }
        return;
    }
    let (kind, variant, behavior, name) = poses[index];
    let flying = behavior == B::Fly;
    let position = Vec3::new(8., 32. + if flying { 7. } else { 0. }, 16.);
    let approach = if behavior == B::Wander {
        -3.4 * state.last.elapsed().as_secs_f32().min(1.2)
    } else {
        0.
    };
    let position = position + Vec3::Z * approach;
    let life = match name {
        "water-baby" => Some(0.15),
        "water-young" => Some(2.),
        "water-adolescent" => Some(4.5),
        "water-young-adult" => Some(9.),
        "water-grown" => Some(12.),
        _ => None,
    }
    .map(|months| terra_core::creatures::dragon_life::DragonLife {
        months,
        ..Default::default()
    });
    let scale = life.map_or(1., |life| life.scale() as f32);
    let mut creatures = world.resource_mut::<ReplicatedCreatures>();
    creatures.boss = if terra_core::creatures::dragons::casting(behavior) || behavior == B::Emerge {
        creatures.creatures.keys().next().copied()
    } else {
        None
    };
    for track in creatures.creatures.values_mut() {
        let a = &mut track.current;
        a.position = position.to_array().map(f64::from);
        a.kind = kind;
        a.variant = variant;
        a.age = terra_core::creatures::breeding::GROWTH_SECONDS + 0.22;
        a.health = if behavior == B::Dead {
            0.
        } else {
            kind.health()
        };
        a.behavior = behavior;
        a.timer = terra_core::creatures::dragons::duration(behavior) * 0.4;
        a.velocity = match name {
            "water-flight" => [0., 6., -8.5],
            "water-glide" => [0., -4., -12.],
            "water-bank" => [6.5, 1., -8.],
            "water-landing" => [0., -3.7, -2.8],
            "earth-takeoff" => [0., 5.8, -3.],
            "earth-stride" => [0., 0., -4.6],
            _ => [0.; 3],
        };
        a.needs = CreatureNeeds::full(kind);
        a.needs.dragon = life;
        if matches!(
            behavior,
            B::Spit | B::Ink | B::DragonVolley | B::DragonSurge
        ) {
            a.needs.goal = Some(NeedGoal {
                kind: NeedKind::AttackAim,
                position: [8, 32, -4],
            });
        }
        track.previous = a.clone();
        track.received = Instant::now();
    }
    let wide = matches!(behavior, B::Spit | B::Fly | B::Ink)
        || terra_core::creatures::dragons::lineage(kind, variant).is_some();
    let center = position + scale * Vec3::new(0., 5.2, if wide { -1.6 } else { 1.0 });
    let eye = Transform::from_translation(
        center
            + scale.sqrt()
                * if wide {
                    Vec3::new(29., 8., -23.)
                } else {
                    Vec3::new(23., 6.3, -14.)
                },
    )
    .looking_at(center, Vec3::Y);
    for mut t in world
        .query_filtered::<&mut Transform, With<terra_player::PlayerBody>>()
        .iter_mut(world)
    {
        t.translation = eye.translation - Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    }
    for mut t in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *t = eye;
    }
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 =
        eye.translation.to_array().map(f64::from);
    if !world.resource::<terra_render::RenderStatus>().ready
        || world
            .query::<&terra_render::creatures::CreatureAvatar>()
            .iter(world)
            .count()
            != 1
    {
        return;
    }
    let state = world.resource::<Inspection>();
    if state.start.elapsed() < Duration::from_secs(6)
        || state.requested != state.captured
        || state.last.elapsed() < Duration::from_secs(1)
    {
        return;
    }
    let dir = PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/terra-dragons".into()),
    );
    world.resource_mut::<Inspection>().requested += 1;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(dir.join(format!("dragon-{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                let valid = event.image.data.as_ref().is_some_and(|pixels| {
                    pixels
                        .chunks_exact(4)
                        .step_by(127)
                        .filter(|p| p[0] > 18 || p[1] > 18 || p[2] > 18)
                        .count()
                        > 300
                });
                state.failed |= !valid;
                state.captured += 1;
                state.last = Instant::now();
            },
        );
}
