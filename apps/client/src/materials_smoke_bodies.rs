//! Generated reservoirs under a moving detached camera; no fluid or terrain edits.
use super::*;
use bevy::{
    camera::primitives::Aabb,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::collections::BTreeSet;
use terra_core::footsteps::WaterMotion;
use terra_render::particle_fluids::{FarFluidSections, ParticleFluidInput, ParticleFluidStatus};
use terra_world::TerraGenerator;

const SHOTS: usize = 8;
#[derive(Clone, Copy)]
struct Site {
    center: [f64; 2],
    level: f64,
    bed: f64,
    flow: [f32; 2],
}
impl Site {
    fn owner(self) -> terra_voxel::SectionPos {
        global_to_section([
            self.center[0].floor() as i64,
            self.level.floor() as i64,
            self.center[1].floor() as i64,
        ])
        .0
    }
}
#[derive(Resource)]
struct Inspection {
    started: Instant,
    since: Instant,
    last_report: Instant,
    sites: Option<[Site; 2]>,
    stage: usize,
    pending: bool,
    particle_revision: Option<u64>,
    baseline: [Option<(Entity, Handle<Mesh>)>; 2],
    focus_domains: BTreeSet<[i32; 3]>,
    motion_events: usize,
}
pub(super) fn install(app: &mut App) {
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(Inspection {
            started: Instant::now(),
            since: Instant::now(),
            last_report: Instant::now(),
            sites: None,
            stage: 0,
            pending: false,
            particle_revision: None,
            baseline: [None, None],
            focus_domains: BTreeSet::new(),
            motion_events: 0,
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}
fn sample(terrain: &TerraGenerator, center: [f64; 2]) -> Option<Site> {
    let water = terrain
        .surface_column(center[0], center[1])
        .water_columns()
        .intervals[0]?;
    (water.depth >= 1.0).then_some(Site {
        center,
        level: water.level,
        bed: water.bed,
        flow: terrain
            .river_at(center[0], center[1])
            .map_or([0.; 2], |river| river.flow),
    })
}
fn sites(terrain: &TerraGenerator) -> Option<[Site; 2]> {
    let ocean = [[-829.5, 1641.5], [-1051.5, 1273.5], [0.5, 0.5]]
        .into_iter()
        .find_map(|p| {
            let site = sample(terrain, p)?;
            (site.level - site.bed >= 3.0 && (site.level - terrain.config().sea_level).abs() < 0.01)
                .then_some(site)
        })?;
    for anchor in [[-1277.5, 1161.5], [-1051.5, 1273.5], [-829.5, 1641.5]] {
        for z in (-48..=48).step_by(2) {
            for x in (-48..=48).step_by(2) {
                let p = [anchor[0] + f64::from(x), anchor[1] + f64::from(z)];
                let Some(river) = terrain.river_at(p[0], p[1]) else {
                    continue;
                };
                if river.covered || river.distance > river.width * 0.35 {
                    continue;
                }
                if let Some(site) = sample(terrain, p) {
                    return Some([ocean, site]);
                }
            }
        }
    }
    None
}
fn pose(site: Site, phase: usize) -> Transform {
    let center = Vec3::new(
        site.center[0] as f32,
        site.level as f32,
        site.center[1] as f32,
    );
    if phase == 2 {
        let direction = Vec3::new(site.flow[0], 0., site.flow[1])
            .try_normalize()
            .unwrap_or(Vec3::Z);
        let eye = center - Vec3::Y * 0.35;
        return Transform::from_translation(eye).looking_at(eye + direction * 10., Vec3::Y);
    }
    let offset = if phase == 1 {
        Vec3::new(37., 38., 17.)
    } else {
        Vec3::new(1., 8., -16.)
    };
    Transform::from_translation(center + offset).looking_at(center, Vec3::Y)
}
fn travel_seconds(phase: usize) -> f32 {
    [0., 2.5, 1., 2.][phase]
}
fn moving_pose(site: Site, phase: usize, seconds: f32) -> Transform {
    let target = pose(site, phase);
    if phase == 0 {
        return target;
    }
    let previous = pose(site, phase - 1);
    let progress = (seconds / travel_seconds(phase)).clamp(0., 1.);
    let progress = progress * progress * (3. - 2. * progress);
    Transform {
        translation: previous.translation.lerp(target.translation, progress),
        rotation: previous.rotation.slerp(target.rotation, progress),
        ..default()
    }
}
fn setup(
    mut state: ResMut<Inspection>,
    terrain: Res<WorldTerrain>,
    mut focus: ResMut<ChunkFocus>,
    mut cameras: Query<&mut Transform, With<TerraCamera>>,
    mut config: ResMut<terra_core::GameConfig>,
    mut gizmos: ResMut<bevy::gizmos::config::GizmoConfigStore>,
) {
    state.sites = sites(&terrain.0);
    config.client.fov_degrees = 65.;
    config.client.water_quality = terra_core::WaterQuality::High;
    if let Some(sites) = state.sites {
        let transform = pose(sites[0], 0);
        focus.0 = transform.translation.to_array().map(f64::from);
        for mut camera in &mut cameras {
            *camera = transform;
        }
    }
    gizmos
        .config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>()
        .0
        .enabled = false;
    gizmos.config_mut::<ChunkBorderGizmos>().0.enabled = false;
}
type BodyMeshData = (
    Entity,
    &'static Name,
    &'static Mesh3d,
    &'static Transform,
    Option<&'static Aabb>,
);
type BodyMeshFilter = (Without<TerraCamera>, Without<terra_player::PlayerBody>);

#[derive(SystemParam)]
struct View<'w, 's> {
    render: Res<'w, RenderStatus>,
    water: Res<'w, terra_render::WaterStatus>,
    underwater: Res<'w, terra_render::UnderwaterState>,
    input: Res<'w, ParticleFluidInput>,
    gpu: Res<'w, ParticleFluidStatus>,
    far: Res<'w, FarFluidSections>,
    focus: ResMut<'w, ChunkFocus>,
    cameras: Query<'w, 's, &'static mut Transform, With<TerraCamera>>,
    bodies: Query<
        'w,
        's,
        &'static mut Transform,
        (With<terra_player::PlayerBody>, Without<TerraCamera>),
    >,
    meshes: Query<'w, 's, BodyMeshData, BodyMeshFilter>,
    motions: MessageReader<'w, 's, WaterMotion>,
}
fn run(
    mut commands: Commands,
    mut state: ResMut<Inspection>,
    mut view: View,
    mut exit: MessageWriter<AppExit>,
) {
    if state.stage >= SHOTS {
        return;
    }
    if state.started.elapsed() > Duration::from_secs(160) || view.render.error.is_some() {
        error!(stage=state.stage,error=?view.render.error,"Generated water body inspection failed");
        state.stage = SHOTS;
        exit.write(AppExit::error());
        return;
    }
    let Some(sites) = state.sites else {
        error!("Generated water inspection could not locate ocean and river");
        state.stage = SHOTS;
        exit.write(AppExit::error());
        return;
    };
    let body = state.stage / 4;
    let phase = state.stage % 4;
    let site = sites[body];
    let elapsed = state.since.elapsed().as_secs_f32();
    let camera = moving_pose(site, phase, elapsed);
    for mut transform in &mut view.cameras {
        *transform = camera;
    }
    // The underwater view is a detached optical camera. Keep the physical actor
    // in the air throughout, so camera travel cannot legitimately inject wakes.
    for mut transform in &mut view.bodies {
        transform.translation = Vec3::new(
            site.center[0] as f32,
            site.level as f32 + 80.,
            site.center[1] as f32,
        );
    }
    view.focus.0 = camera.translation.to_array().map(f64::from);
    state.focus_domains.insert(
        camera
            .translation
            .to_array()
            .map(|v| (v / 4.).floor() as i32),
    );
    let motions = view.motions.read().count();
    if state.particle_revision.is_some() {
        state.motion_events += motions;
    }
    if state.last_report.elapsed() >= Duration::from_secs(10) {
        info!(
            stage = state.stage,
            terrain_ready = view.render.ready,
            water_ready = view.water.ready,
            body_patches = view.far.body_patches,
            sampled_cells = view.far.sampled_cells,
            "Generated water inspection progress"
        );
        state.last_report = Instant::now();
    }
    if state.pending
        || elapsed < travel_seconds(phase) + 1.
        || !view.render.ready
        || !view.water.ready
    {
        return;
    }
    let owner = site.owner();
    let Some(patch) = view
        .far
        .sections
        .get(&owner)
        .and_then(|s| s.published_water.as_ref())
    else {
        return;
    };
    let expected_name = format!("Natural water body {},{},{}", owner.x, owner.y, owner.z);
    let Some((entity, _, mesh, transform, Some(bounds))) = view
        .meshes
        .iter()
        .find(|(_, name, _, _, _)| name.as_str() == expected_name)
    else {
        return;
    };
    assert!(!patch.indices.is_empty());
    let origin = Vec3::from_array(owner.origin().unwrap().map(|v| v as f32));
    assert!(transform.translation.abs_diff_eq(origin, 0.001));
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    for position in &patch.positions {
        minimum = minimum.min(Vec3::from_array(*position));
        maximum = maximum.max(Vec3::from_array(*position));
    }
    // Culling includes the bounded shader wave displacement; the actual worker
    // positions still define the exact water level and shoreline below.
    let padding = Vec3::splat(0.025) + Vec3::Y * 0.24;
    assert!(Vec3::from(bounds.center - bounds.half_extents).abs_diff_eq(minimum - padding, 0.005));
    assert!(Vec3::from(bounds.center + bounds.half_extents).abs_diff_eq(maximum + padding, 0.005));
    let world_min = minimum + origin;
    let world_max = maximum + origin;
    assert!(site.level as f32 >= world_min.y - 0.05 && site.level as f32 <= world_max.y + 0.05);
    assert_eq!(view.input.target_mass[..2], [0., 0.]);
    assert!(!view.input.particles.iter().any(|p| p.kind < 2));
    assert_eq!(
        view.gpu.active_particles, 0,
        "Natural reservoirs must not become camera-relative particle clouds"
    );
    let revision = *state.particle_revision.get_or_insert(view.input.revision);
    assert_eq!(
        view.input.revision, revision,
        "Pure camera flight re-seeded the particle solver"
    );
    assert_eq!(
        state.motion_events, 0,
        "Detached flight generated physical water contact events"
    );
    assert_eq!(
        view.far.sampled_cells, 0,
        "Generated water was sampled again on the main thread"
    );
    assert!(view.far.body_patches > 0);
    assert_eq!(view.underwater.submerged, phase == 2);
    if phase == 0 {
        state.baseline[body] = Some((entity, mesh.0.clone()));
    } else if phase == 3 {
        let baseline = state.baseline[body].as_ref().unwrap();
        assert_eq!(
            entity, baseline.0,
            "Camera flight recreated a retained water entity"
        );
        assert_eq!(
            mesh.0, baseline.1,
            "Camera flight remeshed unchanged reservoir water"
        );
    }
    let dir = PathBuf::from(std::env::var_os("TERRA_CAPTURE_DIR").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let name = format!("water-body-{}", state.stage);
    std::fs::write(dir.join(format!("{name}.json")),serde_json::to_vec_pretty(&serde_json::json!({
        "stage":state.stage,"body":if body==0{"ocean"}else{"river"},"phase":phase,"camera":camera.translation.to_array(),
        "center":site.center,"level":site.level,"bed":site.bed,"flow":site.flow,"owner":[owner.x,owner.y,owner.z],
        "published_vertices":patch.positions.len(),"published_triangles":patch.indices.len()/3,"bounds_min":world_min.to_array(),"bounds_max":world_max.to_array(),
        "particle_revision":view.input.revision,"active_particles":view.gpu.active_particles,"target_mass":view.input.target_mass,
        "sampled_cells":view.far.sampled_cells,"body_patches":view.far.body_patches,"water_motion_events":state.motion_events,
        "focus_domains":state.focus_domains.len(),"submerged":view.underwater.submerged,"retained_return_mesh":phase==3,"generated_without_edits":true,
    })).unwrap()).unwrap();
    commands.spawn(Screenshot::primary_window())
        .observe(save_to_disk(dir.join(format!("{name}.png"))))
        .observe(|capture:On<ScreenshotCaptured>,mut state:ResMut<Inspection>,mut exit:MessageWriter<AppExit>|{
            // CPU mesh readiness can precede the first transmissive GPU pipeline.
            // Hold the first camera pose until the actual reservoir is on screen.
            if state.stage == 0 && !capture.image.data.as_deref().is_some_and(|bytes| {
                let pixels = bytes.chunks_exact(4).step_by(127);
                let total = pixels.len();
                // The opaque horizon is grey even after it clears black.
                // Cyan deep water has a strong red/blue separation in RGB or BGRA.
                total > 0 && pixels.filter(|p| p[0].abs_diff(p[2]) > 36).count() * 2 > total
            }) {
                state.pending = false;
                state.since = Instant::now();
                return;
            }
            state.stage+=1;state.pending=false;state.since=Instant::now();
            if state.stage==SHOTS {
                assert!(state.focus_domains.len()>12);
                info!(captures=SHOTS,focus_domains=state.focus_domains.len(),"Generated water bodies passed: ocean and river above/below, camera flight and return, retained meshes, no particle reseeding or contact impulses");
                exit.write(AppExit::Success);
            }
        });
    state.pending = true;
}
