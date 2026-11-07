//! Native fixtures use actual regional offshoots, never the legacy island bow.
use super::*;
use terra_world::generation::{HydrologyDistributary, HydrologyRegion};

#[derive(Clone)]
pub(super) struct View {
    region: HydrologyRegion,
    branch: usize,
    probe: usize,
}

pub(super) fn find(terrain: &terra_world::TerraGenerator) -> Result<View, String> {
    if terrain.config().river_terrain_revision < 2 || !terrain.config().rivers {
        return Err("distributaries require world.rivers=true and river_terrain_revision=2".into());
    }
    // Same bounded catchment search as the generation regression tests.
    for z in -2..=1 {
        for x in -2..=1 {
            let Some(region) = terrain.hydrology_region_at(
                f64::from(x) * 2048.0 + 1024.0,
                f64::from(z) * 2048.0 + 1024.0,
            ) else {
                continue;
            };
            for (branch, path) in region.distributaries.iter().enumerate() {
                validate(terrain, &region, path)?;
                if let Some((probe, _)) =
                    land_course(terrain, &path.course, 0, Some(&region.course)).1
                {
                    return Ok(View {
                        region,
                        branch,
                        probe,
                    });
                }
            }
        }
    }
    Err("no exposed independent distributary found in the bounded 4x4 region search".into())
}

fn validate(
    terrain: &terra_world::TerraGenerator,
    region: &HydrologyRegion,
    branch: &HydrologyDistributary,
) -> Result<(), String> {
    if region.course.get(branch.split_index) != Some(&branch.source)
        || branch.course.first() != Some(&branch.source)
        || branch.course.last() != Some(&branch.mouth)
    {
        return Err("distributary does not begin at its declared parent split".into());
    }
    if (branch.mouth[0] - region.mouth[0]).hypot(branch.mouth[2] - region.mouth[2]) < 24.0
        || branch.course.iter().flatten().any(|v| !v.is_finite())
        || branch.course.windows(2).any(|p| p[1][1] > p[0][1])
    {
        return Err(
            "distributary needs a finite downstream course and an independent mouth".into(),
        );
    }
    let (branch_land, probe) = land_course(terrain, &branch.course, 0, Some(&region.course));
    if terrain.terrain_base_height(branch.source[0], branch.source[2])
        <= terrain.config().sea_level + 2.0
        || land_course(terrain, &region.course, branch.split_index, None).0 < 128.0
        || branch_land < 128.0
        || probe.is_none()
    {
        return Err(
            "split requires two exposed land-river courses, not submerged ocean paths".into(),
        );
    }
    let mouth = branch.mouth;
    if branch.reaches_sea {
        if mouth[1] != terrain.config().sea_level {
            return Err("sea distributary mouth has an inconsistent water level".into());
        }
    } else if terrain
        .lake_at(mouth[0], mouth[2])
        .is_none_or(|lake| lake.distance >= lake.radius || lake.level != mouth[1])
    {
        return Err("inland distributary must terminate inside its real lake".into());
    }
    for p in branch.course {
        let x = p[0].floor() + 0.5;
        let z = p[2].floor() + 0.5;
        if terrain.water_column(x, z).is_none_or(|water| {
            !water.level.is_finite() || water.bed >= water.level || water.depth <= 0.0
        }) {
            return Err(format!("actual distributary course is dry at [{x}, {z}]"));
        }
    }
    let water = terrain.water_column(mouth[0], mouth[2]);
    if water.is_none_or(|water| (water.level - mouth[1]).abs() > 1e-6) {
        return Err("distributary mouth does not join the declared reservoir water".into());
    }
    Ok(())
}

fn land_course(
    terrain: &terra_world::TerraGenerator,
    course: &[[f64; 3]],
    start: usize,
    parent: Option<&[[f64; 3]]>,
) -> (f64, Option<(usize, f64)>) {
    let mut length = 0.0;
    let mut previous = course[start];
    let mut probe = None;
    for (index, p) in course.iter().enumerate().skip(start) {
        let x = p[0].floor() + 0.5;
        let z = p[2].floor() + 0.5;
        let Some(sample) = terrain.river_at(x, z) else {
            break;
        };
        let Some(water) = terrain.water_column(x, z) else {
            break;
        };
        if terrain.terrain_base_height(x, z) <= terrain.config().sea_level + 1.0
            || sample.covered
            || sample.roof_blend > 0.05
            || terrain.surface_height(x, z) >= water.level
            || terrain
                .lake_at(x, z)
                .is_some_and(|lake| lake.distance <= lake.radius + 18.0)
        {
            break;
        }
        length += (p[0] - previous[0]).hypot(p[2] - previous[2]);
        previous = *p;
        if let Some(parent) = parent {
            let separation = distance_to_course([x, z], parent);
            if index >= 4
                && index < course.len() - 4
                && separation > sample.width + 12.0
                && Vec2::from_array(sample.flow).dot(direction(course, index)) > 0.9
                && probe.is_none_or(|(_, old)| separation > old)
            {
                probe = Some((index, separation));
            }
        }
    }
    (length, probe)
}

fn direction(course: &[[f64; 3]], index: usize) -> Vec2 {
    let a = course[index.saturating_sub(1)];
    let b = course[(index + 1).min(course.len() - 1)];
    Vec2::new((b[0] - a[0]) as f32, (b[2] - a[2]) as f32).normalize_or_zero()
}

fn distance_to_course(point: [f64; 2], course: &[[f64; 3]]) -> f64 {
    course
        .windows(2)
        .map(|pair| {
            let dx = pair[1][0] - pair[0][0];
            let dz = pair[1][2] - pair[0][2];
            let t = (((point[0] - pair[0][0]) * dx + (point[1] - pair[0][2]) * dz)
                / (dx * dx + dz * dz).max(f64::EPSILON))
            .clamp(0.0, 1.0);
            (point[0] - pair[0][0] - dx * t).hypot(point[1] - pair[0][2] - dz * t)
        })
        .fold(f64::INFINITY, f64::min)
}

impl View {
    pub(super) fn split(&self, terrain: &terra_world::TerraGenerator) -> Vec3 {
        self.point(terrain, 0)
    }

    fn point(&self, terrain: &terra_world::TerraGenerator, index: usize) -> Vec3 {
        let p = self.region.distributaries[self.branch].course[index];
        let x = p[0].floor() + 0.5;
        let z = p[2].floor() + 0.5;
        let level = terrain.water_column(x, z).map_or(p[1], |water| water.level);
        Vec3::new(x as f32, level as f32, z as f32)
    }

    pub(super) fn camera(&self, terrain: &terra_world::TerraGenerator, branch: bool) -> Transform {
        let index = if branch { self.probe } else { 0 };
        let point = self.point(terrain, index);
        let flow = direction(&self.region.distributaries[self.branch].course, index);
        let offset = Vec3::new(
            -flow.x * 32.0 + flow.y * 18.0,
            0.0,
            -flow.y * 32.0 - flow.x * 18.0,
        );
        let mut eye = point + offset;
        eye.y = point
            .y
            .max(terrain.surface_height(f64::from(eye.x), f64::from(eye.z)) as f32)
            + 24.0;
        Transform::from_translation(eye).looking_at(point, Vec3::Y)
    }
}

pub(super) fn run(world: &mut World, state: &mut Fixture) {
    let view = state.distributary.as_ref().unwrap();
    let terrain = &world.resource::<WorldTerrain>().0;
    let path = &view.region.distributaries[view.branch];
    if let Err(error) = validate(terrain, &view.region, path) {
        tracing::error!(%error, "Native distributary validation failed");
        state.phase = 254;
        return;
    }
    if world.resource::<terra_render::WaterStatus>().triangles == 0 {
        return;
    }
    match state.phase {
        0 => {
            tracing::info!(region=?view.region.region, split=?path.source,
                branch_mouth=?path.mouth, parent_mouth=?view.region.mouth,
                reaches_sea=path.reaches_sea, "Native real distributary split and independent reservoir validated");
            capture(world, "river-distributary-split");
            state.phase = 20;
        }
        20 => {
            let camera = view.camera(terrain, true);
            for mut pose in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                *pose = camera;
            }
            world.resource_mut::<ChunkFocus>().0 = camera.translation.to_array().map(f64::from);
            state.phase = 21;
        }
        21 | 22 => {
            let point = view.point(terrain, view.probe);
            let river = terrain.river_at(f64::from(point.x), f64::from(point.z));
            let flow = river.map_or(Vec2::ZERO, |r| Vec2::from_array(r.flow));
            if flow.dot(direction(&path.course, view.probe)) <= 0.9
                || distance_to_course(
                    [f64::from(point.x), f64::from(point.z)],
                    &view.region.course,
                ) <= 8.0
                || terrain
                    .water_column(f64::from(point.x), f64::from(point.z))
                    .is_none()
            {
                tracing::error!(
                    ?point,
                    ?flow,
                    "Native branch must retain independent actual water and downstream flow"
                );
                state.phase = 254;
                return;
            }
            tracing::info!(
                ?point,
                ?flow,
                "Native distributary branch water and independent flow verified"
            );
            capture(
                world,
                if state.phase == 21 {
                    "river-distributary-branch"
                } else {
                    "river-distributary-motion"
                },
            );
            state.phase += 1;
        }
        _ => {
            // Drain offscreen preview readbacks as well as the river captures.
            if world
                .resource::<terra_render::RenderBlockPreviews>()
                .pending
                != 0
                || !world.resource::<terra_render::PlayerPreviewImage>().ready
            {
                return;
            }
            tracing::info!(
                "Native distributary fixture passed: real split, independent course, reservoir mouth, water columns and downstream flow"
            );
            world.write_message(AppExit::Success);
            state.phase = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_uses_real_distributaries_and_rejects_changed_split_or_mouth() {
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
            seed: terra_core::WorldSeed(42042),
            rivers: true,
            river_terrain_revision: 2,
            ecology_revision: 0,
            caves: false,
            trees: false,
            structures_revision: 0,
            ..Default::default()
        })
        .unwrap();
        let view = find(&terrain).expect("known seed has a genuine visible independent offshoot");
        let branch = &view.region.distributaries[view.branch];
        validate(&terrain, &view.region, branch).unwrap();
        let mut altered = branch.clone();
        altered.source[0] += 1.0;
        assert!(validate(&terrain, &view.region, &altered).is_err());
        altered = branch.clone();
        altered.mouth = view.region.mouth;
        assert!(validate(&terrain, &view.region, &altered).is_err());
        let old = terra_world::TerraGenerator::new(terra_core::WorldConfig {
            river_terrain_revision: 1,
            ..terrain.config().clone()
        })
        .unwrap();
        assert!(
            find(&old).is_err(),
            "an old island bow is not a regional offshoot"
        );
    }
}
