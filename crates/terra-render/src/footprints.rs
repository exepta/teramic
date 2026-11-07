//! A single bounded, transient mesh for soft-ground sole and cloven-hoof tracks.
use bevy::{asset::RenderAssetUsages, light::NotShadowCaster, prelude::*};
use terra_core::footsteps::{Footstep, FootstepSet, Walker};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};

const MAX_TRACKS: usize = 512;
mod relief;
pub(crate) use relief::Imprints;
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
    dirty: bool,
    rendered_at: f32,
    vertices: usize,
    imprints: Imprints,
    support_fingerprint: u64,
}
#[derive(Component)]
struct TrackBatch;

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Tracks>()
        .init_resource::<FootprintStatus>()
        .init_resource::<Imprints>()
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
        bevy::camera::visibility::NoFrustumCulling,
        Mesh3d(state.mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.97,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
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
    mut imprints: Option<ResMut<Imprints>>,
) {
    let (time, access, chunks, journal) = environment;
    let now = time.elapsed_secs();
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
            state.dirty = true;
        }
    }
    if now < state.next_update {
        return;
    }
    state.next_update = now + 0.05;
    let previous_count = state.marks.len();
    state.marks.retain(|track| {
        let p = (track.step.position - Vec3::Y * 0.02)
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
    let removed = state.marks.len() != previous_count;
    state.dirty |= removed;
    let mut fingerprint = 0xcbf29ce484222325u64;
    for track in &state.marks {
        // At most four touched sections, including a hoof just across a boundary.
        let p = track.step.position;
        let low = (p - Vec3::new(0.7, 0.02, 0.7))
            .floor()
            .as_i64vec3()
            .to_array();
        let high = (p + Vec3::new(0.7, -0.02, 0.7))
            .floor()
            .as_i64vec3()
            .to_array();
        let a = terra_voxel::global_to_section(low).0;
        let b = terra_voxel::global_to_section(high).0;
        for x in a.x..=b.x {
            for z in a.z..=b.z {
                let section = terra_voxel::SectionPos::new(x, a.y, z);
                let epoch = chunks
                    .loaded
                    .get(&section)
                    .map_or(u64::MAX, |s| s.mesh_epoch);
                fingerprint = (fingerprint ^ epoch).wrapping_mul(0x100000001b3);
                fingerprint = (fingerprint ^ journal.revision(section)).wrapping_mul(0x100000001b3);
            }
        }
    }
    if fingerprint != state.support_fingerprint {
        state.support_fingerprint = fingerprint;
        state.dirty = true;
    }
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
    refresh_mesh(&mut state, &mut meshes, now, &chunks, &journal);
    for mut visibility in &mut batches {
        let target = if state.vertices == 0 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != target {
            *visibility = target;
        }
    }
    if let Some(imprints) = imprints.as_deref_mut()
        && imprints.0 != state.imprints.0
    {
        imprints.0 = state.imprints.0;
    }
    if !state.marks.is_empty() {
        status.vertices = state.vertices;
    }
}

fn alpha(track: &Track, now: f32) -> f32 {
    ((track.lifetime - (now - track.born)) / (track.lifetime * 0.35)).clamp(0.0, 1.0) * 0.36
}

fn refresh_mesh(
    state: &mut Tracks,
    meshes: &mut Assets<Mesh>,
    now: f32,
    chunks: &ChunkStore,
    journal: &EditJournal,
) {
    if state.marks.is_empty() {
        state.dirty = false;
        state.vertices = 0;
        state.imprints = Imprints::default();
        return;
    }
    if !state.dirty
        && state
            .marks
            .iter()
            .all(|track| alpha(track, now) == alpha(track, state.rendered_at))
    {
        return;
    }
    let tracks: Vec<_> = state.marks.iter().copied().collect();
    let (next, imprints) = relief::build_checked(&tracks, now, |center, side, half, material| {
        supported_patch(chunks, journal, center, side, half, material)
    });
    state.imprints = imprints;
    if next.count_vertices() == 0 {
        state.vertices = 0;
        state.rendered_at = now;
        state.dirty = false;
        return;
    }
    let Some(previous) = meshes.get(&state.mesh) else {
        return;
    };
    let unchanged = previous.primitive_topology() == next.primitive_topology()
        && previous.indices() == next.indices()
        && previous
            .attributes()
            .map(|(attribute, values)| (attribute.id, attribute.format, values))
            .eq(next
                .attributes()
                .map(|(attribute, values)| (attribute.id, attribute.format, values)));
    state.vertices = next.count_vertices();
    state.rendered_at = now;
    state.dirty = false;
    // Modified mesh events trigger a global Mesh3d metadata scan in Bevy.
    // Compare through an immutable borrow before publishing a real payload change.
    if !unchanged && let Some(mut target) = meshes.get_mut(&state.mesh) {
        *target = next;
    }
}

fn supported_patch(
    chunks: &ChunkStore,
    journal: &EditJournal,
    center: Vec3,
    side: Vec2,
    half: Vec2,
    material: u16,
) -> bool {
    let extent = side.abs() * half.x + Vec2::new(-side.y, side.x).abs() * half.y;
    let low = (center.xz() - extent).floor().as_i64vec2();
    let high = (center.xz() + extent).floor().as_i64vec2();
    let y = (center.y - 0.02).floor() as i64;
    for x in low.x..=high.x {
        for z in low.y..=high.y {
            let p = [x, y, z];
            let Some(actual) = chunks.block_at(p).map(|b| journal.block(p).unwrap_or(b)) else {
                return false;
            };
            if actual != material {
                return false;
            }
            let height = y as f32
                + if actual == 98 {
                    f32::from(
                        journal
                            .object_state(p)
                            .map_or(1, |s| terra_core::construction::snow_layers(s.facing)),
                    ) / 8.0
                } else {
                    1.0
                };
            let above = [x, y + 1, z];
            if (height - center.y).abs() >= 0.006
                || chunks
                    .block_at(above)
                    .map(|b| journal.block(above).unwrap_or(b))
                    != Some(0)
            {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
fn mesh(tracks: &[Track], now: f32) -> Mesh {
    relief::build(tracks, now).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_single_dynamic_world_space_batch_does_not_reuse_its_hidden_startup_aabb() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_resource::<Tracks>();
        app.finish();
        app.cleanup();
        app.world_mut().run_system_cached(setup).unwrap();
        let batches: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, With<TrackBatch>>()
            .iter(app.world())
            .collect();
        assert_eq!(batches.len(), 1);
        assert!(
            app.world()
                .get::<bevy::camera::visibility::NoFrustumCulling>(batches[0])
                .is_some()
        );
    }

    #[test]
    fn recesses_require_real_same_height_soft_support_under_their_entire_extent() {
        let (mut app, _, _) = fixture();
        let supported = |app: &App, center, half| {
            supported_patch(
                app.world().resource(),
                app.world().resource(),
                center,
                Vec2::X,
                half,
                4,
            )
        };
        assert!(supported(
            &app,
            Vec3::new(-0.5, 1.0, -0.5),
            Vec2::splat(0.1)
        ));
        assert!(
            !supported(&app, Vec3::new(-0.02, 1.0, -0.5), Vec2::splat(0.1)),
            "An overhanging sole must not form a floating shell"
        );
        let cell = [-1, 0, -1];
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            journal.record(cell, 98).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(terra_world::edits::BlockObjectState {
                        facing: 19,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        assert!(supported_patch(
            app.world().resource(),
            app.world().resource(),
            Vec3::new(-0.5, 0.375, -0.5),
            Vec2::X,
            Vec2::splat(0.1),
            98
        ));
        assert!(!supported_patch(
            app.world().resource(),
            app.world().resource(),
            Vec3::new(-0.5, 1.0, -0.5),
            Vec2::X,
            Vec2::splat(0.1),
            98
        ));
    }

    fn step(walker: Walker, alternate: bool) -> Footstep {
        Footstep {
            position: Vec3::new(-0.5, 1.0, -0.5),
            forward: Vec2::Y,
            walker,
            material: 4,
            alternate,
        }
    }

    fn fixture() -> (App, Entity, Handle<Mesh>) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_resource::<WorldAccess>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<FootprintStatus>()
            .add_message::<Footstep>()
            .add_systems(
                PostUpdate,
                bevy::mesh::mark_3d_meshes_as_changed_if_their_assets_changed
                    .after(bevy::asset::AssetEventSystems),
            );
        app.finish();
        app.cleanup();
        let position = terra_voxel::SectionPos::new(-1, 0, -1);
        let mut data = terra_voxel::BlockSection::empty(position).unwrap();
        data.set_block([15, 0, 15], 4).unwrap();
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Rectangle::new(0.01, 0.01));
        app.insert_resource(Tracks {
            mesh: handle.clone(),
            ..default()
        });
        app.insert_resource(Time::<()>::default());
        let entity = app
            .world_mut()
            .spawn((TrackBatch, Mesh3d(handle.clone()), Visibility::Hidden))
            .id();
        (app, entity, handle)
    }

    fn tick(app: &mut App, now: f32) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_to(std::time::Duration::from_secs_f32(now));
        app.world_mut().run_system_cached(update).unwrap();
        app.world_mut().run_schedule(PostUpdate);
    }

    fn color_alpha(app: &App, handle: &Handle<Mesh>) -> f32 {
        let assets = app.world().resource::<Assets<Mesh>>();
        let bevy::mesh::VertexAttributeValues::Float32x4(colors) = assets
            .get(handle)
            .unwrap()
            .attribute(Mesh::ATTRIBUTE_COLOR)
            .unwrap()
        else {
            panic!("Track colors must remain Float32x4");
        };
        colors[0][3]
    }

    #[test]
    fn static_first_sixty_five_percent_preserves_asset_events_and_mesh_revision() {
        let (mut app, entity, handle) = fixture();
        app.world_mut()
            .resource_mut::<Messages<Footstep>>()
            .write(step(Walker::Player, false));
        tick(&mut app, 0.0);
        let initial = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&handle)
            .unwrap()
            .clone();
        let revision = app
            .world()
            .entity(entity)
            .get_ref::<Mesh3d>()
            .unwrap()
            .last_changed();
        let mut events = app
            .world()
            .resource::<Messages<AssetEvent<Mesh>>>()
            .get_cursor_current();
        for now in [0.01, 0.06, 0.25, 1.0, 3.0, 6.0, 6.4] {
            tick(&mut app, now);
            assert_eq!(app.world().resource::<FootprintStatus>().vertices, 40);
            assert_eq!(color_alpha(&app, &handle), 0.36);
        }
        // Even an explicit dirty notification cannot republish identical bytes.
        app.world_mut().resource_mut::<Tracks>().dirty = true;
        tick(&mut app, 6.5);
        assert_eq!(events.read(app.world().resource()).count(), 0);
        assert_eq!(
            app.world()
                .entity(entity)
                .get_ref::<Mesh3d>()
                .unwrap()
                .last_changed(),
            revision
        );
        tick(&mut app, 6.6);
        assert_eq!(
            events
                .read(app.world().resource())
                .filter(|event| {
                    matches!(event, AssetEvent::Modified { id } if *id == handle.id())
                })
                .count(),
            1
        );
        assert_ne!(
            app.world()
                .entity(entity)
                .get_ref::<Mesh3d>()
                .unwrap()
                .last_changed(),
            revision
        );
        let assets = app.world().resource::<Assets<Mesh>>();
        let fading = assets.get(&handle).unwrap();
        assert_eq!(
            initial.attribute(Mesh::ATTRIBUTE_UV_0.id),
            fading.attribute(Mesh::ATTRIBUTE_UV_0.id)
        );
        assert_eq!(initial.indices(), fading.indices());
        assert_ne!(
            initial.attribute(Mesh::ATTRIBUTE_POSITION),
            fading.attribute(Mesh::ATTRIBUTE_POSITION)
        );
        assert!((color_alpha(&app, &handle) - (10.0 - 6.6) / 3.5 * 0.36).abs() < 1e-6);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    }

    #[test]
    fn pending_contacts_partial_expiry_and_last_expiry_update_the_same_mesh() {
        let (mut app, entity, handle) = fixture();
        for contact in [step(Walker::Player, false), step(Walker::Cow, false)] {
            app.world_mut()
                .resource_mut::<Messages<Footstep>>()
                .write(contact);
        }
        tick(&mut app, 0.0);
        let before = app.world().resource::<FootprintStatus>().vertices;
        assert!(before > 40 && before <= 120);
        app.world_mut()
            .resource_mut::<Messages<Footstep>>()
            .write(step(Walker::Player, true));
        tick(&mut app, 0.01);
        assert!(app.world().resource::<Tracks>().dirty);
        assert_eq!(app.world().resource::<FootprintStatus>().vertices, before);
        tick(&mut app, 0.06);
        assert!(!app.world().resource::<Tracks>().dirty);
        assert!(app.world().resource::<FootprintStatus>().vertices >= 40);
        tick(&mut app, 3.0);
        assert_eq!(app.world().resource::<FootprintStatus>().vertices, 80);
        let expected = mesh(
            &[
                Track {
                    step: step(Walker::Player, false),
                    born: 0.0,
                    lifetime: 10.0,
                },
                Track {
                    step: step(Walker::Player, true),
                    born: 0.01,
                    lifetime: 10.0,
                },
            ],
            3.0,
        );
        let assets = app.world().resource::<Assets<Mesh>>();
        let actual = assets.get(&handle).unwrap();
        assert_eq!(actual.indices(), expected.indices());
        assert!(
            actual
                .attributes()
                .zip(expected.attributes())
                .all(|((a, av), (b, bv))| a.id == b.id && av == bv)
        );
        tick(&mut app, 10.0);
        assert_eq!(app.world().resource::<FootprintStatus>().vertices, 40);
        assert!(color_alpha(&app, &handle) < 0.002);
        tick(&mut app, 10.1);
        assert!(app.world().resource::<Tracks>().marks.is_empty());
        assert_eq!(app.world().resource::<FootprintStatus>().vertices, 0);
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Hidden
        );
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        assert!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&handle)
                .unwrap()
                .count_vertices()
                > 0
        );
    }

    #[test]
    fn shared_track_geometry_has_valid_indices_and_fades_without_entities_per_print() {
        for walker in [
            Walker::Player,
            Walker::Cow,
            Walker::Sheep,
            Walker::Pig,
            Walker::Bear,
            Walker::Stalker,
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
