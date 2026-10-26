use super::{BuildingStatus, BuildingWorld};
use bevy::{light::NotShadowCaster, prelude::*};
use terra_core::{ItemKind, ResourcePropKind};
use terra_network::{
    client::ConfirmedEdit,
    protocol::session::{EditAction, EditOutcome, MiningTarget},
};

const MAX_PARTICLES: usize = 48;
const LIFETIME: f32 = 0.45;

#[derive(Component)]
pub(super) struct BreakParticle {
    velocity: Vec3,
    life: f32,
    size: Vec3,
}

#[derive(Resource, Default)]
pub(super) struct Effects {
    serial: u32,
    flash: Option<(Vec3, f32)>,
    mining: Option<(MiningTarget, usize)>,
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct ChipScene<'w, 's> {
    journal: Option<Res<'w, terra_world::edits::EditJournal>>,
    properties: Option<Res<'w, terra_core::block_properties::BlockPropertiesCatalog>>,
    cameras:
        Query<'w, 's, (&'static Transform, &'static terra_player::CameraLook), ChipCameraFilter>,
}
type ChipCameraFilter = (With<terra_player::TerraCamera>, Without<BreakParticle>);

#[allow(clippy::too_many_arguments)]
pub(super) fn animate(
    mut commands: Commands,
    world: BuildingWorld,
    mut feedback: MessageReader<ConfirmedEdit>,
    mut particles: Query<(Entity, &mut Transform, &mut BreakParticle)>,
    mut effects: ResMut<Effects>,
    mut status: ResMut<BuildingStatus>,
    mut gizmos: Gizmos,
    scene: ChipScene,
) {
    let delta = world.time.delta_secs().min(0.05);
    let active = world.active();
    let mut count = 0;
    for (entity, mut transform, mut particle) in &mut particles {
        particle.life -= delta;
        let next = transform.translation + particle.velocity * delta;
        let cell = next.to_array().map(|v| v.floor() as i64);
        let collision = particle.life < LIFETIME - 0.08 && world.chunks.block_at(cell) != Some(0);
        if !active || particle.life <= 0.0 || collision {
            commands.entity(entity).despawn();
        } else {
            particle.velocity.y -= delta * 9.8;
            transform.translation = next;
            transform.rotate(Quat::from_rotation_y(delta * 5.0));
            transform.scale = particle.size * (particle.life / LIFETIME).sqrt();
            count += 1;
        }
    }
    if !active {
        feedback.clear();
        effects.flash = None;
        effects.mining = None;
        status.particles = 0;
        return;
    }
    let mining_step = world
        .mining
        .target
        .filter(|_| world.input.captured && !world.blocked.0)
        .map(|target| (target, super::fracture::stage(world.mining.progress)));
    if effects.mining != mining_step
        && let Some((MiningTarget::Block { position, expected }, stage)) = mining_step
        && stage > 1
        && !is_model(&world, expected)
    {
        let faces = super::fracture::exposed_faces(&world.chunks, position, expected);
        let exposed: Vec<_> = (0..6).filter(|side| faces & (1 << side) != 0).collect();
        if !exposed.is_empty() {
            for _ in 0..2.min(MAX_PARTICLES - count) {
                effects.serial = effects.serial.wrapping_add(1);
                let side = exposed[effects.serial as usize % exposed.len()];
                let mut normal = Vec3::ZERO;
                normal[side / 2] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
                let angle = effects.serial as f32 * 2.399_963_1;
                let mut offset = normal * 0.55;
                offset[(side / 2 + 1) % 3] = angle.cos() * 0.24;
                offset[(side / 2 + 2) % 3] = angle.sin() * 0.24;
                let center = Vec3::from_array(position.map(|v| v as f32 + 0.5));
                let size = Vec3::splat(0.035 + stage as f32 * 0.004);
                commands.spawn((
                    BreakParticle {
                        velocity: normal * (0.5 + stage as f32 * 0.03) + Vec3::Y * 0.5,
                        life: LIFETIME,
                        size,
                    },
                    Mesh3d(world.mesh(ItemKind::Block(expected))),
                    MeshMaterial3d(world.material(ItemKind::Block(expected))),
                    Transform::from_translation(center + offset)
                        .with_rotation(Quat::from_rotation_y(angle))
                        .with_scale(size),
                    NotShadowCaster,
                ));
                count += 1;
            }
        }
    }
    if effects.mining != mining_step
        && let Some((MiningTarget::Block { expected, .. }, stage)) = mining_step
        && stage > 1
        && is_model(&world, expected)
        && let (Some(journal), Some(properties), Some((camera, look))) = (
            scene.journal.as_ref(),
            scene.properties.as_ref(),
            scene.cameras.iter().next(),
        )
    {
        let direction = look.rotation() * Vec3::NEG_Z;
        if let Some(hit) = terra_world::interaction_shapes::raycast(
            &world.chunks,
            journal,
            properties,
            camera.translation.to_array().map(f64::from),
            direction.to_array().map(f64::from),
            8.0,
        ) && world.chunks.block_at(hit.position) == Some(expected)
        {
            let normal = Vec3::from_array(hit.normal.map(|v| v as f32));
            let point = camera.translation + direction * hit.distance as f32;
            for _ in 0..2.min(MAX_PARTICLES - count) {
                effects.serial = effects.serial.wrapping_add(1);
                let angle = effects.serial as f32 * 2.399_963_1;
                let size = Vec3::splat(0.025 + (effects.serial % 4) as f32 * 0.008);
                commands.spawn((
                    BreakParticle {
                        velocity: normal * 1.2
                            + Vec3::new(angle.cos() * 0.3, 0.6, angle.sin() * 0.3),
                        life: LIFETIME,
                        size,
                    },
                    Mesh3d(world.assets.cube.clone()),
                    MeshMaterial3d(
                        world.material(ItemKind::Block(
                            world
                                .mining
                                .slab_half()
                                .map_or(expected, |half| half.removed),
                        )),
                    ),
                    Transform::from_translation(point + normal * 0.025).with_scale(size),
                    NotShadowCaster,
                ));
                count += 1;
            }
        }
    }
    effects.mining = mining_step;
    for event in feedback
        .read()
        .filter(|e| e.outcome == EditOutcome::Applied)
    {
        let (position, item) = match event.action {
            EditAction::Block {
                position,
                material,
                expected,
            } => {
                let p = Vec3::from_array(position.map(|v| v as f32 + 0.5));
                if material != 0 {
                    effects.flash = Some((p, 0.20));
                    continue;
                }
                // Furniture loses its real imported surface pieces through the
                // asynchronous isolation path, never miniature full-cube debris.
                if is_model(&world, expected) {
                    continue;
                }
                (p, ItemKind::Block(expected))
            }
            EditAction::Prop { section, id } => {
                let Some(prop) = world
                    .chunks
                    .loaded
                    .get(&section)
                    .and_then(|s| s.props.iter().find(|p| p.id == id))
                else {
                    continue;
                };
                let kind: ResourcePropKind = prop.kind.into();
                (
                    Vec3::from_array(prop.position.map(|v| v as f32)) + Vec3::Y * 0.15,
                    ItemKind::Prop(kind),
                )
            }
            _ => continue,
        };
        for index in 0..12.min(MAX_PARTICLES - count) {
            effects.serial = effects.serial.wrapping_add(1);
            let angle = effects.serial as f32 * 2.399_963_1;
            let spread = Vec3::new(angle.cos(), 0.4 + index as f32 * 0.09, angle.sin());
            let size = Vec3::splat(0.045 + (index % 4) as f32 * 0.012);
            commands.spawn((
                BreakParticle {
                    velocity: spread * 1.6,
                    life: LIFETIME,
                    size,
                },
                Mesh3d(world.mesh(item)),
                MeshMaterial3d(world.material(item)),
                Transform::from_translation(position + spread * 0.12)
                    .with_rotation(Quat::from_euler(
                        EulerRot::XYZ,
                        angle * 0.3,
                        angle,
                        angle * 0.6,
                    ))
                    .with_scale(size),
                NotShadowCaster,
            ));
            count += 1;
        }
    }
    if let Some((position, remaining)) = &mut effects.flash {
        *remaining -= delta;
        if *remaining > 0.0 {
            gizmos.cube(
                Transform::from_translation(*position)
                    .with_scale(Vec3::splat(1.01 + *remaining * 0.12)),
                Color::srgba(0.78, 0.87, 0.47, *remaining * 4.0),
            );
        } else {
            effects.flash = None;
        }
    }
    status.particles = count;
}

fn is_model(world: &BuildingWorld, material: u16) -> bool {
    world
        .blocks
        .templates
        .as_ref()
        .and_then(|templates| templates.blocks.get(&material))
        .is_some_and(|template| !template.model.is_empty())
}
