//! Bounded, opt-in terrain probes; the heatmap reuses one image and one worker.
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use serde::Serialize;
use terra_core::{DebugSettings, WorldConfig};
use terra_player::{PlayerBody, TerraCamera};
use terra_world::{TerrainDebugSample, streaming::WorldTerrain};
use tilt_ui::html_shared;

const SIZE: usize = 16;
const SPACING: f64 = 64.0;
const INTERVAL: f64 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerrainDebugLayer {
    Continentalness,
    Erosion,
    Temperature,
    Humidity,
    PeaksValleys,
    Height,
    FinalDensity,
    BiomeId,
    RiverNetwork,
    WaterLevel,
}

impl TerrainDebugLayer {
    pub const ALL: [Self; 10] = [
        Self::Continentalness,
        Self::Erosion,
        Self::Temperature,
        Self::Humidity,
        Self::PeaksValleys,
        Self::Height,
        Self::FinalDensity,
        Self::BiomeId,
        Self::RiverNetwork,
        Self::WaterLevel,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Continentalness => "Continentalness",
            Self::Erosion => "Erosion",
            Self::Temperature => "Temperatur",
            Self::Humidity => "Feuchtigkeit",
            Self::PeaksValleys => "Peaks / Valleys",
            Self::Height => "Hoehe",
            Self::FinalDensity => "Final Density",
            Self::BiomeId => "Biome-ID",
            Self::RiverNetwork => "River Network",
            Self::WaterLevel => "Water Level",
        }
    }

    fn range(self) -> (f64, f64) {
        match self {
            Self::Continentalness | Self::PeaksValleys => (-1.0, 1.0),
            Self::Height | Self::WaterLevel => (-120.0, 350.0),
            Self::FinalDensity => (-16.0, 16.0),
            Self::BiomeId => (0.0, 7.0),
            _ => (0.0, 1.0),
        }
    }

    fn value(self, sample: &TerrainDebugSample) -> Option<f64> {
        Some(match self {
            Self::Continentalness => sample.fields.continentalness,
            Self::Erosion => sample.fields.erosion,
            Self::Temperature => sample.fields.temperature,
            Self::Humidity => sample.fields.humidity,
            Self::PeaksValleys => sample.fields.peaks_valleys,
            Self::Height => sample.height,
            Self::FinalDensity => sample.density,
            Self::BiomeId => sample.biome as u8 as f64,
            Self::RiverNetwork => sample.river_strength,
            Self::WaterLevel => return sample.water_level,
        })
    }

    fn text(self, sample: &TerrainDebugSample) -> String {
        match self {
            Self::BiomeId => format!("{} / {:?}", sample.biome as u8, sample.biome),
            Self::Height | Self::WaterLevel => self
                .value(sample)
                .map_or_else(|| "--".into(), |value| format!("{value:.1} m")),
            _ => format!("{:.3}", self.value(sample).unwrap_or_default()),
        }
    }

    pub fn color(self, sample: &TerrainDebugSample) -> [u8; 4] {
        let Some(value) = self.value(sample) else {
            return [24, 29, 32, 255];
        };
        if !value.is_finite() {
            return [220, 50, 160, 255];
        }
        if self == Self::BiomeId {
            let [r, g, b] = [
                [50, 115, 190],
                [144, 181, 87],
                [47, 110, 72],
                [222, 190, 109],
                [137, 147, 159],
                [179, 121, 182],
                [188, 135, 91],
                [218, 140, 171],
            ][sample.biome as usize];
            return [r, g, b, 255];
        }
        let (low, high) = self.range();
        let t = ((value - low) / (high - low)).clamp(0.0, 1.0) as f32;
        match self {
            Self::Temperature => ramp(t, &[[70, 110, 206], [239, 241, 229], [215, 83, 67]]),
            Self::Humidity => ramp(t, &[[218, 185, 111], [76, 161, 117], [50, 109, 184]]),
            // Generator density is negative in rock and positive in air.
            Self::FinalDensity => ramp(t, &[[225, 130, 68], [190, 198, 194], [70, 145, 199]]),
            Self::RiverNetwork => ramp(t, &[[30, 38, 40], [90, 196, 223]]),
            Self::WaterLevel => ramp(t, &[[35, 80, 145], [80, 196, 219]]),
            _ => ramp(
                t,
                &[
                    [28, 49, 72],
                    [30, 125, 153],
                    [82, 180, 137],
                    [239, 210, 103],
                    [244, 112, 83],
                ],
            ),
        }
    }
}

fn ramp(t: f32, colors: &[[u8; 3]]) -> [u8; 4] {
    let position = t * (colors.len() - 1) as f32;
    let first = (position as usize).min(colors.len() - 2);
    let blend = position - first as f32;
    let mut result = [255; 4];
    for (channel, value) in result[..3].iter_mut().enumerate() {
        *value = (f32::from(colors[first][channel]) * (1.0 - blend)
            + f32::from(colors[first + 1][channel]) * blend)
            .round() as u8;
    }
    result
}

#[derive(Serialize)]
pub struct TerrainDebugRow {
    id: usize,
    label: &'static str,
    value: String,
    selected: bool,
}

#[html_shared]
#[derive(Resource, Default, Serialize)]
pub struct TerrainDebugView {
    pub visible: bool,
    pub layer: String,
    pub profile: String,
    pub xyz: String,
    pub value: String,
    pub slice: String,
    pub low: String,
    pub high: String,
    pub extra: String,
    pub rows: Vec<TerrainDebugRow>,
}

#[derive(Resource, Default)]
pub(super) struct TerrainDebug {
    pub(super) layer: Option<TerrainDebugLayer>,
    config: Option<WorldConfig>,
    image: Handle<Image>,
    next: f64,
    epoch: u64,
    map_key: Option<[i64; 3]>,
    probe_key: Option<[i64; 3]>,
    map: Vec<TerrainDebugSample>,
    probe: Option<TerrainDebugSample>,
    position: [f64; 3],
    rendered_layer: Option<TerrainDebugLayer>,
    job: Option<Task<ProbeResult>>,
}

impl TerrainDebug {
    pub(super) fn cycle(&mut self) {
        self.layer = match self.layer {
            None => Some(TerrainDebugLayer::ALL[0]),
            Some(layer) => TerrainDebugLayer::ALL.get(layer as usize + 1).copied(),
        };
    }

    fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.map_key = None;
        self.probe_key = None;
        self.map.clear();
        self.probe = None;
        self.rendered_layer = None;
        // A running synchronous probe cannot be preempted by dropping its task.
        // Retain the slot until completion; its old epoch prevents publication.
    }
}

struct ProbeResult {
    epoch: u64,
    position: [f64; 3],
    probe_key: [i64; 3],
    map_key: [i64; 3],
    probe: TerrainDebugSample,
    map: Option<Vec<TerrainDebugSample>>,
}

fn parse_layer(value: &str) -> Result<Option<TerrainDebugLayer>, ()> {
    Ok(Some(match value {
        "continentalness" => TerrainDebugLayer::Continentalness,
        "erosion" => TerrainDebugLayer::Erosion,
        "temperature" => TerrainDebugLayer::Temperature,
        "humidity" => TerrainDebugLayer::Humidity,
        "peaks_valleys" => TerrainDebugLayer::PeaksValleys,
        "height" => TerrainDebugLayer::Height,
        "density" => TerrainDebugLayer::FinalDensity,
        "biome" => TerrainDebugLayer::BiomeId,
        "rivers" => TerrainDebugLayer::RiverNetwork,
        "water" => TerrainDebugLayer::WaterLevel,
        "off" => return Ok(None),
        _ => return Err(()),
    }))
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<DebugSettings>()
        .init_resource::<TerrainDebug>()
        .init_resource::<TerrainDebugView>()
        .add_systems(
            Update,
            update
                .after(super::keyboard)
                .after(terra_player::PlayerSet::Movement),
        )
        .add_systems(PostUpdate, bind.before(bevy::ui::UiSystems::Prepare));
    match std::env::var("TERRA_TERRAIN_DEBUG") {
        Ok(value) => match parse_layer(&value) {
            Ok(Some(layer)) => {
                app.world_mut().resource_mut::<TerrainDebug>().layer = Some(layer);
                app.world_mut().resource_mut::<DebugSettings>().enabled = true;
            }
            Ok(None) => {}
            Err(()) => warn!("Ignoring invalid TERRA_TERRAIN_DEBUG={value:?}"),
        },
        Err(std::env::VarError::NotPresent) => {}
        Err(error) => warn!("Ignoring invalid TERRA_TERRAIN_DEBUG: {error}"),
    }
}

#[derive(bevy::ecs::system::SystemParam)]
struct Scene<'w, 's> {
    time: Res<'w, Time>,
    debug: Res<'w, DebugSettings>,
    menu: Option<Res<'w, crate::menu::MenuState>>,
    terrain: Option<Res<'w, WorldTerrain>>,
    player: Query<'w, 's, &'static Transform, With<PlayerBody>>,
    camera: Query<'w, 's, &'static Transform, With<TerraCamera>>,
}

fn update(
    scene: Scene,
    mut state: ResMut<TerrainDebug>,
    mut view: ResMut<TerrainDebugView>,
    images: Option<ResMut<Assets<Image>>>,
) {
    let allowed = scene.debug.enabled
        && scene
            .menu
            .as_ref()
            .is_none_or(|menu| menu.world_hud_active());
    let Some(layer) = state.layer.filter(|_| allowed) else {
        if view.visible {
            view.visible = false;
        }
        state.next = 0.0;
        return;
    };
    let Some(terrain) = &scene.terrain else {
        if view.visible {
            view.visible = false;
        }
        state.invalidate();
        state.config = None;
        return;
    };
    let Some(observer) = scene
        .player
        .iter()
        .next()
        .or_else(|| scene.camera.iter().next())
    else {
        return;
    };
    let Some(mut images) = images else {
        return;
    };
    let position = observer.translation.to_array().map(f64::from);
    if position
        .iter()
        .any(|value| !value.is_finite() || value.abs() > (1_i64 << 40) as f64)
    {
        return;
    }
    let now = scene.time.elapsed_secs_f64();
    if now >= state.next
        && state
            .config
            .as_ref()
            .is_none_or(|config| !config.same_terrain(terrain.0.config()))
    {
        state.invalidate();
        state.config = Some(terrain.0.config().clone());
        view.visible = false;
    }
    let mut refreshed = false;
    if let Some(result) = state
        .job
        .as_mut()
        .and_then(|job| future::block_on(future::poll_once(job)))
    {
        state.job = None;
        if result.epoch == state.epoch {
            state.position = result.position;
            state.probe_key = Some(result.probe_key);
            state.probe = Some(result.probe);
            if let Some(map) = result.map {
                state.map_key = Some(result.map_key);
                state.map = map;
            }
            refreshed = true;
        }
    }
    if (refreshed || state.rendered_layer != Some(layer))
        && state.map.len() == SIZE * SIZE
        && state.probe.is_some()
    {
        let pixels = state
            .map
            .iter()
            .flat_map(|sample| layer.color(sample))
            .collect();
        upload(&mut state.image, &mut images, pixels);
        let probe = state.probe.as_ref().unwrap();
        let (low, high) = layer.range();
        view.layer = layer.label().into();
        view.profile = probe.profile.label().into();
        view.xyz = format!(
            "{:.1}  {:.1}  {:.1}",
            state.position[0], state.position[1], state.position[2]
        );
        view.value = layer.text(probe);
        view.slice = format!(
            "1024 x 1024 m / Y {:.1}",
            state.map_key.unwrap()[1] as f64 + 0.5
        );
        view.low = if layer == TerrainDebugLayer::FinalDensity {
            "-16 Fels".into()
        } else {
            format!("{low:.0}")
        };
        view.high = if layer == TerrainDebugLayer::FinalDensity {
            "+16 Luft".into()
        } else {
            format!("{high:.0}")
        };
        view.extra = format!(
            "Ridge {:.3} / Weirdness {:.3}",
            probe.fields.ridge_strength, probe.fields.weirdness
        );
        view.rows = TerrainDebugLayer::ALL
            .into_iter()
            .enumerate()
            .map(|(id, candidate)| TerrainDebugRow {
                id,
                label: candidate.label(),
                value: candidate.text(probe),
                selected: candidate == layer,
            })
            .collect();
        view.visible = true;
        state.rendered_layer = Some(layer);
    } else if state.probe.is_some() && state.map.len() == SIZE * SIZE && !view.visible {
        view.visible = true;
    }
    if now < state.next || state.job.is_some() {
        return;
    }
    state.next = now + INTERVAL;
    let probe_key = position.map(|value| (value * 4.0).floor() as i64);
    let map_key = map_key(position);
    if state.probe_key == Some(probe_key) && state.map_key == Some(map_key) {
        return;
    }
    let generate_map = state.map_key != Some(map_key);
    let terrain = terrain.0.clone();
    let epoch = state.epoch;
    state.job = Some(AsyncComputeTaskPool::get().spawn(async move {
        let probe = terrain.terrain_debug_sample(position);
        let map = generate_map.then(|| {
            (0..SIZE * SIZE)
                .map(|index| {
                    let x = map_key[0] as f64 + (index % SIZE) as f64 * SPACING - 7.5 * SPACING;
                    let z = map_key[2] as f64 + (index / SIZE) as f64 * SPACING - 7.5 * SPACING;
                    terrain.terrain_debug_sample([x, map_key[1] as f64 + 0.5, z])
                })
                .collect()
        });
        ProbeResult {
            epoch,
            position,
            probe_key,
            map_key,
            probe,
            map,
        }
    }));
}

fn map_key(position: [f64; 3]) -> [i64; 3] {
    let cell = position.map(|value| value.floor() as i64);
    [
        cell[0].div_euclid(32) * 32,
        cell[1],
        cell[2].div_euclid(32) * 32,
    ]
}

fn upload(handle: &mut Handle<Image>, images: &mut Assets<Image>, pixels: Vec<u8>) {
    let reusable = *handle != Handle::default()
        && images.get(&*handle).is_some_and(|image| {
            image.data.is_some()
                && image.texture_descriptor.size.width == SIZE as u32
                && image.texture_descriptor.size.height == SIZE as u32
        });
    if reusable && let Some(mut image) = images.get_mut(&*handle) {
        if image.data.as_ref() != Some(&pixels) {
            image.data = Some(pixels);
        }
    } else {
        let mut image = Image::new(
            Extent3d {
                width: SIZE as u32,
                height: SIZE as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::nearest();
        *handle = images.add(image);
    }
}

fn bind(
    state: Res<TerrainDebug>,
    view: Res<TerrainDebugView>,
    mut nodes: Query<(&tilt_ui::ElementId, &mut ImageNode)>,
) {
    if !view.visible || state.image == Handle::default() {
        return;
    }
    for (id, mut image) in &mut nodes {
        if id.0 == "terrain-debug-surface" && image.image != state.image {
            image.image = state.image.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_layer_names_cover_all_ten_layers() {
        let names = [
            "continentalness",
            "erosion",
            "temperature",
            "humidity",
            "peaks_valleys",
            "height",
            "density",
            "biome",
            "rivers",
            "water",
        ];
        for (name, layer) in names.into_iter().zip(TerrainDebugLayer::ALL) {
            assert_eq!(parse_layer(name), Ok(Some(layer)));
        }
    }

    #[test]
    fn environment_off_is_inactive_and_invalid_names_are_rejected() {
        assert_eq!(parse_layer("off"), Ok(None));
        for value in ["", "unknown", "HEIGHT", "height extra", "offf"] {
            assert_eq!(parse_layer(value), Err(()));
        }
    }

    #[test]
    fn disabled_debug_never_starts_terrain_work_or_allocates_a_heatmap() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<DebugSettings>()
            .init_resource::<TerrainDebug>()
            .init_resource::<TerrainDebugView>()
            .init_resource::<Assets<Image>>()
            .insert_resource(WorldTerrain(
                terra_world::TerraGenerator::new(WorldConfig::default()).unwrap(),
            ))
            .add_systems(Update, update);
        app.world_mut().resource_mut::<TerrainDebug>().cycle();
        app.world_mut()
            .spawn((PlayerBody, Transform::from_xyz(0.0, 80.0, 0.0)));
        app.update();
        let state = app.world().resource::<TerrainDebug>();
        assert!(state.job.is_none() && state.map.is_empty() && state.probe.is_none());
        assert_eq!(state.image, Handle::default());
        assert!(app.world().resource::<Assets<Image>>().is_empty());
        assert!(!app.world().resource::<TerrainDebugView>().visible);
    }

    #[test]
    fn layer_cycle_contains_exactly_ten_layers_then_restores_the_plain_hud() {
        let mut state = TerrainDebug::default();
        for layer in TerrainDebugLayer::ALL {
            state.cycle();
            assert_eq!(state.layer, Some(layer));
        }
        state.cycle();
        assert_eq!(state.layer, None);
    }

    #[test]
    fn heatmap_reuses_one_small_image_across_layers_and_refreshes() {
        let mut images = Assets::<Image>::default();
        let default_image = Image::default();
        let default_pixels = default_image.data.clone();
        images
            .insert(Handle::<Image>::default().id(), default_image)
            .unwrap();
        let mut handle = Handle::default();
        upload(&mut handle, &mut images, vec![32; SIZE * SIZE * 4]);
        assert_ne!(handle, Handle::default());
        let id = handle.id();
        for shade in [32, 64, 128, 255] {
            upload(&mut handle, &mut images, vec![shade; SIZE * SIZE * 4]);
            assert_eq!(handle.id(), id);
            assert_eq!(images.len(), 2);
            let image = images.get(&handle).unwrap();
            assert_eq!(image.texture_descriptor.size.width, 16);
            assert_eq!(image.texture_descriptor.size.height, 16);
            assert_eq!(image.data.as_ref().unwrap().len(), 1024);
        }
        let default_image = images.get(&Handle::default()).unwrap();
        assert_eq!(default_image.data, default_pixels);
        assert_eq!(default_image.texture_descriptor.size.width, 1);
        assert_eq!(default_image.texture_descriptor.size.height, 1);
    }

    #[test]
    fn image_binding_survives_recreated_tilt_nodes_without_changing_other_images() {
        let mut app = App::new();
        app.init_resource::<TerrainDebug>()
            .insert_resource(TerrainDebugView {
                visible: true,
                ..default()
            })
            .add_systems(Update, bind);
        let mut images = Assets::<Image>::default();
        let mut handle = Handle::default();
        upload(&mut handle, &mut images, vec![128; SIZE * SIZE * 4]);
        app.world_mut().resource_mut::<TerrainDebug>().image = handle.clone();
        let unrelated = app
            .world_mut()
            .spawn((
                tilt_ui::ElementId("other-image".into()),
                ImageNode::default(),
            ))
            .id();
        for _ in 0..3 {
            let node = app
                .world_mut()
                .spawn((
                    tilt_ui::ElementId("terrain-debug-surface".into()),
                    ImageNode::default(),
                ))
                .id();
            app.update();
            assert_eq!(app.world().get::<ImageNode>(node).unwrap().image, handle);
            assert_eq!(
                app.world().get::<ImageNode>(unrelated).unwrap().image,
                ImageNode::default().image
            );
            app.world_mut().despawn(node);
        }
        assert_eq!(images.len(), 1);
    }

    #[test]
    fn inactive_and_invalidated_debug_retains_running_job_and_discards_its_old_epoch() {
        use bevy::tasks::TaskPool;
        use std::{sync::mpsc, time::Duration};

        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<DebugSettings>()
            .init_resource::<TerrainDebug>()
            .init_resource::<TerrainDebugView>()
            .init_resource::<Assets<Image>>()
            .insert_resource(WorldTerrain(
                terra_world::TerraGenerator::new(WorldConfig::default()).unwrap(),
            ))
            .add_systems(Update, update);
        app.world_mut()
            .spawn((PlayerBody, Transform::from_xyz(0.0, 80.0, 0.0)));
        let (started, ready) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let job = AsyncComputeTaskPool::get_or_init(TaskPool::new).spawn(async move {
            started.send(()).unwrap();
            let _ = wait.recv();
            ProbeResult {
                epoch: 0,
                position: [0.0, 80.0, 0.0],
                probe_key: [0, 320, 0],
                map_key: [0, 80, 0],
                probe: TerrainDebugSample {
                    fields: default(),
                    profile: terra_core::TerrainKind::Plains,
                    height: 40.0,
                    density: 40.0,
                    biome: terra_core::BiomeKind::Plains,
                    river_strength: 0.0,
                    water_level: None,
                },
                map: None,
            }
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        app.world_mut().resource_mut::<TerrainDebug>().job = Some(job);
        app.update();
        assert!(app.world().resource::<TerrainDebug>().job.is_some());
        {
            let mut state = app.world_mut().resource_mut::<TerrainDebug>();
            state.invalidate();
            assert!(state.job.is_some());
            state.layer = Some(TerrainDebugLayer::Height);
        }
        app.world_mut().resource_mut::<DebugSettings>().enabled = true;
        app.update();
        assert!(app.world().resource::<TerrainDebug>().job.is_some());
        app.world_mut().resource_mut::<TerrainDebug>().next = f64::INFINITY;
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !app
            .world()
            .resource::<TerrainDebug>()
            .job
            .as_ref()
            .unwrap()
            .is_finished()
        {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        app.update();
        let state = app.world().resource::<TerrainDebug>();
        assert!(state.job.is_none());
        assert!(state.probe.is_none() && state.map.is_empty());
        assert!(!app.world().resource::<TerrainDebugView>().visible);
        assert!(app.world().resource::<Assets<Image>>().is_empty());
    }

    #[test]
    fn map_anchor_handles_negative_cells_and_only_moves_at_bounded_intervals() {
        assert_eq!(map_key([0.0, 80.3, 31.9]), [0, 80, 0]);
        assert_eq!(map_key([31.9, 80.9, 1.0]), [0, 80, 0]);
        assert_eq!(map_key([-0.01, -1.1, -32.1]), [-32, -2, -64]);
        assert_eq!(map_key([32.0, 81.0, 32.0]), [32, 81, 32]);
    }

    #[test]
    fn color_scales_follow_signed_fields_and_density_without_inventing_water() {
        let terrain = terra_world::TerraGenerator::new(WorldConfig::default()).unwrap();
        let mut sample = terrain.terrain_debug_sample([0.0, 80.0, 0.0]);
        sample.water_level = None;
        assert_eq!(
            TerrainDebugLayer::WaterLevel.color(&sample),
            [24, 29, 32, 255]
        );
        sample.density = -16.0;
        assert_eq!(
            TerrainDebugLayer::FinalDensity.color(&sample),
            [225, 130, 68, 255]
        );
        sample.density = 16.0;
        assert_eq!(
            TerrainDebugLayer::FinalDensity.color(&sample),
            [70, 145, 199, 255]
        );
        sample.fields.continentalness = -1.0;
        assert_eq!(
            TerrainDebugLayer::Continentalness.color(&sample),
            [28, 49, 72, 255]
        );
        sample.fields.continentalness = 1.0;
        assert_eq!(
            TerrainDebugLayer::Continentalness.color(&sample),
            [244, 112, 83, 255]
        );
    }
}
