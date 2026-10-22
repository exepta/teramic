mod model;
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_core::creatures::{CreatureBehavior as Behavior, CreatureKind as Kind};
use terra_network::client::ReplicatedCreatures;

#[derive(Component)]
pub struct CreatureAvatar(pub u64);
#[derive(Component)]
struct Motion {
    kind: Kind,
    behavior: Behavior,
    phase: f32,
    clock: f32,
    speed: f32,
    sleep: f32,
    attack: f32,
}
#[derive(Component)]
struct Limb(model::Part, Vec3);
#[derive(Resource, Default)]
struct Models {
    parts: BTreeMap<(Kind, u8), ModelParts>,
    material: Handle<StandardMaterial>,
    bell_material: Handle<StandardMaterial>,
}
type ModelParts = Vec<(model::Part, Vec3, Handle<Mesh>)>;
pub(crate) fn install(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(
        Update,
        (render, animate)
            .chain()
            .after(terra_player::PlayerSet::Movement),
    );
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut models = Models {
        material: materials.add(StandardMaterial {
            perceptual_roughness: 0.85,
            ..default()
        }),
        bell_material: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.68),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.48,
            reflectance: 0.12,
            cull_mode: None,
            ..default()
        }),
        ..default()
    };
    for kind in Kind::ALL {
        for variant in 0..if kind == Kind::Horse {
            16
        } else if kind == Kind::Fish {
            2
        } else {
            1
        } {
            match if variant == 0 {
                model::parts(kind)
            } else {
                model::parts_variant(
                    kind,
                    if kind == Kind::Horse {
                        variant
                    } else {
                        variant * 2
                    },
                )
            } {
                Ok(parts) => {
                    models.parts.insert(
                        (kind, variant),
                        parts
                            .into_iter()
                            .map(|(part, p, mesh)| (part, p, meshes.add(mesh)))
                            .collect(),
                    );
                }
                Err(error) => tracing::error!(%error, ?kind, "Creature model failed"),
            }
        }
    }
    commands.insert_resource(models);
}
fn render(
    mut commands: Commands,
    time: Res<Time>,
    animals: Option<Res<ReplicatedCreatures>>,
    models: Res<Models>,
    mut roots: Query<(Entity, &CreatureAvatar, &mut Transform, &mut Motion)>,
) {
    let mut present = std::collections::HashSet::new();
    for (entity, avatar, mut transform, mut motion) in &mut roots {
        let Some(track) = animals
            .as_ref()
            .and_then(|a| a.creatures.get(&avatar.0))
            .filter(|t| t.current.behavior != Behavior::InNest)
        else {
            commands.entity(entity).despawn();
            continue;
        };
        present.insert(avatar.0);
        let sleeping = track.current.behavior == Behavior::Sleep;
        motion.sleep += ((if sleeping { 1.0 } else { 0.0 }) - motion.sleep)
            * (time.delta_secs() * 7.0).min(1.0);
        transform.scale = instance_scale(&track.current);
        transform.translation =
            track.position() + Vec3::Y * sleep_offset(track.current.kind) * motion.sleep;
        transform.rotation = transform.rotation.slerp(
            rotation(&track.current),
            1.0 - (-time.delta_secs() * 10.0).exp(),
        );
        motion.behavior = track.current.behavior;
        let speed = Vec3::from_array(track.current.velocity).length();
        motion.speed += (speed - motion.speed) * (1.0 - (-time.delta_secs() * 10.0).exp());
        let strike = if track.current.kind == Kind::Bear
            && matches!(track.current.behavior, Behavior::Attack | Behavior::Hunt)
            && speed < 0.45
        {
            (1.0 - track.current.timer / 0.5).clamp(0.0, 1.0)
        } else {
            0.0
        };
        motion.attack += (strike - motion.attack) * (1.0 - (-time.delta_secs() * 16.0).exp());
        motion.phase += time.delta_secs() * (motion.speed * 5.0 + 0.5);
        motion.clock += time.delta_secs();
    }
    let Some(animals) = animals else {
        return;
    };
    for (&id, track) in &animals.creatures {
        if present.contains(&id) || track.current.behavior == Behavior::InNest {
            continue;
        }
        let Some(parts) = models.parts.get(&(
            track.current.kind,
            if track.current.kind == Kind::Horse {
                track.current.variant
            } else {
                u8::from(track.current.kind == Kind::Fish && track.current.variant >= 2)
            },
        )) else {
            continue;
        };
        let root = commands
            .spawn((
                CreatureAvatar(id),
                Motion {
                    kind: track.current.kind,
                    behavior: track.current.behavior,
                    phase: (id % 31) as f32,
                    clock: (id % 29) as f32 * 0.43,
                    speed: 0.0,
                    attack: 0.0,
                    sleep: if track.current.behavior == Behavior::Sleep {
                        1.0
                    } else {
                        0.0
                    },
                },
                Name::new(format!("{:?} {id}", track.current.kind)),
                Transform::from_translation(
                    track.position()
                        + Vec3::Y
                            * if track.current.behavior == Behavior::Sleep {
                                sleep_offset(track.current.kind)
                            } else {
                                0.0
                            },
                )
                .with_rotation(rotation(&track.current))
                .with_scale(instance_scale(&track.current)),
                Visibility::default(),
            ))
            .id();
        for (part, offset, mesh) in parts {
            let mut child = commands.spawn((
                Limb(*part, *offset),
                crate::miniature::creature_layers(),
                terra_core::miniature::MiniatureMesh::Creature,
                if matches!(part, model::Part::Eyelids | model::Part::Proboscis) {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(if matches!(part, model::Part::Bell) {
                    models.bell_material.clone()
                } else {
                    models.material.clone()
                }),
                Transform::from_translation(*offset),
                ChildOf(root),
            ));
            if track.current.kind.is_jelly() {
                child.insert(bevy::light::NotShadowCaster);
            }
        }
    }
}
fn model_scale(kind: Kind) -> Vec3 {
    scale_for_size(kind, kind.size())
}
fn scale_for_size(kind: Kind, size: [f64; 3]) -> Vec3 {
    let authored = match kind {
        Kind::Sheep => [0.85, 1.25, 1.2],
        Kind::Cow => [0.95, 1.65, 1.5],
        Kind::Pig => [0.8, 1.0, 1.15],
        Kind::Fish => [0.3, 0.35, 0.7],
        Kind::Bee => [0.095, 0.095, 0.115],
        Kind::Bear => [1.0, 1.3, 1.8],
        Kind::Horse => [0.595, 1.95, 2.15],
        Kind::Squid => [0.95, 0.95, 2.0],
        Kind::MoonJelly
        | Kind::FireJelly
        | Kind::VoidJelly
        | Kind::SunJelly
        | Kind::CrystalJelly => [0.35, 0.95, 0.35],
        Kind::ReefShark | Kind::HammerheadShark | Kind::WhaleShark => kind.size().map(|v| v as f32),
    };
    Vec3::from_array(std::array::from_fn(|i| size[i] as f32 / authored[i]))
}
fn instance_scale(animal: &terra_core::creatures::Creature) -> Vec3 {
    model_scale(animal.kind) * Vec3::from_array(animal.aquatic_scale())
}
fn sleep_offset(kind: Kind) -> f32 {
    let leg = match kind {
        Kind::Sheep => 0.42,
        Kind::Cow => 0.62,
        Kind::Pig => 0.32,
        Kind::Bear => 0.40,
        Kind::Horse => 0.68,
        _ => 0.0,
    };
    -leg * 0.90 * model_scale(kind).y
}
fn animate(
    roots: Query<&Motion>,
    mut limbs: Query<(&ChildOf, &Limb, &mut Transform, &mut Visibility)>,
) {
    for (parent, part, mut transform, mut visibility) in &mut limbs {
        let Ok(motion) = roots.get(parent.parent()) else {
            continue;
        };
        transform.translation = part.1;
        transform.scale = Vec3::ONE;
        if matches!(part.0, model::Part::Eyelids) {
            *visibility = if motion.sleep > 0.5 || motion.clock.rem_euclid(6.7) < 0.14 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        if matches!(part.0, model::Part::Proboscis) {
            *visibility = if motion.behavior == Behavior::Nectar {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        if matches!(part.0, model::Part::Head | model::Part::Eyelids) {
            transform.translation.y -= motion.sleep * 0.18;
        }
        if matches!(part.0, model::Part::Body) && !matches!(motion.kind, Kind::Fish | Kind::Bee) {
            transform.scale.y = 1.0 + (motion.clock * 2.1).sin() * 0.004;
        }
        if motion.kind == Kind::Bear {
            animate_bear(motion, part, &mut transform);
            continue;
        }
        if motion.kind.is_aquatic() && motion.kind != Kind::Fish {
            animate_aquatic(motion, part, &mut transform);
            continue;
        }
        transform.rotation = match part.0 {
            model::Part::Leg(_)
                if motion.kind == Kind::Horse && motion.behavior == Behavior::Leap =>
            {
                Quat::from_rotation_x(if part.1.z < 0.0 { -0.90 } else { 0.45 })
            }
            model::Part::Leg(sign) => {
                let stride = if motion.behavior == Behavior::Swim {
                    (motion.clock * 4.8).sin() * sign * 0.62
                } else {
                    motion.phase.sin() * sign * (motion.speed * 0.30).min(0.60)
                };
                Quat::from_rotation_x(stride * (1.0 - motion.sleep) + 1.45 * motion.sleep)
            }
            model::Part::Head | model::Part::Eyelids if motion.sleep > 0.01 => {
                Quat::from_euler(EulerRot::XYZ, 0.30 * motion.sleep, 0.0, 0.10 * motion.sleep)
            }
            model::Part::Head | model::Part::Eyelids
                if matches!(motion.behavior, Behavior::Graze | Behavior::Drink) =>
            {
                Quat::from_rotation_x(-0.65 + motion.phase.sin() * 0.08)
            }
            model::Part::Head | model::Part::Eyelids => Quat::from_euler(
                EulerRot::XYZ,
                (motion.clock * 1.4).sin() * 0.012,
                (motion.clock * 0.6).sin() * 0.035,
                (motion.clock * 0.9).sin() * 0.008,
            ),
            model::Part::Tail => Quat::from_rotation_y(motion.phase.sin() * 0.35),
            model::Part::Body if motion.behavior == Behavior::Shake => {
                Quat::from_rotation_z((motion.phase * 10.0).sin() * 0.08)
            }
            model::Part::Body if motion.behavior == Behavior::Nectar => {
                Quat::from_rotation_x(0.25 + (motion.phase * 8.0).sin() * 0.07)
            }
            model::Part::Proboscis => {
                Quat::from_rotation_x(if motion.behavior == Behavior::Nectar {
                    -0.2 + (motion.phase * 12.0).sin() * 0.18
                } else {
                    1.4
                })
            }
            model::Part::Wing(sign)
                if matches!(motion.behavior, Behavior::Fly | Behavior::Flee) =>
            {
                Quat::from_rotation_z(sign * (0.25 + (motion.phase * 35.0).sin() * 0.65))
            }
            model::Part::Wing(sign) => Quat::from_rotation_z(sign * 0.12),
            _ => Quat::IDENTITY,
        };
    }
}

fn animate_aquatic(motion: &Motion, part: &Limb, transform: &mut Transform) {
    let fast = matches!(
        motion.behavior,
        Behavior::Flee | Behavior::Ink | Behavior::Hunt
    );
    let frequency = if fast { 4.0 } else { 1.8 } + motion.speed.min(3.0) * 0.65;
    let phase = motion.clock * frequency;
    if motion.kind.is_jelly() {
        let pulse = (phase.sin() * 0.5 + 0.5).powi(2);
        let settled = if motion.behavior == Behavior::Rest {
            0.25
        } else {
            1.0
        };
        match part.0 {
            model::Part::Bell => {
                transform.scale =
                    Vec3::new(1.0 - pulse * 0.13, 1.0 + pulse * 0.045, 1.0 - pulse * 0.13);
                // Contract around the bell's base, not around the creature's feet.
                transform.translation.y -= 0.62 * (transform.scale.y - 1.0);
            }
            model::Part::Tentacles(offset) => {
                transform.rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    (phase + offset).sin() * 0.065 * settled,
                    0.0,
                    (phase * 0.8 + offset).cos() * 0.070 * settled,
                );
                transform.scale.y = 1.0 + (phase - 0.5).sin() * 0.025;
            }
            model::Part::Body => {
                transform.translation.y += phase.sin() * 0.004;
            }
            _ => {}
        }
        return;
    }
    if motion.kind == Kind::Squid {
        let held = motion.behavior == Behavior::Hold;
        match part.0 {
            model::Part::Tentacles(offset) => {
                let reach = if held { 0.24 } else { 0.0 };
                let amplitude = if held {
                    0.025
                } else if fast {
                    0.20
                } else {
                    0.085
                };
                transform.rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    (phase + offset).sin() * amplitude + reach * offset.cos(),
                    (phase + offset + 0.6).cos() * amplitude + reach * offset.sin(),
                    (phase * 0.7 + offset).sin() * 0.025,
                );
                transform.scale.z = 1.0 + phase.sin() * if fast { 0.05 } else { 0.018 };
            }
            model::Part::Body => {
                let squeeze = phase.sin() * if fast { 0.035 } else { 0.012 };
                transform.scale = Vec3::new(1.0 + squeeze, 1.0 + squeeze, 1.0 - squeeze * 0.6);
            }
            _ => {}
        }
        return;
    }
    transform.rotation = match part.0 {
        model::Part::Tail => Quat::from_rotation_y(phase.sin() * if fast { 0.30 } else { 0.18 }),
        model::Part::Fin(side) => Quat::from_euler(
            EulerRot::XYZ,
            phase.cos() * 0.018,
            phase.sin() * side * 0.022,
            side * (0.04 + (phase * 0.7 + 0.8).sin() * 0.045),
        ),
        model::Part::Body => Quat::from_rotation_z(phase.sin() * 0.012),
        model::Part::Jaw => Quat::from_rotation_x(if motion.behavior == Behavior::Attack {
            -0.18 - phase.sin().abs() * 0.18
        } else {
            -0.025 + phase.sin() * 0.012
        }),
        _ => Quat::IDENTITY,
    };
}

fn animate_bear(motion: &Motion, part: &Limb, transform: &mut Transform) {
    let sleep = motion.sleep;
    let awake = 1.0 - sleep;
    let front = part.1.z < 0.0;
    let stride = if motion.behavior == Behavior::Swim {
        (motion.clock * 4.5).sin() * 0.60
    } else {
        motion.phase.sin() * (motion.speed * 0.22).min(0.44)
    };
    // A raised paw rolls outward on the wind-up, then reaches forward during the strike.
    let attack = motion.attack;
    transform.rotation = match part.0 {
        model::Part::Leg(sign) => {
            let climb = if motion.behavior == Behavior::Climb {
                0.65 + (motion.phase * 1.3 + sign * 1.1).sin() * 0.48
            } else {
                0.0
            };
            let paw = if front { attack * 1.3 } else { 0.0 };
            transform.translation.y += sleep * 0.14;
            Quat::from_euler(
                EulerRot::XYZ,
                (stride * sign + paw + climb) * awake + 1.12 * sleep,
                0.0,
                (if front {
                    attack * part.1.x.signum() * 0.22
                } else {
                    0.0
                }) * awake,
            )
        }
        model::Part::Head | model::Part::Eyelids => {
            let lower = if motion.behavior == Behavior::Drink {
                -0.48
            } else {
                0.0
            };
            Quat::from_euler(
                EulerRot::XYZ,
                (lower - attack * 0.18 + (motion.clock * 1.6).sin() * 0.015) * awake + 0.28 * sleep,
                (motion.clock * 0.65).sin() * 0.028 * awake,
                0.08 * sleep,
            )
        }
        model::Part::Body => Quat::from_euler(
            EulerRot::XYZ,
            attack * 0.065,
            0.0,
            (motion.phase * 0.5).sin() * (motion.speed * 0.012).min(0.035) * awake,
        ),
        model::Part::Tail => Quat::from_rotation_y((motion.clock * 1.3).sin() * 0.08 * awake),
        _ => Quat::IDENTITY,
    };
}

fn rotation(animal: &terra_core::creatures::Creature) -> Quat {
    let yaw = Quat::from_rotation_y(animal.yaw);
    if animal.kind.is_aquatic() && animal.kind != Kind::Fish {
        let forward = Vec2::new(animal.velocity[0], animal.velocity[2]).length();
        let pitch = (animal.velocity[1] / forward.max(0.30))
            .atan()
            .clamp(-0.45, 0.45);
        return yaw * Quat::from_rotation_x(pitch);
    }
    if animal.behavior == Behavior::Sleep {
        return yaw * Quat::from_rotation_z(0.18);
    }
    if animal.behavior == Behavior::Leap {
        return yaw * Quat::from_rotation_x((-animal.velocity[1] * 0.1).clamp(-0.6, 0.6));
    }
    if animal.kind == Kind::Bear && animal.behavior == Behavior::Climb {
        return yaw * Quat::from_rotation_x(1.1);
    }
    if animal.kind == Kind::Bee
        && matches!(animal.behavior, Behavior::Crawl | Behavior::Land)
        && let Some(goal) = animal.needs.goal
    {
        let center = Vec3::from_array(goal.position.map(|v| v as f32 + 0.5));
        let delta = Vec3::from_array(animal.position.map(|v| v as f32)) - center;
        let axis = (0..3)
            .max_by(|&a, &b| delta[a].abs().total_cmp(&delta[b].abs()))
            .unwrap_or(1);
        let mut normal = Vec3::ZERO;
        normal[axis] = delta[axis].signum();
        return Quat::from_rotation_arc(Vec3::Y, normal) * yaw;
    }
    yaw
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_meter_aquatic_meshes_follow_configured_body_and_individual_sizes() {
        assert_eq!(
            scale_for_size(Kind::Squid, [1.9, 1.9, 4.0]),
            Vec3::splat(2.0)
        );
        for kind in Kind::ALL.into_iter().filter(|kind| kind.is_jelly()) {
            assert_eq!(
                scale_for_size(kind, [0.175, 0.475, 0.175]),
                Vec3::splat(0.5)
            );
            let maximum = [0.7, 1.9, 0.7];
            let individual = Vec3::new(0.5, 0.25, 0.75);
            let rendered = scale_for_size(kind, maximum) * individual;
            let expected = Vec3::from_array(maximum.map(|value| value as f32)) * individual
                / Vec3::new(0.35, 0.95, 0.35);
            assert!(rendered.abs_diff_eq(expected, 0.00001));
        }
        assert_eq!(scale_for_size(Kind::Squid, [0.95, 0.95, 2.0]), Vec3::ONE);
        for kind in [Kind::ReefShark, Kind::HammerheadShark, Kind::WhaleShark] {
            assert_eq!(
                model_scale(kind),
                Vec3::ONE,
                "Sharks author their geometry from JSON size"
            );
        }
    }

    #[test]
    fn aquatic_bells_tentacles_fins_and_jaws_have_distinct_finite_animation() {
        let mut motion = Motion {
            kind: Kind::MoonJelly,
            behavior: Behavior::Swim,
            phase: 1.0,
            clock: 0.6,
            speed: 0.7,
            sleep: 0.0,
            attack: 0.0,
        };
        let bell = Limb(model::Part::Bell, Vec3::ZERO);
        let mut pose = Transform::default();
        animate_aquatic(&motion, &bell, &mut pose);
        assert!(pose.scale.x < 0.98 && pose.scale.y > 1.0);
        assert!((pose.translation.y + 0.62 * pose.scale.y - 0.62).abs() < 0.0001);
        let arms = Limb(model::Part::Tentacles(0.0), Vec3::new(0.0, 0.475, -0.02));
        motion.kind = Kind::Squid;
        let mut swim = Transform::from_translation(arms.1);
        animate_aquatic(&motion, &arms, &mut swim);
        motion.behavior = Behavior::Hold;
        let mut held = Transform::from_translation(arms.1);
        animate_aquatic(&motion, &arms, &mut held);
        assert!(held.rotation.angle_between(swim.rotation) > 0.1);
        motion.kind = Kind::ReefShark;
        motion.behavior = Behavior::Swim;
        let jaw = Limb(model::Part::Jaw, Vec3::ZERO);
        let mut closed = Transform::default();
        animate_aquatic(&motion, &jaw, &mut closed);
        motion.behavior = Behavior::Attack;
        let mut bite = Transform::default();
        animate_aquatic(&motion, &jaw, &mut bite);
        assert!(bite.rotation.angle_between(closed.rotation) > 0.1);
        for kind in Kind::ALL.into_iter().filter(|kind| kind.is_aquatic()) {
            motion.kind = kind;
            for part in [
                model::Part::Body,
                model::Part::Tail,
                model::Part::Fin(-1.0),
                model::Part::Bell,
                model::Part::Tentacles(2.1),
            ] {
                let mut transform = Transform::default();
                animate_aquatic(&motion, &Limb(part, Vec3::ZERO), &mut transform);
                assert!(
                    transform.translation.is_finite()
                        && transform.scale.is_finite()
                        && transform.rotation.is_finite()
                );
            }
        }
    }

    #[test]
    fn bear_attack_sleep_and_climb_have_distinct_finite_limb_poses() {
        let mut motion = Motion {
            kind: Kind::Bear,
            behavior: Behavior::Rest,
            phase: 0.8,
            clock: 0.35,
            speed: 0.0,
            sleep: 0.0,
            attack: 0.0,
        };
        let limb = Limb(model::Part::Leg(1.0), Vec3::new(-0.32, 0.57, -0.43));
        let mut rest = Transform::from_translation(limb.1);
        animate_bear(&motion, &limb, &mut rest);
        motion.behavior = Behavior::Attack;
        motion.attack = 1.0;
        let mut attack = rest;
        animate_bear(&motion, &limb, &mut attack);
        assert!(attack.rotation.angle_between(rest.rotation) > 0.8);
        assert!(
            (attack.rotation * -Vec3::Y).z < -0.7,
            "Front paw must reach forward, not behind the bear"
        );
        motion.behavior = Behavior::Climb;
        motion.attack = 0.0;
        let mut climb = rest;
        animate_bear(&motion, &limb, &mut climb);
        assert!(climb.rotation.angle_between(rest.rotation) > 0.5);
        assert!(climb.rotation.angle_between(attack.rotation) > 0.1);
        motion.behavior = Behavior::Sleep;
        motion.sleep = 1.0;
        let mut sleep = rest;
        animate_bear(&motion, &limb, &mut sleep);
        assert!(sleep.rotation.angle_between(rest.rotation) > 1.0);
        assert!(sleep.translation.y > rest.translation.y);
        for pose in [rest, attack, climb, sleep] {
            assert!(pose.rotation.is_finite() && pose.translation.is_finite());
        }
    }

    #[test]
    fn bee_only_extends_its_proboscis_while_feeding() {
        let mut app = App::new();
        app.add_systems(Update, animate);
        let root = app
            .world_mut()
            .spawn(Motion {
                kind: Kind::Bee,
                behavior: Behavior::Fly,
                phase: 0.0,
                clock: 0.0,
                speed: 1.0,
                sleep: 0.0,
                attack: 0.0,
            })
            .id();
        let tongue = app
            .world_mut()
            .spawn((
                Limb(model::Part::Proboscis, Vec3::ZERO),
                Transform::default(),
                Visibility::Inherited,
                ChildOf(root),
            ))
            .id();
        for (behavior, visible) in [
            (Behavior::Fly, false),
            (Behavior::Nectar, true),
            (Behavior::Land, false),
        ] {
            app.world_mut().get_mut::<Motion>(root).unwrap().behavior = behavior;
            app.update();
            assert_eq!(
                *app.world().get::<Visibility>(tongue).unwrap() == Visibility::Inherited,
                visible,
            );
        }
    }
}
