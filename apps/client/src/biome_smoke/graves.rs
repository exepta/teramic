//! Native acceptance views of real, planner-selected cemetery/tomb structures.
use super::*;
use terra_world::generation::structures::{GraveVariant, SiteSummary, StructureKind};

#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    Cemetery,
    Knight,
    Priestess,
    Scholar,
}
impl Kind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Cemetery => "cemetery",
            Self::Knight => "grave-knight",
            Self::Priestess => "grave-priestess",
            Self::Scholar => "grave-scholar",
        }
    }
    fn matches(self, site: &SiteSummary) -> bool {
        match (self, site.grave_dungeon()) {
            (Self::Cemetery, None) => site.kind == StructureKind::Cemetery,
            (Self::Knight, Some(info)) => info.variant == GraveVariant::Knight,
            (Self::Priestess, Some(info)) => info.variant == GraveVariant::Priestess,
            (Self::Scholar, Some(info)) => info.variant == GraveVariant::Scholar,
            _ => false,
        }
    }
    fn known_region(self) -> [i64; 2] {
        match self {
            Self::Cemetery => [-8, -10],
            Self::Knight => [14, 6],
            Self::Priestess => [8, 15],
            Self::Scholar => [-12, -4],
        }
    }
}

#[derive(Resource)]
struct Inspection {
    kind: Kind,
    treasures: Vec<[i64; 3]>,
    report: serde_json::Value,
}

fn parse_region(value: &str) -> Result<[i64; 2], String> {
    let region: [i64; 2] = serde_json::from_str(value).map_err(|e| e.to_string())?;
    if region.iter().any(|v| v.unsigned_abs() > 2500) {
        return Err("TERRA_GRAVE_REGION must stay inside +/-2500 regions".into());
    }
    Ok(region)
}

fn find(
    terrain: &TerraGenerator,
    kind: Kind,
    explicit: Option<[i64; 2]>,
) -> Option<([i64; 2], SiteSummary)> {
    if terrain.config().spirit_blossom_revision < 2 || !terrain.config().structures.ruins {
        return None;
    }
    let started = Instant::now();
    let candidate = |region| {
        terrain.grave_site_candidate(region)?;
        terrain
            .structure_site(region)
            .filter(|site| kind.matches(site))
            .map(|site| (region, site))
    };
    if let Some(region) = explicit {
        // An explicit fixture is a contract: never quietly substitute another site.
        return candidate(region);
    }
    if terrain.config().seed.0 == 42042
        && let Some(found) = candidate(kind.known_region())
    {
        return Some(found);
    }
    for ring in 0_i64..=96 {
        for z in -ring..=ring {
            for x in -ring..=ring {
                if started.elapsed() >= Duration::from_secs(30) {
                    warn!(
                        scene = kind.name(),
                        "Native grave search reached its 30-second bound"
                    );
                    return None;
                }
                if x.abs().max(z.abs()) == ring
                    && let Some(found) = candidate([x, z])
                {
                    return Some(found);
                }
            }
        }
    }
    None
}

pub(super) fn views(world: &mut World, terrain: &TerraGenerator, kind: Kind) -> Option<Views> {
    let explicit = match std::env::var("TERRA_GRAVE_REGION") {
        Ok(value) => match parse_region(&value) {
            Ok(region) => Some(region),
            Err(error) => {
                error!(%error, "Invalid native grave region");
                return None;
            }
        },
        Err(_) => None,
    };
    let (region, site) = find(terrain, kind, explicit)?;
    match prepare(terrain, kind, region, &site) {
        Ok((views, inspection)) => {
            info!(scene=kind.name(), ?region, center=?site.center, "Native grave selected and verified real generated voxels and treasure");
            world.insert_resource(inspection);
            Some(views)
        }
        Err(error) => {
            error!(%error, scene=kind.name(), "Invalid generated grave view");
            None
        }
    }
}

fn prepare(
    terrain: &TerraGenerator,
    kind: Kind,
    region: [i64; 2],
    site: &SiteSummary,
) -> Result<(Views, Inspection), String> {
    let generator = SectionGenerator::new(terrain.clone());
    let center = Vec3::from_array(site.center.map(|v| v as f32));
    let overview = surface_eye(terrain, center + Vec3::new(40., 35., -53.));
    let mut probes = Vec::new();
    let mut probe = |p, expected| -> Result<(), String> {
        let actual = generator.block_at(p).map_err(|e| e.to_string())?;
        if actual != expected {
            return Err(format!(
                "Generated voxel {p:?}: expected {expected}, found {actual}"
            ));
        }
        probes.push((p, actual));
        Ok(())
    };
    let info = site.grave_dungeon();
    let mut loot = Vec::new();
    let mut treasures = Vec::new();
    let (close_eye, close_target) = if let Some(info) = &info {
        for p in &info.route {
            for dy in 0..=2 {
                probe([p[0], p[1] + dy, p[2]], 0)?;
            }
            let floor = [p[0], p[1] - 1, p[2]];
            let material = generator.block_at(floor).map_err(|e| e.to_string())?;
            if material == 0 {
                return Err(format!("Unsupported generated grave route at {p:?}"));
            }
            probe(floor, material)?;
        }
        if info.treasures.is_empty() {
            return Err("Dungeon has no treasure metadata".into());
        }
        for &p in &info.treasures {
            let column = generator
                .prepare_column([p[0].div_euclid(16), p[2].div_euclid(16)], false)
                .map_err(|e| e.to_string())?;
            probe(p, 377)?;
            let object = column
                .structure_objects(terra_voxel::global_to_section(p).0)
                .into_iter()
                .find(|o| o.position == p)
                .ok_or("Missing real treasure object")?;
            let inventory = object
                .loot
                .filter(|i| i.valid() && i.occupied_slots() > 0)
                .ok_or("Missing real treasure inventory")?;
            loot.push(serde_json::json!({"position":p,"inventory":inventory}));
            treasures.push(p);
        }
        let bottom = info.levels.last().ok_or("Dungeon has no floor metadata")?[1] as f32;
        if let Some(floor) = info.floors.last() {
            (
                Vec3::from_array(floor.view_eye.map(|v| v as f32)) + Vec3::splat(0.5),
                Vec3::from_array(floor.view_target.map(|v| v as f32)) + Vec3::splat(0.5),
            )
        } else {
            (
                Vec3::new(center.x + 3.5, bottom + 2.4, center.z + 5.5),
                Vec3::new(center.x, bottom + 1.2, center.z + 13.),
            )
        }
    } else {
        let [x, y, z] = site.center;
        let headstone = if terrain.config().structures_revision >= 10 {
            site.buildings
                .iter()
                .find_map(|building| {
                    let [low, high] = building.bounds;
                    if high[0] - low[0] != 2 || high[2] - low[2] != 4 {
                        return None;
                    }
                    let p = [low[0] + 1, low[1], low[2] + 1];
                    generator
                        .block_at(p)
                        .ok()
                        .filter(|id| (362..=364).contains(id))
                        .map(|_| p)
                })
                .ok_or("Cemetery has no terrain-adapted graves")?
        } else {
            [x - 18, y, z - 17]
        };
        let id = generator.block_at(headstone).map_err(|e| e.to_string())?;
        if !(362..=364).contains(&id) {
            return Err("Cemetery has no generated headstone".into());
        }
        probe(headstone, id)?;
        probe(
            [
                x,
                y,
                z + if terrain.config().structures_revision >= 10 {
                    19
                } else {
                    20
                },
            ],
            368,
        )?;
        let grave = Vec3::from_array(headstone.map(|v| v as f32));
        (
            surface_eye(terrain, grave + Vec3::new(3., 3., -5.)),
            grave + Vec3::Y * 0.8,
        )
    };
    for camera in [overview, close_eye] {
        probe(camera.floor().as_i64vec3().to_array(), 0)?;
    }
    let overview = Transform::from_translation(overview).looking_at(center + Vec3::Y * 3., Vec3::Y);
    let close = Transform::from_translation(close_eye).looking_at(close_target, Vec3::Y);
    let report = serde_json::json!({
        "status":"generated-verified", "scenario":kind.name(), "seed":terrain.config().seed.0,
        "spirit_blossom_revision":terrain.config().spirit_blossom_revision, "structures_revision":terrain.config().structures_revision,
        "region":region, "center":site.center, "bounds":site.bounds,
        "name":info.as_ref().map(|i| &i.name), "variant":info.as_ref().map(|i| format!("{:?}",i.variant)),
        "levels":info.as_ref().map(|i| &i.levels), "entrance":info.as_ref().map(|i| i.entrance),
        "route_points":info.as_ref().map_or(0,|i| i.route.len()), "verified_voxel_probes":probes.len(),
        "loot":loot, "overview_eye":overview.translation.to_array(), "close_eye":close_eye.to_array(),
        "native_views":[], "fixture_edits":0
    });
    Ok((
        Views {
            overview,
            close,
            anchor: center,
            probes,
        },
        Inspection {
            kind,
            treasures,
            report,
        },
    ))
}

pub(super) fn inspect(world: &mut World, stage: usize) -> Result<(), String> {
    let Some(inspection) = world.get_resource::<Inspection>() else {
        return Err("Missing native grave inspection".into());
    };
    let journal = world.resource::<terra_world::edits::EditJournal>();
    for &p in &inspection.treasures {
        if journal
            .container_at(p)
            .is_none_or(|i| !i.valid() || i.occupied_slots() == 0)
        {
            return Err(format!("Streamed treasure inventory missing at {p:?}"));
        }
    }
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-biomes"));
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let lighting = lighting_snapshot(world)?;
    let view = if stage == 0 { "overview" } else { "close" };
    info!(view, %lighting, "Native grave lighting snapshot");
    let mut inspection = world.resource_mut::<Inspection>();
    inspection.report["lighting"][view] = lighting;
    inspection.report["native_views"] = serde_json::json!(if stage == 0 {
        vec!["overview"]
    } else {
        vec!["overview", "close"]
    });
    inspection.report["status"] = serde_json::json!(if stage == 0 {
        "overview-verified"
    } else {
        "passed"
    });
    let data = serde_json::to_vec_pretty(&inspection.report).map_err(|e| e.to_string())?;
    std::fs::write(
        directory.join(format!("{}-inspection.json", inspection.kind.name())),
        data,
    )
    .map_err(|e| e.to_string())
}

fn lighting_snapshot(world: &mut World) -> Result<serde_json::Value, String> {
    let (eye, ev100) = world
        .query_filtered::<(&Transform, &bevy::camera::Exposure), With<TerraCamera>>()
        .iter(world)
        .next()
        .map(|(pose, exposure)| (pose.translation, exposure.ev100))
        .ok_or("Missing native grave camera exposure")?;
    let cave_blend = world
        .get_resource::<terra_render::CaveState>()
        .ok_or("Missing native grave enclosure state")?
        .blend;
    let budget = match world
        .get_resource::<terra_core::GraphicsConfig>()
        .map_or(terra_core::CaveQuality::Medium, |g| g.cave_quality)
    {
        terra_core::CaveQuality::Low => 4,
        terra_core::CaveQuality::Medium => 8,
        terra_core::CaveQuality::High => 12,
        terra_core::CaveQuality::Ultra => 16,
    };
    let mut local_lights: Vec<_> = world
        .query::<(
            &Name,
            &Transform,
            &PointLight,
            Has<bevy::light::VolumetricLight>,
        )>()
        .iter(world)
        .filter(|(name, _, _, _)| name.as_str() == "Voxel local light")
        .map(|(_, pose, light, volumetric)| {
            serde_json::json!({
                "position": pose.translation.to_array(),
                "intensity_lumens": light.intensity,
                "range_m": light.range,
                "distance_to_camera_m": pose.translation.distance(eye),
                "shadow_maps_enabled": light.shadow_maps_enabled,
                "volumetric": volumetric,
            })
        })
        .collect();
    local_lights.sort_by(|a, b| {
        a["distance_to_camera_m"]
            .as_f64()
            .unwrap_or(f64::INFINITY)
            .total_cmp(&b["distance_to_camera_m"].as_f64().unwrap_or(f64::INFINITY))
    });
    Ok(serde_json::json!({
        "camera": eye.to_array(),
        "exposure_ev100": ev100,
        "cave_blend": cave_blend,
        "pool_budget": budget,
        "local_lights": local_lights,
        "streaming_timing": super::light_timing::snapshot(world),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_inspection_records_the_real_local_pool_and_adapted_camera() {
        let mut world = World::new();
        world.init_resource::<terra_render::CaveState>();
        world.resource_mut::<terra_render::CaveState>().blend = 1.0;
        world.spawn((
            TerraCamera,
            Transform::from_xyz(4.0, 35.0, 8.0),
            bevy::camera::Exposure { ev100: 6.45 },
        ));
        for (name, x, intensity, range) in [
            ("Voxel local light", 12.0, 16500.0, 28.0),
            ("Voxel local light", 5.0, 3400.0, 10.0),
            ("Carried torch light", 4.0, 9000.0, 28.0),
        ] {
            world.spawn((
                Name::new(name),
                Transform::from_xyz(x, 35.0, 8.0),
                PointLight {
                    intensity,
                    range,
                    shadow_maps_enabled: false,
                    ..default()
                },
            ));
        }
        let capture = lighting_snapshot(&mut world).unwrap();
        assert_eq!(capture["camera"], serde_json::json!([4.0, 35.0, 8.0]));
        assert!((capture["exposure_ev100"].as_f64().unwrap() - 6.45).abs() < 0.0001);
        assert_eq!(capture["cave_blend"], 1.0);
        assert_eq!(capture["pool_budget"], 8);
        let lights = capture["local_lights"].as_array().unwrap();
        assert_eq!(lights.len(), 2, "Exclude carried or unrelated scene lights");
        assert_eq!(lights[0]["position"], serde_json::json!([5.0, 35.0, 8.0]));
        assert_eq!(lights[0]["distance_to_camera_m"], 1.0);
        assert_eq!(lights[0]["intensity_lumens"], 3400.0);
        assert_eq!(lights[1]["intensity_lumens"], 16500.0);
        assert_eq!(lights[1]["range_m"], 28.0);
        assert!(lights.iter().all(|light| {
            light["shadow_maps_enabled"] == false && light["volumetric"] == false
        }));
    }

    #[test]
    fn explicit_region_rejects_invalid_inputs_without_substituting_a_different_site() {
        assert_eq!(parse_region("[-8,-10]").unwrap(), [-8, -10]);
        for invalid in ["1,2", "[1]", "[2501,0]", "[0,-2501]"] {
            assert!(parse_region(invalid).is_err());
        }
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            spirit_blossom_revision: 1,
            ..default()
        })
        .unwrap();
        assert!(find(&terrain, Kind::Knight, Some([14, 6])).is_none());
    }
}
