//! Opt-in, bounded wall-clock telemetry for native long-running regression runs.
use bevy::{prelude::*, text::FontAtlasSet};
use std::{
    collections::{BTreeMap, HashSet},
    time::{Duration, Instant},
};
use terra_render::{MeshUploadStatus, PropRenderStatus, RenderStatus, WaterStatus};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, GenerationStatus, WorldGeneration},
};

const PERIOD: Duration = Duration::from_secs(30);
const MAX_SAMPLES: usize = 65_536;

#[derive(Resource)]
struct Soak {
    started: Option<Instant>,
    interval: Instant,
    previous: Instant,
    frames: u64,
    times: Vec<f64>,
    dropped_samples: u64,
    upload_vertices: u64,
    uploaded_frames: u64,
    previous_scopes: BTreeMap<&'static str, terra_core::profiling::ScopeSample>,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::var("TERRA_SOAK_REPORT").is_ok_and(|value| value == "1") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(Soak {
        started: None,
        interval: now,
        previous: now,
        frames: 0,
        times: Vec::with_capacity(8192),
        dropped_samples: 0,
        upload_vertices: 0,
        uploaded_frames: 0,
        previous_scopes: BTreeMap::new(),
    })
    .add_systems(Last, sample);
}

fn sample(world: &mut World) {
    let now = Instant::now();
    world.resource_scope(|world, mut state: Mut<Soak>| {
        if state.started.is_none() {
            if !ready(world) {
                state.previous = now;
                return;
            }
            state.started = Some(now);
            state.interval = now;
            state.previous = now;
            state.previous_scopes = terra_core::profiling::snapshot().into_iter().collect();
            tracing::info!("Performance soak sampling started after initial scene readiness; intervals use wall time, GPU pass values are not total GPU frame timings");
            return;
        }
        let seconds = now.duration_since(state.previous).as_secs_f64();
        state.previous = now;
        state.frames += 1;
        if state.times.len() < MAX_SAMPLES {
            state.times.push(seconds * 1000.0);
        } else {
            state.dropped_samples += 1;
        }
        if let Some(upload) = world.get_resource::<MeshUploadStatus>() {
            state.upload_vertices = state.upload_vertices.saturating_add(upload.admitted_vertices as u64);
            state.uploaded_frames += u64::from(upload.admitted_vertices > 0);
        }
        if now.duration_since(state.interval) < PERIOD {
            return;
        }
        report(world, &mut state, now);
        state.interval = now;
        state.frames = 0;
        state.times.clear();
        state.dropped_samples = 0;
        state.upload_vertices = 0;
        state.uploaded_frames = 0;
    });
}

fn ready(world: &World) -> bool {
    if std::env::args().any(|argument| argument.starts_with("--smoke-test")) {
        return crate::smoke_test::benchmark_ready_time(world).is_some();
    }
    world
        .get_resource::<WorldGeneration>()
        .is_some_and(|generation| generation.status == GenerationStatus::Ready)
        && world
            .get_resource::<RenderStatus>()
            .is_some_and(|status| status.ready)
        && world
            .get_resource::<PropRenderStatus>()
            .is_none_or(|status| status.ready)
        && world
            .get_resource::<WaterStatus>()
            .is_none_or(|status| status.ready)
        && world
            .get_resource::<terra_ui::UiDocumentState>()
            .is_none_or(|ui| matches!(ui.0.load, terra_ui::UiLoadState::Ready))
        && world
            .get_resource::<terra_ui::menu::MenuState>()
            .is_none_or(|menu| menu.page == terra_ui::menu::MenuPage::Playing)
}

fn report(world: &mut World, state: &mut Soak, now: Instant) {
    let elapsed = state
        .started
        .map_or(0.0, |started| now.duration_since(started).as_secs_f64());
    let interval = now.duration_since(state.interval).as_secs_f64();
    let summary = frame_summary(&mut state.times);
    let rss = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|text| resident_bytes(&text));
    let entities = world.entities().count_spawned();
    let allocated_entity_indices = world.entities().len();
    let physical_resolution = world
        .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|window| (window.physical_width(), window.physical_height()));
    let mesh_entities = world
        .query_filtered::<Entity, With<Mesh3d>>()
        .iter(world)
        .count();
    let visible_mesh_entities = world
        .query::<(&Mesh3d, &ViewVisibility)>()
        .iter(world)
        .filter(|(_, visible)| visible.get())
        .count();
    let gpu_only_mesh_entities = world
        .query_filtered::<Entity, (With<Mesh3d>, With<bevy::camera::visibility::NoCpuCulling>)>()
        .iter(world)
        .count();
    let meshes = world.get_resource::<Assets<Mesh>>().map_or(0, Assets::len);
    let cpu_mesh_vertices: usize = world.get_resource::<Assets<Mesh>>().map_or(0, |assets| {
        assets.iter().map(|(_, mesh)| readable_vertices(mesh)).sum()
    });
    let materials = world
        .get_resource::<Assets<StandardMaterial>>()
        .map_or(0, Assets::len);
    let images = image_summary(
        world.get_resource::<Assets<Image>>(),
        world.get_resource::<FontAtlasSet>(),
    );
    let cpu = world
        .get_resource::<terra_ui::AppProcessStats>()
        .and_then(|stats| stats.cpu_percent);
    tracing::info!(elapsed_seconds = elapsed, interval_seconds = interval, frames = state.frames,
        fps = state.frames as f64 / interval, median_ms = summary[0], p95_ms = summary[1], p99_ms = summary[2], max_ms = summary[3],
        rss_bytes = ?rss, cpu_capacity_percent = ?cpu, physical_resolution = ?physical_resolution,
        entities, allocated_entity_indices, mesh_entities, visible_mesh_entities, gpu_only_mesh_entities,
        mesh_assets = meshes, cpu_mesh_vertices, standard_material_assets = materials, image_assets = images.assets,
        upload_vertices = state.upload_vertices, upload_frames = state.uploaded_frames, dropped_samples = state.dropped_samples,
        "Performance soak interval");
    tracing::info!(
        elapsed_seconds = elapsed,
        cpu_image_data_bytes = images.cpu_data_bytes,
        descriptor_texture_bytes_estimate = images.descriptor_texture_bytes_estimate,
        descriptor_size_unknown_images = images.descriptor_size_unknown_images,
        data_none_images = images.data_none_images,
        render_world_only_images = images.render_world_only_images,
        images_512_square = images.images_512_square,
        images_1024_square = images.images_1024_square,
        font_atlas_sets = images.font_atlas_sets,
        font_atlas_records = images.font_atlas_records,
        font_atlas_face_ids = images.font_atlas_face_ids,
        font_atlas_font_size_bits = images.font_atlas_font_size_bits,
        font_atlas_font_size_min = ?images.font_atlas_font_size_min,
        font_atlas_font_size_max = ?images.font_atlas_font_size_max,
        font_atlas_variations_hashes = images.font_atlas_variations_hashes,
        font_atlas_texture_ids = images.font_atlas_texture_ids,
        font_atlas_images_present = images.font_atlas_images_present,
        font_atlas_cpu_image_data_bytes = images.font_atlas_cpu_data_bytes,
        font_atlas_descriptor_texture_bytes_estimate = images.font_atlas_descriptor_texture_bytes_estimate,
        "Performance soak image resources; descriptor estimates are not measured VRAM"
    );
    if let Some(fonts) = world.get_resource::<terra_ui::UiFontCacheStatus>() {
        tracing::info!(
            elapsed_seconds = elapsed,
            pinned_fonts = fonts.pinned_fonts,
            pinned_bytes = fonts.pinned_bytes,
            evicted_fonts = fonts.evicted_fonts,
            oversized_fonts = fonts.oversized_fonts,
            "Performance soak UI font cache"
        );
    }
    if let (Some(clock), Some(weather)) = (
        world.get_resource::<terra_core::WorldClock>(),
        world.get_resource::<terra_core::WorldWeather>(),
    ) {
        tracing::info!(elapsed_seconds = elapsed, hour = clock.hour(), day = clock.day,
            weather = ?weather.pattern, weather_intensity = weather.intensity(),
            "Performance soak environment");
    }
    if let Some(replicated) = world.get_resource::<terra_network::client::ReplicatedCreatures>() {
        let hungry = replicated
            .creatures
            .values()
            .filter(|track| track.current.needs.food < track.current.kind.food() * 0.85)
            .count();
        let thirsty = replicated
            .creatures
            .values()
            .filter(|track| track.current.needs.water < track.current.kind.water() * 0.85)
            .count();
        tracing::info!(
            elapsed_seconds = elapsed,
            creatures = replicated.creatures.len(),
            flowers = replicated.ecology.flowers.len(),
            nests = replicated.ecology.nests.len(),
            hungry,
            thirsty,
            "Performance soak replicated population"
        );
    }
    if let Some(session) = world.get_resource::<terra_network::client::ClientSession>() {
        let smoke_cells: usize = session
            .furnaces
            .iter()
            .map(|furnace| furnace.smoke_cells.len())
            .sum();
        let lit = session
            .furnaces
            .iter()
            .filter(|furnace| furnace.lit)
            .count();
        tracing::info!(
            elapsed_seconds = elapsed,
            furnaces = session.furnaces.len(),
            lit,
            smoke_cells,
            "Performance soak replicated furnaces"
        );
    }
    if let Some(chunks) = world.get_resource::<ChunkStore>() {
        let props: usize = chunks
            .loaded
            .values()
            .map(|section| section.props.len())
            .sum();
        tracing::info!(
            elapsed_seconds = elapsed,
            sections = chunks.loaded.len(),
            stored_props = props,
            "Performance soak world resources"
        );
    }
    if let Some(terrain) = world.get_resource::<terra_render::RenderedSections>() {
        let mut sections = [0_usize; 3];
        let mut meshes = [0_usize; 3];
        for section in terrain.sections.values() {
            let index = match section.detail_level {
                terra_render::meshing::DetailLevel::Near => 0,
                terra_render::meshing::DetailLevel::Middle => 1,
                terra_render::meshing::DetailLevel::Far => 2,
            };
            sections[index] += 1;
            meshes[index] += section.meshes.len();
        }
        tracing::info!(
            elapsed_seconds = elapsed,
            ?sections,
            ?meshes,
            "Performance soak terrain ownership; near/middle/far live mesh handles, not draw calls"
        );
    }
    if let Some(generation) = world.get_resource::<WorldGeneration>() {
        tracing::info!(elapsed_seconds = elapsed, status = ?generation.status, sections = generation.loaded_sections,
            wanted_sections = generation.wanted_sections, pending_jobs = generation.pending_jobs, prepared_columns = generation.prepared_columns,
            "Performance soak generation");
    }
    if let Some(render) = world.get_resource::<RenderStatus>() {
        tracing::info!(
            elapsed_seconds = elapsed,
            ready = render.ready,
            jobs = render.pending_jobs,
            pending_columns = render.pending_columns.len(),
            completed_jobs = render.completed_mesh_jobs,
            composition_seconds = render.composition_seconds,
            max_composition_seconds = render.max_composition_seconds,
            "Performance soak terrain"
        );
    }
    if let Some(props) = world.get_resource::<PropRenderStatus>() {
        tracing::info!(
            elapsed_seconds = elapsed,
            props = props.visible_props,
            batches = props.batches,
            jobs = props.pending_jobs,
            completed_jobs = props.completed_jobs,
            composition_seconds = props.composition_seconds,
            "Performance soak props"
        );
    }
    if let Some(water) = world.get_resource::<WaterStatus>() {
        tracing::info!(
            elapsed_seconds = elapsed,
            tiles = water.visible_tiles,
            triangles = water.triangles,
            spray = water.spray_particles,
            mist = water.mist_volumes,
            "Performance soak water rendering"
        );
    }
    if let Some(upload) = world.get_resource::<MeshUploadStatus>() {
        tracing::info!(
            elapsed_seconds = elapsed,
            deferred_batches = upload.deferred_batches,
            peak_frame_vertices = upload.peak_admitted_vertices,
            "Performance soak upload budgets"
        );
    }
    if let Some(journal) = world.get_resource::<EditJournal>() {
        let gameplay = journal.gameplay();
        let fluid = journal.fluid_work_counts();
        tracing::info!(elapsed_seconds = elapsed, changed_sections = journal.changed_sections().count(), surface_conditions = journal.surface_condition_count(),
            drops = gameplay.drops.len(), creatures = gameplay.creatures.animals.len(), flowers = gameplay.creatures.ecology.flowers.len(),
            nests = gameplay.creatures.ecology.nests.len(), furnaces = journal.furnaces().count(), fluid = ?fluid,
            "Performance soak journal");
    }
    if let Some(fluid) = world.get_resource::<terra_world::water::FluidDiagnostics>() {
        tracing::info!(
            elapsed_seconds = elapsed,
            visited = fluid.visited,
            elapsed_ms = fluid.elapsed_ms,
            "Performance soak latest fluid step"
        );
    }
    if let Some(draws) = world
        .get_resource::<terra_render::DrawCallStatus>()
        .and_then(|draws| draws.snapshot())
    {
        tracing::info!(elapsed_seconds = elapsed, snapshot = ?draws, "Performance soak latest draw submission");
    }
    if let Some(diagnostics) = world.get_resource::<bevy::diagnostic::DiagnosticsStore>() {
        for diagnostic in diagnostics.iter().filter(|diagnostic| {
            diagnostic.path().as_str().ends_with("/elapsed_gpu")
                || diagnostic.path().as_str().ends_with("/elapsed_cpu")
        }) {
            if let Some(measurement) = diagnostic.measurement() {
                tracing::info!(
                    elapsed_seconds = elapsed,
                    diagnostic = diagnostic.path().as_str(),
                    value_ms = measurement.value,
                    "Performance soak latest render pass diagnostic"
                );
            }
        }
    }
    for (name, current) in terra_core::profiling::snapshot() {
        let previous = state
            .previous_scopes
            .insert(name, current)
            .unwrap_or_default();
        let calls = current.calls.saturating_sub(previous.calls);
        if calls == 0 || name == "perf.soak" {
            continue;
        }
        let total_ms = (current.total_ms - previous.total_ms).max(0.0);
        tracing::info!(
            elapsed_seconds = elapsed,
            scope = name,
            calls,
            total_ms,
            mean_ms = total_ms / calls as f64,
            cumulative_max_ms = current.max_ms,
            over_8_ms = current.over_8_ms.saturating_sub(previous.over_8_ms),
            "Performance soak CPU scope interval"
        );
    }
}

pub(super) fn frame_summary(values: &mut [f64]) -> [f64; 4] {
    if values.is_empty() {
        return [0.0; 4];
    }
    values.sort_unstable_by(f64::total_cmp);
    [0.5, 0.95, 0.99, 1.0].map(|fraction| {
        values[((values.len() as f64 * fraction).ceil() as usize).saturating_sub(1)]
    })
}

fn resident_bytes(text: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let value = line
            .strip_prefix("VmRSS:")?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()?;
        value.checked_mul(1024)
    })
}

fn readable_vertices(mesh: &Mesh) -> usize {
    mesh.try_attribute(Mesh::ATTRIBUTE_POSITION)
        .map_or(0, |positions| positions.len())
}

#[derive(Default)]
struct ImageSummary {
    assets: usize,
    cpu_data_bytes: u64,
    descriptor_texture_bytes_estimate: u64,
    descriptor_size_unknown_images: usize,
    data_none_images: usize,
    render_world_only_images: usize,
    images_512_square: usize,
    images_1024_square: usize,
    font_atlas_sets: usize,
    font_atlas_records: usize,
    font_atlas_face_ids: usize,
    font_atlas_font_size_bits: usize,
    font_atlas_font_size_min: Option<f32>,
    font_atlas_font_size_max: Option<f32>,
    font_atlas_variations_hashes: usize,
    font_atlas_texture_ids: usize,
    font_atlas_images_present: usize,
    font_atlas_cpu_data_bytes: u64,
    font_atlas_descriptor_texture_bytes_estimate: u64,
}

fn image_summary(images: Option<&Assets<Image>>, fonts: Option<&FontAtlasSet>) -> ImageSummary {
    let mut summary = ImageSummary::default();
    let mut font_textures = HashSet::new();
    if let Some(fonts) = fonts {
        let mut face_ids = HashSet::new();
        let mut font_size_bits = HashSet::new();
        let mut variations_hashes = HashSet::new();
        summary.font_atlas_sets = fonts.len();
        for (key, atlases) in fonts.iter() {
            face_ids.insert((key.id, key.index));
            font_size_bits.insert(key.font_size_bits);
            variations_hashes.insert(key.variations_hash);
            let font_size = f32::from_bits(key.font_size_bits);
            if font_size.is_finite() {
                summary.font_atlas_font_size_min = Some(
                    summary
                        .font_atlas_font_size_min
                        .map_or(font_size, |previous| previous.min(font_size)),
                );
                summary.font_atlas_font_size_max = Some(
                    summary
                        .font_atlas_font_size_max
                        .map_or(font_size, |previous| previous.max(font_size)),
                );
            }
            summary.font_atlas_records += atlases.len();
            font_textures.extend(atlases.iter().map(|atlas| atlas.texture.id()));
        }
        summary.font_atlas_face_ids = face_ids.len();
        summary.font_atlas_font_size_bits = font_size_bits.len();
        summary.font_atlas_variations_hashes = variations_hashes.len();
    }
    summary.font_atlas_texture_ids = font_textures.len();
    let Some(images) = images else {
        return summary;
    };
    // One read-only image scan per opt-in report, with shared atlas IDs counted once.
    for (id, image) in images.iter() {
        summary.assets += 1;
        let cpu_bytes = image.data.as_ref().map_or(0, |data| data.len() as u64);
        summary.cpu_data_bytes = summary.cpu_data_bytes.saturating_add(cpu_bytes);
        summary.data_none_images += usize::from(image.data.is_none());
        summary.render_world_only_images += usize::from(
            image
                .asset_usage
                .contains(bevy::asset::RenderAssetUsages::RENDER_WORLD)
                && !image
                    .asset_usage
                    .contains(bevy::asset::RenderAssetUsages::MAIN_WORLD),
        );
        let size = image.texture_descriptor.size;
        summary.images_512_square += usize::from(size.width == 512 && size.height == 512);
        summary.images_1024_square += usize::from(size.width == 1024 && size.height == 1024);
        let descriptor_bytes = descriptor_texture_bytes(image);
        if let Some(bytes) = descriptor_bytes {
            summary.descriptor_texture_bytes_estimate = summary
                .descriptor_texture_bytes_estimate
                .saturating_add(bytes);
        } else {
            summary.descriptor_size_unknown_images += 1;
        }
        if font_textures.contains(&id) {
            summary.font_atlas_images_present += 1;
            summary.font_atlas_cpu_data_bytes =
                summary.font_atlas_cpu_data_bytes.saturating_add(cpu_bytes);
            summary.font_atlas_descriptor_texture_bytes_estimate = summary
                .font_atlas_descriptor_texture_bytes_estimate
                .saturating_add(descriptor_bytes.unwrap_or(0));
        }
    }
    summary
}

fn descriptor_texture_bytes(image: &Image) -> Option<u64> {
    // Texel block footprint only: not GPU allocation, alignment, residency or VRAM usage.
    let descriptor = &image.texture_descriptor;
    let block_bytes = u64::from(descriptor.format.block_copy_size(None)?);
    let (block_width, block_height) = descriptor.format.block_dimensions();
    let mut bytes = 0_u64;
    for level in 0..descriptor.mip_level_count {
        let size = descriptor.mip_level_size(level)?;
        let level_bytes = u64::from(size.width)
            .div_ceil(u64::from(block_width))
            .checked_mul(u64::from(size.height).div_ceil(u64::from(block_height)))?
            .checked_mul(u64::from(size.depth_or_array_layers))?
            .checked_mul(block_bytes)?
            .checked_mul(u64::from(descriptor.sample_count))?;
        bytes = bytes.checked_add(level_bytes)?;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn telemetry_accepts_meshes_whose_cpu_vertex_data_has_been_extracted() {
        use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology};
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0_f32; 3]; 3]);
        assert_eq!(readable_vertices(&mesh), 3);
        let _gpu_data = mesh.take_gpu_data().unwrap();
        assert_eq!(readable_vertices(&mesh), 0);
    }
    #[test]
    fn wall_time_percentiles_use_nearest_rank_without_allocating_a_second_window() {
        let mut values: Vec<_> = (1..=100).rev().map(f64::from).collect();
        assert_eq!(frame_summary(&mut values), [50.0, 95.0, 99.0, 100.0]);
        assert_eq!(frame_summary(&mut []), [0.0; 4]);
    }
    #[test]
    fn rss_parsing_requires_the_resident_field_and_does_not_overflow() {
        assert_eq!(
            resident_bytes("VmSize: 900 kB\nVmRSS:\t123 kB\n"),
            Some(125_952)
        );
        assert_eq!(resident_bytes("VmSize: 900 kB\n"), None);
        assert_eq!(resident_bytes("VmRSS: 18446744073709551615 kB"), None);
    }
    #[test]
    fn image_aggregation_separates_cpu_data_descriptors_and_font_atlas_overlap() {
        use bevy::{
            asset::RenderAssetUsages,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat},
            text::{FontAtlas, FontAtlasKey, FontHinting, FontSmoothing},
        };
        let mut images = Assets::<Image>::default();
        let atlas = FontAtlas::new(&mut images, UVec2::splat(512), FontSmoothing::AntiAliased);
        let mut fonts = FontAtlasSet::default();
        fonts.insert(
            FontAtlasKey {
                id: 1,
                index: 0,
                font_size_bits: 16_f32.to_bits(),
                variations_hash: 0,
                hinting: FontHinting::Disabled,
                font_smoothing: FontSmoothing::AntiAliased,
            },
            vec![atlas],
        );
        let second_atlas =
            FontAtlas::new(&mut images, UVec2::splat(512), FontSmoothing::AntiAliased);
        fonts.insert(
            FontAtlasKey {
                id: 1,
                index: 1,
                font_size_bits: 24_f32.to_bits(),
                variations_hash: 7,
                hinting: FontHinting::Disabled,
                font_smoothing: FontSmoothing::AntiAliased,
            },
            vec![second_atlas],
        );
        let mut gpu_only = Image::new_uninit(
            Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 2,
            },
            TextureDimension::D2,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        gpu_only.texture_descriptor.mip_level_count = 2;
        let _gpu_only = images.add(gpu_only);
        let summary = image_summary(Some(&images), Some(&fonts));
        assert_eq!(summary.assets, 3);
        assert_eq!(summary.cpu_data_bytes, 2 * 512 * 512 * 4);
        assert_eq!(
            summary.descriptor_texture_bytes_estimate,
            2 * 512 * 512 * 4 + (1024 * 1024 + 512 * 512) * 2 * 4
        );
        assert_eq!(summary.descriptor_size_unknown_images, 0);
        assert_eq!(summary.data_none_images, 1);
        assert_eq!(summary.render_world_only_images, 1);
        assert_eq!(summary.images_512_square, 2);
        assert_eq!(summary.images_1024_square, 1);
        assert_eq!(summary.font_atlas_sets, 2);
        assert_eq!(summary.font_atlas_records, 2);
        assert_eq!(summary.font_atlas_face_ids, 2);
        assert_eq!(summary.font_atlas_font_size_bits, 2);
        assert_eq!(summary.font_atlas_font_size_min, Some(16.0));
        assert_eq!(summary.font_atlas_font_size_max, Some(24.0));
        assert_eq!(summary.font_atlas_variations_hashes, 2);
        assert_eq!(summary.font_atlas_texture_ids, 2);
        assert_eq!(summary.font_atlas_images_present, 2);
        assert_eq!(summary.font_atlas_cpu_data_bytes, 2 * 512 * 512 * 4);
        assert_eq!(
            summary.font_atlas_descriptor_texture_bytes_estimate,
            2 * 512 * 512 * 4
        );
    }
    #[test]
    fn descriptor_estimates_handle_volume_mips_compressed_blocks_and_unknown_formats() {
        use bevy::{
            asset::RenderAssetUsages,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat},
        };
        let mut image = Image::new_uninit(
            Extent3d {
                width: 8,
                height: 4,
                depth_or_array_layers: 4,
            },
            TextureDimension::D3,
            TextureFormat::R8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.texture_descriptor.mip_level_count = 4;
        assert_eq!(descriptor_texture_bytes(&image), Some(147));
        image.texture_descriptor.dimension = TextureDimension::D2;
        image.texture_descriptor.size = Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        };
        image.texture_descriptor.format = TextureFormat::Bc1RgbaUnorm;
        assert_eq!(descriptor_texture_bytes(&image), Some(56));
        image.texture_descriptor.format = TextureFormat::Depth24Plus;
        assert_eq!(descriptor_texture_bytes(&image), None);
    }
}
