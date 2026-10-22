//! A single bounded, transient mesh for soft-ground sole and cloven-hoof tracks.
use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use terra_core::footsteps::{Footstep, FootstepSet, Walker};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};

const MAX_TRACKS: usize = 512;
#[derive(Resource, Default, Debug)]
pub struct FootprintStatus {
    pub player_sand: usize,
    pub player_snow: usize,
    pub animal_sand: usize,
    pub animal_snow: usize,
    pub vertices: usize,
}
#[derive(Clone, Copy)]
struct Track {
    step: Footstep,
    born: f32,
    lifetime: f32,
}
#[derive(Resource, Default)]
struct Tracks {
    marks: std::collections::VecDeque<Track>,
    mesh: Handle<Mesh>,
    next_update: f32,
}
#[derive(Component)]
struct TrackBatch;

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Tracks>()
        .init_resource::<FootprintStatus>()
        .add_systems(Startup, setup)
        .add_systems(Update, update.after(FootstepSet));
}
fn setup(
    mut commands: Commands,
    mut state: ResMut<Tracks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Empty vertex/index allocations are invalid for Bevy's shared GPU allocator.
    // Keep a hidden valid mesh until the first contact, and hide it after expiry.
    state.mesh = meshes.add(Rectangle::new(0.01, 0.01));
    commands.spawn((
        Name::new("Transient sand and snow tracks"),
        TrackBatch,
        Mesh3d(state.mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            depth_bias: 2.0,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}
fn update(
    environment: (
        Res<Time>,
        Res<WorldAccess>,
        Res<ChunkStore>,
        Res<EditJournal>,
    ),
    mut events: MessageReader<Footstep>,
    mut state: ResMut<Tracks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut status: ResMut<FootprintStatus>,
    mut batches: Query<&mut Visibility, With<TrackBatch>>,
) {
    let (time, access, chunks, journal) = environment;
    let now = time.elapsed_secs();
    let mut dirty = false;
    for step in events.read() {
        if !access.ready() {
            continue;
        }
        if let Some(lifetime) = step.lifetime() {
            if state.marks.len() >= MAX_TRACKS {
                state.marks.pop_front();
            }
            state.marks.push_back(Track {
                step: *step,
                born: now,
                lifetime,
            });
            dirty = true;
        }
    }
    if now < state.next_update {
        return;
    }
    state.next_update = now + 0.05;
    let had_marks = !state.marks.is_empty();
    state.marks.retain(|track| {
        let p = (track.step.position - Vec3::Y * 0.08)
            .floor()
            .as_i64vec3()
            .to_array();
        access.ready()
            && now - track.born < track.lifetime
            && chunks.block_at(p).map(|b| journal.block(p).unwrap_or(b))
                == Some(track.step.material)
            && chunks
                .block_at([p[0], p[1] + 1, p[2]])
                .map(|b| journal.block([p[0], p[1] + 1, p[2]]).unwrap_or(b))
                == Some(0)
    });
    *status = FootprintStatus::default();
    for track in &state.marks {
        match (track.step.walker, track.step.material) {
            (Walker::Player, 4) => status.player_sand += 1,
            (Walker::Player, 5 | 98) => status.player_snow += 1,
            (_, 4) => status.animal_sand += 1,
            (_, 5 | 98) => status.animal_snow += 1,
            _ => {}
        }
    }
    for mut visibility in &mut batches {
        let target = if state.marks.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != target {
            *visibility = target;
        }
    }
    if !state.marks.is_empty()
        && (had_marks || dirty)
        && let Some(mut target) = meshes.get_mut(&state.mesh)
    {
        let tracks: Vec<_> = state.marks.iter().copied().collect();
        *target = mesh(&tracks, now);
        status.vertices = target.count_vertices();
    }
}

fn mesh(tracks: &[Track], now: f32) -> Mesh {
    let mut positions = Vec::<[f32; 3]>::new();
    let mut colors = Vec::<[f32; 4]>::new();
    let mut indices = Vec::new();
    for track in tracks {
        let step = track.step;
        let forward = step.forward.normalize_or(Vec2::Y);
        let side = Vec2::new(forward.y, -forward.x);
        let fade =
            ((track.lifetime - (now - track.born)) / (track.lifetime * 0.35)).clamp(0.0, 1.0);
        let color = if matches!(step.material, 5 | 98) {
            [0.24, 0.34, 0.46, fade * 0.36]
        } else {
            [0.32, 0.23, 0.13, fade * 0.36]
        };
        let mut quad = |offset: Vec2, half: Vec2| {
            let start = positions.len() as u32;
            for corner in [
                Vec2::new(-1.0, -1.0),
                Vec2::new(1.0, -1.0),
                Vec2::ONE,
                Vec2::new(-1.0, 1.0),
            ] {
                let p = offset + corner * half;
                let horizontal = side * p.x + forward * p.y;
                positions.push(
                    (step.position + Vec3::new(horizontal.x, 0.012, horizontal.y)).to_array(),
                );
                colors.push(color);
            }
            indices.extend([start, start + 2, start + 1, start, start + 3, start + 2]);
        };
        let sign = if step.alternate { 1.0 } else { -1.0 };
        if step.walker == Walker::Player {
            // Separate toe and heel leave a small arch, rather than a painted box.
            quad(Vec2::new(sign * 0.13, 0.07), Vec2::new(0.065, 0.105));
            quad(Vec2::new(sign * 0.13, -0.12), Vec2::new(0.055, 0.055));
        } else if step.walker == Walker::Bear {
            for (x, z) in [(sign * 0.30, 0.40), (-sign * 0.30, -0.40)] {
                quad(Vec2::new(x, z), Vec2::new(0.095, 0.10));
                for toe in -2..=2 {
                    quad(
                        Vec2::new(x + toe as f32 * 0.038, z + 0.13),
                        Vec2::new(0.014, 0.035),
                    );
                }
            }
        } else {
            let scale = if step.walker == Walker::Cow {
                1.15
            } else {
                0.85
            };
            for (x, z) in [(sign * 0.26, 0.35), (-sign * 0.26, -0.35)] {
                for split in [-0.034, 0.034] {
                    quad(
                        Vec2::new(x + split, z) * scale,
                        Vec2::new(0.024, 0.070) * scale,
                    );
                }
            }
        }
    }
    let count = positions.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; count])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_track_geometry_has_valid_indices_and_fades_without_entities_per_print() {
        for walker in [
            Walker::Player,
            Walker::Cow,
            Walker::Sheep,
            Walker::Pig,
            Walker::Bear,
        ] {
            let track = Track {
                step: Footstep {
                    position: Vec3::new(-5.0, 2.0, -3.0),
                    forward: Vec2::Y,
                    walker,
                    material: 4,
                    alternate: false,
                },
                born: 0.0,
                lifetime: 10.0,
            };
            let m = mesh(&[track], 9.9);
            assert!(m.indices().unwrap().iter().all(|i| i < m.count_vertices()));
            let bevy::mesh::VertexAttributeValues::Float32x4(colors) =
                m.attribute(Mesh::ATTRIBUTE_COLOR).unwrap()
            else {
                panic!()
            };
            assert!(colors.iter().all(|c| c[3] > 0.0 && c[3] < 0.02));
        }
    }
}
