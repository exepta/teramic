mod appearance;
mod blink;
mod character_gallery;
mod debug;
mod eye_gallery;
mod forest_animation;
mod gloom_effects;
pub(crate) mod horse_pose;
mod materials;
mod military;
mod model;
mod pet_preview;
mod sandworm;
mod spirit_animation;
use appearance::Appearance;
use bevy::prelude::*;
pub use pet_preview::RenderPetPortrait;
use std::collections::BTreeMap;
use terra_core::creatures::{CreatureBehavior as Behavior, CreatureKind as Kind};
use terra_network::client::ReplicatedCreatures;

#[derive(Component)]
pub struct CreatureAvatar(pub u64);

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CreatureAnimationSet;

#[derive(Component)]
pub struct VillagerHeldItem {
    pub item: terra_core::ItemKind,
    pub arm: Entity,
}

#[derive(Component)]
pub(crate) struct VillagerHand {
    pub item: terra_core::ItemKind,
    /// Reference residents carry their stowed tools in the authored mesh until working.
    pub work_only: bool,
    pub visible: bool,
    pub swing: f32,
}

impl VillagerHand {
    pub(crate) const PALM: Vec3 = Vec3::new(0.008, -0.43, -0.028);

    fn new(role: u8) -> Option<Self> {
        use terra_core::{ItemKind, ToolKind};
        let item = match role % terra_core::creatures::villagers::ROLE_COUNT {
            1 => ItemKind::Tool(ToolKind::IronHoe),
            2..=4 => ItemKind::Tool(ToolKind::IronSword),
            6 => ItemKind::Stick,
            7 => ItemKind::Flower(terra_core::beds::DyeColor::White),
            14 => ItemKind::Tool(ToolKind::IronPickaxe),
            9 => ItemKind::Tool(ToolKind::IronAxe),
            _ => return None,
        };
        let work_only = matches!(role, 1 | 9 | 14);
        Some(Self {
            item,
            work_only,
            visible: !work_only,
            swing: 0.0,
        })
    }
}
#[derive(Component)]
struct Motion {
    kind: Kind,
    behavior: Behavior,
    phase: f32,
    clock: f32,
    speed: f32,
    sleep: f32,
    attack: f32,
    worm_progress: f32,
    worm_spit: Option<[f32; 3]>,
    silk_length: f32,
    youth: f32,
    jump: horse_pose::JumpPose,
}
#[derive(Component)]
struct Limb(model::Part, Vec3);
#[derive(Resource, Default)]
struct Models {
    parts: BTreeMap<(Kind, u8), ModelParts>,
    material: Handle<StandardMaterial>,
    bell_material: Handle<StandardMaterial>,
    spirit_material: Handle<StandardMaterial>,
    glow_material: Handle<StandardMaterial>,
    violet_spirit_material: Handle<StandardMaterial>,
    violet_glow_material: Handle<StandardMaterial>,
    blood_spirit_material: Handle<StandardMaterial>,
    blood_glow_material: Handle<StandardMaterial>,
    eye_material: Handle<StandardMaterial>,
    stalker_eye_material: Handle<StandardMaterial>,
    slime_material: Handle<StandardMaterial>,
    skeleton_eye_material: Handle<StandardMaterial>,
    gloom_shell: [Handle<StandardMaterial>; 7],
    gloom_wings: [Handle<StandardMaterial>; 7],
    gloom_glow: [Handle<StandardMaterial>; 7],
}
type ModelParts = Vec<(model::Part, Vec3, Handle<Mesh>)>;
pub(crate) fn install(app: &mut App) {
    sandworm::install(app);
    gloom_effects::install(app);
    character_gallery::install(app);
    eye_gallery::install(app);
    pet_preview::install(app);
    debug::install(app);
    app.init_resource::<military::Uniforms>();
    app.add_systems(Startup, setup).add_systems(
        Update,
        (
            render,
            military::update,
            animate.in_set(CreatureAnimationSet),
        )
            .chain()
            .after(crate::players::MountedPresentationSet)
            .after(terra_player::PlayerSet::Movement),
    );
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut models = Models {
        slime_material: materials.add(materials::slime_body()),
        skeleton_eye_material: materials.add(materials::skeleton_eyes()),
        material: materials.add(StandardMaterial {
            perceptual_roughness: 0.85,
            ..default()
        }),
        spirit_material: materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(0.10, 0.14, 0.21),
            perceptual_roughness: 0.6,
            ..default()
        }),
        glow_material: materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(0.7, 1.8, 2.6),
            perceptual_roughness: 0.5,
            ..default()
        }),
        violet_spirit_material: materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(0.10, 0.015, 0.16),
            perceptual_roughness: 0.6,
            ..default()
        }),
        violet_glow_material: materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(0.75, 0.12, 1.2),
            perceptual_roughness: 0.5,
            ..default()
        }),
        blood_spirit_material: materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(0.15, 0.008, 0.02),
            perceptual_roughness: 0.6,
            ..default()
        }),
        blood_glow_material: materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(1.2, 0.08, 0.13),
            perceptual_roughness: 0.5,
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
        eye_material: materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(0.16, 0.016, 0.006),
            perceptual_roughness: 0.65,
            ..default()
        }),
        stalker_eye_material: materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(0.26, 0.12, 0.025),
            perceptual_roughness: 0.75,
            ..default()
        }),
        ..default()
    };
    for variant in 0..terra_core::creatures::gloom::COUNT {
        let palette = terra_core::creatures::gloom::palette(variant);
        let [r, g, b] = palette.glow;
        let index = usize::from(variant);
        models.gloom_shell[index] = materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(r * 0.018, g * 0.018, b * 0.018),
            perceptual_roughness: 0.68,
            ..default()
        });
        models.gloom_wings[index] = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.64),
            alpha_mode: AlphaMode::Blend,
            emissive: LinearRgba::rgb(r * 0.035, g * 0.035, b * 0.035),
            perceptual_roughness: 0.42,
            reflectance: 0.12,
            cull_mode: None,
            ..default()
        });
        models.gloom_glow[index] = materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(r * 1.6, g * 1.6, b * 1.6),
            perceptual_roughness: 0.55,
            ..default()
        });
    }
    for kind in Kind::ALL {
        for variant in 0..if kind == Kind::Horse {
            16
        } else if kind == Kind::Fish {
            2
        } else if kind == Kind::Villager {
            terra_core::creatures::villagers::ROLE_COUNT * 2
        } else if matches!(kind, Kind::Butterfly | Kind::SpiritGloom | Kind::Skeleton) {
            kind.variant_count()
        } else {
            1
        } {
            if kind == Kind::Villager
                && variant >= terra_core::creatures::villagers::ROLE_COUNT
                && !terra_core::creatures::villagers::is_civilian(
                    variant % terra_core::creatures::villagers::ROLE_COUNT,
                )
            {
                continue;
            }
            match if variant == 0 {
                model::parts(kind)
            } else {
                model::parts_variant(
                    kind,
                    if kind == Kind::Fish {
                        variant * 2
                    } else {
                        variant
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
    mounts: Option<Res<crate::players::MountedPresentation>>,
    mut roots: Query<(
        Entity,
        &CreatureAvatar,
        &mut Transform,
        &mut Motion,
        Option<&Appearance>,
    )>,
) {
    let mut present = std::collections::HashSet::new();
    for (entity, avatar, mut transform, mut motion, appearance) in &mut roots {
        let Some((animal, position, mounted)) =
            presented_creature(avatar.0, animals.as_deref(), mounts.as_deref())
        else {
            commands.entity(entity).despawn();
            continue;
        };
        if appearance.is_some_and(|old| *old != Appearance::of(animal))
            && models.parts.contains_key(&Appearance::of(animal).0)
        {
            // Children, held tools and the new uniform are rebuilt together.
            // The same replicated NPC ID is spawned below in this frame.
            commands.entity(entity).despawn();
            continue;
        }
        present.insert(avatar.0);
        let previous_position = transform.translation;
        let sleeping = animal.behavior == Behavior::Sleep;
        motion.sleep += ((if sleeping { 1.0 } else { 0.0 }) - motion.sleep)
            * (time.delta_secs() * 7.0).min(1.0);
        transform.scale = instance_scale(animal);
        if animal.kind == Kind::Horse {
            motion.jump = mounts
                .as_ref()
                .and_then(|m| m.horses.get(&avatar.0))
                .map_or_else(
                    || {
                        let mut jump = motion.jump;
                        jump.advance(animal, time.delta_secs());
                        jump
                    },
                    |p| p.jump,
                );
        }
        transform.rotation = if animal.kind == Kind::Horse {
            let yaw = if mounted {
                animal.yaw
            } else {
                Quat::from_rotation_y(transform.rotation.to_euler(EulerRot::YXZ).0)
                    .slerp(
                        Quat::from_rotation_y(animal.yaw),
                        1.0 - (-time.delta_secs() * 10.0).exp(),
                    )
                    .to_euler(EulerRot::YXZ)
                    .0
            };
            motion.jump.orientation(yaw)
        } else if mounted {
            rotation(animal)
        } else {
            transform
                .rotation
                .slerp(rotation(animal), 1.0 - (-time.delta_secs() * 10.0).exp())
        };
        transform.translation = spider_origin(animal, position, transform.rotation)
            + Vec3::Y * sleep_offset(animal.kind) * motion.sleep * animal.growth_scale() as f32;
        if animal.kind == Kind::Horse {
            transform.translation =
                horse_pose::seat_pivot_origin(animal, transform.translation, transform.rotation);
        }
        motion.behavior = animal.behavior;
        motion.youth = 1.0 - animal.growth();
        motion.silk_length = silk_length(animal);
        let reported = Vec3::from_array(animal.velocity).length();
        let speed = if mounted {
            reported
        } else if animal.kind == Kind::SandWorm {
            // A throttled authority tick may report zero velocity while the
            // replicated endpoints still show real burrow travel. Its sand
            // crest follows that presented displacement, never the camera.
            let delta = transform.translation - previous_position;
            (delta.x.hypot(delta.z) / time.delta_secs().max(0.001))
                .min((animal.kind.speed() as f32).max(reported) * 1.25)
        } else {
            (transform.translation.distance(previous_position) / time.delta_secs().max(0.001))
                .min(reported * 1.2)
        };
        motion.speed += (speed - motion.speed) * (1.0 - (-time.delta_secs() * 10.0).exp());
        // Extrapolate at most one snapshot interval, rather than visibly
        // stepping the breach at network tick rate. Frozen gallery poses have
        // their receive time refreshed and remain fixed.
        let since = animals
            .as_ref()
            .and_then(|r| r.creatures.get(&animal.id))
            .map_or(0.0, |track| {
                track
                    .received
                    .elapsed()
                    .as_secs_f32()
                    .min(track.interval.clamp(0.0, 0.25))
            });
        if animal.kind == Kind::SandWorm {
            motion.worm_spit = if animal.behavior == Behavior::Spit {
                terra_core::creatures::sandworm::aimed_spit_launch(animal).map(|(_, v)| {
                    let (sin, cos) = animal.yaw.sin_cos();
                    [
                        (v[0] as f32) * cos - (v[2] as f32) * sin,
                        v[1] as f32,
                        (v[0] as f32) * sin + (v[2] as f32) * cos,
                    ]
                })
            } else {
                None
            };
            motion.worm_progress = terra_core::creatures::sandworm::phase_progress(
                animal.behavior,
                animal.timer - since,
            );
        }
        let forest_melee = matches!(
            animal.kind,
            Kind::Skeleton | Kind::BloodStalker | Kind::BloodWisp | Kind::ThornBrute
        );
        let strike = if forest_melee {
            if animal.behavior == Behavior::Attack {
                terra_core::creatures::definitions::get(animal.kind)
                    .combat
                    .map_or(0.0, |combat| combat.melee_phase(animal.timer - since))
            } else {
                0.0
            }
        } else if animal.kind == Kind::SandWorm {
            terra_core::creatures::sandworm::emergence(animal)
        } else if animal.kind == Kind::Stalker && animal.behavior == Behavior::Hold {
            1.0
        } else if (animal.kind == Kind::Bear
            || animal.kind.is_spider()
            || matches!(
                animal.kind,
                Kind::Skeleton | Kind::BloodStalker | Kind::BloodWisp | Kind::ThornBrute
            ))
            && matches!(animal.behavior, Behavior::Attack | Behavior::Hunt)
            && speed < 0.45
        {
            (1.0 - animal.timer / 0.5).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if forest_melee && strike < motion.attack - 0.5 {
            // The recovery endpoint and next cycle's neutral pose coincide.
            motion.attack = strike;
        } else {
            motion.attack += (strike - motion.attack) * (1.0 - (-time.delta_secs() * 16.0).exp());
        }
        motion.phase +=
            time.delta_secs() * (motion.speed * 5.0 + 0.5) * (1.0 + motion.youth * 0.35);
        if animal.kind == Kind::SandWorm {
            motion.phase = animal.age as f32 + since;
        }
        motion.clock += time.delta_secs() * (1.0 + motion.youth * 0.20);
    }
    let ids = animals
        .as_ref()
        .into_iter()
        .flat_map(|a| a.creatures.keys())
        .chain(mounts.as_ref().into_iter().flat_map(|m| m.horses.keys()));
    for &id in ids {
        if present.contains(&id) {
            continue;
        }
        let Some((animal, position, _)) =
            presented_creature(id, animals.as_deref(), mounts.as_deref())
        else {
            continue;
        };
        let appearance = Appearance::of(animal);
        let Some(parts) = models.parts.get(&appearance.0) else {
            continue;
        };
        present.insert(id);
        let mut jump = horse_pose::JumpPose::default();
        if animal.kind == Kind::Horse {
            jump = mounts.as_ref().and_then(|m| m.horses.get(&id)).map_or_else(
                || {
                    let mut pose = horse_pose::JumpPose::default();
                    pose.advance(animal, time.delta_secs());
                    pose
                },
                |p| p.jump,
            );
        }
        let orientation = if animal.kind == Kind::Horse {
            jump.orientation(animal.yaw)
        } else {
            rotation(animal)
        };
        let mut origin = spider_origin(animal, position, orientation)
            + Vec3::Y
                * if animal.behavior == Behavior::Sleep {
                    sleep_offset(animal.kind)
                } else {
                    0.0
                }
                * animal.growth_scale() as f32;
        if animal.kind == Kind::Horse {
            origin = horse_pose::seat_pivot_origin(animal, origin, orientation);
        }
        let root = commands
            .spawn((
                CreatureAvatar(id),
                appearance,
                Motion {
                    kind: animal.kind,
                    behavior: animal.behavior,
                    phase: (id % 31) as f32,
                    clock: (id % 29) as f32 * 0.43,
                    speed: 0.0,
                    attack: 0.0,
                    worm_progress: terra_core::creatures::sandworm::phase_progress(
                        animal.behavior,
                        animal.timer,
                    ),
                    worm_spit: if animal.behavior == Behavior::Spit {
                        terra_core::creatures::sandworm::aimed_spit_launch(animal).map(|(_, v)| {
                            let (sin, cos) = animal.yaw.sin_cos();
                            [
                                (v[0] as f32) * cos - (v[2] as f32) * sin,
                                v[1] as f32,
                                (v[0] as f32) * sin + (v[2] as f32) * cos,
                            ]
                        })
                    } else {
                        None
                    },
                    silk_length: silk_length(animal),
                    youth: 1.0 - animal.growth(),
                    jump,
                    sleep: if animal.behavior == Behavior::Sleep {
                        1.0
                    } else {
                        0.0
                    },
                },
                Name::new(format!("{:?} {id}", animal.kind)),
                Transform::from_translation(origin)
                    .with_rotation(orientation)
                    .with_scale(instance_scale(animal)),
                Visibility::default(),
            ))
            .id();
        for (part, offset, mesh) in parts {
            let mut child = commands.spawn((
                Limb(*part, *offset),
                crate::miniature::creature_layers(),
                terra_core::miniature::MiniatureMesh::Creature,
                if matches!(
                    part,
                    model::Part::Eyelids | model::Part::Proboscis | model::Part::Silk
                ) {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(models.material_for(animal.kind, animal.variant, *part)),
                Transform::from_translation(*offset),
                ChildOf(root),
            ));
            if matches!(part, model::Part::WormRipple(_) | model::Part::WormMound) {
                child.insert(sandworm::WaveGround::default());
            }
            if matches!(part, model::Part::MilitaryTabard) {
                child.insert(military::Heraldry(id));
            }
            if animal.kind.is_jelly()
                || animal.kind == Kind::BloodSlime && matches!(part, model::Part::Body)
                || matches!(
                    part,
                    model::Part::WormRipple(_) | model::Part::WormDust | model::Part::WormSpit(_)
                )
            {
                child.insert(bevy::light::NotShadowCaster);
            }
            if animal.kind == Kind::Villager
                && matches!(part, model::Part::VillagerArm(side) if *side > 0.0)
                && let Some(hand) = VillagerHand::new(animal.variant)
            {
                child.insert(hand);
            }
        }
    }
}

fn presented_creature<'a>(
    id: u64,
    animals: Option<&'a ReplicatedCreatures>,
    mounts: Option<&'a crate::players::MountedPresentation>,
) -> Option<(&'a terra_core::creatures::Creature, Vec3, bool)> {
    if let Some(pose) = mounts.and_then(|m| m.horses.get(&id)) {
        return Some((
            &pose.horse,
            Vec3::from_array(pose.horse.position.map(|v| v as f32)),
            true,
        ));
    }
    let track = animals?
        .creatures
        .get(&id)
        .filter(|t| t.current.behavior != Behavior::InNest)?;
    Some((&track.current, track.position(), false))
}
fn model_scale(kind: Kind) -> Vec3 {
    scale_for_size(kind, kind.size())
}
fn scale_for_size(kind: Kind, size: [f64; 3]) -> Vec3 {
    let authored = terra_core::creatures::hitboxes::model_reference_size(kind);
    Vec3::from_array(std::array::from_fn(|i| (size[i] / authored[i]) as f32))
}
fn instance_scale(animal: &terra_core::creatures::Creature) -> Vec3 {
    model_scale(animal.kind)
        * Vec3::from_array(animal.aquatic_scale())
        * animal.growth_scale() as f32
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
    mut limbs: Query<(
        &ChildOf,
        &Limb,
        &mut Transform,
        &mut Visibility,
        Option<&mut VillagerHand>,
    )>,
) {
    for (parent, part, mut transform, mut visibility, hand) in &mut limbs {
        let Ok(motion) = roots.get(parent.parent()) else {
            continue;
        };
        transform.translation = part.1;
        transform.scale = Vec3::ONE;
        if matches!(
            part.0,
            model::Part::Head | model::Part::Eyelids | model::Part::SpiderEyes
        ) {
            transform.scale *= 1.0 + motion.youth * 0.12;
        }
        if matches!(part.0, model::Part::Eyelids) {
            *visibility = if blink::closure(motion.clock, motion.sleep) > 0.001 {
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
            transform.scale.y = 1.0 + (motion.clock * 2.1).sin() * (0.004 + motion.youth * 0.003);
        }
        if motion.kind == Kind::SandWorm {
            sandworm::animate(motion, part, &mut transform, &mut visibility);
            continue;
        }
        if matches!(
            motion.kind,
            Kind::Butterfly | Kind::SpiritGloom | Kind::SpiritFox
        ) {
            spirit_animation::animate(motion, part, &mut transform);
            continue;
        }
        if matches!(
            motion.kind,
            Kind::Skeleton
                | Kind::Raven
                | Kind::BloodSlime
                | Kind::BloodStalker
                | Kind::BloodWisp
                | Kind::ThornBrute
        ) {
            forest_animation::animate(motion, part, &mut transform);
            continue;
        }
        if motion.kind == Kind::Bear {
            animate_bear(motion, part, &mut transform);
            blink::apply(motion, part, &mut transform);
            continue;
        }
        if motion.kind == Kind::Stalker {
            animate_stalker(motion, part, &mut transform);
            continue;
        }
        if motion.kind == Kind::Villager {
            if matches!(part.0, model::Part::MilitaryWeapon(_)) {
                *visibility = if motion.sleep > 0.5 {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
            }
            animate_villager(motion, part, &mut transform);
            if let Some(mut hand) = hand {
                hand.visible = motion.behavior != Behavior::Sleep
                    && motion.sleep < 0.5
                    && (!hand.work_only
                        || motion.behavior == Behavior::Attack
                        || motion.attack > 0.0);
                hand.swing = villager_action(motion);
            }
            continue;
        }
        if motion.kind.is_spider() {
            if matches!(part.0, model::Part::Silk) {
                *visibility = if motion.silk_length > 0.01 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            animate_spider(motion, part, &mut transform);
            continue;
        }
        if motion.kind.is_aquatic() && motion.kind != Kind::Fish {
            animate_aquatic(motion, part, &mut transform);
            continue;
        }
        transform.rotation = match part.0 {
            model::Part::Leg(sign) if motion.kind == Kind::Horse && motion.jump.blend > 0.001 => {
                let stride = motion.phase.sin() * sign * (motion.speed * 0.30).min(0.60);
                let tucked = if part.1.z < 0.0 {
                    motion.jump.front
                } else {
                    motion.jump.rear
                };
                Quat::from_rotation_x(stride * (1.0 - motion.jump.blend) + tucked)
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
        blink::apply(motion, part, &mut transform);
    }
}

fn animate_villager(motion: &Motion, part: &Limb, transform: &mut Transform) {
    let awake = 1.0 - motion.sleep;
    let movement = (motion.speed / 1.35).clamp(0.0, 1.0) * awake;
    let stride = (motion.speed * 0.34).min(0.52) * awake;
    let bob = (motion.phase * 2.0).sin().abs() * 0.012 * movement;
    transform.translation = part.1 + Vec3::Y * bob;
    transform.rotation = match part.0 {
        model::Part::Leg(side) => {
            let angle = motion.phase.sin() * side * stride;
            let sin = angle.sin();
            let sole_bottom = -(part.1.y - 0.0025) * angle.cos()
                - if sin > 0.0 { 0.125 * sin } else { -0.225 * sin };
            // Keep the planted boot on its support; the returning foot lifts.
            transform.translation.y += (0.0025 - (part.1.y + sole_bottom)) * awake
                + (motion.phase.cos() * side).max(0.0) * 0.035 * movement;
            Quat::from_rotation_x(angle)
        }
        model::Part::VillagerArm(side) | model::Part::MilitaryWeapon(side) => Quat::from_euler(
            EulerRot::XYZ,
            -motion.phase.sin() * side * stride * 0.65
                - 0.06 * awake
                - 0.18 * motion.sleep
                - if side > 0.0 {
                    villager_action(motion) * 0.95
                } else {
                    0.0
                },
            0.0,
            side * (0.045 * awake + 0.22 * motion.sleep),
        ),
        model::Part::Head => {
            // The bed pillow is above its blanket; don't sink the head into it.
            transform.translation.z -= 0.085 * motion.sleep;
            Quat::from_euler(
                EulerRot::XYZ,
                (motion.clock * 1.7).sin() * 0.018 * awake - 0.04 * motion.sleep,
                (motion.clock * 0.41).sin() * 0.10 * awake,
                (motion.clock * 0.83).sin() * 0.008 * awake,
            )
        }
        model::Part::Body | model::Part::MilitaryTabard => {
            transform.scale.y = 1.0 + (motion.clock * 1.8).sin() * 0.003;
            Quat::from_euler(
                EulerRot::XYZ,
                0.0,
                -motion.phase.sin() * 0.025 * movement,
                (motion.phase * 0.5).sin() * 0.01 * movement,
            )
        }
        _ => Quat::IDENTITY,
    };
}

fn villager_action(motion: &Motion) -> f32 {
    if motion.behavior == Behavior::Attack {
        (motion.clock * 5.0).sin().max(0.0) * (1.0 - motion.sleep)
    } else {
        0.0
    }
}

fn animate_stalker(motion: &Motion, part: &Limb, transform: &mut Transform) {
    let breath = (motion.clock
        * if motion.behavior == Behavior::Attack {
            6.2
        } else {
            1.5
        })
    .sin();
    let stride = motion.phase.sin() * (motion.speed * 0.17).min(0.48);
    let grabbing = motion.attack;
    let striking = if motion.behavior == Behavior::Attack {
        (motion.clock * 5.0).sin().max(0.0)
    } else {
        0.0
    };
    let recoil = if motion.behavior == Behavior::Flee {
        1.0
    } else {
        0.0
    };
    match part.0 {
        model::Part::Body => {
            transform.scale.y = 1.0 + breath * 0.007;
            transform.rotation =
                Quat::from_euler(EulerRot::XYZ, recoil * -0.055, 0.0, stride * 0.035);
        }
        model::Part::Head | model::Part::StalkerEyes => {
            let peek = if motion.behavior == Behavior::Hunt && motion.speed < 0.1 {
                (motion.clock * 0.7).sin()
            } else {
                0.0
            };
            transform.translation.x += peek * 0.12;
            transform.translation.y += breath * 0.006 + recoil * 0.045;
            transform.rotation = Quat::from_euler(
                EulerRot::XYZ,
                recoil * -0.12 + grabbing * 0.10,
                peek * 0.13 + (motion.clock * 0.45).sin() * 0.035,
                peek * -0.065,
            );
        }
        model::Part::Leg(side) => {
            transform.rotation = Quat::from_rotation_x(stride * side);
            transform.translation.y +=
                (motion.phase.sin() * side).max(0.0) * motion.speed.min(2.0) * 0.025;
        }
        model::Part::StalkerArm(side)
        | model::Part::StalkerForearm(side)
        | model::Part::StalkerHand(side) => {
            let shoulder = Vec3::new(side * 0.51, 2.06, 0.03);
            let elbow_rest = Vec3::new(side * 0.80, 1.17, -0.16);
            let wrist_rest = Vec3::new(side * 1.01, 0.52, -0.45);
            let upper = Quat::from_euler(
                EulerRot::XYZ,
                -stride * side * 0.45 + grabbing * 0.80 + striking * 0.75 - recoil * 0.20,
                0.0,
                side * (breath * 0.015 - grabbing * 0.13),
            );
            let elbow = shoulder + upper * (elbow_rest - shoulder);
            let forearm = upper * Quat::from_rotation_x(grabbing * 0.36 + striking * 0.28);
            let wrist = elbow + forearm * (wrist_rest - elbow_rest);
            (transform.translation, transform.rotation) = match part.0 {
                model::Part::StalkerArm(_) => (shoulder, upper),
                model::Part::StalkerForearm(_) => (elbow, forearm),
                _ => (
                    wrist,
                    forearm * Quat::from_rotation_z(side * grabbing * 0.10),
                ),
            };
        }
        _ => {}
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

fn animate_spider(motion: &Motion, part: &Limb, transform: &mut Transform) {
    let body_height = match motion.kind {
        Kind::SmallSpider => 0.1,
        Kind::MediumSpider | Kind::BloodStalker => 0.5,
        _ => 1.25,
    };
    match part.0 {
        model::Part::SpiderLeg { side, index } => {
            let active = (motion.speed * 1.8).clamp(0.0, 1.0);
            let phase = motion.phase * 1.7
                + f32::from(index % 2) * std::f32::consts::PI
                + if side > 0.0 {
                    std::f32::consts::PI
                } else {
                    0.0
                };
            let lift = phase.sin().max(0.0) * active;
            transform.translation.y += lift * body_height * 0.12;
            transform.rotation = Quat::from_euler(
                EulerRot::XYZ,
                phase.cos() * active * 0.10,
                phase.cos() * side * active * 0.18,
                side * lift * 0.13 + (motion.clock * 0.7 + f32::from(index)).sin() * 0.008,
            );
        }
        model::Part::Head | model::Part::SpiderEyes => {
            transform.rotation = Quat::from_euler(
                EulerRot::XYZ,
                -motion.attack * 0.15 + (motion.clock * 1.3).sin() * 0.018,
                (motion.clock * 0.8).sin() * 0.035,
                0.0,
            );
            transform.translation.z -= motion.attack * body_height * 0.08;
        }
        model::Part::Body => {
            transform.rotation = Quat::from_rotation_z(
                (motion.phase * 1.7).sin() * (motion.speed * 0.012).min(0.025),
            );
        }
        model::Part::Silk => {
            transform.translation.y = body_height * 1.35;
            transform.scale.y = motion.silk_length.max(0.001);
            transform.rotation = Quat::IDENTITY;
        }
        _ => {
            transform.rotation = Quat::IDENTITY;
        }
    }
}

fn silk_length(animal: &terra_core::creatures::Creature) -> f32 {
    if !animal.kind.is_spider() || animal.behavior != Behavior::Descend {
        return 0.0;
    }
    animal.needs.goal.map_or(0.0, |goal| {
        (goal.position[1] as f32
            - animal.position[1] as f32
            - animal.kind.size()[1] as f32 * 1.35 * animal.growth_scale() as f32)
            .clamp(0.0, 48.0)
    })
}

fn spider_origin(
    animal: &terra_core::creatures::Creature,
    interpolated: Vec3,
    orientation: Quat,
) -> Vec3 {
    if !animal.kind.is_spider() {
        return interpolated;
    }
    let center =
        Vec3::from_array(terra_core::creatures::spiders::visual_center(animal).map(|v| v as f32));
    let pivot = Vec3::from_array(
        terra_core::creatures::spiders::model_pivot(animal.kind)
            .map(|v| (v * animal.growth_scale()) as f32),
    );
    interpolated + center
        - Vec3::from_array(animal.position.map(|v| v as f32))
        - orientation * pivot
}

fn rotation(animal: &terra_core::creatures::Creature) -> Quat {
    let yaw = Quat::from_rotation_y(animal.yaw);
    if animal.kind == Kind::Villager {
        return yaw
            * Quat::from_rotation_x(if animal.behavior == Behavior::Sleep {
                std::f32::consts::FRAC_PI_2
            } else {
                0.0
            });
    }
    if animal.kind.is_spider() {
        return yaw
            * match animal.behavior {
                Behavior::Ceiling => Quat::from_rotation_z(std::f32::consts::PI),
                Behavior::Climb => Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                _ => Quat::IDENTITY,
            };
    }
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
    fn all_butterfly_variants_render_at_half_the_original_dimensions() {
        assert_eq!(model_scale(Kind::Butterfly), Vec3::splat(0.5));
        for variant in 0..Kind::Butterfly.variant_count() {
            let animal = terra_core::creatures::Creature {
                id: 1,
                kind: Kind::Butterfly,
                home: [0; 2],
                position: [0.0; 3],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: 2.0,
                behavior: Behavior::Fly,
                timer: 1.0,
                age: 0.0,
                variant,
                needs: terra_core::creatures::CreatureNeeds::full(Kind::Butterfly),
            };
            assert_eq!(instance_scale(&animal), Vec3::splat(0.5));
        }
    }

    #[test]
    fn blinking_lids_follow_the_exact_head_transform_and_reopen_after_a_blink() {
        for kind in [Kind::Sheep, Kind::Cow, Kind::Pig, Kind::Bear, Kind::Horse] {
            let mut app = App::new();
            app.add_systems(Update, animate);
            let root = app
                .world_mut()
                .spawn(Motion {
                    kind,
                    behavior: Behavior::Rest,
                    phase: 0.7,
                    clock: 1.0,
                    speed: 0.0,
                    sleep: 0.0,
                    attack: 0.0,
                    worm_progress: 0.0,
                    worm_spit: None,
                    silk_length: 0.0,
                    youth: 0.8,
                    jump: Default::default(),
                })
                .id();
            let pivot = Vec3::new(0.0, 1.2, -0.5);
            let head = app
                .world_mut()
                .spawn((
                    ChildOf(root),
                    Limb(model::Part::Head, pivot),
                    Transform::default(),
                    Visibility::Inherited,
                ))
                .id();
            let lids = app
                .world_mut()
                .spawn((
                    ChildOf(root),
                    Limb(model::Part::Eyelids, pivot),
                    Transform::default(),
                    Visibility::Hidden,
                ))
                .id();
            for (clock, behavior, sleep, closed) in [
                (1.0, Behavior::Rest, 0.0, false),
                (6.77, Behavior::Graze, 0.0, true),
                (6.9, Behavior::Drink, 0.0, false),
                (7.5, Behavior::Sleep, 1.0, true),
                (8.0, Behavior::Rest, 0.0, false),
            ] {
                {
                    let mut motion = app.world_mut().get_mut::<Motion>(root).unwrap();
                    motion.clock = clock;
                    motion.behavior = behavior;
                    motion.sleep = sleep;
                }
                app.update();
                if closed {
                    assert_eq!(
                        app.world().get::<Transform>(head),
                        app.world().get::<Transform>(lids),
                        "{kind:?}/{behavior:?}: lid drift"
                    );
                }
                assert_eq!(
                    *app.world().get::<Visibility>(lids).unwrap(),
                    if closed {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    }
                );
            }
        }
    }

    #[test]
    fn villager_right_hand_tracks_walk_and_action_but_hides_held_tools_during_sleep() {
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .add_systems(Update, animate);
        let root = app
            .world_mut()
            .spawn((
                Motion {
                    kind: Kind::Villager,
                    behavior: Behavior::Rest,
                    phase: 0.0,
                    clock: 0.25,
                    speed: 0.0,
                    sleep: 0.0,
                    attack: 0.0,
                    worm_progress: 0.0,
                    worm_spit: None,
                    silk_length: 0.0,
                    youth: 0.0,
                    jump: Default::default(),
                },
                Transform::default(),
                Visibility::Inherited,
            ))
            .id();
        let hand = VillagerHand::new(14).unwrap();
        assert_eq!(
            hand.item,
            terra_core::ItemKind::Tool(terra_core::ToolKind::IronPickaxe)
        );
        let arm = app
            .world_mut()
            .spawn((
                ChildOf(root),
                Limb(model::Part::VillagerArm(1.0), Vec3::new(0.3, 1.15, 0.0)),
                Transform::default(),
                Visibility::Inherited,
                hand,
            ))
            .id();
        let mut grips = Vec::new();
        for (behavior, speed, phase, sleep) in [
            (Behavior::Rest, 0.0, 0.0, 0.0),
            (Behavior::Wander, 1.35, 0.7, 0.0),
            (Behavior::Attack, 0.0, 0.0, 0.0),
            (Behavior::Sleep, 0.0, 0.0, 1.0),
        ] {
            {
                let mut motion = app.world_mut().get_mut::<Motion>(root).unwrap();
                motion.behavior = behavior;
                motion.speed = speed;
                motion.phase = phase;
                motion.sleep = sleep;
            }
            app.update();
            let hand = app.world().get::<VillagerHand>(arm).unwrap();
            assert_eq!(hand.visible, behavior == Behavior::Attack);
            assert_eq!(hand.swing > 0.0, behavior == Behavior::Attack);
            grips.push(
                app.world()
                    .get::<Transform>(arm)
                    .unwrap()
                    .transform_point(VillagerHand::PALM),
            );
        }
        assert!(
            grips[0].distance(grips[1]) > 0.03,
            "walking carries the tool with the arm"
        );
        assert!(
            grips[0].distance(grips[2]) > 0.25,
            "actions move the hand, not just a detached prop"
        );
        assert!(VillagerHand::new(0).is_none());
        for role in [1, 2, 3, 4, 6, 7, 9, 14] {
            assert!(VillagerHand::new(role).is_some());
        }
        for role in [1, 9, 14] {
            let hand = VillagerHand::new(role).unwrap();
            assert!(
                hand.work_only && !hand.visible,
                "stowed reference tools must not be duplicated at rest"
            );
        }
        for role in [5, 8] {
            assert!(
                VillagerHand::new(role).is_none(),
                "bishop and smith use their authored crozier and hammer"
            );
        }
    }

    #[test]
    fn villager_walk_counter_swings_arms_keeps_planted_boots_grounded_and_sleeps_on_the_pillow() {
        use bevy::mesh::VertexAttributeValues;
        use terra_core::creatures::{Creature, CreatureNeeds};
        let mut motion = Motion {
            kind: Kind::Villager,
            behavior: Behavior::Wander,
            phase: 0.7,
            clock: 0.4,
            speed: 1.35,
            sleep: 0.0,
            attack: 0.0,
            worm_progress: 0.0,
            worm_spit: None,
            silk_length: 0.0,
            youth: 0.0,
            jump: Default::default(),
        };
        let parts = model::parts(Kind::Villager).unwrap();
        for phase in 0..24 {
            motion.phase = phase as f32 * std::f32::consts::TAU / 24.0;
            for (part, offset, mesh) in &parts {
                let mut pose = Transform::from_translation(*offset);
                animate_villager(&motion, &Limb(*part, *offset), &mut pose);
                assert!(pose.translation.is_finite() && pose.rotation.is_finite());
                if let model::Part::Leg(_) = part {
                    let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    else {
                        panic!("Missing villager boot positions");
                    };
                    let bottom = positions
                        .iter()
                        .map(|position| pose.transform_point(Vec3::from_array(*position)).y)
                        .fold(f32::INFINITY, f32::min);
                    assert!(
                        (-0.0001..0.05).contains(&bottom),
                        "Planted/swing boot must stay near the floor: {bottom}"
                    );
                }
            }
        }
        motion.phase = 0.7;
        let mut leg = Transform::default();
        let mut arm = Transform::default();
        animate_villager(
            &motion,
            &Limb(model::Part::Leg(1.0), Vec3::new(0.125, 0.6025, 0.0)),
            &mut leg,
        );
        animate_villager(
            &motion,
            &Limb(model::Part::VillagerArm(1.0), Vec3::new(0.3, 1.15, 0.0)),
            &mut arm,
        );
        assert!(
            leg.rotation.to_euler(EulerRot::XYZ).0 > 0.0
                && arm.rotation.to_euler(EulerRot::XYZ).0 < 0.0
        );
        motion.behavior = Behavior::Sleep;
        motion.sleep = 1.0;
        motion.speed = 0.0;
        let mut animal = Creature {
            id: terra_core::creatures::villagers::id([0, 0], 0).unwrap(),
            kind: Kind::Villager,
            home: [0, 0],
            position: [0.5, 0.79, 0.1],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: Kind::Villager.health(),
            behavior: Behavior::Sleep,
            timer: 1.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(Kind::Villager),
        };
        for facing in 0..4 {
            animal.yaw = facing as f32 * std::f32::consts::FRAC_PI_2;
            let orientation = rotation(&animal);
            let direction = Quat::from_rotation_y(animal.yaw) * Vec3::Z;
            assert!((orientation * Vec3::Y).dot(direction) > 0.999);
            assert!(
                (orientation * Vec3::NEG_Z).dot(Vec3::Y) > 0.999,
                "Lying chest should face upward"
            );
            for (part, offset, _) in &parts {
                let mut pose = Transform::from_translation(*offset);
                animate_villager(&motion, &Limb(*part, *offset), &mut pose);
                if matches!(part, model::Part::Leg(_)) {
                    assert!(pose.rotation.angle_between(Quat::IDENTITY) < 0.0001);
                }
                if matches!(part, model::Part::Head) {
                    let center_height = 0.79 + (orientation * pose.translation).y;
                    assert!(
                        center_height > 0.85,
                        "Head must be above the bed pillow, not inside the blanket"
                    );
                }
            }
        }
    }

    #[test]
    fn juvenile_scale_and_spider_pivot_match_continuously_growing_physical_bounds() {
        use terra_core::creatures::{Creature, CreatureNeeds, breeding};
        let mut animal = Creature {
            id: breeding::next_birth_id(&Default::default(), 0).unwrap(),
            kind: Kind::Cow,
            home: [0, 0],
            position: [0.0, 3.0, 0.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 10.0,
            behavior: Behavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(Kind::Cow),
        };
        assert!(instance_scale(&animal).abs_diff_eq(model_scale(Kind::Cow) * 0.45, 0.000_01));
        let first = instance_scale(&animal);
        animal.age = 600.0;
        let middle = instance_scale(&animal);
        animal.age = breeding::GROWTH_SECONDS;
        assert!(first.y < middle.y && middle.y < instance_scale(&animal).y);
        assert!(instance_scale(&animal).abs_diff_eq(model_scale(Kind::Cow), 0.000_01));
        animal.kind = Kind::MediumSpider;
        animal.age = 100.0;
        animal.behavior = Behavior::Ceiling;
        let position = Vec3::from_array(animal.position.map(|v| v as f32));
        let orientation = rotation(&animal);
        let pivot = Vec3::from_array(
            terra_core::creatures::spiders::model_pivot(animal.kind)
                .map(|v| (v * animal.growth_scale()) as f32),
        );
        let center = Vec3::from_array(
            terra_core::creatures::spiders::visual_center(&animal).map(|v| v as f32),
        );
        assert!(
            (spider_origin(&animal, position, orientation) + orientation * pivot)
                .abs_diff_eq(center, 0.000_01)
        );
    }

    #[test]
    fn spider_attachments_rotate_about_the_physical_center_and_silk_stays_ceiling_anchored() {
        use terra_core::creatures::{Creature, CreatureNeeds, NeedGoal, NeedKind};
        let mut animal = Creature {
            id: 1,
            kind: Kind::MediumSpider,
            home: [0, 0],
            position: [-16.5, -23.0, 2.5],
            velocity: [0.0; 3],
            yaw: std::f32::consts::FRAC_PI_2,
            health: 8.0,
            behavior: Behavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(Kind::MediumSpider),
        };
        for behavior in [
            Behavior::Rest,
            Behavior::Climb,
            Behavior::Ceiling,
            Behavior::Descend,
        ] {
            animal.behavior = behavior;
            let orientation = rotation(&animal);
            let position = Vec3::from_array(animal.position.map(|v| v as f32));
            let origin = spider_origin(&animal, position, orientation);
            let center = Vec3::from_array(
                terra_core::creatures::spiders::visual_center(&animal).map(|v| v as f32),
            );
            let pivot = Vec3::from_array(
                terra_core::creatures::spiders::model_pivot(animal.kind).map(|v| v as f32),
            );
            assert!((origin + orientation * pivot).abs_diff_eq(center, 0.00001));
            let normal = Vec3::from_array(
                terra_core::creatures::spiders::surface_normal(&animal).map(f32::from),
            );
            assert!((orientation * Vec3::Y).dot(normal) > 0.999);
        }
        animal.needs.goal = Some(NeedGoal {
            kind: NeedKind::Food,
            position: [-17, -18, 2],
        });
        assert!((silk_length(&animal) - 4.325).abs() < 0.001);
        animal.behavior = Behavior::Rest;
        assert_eq!(silk_length(&animal), 0.0);
    }

    #[test]
    fn spider_gait_has_eight_finite_alternating_legs_and_a_distinct_bite_pose() {
        let mut motion = Motion {
            kind: Kind::LargeSpider,
            behavior: Behavior::Wander,
            phase: 0.7,
            clock: 0.4,
            speed: 0.9,
            sleep: 0.0,
            attack: 0.0,
            worm_progress: 0.0,
            worm_spit: None,
            silk_length: 2.5,
            youth: 0.0,
            jump: Default::default(),
        };
        let mut poses = Vec::new();
        for side in [-1.0, 1.0] {
            for index in 0..4 {
                let mut pose = Transform::default();
                animate_spider(
                    &motion,
                    &Limb(model::Part::SpiderLeg { side, index }, Vec3::ZERO),
                    &mut pose,
                );
                assert!(pose.translation.is_finite() && pose.rotation.is_finite());
                poses.push(pose);
            }
        }
        assert!(poses[0].rotation.angle_between(poses[1].rotation) > 0.1);
        let head = Limb(model::Part::Head, Vec3::ZERO);
        let mut rest = Transform::default();
        animate_spider(&motion, &head, &mut rest);
        motion.attack = 1.0;
        let mut strike = Transform::default();
        animate_spider(&motion, &head, &mut strike);
        assert!(strike.translation.z < rest.translation.z);
        let mut silk = Transform::default();
        animate_spider(&motion, &Limb(model::Part::Silk, Vec3::ZERO), &mut silk);
        assert_eq!(silk.scale.y, 2.5);
    }

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
            worm_progress: 0.0,
            worm_spit: None,
            silk_length: 0.0,
            youth: 0.0,
            jump: Default::default(),
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
            worm_progress: 0.0,
            worm_spit: None,
            silk_length: 0.0,
            youth: 0.0,
            jump: Default::default(),
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
                worm_progress: 0.0,
                worm_spit: None,
                silk_length: 0.0,
                youth: 0.0,
                jump: Default::default(),
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
