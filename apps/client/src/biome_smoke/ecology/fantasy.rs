//! Photograph an actual revisioned open arch, including loaded rock/air probes.
use super::*;

pub(super) fn views(
    world: &mut World,
    terrain: &TerraGenerator,
    generator: &SectionGenerator,
    started: Instant,
) -> Option<Views> {
    if terrain.config().ecology_revision < 9 {
        error!("Fantasy arch inspection requires ecology revision nine");
        return None;
    }
    let range = if terrain.config().ecology_revision >= 11 {
        128
    } else {
        32
    };
    for ring in 0_i64..=range {
        for z in -ring..=ring {
            for x in -ring..=ring {
                if x.abs().max(z.abs()) != ring {
                    continue;
                }
                if !terrain.fantasy_landmark_candidate([x, z]) {
                    continue;
                }
                if started.elapsed() > Duration::from_secs(35) {
                    warn!("No actual open fantasy arch found within the bounded search");
                    return None;
                }
                for formation in terrain.surface_rock_formations([x, z]) {
                    let span = std::array::from_fn::<_, 3, _>(|a| {
                        formation.bounds[1][a] - formation.bounds[0][a] + 1
                    });
                    // Old boulders and the short split horns do not meet the
                    // span of an open, two-rooted arch. Never photograph them
                    // as an arch merely because they use the same material.
                    let minimum = if terrain.config().ecology_revision >= 10 {
                        [18, 20]
                    } else {
                        [10, 13]
                    };
                    if span[1] < minimum[0] || span[0].max(span[2]) < minimum[1] {
                        continue;
                    }
                    let axis = if span[0] >= span[2] { 0 } else { 2 };
                    let base = formation.bounds[0][1];
                    let air = [
                        formation.center[0],
                        base + span[1] * 2 / 5,
                        formation.center[2],
                    ];
                    let aperture: Vec<_> = (-1..=1)
                        .flat_map(|dy| {
                            (-1..=1).map(move |du| {
                                let mut p = air;
                                p[1] += dy;
                                p[axis] += du;
                                (p, 0)
                            })
                        })
                        .collect();
                    if aperture
                        .iter()
                        .any(|(p, m)| generator.block_at(*p).ok() != Some(*m))
                    {
                        continue;
                    }
                    let mut crests = Vec::new();
                    let mut roots = [None, None];
                    for y in formation.bounds[0][1]..=formation.bounds[1][1] {
                        for z in formation.bounds[0][2]..=formation.bounds[1][2] {
                            for x in formation.bounds[0][0]..=formation.bounds[1][0] {
                                let p = [x, y, z];
                                let Some(material) = generator.block_at(p).ok() else {
                                    continue;
                                };
                                if material != formation.material
                                    && material != MaterialId::MossyStone as u16
                                {
                                    continue;
                                }
                                if y >= formation.bounds[1][1] - 1 {
                                    crests.push((p, material));
                                }
                                if y <= base + 3 {
                                    let side = usize::from(p[axis] >= air[axis]);
                                    if roots[side].is_none() {
                                        roots[side] = Some((p, material));
                                    }
                                }
                            }
                        }
                    }
                    let [Some(left), Some(right)] = roots else {
                        continue;
                    };
                    if crests.len() < 3 || (left.0[axis] - right.0[axis]).abs() < 6 {
                        continue;
                    }
                    let target = Vec3::from_array(air.map(|p| p as f32)) + Vec3::splat(0.5);
                    let Some([overview, close]) = cameras(terrain, generator, target, axis) else {
                        continue;
                    };
                    let mut probes = vec![left, right];
                    probes.extend(crests.into_iter().take(6));
                    probes.extend(aperture);
                    let cells: Vec<_> = probes.iter().map(|(p, _)| *p).collect();
                    report(
                        world,
                        Kind::Rocks,
                        formation.center,
                        &cells,
                        serde_json::json!({
                            "fantasy_arch":true,"bounds":formation.bounds,"material":formation.material,
                            "formation_voxels":formation.voxel_count,"span":span,"open_aperture":air,
                            "root_probes":[left,right],"voxel_probes":probes,
                            "clear_generator_sightlines":true,"fixture_edits":0
                        }),
                    );
                    info!(
                        ?formation,
                        ?air,
                        "Native fantasy scene selected a genuine rooted open arch"
                    );
                    return Some(Views {
                        overview,
                        close,
                        anchor: target,
                        probes,
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
    axis: usize,
) -> Option<[Transform; 2]> {
    for direction in [1., -1.] {
        let offsets = if axis == 0 {
            [
                Vec3::new(5., 9., 29. * direction),
                Vec3::new(3., 4., 18. * direction),
            ]
        } else {
            [
                Vec3::new(29. * direction, 9., 5.),
                Vec3::new(18. * direction, 4., 3.),
            ]
        };
        let scale = if terrain.config().ecology_revision >= 10 {
            1.4
        } else {
            1.0
        };
        let poses = offsets.map(|offset| {
            Transform::from_translation(surface_eye(terrain, target + offset * scale))
                .looking_at(target, Vec3::Y)
        });
        if poses.iter().all(|pose| {
            let steps = (pose.translation.distance(target) * 4.).ceil() as usize;
            (0..=steps).all(|i| {
                let p = pose
                    .translation
                    .lerp(target, i as f32 / steps as f32)
                    .floor()
                    .as_i64vec3()
                    .to_array();
                generator.block_at(p).ok() == Some(0)
            })
        }) {
            return Some(poses);
        }
    }
    None
}
