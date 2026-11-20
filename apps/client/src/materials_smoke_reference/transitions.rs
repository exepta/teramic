//! A real climate boundary with interleaved turf IDs, using the normal mesher.
use super::*;

#[derive(Clone, Copy)]
struct Boundary {
    center: [i64; 2],
    axis: usize,
    base: i64,
    biome: usize,
    material: u16,
}
#[derive(Resource)]
struct TransitionInspection {
    boundaries: [Boundary; 2],
    samples: [Vec<([i64; 3], u16)>; 2],
    spans: [[f64; 2]; 2],
    shot: usize,
    placed: bool,
    pending: bool,
    started: Instant,
    settled: Instant,
}
pub(super) fn install(app: &mut App) {
    app.add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}
fn boundary(terrain: &terra_world::TerraGenerator, biome: usize, material: u16) -> Boundary {
    let weight = |x, z| terrain.biome_at(x as f64, z as f64).weights.values()[biome];
    let mut best = (f64::INFINITY, [0_i64; 2], 0);
    for z in (-4096..=4096).step_by(128) {
        for x in (-4096..=4096).step_by(128) {
            let w = weight(x, z);
            if !(0.20..0.75).contains(&w) {
                continue;
            }
            let gradients = [
                (weight(x + 48, z) - weight(x - 48, z)).abs(),
                (weight(x, z + 48) - weight(x, z - 48)).abs(),
            ];
            let axis = usize::from(gradients[1] > gradients[0]);
            let score = (w - 0.42).abs() - gradients[axis] * 0.6;
            if score < best.0 {
                best = (score, [x, z], axis);
            }
        }
    }
    assert!(
        best.0.is_finite(),
        "Missing real biome transition for {biome}"
    );
    // Align to a complete section, preserving genuine global-coordinate tint.
    let center = best.1.map(|v| v.div_euclid(16) * 16);
    let base =
        ((terrain.surface_height(center[0] as f64, center[1] as f64) / 16.).ceil() as i64 + 4) * 16;
    Boundary {
        center,
        axis: best.2,
        base,
        biome,
        material,
    }
}
fn setup(
    mut commands: Commands,
    terrain: Res<WorldTerrain>,
    mut config: ResMut<terra_core::GameConfig>,
    mut gizmos: ResMut<bevy::gizmos::config::GizmoConfigStore>,
) {
    config.client.fov_degrees = 65.;
    gizmos
        .config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>()
        .0
        .enabled = false;
    gizmos.config_mut::<ChunkBorderGizmos>().0.enabled = false;
    let boundaries = [boundary(&terrain.0, 8, 306), boundary(&terrain.0, 9, 400)];
    let samples = boundaries.map(|boundary| cells(boundary, &terrain.0));
    let spans = std::array::from_fn(|i| {
        samples[i].iter().fold([1.0_f64, 0.0_f64], |span, &(p, _)| {
            let w = terrain
                .0
                .biome_at(p[0] as f64, p[2] as f64)
                .weights
                .values()[boundaries[i].biome];
            [span[0].min(w), span[1].max(w)]
        })
    });
    commands.insert_resource(TransitionInspection {
        boundaries,
        samples,
        spans,
        shot: 0,
        placed: false,
        pending: false,
        started: Instant::now(),
        settled: Instant::now(),
    });
}
fn cells(boundary: Boundary, terrain: &terra_world::TerraGenerator) -> Vec<([i64; 3], u16)> {
    let mut result = Vec::new();
    for along in -48..48 {
        for across in -9..9 {
            let mut p = [boundary.center[0], boundary.base, boundary.center[1]];
            p[boundary.axis * 2] += along;
            p[(1 - boundary.axis) * 2] += across;
            let weight =
                terrain.biome_at(p[0] as f64, p[2] as f64).weights.values()[boundary.biome];
            // The three lanes compare the same global climate on meadow turf,
            // a real threshold switch and the fantasy turf. No fake tint ramp.
            let material = if across < -3 {
                1
            } else if across >= 3 || weight >= 0.42 {
                boundary.material
            } else {
                1
            };
            result.push((p, material));
        }
    }
    result
}
fn camera(boundary: Boundary) -> Transform {
    let mut p = Vec3::new(
        boundary.center[0] as f32,
        boundary.base as f32,
        boundary.center[1] as f32,
    );
    let target = p + Vec3::new(0., 0.5, 0.);
    p[boundary.axis * 2] += 25.;
    p[(1 - boundary.axis) * 2] -= 65.;
    p.y += 58.;
    Transform::from_translation(p).looking_at(target, Vec3::Y)
}
#[allow(clippy::too_many_arguments)]
fn run(
    mut commands: Commands,
    mut state: ResMut<TransitionInspection>,
    mut world: View,
    mut edits: MessageWriter<BlockEditRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.shot >= 2 {
        return;
    }
    if state.started.elapsed() > Duration::from_secs(200) || world.render.error.is_some() {
        error!(error=?world.render.error,"Biome transition gallery failed or timed out");
        state.shot = 2;
        exit.write(AppExit::error());
        return;
    }
    let boundary = state.boundaries[state.shot];
    let pose = camera(boundary);
    for mut camera in &mut world.camera {
        *camera = pose;
    }
    world.focus.0 = pose.translation.to_array().map(f64::from);
    if !world.render.ready || state.pending {
        return;
    }
    let samples = &state.samples[state.shot];
    if !state.placed {
        if samples
            .iter()
            .any(|&(p, _)| world.chunks.block_at(p).is_none())
        {
            return;
        }
        for &(position, material) in samples {
            edits.write(BlockEditRequest {
                position,
                material,
                expected_material: world.chunks.block_at(position).unwrap(),
            });
        }
        state.placed = true;
        state.settled = Instant::now();
        return;
    }
    if state.settled.elapsed() < Duration::from_secs(4)
        || samples.iter().any(|&(p, m)| {
            let section = global_to_section(p).0;
            world.chunks.block_at(p) != Some(m)
                || world
                    .rendered
                    .sections
                    .get(&section)
                    .is_none_or(|s| s.mesh_epoch != world.journal.mesh_epoch(section))
        })
    {
        return;
    }
    let name = if boundary.biome == 8 {
        "spirit"
    } else {
        "blood"
    };
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-biome-transitions"));
    if std::fs::create_dir_all(&directory).is_err() {
        exit.write(AppExit::error());
        return;
    }
    let report = serde_json::json!({"status":"passed","biome":name,"center":boundary.center,
        "base":boundary.base,"axis":boundary.axis,"real_climate":true,"fixture_cells":samples.len(),
        "weight_min":state.spans[state.shot][0],
        "weight_max":state.spans[state.shot][1],
        "turf_ids":[1,boundary.material],"lanes":["meadow","material switch","fantasy"]});
    if std::fs::write(
        directory.join(format!("biome-transition-{name}.json")),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .is_err()
    {
        exit.write(AppExit::error());
        return;
    }
    commands.spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("biome-transition-{name}.png"))))
        .observe(|capture: On<ScreenshotCaptured>, mut state: ResMut<TransitionInspection>,mut exit: MessageWriter<AppExit>| {
            if capture.image.width() < 1000 { exit.write(AppExit::error()); return; }
            state.shot += 1; state.placed = false; state.pending = false; state.settled = Instant::now();
            if state.shot == 2 { info!("Biome transition gallery passed: real Spirit/Blood climate boundaries, meadow/switch/fantasy lanes");exit.write(AppExit::Success); }
        });
    state.pending = true;
}
