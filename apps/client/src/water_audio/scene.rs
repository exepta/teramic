use super::source::{MixControls, WaterAudioSource};
use bevy::{
    audio::AudioPlayer,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::sync::Arc;
use terra_core::AudioConfig;
use terra_physics::character::PLAYER_EYE_OFFSET;
use terra_player::{PlayerBody, TerraCamera};
use terra_ui::menu::MenuState;
use terra_world::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess, WorldTerrain},
};

#[derive(Clone, Copy)]
struct ColumnSample {
    cell: [i64; 3],
    column: Option<terra_world::water::WaterColumn>,
}

#[derive(Default)]
struct ColumnCache([Option<ColumnSample>; 4]);

impl ColumnCache {
    fn invalidate(&mut self, world_changed: bool) {
        if world_changed {
            self.0 = [None; 4];
        }
    }

    fn sample(
        &mut self,
        slot: usize,
        point: Vec3,
        mut query: impl FnMut([i64; 3]) -> Option<terra_world::water::WaterColumn>,
    ) -> Option<terra_world::water::WaterColumn> {
        let cell = point.floor().as_i64vec3().to_array();
        if let Some(sample) = self.0[slot].filter(|sample| sample.cell == cell) {
            return sample.column;
        }
        let _scope = terra_core::profiling::ScopeTimer::new("water_audio.column");
        let column = query(cell).or_else(|| query([cell[0], cell[1] - 1, cell[2]]));
        self.0[slot] = Some(ColumnSample { cell, column });
        column
    }
}

#[derive(Clone, Copy)]
struct Emitter {
    position: Vec3,
    strength: f32,
}

#[derive(Default, Resource)]
pub(super) struct WaterSoundscape {
    controls: Arc<MixControls>,
    entity: Option<Entity>,
    sources: [Option<Emitter>; 3],
    scan: Option<Task<[Option<Emitter>; 3]>>,
    next_scan: f32,
    next_occlusion: f32,
    transmission: [f32; 3],
    submerged: bool,
    columns: ColumnCache,
}

#[derive(Default, Resource, Debug)]
pub(super) struct WaterSoundStatus {
    pub listener: Option<Vec3>,
    pub gains: [[f32; 2]; 3],
    pub submerged: bool,
    pub filter_blend: f32,
    pub playback_started: bool,
    pub output_rms: f32,
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct SoundWorld<'w, 's> {
    time: Res<'w, Time>,
    terrain: Res<'w, WorldTerrain>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    access: Res<'w, WorldAccess>,
    menu: Option<Res<'w, MenuState>>,
    preferences: crate::audio_preferences::AudioPreferences<'w>,
    bodies: Query<'w, 's, &'static Transform, With<PlayerBody>>,
    cameras: Query<'w, 's, &'static Transform, With<TerraCamera>>,
    sinks: Query<'w, 's, (), With<bevy::audio::AudioSink>>,
}

pub(super) fn update(
    world: SoundWorld,
    mut sounds: ResMut<WaterSoundscape>,
    mut assets: ResMut<Assets<WaterAudioSource>>,
    mut status: ResMut<WaterSoundStatus>,
    mut commands: Commands,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("water_audio.update");
    let active = world.access.ready()
        && world.terrain.0.config().water
        && world
            .menu
            .as_ref()
            .is_none_or(|menu| menu.world_audio_active());
    let body = world.bodies.iter().next();
    let now = world.time.elapsed_secs();
    if !active || body.is_none() {
        sounds.controls.update([[0.0; 2]; 3], false, now);
        if let Some(entity) = sounds.entity.take() {
            commands.entity(entity).despawn();
        }
        sounds.scan = None;
        sounds.sources = [None; 3];
        sounds.next_scan = 0.0;
        sounds.next_occlusion = 0.0;
        sounds.transmission = [0.0; 3];
        sounds.submerged = false;
        sounds.columns.invalidate(true);
        *status = WaterSoundStatus::default();
        return;
    }
    let Some(body) = body else { return };
    let listener = listener_position(body);
    // Connected liquid spans are unchanged while remaining in one voxel. Keep
    // per-frame head depth and stereo panning, but do not rescan the same lake.
    sounds.columns.invalidate(
        world.chunks.is_changed() || world.journal.is_changed() || world.terrain.is_changed(),
    );
    // Looking changes panning only. A third-person boom cannot move the hearing point.
    let right = world.cameras.iter().next().map_or(Vec3::X, |c| *c.right());
    if world.terrain.is_changed() {
        sounds.scan = None;
        sounds.sources = [None; 3];
        sounds.next_scan = 0.0;
        sounds.next_occlusion = 0.0;
    }
    if sounds.entity.is_none() {
        let handle = assets.add(WaterAudioSource(sounds.controls.clone()));
        sounds.entity = Some(
            commands
                .spawn((Name::new("Water soundscape"), AudioPlayer(handle)))
                .id(),
        );
    }
    if let Some(mut sources) = sounds
        .scan
        .as_mut()
        .and_then(|task| future::block_on(future::poll_once(task)))
    {
        include_fluid_sources(
            &mut sources,
            listener,
            &world.terrain.0,
            &world.chunks,
            &world.journal,
        );
        sounds.sources = sources;
        sounds.scan = None;
        sounds.next_occlusion = 0.0;
    }
    if sounds.scan.is_none() && now >= sounds.next_scan {
        let terrain = world.terrain.0.clone();
        sounds.scan = Some(
            AsyncComputeTaskPool::get().spawn(async move { scan_sources(&terrain, listener) }),
        );
        sounds.next_scan = now + 0.5;
    }
    let column = sounds.columns.sample(0, listener, |cell| {
        world
            .chunks
            .water_column_at(&world.terrain.0, &world.journal, cell)
    });
    let immersed_depth = column
        .filter(|column| {
            listener.y > column.bed as f32
                && world
                    .chunks
                    .block_at(listener.floor().as_i64vec3().to_array())
                    .is_some_and(terra_world::water::can_hold_fluid)
        })
        .map_or(-1.0, |column| column.level as f32 - listener.y);
    sounds.submerged = immersed_depth > if sounds.submerged { -0.03 } else { 0.06 };
    if now >= sounds.next_occlusion {
        sounds.transmission = std::array::from_fn(|kind| {
            sounds.sources[kind].map_or(0.0, |source| {
                acoustic_transmission(listener, source.position, |p| world.chunks.block_at(p))
            })
        });
        sounds.next_occlusion = now + 0.125;
    }
    let gains = std::array::from_fn(|kind| {
        sounds.sources[kind].map_or([0.0; 2], |source| {
            if kind == 2 {
                return if terra_world::water::falling_fluid_at(
                    &world.terrain.0,
                    &world.chunks,
                    &world.journal,
                    source.position.floor().as_i64vec3().to_array(),
                ) {
                    spatial_gains(listener, right, source, kind)
                        .map(|gain| gain * sounds.transmission[kind])
                } else {
                    [0.0; 2]
                };
            }
            // A placed solid block or an unloaded region must not emit nearby water sound.
            if !world
                .chunks
                .block_at(source.position.floor().as_i64vec3().to_array())
                .is_some_and(terra_world::water::can_hold_fluid)
                || sounds
                    .columns
                    .sample(kind + 1, source.position, |cell| {
                        world
                            .chunks
                            .water_column_at(&world.terrain.0, &world.journal, cell)
                    })
                    .is_none_or(|water| (water.level as f32 - source.position.y).abs() > 1.0)
            {
                return [0.0; 2];
            }
            spatial_gains(listener, right, source, kind)
                .map(|gain| gain * sounds.transmission[kind])
        })
    });
    let gains = apply_audio_settings(gains, world.preferences.current());
    sounds.controls.update(gains, sounds.submerged, now);
    *status = WaterSoundStatus {
        listener: Some(listener),
        gains,
        submerged: sounds.submerged,
        filter_blend: sounds.controls.filter_blend(),
        output_rms: sounds.controls.output_rms(),
        playback_started: sounds
            .entity
            .is_some_and(|entity| world.sinks.contains(entity)),
    };
}

fn include_fluid_sources(
    sources: &mut [Option<Emitter>; 3],
    listener: Vec3,
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
) {
    let center = terra_voxel::global_to_section(listener.floor().as_i64vec3().to_array()).0;
    // Query only nearby sparse overrides once per acoustic scan, not the entire save.
    for dz in -2..=2 {
        for dy in -2..=2 {
            for dx in -2..=2 {
                let Ok(section) = center.offset([dx, dy, dz]) else {
                    continue;
                };
                if !chunks.loaded.contains_key(&section) {
                    continue;
                }
                let fall = chunks
                    .prepared_column([section.x, section.z])
                    .and_then(|c| c.nearest_waterfall(section, listener.to_array().map(f64::from)))
                    .and_then(|p| {
                        chunks
                            .block_at(p)
                            .map(|m| (p, chunks.fluid_cell_at(terrain, journal, p, m)))
                    });
                for (cell, fluid) in journal.fluid_cells_in(section).chain(fall) {
                    if fluid.amount == 0
                        || !chunks
                            .block_at(cell)
                            .is_some_and(terra_world::water::can_hold_fluid)
                    {
                        continue;
                    }
                    let falling =
                        terra_world::water::falling_fluid_at(terrain, chunks, journal, cell);
                    if !falling
                        && journal
                            .fluid([cell[0], cell[1] + 1, cell[2]])
                            .is_some_and(|above| above.amount > 0)
                    {
                        continue;
                    }
                    let position = Vec3::new(
                        cell[0] as f32 + 0.5,
                        cell[1] as f32
                            + if falling {
                                fluid.height() as f32 * 0.5
                            } else {
                                fluid.height() as f32 + 0.05
                            },
                        cell[2] as f32 + 0.5,
                    );
                    let radius = if falling { 44.0 } else { 30.0 };
                    if position.distance_squared(listener) > radius * radius {
                        continue;
                    }
                    let flowing = fluid.flow != [0, 0];
                    let kind = if falling { 2 } else { usize::from(flowing) };
                    choose_source(
                        &mut sources[kind],
                        Emitter {
                            position,
                            strength: if falling {
                                0.12
                            } else if flowing {
                                0.075
                            } else {
                                0.045
                            },
                        },
                        listener,
                        kind,
                    );
                }
            }
        }
    }
}

pub(super) fn apply_audio_settings(
    mut gains: [[f32; 2]; 3],
    config: &AudioConfig,
) -> [[f32; 2]; 3] {
    let master = if config.muted {
        0.0
    } else {
        config.master_volume * config.ambience_volume
    };
    for (voice, category) in gains.iter_mut().zip([
        config.water_volume,
        config.river_volume,
        config.waterfall_volume,
    ]) {
        for gain in voice {
            *gain *= master * category;
        }
    }
    gains
}

fn listener_position(body: &Transform) -> Vec3 {
    body.translation + Vec3::Y * PLAYER_EYE_OFFSET
}

pub(crate) fn acoustic_transmission(
    listener: Vec3,
    source: Vec3,
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
) -> f32 {
    let offset = source - listener;
    let distance = offset.length();
    if distance > 44.0 || !distance.is_finite() {
        return 0.0;
    }
    if !block_at(source.floor().as_i64vec3().to_array())
        .is_some_and(terra_world::water::can_hold_fluid)
        || !block_at(listener.floor().as_i64vec3().to_array())
            .is_some_and(terra_world::water::can_hold_fluid)
    {
        return 0.0;
    }
    if distance <= f32::EPSILON {
        return 1.0;
    }
    // At most 78 visited cells per ray, three rays at 8 Hz. Reuse terrain picking's
    // exact traversal so even a one-block roof cannot leak through sparse samples.
    let obstruction = terra_voxel::blocks::raycast_blocks(
        listener.to_array().map(f64::from),
        offset.to_array().map(f64::from),
        f64::from(distance),
        |p| {
            Some(u16::from(
                !block_at(p).is_some_and(terra_world::water::can_hold_fluid),
            ))
        },
    );
    if obstruction.is_some() { 0.05 } else { 1.0 }
}

fn spatial_gains(listener: Vec3, right: Vec3, source: Emitter, kind: usize) -> [f32; 2] {
    let offset = source.position - listener;
    let distance = offset.length();
    let radius = if kind == 2 { 44.0 } else { 30.0 };
    let fade = (1.0 - distance / radius).clamp(0.0, 1.0);
    let gain = source.strength * fade * fade * (3.0 - 2.0 * fade);
    let pan = if distance > 0.25 {
        (offset / distance).dot(right).clamp(-0.85, 0.85)
    } else {
        0.0
    };
    [
        gain * ((1.0 - pan) * 0.5).sqrt(),
        gain * ((1.0 + pan) * 0.5).sqrt(),
    ]
}

fn scan_sources(terrain: &TerraGenerator, listener: Vec3) -> [Option<Emitter>; 3] {
    let _scope = terra_core::profiling::ScopeTimer::new("water_audio.scan");
    let mut sources = [None; 3];
    let center_x = (listener.x / 4.0).floor() as i64 * 4;
    let center_z = (listener.z / 4.0).floor() as i64 * 4;
    for dz in -8..=8 {
        for dx in -8..=8 {
            let x = (center_x + dx * 4) as f64 + 0.5;
            let z = (center_z + dz * 4) as f64 + 0.5;
            let columns = terrain.water_columns(x, z);
            if columns.intervals.iter().all(Option::is_none) {
                continue;
            }
            let river = terrain.river_at(x, z).filter(|r| {
                r.distance <= r.width + 2.0 && r.level > terrain.config().sea_level + 0.05
            });
            let next = river.and_then(|river| {
                terrain.water_column(
                    x + f64::from(river.flow[0]) * 4.0,
                    z + f64::from(river.flow[1]) * 4.0,
                )
            });
            let Some(water) = columns.below(f64::from(listener.y) + 4.0) else {
                continue;
            };
            let position = Vec3::new(x as f32, water.level as f32 + 0.05, z as f32);
            let river = river.filter(|river| (river.level - water.level).abs() < 1.0);
            if river.is_some() {
                let slope = next.map_or(0.0, |next| ((water.level - next.level) / 4.0).max(0.0));
                let strength = 0.075 + (slope as f32).min(0.8) * 0.025;
                choose_source(&mut sources[1], Emitter { position, strength }, listener, 1);
            }
            if river.is_none()
                && terrain.cave_structure_at(x, z).kind
                    == terra_world::generation::CaveEcology::Waterfall
            {
                choose_source(
                    &mut sources[1],
                    Emitter {
                        position,
                        strength: 0.06,
                    },
                    listener,
                    1,
                );
            } else if river.is_none() {
                let strength = if water.depth < 3.5 { 0.055 } else { 0.04 };
                choose_source(&mut sources[0], Emitter { position, strength }, listener, 0);
            }
        }
    }
    sources
}

fn choose_source(slot: &mut Option<Emitter>, source: Emitter, listener: Vec3, kind: usize) {
    let loudness = |s| {
        spatial_gains(listener, Vec3::X, s, kind)
            .into_iter()
            .sum::<f32>()
    };
    if slot.is_none_or(|previous| loudness(source) > loudness(previous)) {
        *slot = Some(source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liquid_spans_are_cached_per_listener_emitter_cell_not_per_frame() {
        let mut cache = ColumnCache::default();
        let water = terra_world::water::WaterColumn {
            level: 8.0,
            bed: -64.0,
            depth: 72.0,
        };
        let mut calls = 0;
        for point in [Vec3::new(-0.9, 7.1, -16.9), Vec3::new(-0.1, 7.9, -16.1)] {
            assert_eq!(
                cache.sample(0, point, |cell| {
                    calls += 1;
                    assert_eq!(cell, [-1, 7, -17]);
                    Some(water)
                }),
                Some(water)
            );
        }
        assert_eq!(calls, 1);
        cache.invalidate(false);
        assert_eq!(
            cache.sample(0, Vec3::new(-0.5, 7.5, -16.5), |_| panic!("cached")),
            Some(water)
        );
        cache.sample(0, Vec3::new(0.1, 7.5, -16.5), |_| {
            calls += 1;
            Some(water)
        });
        cache.sample(1, Vec3::new(0.1, 7.5, -16.5), |_| {
            calls += 1;
            Some(water)
        });
        assert_eq!(calls, 3, "cell movement and independent emitters refresh");
    }

    #[test]
    fn water_cache_preserves_lower_cell_fallback_and_invalidates_dry_results() {
        let mut cache = ColumnCache::default();
        let point = Vec3::new(-2.5, 8.1, -3.5);
        let mut calls = Vec::new();
        assert_eq!(
            cache.sample(2, point, |cell| {
                calls.push(cell);
                None
            }),
            None
        );
        assert_eq!(calls, [[-3, 8, -4], [-3, 7, -4]]);
        assert_eq!(cache.sample(2, point, |_| panic!("cached dry span")), None);
        cache.invalidate(true);
        let water = terra_world::water::WaterColumn {
            level: 8.0,
            bed: 5.0,
            depth: 3.0,
        };
        assert_eq!(
            cache.sample(2, point, |cell| (cell[1] == 7).then_some(water)),
            Some(water),
            "newly placed water must replace a cached dry column immediately"
        );
        cache.invalidate(true);
        assert_eq!(
            cache.sample(2, point, |_| None),
            None,
            "unloaded or blocked spans must not retain water"
        );
    }

    #[test]
    fn hearing_origin_is_the_player_head_and_attenuation_is_spatial() {
        let body = Transform::from_xyz(-10.0, 3.0, 20.0);
        let listener = listener_position(&body);
        assert_eq!(listener, Vec3::new(-10.0, 3.72, 20.0));
        let source = Emitter {
            position: listener + Vec3::X * 4.0,
            strength: 0.3,
        };
        let near = spatial_gains(listener, Vec3::X, source, 1);
        let far = spatial_gains(listener - Vec3::X * 20.0, Vec3::X, source, 1);
        assert!(near[1] > near[0] && far[1] < near[1]);
        assert_eq!(
            spatial_gains(listener - Vec3::X * 40.0, Vec3::X, source, 1),
            [0.0; 2]
        );
    }

    #[test]
    fn dry_world_does_not_create_emitters() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            water: false,
            ..default()
        })
        .unwrap();
        assert!(
            scan_sources(&terrain, Vec3::ZERO)
                .iter()
                .all(Option::is_none)
        );
    }

    #[test]
    fn waterfall_audio_requires_real_loaded_falling_cells_and_is_quieter_than_legacy_roar() {
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::{streaming::LoadedSection, water::FluidCell};
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            base_height: -20.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            biomes: false,
            sea_level: -100.0,
            caves: false,
            rivers: false,
            ..default()
        })
        .unwrap();
        let owner = SectionPos::default();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: BlockSection::empty(owner).unwrap(),
                mesh: None,
                water: None,
                mesh_epoch: 0,
                water_epoch: 0,
                props: Vec::new(),
            },
        );
        let mut journal = EditJournal::default();
        let listener = Vec3::new(8.5, 5.5, 9.5);
        let mut sources = [None; 3];
        include_fluid_sources(&mut sources, listener, &terrain, &chunks, &journal);
        assert!(
            sources[2].is_none(),
            "a terrain waterfall shape without liquid must be silent"
        );
        for y in 4..=6 {
            journal
                .set_fluid(
                    [8, y, 8],
                    FluidCell {
                        amount: 8,
                        ..default()
                    },
                )
                .unwrap();
        }
        include_fluid_sources(&mut sources, listener, &terrain, &chunks, &journal);
        let fall = sources[2]
            .expect("a full vertical flow stack must emit even though the cell below is full");
        assert!(fall.strength <= 0.12 && fall.strength > 0.0);
        assert!(
            spatial_gains(listener, Vec3::X, fall, 2)
                .iter()
                .sum::<f32>()
                > 0.02
        );
        for y in 4..=6 {
            journal.set_fluid([8, y, 8], FluidCell::default()).unwrap();
        }
        let mut drained = [None; 3];
        include_fluid_sources(&mut drained, listener, &terrain, &chunks, &journal);
        assert!(
            drained[2].is_none(),
            "closing and draining a real fall removes its emitter"
        );
        chunks.loaded.clear();
        let mut unloaded = [None; 3];
        include_fluid_sources(&mut unloaded, listener, &terrain, &chunks, &journal);
        assert!(unloaded[2].is_none());
    }

    #[test]
    fn a_still_water_surface_never_plays_river_or_waterfall_roar() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            base_height: 5.0,
            sea_level: 8.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            biomes: false,
            trees: false,
            rivers: false,
            water: true,
            caves: false,
            ..default()
        })
        .unwrap();
        let sources = scan_sources(&terrain, Vec3::new(-0.5, 8.5, -0.5));
        assert!(sources[0].is_some_and(|emitter| emitter.strength <= 0.055));
        assert!(sources[1].is_none());
        assert!(sources[2].is_none());
    }

    #[test]
    fn rock_roof_and_unloaded_cells_attenuate_water_while_open_paths_do_not() {
        let source = Vec3::new(-2.5, 1.2, -3.5);
        let listener = Vec3::new(-2.5, 8.7, -3.5);
        assert_eq!(acoustic_transmission(listener, source, |_| Some(0)), 1.0);
        let roof = |p: [i64; 3]| Some(if p[1] == 5 { 3 } else { 0 });
        assert_eq!(acoustic_transmission(listener, source, roof), 0.05);
        assert_eq!(
            acoustic_transmission(listener, source, |p| {
                if p[1] == 5 { None } else { Some(0) }
            }),
            0.05
        );
        let beneath_roof = Vec3::new(-2.4, 2.0, -3.5);
        assert_eq!(acoustic_transmission(beneath_roof, source, roof), 1.0);
    }

    #[test]
    fn nearby_water_remains_audible_and_occlusion_queries_are_bounded() {
        let source = Vec3::new(0.5, 0.5, 0.5);
        assert_eq!(acoustic_transmission(source, source, |_| Some(0)), 1.0);
        assert_eq!(
            acoustic_transmission(source + Vec3::X * 0.2, source, |_| Some(0)),
            1.0
        );
        let mut queries = 0;
        let listener = source + Vec3::splat(25.0);
        assert_eq!(
            acoustic_transmission(listener, source, |_| {
                queries += 1;
                Some(0)
            }),
            1.0
        );
        assert!(queries <= 80);
        assert_eq!(
            acoustic_transmission(source + Vec3::X * 45.0, source, |_| panic!("out of range")),
            0.0
        );
    }
}
