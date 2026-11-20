//! Native photographs of actual surface decoration and multi-block rocks.
//! No fixture blocks, terrain edits or alternate model-rendering path.
use super::*;
mod fantasy;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    BloodProps,
    SpiritProps,
    Rocks,
}

impl Kind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::BloodProps => "blood-props",
            Self::SpiritProps => "spirit-props",
            Self::Rocks => "rock-formations",
        }
    }

    fn accepts(self, material: u16) -> bool {
        match self {
            Self::BloodProps => matches!(material, 412..=414 | 416..=421),
            Self::SpiritProps => matches!(material, 316..=320 | 322..=323),
            Self::Rocks => false,
        }
    }
}

pub(super) fn views(world: &mut World, terrain: &TerraGenerator, kind: Kind) -> Option<Views> {
    let generator = SectionGenerator::new(terrain.clone());
    let started = Instant::now();
    if matches!(kind, Kind::Rocks) {
        return if std::env::var_os("TERRA_FANTASY_ROCKS").is_some() {
            fantasy::views(world, terrain, &generator, started)
        } else {
            rocks(world, terrain, &generator, started)
        };
    }
    let anchor = std::env::var("TERRA_ECOLOGY_ANCHOR")
        .ok()
        .and_then(|s| serde_json::from_str::<[i64; 2]>(&s).ok())
        .unwrap_or(match kind {
            Kind::BloodProps => [-664, 520],
            Kind::SpiritProps => [-2872, -4045],
            Kind::Rocks => [0, 0],
        });
    let center = anchor.map(|v| v.div_euclid(16));
    for ring in 0_i64..=6 {
        for dz in -ring..=ring {
            for dx in -ring..=ring {
                if dx.abs().max(dz.abs()) != ring {
                    continue;
                }
                if started.elapsed() > Duration::from_secs(30) {
                    return None;
                }
                let column = [center[0] + dx, center[1] + dz];
                let height = terrain
                    .surface_height((column[0] * 16 + 8) as f64, (column[1] * 16 + 8) as f64);
                let prepared = generator.prepare_column(column, false).ok()?;
                let bottom = (height.floor() as i64).div_euclid(16);
                for y in bottom - 1..=bottom + 1 {
                    let section = generator
                        .build_prepared(
                            SectionPos::new(column[0], y, column[1]),
                            0,
                            &Default::default(),
                            &prepared,
                        )
                        .ok()?;
                    for (index, &id) in section.data.blocks().iter().enumerate() {
                        if !kind.accepts(id) {
                            continue;
                        }
                        let cell = [
                            column[0] * 16 + (index % 16) as i64,
                            y * 16 + (index / 16 % 16) as i64,
                            column[1] * 16 + (index / 256) as i64,
                        ];
                        let target = Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5);
                        let Some((overview, close)) = cameras(terrain, &generator, target, false)
                        else {
                            continue;
                        };
                        report(
                            world,
                            kind,
                            cell,
                            &[cell],
                            serde_json::json!({"material":id}),
                        );
                        return Some(Views {
                            overview,
                            close,
                            anchor: target,
                            probes: vec![(cell, id)],
                        });
                    }
                }
            }
        }
    }
    None
}

fn rocks(
    world: &mut World,
    terrain: &TerraGenerator,
    generator: &SectionGenerator,
    started: Instant,
) -> Option<Views> {
    for ring in 0_i64..=32 {
        for z in -ring..=ring {
            for x in -ring..=ring {
                if x.abs().max(z.abs()) != ring {
                    continue;
                }
                if started.elapsed() > Duration::from_secs(30) {
                    return None;
                }
                for formation in terrain.surface_rock_formations([x, z]) {
                    let mut probes = Vec::new();
                    for y in (formation.bounds[0][1]..=formation.bounds[1][1]).rev() {
                        for z in formation.bounds[0][2]..=formation.bounds[1][2] {
                            for x in formation.bounds[0][0]..=formation.bounds[1][0] {
                                let cell = [x, y, z];
                                let id = generator.block_at(cell).ok()?;
                                if id == formation.material
                                    && y as f64
                                        > terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5)
                                            + 0.5
                                {
                                    probes.push((cell, id));
                                }
                            }
                        }
                        if probes.len() >= 4 {
                            break;
                        }
                    }
                    if probes.len() < 4 {
                        continue;
                    }
                    let top = probes[0].0;
                    let target = Vec3::from_array(top.map(|v| v as f32)) + Vec3::new(0.5, 1.1, 0.5);
                    let Some((overview, close)) = cameras(terrain, generator, target, true) else {
                        continue;
                    };
                    let cells: Vec<_> = probes.iter().take(4).map(|(p, _)| *p).collect();
                    report(
                        world,
                        Kind::Rocks,
                        formation.center,
                        &cells,
                        serde_json::json!({"bounds":formation.bounds,"material":formation.material,"formation_voxels":formation.voxel_count}),
                    );
                    return Some(Views {
                        overview,
                        close,
                        anchor: target,
                        probes: probes.into_iter().take(4).collect(),
                    });
                }
            }
        }
    }
    None
}

fn cameras(
    terrain: &TerraGenerator,
    generator: &SectionGenerator,
    target: Vec3,
    large: bool,
) -> Option<(Transform, Transform)> {
    for [x, z] in [[1., 1.], [-1., 1.], [1., -1.], [-1., -1.]] {
        let offsets = if large {
            [
                Vec3::new(x * 13., 10., z * 16.),
                Vec3::new(x * 8., 5., z * 10.),
            ]
        } else {
            [
                Vec3::new(x * 6., 5., z * 8.),
                Vec3::new(x * 2., 1.6, z * 3.),
            ]
        };
        let poses = offsets.map(|offset| {
            let eye = surface_eye(terrain, target + offset);
            Transform::from_translation(eye).looking_at(target, Vec3::Y)
        });
        if poses.iter().all(|pose| {
            // Inspect every 25cm to avoid photographs through an unseen canopy
            // or nearby bank. The last occupied cell contains the target prop.
            let distance = pose.translation.distance(target);
            let steps = (distance * 4.).ceil() as usize;
            (0..steps).all(|i| {
                let p = pose.translation.lerp(target, i as f32 / steps as f32);
                let cell = p.floor().as_i64vec3().to_array();
                cell == target.floor().as_i64vec3().to_array()
                    || generator.block_at(cell).ok() == Some(0)
            })
        }) {
            return Some((poses[0], poses[1]));
        }
    }
    None
}

fn report(
    world: &World,
    kind: Kind,
    center: [i64; 3],
    cells: &[[i64; 3]],
    extra: serde_json::Value,
) {
    let Some(directory) = std::env::var_os("TERRA_CAPTURE_DIR").map(PathBuf::from) else {
        return;
    };
    let config = &world.resource::<WorldTerrain>().0;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all(&directory)?;
        std::fs::write(
            directory.join(format!("{}-source.json", kind.name())),
            serde_json::to_vec_pretty(&serde_json::json!({
                "scene":kind.name(),"seed":config.config().seed.0,
                "ecology_revision":config.config().ecology_revision,
                "fixture_edits":0,"center":center,"source_cells":cells,"extra":extra
            }))?,
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        warn!(%error, "Cannot write native ecology source report");
    }
}
