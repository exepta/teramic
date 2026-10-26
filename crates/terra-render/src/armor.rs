//! Bounded shared equipment meshes, attached to the avatar's existing animated joints.
mod cape;
mod model;

use crate::players::{AvatarJoint, AvatarPart, WorldAvatarPart};
use bevy::{camera::visibility::RenderLayers, prelude::*};
use std::collections::HashMap;
use terra_core::{AccessoryKind, ArmorItem, ArmorMaterial, EquipmentSlots, ItemKind, PlayerGender};

#[derive(Component)]
struct CapeMotion {
    root: Entity,
    panel: u8,
    previous: Option<Vec3>,
    velocity: Vec3,
}

#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AvatarEquipment(pub [Option<ItemKind>; terra_core::EQUIPMENT_SLOTS]);

impl From<EquipmentSlots> for AvatarEquipment {
    fn from(slots: EquipmentSlots) -> Self {
        Self(std::array::from_fn(|index| {
            slots.slot(index).map(|stack| stack.item)
        }))
    }
}

#[derive(Component)]
pub(crate) struct EquipmentAttachment(Entity);

type WearableParts = Vec<(AvatarPart, Handle<Mesh>)>;

#[derive(Resource, Default)]
struct ArmorAssets {
    parts: HashMap<(ItemKind, bool), WearableParts>,
    materials: Vec<Handle<StandardMaterial>>,
}

#[derive(Resource, Default, Debug)]
pub struct ArmorRenderStatus {
    pub cached_meshes: usize,
    pub cached_materials: usize,
    pub equipped_parts: usize,
    pub rebuilds: u64,
}

pub(crate) use model::item_batches;

pub(crate) fn install(app: &mut App) {
    app.init_resource::<ArmorRenderStatus>()
        .add_systems(Startup, prepare)
        .add_systems(
            Update,
            (
                sync.after(crate::players::local_avatar)
                    .after(crate::players::render),
                count.after(sync),
                animate_capes
                    .after(sync)
                    .after(crate::weather::WeatherSet::Local),
            ),
        );
}

fn prepare(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut status: ResMut<ArmorRenderStatus>,
) {
    let mut assets = ArmorAssets::default();
    for tier in ArmorMaterial::ALL {
        assets.materials.push(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            metallic: if tier == ArmorMaterial::Leather {
                0.0
            } else {
                0.38
            },
            perceptual_roughness: if tier == ArmorMaterial::Leather {
                0.88
            } else {
                0.44
            },
            ..default()
        }));
    }
    assets.materials.push(materials.add(StandardMaterial {
        base_color: Color::WHITE,
        metallic: 0.18,
        perceptual_roughness: 0.68,
        ..default()
    }));
    assets.materials.push(materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.94,
        ..default()
    }));
    for item in ArmorItem::ALL
        .map(ItemKind::Armor)
        .into_iter()
        .chain(AccessoryKind::ALL.map(ItemKind::Accessory))
    {
        for gender in [PlayerGender::Male, PlayerGender::Female] {
            match model::parts(item, gender) {
                Ok(parts) => {
                    let parts: Vec<_> = parts
                        .into_iter()
                        .map(|(part, batch)| {
                            (part, meshes.add(crate::previews::batch_mesh(&batch)))
                        })
                        .collect();
                    status.cached_meshes += parts.len();
                    assets
                        .parts
                        .insert((item, gender == PlayerGender::Female), parts);
                }
                Err(error) => {
                    tracing::error!(%error, ?item, "Cannot build shared equipment geometry")
                }
            }
        }
    }
    status.cached_materials = assets.materials.len();
    commands.insert_resource(assets);
}

fn material_index(item: ItemKind) -> usize {
    match item {
        ItemKind::Armor(armor) => ArmorMaterial::ALL
            .iter()
            .position(|tier| *tier == armor.material)
            .unwrap_or(0),
        ItemKind::Cape(_) => 6,
        _ => 5,
    }
}

type AvatarJoints<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static AvatarJoint,
        &'static AvatarPart,
        &'static RenderLayers,
    ),
>;

fn sync(
    mut commands: Commands,
    mut assets: ResMut<ArmorAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    roots: Query<
        (
            Entity,
            &AvatarEquipment,
            Option<&crate::players::AvatarGender>,
        ),
        Changed<AvatarEquipment>,
    >,
    joints: AvatarJoints,
    attachments: Query<(Entity, &EquipmentAttachment)>,
    mut status: ResMut<ArmorRenderStatus>,
) {
    if roots.is_empty() {
        return;
    }
    for (root, equipment, gender) in &roots {
        for (entity, attachment) in &attachments {
            if attachment.0 == root {
                commands.entity(entity).try_despawn();
            }
        }
        let female = gender.is_some_and(|gender| gender.0 == PlayerGender::Female);
        for item in equipment.0.into_iter().flatten() {
            if matches!(item, ItemKind::Cape(_)) && !assets.parts.contains_key(&(item, female)) {
                let gender = if female {
                    PlayerGender::Female
                } else {
                    PlayerGender::Male
                };
                if let Ok(parts) = model::parts(item, gender) {
                    let parts: Vec<_> = parts
                        .into_iter()
                        .map(|(part, batch)| {
                            (part, meshes.add(crate::previews::batch_mesh(&batch)))
                        })
                        .collect();
                    status.cached_meshes += parts.len();
                    assets.parts.insert((item, female), parts);
                }
            }
            let Some(parts) = assets.parts.get(&(item, female)) else {
                continue;
            };
            let is_cape = matches!(item, ItemKind::Cape(_));
            let mut upper_cape = None;
            for (index, (part, mesh)) in parts.iter().enumerate() {
                let Some((joint_entity, _, _, layers)) = joints
                    .iter()
                    .find(|(_, joint, candidate, _)| joint.root == root && *candidate == part)
                else {
                    continue;
                };
                let world = layers.intersects(&RenderLayers::default())
                    || layers.intersects(&RenderLayers::layer(terra_core::miniature::WORLD_LAYER));
                let parent = if is_cape && index == 2 {
                    upper_cape.unwrap_or(joint_entity)
                } else {
                    joint_entity
                };
                let mut child = commands.spawn((
                    EquipmentAttachment(root),
                    Name::new(format!("Equipment {} {:?}", item.identifier(), part)),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(assets.materials[material_index(item)].clone()),
                    Transform::default(),
                    layers.clone(),
                    ChildOf(parent),
                ));
                if world {
                    child.insert((
                        WorldAvatarPart(root),
                        terra_core::miniature::MiniatureMesh::Player,
                    ));
                }
                if is_cape && index == 1 {
                    upper_cape = Some(child.id());
                }
                if is_cape && index > 0 && world {
                    child.insert(CapeMotion {
                        root,
                        panel: index as u8,
                        previous: None,
                        velocity: Vec3::ZERO,
                    });
                }
            }
        }
        status.rebuilds = status.rebuilds.saturating_add(1);
    }
}

fn animate_capes(
    time: Res<Time>,
    weather: Option<Res<crate::weather::WeatherVisual>>,
    roots: Query<&GlobalTransform, Without<CapeMotion>>,
    mut capes: Query<(&mut CapeMotion, &mut Transform)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    if dt <= 0.0 {
        return;
    }
    let wind = weather.as_ref().map_or(Vec3::ZERO, |weather| {
        let wind = Vec3::new(weather.wind.x, 0.0, weather.wind.y);
        if wind.is_finite() {
            wind.clamp_length_max(30.0)
        } else {
            Vec3::ZERO
        }
    });
    for (mut motion, mut transform) in &mut capes {
        let Ok(root) = roots.get(motion.root) else {
            continue;
        };
        let position = root.translation();
        let velocity = motion.previous.map_or(Vec3::ZERO, |previous| {
            ((position - previous).with_y(0.0) / dt).clamp_length_max(12.0)
        });
        motion.previous = Some(position);
        let velocity_delta = velocity - motion.velocity;
        motion.velocity += velocity_delta * (1.0 - (-12.0 * dt).exp());
        let airflow = root.rotation().inverse() * (wind - motion.velocity);
        let phase = time.elapsed_secs() * (2.3 + airflow.length().min(15.0) * 0.17)
            + motion.root.index_u32() as f32 * 0.71
            - f32::from(motion.panel) * 0.85;
        let target = cape_rotation(motion.panel, airflow, phase);
        let response = if motion.panel == 1 { 7.0 } else { 4.5 };
        let rotation = transform
            .rotation
            .slerp(target, 1.0 - (-response * dt).exp());
        let pivot = if motion.panel == 1 {
            cape::SHOULDER_PIVOT
        } else {
            cape::HEM_PIVOT
        };
        transform.translation = pivot - rotation * pivot;
        transform.rotation = rotation;
    }
}

fn cape_rotation(panel: u8, airflow: Vec3, phase: f32) -> Quat {
    let lower = panel == 2;
    let force = (airflow.length() / 6.0).min(2.0);
    let pull = (airflow.z.max(0.0) * 0.095 + airflow.x.abs() * 0.025).min(0.82);
    let flutter =
        (phase.sin() + (phase * 1.73).sin() * 0.35) * force * if lower { 0.035 } else { 0.012 };
    let lift = (pull * if lower { 0.52 } else { 1.0 } + flutter)
        .clamp(0.0, if lower { 0.48 } else { 0.88 });
    let sway = ((airflow.x * 0.03).clamp(-0.32, 0.32) * if lower { 0.65 } else { 1.0 }
        + (phase * 0.71).sin() * force * if lower { 0.025 } else { 0.012 })
    .clamp(-0.38, 0.38);
    Quat::from_rotation_x(-lift) * Quat::from_rotation_z(sway)
}

fn count(attachments: Query<(), With<EquipmentAttachment>>, mut status: ResMut<ArmorRenderStatus>) {
    let count = attachments.iter().count();
    if status.equipped_parts != count {
        status.equipped_parts = count;
    }
}

#[cfg(test)]
mod tests;
