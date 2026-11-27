//! A fixed pool of shallow sand impressions, shared meshes, no terrain edits.
use super::super::{CreatureAnimationSet, CreatureAvatar, Kind, Models, Motion, model::Part};
use bevy::{light::NotShadowCaster, prelude::*};
use std::collections::HashMap;
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};

const CAPACITY: usize = 128;
const SPACING: f32 = 0.35;
const LIFETIME: f32 = 10.0;
#[derive(Component)]
struct WormTrail(usize);
#[derive(Clone, Copy)]
struct Stamp {
    point: Vec3,
    rotation: Quat,
    born: f32,
}
#[derive(Clone, Copy)]
struct ActiveWorm {
    id: u64,
    point: Vec3,
    rotation: Quat,
    behavior: super::B,
    speed: f32,
}
#[derive(Resource)]
struct Trail {
    entities: Vec<Entity>,
    stamps: [Option<Stamp>; CAPACITY],
    last: HashMap<u64, Vec3>,
    cursor: usize,
    active: Vec<ActiveWorm>,
}
impl Default for Trail {
    fn default() -> Self {
        Self {
            entities: Vec::new(),
            stamps: [None; CAPACITY],
            last: HashMap::new(),
            cursor: 0,
            active: Vec::new(),
        }
    }
}
impl Trail {
    fn stamp(&mut self, stamp: Stamp) {
        self.stamps[self.cursor] = Some(stamp);
        self.cursor = (self.cursor + 1) % CAPACITY;
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Trail>()
        .add_systems(Update, update.after(CreatureAnimationSet));
}

type TrackMark = (
    &'static WormTrail,
    &'static mut Transform,
    &'static mut Visibility,
    Option<&'static mut MeshMaterial3d<StandardMaterial>>,
);

#[allow(clippy::too_many_arguments)]
fn update(
    mut commands: Commands,
    time: Res<Time>,
    access: Option<Res<WorldAccess>>,
    chunks: Option<Res<ChunkStore>>,
    journal: Option<Res<EditJournal>>,
    models: Option<Res<Models>>,
    mut state: ResMut<Trail>,
    roots: Query<(&CreatureAvatar, &Transform, &Motion)>,
    mut marks: Query<TrackMark, Without<CreatureAvatar>>,
) {
    let ready = access.is_none_or(|a| a.ready());
    if !ready {
        state.stamps.fill(None);
    }
    // Reuse storage: a long idle session must not allocate a fresh list at FPS.
    let mut worms = std::mem::take(&mut state.active);
    worms.clear();
    worms.extend(roots.iter().filter_map(|(avatar, pose, motion)| {
        (motion.kind == Kind::SandWorm).then_some(ActiveWorm {
            id: avatar.0,
            point: pose.translation,
            rotation: pose.rotation,
            behavior: motion.behavior,
            speed: motion.speed,
        })
    }));
    if state.entities.is_empty()
        && !worms.is_empty()
        && let Some(models) = &models
    {
        let mesh = models.parts.get(&(Kind::SandWorm, 0)).and_then(|parts| {
            parts
                .iter()
                .find(|(part, _, _)| matches!(part, Part::WormMound))
                .map(|(_, _, mesh)| mesh)
        });
        if let Some(mesh) = mesh {
            for index in 0..CAPACITY {
                state.entities.push(
                    commands
                        .spawn((
                            Name::new("Fading sandworm track"),
                            WormTrail(index),
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(models.material.clone()),
                            Transform::default(),
                            Visibility::Hidden,
                            NotShadowCaster,
                            crate::miniature::creature_layers(),
                        ))
                        .id(),
                );
            }
        }
    }
    state
        .last
        .retain(|id, _| ready && worms.iter().any(|worm| worm.id == *id));
    let now = time.elapsed_secs();
    if ready && let (Some(chunks), Some(journal)) = (chunks.as_ref(), journal.as_ref()) {
        for worm in &worms {
            // Passive patrols and fast underground charges both displace sand.
            // Aggression, attack goals and reported velocity never gate trails;
            // the presented world-space path is the source of truth.
            if !matches!(
                worm.behavior,
                super::B::Burrowed | super::B::Hunt | super::B::Tremor | super::B::Attack
            ) {
                state.last.remove(&worm.id);
                continue;
            }
            let previous = state.last.get(&worm.id).copied();
            if previous.is_none() && worm.speed < 0.03 {
                state.last.insert(worm.id, worm.point);
                continue;
            }
            let from = previous.unwrap_or(worm.point);
            let delta = worm.point - from;
            let distance = delta.x.hypot(delta.z);
            // A reanchored leap or an administrative teleport does not draw a
            // false scar across unloaded air. Ordinary travel is resampled at
            // fixed world spacing, independent of FPS and snapshot frequency.
            if distance > 6.0 {
                state.last.insert(worm.id, worm.point);
                continue;
            }
            if worm.speed < 0.03 && distance < SPACING {
                continue;
            }
            if previous.is_some() && distance < SPACING {
                continue;
            }
            let count = ((distance / SPACING).floor() as usize).clamp(1, 16);
            for step in 1..=count {
                let point = if distance < SPACING {
                    worm.point
                } else {
                    from + delta * (step as f32 * SPACING / distance).min(1.0)
                };
                let cell = [point.x.floor() as i64, point.z.floor() as i64];
                let Some(height) =
                    super::ground::surface(chunks, journal, cell, point.y.floor() as i64)
                else {
                    continue;
                };
                state.last.insert(worm.id, point);
                state.stamp(Stamp {
                    point: Vec3::new(point.x, height, point.z),
                    rotation: worm.rotation,
                    born: now,
                });
            }
        }
    }
    for (mark, mut transform, mut visibility, material) in &mut marks {
        let stamp = state.stamps[mark.0].filter(|s| ready && now - s.born < LIFETIME);
        let Some(stamp) = stamp else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        let cell = [
            stamp.point.x.floor() as i64,
            stamp.point.y.round() as i64 - 1,
            stamp.point.z.floor() as i64,
        ];
        let edit = journal.as_ref().and_then(|journal| journal.block(cell));
        let ground = chunks
            .as_ref()
            .and_then(|chunks| chunks.block_at(cell))
            .map(|block| edit.unwrap_or(block))
            .filter(|block| {
                terra_core::creatures::sandworm::burrowable_ground(*block, edit.is_some())
            });
        let Some(block) = ground else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        if let (Some(mut material), Some(models)) = (material, &models) {
            super::ground::apply_sand_material(&mut material, models, block);
        }
        let fade = ((now - stamp.born) / LIFETIME).clamp(0.0, 1.0);
        transform.translation = stamp.point + Vec3::Y * (0.16 * (1.0 - fade));
        transform.rotation = stamp.rotation;
        transform.scale = Vec3::new(0.65 + fade * 0.10, 1.35 * (1.0 - fade), 1.00);
        super::ground::fit_to_cell(&Transform::IDENTITY, &mut transform, Vec2::new(0.97, 1.10));
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
    }
    if !ready {
        state.stamps = [None; CAPACITY];
    }
    state.active = worms;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_travel_reuses_a_fixed_track_pool() {
        let mut trail = Trail::default();
        for step in 0..10_000 {
            trail.stamp(Stamp {
                point: Vec3::new(step as f32, 1.0, 0.0),
                rotation: Quat::IDENTITY,
                born: step as f32,
            });
        }
        assert_eq!(trail.stamps.iter().flatten().count(), CAPACITY);
        assert!(
            trail
                .stamps
                .iter()
                .flatten()
                .all(|s| s.born >= (10_000 - CAPACITY) as f32)
        );
        assert!(trail.cursor < CAPACITY);
    }

    fn trail_scene(behavior: super::super::B) -> (App, Entity, Entity) {
        use terra_voxel::{BlockSection, MaterialId, SectionPos};
        use terra_world::streaming::LoadedSection;
        let mut app = App::new();
        let mut chunks = ChunkStore::default();
        let position = SectionPos::default();
        let mut data = BlockSection::empty(position).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                let material = if x < 3 {
                    MaterialId::RedDuneSand
                } else {
                    MaterialId::RedSandstone
                };
                data.set_block([x, 0, z], material as u16).unwrap();
            }
        }
        chunks.loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        app.insert_resource(chunks)
            .insert_resource(EditJournal::default())
            .init_resource::<Time>()
            .init_resource::<Trail>()
            .add_systems(Update, update);
        let mark = app
            .world_mut()
            .spawn((WormTrail(0), Transform::default(), Visibility::Hidden))
            .id();
        app.world_mut().resource_mut::<Trail>().entities.push(mark);
        let root = app
            .world_mut()
            .spawn((
                CreatureAvatar(5),
                Transform::from_xyz(2.5, 1.0, 2.5),
                Motion {
                    kind: Kind::SandWorm,
                    behavior,
                    phase: 0.0,
                    clock: 0.0,
                    speed: 0.0,
                    sleep: 0.0,
                    attack: 0.0,
                    worm_progress: 0.0,
                    worm_spit: None,
                    silk_length: 0.0,
                    youth: 0.0,
                    jump: Default::default(),
                },
            ))
            .id();
        (app, root, mark)
    }

    #[test]
    fn actual_burrow_displacement_leaves_visible_sand_even_with_a_zero_velocity_snapshot() {
        let (mut app, root, mark) = trail_scene(super::super::B::Burrowed);
        app.update();
        assert_eq!(
            app.world()
                .resource::<Trail>()
                .stamps
                .iter()
                .flatten()
                .count(),
            0
        );
        app.world_mut()
            .get_mut::<Transform>(root)
            .unwrap()
            .translation
            .x += 1.4;
        app.update();
        assert_eq!(
            app.world()
                .resource::<Trail>()
                .stamps
                .iter()
                .flatten()
                .count(),
            4
        );
        assert_eq!(
            *app.world().get::<Visibility>(mark).unwrap(),
            Visibility::Inherited
        );
        assert!(app.world().get::<Transform>(mark).unwrap().translation.y > 1.1);
        // The transport/reanchor discontinuity must not draw across unknown ground.
        app.world_mut()
            .get_mut::<Transform>(root)
            .unwrap()
            .translation
            .x += 50.0;
        app.update();
        assert_eq!(
            app.world()
                .resource::<Trail>()
                .stamps
                .iter()
                .flatten()
                .count(),
            4
        );
    }

    #[test]
    fn passive_patrol_and_charge_share_a_continuous_trail_without_attack_or_velocity() {
        let (mut app, root, mark) = trail_scene(super::super::B::Burrowed);
        app.update();
        for behavior in [
            super::super::B::Burrowed,
            super::super::B::Hunt,
            super::super::B::Burrowed,
        ] {
            {
                let mut motion = app.world_mut().get_mut::<Motion>(root).unwrap();
                motion.behavior = behavior;
                assert_eq!(motion.attack, 0.0);
                assert_eq!(motion.speed, 0.0);
                assert!(motion.worm_spit.is_none());
            }
            app.world_mut()
                .get_mut::<Transform>(root)
                .unwrap()
                .translation
                .x += 1.4;
            let before = app.world().resource::<Trail>().cursor;
            app.update();
            let state = app.world().resource::<Trail>();
            assert!(
                state.cursor >= before + 3,
                "{behavior:?} lost its sand path"
            );
            assert_eq!(
                *app.world().get::<Visibility>(mark).unwrap(),
                Visibility::Inherited
            );
            let at_rest = state.cursor;
            app.update();
            assert_eq!(app.world().resource::<Trail>().cursor, at_rest);
        }
    }
}
