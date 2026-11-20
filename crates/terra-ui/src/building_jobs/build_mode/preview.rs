//! One cached translucent surface mesh, never an entity or allocation per cell/frame.
use super::*;
use bevy::{asset::RenderAssetUsages, mesh::Indices, render::render_resource::PrimitiveTopology};
use std::collections::BTreeSet;

#[derive(Component)]
struct Ghost;
#[derive(Resource, Default)]
struct Geometry {
    source: Option<TemplatePreview>,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Geometry>()
        .add_systems(Update, draw.after(BuildModeSet));
}

pub(super) fn mesh(source: &TemplatePreview) -> Mesh {
    let occupied: BTreeSet<_> = source
        .cells
        .iter()
        .map(|cell| cell.position.map(i32::from))
        .collect();
    let sides = [
        ([1, 0, 0], [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]]),
        ([-1, 0, 0], [[0, 0, 1], [0, 1, 1], [0, 1, 0], [0, 0, 0]]),
        ([0, 1, 0], [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]]),
        ([0, -1, 0], [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]]),
        ([0, 0, 1], [[1, 0, 1], [1, 1, 1], [0, 1, 1], [0, 0, 1]]),
        ([0, 0, -1], [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]]),
    ];
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for cell in &occupied {
        for (normal, corners) in &sides {
            let neighbor = std::array::from_fn(|i| cell[i] + normal[i]);
            if occupied.contains(&neighbor) {
                continue;
            }
            let offset = positions.len() as u32;
            for corner in corners {
                positions.push(std::array::from_fn::<_, 3, _>(|i| {
                    (cell[i] + corner[i]) as f32
                }));
                normals.push(normal.map(|v| v as f32));
            }
            uvs.extend([[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
            indices.extend([
                offset,
                offset + 1,
                offset + 2,
                offset,
                offset + 2,
                offset + 3,
            ]);
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    camera: Res<ConstructionCamera>,
    state: Res<State>,
    view: Res<BuildModeView>,
    mut geometry: ResMut<Geometry>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut ghosts: Query<(Entity, &mut Transform, &mut Visibility), With<Ghost>>,
) {
    let show = camera.active
        && !view.template_name.is_empty()
        && view.visible
        && state.placing
        && state.origin.is_some()
        && state
            .preview
            .as_ref()
            .is_some_and(|preview| preview.id == state.template);
    if !show {
        for (_, _, mut visibility) in &mut ghosts {
            *visibility = Visibility::Hidden;
        }
        return;
    }
    let (Some(meshes), Some(materials), Some(source), Some(origin)) = (
        meshes.as_mut(),
        materials.as_mut(),
        state.preview.as_ref(),
        state.origin,
    ) else {
        return;
    };
    let changed = geometry.source.as_ref() != Some(source);
    if changed {
        let next = meshes.add(mesh(source));
        if geometry.mesh != Handle::default() {
            meshes.remove(geometry.mesh.id());
        }
        geometry.mesh = next;
        geometry.source = Some(source.clone());
    }
    if geometry.material == Handle::default() {
        geometry.material = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.42, 0.05, 0.48),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
    }
    let pose = placement_pose(origin, state.facing);
    if ghosts.is_empty() {
        commands.spawn((
            Ghost,
            Name::new("Construction blueprint preview"),
            Mesh3d(geometry.mesh.clone()),
            MeshMaterial3d(geometry.material.clone()),
            pose,
            Visibility::Visible,
        ));
    } else {
        for (entity, mut transform, mut visibility) in &mut ghosts {
            *transform = pose;
            *visibility = Visibility::Visible;
            if changed {
                commands
                    .entity(entity)
                    .insert(Mesh3d(geometry.mesh.clone()));
            }
        }
    }
}

pub(super) fn placement_pose(origin: [i64; 3], facing: u8) -> Transform {
    let rotation = Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2);
    let pivot = Vec3::new(0.5, 0.0, 0.5);
    Transform::from_translation(
        Vec3::from_array(origin.map(|v| v as f32)) + pivot - rotation * pivot,
    )
    .with_rotation(rotation)
}
