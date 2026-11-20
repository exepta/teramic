//! Real voxel sightlines for native Blood Forest photographs only.
use super::*;
use std::collections::BTreeMap;
use terra_world::edits::EditJournal;

#[derive(Resource)]
pub(super) struct Evidence {
    targets: Vec<Vec3>,
    picker: bool,
    captures: BTreeMap<usize, serde_json::Value>,
}

impl Evidence {
    pub(super) fn water(targets: Vec<Vec3>) -> Self {
        Self {
            targets,
            picker: false,
            captures: BTreeMap::new(),
        }
    }
}

/// Native discovery only. Cache generated blocks rather than regenerating a
/// tree neighbourhood for every voxel visited by the camera rays.
struct Probe {
    generator: SectionGenerator,
    sections: BTreeMap<SectionPos, terra_voxel::BlockSection>,
    started: Instant,
}
impl Probe {
    fn new(terrain: &TerraGenerator, started: Instant) -> Self {
        Self {
            generator: SectionGenerator::new(terrain.clone()),
            sections: BTreeMap::new(),
            started,
        }
    }
    fn block(&mut self, p: [i64; 3]) -> Option<u16> {
        if self.started.elapsed() > Duration::from_secs(30) {
            return None;
        }
        let (section, local) = terra_voxel::global_to_section(p);
        let full = self.sections.len() >= 128;
        let data = match self.sections.entry(section) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                if full {
                    return None;
                }
                entry.insert(self.generator.generate(section).ok()?)
            }
        };
        data.block(local).ok()
    }
}

fn clear(eye: Vec3, target: Vec3, mut block: impl FnMut([i64; 3]) -> Option<u16>) -> bool {
    let delta = target - eye;
    let length = delta.length();
    if !length.is_finite() || !(0.05..=120.0).contains(&length) {
        return false;
    }
    // Nonopaque leaves still hide the photo. Missing data is never evidence of
    // a free ray; water is rendered separately and may occupy an Air cell.
    terra_voxel::blocks::raycast_blocks(
        eye.to_array().map(f64::from),
        delta.to_array().map(f64::from),
        f64::from(length),
        |p| Some(u16::from(!matches!(block(p), Some(0 | 27)))),
    )
    .is_none()
}

pub(super) fn water_view(
    terrain: &TerraGenerator,
    p: [i64; 2],
    started: Instant,
) -> Option<(Transform, Vec<Vec3>)> {
    let mut targets = Vec::with_capacity(5);
    for [dx, dz] in [[0., 0.], [-1.25, 0.], [1.25, 0.], [0., -1.25], [0., 1.25]] {
        let x = p[0] as f64 + 0.5 + dx;
        let z = p[1] as f64 + 0.5 + dz;
        let water = terrain.water_column(x, z)?;
        if water.depth < 0.35 {
            return None;
        }
        targets.push(Vec3::new(x as f32, water.level as f32 + 0.08, z as f32));
    }
    let target = targets[0];
    if targets.iter().any(|p| (p.y - target.y).abs() > 0.35) {
        return None;
    }
    let mut probe = Probe::new(terrain, started);
    // Prefer an oblique view with shore context and reflections. Every pose
    // must see all five real water points; overhead remains the safe fallback.
    for offset in [
        Vec3::new(6., 7., 10.),
        Vec3::new(-6., 7., -10.),
        Vec3::new(0., 5.5, 8.),
        Vec3::new(0., 5.5, -8.),
        Vec3::new(0., 4.5, 1.),
    ] {
        let eye = target + offset;
        if probe.block(eye.floor().as_i64vec3().to_array()) == Some(0)
            && targets.iter().all(|&p| clear(eye, p, |p| probe.block(p)))
        {
            return Some((
                Transform::from_translation(eye).looking_at(target, Vec3::Y),
                targets,
            ));
        }
    }
    None
}

pub(super) fn picker_views(world: &mut World, terrain: &TerraGenerator) -> Option<Views> {
    let mut views = super::super::picker_views(terrain, terra_core::BiomeKind::BloodForest)?;
    let mut probe = Probe::new(terrain, Instant::now());
    let point = views.anchor;
    let ground = point.y.floor() as i64;
    let mut leaf = None;
    // Find an actual outer crown rather than aiming at hidden ground through
    // thirty metres of foliage. Every vertical column includes generated trees.
    for [dx, dz] in [
        [0, 0],
        [-8, 0],
        [8, 0],
        [0, -8],
        [0, 8],
        [-16, -16],
        [16, 16],
        [-16, 16],
        [16, -16],
    ] {
        let x = point.x.floor() as i64 + dx;
        let z = point.z.floor() as i64 + dz;
        for y in (ground..=ground + 80).rev() {
            match probe.block([x, y, z])? {
                0 | 27 => {}
                406 => {
                    if leaf.is_none_or(|old: [i64; 3]| old[1] < y) {
                        leaf = Some([x, y, z]);
                    }
                    break;
                }
                _ => break,
            }
        }
    }
    let leaf = leaf?;
    let target = Vec3::from_array(leaf.map(|v| v as f32)) + Vec3::new(0.5, 1.08, 0.5);
    for height in [18., 26., 34.] {
        for step in 0..8 {
            let angle = step as f32 * std::f32::consts::FRAC_PI_4;
            let eye = target + Vec3::new(angle.cos() * 32., height, angle.sin() * 32.);
            let camera_air = [
                Vec3::ZERO,
                Vec3::X,
                -Vec3::X,
                Vec3::Z,
                -Vec3::Z,
                Vec3::Y * 3.,
            ]
            .into_iter()
            .all(|d| probe.block((eye + d).floor().as_i64vec3().to_array()) == Some(0));
            if camera_air && clear(eye, target, |p| probe.block(p)) {
                views.overview = Transform::from_translation(eye).looking_at(target, Vec3::Y);
                views.close = views.overview;
                views.probes.push((leaf, 406));
                views.probes.push((eye.floor().as_i64vec3().to_array(), 0));
                world.insert_resource(Evidence {
                    targets: vec![target],
                    picker: true,
                    captures: BTreeMap::new(),
                });
                info!(
                    ?eye,
                    ?target,
                    ?leaf,
                    distance = eye.distance(target),
                    "Native Blood picker is above a generated crown with a clear voxel sightline"
                );
                return Some(views);
            }
        }
    }
    None
}

pub(super) fn inspect(world: &mut World, stage: usize) -> Result<serde_json::Value, String> {
    let (pose, projection, ev, ambient, fog) = world
        .query_filtered::<(
            &Transform,
            &Projection,
            &bevy::camera::Exposure,
            &AmbientLight,
            &DistanceFog,
        ), With<TerraCamera>>()
        .iter(world)
        .next()
        .map(|(p, projection, e, a, f)| {
            (
                *p,
                projection.get_clip_from_view(),
                e.ev100,
                a.clone(),
                f.clone(),
            )
        })
        .ok_or("Missing native Blood camera lighting components")?;
    let evidence = world
        .get_resource::<Evidence>()
        .ok_or("Missing native Blood view evidence")?;
    let chunks = world.resource::<ChunkStore>();
    let journal = world.resource::<EditJournal>();
    let block = |p| chunks.block_at(p).map(|id| journal.block(p).unwrap_or(id));
    if block(pose.translation.floor().as_i64vec3().to_array()) != Some(0) {
        return Err("Native Blood camera is not in loaded Air".into());
    }
    let mut rays = Vec::new();
    if stage == 0 {
        let view = pose.compute_affine().inverse();
        for &target in &evidence.targets {
            if !clear(pose.translation, target, block) {
                return Err(format!(
                    "Native Blood sightline is blocked or unloaded: {target:?}"
                ));
            }
            let clip = projection * view.transform_point3(target).extend(1.);
            let ndc = clip.truncate() / clip.w;
            if clip.w <= 0. || ndc.x.abs() > 0.85 || ndc.y.abs() > 0.85 {
                return Err("Verified Blood target is outside the central camera view".into());
            }
            let water = if evidence.picker {
                None
            } else {
                let terrain = &world.resource::<WorldTerrain>().0;
                let water = chunks
                    .water_column_at_point(
                        terrain,
                        journal,
                        (target - Vec3::Y * 0.20).to_array().map(f64::from),
                    )
                    .ok_or("The photographed target has no real loaded water span")?;
                Some(serde_json::json!({"level":water.level,"bed":water.bed,"depth":water.depth}))
            };
            rays.push(
                serde_json::json!({"target":target.to_array(),"ndc":ndc.to_array(),
                "clear_loaded_voxels":true,"water":water}),
            );
        }
    }
    let (fog_start, fog_end) = match fog.falloff {
        FogFalloff::Linear { start, end } => (Some(start), Some(end)),
        _ => (None, None),
    };
    if stage == 0
        && fog_start.is_some_and(|start| {
            evidence
                .targets
                .iter()
                .any(|p| pose.translation.distance(*p) >= start)
        })
    {
        return Err("Blood photo target lies in the distance-fog band".into());
    }
    let report = serde_json::json!({
        "camera":pose.translation.to_array(),"camera_in_loaded_air":true,
        "exposure_ev100":ev,"cave_blend":world.resource::<terra_render::CaveState>().blend,
        "ambient_brightness":ambient.brightness,"ambient_color_linear":ambient.color.to_linear().to_f32_array(),
        "fog_start_m":fog_start,"fog_end_m":fog_end,"sightlines":rays,
    });
    let mut evidence = world.resource_mut::<Evidence>();
    evidence.captures.insert(stage, report.clone());
    let filename = if evidence.picker {
        "picker-blood-forest-view.json"
    } else {
        "blood-forest-view.json"
    };
    let document = serde_json::json!({"fixture_edits":0,"captures":evidence.captures});
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-biomes"));
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    std::fs::write(
        directory.join(filename),
        serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    info!(stage,inspection=%report,"Native Blood camera and loaded sightline inspection");
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn photo_sightlines_reject_canopies_solids_missing_data_and_invalid_rays() {
        let eye = Vec3::new(0.5, 6.5, 0.5);
        let target = Vec3::new(0.5, 1.1, 0.5);
        assert!(clear(eye, target, |_| Some(0)));
        for material in [3, 405, 406] {
            assert!(!clear(eye, target, |p| Some(if p[1] == 4 {
                material
            } else {
                0
            })));
        }
        assert!(!clear(eye, target, |p| if p[1] == 4 {
            None
        } else {
            Some(0)
        }));
        assert!(!clear(eye, eye, |_| Some(0)));
        assert!(!clear(eye, eye + Vec3::Y * 129., |_| Some(0)));
    }
}
