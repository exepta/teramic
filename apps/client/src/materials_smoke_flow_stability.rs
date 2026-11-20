//! Fixed-camera sequence while the wind turns over a full, distant test basin.
use super::*;

pub(super) const ORIGIN: [i64; 2] = [2048, -1792];
const FRAMES: usize = 12;

#[derive(Resource, Default)]
pub(super) struct Sequence {
    scheduled: usize,
    saved: usize,
}

pub(super) fn enabled() -> bool {
    std::env::var("TERRA_BASIN_WIND_SEQUENCE").as_deref() == Ok("1")
}

pub(super) fn install(app: &mut App) {
    if enabled() {
        app.init_resource::<Sequence>()
            .add_systems(First, rotate_wind);
    }
}

fn rotate_wind(mut weather: ResMut<terra_core::WorldWeather>, sequence: Res<Sequence>) {
    let (s, c) = (0.64 + sequence.scheduled as f32 * 0.045).sin_cos();
    weather.from_wind = [5.2 * c, 5.2 * s];
    weather.target_wind = weather.from_wind;
}

pub(super) fn fill(view: &mut View, base: i64) {
    for x in 7..=10 {
        for z in 7..=10 {
            cell(view, 0, [x, base, z], 8, true);
        }
    }
}

pub(super) fn capture(commands: &mut Commands, seq: &mut Sequence, view: &View, base: i64) {
    if seq.scheduled >= FRAMES {
        return;
    }
    let (_, camera, transform) = view.cameras.single().expect("native basin camera");
    let scale = camera.physical_viewport_size().unwrap().as_vec2()
        / camera.logical_viewport_size().unwrap();
    let pixels = [[7.35, 7.35], [9.60, 7.35], [9.60, 9.60], [7.35, 9.60]].map(|[x, z]| {
        (camera
            .world_to_viewport(
                transform,
                Vec3::new(ORIGIN[0] as f32 + x, base as f32 + 1., ORIGIN[1] as f32 + z),
            )
            .unwrap()
            * scale)
            .to_array()
    });
    let wind = view.weather.wind();
    let dir = PathBuf::from(std::env::var_os("TERRA_CAPTURE_DIR").unwrap());
    let name = format!("wind-basin-{:02}", seq.scheduled);
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_vec(&serde_json::json!({
            "frame":view.frame.0, "seconds":view.time.elapsed_secs(),
            "polygon":pixels, "wind":wind, "origin":ORIGIN,
            "publication":view.flow.publications,"volume":units(view,0,[8,base,8])
        }))
        .unwrap(),
    )
    .unwrap();
    seq.scheduled += 1;
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(dir.join(format!("{name}.png"))))
        .observe(
            |_: On<ScreenshotCaptured>,
             mut seq: ResMut<Sequence>,
             mut exit: MessageWriter<AppExit>| {
                seq.saved += 1;
                if seq.saved == FRAMES {
                    info!("Captured successive windy basin frames");
                    exit.write(AppExit::Success);
                }
            },
        );
}
