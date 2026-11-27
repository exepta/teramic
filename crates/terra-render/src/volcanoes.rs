//! The front of the plume rises before the first stones clear the crater rim.
use crate::furnaces::{Geometry, SmokeExtension, SmokeMaterial};
use bevy::{light::NotShadowCaster, pbr::ExtendedMaterial, prelude::*};
use terra_network::client::ClientSession;
use terra_world::volcano::{DURATION, EJECTA_COUNT, Eruption};

const PUFFS: u8 = 60;

#[derive(Component, Clone, Copy)]
enum Particle {
    Ash(u8),
    Bomb(u8),
    Trail(u8),
}

#[derive(Resource)]
struct Palette {
    cloud: Handle<Mesh>,
    bomb: Handle<Mesh>,
    ash: Handle<SmokeMaterial>,
    rock: Handle<StandardMaterial>,
    melt: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct Inspection(Eruption);

pub(crate) fn install(app: &mut App) {
    // Reproducible native screenshots of the real generated vent. Multiplayer
    // sessions always consume the authoritative event and its impact mask.
    if std::env::args().any(|arg| arg == "--smoke-test-benchmark")
        && let Ok(value) = std::env::var("TERRA_BENCH_ERUPTION_SECONDS")
        && let Ok(age) = value.parse::<f32>()
        && (0. ..DURATION).contains(&age)
        && let Ok(eye) = std::env::var("TERRA_BENCH_CAMERA")
    {
        let coords: Vec<_> = eye
            .split(',')
            .map(str::parse::<f64>)
            .collect::<Result<_, _>>()
            .unwrap_or_default();
        if let [x, _, z] = coords.as_slice() {
            let (x, z) = (*x, *z);
            app.add_systems(
                PostStartup,
                move |mut commands: Commands,
                      terrain: Res<terra_world::streaming::WorldTerrain>| {
                    if let Some(vent) = terrain.0.eruptive_vent(x, z) {
                        tracing::info!(?vent.origin,age,"Native eruption inspection");
                        commands.insert_resource(Inspection(Eruption {
                            vent,
                            cycle: 1,
                            age,
                            landed: 0,
                        }));
                    }
                },
            );
        }
    }
    app.add_systems(Startup, setup).add_systems(Update, animate);
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut smoke: ResMut<Assets<SmokeMaterial>>,
) {
    // Overlapping turbulent, soft edged lobes. The lit cap and soot-heavy
    // underside are drawn together and always share a billow's movement.
    let mut shape = Geometry::default();
    for (offset, radius, color) in [
        ((0., -0.1), 1.32, [0.047, 0.039, 0.043, 1.]),
        ((-0.33, 0.16), 0.94, [0.086, 0.071, 0.075, 0.93]),
        ((0.38, 0.19), 0.83, [0.114, 0.094, 0.093, 0.80]),
        ((0.04, 0.53), 0.75, [0.146, 0.120, 0.116, 0.70]),
    ] {
        shape.wisp(
            Vec3::new(offset.0, offset.1, 0.),
            radius,
            Vec3::X,
            Vec3::Y,
            color,
        );
    }
    let mut cloud = crate::furnaces::mesh();
    shape.write(&mut cloud);
    let ash = smoke.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            ..default()
        },
        extension: SmokeExtension {
            animation: Vec4::ZERO,
        },
    });
    commands.insert_resource(Palette {
        cloud: meshes.add(cloud),
        bomb: meshes.add(
            Sphere::new(0.73)
                .mesh()
                .ico(0)
                .expect("icosahedral volcanic bomb"),
        ),
        ash,
        rock: materials.add(StandardMaterial {
            base_color: Color::srgb(0.17, 0.115, 0.108),
            perceptual_roughness: 0.9,
            ..default()
        }),
        melt: materials.add(StandardMaterial {
            base_color: Color::srgb(1., 0.27, 0.025),
            emissive: LinearRgba::rgb(5.2, 0.72, 0.035),
            ..default()
        }),
    });
}

fn puff(event: Eruption, id: u8, age: f32) -> Option<Transform> {
    let i = f32::from(id);
    let launched = i / 4.1;
    let t = age - launched;
    if !(0.0..12.).contains(&t) {
        return None;
    }
    let angle = i * 2.39996 + (event.vent.seed & 255) as f32 * 0.037;
    let outer = 0.36 + 0.64 * (i * 0.731).sin().abs();
    let spread = (3. + t * t * 0.17) * outer;
    let drift = t * t * 0.11;
    let rise = 12.2 * t - 0.21 * t * t;
    let gain = (t * 1.9).min(1.) * ((12. - t) / 3.).clamp(0., 1.);
    let radius = (2.6 + t * 0.72) * (0.88 + (i * 0.47).sin() * 0.17) * gain;
    let center = Vec3::from_array(event.vent.origin.map(|v| v as f32));
    Some(
        Transform::from_translation(
            center
                + Vec3::new(
                    drift + angle.cos() * spread,
                    2. + rise + (angle + 0.6 * t).sin() * 1.1,
                    angle.sin() * spread + 0.45 * t,
                ),
        )
        .with_scale(Vec3::new(radius * 1.17, radius, radius * 1.12)),
    )
}

fn flight(event: Eruption, id: u8, age: f32, trail: bool) -> Option<Transform> {
    if event.landed & (1 << id) != 0 {
        return None;
    }
    let bomb = event.fragment(id);
    if trail && !bomb.molten {
        return None;
    }
    let current = bomb.position(age)?;
    let position = Vec3::from_array(current.map(|v| v as f32));
    let size = 0.66 + f32::from(id % 5) * 0.21;
    if trail {
        let tail = Vec3::from_array(bomb.position(age - 0.105)?.map(|v| v as f32));
        let axis = position - tail;
        return Some(
            Transform::from_translation((position + tail) * 0.5)
                .with_scale(Vec3::new(size * 0.45, axis.length().max(0.01), size * 0.45))
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, axis.normalize_or(Vec3::Y))),
        );
    }
    Some(
        Transform::from_translation(position)
            .with_rotation(
                Quat::from_rotation_y(age * 2.8 + f32::from(id)) * Quat::from_rotation_x(age * 1.9),
            )
            .with_scale(Vec3::new(size * 0.87, size * 1.17, size)),
    )
}

type PlumeParticles<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Particle,
        &'static mut Transform,
        &'static mut Visibility,
        Option<&'static mut MeshMaterial3d<StandardMaterial>>,
    ),
>;

fn animate(
    mut commands: Commands,
    session: Option<Res<ClientSession>>,
    palette: Res<Palette>,
    inspection: Option<Res<Inspection>>,
    camera: Query<&Transform, (With<terra_player::TerraCamera>, Without<Particle>)>,
    mut particles: PlumeParticles<'_, '_>,
) {
    let event = session.as_deref().and_then(ClientSession::current_eruption);
    let event = if session.is_none() {
        inspection.map(|f| f.0)
    } else {
        event
    };
    let Some(event) = event else {
        for (entity, ..) in &mut particles {
            commands.entity(entity).despawn();
        }
        return;
    };
    if particles.is_empty() {
        for particle in (0..PUFFS)
            .map(Particle::Ash)
            .chain((0..EJECTA_COUNT).map(Particle::Bomb))
            .chain((0..EJECTA_COUNT).map(Particle::Trail))
        {
            let mut entity = commands.spawn((
                particle,
                Transform::default(),
                Visibility::Hidden,
                NotShadowCaster,
            ));
            match particle {
                Particle::Ash(_) => {
                    entity.insert((
                        Mesh3d(palette.cloud.clone()),
                        MeshMaterial3d(palette.ash.clone()),
                    ));
                }
                Particle::Bomb(_) | Particle::Trail(_) => {
                    let handle = if matches!(particle, Particle::Bomb(_)) {
                        &palette.rock
                    } else {
                        &palette.melt
                    };
                    entity.insert((Mesh3d(palette.bomb.clone()), MeshMaterial3d(handle.clone())));
                }
            }
        }
        return;
    }
    let facing = camera.iter().next().map_or(Quat::IDENTITY, |t| t.rotation);
    for (_, particle, mut transform, mut visible, material) in &mut particles {
        let pose = match *particle {
            Particle::Ash(id) => puff(event, id, event.age).map(|mut pose| {
                pose.rotation = facing
                    * Quat::from_rotation_z((f32::from(id) * 1.37 + event.age * 0.1).sin() * 0.29);
                pose
            }),
            Particle::Bomb(id) => {
                let handle = if event.fragment(id).molten {
                    &palette.melt
                } else {
                    &palette.rock
                };
                if let Some(mut material) = material
                    && material.0 != *handle
                {
                    material.0 = handle.clone();
                }
                flight(event, id, event.age, false)
            }
            Particle::Trail(id) => flight(event, id, event.age, true),
        };
        if let Some(pose) = pose {
            *transform = pose;
            *visible = Visibility::Inherited;
        } else {
            *visible = Visibility::Hidden;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use terra_world::volcano::Vent;

    #[test]
    fn native_eruption_scene_rises_above_the_rim_and_retires_reported_impacts() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<SmokeMaterial>>()
            .init_resource::<ClientSession>();
        install(&mut app);
        let event = Eruption {
            vent: Vent {
                origin: [-2243.5, 100., -4010.5],
                seed: 3586461654,
            },
            cycle: 1,
            age: 9.,
            landed: 0,
        };
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.ready = true;
            session.eruption = Some((event, Instant::now()));
        }
        app.update();
        app.update();
        let world = app.world_mut();
        let mut seen_ash = 0;
        let mut burning = 0;
        let mut stone = 0;
        let mut highest = 0_f32;
        for (part, pose, vis) in world
            .query::<(&Particle, &Transform, &Visibility)>()
            .iter(world)
        {
            if *vis == Visibility::Hidden {
                continue;
            }
            match *part {
                Particle::Ash(_) => {
                    seen_ash += 1;
                    highest = highest.max(pose.translation.y);
                }
                Particle::Bomb(n) => {
                    if event.fragment(n).molten {
                        burning += 1;
                    } else {
                        stone += 1;
                    }
                }
                Particle::Trail(_) => {}
            }
        }
        assert!(seen_ash >= 25 && burning >= 2 && stone >= 2);
        assert!(highest > 180., "the cloud needs to clear the 110-block rim");
        app.world_mut().resource_mut::<ClientSession>().eruption = Some((
            Eruption {
                landed: u64::MAX,
                ..event
            },
            Instant::now(),
        ));
        app.update();
        let world = app.world_mut();
        assert!(
            world
                .query::<(&Particle, &Visibility)>()
                .iter(world)
                .all(|(part, v)| matches!(part, Particle::Ash(_)) || *v == Visibility::Hidden)
        );
        app.world_mut().resource_mut::<ClientSession>().eruption = None;
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&Particle>().iter(world).count(), 0);
    }
}
