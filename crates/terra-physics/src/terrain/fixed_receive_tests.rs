use super::*;
use bevy::time::TimeUpdateStrategy;
use std::time::{Duration, Instant};
use terra_core::{GameConfig, TerraCorePlugin};
use terra_world::streaming::LoadedSection;

const OWNER: SectionPos = SectionPos::new(-1, 0, -1);

#[derive(Resource, Default)]
struct FixedObservations(Vec<(bool, Option<f32>)>);

fn observe_first_fixed_collision(
    state: Res<TerrainCollisionState>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    spatial: SpatialQuery,
    mut observations: ResMut<FixedObservations>,
) {
    let ready = state.ready_for_aabb(
        Vec3::new(-13.9, 0.1, -13.9),
        Vec3::new(-13.1, 1.9, -13.1),
        &chunks,
        &journal,
    );
    let distance = spatial
        .cast_ray(
            Vec3::new(-13.5, 4.0, -13.5),
            Dir3::NEG_Y,
            5.0,
            true,
            &SpatialQueryFilter::default(),
        )
        .map(|hit| hit.distance);
    observations.0.push((ready, distance));
}

fn completed_job_fixture() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(TerraCorePlugin(GameConfig::default()))
        .add_plugins(crate::TerraPhysicsPlugin)
        .insert_resource(ChunkFocus([-13.5, 3.0, -13.5]))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .init_resource::<FixedObservations>()
        // The actual movement systems run here, before Avian's later
        // FixedPostUpdate. This must already see the new spatial collider.
        .add_systems(FixedUpdate, observe_first_fixed_collision);
    let mut data = BlockSection::empty(OWNER).unwrap();
    data.set_block([2, 0, 2], 3).unwrap();
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        OWNER,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    app.finish();
    app.cleanup();
    app.update();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let jobs = app.world().resource::<ColliderJobs>();
        if jobs.0.get(&OWNER).is_some_and(|job| job.task.is_finished()) {
            break;
        }
        assert!(Instant::now() < deadline, "collider worker did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(app.world().resource::<FixedObservations>().0.is_empty());
    assert!(
        !app.world()
            .resource::<TerrainCollisionState>()
            .sections
            .contains_key(&OWNER)
    );
    app
}

#[test]
fn completed_current_collider_is_spatially_usable_on_the_first_catch_up_fixed_step() {
    let mut app = completed_job_fixture();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        100,
    )));
    app.update();
    let samples = &app.world().resource::<FixedObservations>().0;
    assert!(
        samples.len() >= 5,
        "the frame must contain several fixed steps"
    );
    for &(ready, distance) in samples {
        assert!(ready, "the first fixed step must not wait until Update");
        assert!(
            distance.is_some_and(|distance| (distance - 3.0).abs() < 0.001),
            "logical readiness must coincide with the real negative-origin spatial collider: {distance:?}"
        );
    }
    assert_eq!(
        app.world()
            .resource::<TerrainCollisionState>()
            .completed_jobs,
        1
    );
}

#[test]
fn completed_stale_collider_is_not_published_before_catch_up_fixed_steps() {
    let mut app = completed_job_fixture();
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([-14, 1, -14], 3)
        .unwrap();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        100,
    )));
    app.update();
    let samples = &app.world().resource::<FixedObservations>().0;
    assert!(samples.len() >= 5);
    assert!(
        samples
            .iter()
            .all(|&(ready, distance)| !ready && distance.is_none())
    );
    assert_eq!(
        app.world()
            .resource::<TerrainCollisionState>()
            .completed_jobs,
        0
    );
    assert!(
        !app.world()
            .resource::<TerrainCollisionState>()
            .sections
            .contains_key(&OWNER)
    );
}
