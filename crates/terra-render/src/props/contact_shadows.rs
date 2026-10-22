//! Short, soft vegetation contact shadows instead of long moving solar silhouettes.
use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};
#[derive(Resource, Default)]
struct State {
    mesh: Handle<Mesh>,
    next: f32,
    dirty: bool,
    focus: Option<Vec3>,
}
#[derive(Component)]
struct Batch;
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(Startup, setup)
        .add_systems(Update, update);
}
fn setup(
    mut commands: Commands,
    mut s: ResMut<State>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    s.mesh = meshes.add(Rectangle::new(0.01, 0.01));
    commands.spawn((
        Name::new("Soft vegetation ground contacts"),
        Batch,
        Mesh3d(s.mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            depth_bias: 1.0,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
        bevy::camera::visibility::NoFrustumCulling,
    ));
}
#[allow(clippy::too_many_arguments)]
fn update(
    time: Res<Time>,
    access: Res<WorldAccess>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    bodies: Query<&Transform, With<terra_player::PlayerBody>>,
    mut s: ResMut<State>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut batch: Query<&mut Visibility, With<Batch>>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("props.contact_shadows");
    s.dirty |= chunks.is_changed() || journal.is_changed();
    if time.elapsed_secs() < s.next {
        return;
    }
    s.next = time.elapsed_secs() + 0.25;
    let Ok(mut visible) = batch.single_mut() else {
        return;
    };
    let Some(body) = bodies.iter().next().filter(|_| access.ready()) else {
        *visible = Visibility::Hidden;
        s.focus = None;
        return;
    };
    let focus = body.translation;
    if !s.dirty
        && s.focus
            .is_some_and(|old| old.distance_squared(focus) < 0.25)
    {
        return;
    }
    s.dirty = false;
    s.focus = Some(focus);
    let mut props: Vec<_> = chunks
        .loaded
        .values()
        .filter(|s| {
            s.data.position().origin().is_ok_and(|p| {
                (Vec3::from_array(p.map(|v| v as f32)) + Vec3::splat(8.0)).distance_squared(focus)
                    < 48.0 * 48.0
            })
        })
        .flat_map(|s| &s.props)
        .filter(|p| {
            !p.removed
                && !p.hidden_growth()
                && !p.wall_attached()
                && !journal.prop_removed(p.section, p.id)
        })
        .filter(|p| {
            Vec3::from_array(p.position.map(|v| v as f32)).distance_squared(focus) < 32.0 * 32.0
        })
        .collect();
    props.sort_by(|a, b| {
        let d = |p: &terra_world::props::WorldProp| {
            Vec3::from_array(p.position.map(|v| v as f32)).distance_squared(focus)
        };
        d(a).total_cmp(&d(b))
    });
    props.truncate(768);
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    for p in props {
        let support = terra_world::props::placement::support(p);
        if !chunks
            .block_at(support)
            .map(|m| journal.block(support).unwrap_or(m))
            .and_then(terra_voxel::MaterialId::from_raw)
            .is_some_and(terra_voxel::MaterialId::is_natural_ground)
        {
            continue;
        }
        let center = Vec3::new(
            p.position[0] as f32,
            (support[1] + 1) as f32 + 0.003,
            p.position[2] as f32,
        );
        let radius = if p.kind == terra_world::props::PropKind::Bush {
            0.29
        } else {
            0.15
        } * p.visual_scale();
        let base = positions.len() as u32;
        let alpha = 0.12 * (1.0 - center.distance(focus) / 32.0).clamp(0.0, 1.0);
        positions.push(center.to_array());
        colors.push([0.09, 0.12, 0.08, alpha]);
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::TAU / 8.0;
            positions
                .push((center + Vec3::new(a.cos() * radius, 0.0, a.sin() * radius)).to_array());
            colors.push([0.09, 0.12, 0.08, 0.0]);
            indices.extend([base, base + 1 + i, base + 1 + (i + 1) % 8]);
        }
    }
    if positions.is_empty() {
        *visible = Visibility::Hidden;
        return;
    }
    let count = positions.len();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count]);
    mesh.insert_indices(Indices::U32(indices));
    if let Some(mut m) = meshes.get_mut(&s.mesh) {
        *m = mesh;
    }
    *visible = Visibility::Visible;
}
