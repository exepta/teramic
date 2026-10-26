//! One persistent GPU batch; only cached roof data is uploaded after movement/edits.
use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use std::collections::HashMap;
use terra_core::{GameConfig, WeatherKind, WeatherQuality, WorldWeather};
use terra_player::TerraCamera;
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldTerrain},
};

/// Explicit render-fixture override; gameplay never inserts this resource.
#[derive(Resource, Clone, Copy)]
pub struct WeatherPreview {
    pub kind: WeatherKind,
    pub intensity: f32,
}

#[derive(Resource, Default, Debug)]
pub struct WeatherRenderStatus {
    pub rain: f32,
    pub snow: f32,
    pub clouds: f32,
    pub quads: usize,
    pub batches: usize,
    pub cached_roofs: usize,
    pub roof_queries: usize,
    pub mesh_uploads: u64,
    pub wind_strength: f32,
    pub wet_batches: usize,
    pub cloud_faces: usize,
    pub cloud_batches: usize,
    pub cloud_uploads: u64,
    pub cloud_shaft_batches: usize,
    pub cloud_shaft_vertices: usize,
    pub snow_level: u8,
    pub snow_size: f32,
    pub lightning_flash: f32,
    pub lightning_bolts: usize,
}

#[derive(Resource, Default, Clone, Copy)]
pub(crate) struct WeatherVisual {
    pub rain: f32,
    pub snow: f32,
    pub clouds: f32,
    pub wetness: f32,
    pub wind: Vec2,
    pub snow_level: u8,
    pub snow_density: f32,
    pub snow_size: f32,
    pub flash: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
struct PrecipitationExtension {
    /// Rain, snow, daylight luminance, enabled.
    #[uniform(100)]
    conditions: Vec4,
    /// Camera world position and particle-volume radius.
    #[uniform(101)]
    center: Vec4,
    /// Horizontal wind and particle volume lower/upper height.
    #[uniform(102)]
    wind: Vec4,
    /// Cloud cover and cave suppression.
    #[uniform(103)]
    sky: Vec4,
    /// Density, size and blizzard wind influence for snowfall.
    #[uniform(104)]
    snow_profile: Vec4,
}
impl MaterialExtension for PrecipitationExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_precipitation.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_precipitation.wgsl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}
type PrecipitationMaterial = ExtendedMaterial<StandardMaterial, PrecipitationExtension>;

#[derive(Component)]
struct PrecipitationBatch;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WeatherSet {
    Local,
    Particles,
}

#[derive(Resource, Default)]
struct Batch {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<PrecipitationMaterial>>,
    center: Option<[i64; 2]>,
    height: f32,
    roofs: HashMap<[i64; 2], f32>,
    water_roofs: HashMap<[i64; 2], f32>,
    roof_signatures: HashMap<[i64; 2], u64>,
    pending_roofs: Vec<[i64; 2]>,
    roof_slice: Option<i64>,
    signature: u64,
    next_upload: f32,
    next_signature: f32,
    dirty: bool,
    quality: WeatherQuality,
    kinds: u8,
    snow_layers: usize,
}
impl Batch {
    fn invalidate_roofs(&mut self) {
        self.roofs.clear();
        self.water_roofs.clear();
        self.roof_signatures.clear();
        self.pending_roofs.clear();
        self.roof_slice = None;
        self.signature = 0;
        self.next_signature = 0.0;
        self.next_upload = 0.0;
        self.center = None;
        self.dirty = true;
    }

    fn queue_missing_roofs(&mut self, budget: Budget, center: [i64; 2], kinds: u8) {
        self.pending_roofs.clear();
        self.pending_roofs.extend(
            budget
                .roof_columns(center, kinds)
                .filter(|p| !self.roofs.contains_key(p)),
        );
        // Pop nearest first, retaining the allocation between movement/revision events.
        self.pending_roofs.sort_unstable_by_key(|p| {
            std::cmp::Reverse((p[0] - center[0]).pow(2) + (p[1] - center[1]).pow(2))
        });
    }

    fn invalidate_changed_roofs(&mut self, mut signature: impl FnMut([i64; 2]) -> u64) {
        let Self {
            roofs,
            water_roofs,
            roof_signatures,
            ..
        } = self;
        roofs.retain(|p, _| {
            let current = signature(*p);
            if roof_signatures.get(p).copied() == Some(current) {
                true
            } else {
                water_roofs.remove(p);
                roof_signatures.remove(p);
                false
            }
        });
    }
}

#[derive(Clone, Copy)]
struct Budget {
    radius: i64,
    spacing: i64,
    rain: usize,
    snow: usize,
}
impl Budget {
    fn submerged() -> Self {
        Self {
            radius: 8,
            spacing: 1,
            rain: 0,
            snow: 0,
        }
    }

    fn at(quality: WeatherQuality) -> Self {
        match quality {
            WeatherQuality::Off => Self {
                radius: 0,
                spacing: 3,
                rain: 0,
                snow: 0,
            },
            WeatherQuality::Low => Self {
                radius: 12,
                spacing: 3,
                rain: 3,
                snow: 1,
            },
            WeatherQuality::Medium => Self {
                radius: 16,
                spacing: 2,
                rain: 4,
                snow: 1,
            },
            WeatherQuality::High => Self {
                radius: 16,
                spacing: 2,
                rain: 8,
                snow: 2,
            },
            WeatherQuality::Ultra => Self {
                radius: 24,
                spacing: 2,
                rain: 12,
                snow: 3,
            },
        }
    }
    fn columns(self, center: [i64; 2]) -> impl Iterator<Item = [i64; 2]> {
        (-self.radius..=self.radius)
            .step_by(self.spacing as usize)
            .flat_map(move |x| {
                (-self.radius..=self.radius)
                    .step_by(self.spacing as usize)
                    .map(move |z| [center[0] + x, center[1] + z])
            })
    }

    fn roof_columns(self, center: [i64; 2], kinds: u8) -> impl Iterator<Item = [i64; 2]> {
        let snow = kinds & 2 != 0;
        let extra = if snow { self.spacing - 1 } else { 0 };
        let step = if snow { 1 } else { self.spacing as usize };
        (-self.radius..=self.radius + extra)
            .step_by(step)
            .flat_map(move |x| {
                (-self.radius..=self.radius + extra)
                    .step_by(step)
                    .map(move |z| [center[0] + x, center[1] + z])
            })
    }

    fn snow_layers(self, density: f32, quality: WeatherQuality) -> (usize, f32) {
        let requested = self.snow as f32 * density.max(1.0);
        let cap = match quality {
            WeatherQuality::Off => 0,
            WeatherQuality::Low => 12,
            WeatherQuality::Medium => 24,
            WeatherQuality::High => 48,
            WeatherQuality::Ultra => 64,
        };
        let layers = (requested.ceil() as usize).min(cap);
        let background = (self.snow * 4).min(layers);
        let near = layers.saturating_sub(background).max(1) as f32;
        (layers, ((requested - background as f32) / near).max(1.0))
    }
}

pub(super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<PrecipitationMaterial>::default())
        .init_resource::<Batch>()
        .init_resource::<WeatherVisual>()
        .init_resource::<WeatherRenderStatus>()
        .add_systems(
            Update,
            local_weather
                .in_set(WeatherSet::Local)
                .after(terra_player::PlayerSet::Movement)
                .before(crate::appearance::update_weather_materials)
                .before(crate::water::environment::update),
        )
        .add_systems(
            Update,
            update.after(local_weather).in_set(WeatherSet::Particles),
        );
}

fn local_weather(
    config: Res<GameConfig>,
    authority: Option<Res<WorldWeather>>,
    clock: Option<Res<terra_core::WorldClock>>,
    preview: Option<Res<WeatherPreview>>,
    world: (
        Res<WorldTerrain>,
        Res<ChunkStore>,
        Local<terra_world::weather::SurfaceWeatherProbe>,
    ),
    cameras: Query<&Transform, With<TerraCamera>>,
    mut visual: ResMut<WeatherVisual>,
) {
    let (terrain, chunks, mut weather_probe) = world;
    let Some(camera) = cameras.iter().next() else {
        return;
    };
    let profile = authority.as_ref().map(|weather| weather.snow_profile());
    let snow_level = authority.as_ref().map_or(1, |weather| weather.snow_level);
    let snow_density = profile
        .as_ref()
        .map_or(1.0, |profile| profile.flake_multiplier);
    let snow_size = profile
        .as_ref()
        .map_or(1.0, |profile| profile.size_multiplier);
    let flash = authority
        .as_ref()
        .and_then(|weather| {
            weather.latest_lightning.map(|event| {
                super::lightning::flash_strength((weather.elapsed_total - event.at) as f32)
            })
        })
        .unwrap_or(0.0);
    let value = if let Some(preview) = preview.as_deref() {
        let rain = if preview.kind == WeatherKind::Rain {
            preview.intensity
        } else {
            0.0
        };
        let snow = if preview.kind == WeatherKind::Snow {
            preview.intensity
        } else {
            0.0
        };
        WeatherVisual {
            rain,
            snow,
            clouds: 0.32 + rain.max(snow) * 0.65,
            wetness: rain,
            wind: authority.as_ref().map_or(Vec2::new(1.9, 0.8), |weather| {
                Vec2::from_array(weather.wind())
            }),
            snow_level,
            snow_density,
            snow_size,
            flash,
        }
    } else if let Some(authority) = authority {
        let local = weather_probe.sample(
            &authority,
            &terrain.0,
            &chunks,
            camera.translation.to_array().map(f64::from),
            clock.as_deref().copied().unwrap_or_default(),
        );
        WeatherVisual {
            rain: local.rain,
            snow: local.snow,
            clouds: local.clouds,
            wetness: local.wetness,
            wind: Vec2::from_array(local.wind),
            snow_level,
            snow_density,
            snow_size,
            flash,
        }
    } else {
        let intensity = config.client.weather_intensity;
        let rain = if config.client.weather == WeatherKind::Rain {
            intensity
        } else {
            0.0
        };
        let snow = if config.client.weather == WeatherKind::Snow {
            intensity
        } else {
            0.0
        };
        WeatherVisual {
            rain,
            snow,
            clouds: rain.max(snow),
            wetness: rain,
            wind: Vec2::ZERO,
            snow_level,
            snow_density,
            snow_size,
            flash: 0.0,
        }
    };
    if visual.rain != value.rain
        || visual.snow != value.snow
        || visual.clouds != value.clouds
        || visual.wetness != value.wetness
        || visual.wind != value.wind
        || visual.snow_level != value.snow_level
        || visual.snow_density != value.snow_density
        || visual.snow_size != value.snow_size
        || visual.flash != value.flash
    {
        *visual = value;
    }
}

#[derive(bevy::ecs::system::SystemParam)]
struct WeatherWorld<'w> {
    config: Res<'w, GameConfig>,
    time: Res<'w, Time>,
    visual: Res<'w, WeatherVisual>,
    chunks: Res<'w, ChunkStore>,
    terrain: Res<'w, WorldTerrain>,
    journal: Res<'w, EditJournal>,
    underwater: Res<'w, crate::UnderwaterState>,
    daylight: Res<'w, crate::scene::daylight::Daylight>,
}

fn update(
    mut commands: Commands,
    world: WeatherWorld,
    cameras: Query<&Transform, With<TerraCamera>>,
    mut batch: ResMut<Batch>,
    mut status: ResMut<WeatherRenderStatus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PrecipitationMaterial>>,
) {
    status.roof_queries = 0;
    status.rain = world.visual.rain;
    status.snow = world.visual.snow;
    status.clouds = world.visual.clouds;
    status.wind_strength = world.visual.wind.length();
    status.snow_level = world.visual.snow_level;
    status.snow_size = world.visual.snow_size;
    // Session resets may reuse coordinates and block revisions in a different world.
    // Invalidate even while clear/disabled, before the early return below.
    if world.terrain.is_changed() || world.chunks.loaded.is_empty() {
        batch.invalidate_roofs();
        status.cached_roofs = 0;
    }
    let Some(camera) = cameras.iter().next() else {
        return;
    };
    let quality = world.config.client.weather_quality;
    let submerged = world.underwater.blend >= 0.99;
    let kinds = precipitation_kinds(world.visual.rain, world.visual.snow, submerged);
    let enabled = quality != WeatherQuality::Off && !world.chunks.loaded.is_empty() && kinds != 0;
    if !enabled {
        if let Some(entity) = batch.entity.take() {
            commands.entity(entity).despawn();
        }
        if let Some(mesh) = batch.mesh.take() {
            meshes.remove(mesh.id());
        }
        batch.center = None;
        batch.pending_roofs.clear();
        status.batches = 0;
        status.quads = 0;
        return;
    }
    let mut budget = if submerged {
        Budget::submerged()
    } else {
        Budget::at(quality)
    };
    let (snow_layers, snow_concentration) = budget.snow_layers(world.visual.snow_density, quality);
    budget.snow = snow_layers;
    if batch.snow_layers != budget.snow {
        batch.snow_layers = budget.snow;
        batch.dirty = true;
    }
    let center = [
        (camera.translation.x.floor() as i64).div_euclid(4) * 4,
        (camera.translation.z.floor() as i64).div_euclid(4) * 4,
    ];
    let height = camera.translation.y.floor() as i64;
    let slice = height.div_euclid(16);
    let mut replan = false;
    if batch.roof_slice != Some(slice) {
        // Roof scans are relative to the observer's vertical section, not just X/Z.
        batch.roofs.clear();
        batch.water_roofs.clear();
        batch.roof_signatures.clear();
        batch.roof_slice = Some(slice);
        batch.dirty = true;
        replan = true;
    }
    let signature = if world.time.elapsed_secs() >= batch.next_signature {
        batch.next_signature = world.time.elapsed_secs() + 0.125;
        roof_signature(&world.chunks, &world.journal, center, height)
    } else {
        batch.signature
    };
    if kinds != batch.kinds {
        batch.kinds = kinds;
        batch.dirty = true;
        replan = true;
    }
    if batch.quality != quality {
        batch.roofs.clear();
        batch.water_roofs.clear();
        batch.roof_signatures.clear();
        batch.dirty = true;
        batch.quality = quality;
        replan = true;
    }
    let mut column_signatures = HashMap::new();
    if signature != batch.signature {
        batch.invalidate_changed_roofs(|p| {
            cached_column_signature(&mut column_signatures, p, |section| {
                column_signature(&world.chunks, &world.journal, section, slice)
            })
        });
        batch.signature = signature;
        batch.dirty = true;
        replan = true;
    }
    if batch.center != Some(center) {
        let extra = if kinds & 2 != 0 {
            budget.spacing - 1
        } else {
            0
        };
        let keep = |p: &[i64; 2]| {
            (0..2).all(|axis| {
                (center[axis] - budget.radius..=center[axis] + budget.radius + extra)
                    .contains(&p[axis])
            })
        };
        batch.roofs.retain(|p, _| keep(p));
        batch.water_roofs.retain(|p, _| keep(p));
        batch.roof_signatures.retain(|p, _| keep(p));
        batch.center = Some(center);
        batch.dirty = true;
        replan = true;
    }
    if (batch.height - camera.translation.y).abs() > 4.0 {
        batch.dirty = true;
    }
    if replan {
        batch.queue_missing_roofs(budget, center, kinds);
    }
    // Warm caches do no column traversal or sorting. Pending columns retain the
    // existing near-to-far 24-query budget, including immediately after a teleport.
    for _ in 0..24 {
        let Some(p) = batch.pending_roofs.pop() else {
            break;
        };
        let (roof, water) = roof_at(&world, p, camera.translation.y);
        batch.roofs.insert(p, roof);
        if let Some(water) = water {
            batch.water_roofs.insert(p, water);
        }
        batch.dirty = true;
        status.roof_queries += 1;
        let signature = cached_column_signature(&mut column_signatures, p, |section| {
            column_signature(&world.chunks, &world.journal, section, slice)
        });
        batch.roof_signatures.insert(p, signature);
    }
    if batch.material.is_none() {
        batch.material = Some(materials.add(ExtendedMaterial {
            base: StandardMaterial {
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                ..default()
            },
            extension: PrecipitationExtension {
                conditions: Vec4::ZERO,
                center: Vec4::ZERO,
                wind: Vec4::ZERO,
                sky: Vec4::ZERO,
                snow_profile: Vec4::ONE,
            },
        }));
    }
    if let Some(mut material) = batch.material.as_ref().and_then(|h| materials.get_mut(h)) {
        material.extension.conditions = Vec4::new(
            world.visual.rain,
            world.visual.snow,
            0.16 + 0.84 * world.daylight.daylight,
            1.0 - world.underwater.blend,
        );
        material.extension.center = camera.translation.extend(budget.radius as f32);
        material.extension.wind = Vec4::new(world.visual.wind.x, world.visual.wind.y, -14.0, 24.0);
        material.extension.sky = Vec4::new(
            world.visual.clouds,
            world.visual.rain.max(world.visual.snow),
            (Budget::at(quality).snow * 4).min(snow_layers) as f32,
            world.config.client.water_wave_strength,
        );
        material.extension.snow_profile = Vec4::new(
            (Budget::at(quality).snow as f32 * world.visual.snow_density.max(1.0))
                .min(snow_layers as f32),
            world.visual.snow_size,
            // The old blizzard's fall/drift speed is now the first level too.
            1.0,
            snow_concentration,
        );
    }
    if batch.dirty && world.time.elapsed_secs() >= batch.next_upload {
        batch.next_upload = world.time.elapsed_secs() + 0.125;
        let roofs = if submerged {
            &batch.water_roofs
        } else {
            &batch.roofs
        };
        let (mesh, quads) = mesh_for(budget, center, camera.translation.y, roofs, kinds);
        let (lower, upper) = if submerged {
            roofs.values().fold(
                (camera.translation.y - 0.5, camera.translation.y + 0.5),
                |(low, high), &roof| (low.min(roof - 0.5), high.max(roof + 0.5)),
            )
        } else {
            (camera.translation.y - 18.0, camera.translation.y + 28.0)
        };
        let bounds = Aabb::from_min_max(
            Vec3::new(
                center[0] as f32 - budget.radius as f32 - 2.0,
                lower,
                center[1] as f32 - budget.radius as f32 - 2.0,
            ),
            Vec3::new(
                center[0] as f32 + budget.radius as f32 + budget.spacing as f32,
                upper,
                center[1] as f32 + budget.radius as f32 + budget.spacing as f32,
            ),
        );
        batch.height = camera.translation.y;
        status.quads = quads;
        if quads == 0 {
            // Missing/open-water roofs are normal during underwater warmup.
            // Bevy skips allocation for empty meshes but still tries to copy them.
            if let Some(entity) = batch.entity {
                commands.entity(entity).insert(Visibility::Hidden);
            }
            batch.dirty = false;
            status.cached_roofs = batch.roofs.len();
            status.batches = 0;
            return;
        }
        if let Some(handle) = &batch.mesh {
            if let Some(mut existing) = meshes.get_mut(handle) {
                *existing = mesh;
            }
        } else {
            let mesh = meshes.add(mesh);
            if let Some(material) = batch.material.clone() {
                batch.entity = Some(
                    commands
                        .spawn((
                            Name::new("GPU precipitation"),
                            PrecipitationBatch,
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material),
                            Transform::default(),
                            // All Y movement is in the shader; the explicit bounds must include it.
                            bounds,
                            NotShadowCaster,
                            NotShadowReceiver,
                            bevy::camera::visibility::NoFrustumCulling,
                        ))
                        .id(),
                );
            }
            batch.mesh = Some(mesh);
        }
        if let Some(entity) = batch.entity {
            commands
                .entity(entity)
                .insert((bounds, Visibility::Inherited));
        }
        status.mesh_uploads += 1;
        batch.dirty = false;
    }
    status.cached_roofs = batch.roofs.len();
    status.batches = usize::from(batch.entity.is_some() && status.quads > 0);
}

fn roof_signature(chunks: &ChunkStore, journal: &EditJournal, center: [i64; 2], y: i64) -> u64 {
    let origin = [
        center[0].div_euclid(16),
        y.div_euclid(16),
        center[1].div_euclid(16),
    ];
    let mut result = 0_u64;
    for x in -2..=2 {
        for z in -2..=2 {
            for dy in -2..=3 {
                let p = terra_voxel::SectionPos::new(origin[0] + x, origin[1] + dy, origin[2] + z);
                result = result.rotate_left(7) ^ roof_revision(chunks, journal, p);
            }
        }
    }
    result
}

fn cached_column_signature(
    cache: &mut HashMap<[i64; 2], u64>,
    column: [i64; 2],
    signature: impl FnOnce([i64; 2]) -> u64,
) -> u64 {
    let section = column.map(|axis| axis.div_euclid(16));
    *cache.entry(section).or_insert_with(|| signature(section))
}

fn roof_revision(
    chunks: &ChunkStore,
    journal: &EditJournal,
    section: terra_voxel::SectionPos,
) -> u64 {
    let terrain = chunks
        .loaded
        .get(&section)
        .map_or(0, |s| s.data.revision().wrapping_add(1));
    // Liquid-only edits deliberately leave terrain revisions unchanged. Local
    // water epochs also cover replicated deltas and cells at section borders.
    terrain.wrapping_mul(0x9e3779b97f4a7c15)
        ^ journal
            .water_mesh_epoch(section)
            .wrapping_mul(0xbf58476d1ce4e5b9)
}

fn column_signature(
    chunks: &ChunkStore,
    journal: &EditJournal,
    section: [i64; 2],
    slice: i64,
) -> u64 {
    let mut value = 0_u64;
    for dy in -2..=3 {
        let p = terra_voxel::SectionPos::new(section[0], slice + dy, section[1]);
        value = value.rotate_left(7) ^ roof_revision(chunks, journal, p);
    }
    value
}

fn roof_at(world: &WeatherWorld, p: [i64; 2], y: f32) -> (f32, Option<f32>) {
    let prepared = world
        .chunks
        .prepared_column([p[0].div_euclid(16), p[1].div_euclid(16)]);
    let height = prepared
        .as_ref()
        .and_then(|column| column.surface_at(p[0], p[1]))
        .map_or_else(
            || world.terrain.0.surface_height(p[0] as f64, p[1] as f64),
            |surface| surface.height,
        );
    let natural = height.floor() as i64 + 1;
    let mut roof = roof_from_cells(natural, y, |height| {
        let cell = [p[0], height, p[1]];
        world
            .chunks
            .block_at(cell)
            .map(|id| world.journal.block(cell).unwrap_or(id))
    });
    let water = world
        .chunks
        .water_column_at(
            &world.terrain.0,
            &world.journal,
            [p[0], roof.floor() as i64, p[1]],
        )
        .and_then(|water| {
            (water.level as f32 + 0.025 >= roof).then_some(water.level as f32 + 0.025)
        });
    if let Some(water) = water {
        roof = water;
    }
    (roof, water)
}

fn roof_from_cells(natural: i64, y: f32, mut cell: impl FnMut(i64) -> Option<u16>) -> f32 {
    let lower = y.floor() as i64 - 18;
    let upper = (y.floor() as i64 + 32).max(natural.min(y.floor() as i64 + 48));
    if natural > upper {
        return natural as f32;
    }
    let mut roof = lower as f32;
    for height in (lower..=upper).rev() {
        let Some(id) = cell(height) else {
            // Unknown geometry must not permit rain through unloaded roofs.
            if height <= natural {
                return upper as f32 + 100.0;
            }
            continue;
        };
        if terra_voxel::MaterialId::from_raw(id).is_some_and(terra_voxel::MaterialId::is_solid) {
            roof = height as f32 + 1.025;
            break;
        }
    }
    roof
}

fn hash(p: [i64; 2], layer: usize) -> f32 {
    let mut value = (p[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (p[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9)
        ^ (layer as u64).wrapping_mul(0x94d049bb133111eb);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    (value >> 40) as f32 / 16_777_216.0
}

fn mesh_for(
    budget: Budget,
    center: [i64; 2],
    height: f32,
    roofs: &HashMap<[i64; 2], f32>,
    kinds: u8,
) -> (Mesh, usize) {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut metadata = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    for p in budget.columns(center) {
        for kind in 0..3 {
            if kinds & (1 << kind) == 0 {
                continue;
            }
            let layers = match kind {
                0 => budget.rain,
                1 => budget.snow,
                _ => usize::from(budget.rain == 0 || hash(p, 9) < 0.40),
            };
            for layer in 0..layers {
                let seed = hash(p, layer + kind * 13);
                let (inset, span) = if kind == 1 {
                    (0.01, budget.spacing as f32 - 0.02)
                } else {
                    (0.2, 0.6)
                };
                let x = p[0] as f32 + inset + hash(p, layer + 33) * span;
                let z = p[1] as f32 + inset + hash(p, layer + 71) * span;
                let cell = [x.floor() as i64, z.floor() as i64];
                let Some(&roof) = roofs.get(&cell) else {
                    continue;
                };
                let start = positions.len() as u32;
                for uv in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
                    // Extraction sorts transparent batches by their unanimated
                    // mesh bounds, so keep those centered on the actual weather volume.
                    positions.push([x, height, z]);
                    // The unlit shader uses this channel only for independent density thinning.
                    normals.push([hash(p, layer + kind * 67 + 239), 1.0, 0.0]);
                    uvs.push(uv);
                    metadata.push([roof, kind as f32]);
                    colors.push([seed, hash(p, layer + 6), layer as f32, hash(p, layer + 119)]);
                }
                indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
            }
        }
    }
    let quads = positions.len() / 4;
    (
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, metadata)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices)),
        quads,
    )
}

fn precipitation_kinds(rain: f32, snow: f32, submerged: bool) -> u8 {
    u8::from(rain > 0.001 && !submerged)
        | (u8::from(snow > 0.001 && !submerged) << 1)
        | (u8::from(rain > 0.001) << 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_rain_keeps_drops_in_every_height_band_across_the_full_fall_cycle() {
        use bevy::mesh::VertexAttributeValues;

        for quality in [
            WeatherQuality::Low,
            WeatherQuality::Medium,
            WeatherQuality::High,
            WeatherQuality::Ultra,
        ] {
            let budget = Budget::at(quality);
            for center in [[0, 0], [-16, 32], [4, 4]] {
                let roofs = budget.columns(center).map(|p| (p, 0.0)).collect();
                let (mesh, _) = mesh_for(budget, center, 20.0, &roofs, 1);
                let Some(VertexAttributeValues::Float32x4(colors)) =
                    mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                else {
                    panic!("rain phase seeds");
                };
                let Some(VertexAttributeValues::Float32x3(normals)) =
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                else {
                    panic!("rain density seeds");
                };
                for strength in [0.04_f32, 0.1, 0.25, 0.64] {
                    let visible: Vec<_> = colors
                        .iter()
                        .zip(normals)
                        .step_by(4)
                        .filter(|(_, normal)| normal[0] < strength.sqrt() - 0.025)
                        .map(|(color, _)| color[0])
                        .collect();
                    for frame in 0..100 {
                        let mut bands = [0; 4];
                        for seed in &visible {
                            let phase = (seed - frame as f32 * 0.01).rem_euclid(1.0);
                            bands[((phase * 4.0) as usize).min(3)] += 1;
                        }
                        assert!(
                            bands.iter().all(|&count| count > 0),
                            "{quality:?} at {center:?}, intensity {strength}, frame {frame}: {bands:?}",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn roof_warmup_is_persistent_near_to_far_and_queries_at_most_twenty_four() {
        let budget = Budget::at(WeatherQuality::Ultra);
        let center = [-4, 4];
        let mut batch = Batch::default();
        batch.roofs.insert(center, 10.0);
        batch.queue_missing_roofs(budget, center, 2);
        let expected = budget.roof_columns(center, 2).count() - 1;
        assert_eq!(batch.pending_roofs.len(), expected);
        let capacity = batch.pending_roofs.capacity();
        let mut previous = 0;
        let mut queried = std::collections::HashSet::new();
        while !batch.pending_roofs.is_empty() {
            let before = batch.pending_roofs.len();
            for _ in 0..24 {
                let Some(p) = batch.pending_roofs.pop() else {
                    break;
                };
                let distance = (p[0] - center[0]).pow(2) + (p[1] - center[1]).pow(2);
                assert!(distance >= previous);
                previous = distance;
                assert!(queried.insert(p));
                assert_ne!(p, center);
                batch.roofs.insert(p, 10.0);
            }
            assert!(before - batch.pending_roofs.len() <= 24);
        }
        assert_eq!(queried.len(), expected);
        batch.queue_missing_roofs(budget, center, 2);
        assert!(batch.pending_roofs.is_empty());
        assert_eq!(batch.pending_roofs.capacity(), capacity);
    }

    #[test]
    fn roof_revisions_are_queried_once_per_section_including_negative_coordinates() {
        let budget = Budget::at(WeatherQuality::Ultra);
        let mut signatures = HashMap::new();
        let mut queries = 0;
        let columns: Vec<_> = budget.roof_columns([-4, 4], 2).collect();
        assert_eq!(columns.len(), 2500);
        for &column in &columns {
            let signature = cached_column_signature(&mut signatures, column, |section| {
                queries += 1;
                section[0].wrapping_mul(31).wrapping_add(section[1]) as u64
            });
            assert_eq!(
                signature,
                column[0]
                    .div_euclid(16)
                    .wrapping_mul(31)
                    .wrapping_add(column[1].div_euclid(16)) as u64
            );
        }
        assert_eq!(queries, 16);
        assert_eq!(signatures.len(), 16);
    }

    #[test]
    fn roof_revision_invalidation_only_discards_changed_columns() {
        let mut batch = Batch::default();
        for (column, signature) in [([0, 0], 1), ([1, 0], 1), ([16, 0], 2)] {
            batch.roofs.insert(column, 10.0);
            batch.water_roofs.insert(column, 10.0);
            batch.roof_signatures.insert(column, signature);
        }
        batch.invalidate_changed_roofs(|column| if column[0] < 16 { 7 } else { 2 });
        assert_eq!(batch.roofs.len(), 1);
        assert_eq!(batch.water_roofs.len(), 1);
        assert_eq!(batch.roof_signatures.len(), 1);
        assert!(batch.roofs.contains_key(&[16, 0]));
        batch.queue_missing_roofs(Budget::at(WeatherQuality::Low), [0, 0], 1);
        assert!(batch.pending_roofs.contains(&[0, 0]));
    }

    #[test]
    fn fluid_only_changes_invalidate_water_roofs_even_in_negative_sections() {
        use terra_voxel::{BlockSection, global_to_section};
        use terra_world::{streaming::LoadedSection, water::FluidCell};

        for position in [[3, 5, 7], [-17, -1, -17]] {
            let (owner, _) = global_to_section(position);
            let column = [position[0], position[2]];
            let section = column.map(|axis| axis.div_euclid(16));
            let slice = position[1].div_euclid(16);
            let mut chunks = ChunkStore::default();
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let terrain_revision = chunks.loaded[&owner].data.revision();
            let mut journal = EditJournal::default();
            let mut previous_global = roof_signature(&chunks, &journal, column, position[1]);
            let mut previous_column = column_signature(&chunks, &journal, section, slice);
            let mut batch = Batch::default();
            for amount in [7, 3, 0] {
                batch.roofs.insert(column, 10.0);
                batch.water_roofs.insert(column, 10.0);
                batch.roof_signatures.insert(column, previous_column);
                journal
                    .set_fluid(
                        position,
                        FluidCell {
                            amount,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert_eq!(chunks.loaded[&owner].data.revision(), terrain_revision);
                let current_global = roof_signature(&chunks, &journal, column, position[1]);
                let current_column = column_signature(&chunks, &journal, section, slice);
                assert_ne!(current_global, previous_global);
                assert_ne!(current_column, previous_column);
                let mut signatures = HashMap::new();
                batch.invalidate_changed_roofs(|p| {
                    cached_column_signature(&mut signatures, p, |section| {
                        column_signature(&chunks, &journal, section, slice)
                    })
                });
                assert!(batch.roofs.is_empty());
                assert!(batch.water_roofs.is_empty());
                assert!(batch.roof_signatures.is_empty());
                previous_global = current_global;
                previous_column = current_column;
            }
        }
    }

    #[test]
    fn underwater_keeps_rain_impacts_but_not_falling_particles() {
        assert_eq!(precipitation_kinds(1.0, 1.0, false), 7);
        assert_eq!(precipitation_kinds(1.0, 1.0, true), 4);
        assert_eq!(precipitation_kinds(0.0, 1.0, true), 0);
        let budget = Budget::submerged();
        let roofs = budget.columns([0, 0]).map(|p| (p, 10.0)).collect();
        let (mesh, quads) = mesh_for(budget, [0, 0], 8.0, &roofs, 4);
        assert_eq!(quads, 289);
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(metadata)) =
            mesh.attribute(Mesh::ATTRIBUTE_UV_1)
        else {
            panic!("weather metadata");
        };
        assert!(metadata.iter().all(|entry| entry[1] == 2.0));
    }

    #[test]
    fn snow_has_independent_stable_size_phase_and_speed_seeds() {
        let budget = Budget::at(WeatherQuality::High);
        let roofs = budget
            .roof_columns([-16, 32], 2)
            .map(|p| (p, 10.0))
            .collect();
        let (mesh, quads) = mesh_for(budget, [-16, 32], 40.0, &roofs, 2);
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
            mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("particle random channels");
        };
        let speeds: std::collections::HashSet<_> = colors
            .chunks_exact(4)
            .map(|quad| quad[0][3].to_bits())
            .collect();
        assert!(speeds.len() > quads * 9 / 10);
        assert!(colors.iter().all(|color| (0.0..1.0).contains(&color[3])));
        assert!(colors.iter().any(|color| color[3] < 0.1));
        assert!(colors.iter().any(|color| color[3] > 0.9));
        assert!(colors.iter().any(|color| (color[0] - color[3]).abs() > 0.5));
    }

    #[test]
    fn snow_fills_the_complete_stratum_and_uses_each_actual_cell_roof() {
        let budget = Budget {
            radius: 0,
            spacing: 2,
            rain: 0,
            snow: 48,
        };
        let center = [-16, 32];
        let mut roofs: HashMap<_, _> = budget
            .roof_columns(center, 2)
            .map(|p| {
                (
                    p,
                    10.0 + (p[0] - center[0]) as f32 + (p[1] - center[1]) as f32 * 2.0,
                )
            })
            .collect();
        assert_eq!(roofs.len(), 4);
        let (mesh, count) = mesh_for(budget, center, 40.0, &roofs, 2);
        assert_eq!(count, 48);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("snow positions");
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(metadata)) =
            mesh.attribute(Mesh::ATTRIBUTE_UV_1)
        else {
            panic!("snow roofs");
        };
        let mut occupied = HashMap::<_, usize>::new();
        for index in (0..positions.len()).step_by(4) {
            let position = positions[index];
            let cell = [position[0].floor() as i64, position[2].floor() as i64];
            *occupied.entry(cell).or_default() += 1;
            assert_eq!(metadata[index][0], roofs[&cell]);
        }
        assert_eq!(occupied.len(), 4);
        assert!(occupied.values().all(|&count| count >= 4));
        let unknown = [-15, 32];
        roofs.remove(&unknown);
        let (partial, remaining) = mesh_for(budget, center, 40.0, &roofs, 2);
        assert_eq!(remaining, count - occupied[&unknown]);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            partial.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("partial snow positions");
        };
        assert!(
            positions
                .iter()
                .all(|p| [p[0].floor() as i64, p[2].floor() as i64] != unknown)
        );
        assert_eq!(budget.roof_columns(center, 1).count(), 1);
    }

    #[test]
    fn static_gpu_batches_have_valid_indices_and_bounded_quality() {
        let mut previous = 0;
        for quality in [
            WeatherQuality::Low,
            WeatherQuality::Medium,
            WeatherQuality::High,
            WeatherQuality::Ultra,
        ] {
            let budget = Budget::at(quality);
            let roofs = budget
                .roof_columns([-16, -32], 3)
                .map(|p| (p, 10.0))
                .collect();
            let (mesh, quads) = mesh_for(budget, [-16, -32], 40.0, &roofs, 3);
            assert!(quads > previous && quads < 12000);
            previous = quads;
            assert_eq!(mesh.count_vertices(), quads * 4);
            assert!(
                mesh.indices()
                    .unwrap()
                    .iter()
                    .all(|i| i < mesh.count_vertices())
            );
        }
    }

    #[test]
    fn extreme_snow_keeps_hard_geometry_budgets_and_concentrates_near_the_eye() {
        for quality in [
            WeatherQuality::Low,
            WeatherQuality::Medium,
            WeatherQuality::High,
            WeatherQuality::Ultra,
        ] {
            let mut budget = Budget::at(quality);
            let (layers, concentration) = budget.snow_layers(180.0, quality);
            assert!(concentration > 1.0);
            let background = (budget.snow * 4).min(layers);
            let local_density = (layers - background) as f32 * concentration + background as f32;
            assert!((local_density - budget.snow as f32 * 180.0).abs() < 0.01);
            budget.snow = layers;
            let roofs = budget.roof_columns([0, 0], 2).map(|p| (p, 10.0)).collect();
            let (mesh, quads) = mesh_for(budget, [0, 0], 40.0, &roofs, 2);
            assert!(quads <= 40_000, "{quality:?}: {quads}");
            assert_eq!(mesh.count_vertices(), quads * 4);
        }
    }
    #[test]
    fn unknown_roofs_have_no_precipitation_geometry() {
        let (mesh, count) = mesh_for(
            Budget::at(WeatherQuality::High),
            [0, 0],
            40.0,
            &HashMap::new(),
            3,
        );
        assert_eq!(count, 0);
        assert_eq!(mesh.count_vertices(), 0);
    }
    #[test]
    fn dry_weather_has_no_hidden_particle_vertices() {
        let budget = Budget::at(WeatherQuality::Ultra);
        let roofs = budget.columns([0, 0]).map(|p| (p, 10.0)).collect();
        let (mesh, count) = mesh_for(budget, [0, 0], 40.0, &roofs, 0);
        assert_eq!(count, 0);
        assert_eq!(mesh.count_vertices(), 0);
    }
    #[test]
    fn roofs_react_to_building_and_glass_without_torch_or_missing_chunk_leaks() {
        let floor = |y| Some(if y <= 0 { 3 } else { 0 });
        assert!((roof_from_cells(1, 5.0, floor) - 1.025).abs() < 0.001);
        for material in [3, 15] {
            let roof = roof_from_cells(1, 5.0, |y| if y == 10 { Some(material) } else { floor(y) });
            assert!((roof - 11.025).abs() < 0.001);
        }
        let torch = roof_from_cells(1, 5.0, |y| if y == 10 { Some(40) } else { floor(y) });
        assert!((torch - 1.025).abs() < 0.001);
        assert!(roof_from_cells(30, 5.0, |_| None) > 50.0);
    }
    #[test]
    fn transparent_sort_center_follows_the_volume_not_the_world_origin() {
        let budget = Budget::at(WeatherQuality::High);
        let center = [-10000, 20000];
        let roofs = budget.roof_columns(center, 3).map(|p| (p, 10.0)).collect();
        let (mesh, _) = mesh_for(budget, center, 180.0, &roofs, 3);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("Expected float positions");
        };
        for axis in 0..3 {
            let low = positions
                .iter()
                .map(|p| p[axis])
                .fold(f32::INFINITY, f32::min);
            let high = positions
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            let middle = budget.spacing as f32 * 0.5;
            let expected = [center[0] as f32 + middle, 180.0, center[1] as f32 + middle][axis];
            assert!(((low + high) * 0.5 - expected).abs() < 1.0);
        }
    }
    #[test]
    fn world_reset_invalidates_roofs_even_when_weather_is_disabled() {
        let mut batch = Batch {
            signature: 42,
            next_upload: 10.0,
            next_signature: 10.0,
            center: Some([0, 0]),
            ..default()
        };
        batch.roofs.insert([0, 0], 80.0);
        batch.water_roofs.insert([0, 0], 80.0);
        batch.roof_signatures.insert([0, 0], 1);
        batch.invalidate_roofs();
        assert!(
            batch.roofs.is_empty()
                && batch.roof_signatures.is_empty()
                && batch.water_roofs.is_empty()
        );
        assert!(batch.center.is_none() && batch.dirty);
        assert_eq!(batch.signature, 0);
        assert_eq!(batch.next_upload, 0.0);
        assert_eq!(batch.next_signature, 0.0);
    }

    #[test]
    fn empty_underwater_impacts_never_upload_or_replace_a_valid_mesh() {
        use terra_world::streaming::LoadedSection;

        let mut chunks = ChunkStore::default();
        let position = terra_voxel::SectionPos::new(0, 0, 0);
        chunks.loaded.insert(
            position,
            LoadedSection {
                data: terra_voxel::BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<GameConfig>()
            .init_resource::<EditJournal>()
            .init_resource::<Batch>()
            .init_resource::<WeatherVisual>()
            .init_resource::<WeatherRenderStatus>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<PrecipitationMaterial>>()
            .init_resource::<crate::scene::daylight::Daylight>()
            .insert_resource(crate::UnderwaterState {
                submerged: true,
                blend: 1.0,
                depth: 1.0,
            })
            .insert_resource(WorldTerrain(
                terra_world::TerraGenerator::new(Default::default()).unwrap(),
            ))
            .insert_resource(chunks)
            .add_systems(Update, update);
        app.world_mut()
            .spawn((TerraCamera, Transform::from_xyz(0.0, 5.0, 0.0)));
        app.update();
        app.world_mut().resource_mut::<WeatherVisual>().rain = 1.0;
        let quality = app.world().resource::<GameConfig>().client.weather_quality;
        let prepare = |app: &mut App, water: bool| {
            {
                let mut batch = app.world_mut().resource_mut::<Batch>();
                batch.center = Some([0, 0]);
                batch.height = 5.0;
                batch.roof_slice = Some(0);
                batch.pending_roofs.clear();
                batch.quality = quality;
                batch.kinds = 4;
                batch.snow_layers = 0;
                batch.next_signature = f32::INFINITY;
                batch.roofs = Budget::submerged()
                    .columns([0, 0])
                    .map(|p| (p, 6.0))
                    .collect();
                batch.water_roofs = if water {
                    batch.roofs.clone()
                } else {
                    HashMap::new()
                };
                batch.dirty = true;
            }
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs(1));
        };
        prepare(&mut app, false);
        app.update();
        assert!(app.world().resource::<Batch>().mesh.is_none());
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());
        assert_eq!(
            app.world().resource::<WeatherRenderStatus>().mesh_uploads,
            0
        );

        prepare(&mut app, true);
        app.update();
        let batch = app.world().resource::<Batch>();
        let entity = batch.entity.unwrap();
        let handle = batch.mesh.as_ref().unwrap().clone();
        let vertex_count = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&handle)
            .unwrap()
            .count_vertices();
        assert!(vertex_count > 0);
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Inherited)
        );

        prepare(&mut app, false);
        app.update();
        assert_eq!(app.world().resource::<Batch>().mesh.as_ref(), Some(&handle));
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().resource::<WeatherRenderStatus>().mesh_uploads,
            1
        );
        assert_eq!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&handle)
                .unwrap()
                .count_vertices(),
            vertex_count,
        );
        assert_eq!(app.world().resource::<WeatherRenderStatus>().batches, 0);

        prepare(&mut app, true);
        app.update();
        assert_eq!(app.world().resource::<Batch>().mesh.as_ref(), Some(&handle));
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Inherited)
        );
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    }
}
