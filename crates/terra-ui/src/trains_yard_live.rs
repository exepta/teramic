//! Opt-in workshop inspection against a disposable, authoritative saved-world session.
use super::*;
use terra_world::{edits::EditJournal, streaming::ChunkStore};
#[derive(Resource, Default)]
struct YardProbe {
    frames: u32,
    started: bool,
}
pub(super) fn install(app: &mut App) {
    if std::env::var("TERRA_TRAIN_YARD_LIVE").as_deref() == Ok("1") {
        app.init_resource::<YardProbe>()
            .add_systems(Update, run.after(super::super::update))
            .add_systems(
                PostUpdate,
                look.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut probe: Mut<YardProbe>| {
        if !super::super::playing(world) {
            return;
        }
        if !probe.started {
            let cell = std::env::var("TERRA_TRAIN_YARD_STATION")
                .expect("yard station required")
                .split(',')
                .map(|part| part.parse::<i64>().unwrap())
                .collect::<Vec<_>>();
            let station: [i64; 3] = cell.try_into().expect("three station coordinates");
            if world.resource::<ChunkStore>().block_at(station) != Some(217) {
                return;
            }
            if terra_world::train_yard::blueprint(
                world.resource::<ChunkStore>(),
                world.resource::<EditJournal>(),
                None,
                station,
                VehicleKind::Locomotive,
                8,
                3,
            )
            .is_none()
            {
                return;
            }
            let identity = {
                let session = world.resource::<ClientSession>();
                (session.connection, session.uuid)
            };
            let mut state = world.resource_mut::<TrainUiState>();
            state.open = true;
            state.station = Some(station);
            state.identity = Some(identity);
            state.wheelbase = 8;
            state.width = 3;
            world.write_message(TrainUiAction::Command("blueprint".into()));
            world.write_message(terra_player::CaptureRequest(false));
            probe.started = true;
            return;
        }
        probe.frames += 1;
        if probe.frames == 120 {
            let guide = world.resource::<TrainBlueprint>();
            assert!(
                !guide.cells.is_empty(),
                "saved-world assembly must expose a real guide"
            );
            let chassis = guide
                .cells
                .iter()
                .find(|part| part.material == 210)
                .unwrap();
            let state = world.resource::<TrainUiState>();
            let request = terra_world::trains::BuildRequest {
                actor: world.resource::<ClientSession>().uuid.unwrap(),
                station: state.station.unwrap(),
                chassis: chassis.cell,
                facing: chassis.facing,
                kind: state.kind,
                wheelbase: state.wheelbase(),
                width: state.width(),
                creative: true,
            };
            let plan = terra_world::trains::plan_blueprint(
                world.resource::<ChunkStore>(),
                world.resource::<EditJournal>(),
                request,
            )
            .expect("guide must pass the real build planner");
            info!(
                cells = guide.cells.len(),
                missing = plan.cells.len(),
                "LIVE YARD PASS: real saved-world guide and authoritative build validation"
            );
            capture(world, "saved-workshop.png");
        }
        if probe.frames == 180 {
            world.write_message(TrainUiAction::Command("close".into()));
        }
        if probe.frames == 240 {
            capture(world, "saved-guide.png");
        }
        if probe.frames == 300 {
            *world.resource_mut::<TrainBlueprint>() = TrainBlueprint::default();
        }
        if probe.frames == 360 {
            capture(world, "saved-yard.png");
        }
    });
}
fn look(mut cameras: Query<&mut Transform, With<terra_player::TerraCamera>>) {
    let cell = std::env::var("TERRA_TRAIN_YARD_STATION")
        .unwrap()
        .split(',')
        .map(|part| part.parse::<f32>().unwrap())
        .collect::<Vec<_>>();
    let target = Vec3::new(cell[0] + 3.0, cell[1] + 1.0, cell[2] + 1.0);
    for mut camera in &mut cameras {
        if camera.translation.distance_squared(target) > 0.001 {
            camera.look_at(target, Vec3::Y);
        }
    }
}
fn capture(world: &mut World, name: &str) {
    let folder = std::path::PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").expect("capture directory required"),
    );
    std::fs::create_dir_all(&folder).unwrap();
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(name)));
}
