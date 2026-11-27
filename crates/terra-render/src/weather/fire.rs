//! Layered flame sheets follow exposed fuel faces. Their internal motion runs
//! continuously on the GPU even when the authority updates heat slowly.
use crate::furnaces::{Geometry, mesh};
use bevy::{
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use terra_player::TerraCamera;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
struct FlameExtension {
    #[uniform(100)]
    wind: Vec4,
}
impl MaterialExtension for FlameExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_wildfire.wesl".into()
    }
}
type FlameMaterial = ExtendedMaterial<StandardMaterial, FlameExtension>;

#[derive(Resource)]
struct Flames {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<FlameMaterial>,
    next: f32,
    visible: bool,
}

#[derive(Component)]
struct BurningView;

type FlameCamera = (With<TerraCamera>, Without<BurningView>);

pub(super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<FlameMaterial>::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (update, burning_view).after(terra_player::PlayerSet::Movement),
        );
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<FlameMaterial>>,
) {
    let fuel_mesh = meshes.add(mesh());
    let material = mats.add(ExtendedMaterial {
        base: StandardMaterial {
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        },
        extension: FlameExtension { wind: Vec4::ZERO },
    });
    let entity = commands
        .spawn((
            Name::new("Burning fuel"),
            Mesh3d(fuel_mesh.clone()),
            MeshMaterial3d(material.clone()),
            Visibility::Hidden,
            Transform::default(),
            NotShadowCaster,
            NoFrustumCulling,
        ))
        .id();
    // Camera-local wisps around the lower edge leave the sights and HUD legible.
    let mut embers = Geometry::default();
    for (x, seed) in [(-0.9, 0.13), (-0.56, 0.55), (0.60, 0.38), (0.92, 0.87)] {
        embers.wisp(
            Vec3::new(x, -0.85, -1.),
            1.,
            Vec3::X * 0.18,
            Vec3::Y * 0.25,
            [seed, 0.70, 1., 1.],
        );
    }
    let mut edge_mesh = mesh();
    embers.write(&mut edge_mesh);
    commands.spawn((
        BurningView,
        Name::new("Burning player view"),
        Mesh3d(meshes.add(edge_mesh)),
        MeshMaterial3d(material.clone()),
        Transform::default(),
        Visibility::Hidden,
        NoFrustumCulling,
        NotShadowCaster,
    ));
    commands.insert_resource(Flames {
        entity,
        mesh: fuel_mesh,
        material,
        next: 0.,
        visible: false,
    });
}

fn burning_view(
    session: Option<Res<terra_network::client::ClientSession>>,
    perspective: Option<Res<terra_player::CameraPerspective>>,
    cameras: Query<(&Transform, &Projection), FlameCamera>,
    mut view: Query<(&mut Transform, &mut Visibility), With<BurningView>>,
) {
    let Ok((mut t, mut visibility)) = view.single_mut() else {
        return;
    };
    let burning = session.as_ref().is_some_and(|s| {
        s.ready
            && !s.stats.survival.dead()
            && s.stats
                .survival
                .effects
                .contains(terra_core::EffectKind::OnFire)
    });
    if burning
        && !perspective.is_some_and(|p| p.is_third_person())
        && let Ok((camera, Projection::Perspective(p))) = cameras.single()
    {
        let depth = (p.near * 1.5).max(0.20);
        let half_height = depth * (p.fov * 0.5).tan();
        *t = camera.with_scale(Vec3::new(half_height * p.aspect_ratio, half_height, depth));
        *visibility = Visibility::Visible;
    } else if *visibility != Visibility::Hidden {
        *visibility = Visibility::Hidden;
    }
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut commands: Commands,
    time: Res<Time>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    cameras: Query<&Transform, With<TerraCamera>>,
    weather: Option<Res<super::WeatherVisual>>,
    players: Option<Res<terra_network::client::ReplicatedPlayers>>,
    session: Option<Res<terra_network::client::ClientSession>>,
    perspective: Option<Res<terra_player::CameraPerspective>>,
    body: Query<&Transform, With<terra_player::PlayerBody>>,
    mut flames: ResMut<Flames>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<FlameMaterial>>,
) {
    if time.elapsed_secs() < flames.next {
        return;
    }
    flames.next = time.elapsed_secs() + 0.08;
    let mut geometry = Geometry::default();
    if let Some(camera) = cameras.iter().next() {
        let mut coat = |center: Vec3| {
            if center.distance_squared(camera.translation) > 72.0_f32.powi(2) {
                return;
            }
            for (offset, seed) in [(-0.21, 0.30), (0.23, 0.85), (0., 0.56)] {
                geometry.wisp(
                    center + *camera.right() * offset - Vec3::Y * offset.abs()
                        + *camera.back() * 0.29,
                    1.,
                    *camera.right() * 0.25,
                    Vec3::Y * (0.55 + seed * 0.21),
                    [seed, 0.86, 1., 1.],
                );
            }
        };
        if let Some(s) = session.as_ref().filter(|s| s.ready) {
            if perspective.is_some_and(|p| p.is_third_person())
                && s.stats
                    .survival
                    .effects
                    .contains(terra_core::EffectKind::OnFire)
                && !s.stats.survival.dead()
                && let Ok(body) = body.single()
            {
                coat(body.translation);
            }
            if let Some(players) = players.as_ref() {
                for (id, track) in &players.players {
                    if Some(*id) != s.player
                        && track
                            .current
                            .survival
                            .effects
                            .contains(terra_core::EffectKind::OnFire)
                        && !track.current.survival.dead()
                    {
                        coat(track.position());
                    }
                }
            }
        }
        let mut nearest: Vec<_> = journal
            .active_fires()
            .filter_map(|(p, state)| {
                let anchor = Vec3::from_array(p.map(|v| v as f32)) + Vec3::splat(0.5);
                let distance = anchor.distance_squared(camera.translation);
                (distance < 64. * 64. && chunks.block_at(p).is_some())
                    .then_some((distance, p, anchor, state.fire))
            })
            .collect();
        nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
        nearest.truncate(80);
        let wind = super::foliage_wind(weather.as_deref());
        if !nearest.is_empty()
            && let Some(mut mat) = mats.get_mut(&flames.material)
        {
            let sidewind = Vec3::new(wind.x, 0., wind.y).dot(*camera.right()) * wind.z;
            if mat.extension.wind.x != sidewind {
                mat.extension.wind.x = sidewind;
            }
        }
        for (_, p, anchor, intensity) in nearest.into_iter().rev() {
            let open =
                |d: [i64; 3]| chunks.block_at(std::array::from_fn(|i| p[i] + d[i])) == Some(0);
            let top = open([0, 1, 0]);
            let phase = ((p[0].wrapping_mul(29) ^ p[1].wrapping_mul(7) ^ p[2].wrapping_mul(137))
                .rem_euclid(1171) as f32)
                / 1171.;
            for (i, dir) in [[-1, 0, 0], [0, 0, 1], [1, 0, 0], [0, 0, -1]]
                .into_iter()
                .enumerate()
            {
                if !open(dir) {
                    continue;
                }
                // Keep the fire attached to the entire exposed face of its fuel.
                // A sideways view then sees one continuous burning edge.
                let normal = Vec3::from_array(dir.map(|v| v as f32));
                let root = anchor + normal * 0.516 - Vec3::Y * 0.48;
                let seed = (phase + i as f32 * 0.271).fract();
                let half_height = 0.50 + intensity * (0.18 + 0.12 * seed);
                geometry.wisp(
                    root + Vec3::Y * half_height,
                    1.,
                    normal.cross(Vec3::Y) * 0.50,
                    Vec3::Y * half_height,
                    [seed, intensity, 0., 1.],
                );
            }
            if top {
                let rise = Vec3::Y.lerp(*camera.up(), 0.18);
                let half_height = 0.45 + intensity * (0.52 + phase * 0.22);
                geometry.wisp(
                    anchor + Vec3::Y * 0.48 + rise * half_height,
                    1.,
                    *camera.right() * (0.50 + 0.27 * intensity),
                    rise * half_height,
                    [phase, intensity, 1., 1.],
                );
            }
        }
    }
    let visible = geometry.upload(&mut meshes, &flames.mesh);
    if visible != flames.visible {
        commands.entity(flames.entity).insert(if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
        flames.visible = visible;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_network::client::ClientSession;
    use terra_player::CameraPerspective;

    #[test]
    fn burning_view_tracks_first_person_camera_until_the_status_expires() {
        let mut app = App::new();
        app.init_resource::<ClientSession>()
            .init_resource::<CameraPerspective>()
            .add_systems(Update, burning_view);
        let camera = Transform::from_xyz(28., 65., 11.).with_rotation(Quat::from_rotation_y(0.82));
        app.world_mut().spawn((
            TerraCamera,
            camera,
            Projection::Perspective(PerspectiveProjection {
                aspect_ratio: 2.,
                ..default()
            }),
        ));
        let view = app
            .world_mut()
            .spawn((BurningView, Transform::IDENTITY, Visibility::Hidden))
            .id();
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.ready = true;
            session
                .stats
                .survival
                .effects
                .apply(terra_core::EffectKind::OnFire, 5., 1);
        }
        app.update();
        let transform = app.world().get::<Transform>(view).unwrap();
        assert_eq!(transform.translation, camera.translation);
        assert_eq!(transform.rotation, camera.rotation);
        assert!((transform.scale.x / transform.scale.y - 2.).abs() < 0.001);
        assert_eq!(
            *app.world().get::<Visibility>(view).unwrap(),
            Visibility::Visible
        );
        app.insert_resource(CameraPerspective::ThirdPerson);
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(view).unwrap(),
            Visibility::Hidden
        );
        app.insert_resource(CameraPerspective::FirstPerson);
        for _ in 0..20 {
            app.world_mut()
                .resource_mut::<ClientSession>()
                .stats
                .survival
                .advance_life(false, 0.25);
        }
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(view).unwrap(),
            Visibility::Hidden
        );
    }
}
