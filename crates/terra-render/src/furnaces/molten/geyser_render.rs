//! One render-frame batch for geyser jets, spray and fumarole steam.
use super::{VolcanicSound, VolcanicSources};
use crate::furnaces::{Geometry, SmokeMaterial};
use bevy::prelude::*;
use terra_player::TerraCamera;
use terra_world::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, WorldTerrain},
    volcano::geyser,
};

mod boiling;

#[derive(Resource)]
pub(super) struct GeothermalBatch {
    mesh: Handle<Mesh>,
    entity: Entity,
}

#[derive(Component)]
pub(super) struct GeothermalMesh;

pub(in crate::furnaces) fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<SmokeMaterial>,
) {
    let mesh = meshes.add(crate::furnaces::mesh());
    let entity = commands
        .spawn((
            Name::new("Geothermal jets and steam"),
            GeothermalMesh,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
            bevy::light::NotShadowCaster,
            bevy::camera::visibility::NoFrustumCulling,
        ))
        .id();
    commands.insert_resource(GeothermalBatch { mesh, entity });
}

#[derive(Default)]
pub(super) struct RenderClock {
    authority: f64,
    continuity: f64,
    presented: Option<f64>,
}
impl RenderClock {
    fn frame(
        &mut self,
        authority: f64,
        continuity: f64,
        delta: f64,
        inspection: bool,
    ) -> (Option<f64>, f64) {
        let previous = self.presented.filter(|_| {
            !inspection
                && continuity == self.continuity
                && authority >= self.authority
                && authority - self.authority < 0.5
                && delta < 0.5
        });
        self.authority = authority;
        self.continuity = continuity;
        // Replication brings discrete time samples. Advance on every rendered
        // frame and correct phase gradually while the next samples arrive.
        let now = previous.map_or(authority, |previous| {
            let error = authority + 0.035 - (previous + delta);
            (previous + delta * (1.0 + error * 3.0).clamp(0.72, 1.28)).min(authority + 0.24)
        });
        self.presented = Some(now);
        (previous, now)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    sources: Res<VolcanicSources>,
    time: Res<Time>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    terrain: Option<Res<WorldTerrain>>,
    weather: Option<Res<crate::weather::WeatherVisual>>,
    cameras: Query<&Transform, With<TerraCamera>>,
    batch: Res<GeothermalBatch>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut visibility: Query<&mut Visibility, With<GeothermalMesh>>,
    mut sounds: MessageWriter<VolcanicSound>,
    mut clock: Local<RenderClock>,
) {
    let mut geometry = Geometry::default();
    if let Some(camera) = cameras.iter().next()
        && (!sources.geysers.is_empty() || !sources.fumaroles.is_empty())
    {
        let (before, now) = clock.frame(
            sources.seconds,
            sources.continuous_from,
            time.delta_secs_f64(),
            sources.inspection_seconds.is_some(),
        );
        let basis = (*camera.right(), *camera.up());
        if let Some(terrain) = terrain.as_deref() {
            let world = GeyserWorld {
                chunks: &chunks,
                journal: &journal,
                terrain: &terrain.0,
            };
            for &cell in &sources.geysers {
                let origin = append_vent(&mut geometry, &world, cell, now, basis);
                if let Some(origin) = origin
                    && let Some(before) = before
                    && geyser::surge(before, cell) < 0.20
                    && geyser::surge(now, cell) >= 0.20
                {
                    sounds.write(VolcanicSound::Steam(origin));
                }
            }
        }
        let wind = weather.as_ref().map_or(Vec2::ZERO, |weather| weather.wind);
        sources.append_fumaroles(
            &mut geometry,
            (before, now),
            wind,
            basis,
            |p| {
                chunks
                    .block_at(p)
                    .map(|raw| journal.block(p).unwrap_or(raw))
            },
            |origin| {
                sounds.write(VolcanicSound::Steam(origin));
            },
        );
    } else {
        clock.presented = None;
    }
    let visible = if geometry.upload(&mut meshes, &batch.mesh) {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if let Ok(mut current) = visibility.get_mut(batch.entity)
        && *current != visible
    {
        *current = visible;
    }
}

pub(super) struct GeyserWorld<'a> {
    pub chunks: &'a ChunkStore,
    pub journal: &'a EditJournal,
    pub terrain: &'a TerraGenerator,
}

pub(super) fn append_vent(
    vapor: &mut Geometry,
    world: &GeyserWorld,
    cell: [i64; 3],
    seconds: f64,
    (right, up): (Vec3, Vec3),
) -> Option<Vec3> {
    let t = geyser::phase(seconds, cell);
    if t >= 4.6 {
        if let Some(progress) = geyser::boil_progress(seconds, cell)
            && geyser::submerged(world.chunks, world.journal, world.terrain, cell)
        {
            boiling::append(vapor, world, cell, progress, (right, up));
        }
        return None;
    }
    // The longest raindrops fall for less than this after the jet begins.
    if !geyser::submerged(world.chunks, world.journal, world.terrain, cell) {
        return None;
    }
    let open = |p| geyser::open(world.chunks, world.journal, p);
    let clearance = (1..=30)
        .take_while(|dy| open([cell[0], cell[1] + dy, cell[2]]))
        .count() as f32;
    if clearance <= 0. {
        return None;
    }
    let origin = Vec3::from_array(cell.map(|n| n as f32)) + Vec3::new(0.5, 1.02, 0.5);
    let pressure = geyser::surge(seconds, cell);
    let height = (pressure * geyser::HEIGHT).min(clearance - 0.18);
    if height > 0. {
        jet(vapor, origin, height, clearance, pressure, t, right, up);
    }
    if clearance > geyser::HEIGHT - 1. {
        for i in 0..geyser::DROPS {
            let Some(offset) = geyser::drop_position(cell, t, i) else {
                continue;
            };
            let p = origin + Vec3::from_array(offset);
            if open(p.floor().as_i64vec3().to_array()) {
                vapor.wisp(
                    p,
                    0.075 + (i % 3) as f32 * 0.022,
                    right,
                    up * 2.5,
                    [0.49, 0.85, 0.91, 0.83],
                );
            }
        }
    }
    Some(origin)
}

#[allow(clippy::too_many_arguments)]
fn jet(
    vapor: &mut Geometry,
    origin: Vec3,
    height: f32,
    clearance: f32,
    pressure: f32,
    time: f32,
    right: Vec3,
    up: Vec3,
) {
    for strand in 0..8 {
        let angle = strand as f32 * 2.39996;
        for step in 0..12 {
            let h = (step as f32 + 0.5) / 12.;
            let lift = (h * 14. - time * 5. + angle).sin();
            let reach =
                (0.15 + 1.05 * h * h) * (0.72 + 0.4 * (strand as f32 * 1.3).sin()) * pressure;
            let bend = angle + 0.28 * h + (h * 9. + time * 2. + angle).sin() * 0.16;
            let center = origin + Vec3::new(bend.cos() * reach, h * height, bend.sin() * reach);
            let width = (0.44 + h * 0.60 + lift * 0.14) * pressure;
            let half_height = (height / 12. * 1.65)
                .min(center.y - origin.y + 0.035)
                .min(origin.y + clearance - center.y - 0.1);
            let crest = (h * h).mul_add(0.43, 0.35) + 0.08 * lift;
            vapor.wisp(
                center,
                width,
                right,
                Vec3::Y * (half_height / width),
                [
                    0.21 + crest * 0.58,
                    0.65 + crest * 0.33,
                    0.75 + crest * 0.22,
                    (0.77 - h * 0.20) * pressure,
                ],
            );
        }
    }
    for i in 0..104 {
        let rise = (i as f32 / 104. + time * 0.73).fract();
        let angle = i as f32 * 2.39996 + rise * 0.65 + (time + i as f32).sin() * 0.12;
        let envelope = geyser::BASE_RADIUS + (geyser::CROWN_RADIUS - geyser::BASE_RADIUS) * rise;
        let radius = envelope * (0.22 + 0.78 * ((i * 37 % 103) as f32 / 103.).sqrt()) * pressure;
        let center = origin + Vec3::new(angle.cos() * radius, height * rise, angle.sin() * radius);
        let width = 0.09 + pressure * (0.24 + rise * 0.41);
        let fade = (rise * 14.).min(1.) * ((1. - rise) * 7.).min(1.);
        if center.y + width < origin.y + clearance {
            vapor.wisp(
                center,
                width,
                right,
                up * (1.8 - rise * 0.65),
                [
                    0.49 + rise * 0.33,
                    0.84 + rise * 0.12,
                    0.90,
                    (0.65 - rise * 0.26) * pressure * fade,
                ],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fumarole_mesh_moves_on_consecutive_frames_with_the_same_server_sample() {
        use std::time::Duration;
        use terra_voxel::{BlockSection, MaterialId, SectionPos};
        use terra_world::streaming::LoadedSection;

        let cell = [4, 3, 4];
        let owner = SectionPos::default();
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block([4, 3, 4], MaterialId::Fumarole as u16)
            .unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let seconds = (0..200)
            .map(|i| f64::from(i) * 0.05)
            .find(|&t| terra_world::lava::plume_height(cell, t) > 3.)
            .unwrap();
        let mut app = App::new();
        app.insert_resource(chunks)
            .insert_resource(VolcanicSources {
                fumaroles: vec![cell],
                seconds,
                ..default()
            })
            .init_resource::<Assets<Mesh>>()
            .init_resource::<EditJournal>()
            .init_resource::<Time>()
            .add_message::<VolcanicSound>()
            .add_systems(PostUpdate, update);
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(crate::furnaces::mesh());
        let entity = app
            .world_mut()
            .spawn((GeothermalMesh, Visibility::Hidden))
            .id();
        app.world_mut().insert_resource(GeothermalBatch {
            mesh: mesh.clone(),
            entity,
        });
        app.world_mut()
            .spawn((TerraCamera, Transform::from_xyz(0., 6., 0.)));
        let positions = |app: &App| {
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&mesh)
                .unwrap()
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .clone()
        };
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Visible
        );
        let mut previous = positions(&app);
        for _ in 0..8 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f64(1. / 120.));
            app.update();
            let current = positions(&app);
            assert_ne!(current, previous);
            previous = current;
        }
        let mut source = app.world_mut().resource_mut::<VolcanicSources>();
        source.seconds += 3.;
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Hidden
        );
    }

    #[test]
    fn render_clock_advances_between_ten_hertz_snapshots() {
        let mut clock = RenderClock::default();
        for i in 0..360 {
            let authority = 100. + f64::from(i / 12) / 10.;
            let (previous, now) = clock.frame(authority, 100., 1. / 120., false);
            if let Some(previous) = previous {
                let frame = now - previous;
                assert!(frame > 0.005 && frame < 0.012, "{i}: {frame}");
            }
            assert!((now - authority).abs() < 0.16, "{i}: {now} {authority}");
        }
        let (previous, now) = clock.frame(22., 22., 1. / 60., false);
        assert!(previous.is_none());
        assert_eq!(now, 22.);
        assert_eq!(clock.frame(7., 22., 1. / 60., true), (None, 7.));
    }
}
