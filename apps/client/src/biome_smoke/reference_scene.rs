//! Opt-in captures of real ecology-seven plants. Finds sites through the public
//! generator API, then checks the same cells after normal streaming/rendering.
mod birch;
use super::*;
use std::collections::{BTreeMap, HashSet};
use terra_world::generation::natural_features::CactusPlant;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Cactus,
    Birch,
    Plains,
    Desert,
    Spirit,
}
impl Kind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Cactus => "reference-cactus",
            Self::Birch => "reference-birch",
            Self::Plains => "reference-plains",
            Self::Desert => "reference-desert",
            Self::Spirit => "reference-spirit",
        }
    }
    fn weight(self, terrain: &TerraGenerator, point: [i64; 2]) -> f64 {
        let w = terrain
            .biome_at(point[0] as f64 + 0.5, point[1] as f64 + 0.5)
            .weights;
        match self {
            Self::Cactus | Self::Desert => w.desert + w.cliffted_desert,
            Self::Plains => w.plains,
            Self::Birch => w.plains + w.forest,
            Self::Spirit => w.spirit_blossom,
        }
    }
    fn accepts(self, id: u16) -> bool {
        match self {
            Self::Cactus => id == 422,
            Self::Birch => matches!(id, 9 | 10),
            Self::Plains => matches!(id, 423 | 424 | 426),
            Self::Desert => id == 428,
            Self::Spirit => matches!(id, 316 | 317 | 322),
        }
    }
}

#[derive(Resource)]
struct Inspection {
    kind: Kind,
    tile: [i64; 2],
    probes: Vec<([i64; 3], u16)>,
    cactus: Option<CactusPlant>,
    birch: Option<birch::Specimen>,
    target: Vec3,
    poses: [Transform; 2],
    captures: BTreeMap<&'static str, serde_json::Value>,
}

pub(super) fn views(world: &mut World, terrain: &TerraGenerator, kind: Kind) -> Option<Views> {
    if terrain.config().ecology_revision != 9 {
        error!("Reference flora requires ecology revision 9");
        return None;
    }
    let started = Instant::now();
    let generator = SectionGenerator::new(terrain.clone());
    let override_anchor = std::env::var("TERRA_REFERENCE_ANCHOR")
        .ok()
        .and_then(|s| serde_json::from_str::<[i64; 2]>(&s).ok());
    let mut visited = HashSet::new();
    let mut inspected = 0;
    for ring in 0_i64..=64 {
        for n in -ring..=ring {
            for [gx, gz] in [[n, -ring], [n, ring], [-ring, n], [ring, n]] {
                let point = override_anchor
                    .map_or([gx * 128, gz * 128], |p| [p[0] + gx * 32, p[1] + gz * 32]);
                if kind.weight(terrain, point) < 0.65 {
                    continue;
                }
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        if started.elapsed() > Duration::from_secs(40) || inspected >= 240 {
                            return None;
                        }
                        let tile = [point[0].div_euclid(32) + dx, point[1].div_euclid(32) + dz];
                        if !visited.insert(tile) {
                            continue;
                        }
                        inspected += 1;
                        let candidate = if matches!(kind, Kind::Birch) {
                            birch::candidate(terrain, &generator, tile)
                        } else if matches!(kind, Kind::Cactus) {
                            let mut plants = terrain.surface_cacti(tile);
                            plants.sort_by_key(|p| std::cmp::Reverse(p.height));
                            plants
                                .into_iter()
                                .find_map(|c| cactus_candidate(&generator, c))
                        } else {
                            let flora: Vec<_> = terrain
                                .surface_flora(tile)
                                .into_iter()
                                .filter(|(_, id)| kind.accepts(*id))
                                .collect();
                            if flora.len() < 3 {
                                None
                            } else {
                                let anchor = flora[flora.len() / 2].0;
                                let nearby: Vec<_> = flora
                                    .into_iter()
                                    .filter(|(p, _)| {
                                        (p[0] - anchor[0]).pow(2) + (p[2] - anchor[2]).pow(2) <= 64
                                    })
                                    .take(32)
                                    .collect();
                                if nearby.len() < 3
                                    || nearby
                                        .iter()
                                        .any(|(p, m)| generator.block_at(*p).ok() != Some(*m))
                                {
                                    None
                                } else {
                                    Some((
                                        Vec3::from_array(anchor.map(|v| v as f32))
                                            + Vec3::splat(0.5),
                                        nearby,
                                        None,
                                        None,
                                    ))
                                }
                            }
                        };
                        let Some((target, probes, cactus, birch)) = candidate else {
                            continue;
                        };
                        let Some(poses) =
                            cameras(terrain, &generator, target, cactus.as_ref(), birch.as_ref())
                        else {
                            continue;
                        };
                        info!(
                            scene = kind.name(),
                            ?tile,
                            ?target,
                            probes = probes.len(),
                            "Native reference uses real generated flora"
                        );
                        world.insert_resource(Inspection {
                            kind,
                            tile,
                            probes: probes.clone(),
                            cactus,
                            birch,
                            target,
                            poses,
                            captures: BTreeMap::new(),
                        });
                        return Some(Views {
                            overview: poses[0],
                            close: poses[1],
                            anchor: target,
                            probes,
                        });
                    }
                }
            }
        }
    }
    None
}

type Candidate = (
    Vec3,
    Vec<([i64; 3], u16)>,
    Option<CactusPlant>,
    Option<birch::Specimen>,
);
fn cactus_candidate(generator: &SectionGenerator, cactus: CactusPlant) -> Option<Candidate> {
    // Prefer a tall branched specimen so the two photographs expose scale and
    // connected arms; the complete 2–8 range is covered by generation tests.
    if !(5..=8).contains(&cactus.height) || cactus.voxel_count <= usize::from(cactus.height) {
        return None;
    }
    let mut cells = HashSet::new();
    for z in cactus.bounds[0][2]..=cactus.bounds[1][2] {
        for y in cactus.bounds[0][1]..=cactus.bounds[1][1] {
            for x in cactus.bounds[0][0]..=cactus.bounds[1][0] {
                let p = [x, y, z];
                if generator.block_at(p).ok()? == 422 {
                    cells.insert(p);
                }
            }
        }
    }
    if cells.len() != cactus.voxel_count || !connected(&cells, cactus.origin) {
        return None;
    }
    if !(0..i64::from(cactus.height))
        .all(|dy| cells.contains(&[cactus.origin[0], cactus.origin[1] + dy, cactus.origin[2]]))
    {
        return None;
    }
    let sand = [cactus.origin[0], cactus.origin[1] - 1, cactus.origin[2]];
    let air = [
        cactus.origin[0],
        cactus.origin[1] + i64::from(cactus.height),
        cactus.origin[2],
    ];
    if generator.block_at(sand).ok() != Some(4) || generator.block_at(air).ok() != Some(0) {
        return None;
    }
    let mut probes: Vec<_> = cells.into_iter().map(|p| (p, 422)).collect();
    probes.sort_unstable();
    probes.extend([(sand, 4), (air, 0)]);
    let target = Vec3::from_array(cactus.origin.map(|v| v as f32))
        + Vec3::new(0.5, f32::from(cactus.height) * 0.5, 0.5);
    Some((target, probes, Some(cactus), None))
}

fn connected(cells: &HashSet<[i64; 3]>, origin: [i64; 3]) -> bool {
    let mut found = HashSet::from([origin]);
    let mut queue = vec![origin];
    while let Some(p) = queue.pop() {
        for d in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let q = std::array::from_fn(|i| p[i] + d[i]);
            if cells.contains(&q) && found.insert(q) {
                queue.push(q);
            }
        }
    }
    found.len() == cells.len()
}

fn cameras(
    terrain: &TerraGenerator,
    generator: &SectionGenerator,
    target: Vec3,
    cactus: Option<&CactusPlant>,
    birch: Option<&birch::Specimen>,
) -> Option<[Transform; 2]> {
    for [x, z] in [[1., 1.], [-1., 1.], [1., -1.], [-1., -1.]] {
        let offsets = if birch.is_some() {
            [
                Vec3::new(x * 24., 10., z * 28.),
                Vec3::new(x * 11., 1., z * 14.),
            ]
        } else if cactus.is_some() {
            [
                Vec3::new(x * 18., 12., z * 22.),
                Vec3::new(x * 3.1, 0.5, z * 3.8),
            ]
        } else {
            [
                Vec3::new(x * 9., 7., z * 12.),
                Vec3::new(x * 3., 2., z * 4.),
            ]
        };
        let poses = offsets.map(|offset| {
            Transform::from_translation(surface_eye(terrain, target + offset))
                .looking_at(target, Vec3::Y)
        });
        if poses.iter().all(|pose| {
            if generator
                .block_at(pose.translation.floor().as_i64vec3().to_array())
                .ok()
                != Some(0)
            {
                return false;
            }
            let steps = (pose.translation.distance(target) * 4.).ceil() as usize;
            (0..steps).all(|i| {
                let p = pose
                    .translation
                    .lerp(target, i as f32 / steps as f32)
                    .floor()
                    .as_i64vec3()
                    .to_array();
                generator
                    .block_at(p)
                    .ok()
                    .is_some_and(|m| clear_material(p, m, cactus, birch))
            })
        }) {
            return Some(poses);
        }
    }
    None
}
fn clear_material(
    p: [i64; 3],
    material: u16,
    cactus: Option<&CactusPlant>,
    birch: Option<&birch::Specimen>,
) -> bool {
    material == 0
        || matches!(material, 9 | 10)
            && birch
                .is_some_and(|b| (0..3).all(|i| (b.bounds[0][i]..=b.bounds[1][i]).contains(&p[i])))
        || MaterialId::from_raw(material).is_some_and(|m| !m.is_solid())
        || material == 422
            && cactus
                .is_some_and(|c| (0..3).all(|i| p[i] >= c.bounds[0][i] && p[i] <= c.bounds[1][i]))
}

pub(super) fn inspect(world: &mut World, stage: usize) -> Result<(), String> {
    let sample = world
        .get_resource::<Inspection>()
        .ok_or("missing reference inspection")?;
    let chunks = world.resource::<ChunkStore>();
    if sample
        .probes
        .iter()
        .any(|(p, m)| chunks.block_at(*p) != Some(*m))
    {
        return Err("generated flora differs from loaded global voxels".into());
    }
    let pose = sample.poses[stage.min(1)];
    let camera_cell = pose.translation.floor().as_i64vec3().to_array();
    if chunks.block_at(camera_cell) != Some(0) {
        return Err("reference camera is not in loaded air".into());
    }
    let steps = (pose.translation.distance(sample.target) * 4.).ceil() as usize;
    if !(0..steps).all(|i| {
        let p = pose
            .translation
            .lerp(sample.target, i as f32 / steps as f32)
            .floor()
            .as_i64vec3()
            .to_array();
        chunks
            .block_at(p)
            .is_some_and(|m| clear_material(p, m, sample.cactus.as_ref(), sample.birch.as_ref()))
    }) {
        return Err("reference plant sightline is occluded in loaded world".into());
    }
    let view = if stage == 0 { "overview" } else { "close" };
    let capture = serde_json::json!({"camera":pose.translation.to_array(),"camera_in_loaded_air":true,
        "clear_loaded_voxel_sightline":true,"verified_voxel_probes":sample.probes.len()});
    world
        .resource_mut::<Inspection>()
        .captures
        .insert(view, capture);
    let sample = world.resource::<Inspection>();
    if let Some(path) = std::env::var_os("TERRA_CAPTURE_DIR").map(PathBuf::from) {
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        let c = world.resource::<WorldTerrain>().0.config();
        let report = serde_json::json!({"status":if sample.captures.len()==2{"passed"}else{"in_progress"},
            "scene":sample.kind.name(),"seed":c.seed.0,"ecology_revision":c.ecology_revision,
            "fixture_edits":0,"tile":sample.tile,"birch":sample.birch.as_ref().map(birch::Specimen::report),"source_cells":sample.probes,"captures":sample.captures,
            "cactus":sample.cactus.as_ref().map(|p|serde_json::json!({"origin":p.origin,"height":p.height,
                "bounds":p.bounds,"voxel_count":p.voxel_count,"face_connected":true,"sand_supported":true}))});
        std::fs::write(
            path.join(format!("{}-inspection.json", sample.kind.name())),
            serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    info!(
        scene = sample.kind.name(),
        stage, "Native reference flora verified in loaded voxels"
    );
    Ok(())
}

pub(super) fn inspect_picker(world: &World, name: &str, views: &Views) -> Result<(), String> {
    let terrain = &world.resource::<WorldTerrain>().0;
    let chunks = world.resource::<ChunkStore>();
    if terrain.config().ecology_revision != 9
        || views
            .probes
            .iter()
            .any(|(p, m)| chunks.block_at(*p) != Some(*m))
    {
        return Err(
            "biome panorama was not loaded with ecology nine and matching voxel probes".into(),
        );
    }
    if let Some(directory) = std::env::var_os("TERRA_CAPTURE_DIR").map(PathBuf::from) {
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let report = serde_json::json!({"status":"passed","scene":name,"seed":terrain.config().seed.0,
            "ecology_revision":9,"fixture_edits":0,"camera":views.overview.translation.to_array(),
            "anchor":views.anchor.to_array(),"source_cells":views.probes,"verified_voxel_probes":views.probes.len(),
            "biome_weights":terrain.biome_at(f64::from(views.anchor.x),f64::from(views.anchor.z)).weights.values()});
        std::fs::write(
            directory.join(format!("{name}-reference.json")),
            serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
