//! One continuous conveyor ribbon and two spring gates per visible filling station.
use crate::{blocks::BlockAssets, meshing::builder::MeshBatch};
use bevy::{ecs::system::SystemParam, prelude::*};
use std::collections::{HashMap, HashSet};
use terra_core::infusion::{HOLD_BLOCK, HOLD_TOP, HoldPhase, HoldVisual};
use terra_network::client::ClientSession;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

type Cell = [i64; 3];
const MAX_VISIBLE: usize = 64;
const HALF_WIDTH: f32 = 0.32;

#[derive(Resource, Default, Debug)]
pub struct InfusionRenderStatus {
    pub surfaces: usize,
    pub raised_gates: usize,
}

struct Station {
    root: Entity,
    ribbon: Entity,
    gates: [Entity; 2],
    amounts: [f32; 2],
    material: Handle<StandardMaterial>,
    offset: f32,
    key: (u8, u8),
}

#[derive(Resource, Default)]
struct State {
    stations: HashMap<Cell, Station>,
    ribbons: HashMap<u8, Handle<Mesh>>,
    gate_mesh: Option<Handle<Mesh>>,
    gate_material: Option<Handle<StandardMaterial>>,
}

#[derive(SystemParam)]
struct Scene<'w> {
    time: Res<'w, Time>,
    blocks: Res<'w, BlockAssets>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    session: Option<Res<'w, ClientSession>>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<InfusionRenderStatus>()
        .add_systems(Update, update);
}

fn ends(view: HoldVisual) -> u8 {
    u8::from(view.connections & (1 << ((view.facing + 2) % 4)) != 0)
        | (u8::from(view.connections & (1 << view.facing) != 0) << 1)
}

fn ribbon_geometry(mask: u8) -> MeshBatch {
    let near = if mask & 1 != 0 { -1.0 } else { -0.5 };
    let far = if mask & 2 != 0 { 1.0 } else { 0.5 };
    // Reach the adjacent roller center exactly. The roller's top then covers
    // the same return arc as the existing machine/inventory transfer tongues.
    let minimum = Vec3::new(-HALF_WIDTH, HOLD_TOP as f32 - 0.02, near);
    let maximum = Vec3::new(HALF_WIDTH, HOLD_TOP as f32, far);
    let mut batch = MeshBatch::default();
    for side in 0..6 {
        let axis = side / 2;
        let positive = side % 2 == 1;
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let mut normal = Vec3::ZERO;
        normal[axis] = if positive { 1.0 } else { -1.0 };
        let mut points: [Vec3; 4] = std::array::from_fn(|i| {
            let mut p = minimum;
            p[axis] = if positive {
                maximum[axis]
            } else {
                minimum[axis]
            };
            p[u] = if i == 1 || i == 2 {
                maximum[u]
            } else {
                minimum[u]
            };
            p[v] = if i >= 2 { maximum[v] } else { minimum[v] };
            p
        });
        if !positive {
            points.reverse();
        }
        let base = batch.positions.len() as u32;
        let tangent =
            crate::blocks::tangent_frame(Vec3::X, normal, if side == 3 { -1.0 } else { 1.0 });
        batch.positions.extend(points.map(|p| p.to_array()));
        batch.normals.extend([normal.to_array(); 4]);
        batch.tangents.extend([tangent; 4]);
        batch.uvs.extend(points.map(|p| [p.x + HALF_WIDTH, p.z]));
        let shade = if side == 3 { 1.0 } else { 0.58 };
        batch.colors.extend([[shade, shade, shade, 1.0]; 4]);
        batch
            .indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    batch
}

fn gate_mesh() -> Mesh {
    let mut builder = crate::players::model::Builder::default();
    builder
        .voxel([0., 0., 0.], [0.72, 0.045, 0.055], [0.83, 0.63, 0.28])
        .expect("bounded separator");
    for x in [-0.35, 0.35] {
        builder
            .voxel([x, -0.12, 0.], [0.04, 0.24, 0.065], [0.57, 0.63, 0.68])
            .expect("bounded separator post");
    }
    crate::previews::batch_mesh(&builder.finish_batch().expect("bounded gate mesh"))
}

fn gate_amount(current: f32, raised: bool, delta: f32) -> f32 {
    let step = delta.clamp(0.0, 0.05) / if raised { 0.18 } else { 0.045 };
    if raised {
        (current + step).min(1.0)
    } else {
        (current - step).max(0.0)
    }
}

fn gate_pose(end: usize, amount: f32) -> Transform {
    Transform::from_xyz(
        0.0,
        0.59 + amount * 0.24,
        if end == 0 { -0.49 } else { 0.49 },
    )
}

fn update(
    mut commands: Commands,
    scene: Scene,
    mut state: ResMut<State>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<&Transform, With<terra_player::TerraCamera>>,
    mut status: ResMut<InfusionRenderStatus>,
) {
    let ready = scene.session.as_deref().filter(|s| s.ready);
    let eye = cameras.iter().next().map_or(Vec3::ZERO, |t| t.translation);
    let mut views: Vec<_> = ready
        .into_iter()
        .flat_map(|s| &s.infusion)
        .copied()
        .filter(|v| scene.chunks.block_at(v.position) == Some(HOLD_BLOCK))
        .collect();
    if ready.is_some() && views.is_empty() && scene.journal.has_holds() {
        for &section in scene.chunks.loaded.keys() {
            for p in scene.journal.holds_in_section(section) {
                if let Some(view) = terra_world::conveyors::infusion_geometry(p, &scene.journal) {
                    views.push(view);
                }
            }
        }
    }
    let distance =
        |v: &HoldVisual| Vec3::from_array(v.position.map(|n| n as f32)).distance_squared(eye);
    views.retain(|v| v.valid() && distance(v) <= 96.0_f32.powi(2));
    views.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
    views.truncate(MAX_VISIBLE);
    let live: HashSet<_> = views.iter().map(|v| v.position).collect();
    state.stations.retain(|p, station| {
        if live.contains(p) {
            true
        } else {
            commands.entity(station.root).despawn();
            false
        }
    });
    status.surfaces = state.stations.len();
    status.raised_gates = 0;
    let Some(templates) = scene.blocks.templates.as_ref() else {
        return;
    };
    let source = templates
        .blocks
        .get(&terra_core::kinetic::CONVEYOR)
        .and_then(|block| {
            block
                .model
                .iter()
                .find(|p| templates.conveyor_ribbon_materials.contains(&p.material))
        })
        .and_then(|part| scene.blocks.materials.get(part.material))
        .and_then(|handle| materials.get(handle))
        .cloned();
    let Some(source) = source else {
        return;
    };
    if state.gate_mesh.is_none() {
        state.gate_mesh = Some(meshes.add(gate_mesh()));
        state.gate_material = Some(materials.add(StandardMaterial {
            metallic: 0.65,
            perceptual_roughness: 0.56,
            ..Default::default()
        }));
    }
    let gate_mesh = state.gate_mesh.clone().unwrap();
    let gate_material = state.gate_material.clone().unwrap();
    for view in views {
        let mask = ends(view);
        let mesh = state
            .ribbons
            .entry(mask)
            .or_insert_with(|| meshes.add(crate::previews::batch_mesh(&ribbon_geometry(mask))))
            .clone();
        let key = (view.facing, mask);
        let pose = Transform::from_translation(
            Vec3::from_array(view.position.map(|n| n as f32)) + Vec3::new(0.5, 0., 0.5),
        )
        .with_rotation(Quat::from_rotation_y(
            f32::from(view.facing) * std::f32::consts::FRAC_PI_2,
        ));
        if let std::collections::hash_map::Entry::Vacant(entry) =
            state.stations.entry(view.position)
        {
            let root = commands
                .spawn((
                    pose,
                    Visibility::default(),
                    Name::new("Hold conveyor and separators"),
                ))
                .id();
            let material = materials.add(source.clone());
            let ribbon = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    ChildOf(root),
                    crate::miniature::layers(),
                    terra_core::miniature::MiniatureMesh::Terrain,
                ))
                .id();
            let gates = std::array::from_fn(|end| {
                commands
                    .spawn((
                        Mesh3d(gate_mesh.clone()),
                        MeshMaterial3d(gate_material.clone()),
                        gate_pose(end, 0.),
                        ChildOf(root),
                        crate::miniature::layers(),
                        terra_core::miniature::MiniatureMesh::Terrain,
                    ))
                    .id()
            });
            entry.insert(Station {
                root,
                ribbon,
                gates,
                amounts: [0.; 2],
                material,
                offset: 0.,
                key,
            });
        }
        let station = state.stations.get_mut(&view.position).unwrap();
        if station.key != key {
            commands.entity(station.root).insert(pose);
            commands.entity(station.ribbon).insert(Mesh3d(mesh));
            station.key = key;
        }
        let speed = if matches!(view.phase, HoldPhase::Filling | HoldPhase::WaitingForFluid) {
            0.0
        } else {
            view.speed
        };
        if speed > 0.0 {
            station.offset =
                (station.offset - speed * scene.time.delta_secs().clamp(0., 0.1)).rem_euclid(1.0);
            if let Some(mut material) = materials.get_mut(&station.material) {
                material.uv_transform =
                    bevy::math::Affine2::from_translation(Vec2::new(0., station.offset));
            }
        }
        let inside = ready
            .and_then(|s| {
                s.drops
                    .iter()
                    .find(|drop| Some(drop.id) == view.active_drop)
            })
            .is_some_and(|drop| {
                let axis = terra_core::kinetic::forward(view.facing);
                (0..3)
                    .map(|i| {
                        (drop.position[i]
                            - view.position[i] as f64
                            - if i == 1 { 0.0 } else { 0.5 })
                            * axis[i] as f64
                    })
                    .sum::<f64>()
                    > -0.34
            });
        for end in 0..2 {
            let amount = gate_amount(
                station.amounts[end],
                view.gates && inside,
                scene.time.delta_secs(),
            );
            if amount != station.amounts[end] {
                commands
                    .entity(station.gates[end])
                    .insert(gate_pose(end, amount));
                station.amounts[end] = amount;
            }
            status.raised_gates += usize::from(amount > 0.75);
        }
    }
    status.surfaces = state.stations.len();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_belt_contacts_share_height_width_texture_scale_and_closed_winding() {
        for mask in 0..4 {
            let mesh = ribbon_geometry(mask);
            assert!(
                mesh.positions
                    .iter()
                    .all(|p| p[1] <= HOLD_TOP as f32 && p[0].abs() <= HALF_WIDTH)
            );
            let top: Vec<_> = mesh
                .positions
                .iter()
                .zip(&mesh.normals)
                .zip(&mesh.uvs)
                .filter(|((_, n), _)| n[1] > 0.9)
                .collect();
            assert_eq!(top.len(), 4);
            for ((p, _), uv) in top {
                assert_eq!(*uv, [p[0] + HALF_WIDTH, p[2]]);
            }
            let lo = mesh
                .positions
                .iter()
                .map(|p| p[2])
                .fold(f32::INFINITY, f32::min);
            let hi = mesh
                .positions
                .iter()
                .map(|p| p[2])
                .fold(f32::NEG_INFINITY, f32::max);
            assert_eq!(lo, if mask & 1 != 0 { -1.0 } else { -0.5 });
            assert_eq!(hi, if mask & 2 != 0 { 1.0 } else { 0.5 });
            for t in mesh.indices.chunks_exact(3) {
                let [a, b, c] =
                    std::array::from_fn(|i| Vec3::from_array(mesh.positions[t[i] as usize]));
                assert!(
                    (b - a)
                        .cross(c - a)
                        .dot(Vec3::from_array(mesh.normals[t[0] as usize]))
                        > 0.
                );
            }
        }
    }

    #[test]
    fn separators_raise_smoothly_above_the_belt_and_open_before_release_crosses_them() {
        let first = gate_amount(0., true, 1. / 60.);
        assert!(first > 0. && first < 1.);
        let mut value = 0.;
        for _ in 0..12 {
            value = gate_amount(value, true, 1. / 60.);
        }
        assert_eq!(value, 1.);
        assert!(gate_pose(0, value).translation.y > HOLD_TOP as f32 + 0.15);
        for _ in 0..3 {
            value = gate_amount(value, false, 1. / 60.);
        }
        assert_eq!(value, 0.);
        assert!(gate_pose(0, value).translation.y < HOLD_TOP as f32);
    }
}
