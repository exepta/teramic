//! A short-lived ribbon of parted crust and hot crests follows actual replicated motion.
use super::super::{CreatureAnimationSet, CreatureAvatar, Kind, Motion};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::{HashMap, VecDeque};
use terra_core::creatures::{CreatureBehavior as B, lava_worm as pose};
use terra_world::{
    edits::EditJournal,
    lava,
    streaming::{ChunkStore, WorldAccess},
};

const SPACING: f32 = 0.68;
const LIFE: f32 = 4.6;
const CAPACITY: usize = 96;

#[derive(Component)]
struct WakeBatch;
struct Crest {
    position: Vec3,
    direction: Vec3,
    born: f32,
}

#[derive(Resource, Default)]
struct Wake {
    previous: HashMap<(u64, bool), Vec3>,
    crests: VecDeque<Crest>,
    next_update: f32,
    mesh: Handle<Mesh>,
}

impl Wake {
    fn travel(&mut self, id: (u64, bool), position: Vec3, now: f32) {
        let from = *self.previous.entry(id).or_insert(position);
        let delta = (position - from).with_y(0.);
        let len = delta.length();
        if len > 5. || (position.y - from.y).abs() > 0.7 {
            self.previous.insert(id, position);
            return;
        }
        let direction = delta.normalize_or_zero();
        for i in 1..=(len / SPACING) as u32 {
            let sample = from + direction * (i as f32 * SPACING);
            if self.crests.len() == CAPACITY {
                self.crests.pop_front();
            }
            self.crests.push_back(Crest {
                position: sample,
                direction,
                born: now,
            });
            self.previous.insert(id, sample);
        }
    }
}

pub(in crate::creatures) fn install(app: &mut App) {
    app.init_resource::<Wake>()
        .add_systems(Startup, setup)
        .add_systems(Update, update.after(CreatureAnimationSet));
}

fn setup(
    mut commands: Commands,
    mut wake: ResMut<Wake>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    wake.mesh = meshes.add(Rectangle::new(0.01, 0.01));
    commands.spawn((
        Name::new("Magma worm wakes"),
        WakeBatch,
        Mesh3d(wake.mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        })),
        Visibility::Hidden,
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

fn surface(chunks: &ChunkStore, journal: &EditJournal, p: Vec3) -> Option<f32> {
    let cell = [
        p.x.floor() as i64,
        p.y.round() as i64 - 1,
        p.z.floor() as i64,
    ];
    let raw = |p| chunks.block_at(p).map(|b| journal.block(p).unwrap_or(b));
    (raw(cell) == Some(lava::LAVA) && raw([cell[0], cell[1] + 1, cell[2]]) == Some(0))
        .then(|| cell[1] as f32 + lava::level(journal, cell) as f32)
}

#[derive(bevy::ecs::system::SystemParam)]
struct PoolWorld<'w> {
    chunks: Option<Res<'w, ChunkStore>>,
    journal: Option<Res<'w, EditJournal>>,
    access: Option<Res<'w, WorldAccess>>,
}

fn update(
    time: Res<Time>,
    mut state: ResMut<Wake>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut batch: Query<&mut Visibility, With<WakeBatch>>,
    roots: Query<(&CreatureAvatar, &Transform, &Motion)>,
    world: PoolWorld,
) {
    let Ok(mut visible) = batch.single_mut() else {
        return;
    };
    let now = time.elapsed_secs();
    let (Some(chunks), Some(journal), Some(access)) = (world.chunks, world.journal, world.access)
    else {
        return;
    };
    if !access.ready() {
        state.previous.clear();
        state.crests.clear();
        *visible = Visibility::Hidden;
        return;
    }
    if now < state.next_update {
        return;
    }
    state.next_update = now + 0.075;
    state.crests.retain(|s| now - s.born < LIFE);
    state.previous.retain(|&(id, surface), _| {
        roots.iter().any(|(a, _, m)| {
            a.0 == id
                && matches!(m.behavior, B::Swim | B::Burrowed)
                && (m.behavior == B::Swim) == surface
        })
    });
    for (avatar, t, m) in &roots {
        if m.kind == Kind::LavaWorm && matches!(m.behavior, B::Swim | B::Burrowed) {
            let surfaced = m.behavior == B::Swim;
            let trail = if surfaced {
                // Draw from the rear plates so slow swimmers do not hide their entire wake.
                let tail = pose::center(
                    m.behavior,
                    m.worm_progress,
                    f64::from(m.clock),
                    pose::SEGMENTS - 3,
                );
                t.transform_point(tail).with_y(t.translation.y)
            } else {
                t.translation
            };
            state.travel((avatar.0, surfaced), trail, now);
        }
    }
    if state.crests.is_empty() {
        *visible = Visibility::Hidden;
        return;
    }
    let mut points = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    for crest in &state.crests {
        let age = ((now - crest.born) / LIFE).clamp(0., 1.);
        let side = Vec3::new(-crest.direction.z, 0., crest.direction.x);
        let opacity = (1. - age).powi(2);
        for flank in [-1., 1.] {
            let mut last = None;
            for j in 0..5 {
                let u = j as f32 / 4.;
                let p = crest.position + side * (flank * (0.08 + u * (0.63 + age * 0.60)))
                    - crest.direction * (u * u * 0.43 + age * 0.19);
                let Some(y) = surface(&chunks, &journal, p) else {
                    last = None;
                    continue;
                };
                let tip = 1. - (u - 0.45).abs() * 1.45;
                let index = points.len() as u32;
                for (along, lift, color) in [
                    (-0.16, 0.019, [0.12, 0.025, 0.018, 0.82 * opacity]),
                    (
                        0.,
                        0.048 * (1. - age) + 0.02,
                        [1.65, 0.48, 0.034, 0.95 * opacity],
                    ),
                    (0.064, 0.016, [1., 0.19, 0.015, 0.08 * opacity]),
                ] {
                    points.push((p.with_y(y + lift) + crest.direction * (along * tip)).to_array());
                    colors.push(color);
                }
                if let Some(prev) = last {
                    for k in 0..2 {
                        indices.extend([
                            prev + k,
                            index + k,
                            prev + k + 1,
                            prev + k + 1,
                            index + k,
                            index + k + 1,
                        ]);
                    }
                }
                last = Some(index);
            }
        }
    }
    if indices.is_empty() {
        *visible = Visibility::Hidden;
    } else {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; points.len()]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, points);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(Indices::U32(indices));
        if let Some(mut old) = meshes.get_mut(&state.mesh) {
            *old = mesh;
        }
        *visible = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wakes_follow_distance_skip_teleports_and_have_bounded_storage() {
        let mut wake = Wake::default();
        wake.travel((7, true), Vec3::ZERO, 0.);
        for i in 1..300 {
            wake.travel((7, true), Vec3::Z * (i as f32 * 0.5), i as f32);
        }
        assert_eq!(wake.crests.len(), CAPACITY);
        assert!((wake.crests.back().unwrap().position.z - 149.5).abs() < SPACING);
        let latest = wake.crests.back().unwrap().born;
        wake.travel((7, true), Vec3::X * 200., 400.);
        assert_eq!(wake.crests.back().unwrap().born, latest);
    }
}
