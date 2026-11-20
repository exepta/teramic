//! Near-surface camera crossing through the real water mesher and optical pass.
use super::*;
#[derive(Resource)]
struct WaterlineInspection {
    base: i64,
    stage: usize,
    placed: bool,
    filled: bool,
    pending: bool,
    started: Instant,
    settled: Instant,
    baseline: Option<(u64, u64, u64, u64)>,
}
pub(super) fn install(app: &mut App) {
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(WaterlineInspection {
            base: 0,
            stage: 0,
            placed: false,
            filled: false,
            pending: false,
            started: Instant::now(),
            settled: Instant::now(),
            baseline: None,
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}
fn pose(base: i64, stage: usize) -> Transform {
    let level = base as f32 + 2.;
    let eye = level + if stage.is_multiple_of(2) { 0.03 } else { -0.03 };
    // Cross one real4m near-field boundary, then return. The six captures
    // exercise moving-world provenance as well as the optical waterline.
    let x = if stage / 2 == 1 { 20.5 } else { 16.5 };
    Transform::from_xyz(x, eye, 12.).looking_at(Vec3::new(x, eye - 0.12, 24.), Vec3::Y)
}
fn setup(
    mut state: ResMut<WaterlineInspection>,
    terrain: Res<WorldTerrain>,
    mut focus: ResMut<ChunkFocus>,
    mut camera: Query<&mut Transform, With<TerraCamera>>,
    mut config: ResMut<terra_core::GameConfig>,
    mut gizmos: ResMut<bevy::gizmos::config::GizmoConfigStore>,
) {
    state.base = ((terrain.0.surface_height(16., 16.) / 16.).ceil() as i64 + 3) * 16;
    config.world.water = true;
    config.client.fov_degrees = 65.;
    config.client.water_quality = terra_core::WaterQuality::Low;
    let p = pose(state.base, 0);
    focus.0 = p.translation.to_array().map(f64::from);
    for mut c in &mut camera {
        *c = p;
    }
    gizmos
        .config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>()
        .0
        .enabled = false;
    gizmos.config_mut::<ChunkBorderGizmos>().0.enabled = false;
}
#[derive(SystemParam)]
struct PoolView<'w, 's> {
    chunks: Res<'w, ChunkStore>,
    journal: ResMut<'w, EditJournal>,
    render: Res<'w, RenderStatus>,
    water: Res<'w, terra_render::WaterStatus>,
    underwater: Res<'w, terra_render::UnderwaterState>,
    particle: Res<'w, terra_render::ParticleFluidInput>,
    particle_gpu: Res<'w, terra_render::ParticleFluidStatus>,
    config: ResMut<'w, terra_core::GameConfig>,
    focus: ResMut<'w, ChunkFocus>,
    camera: Query<'w, 's, (&'static mut Transform, &'static Projection), With<TerraCamera>>,
    names: Query<'w, 's, &'static Name>,
    far: Res<'w, terra_render::particle_fluids::FarFluidSections>,
}
fn cells(base: i64) -> Vec<([i64; 3], u16)> {
    let mut result = Vec::new();
    for x in 7..=25 {
        for z in 7..=25 {
            result.push(([x, base - 1, z], 2));
            if x == 7 || x == 25 || z == 7 || z == 25 {
                for y in base..=base + 2 {
                    result.push(([x, y, z], 3));
                }
            }
        }
    }
    result
}
fn run(
    mut commands: Commands,
    mut state: ResMut<WaterlineInspection>,
    mut world: PoolView,
    mut edits: MessageWriter<BlockEditRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.stage >= 6 {
        return;
    }
    if state.started.elapsed() > Duration::from_secs(150) || world.render.error.is_some() {
        error!("Waterline fixture failed: {:?}", world.render.error);
        state.stage = 6;
        exit.write(AppExit::error());
        return;
    }
    let p = pose(state.base, state.stage);
    for (mut camera, _) in &mut world.camera {
        *camera = p;
    }
    world.focus.0 = p.translation.to_array().map(f64::from);
    let quality = [
        terra_core::WaterQuality::Low,
        terra_core::WaterQuality::Medium,
        terra_core::WaterQuality::High,
    ][state.stage / 2];
    if world.config.client.water_quality != quality {
        world.config.client.water_quality = quality;
        state.settled = Instant::now();
    }
    if !world.render.ready {
        return;
    }
    let blocks = cells(state.base);
    if !state.placed {
        if blocks
            .iter()
            .any(|(p, _)| world.chunks.block_at(*p).is_none())
        {
            return;
        }
        for &(position, material) in &blocks {
            edits.write(BlockEditRequest {
                position,
                expected_material: world.chunks.block_at(position).unwrap(),
                material,
            });
        }
        state.placed = true;
        state.settled = Instant::now();
        return;
    }
    if blocks
        .iter()
        .any(|&(p, id)| world.chunks.block_at(p) != Some(id))
    {
        return;
    }
    if !state.filled {
        for x in 8..=24 {
            for z in 8..=24 {
                for y in state.base..state.base + 2 {
                    world
                        .journal
                        .set_fluid(
                            [x, y, z],
                            terra_world::water::FluidCell {
                                amount: 8,
                                source: x == 16 && z == 16 && y == state.base + 1,
                                flow: [0, 0],
                            },
                        )
                        .unwrap();
                }
            }
        }
        state.filled = true;
        state.settled = Instant::now();
        return;
    }
    if state.pending || state.settled.elapsed() < Duration::from_millis(1500) || !world.water.ready
    {
        return;
    }
    // Publication is asynchronous. Wait for the four real surface owners,
    // rather than mistaking an incomplete accepted cache for a whole pool.
    if [8, 24].into_iter().any(|x| {
        [8, 24].into_iter().any(|z| {
            let owner = terra_voxel::global_to_section([x, state.base + 1, z]).0;
            world
                .far
                .sections
                .get(&owner)
                .and_then(|section| section.published_water.as_ref())
                .is_none()
        })
    }) {
        return;
    }
    if world.underwater.submerged == state.stage.is_multiple_of(2) {
        error!(
            "Waterline medium detection mismatch at stage {}",
            state.stage
        );
        exit.write(AppExit::error());
        state.stage = 6;
        return;
    }
    for (_, projection) in &world.camera {
        if let Projection::Perspective(p) = projection {
            assert!(
                p.near <= 0.0051,
                "near plane must not cut the liquid interface"
            );
        }
    }
    assert!(world.particle_gpu.error.is_none());
    assert!(
        world.particle.particles.iter().all(|p| p.kind > 1),
        "A stable reservoir must never become camera-local water particles"
    );
    assert_eq!(&world.particle.target_mass[..2], &[0.0, 0.0]);
    assert_eq!(
        world.particle_gpu.active_particles, 0,
        "Moving the camera must not disturb this saved water body"
    );

    // Verify the actual unchanged source volume independently of rendered data.
    use std::hash::{Hash, Hasher};
    let mut source_hash = std::collections::hash_map::DefaultHasher::new();
    let mut source_count = 0;
    let mut units = 0_u64;
    for x in 8..=24 {
        for z in 8..=24 {
            for y in state.base..state.base + 2 {
                let cell = world.journal.fluid([x, y, z]).expect("Pool fluid vanished");
                assert_eq!(cell.amount, 8);
                assert_eq!(cell.source, x == 16 && z == 16 && y == state.base + 1);
                assert_eq!(cell.flow, [0, 0]);
                units += u64::from(cell.amount);
                source_count += usize::from(cell.source);
                (x, y, z, cell.amount, cell.source, cell.flow).hash(&mut source_hash);
            }
        }
    }
    assert_eq!(units, 17 * 17 * 2 * 8);
    assert_eq!(source_count, 1);
    let source_hash = source_hash.finish();
    let level = state.base as f32 + 2.0;
    let mut patches: Vec<_> = world
        .far
        .sections
        .iter()
        .filter_map(|(owner, section)| {
            let patch = section.published_water.as_ref()?;
            let origin = owner.origin().ok()?;
            patch
                .positions
                .iter()
                .any(|v| {
                    let [x, y, z] = std::array::from_fn(|i| v[i] + origin[i] as f32);
                    (8.0..=25.0).contains(&x)
                        && (8.0..=25.0).contains(&z)
                        && (y - level).abs() < 0.002
                })
                .then_some((*owner, patch))
        })
        .collect();
    patches.sort_by_key(|(owner, _)| (owner.x, owner.y, owner.z));
    assert!(
        !patches.is_empty(),
        "A ready water body needs its published surface"
    );
    let mut geometry_hash = std::collections::hash_map::DefaultHasher::new();
    let mut referenced_vertices = Vec::new();
    let mut surface_area = 0.0;
    let mut interior_heights = Vec::new();
    for (owner, patch) in &patches {
        let origin = owner.origin().unwrap().map(|v| v as f32);
        (owner.x, owner.y, owner.z).hash(&mut geometry_hash);
        patch.indices.hash(&mut geometry_hash);
        for p in &patch.positions {
            p.map(f32::to_bits).hash(&mut geometry_hash);
        }
        for triangle in patch.indices.chunks_exact(3) {
            let positions = std::array::from_fn::<_, 3, _>(|i| {
                let p = patch.positions[triangle[i] as usize];
                Vec3::from_array(std::array::from_fn(|i| p[i] + origin[i]))
            });
            let normal = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
            if normal.y.abs() < normal.length() * 0.99 {
                continue;
            }
            let center = (positions[0] + positions[1] + positions[2]) / 3.0;
            if (center.y - level).abs() > 0.002
                || !(8.0..=25.0).contains(&center.x)
                || !(8.0..=25.0).contains(&center.z)
            {
                continue;
            }
            surface_area += normal.y.abs() * 0.5;
            for p in positions {
                assert!(
                    (p.y - level).abs() < 0.002,
                    "The published pool surface has a false peak"
                );
                referenced_vertices.push(p.to_array());
                if (9.0..=24.0).contains(&p.x) && (9.0..=24.0).contains(&p.z) {
                    interior_heights.push(p.y);
                }
            }
        }
    }
    assert!(
        (288.5..289.5).contains(&surface_area),
        "Pool surface has holes: {surface_area}"
    );
    assert!(!interior_heights.is_empty());
    let geometry_hash = geometry_hash.finish();
    let current = (
        geometry_hash,
        source_hash,
        world.far.sampled_cells,
        world.far.body_patches,
    );
    if let Some(baseline) = state.baseline {
        assert_eq!(
            current, baseline,
            "A camera/quality change must neither remesh nor resample this reservoir"
        );
    } else {
        state.baseline = Some(current);
    }
    let legacy_surfaces = world
        .names
        .iter()
        .filter(|n| {
            n.as_str() == "Draining water surface"
                || n.as_str()
                    .strip_prefix("Water ")
                    .is_some_and(|s| s.starts_with(|c: char| c.is_ascii_digit() || c == '-'))
        })
        .count();
    let legacy_spray = world
        .names
        .iter()
        .filter(|n| matches!(n.as_str(), "Waterfall spray" | "Waterfall mist volume"))
        .count();
    let body_surfaces = world
        .names
        .iter()
        .filter(|n| n.as_str().starts_with("Natural water body "))
        .count();
    assert_eq!((legacy_surfaces, legacy_spray), (0, 0));
    assert!(body_surfaces >= patches.len());
    let dir = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let report = serde_json::json!({
        "stage":state.stage,"quality":format!("{quality:?}"),"focus":world.focus.0,"pool_surface_y":level,
        "underwater":world.underwater.submerged,"underwater_blend":world.underwater.blend,
        "water_ready":world.water.ready,"visible_tiles":world.water.visible_tiles,
        "body_surfaces":body_surfaces,"published_patches":patches.len(),
        "surface_area":surface_area,"published_vertices":referenced_vertices,
        "geometry_hash":format!("{geometry_hash:016x}"),"source_hash":format!("{source_hash:016x}"),
        "source_count":source_count,"water_units":units,
        "active_particles":world.particle_gpu.active_particles,"particle_mass":world.particle.target_mass,
        "particle_water_seeds":world.particle.particles.iter().filter(|p|p.kind<2).count(),
        "sampled_cells":world.far.sampled_cells,"body_patches":world.far.body_patches,
        "legacy_surfaces":legacy_surfaces,"legacy_spray":legacy_spray,
        "far_revision":world.far.revision,
    });
    std::fs::write(
        dir.join(format!("waterline-{}.json", state.stage)),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    info!(
        stage = state.stage,
        surface_area,
        patches = patches.len(),
        "Waterline stable body verified"
    );
    commands.spawn(Screenshot::primary_window())
        .observe(save_to_disk(dir.join(format!("waterline-{}.png",state.stage))))
        .observe(|_:On<ScreenshotCaptured>,mut state:ResMut<WaterlineInspection>,mut exit:MessageWriter<AppExit>| {
            state.stage+=1;state.pending=false;state.settled=Instant::now();
            if state.stage==6 {
                info!("Waterline gallery passed: stable body, source volume, no camera particles, above/below at all three qualities");
                exit.write(AppExit::Success);
            }
        });
    state.pending = true;
}
