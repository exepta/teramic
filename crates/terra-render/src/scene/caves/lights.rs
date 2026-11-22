//! A bounded pool of real PBR lights, discovered only when section geometry changes.
mod distant;
mod soft_core;
mod sunlight;
pub(super) use sunlight::known_top;
mod updates;
use crate::blocks::{BlockAssets, definitions::BlockLight};
use bevy::prelude::*;
pub use soft_core::TorchLightingStatus;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
};
use terra_core::{CaveQuality, GraphicsConfig};
use terra_voxel::{SectionPos, global_to_section};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};

#[derive(Component)]
struct VoxelLamp([i64; 3], BlockLight, Vec3);
#[derive(Resource, Default)]
pub struct TorchSources(pub Vec<Vec3>);
/// Observable streaming latency/work counters; no GPU or per-lamp allocation.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct LightDiscoveryStatus {
    pub selections: u64,
    pub source_scans: u64,
    pub fast_rejections: u64,
    pub pending_sections: usize,
    pub cached_sections: usize,
    pub discovered_emitters: usize,
}
#[derive(Default)]
struct Cached {
    epoch: u64,
    neighbors: u8,
    sources: Vec<([i64; 3], u16, BlockLight)>,
    lamps: Vec<([i64; 3], BlockLight)>,
}
#[derive(Resource, Default)]
struct Lamps {
    sections: HashMap<SectionPos, Cached>,
    emitters: Vec<([i64; 3], BlockLight)>,
    focus: Option<[i64; 3]>,
    dirty: bool,
    next: f32,
    updates: updates::State,
    diagnostics: LightDiscoveryStatus,
}
type LampEntities<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut VoxelLamp,
        &'static mut Transform,
        &'static mut PointLight,
    ),
    Without<terra_player::TerraCamera>,
>;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ApplyLocalLights;

fn configure_light_updates(app: &mut App) {
    // Daylight writes a provisional outdoor exposure during Update; caves
    // replace it in PostUpdate. Reading the provisional value makes a noon
    // torch up to 208 times too bright after cave eye adaptation.
    app.configure_sets(
        PostUpdate,
        ApplyLocalLights
            .after(super::exposure)
            .before(bevy::transform::TransformSystems::Propagate),
    );
}

pub(super) fn install(app: &mut App) {
    soft_core::install(app);
    distant::install(app);
    configure_light_updates(app);
    app.init_resource::<Lamps>()
        .init_resource::<LightDiscoveryStatus>()
        .init_resource::<TorchSources>()
        .init_resource::<sunlight::State>()
        .add_systems(
            Update,
            (shadow_resolution, select)
                .chain()
                .after(terra_player::PlayerSet::Movement),
        )
        .add_systems(Update, publish_diagnostics.after(select))
        .add_systems(
            PostUpdate,
            (held, sunlight::update, flicker)
                .chain()
                .in_set(ApplyLocalLights),
        );
}
fn publish_diagnostics(cache: Res<Lamps>, mut status: ResMut<LightDiscoveryStatus>) {
    *status = LightDiscoveryStatus {
        pending_sections: cache.updates.pending_len(),
        cached_sections: cache.sections.len(),
        discovered_emitters: cache.emitters.len(),
        ..cache.diagnostics
    };
}
fn shadow_resolution(
    graphics: Option<Res<GraphicsConfig>>,
    mut map: ResMut<bevy::light::PointLightShadowMap>,
) {
    let quality = graphics
        .as_ref()
        .map_or(CaveQuality::Medium, |g| g.cave_quality);
    let size = if matches!(quality, CaveQuality::Low | CaveQuality::Medium) {
        512
    } else {
        1024
    };
    if map.size != size {
        map.size = size;
    }
}
fn select(
    mut commands: Commands,
    (time, access, graphics): (Res<Time>, Res<WorldAccess>, Option<Res<GraphicsConfig>>),
    (chunks, journal, assets): (Res<ChunkStore>, Res<EditJournal>, Res<BlockAssets>),
    cameras: Query<&Transform, With<terra_player::TerraCamera>>,
    mut lamps: LampEntities,
    mut cache: ResMut<Lamps>,
    (soft_core, messages): (Res<TorchLightingStatus>, updates::Messages),
) {
    let _scope = terra_core::profiling::ScopeTimer::new("lights.select");
    let incremental = matches!(*access, WorldAccess::Replica { .. }) && messages.available();
    if cache.updates.enabled != incremental || incremental && access.is_changed() {
        updates::clear(&mut cache);
        cache.updates = updates::State::default();
        cache.updates.enabled = incremental;
        cache.dirty = true;
    }
    // Cursors must advance every frame, even between the selection ticks.
    let urgent = incremental && updates::collect(&mut cache, &chunks, &messages);
    if !incremental {
        cache.dirty |= chunks.is_changed()
            || journal.is_changed()
            || assets.is_changed()
            || graphics.as_ref().is_some_and(|g| g.is_changed());
    }
    let Some(camera) = cameras.iter().next().filter(|_| access.ready()) else {
        for (entity, _, _, _) in &mut lamps {
            commands.entity(entity).despawn();
        }
        updates::clear(&mut cache);
        return;
    };
    if time.elapsed_secs() < cache.next && !(urgent || !incremental && journal.is_changed()) {
        return;
    }
    cache.next = time.elapsed_secs() + 0.1;
    cache.diagnostics.selections += 1;
    let definitions: BTreeMap<_, _> = assets.lights().collect();
    if definitions.is_empty() {
        for (entity, _, _, _) in &mut lamps {
            commands.entity(entity).despawn();
        }
        updates::clear(&mut cache);
        return;
    }
    let focus = camera.translation;
    let reach = graphics
        .as_ref()
        .map_or(192.0, |g| f32::from(g.render_distance) * 16.0)
        + 28.0;
    let focus_cell = (focus / 16.0).floor().as_i64vec3().to_array();
    if incremental {
        updates::refresh(&mut cache, &chunks, &journal, &definitions, focus, reach);
    } else if cache.dirty || cache.focus != Some(focus_cell) {
        cache.focus = Some(focus_cell);
        discover(&mut cache, &chunks, &journal, &definitions, focus, reach);
    }
    let quality = graphics
        .as_ref()
        .map_or(CaveQuality::Medium, |g| g.cave_quality);
    let budget = match quality {
        CaveQuality::Low => 4,
        CaveQuality::Medium => 8,
        CaveQuality::High => 12,
        CaveQuality::Ultra => 16,
    };
    let selected = select_emitters(
        &cache.emitters,
        &chunks,
        &journal,
        focus,
        *camera.forward(),
        budget,
    );
    sync_lamps(
        &mut commands,
        &mut lamps,
        selected,
        &chunks,
        &journal,
        &soft_core,
        graphics.as_deref(),
    );
}

fn sync_lamps(
    commands: &mut Commands,
    lamps: &mut LampEntities,
    mut selected: BTreeMap<[i64; 3], BlockLight>,
    chunks: &ChunkStore,
    journal: &EditJournal,
    soft_core: &TorchLightingStatus,
    graphics: Option<&GraphicsConfig>,
) {
    let mut reusable = Vec::new();
    for (entity, mut lamp, mut transform, _) in lamps.iter_mut() {
        if !retain_definition(&mut lamp, &mut selected) {
            reusable.push(entity);
            continue;
        }
        {
            let position = emitter_location(
                journal,
                lamp.0,
                lamp.1,
                location(
                    lamp.0,
                    journal.block(lamp.0).or_else(|| chunks.block_at(lamp.0)) == Some(40),
                    |p| chunks.block_at(p).map(|m| journal.block(p).unwrap_or(m)),
                ),
            );
            if transform.translation != position {
                transform.translation = position;
            }
            lamp.2 = position;
        }
    }
    for (p, light) in selected {
        let position = emitter_location(
            journal,
            p,
            light,
            location(
                p,
                journal.block(p).or_else(|| chunks.block_at(p)) == Some(40),
                |p| chunks.block_at(p).map(|m| journal.block(p).unwrap_or(m)),
            ),
        );
        let point_light = PointLight {
            color: Color::srgb(light.color[0], light.color[1], light.color[2]),
            intensity: light.intensity,
            range: light.range,
            radius: soft_core.radius(light),
            shadow_maps_enabled: source_shadows(light, true, graphics),
            shadow_depth_bias: 0.035,
            shadow_normal_bias: 0.16,
            ..default()
        };
        // Keep the main entity and its six Bevy shadow views when a different
        // emitter enters the bounded pool. Only a smaller pool retires slots.
        if let Some(entity) = reusable.pop()
            && let Ok((_, mut lamp, mut transform, mut existing_light)) = lamps.get_mut(entity)
        {
            *lamp = VoxelLamp(p, light, position);
            transform.translation = position;
            *existing_light = point_light;
            continue;
        }
        commands.spawn((
            Name::new("Voxel local light"),
            VoxelLamp(p, light, position),
            bevy::camera::visibility::RenderLayers::default()
                .with(crate::meshing::shadows::LAYER)
                .with(crate::transitions::QUEUE_SHADOW_LAYER),
            point_light,
            Transform::from_translation(position),
        ));
    }
    for entity in reusable {
        commands.entity(entity).despawn();
    }
}

fn select_emitters(
    emitters: &[([i64; 3], BlockLight)],
    chunks: &ChunkStore,
    journal: &EditJournal,
    focus: Vec3,
    forward: Vec3,
    budget: usize,
) -> BTreeMap<[i64; 3], BlockLight> {
    let mut candidates: Vec<_> = emitters
        .iter()
        .filter_map(|&(cell, light)| {
            let section = global_to_section(cell).0;
            let delta = Vec3::from_array(cell.map(|v| v as f32)) - focus;
            chunks
                .loaded
                .contains_key(&section)
                .then_some((delta.length_squared(), cell, light))
        })
        .collect();
    candidates.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    candidates.dedup_by_key(|candidate| candidate.1);

    let exposed = |cell, light| {
        let block = |p| chunks.block_at(p).map(|id| journal.block(p).unwrap_or(id));
        let source = emitter_location(
            journal,
            cell,
            light,
            location(cell, block(cell) == Some(40), block),
        );
        visible(
            focus,
            source,
            |p| if p == cell { Some(0) } else { block(p) },
        )
    };

    // Reserve a few of the existing slots for visible sections farther ahead.
    // A dense near wall of crystals must not leave the next chamber completely dark.
    let mut selected = BTreeMap::new();
    let mut sampled = HashMap::<SectionPos, u8>::new();
    for &(_, cell, _) in candidates.iter().take(budget - budget / 4) {
        sampled.insert(global_to_section(cell).0, 3);
    }
    let mut probes = 0;
    for &(distance, cell, light) in &candidates {
        if selected.len() >= budget / 4 || probes >= 32 {
            break;
        }
        let delta = Vec3::from_array(cell.map(|v| v as f32)) - focus;
        if delta.dot(forward) < 0.4 * distance.sqrt() {
            continue;
        }
        let section = global_to_section(cell).0;
        let count = sampled.entry(section).or_default();
        if *count >= 3 {
            continue;
        }
        *count += 1;
        probes += 1;
        if exposed(cell, light) {
            selected.insert(cell, light);
            *count = 3;
        }
    }
    let mut ranked = Vec::with_capacity(64);
    for (distance, cell, light) in candidates.into_iter().take(64) {
        let clear = exposed(cell, light);
        if !clear && distance > (light.range + 2.0).powi(2) {
            continue;
        }
        // Nearby light can spill around a doorway even if the bulb itself is hidden.
        let penalty = if clear {
            0.0
        } else {
            (light.range * 0.65).powi(2)
        };
        ranked.push((distance + penalty, cell, light));
    }
    ranked.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for (_, cell, light) in ranked {
        if selected.len() >= budget {
            break;
        }
        selected.insert(cell, light);
    }
    selected
}

fn discover(
    cache: &mut Lamps,
    chunks: &ChunkStore,
    journal: &EditJournal,
    definitions: &BTreeMap<u16, BlockLight>,
    focus: Vec3,
    reach: f32,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("lights.discover");
    cache.sections.retain(|p, _| chunks.loaded.contains_key(p));
    cache.dirty = false;
    let mut immediate = Vec::new();
    let mut discoveries = Vec::new();
    for (&section, data) in &chunks.loaded {
        let Ok(origin) = section.origin() else {
            continue;
        };
        let center = Vec3::from_array(origin.map(|v| v as f32)) + Vec3::splat(8.0);
        let distance = center.distance_squared(focus);
        if distance > (reach * 1.75).powi(2) {
            continue;
        }
        let epoch = journal
            .mesh_epoch(section)
            .wrapping_add(data.data.revision().rotate_left(32))
            .wrapping_add(journal.prop_epoch(section).rotate_left(17));
        if let Some(old) = cache.sections.get_mut(&section)
            && old.epoch == epoch
        {
            // Empty sections cannot gain a light from a neighboring load.
            // Occupied ones recheck only their cached emitter positions.
            if old.sources.is_empty() {
                continue;
            }
            let neighbors = neighbor_mask(section, chunks);
            old.lamps = exposed_sources(&old.sources, chunks, journal);
            old.neighbors = neighbors;
            continue;
        }
        if cache.sections.contains_key(&section)
            || journal.revision(section) > 0
            || source_free_section(section, chunks, journal, definitions)
        {
            immediate.push((section, epoch, distance));
        } else {
            discoveries.push((section, epoch, distance));
        }
    }
    // A fixed nearest-first discovery window avoids arbitrary HashMap order
    // delaying a nearby imported lamp behind thousands of empty sky sections.
    if discoveries.len() > 128 {
        cache.dirty = true;
        discoveries.select_nth_unstable_by(128, |a, b| {
            a.2.total_cmp(&b.2)
                .then((a.0.x, a.0.y, a.0.z).cmp(&(b.0.x, b.0.y, b.0.z)))
        });
        discoveries.truncate(128);
    }
    for (section, epoch, _) in immediate.into_iter().chain(discoveries) {
        if !refresh_section(cache, chunks, journal, definitions, section, epoch) {
            cache.dirty = true;
        }
    }
    // Rendering and first-person torch audio need actual emitters, not a scan
    // through every empty streamed section on every frame.
    cache.emitters.clear();
    cache.emitters.extend(
        cache
            .sections
            .values()
            .flat_map(|c| c.lamps.iter().copied()),
    );
}

fn refresh_section(
    cache: &mut Lamps,
    chunks: &ChunkStore,
    journal: &EditJournal,
    definitions: &BTreeMap<u16, BlockLight>,
    section: SectionPos,
    epoch: u64,
) -> bool {
    let data = &chunks.loaded[&section];
    if source_free_section(section, chunks, journal, definitions) {
        cache.diagnostics.fast_rejections += 1;
        cache.sections.insert(
            section,
            Cached {
                epoch,
                ..Default::default()
            },
        );
        return true;
    }
    cache.diagnostics.source_scans += 1;
    let Ok(mut sources) = discover_sources(&data.data, journal, definitions) else {
        cache.sections.remove(&section);
        return false;
    };
    let mut props = data.props.clone();
    journal.apply_prop_removals(section, &mut props);
    sources.extend(props.iter().filter_map(scarecrow_source));
    let neighbors = if sources.is_empty() {
        0
    } else {
        neighbor_mask(section, chunks)
    };
    let lamps = exposed_sources(&sources, chunks, journal);
    cache.sections.insert(
        section,
        Cached {
            epoch,
            neighbors,
            sources,
            lamps,
        },
    );
    true
}

/// Negative membership proof is independent of section density. Solid terrain
/// must not consume the bounded source-discovery queue before distant lamps.
fn source_free_section(
    section: SectionPos,
    chunks: &ChunkStore,
    journal: &EditJournal,
    definitions: &BTreeMap<u16, BlockLight>,
) -> bool {
    let data = &chunks.loaded[&section];
    journal.revision(section) == 0
        && !data
            .props
            .iter()
            .chain(journal.placed_props(section))
            .any(|prop| scarecrow_source(prop).is_some())
        && !definitions
            .keys()
            .any(|&material| data.data.may_contain(material))
}

fn retain_definition(lamp: &mut VoxelLamp, selected: &mut BTreeMap<[i64; 3], BlockLight>) -> bool {
    let Some(definition) = selected.remove(&lamp.0) else {
        return false;
    };
    lamp.1 = definition;
    true
}

fn scarecrow_source(prop: &terra_world::props::WorldProp) -> Option<([i64; 3], u16, BlockLight)> {
    if prop.kind != terra_world::props::PropKind::Scarecrow || prop.removed {
        return None;
    }
    let support = terra_world::props::placement::support(prop);
    let local =
        Quat::from_rotation_y(prop.rotation) * Vec3::new(0.64, 0.84, 0.0) * prop.visual_scale();
    let position = Vec3::from_array(prop.position.map(|v| v as f32)) + local;
    let offset = (position - Vec3::from_array(support.map(|v| v as f32))).to_array();
    Some((
        support,
        u16::MAX,
        BlockLight {
            color: [1.0, 0.76, 0.45],
            intensity: 55.0,
            range: 4.0,
            falloff: Default::default(),
            flicker: 0.0,
            cave_intensity: 1.0,
            shadows: false,
            offset: Some(offset),
        },
    ))
}

fn emitter_location(journal: &EditJournal, p: [i64; 3], light: BlockLight, ordinary: Vec3) -> Vec3 {
    let Some(offset) = light.offset else {
        return mounted_location(journal, p, ordinary);
    };
    let state = journal.object_state(p).unwrap_or_default();
    let width = if state.peer.is_some() { 2.0 } else { 1.0 };
    let point = Vec3::new(offset[0] * width - 0.5, offset[1], offset[2] - 0.5);
    Vec3::from_array(p.map(|v| v as f32))
        + Vec3::new(0.5, 0.0, 0.5)
        + Quat::from_rotation_y(f32::from(state.facing) * std::f32::consts::FRAC_PI_2) * point
}

fn mounted_location(journal: &EditJournal, p: [i64; 3], ordinary: Vec3) -> Vec3 {
    if journal.block(p) == Some(terra_core::lantern::LANTERN) {
        return Vec3::from_array(
            terra_core::lantern::emission(journal.object_state(p).unwrap_or_default().facing)
                .map(|v| v as f32),
        ) + Vec3::from_array(p.map(|v| v as f32));
    }
    if journal.block(p) != Some(40) {
        return ordinary;
    }
    let facing = journal.object_state(p).map_or(0, |s| s.facing);
    Vec3::from_array(
        terra_core::block_shapes::torch_point(
            facing,
            [0.5, if facing >= 4 { 0.90 } else { 0.98 }, 0.5],
        )
        .map(|v| v as f32),
    ) + Vec3::from_array(p.map(|v| v as f32))
}

const NEIGHBORS: [[i64; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

fn neighbor_mask(section: SectionPos, chunks: &ChunkStore) -> u8 {
    NEIGHBORS
        .into_iter()
        .enumerate()
        .fold(0, |mask, (bit, delta)| {
            mask | if section
                .offset(delta)
                .is_ok_and(|p| chunks.loaded.contains_key(&p))
            {
                1 << bit
            } else {
                0
            }
        })
}

fn discover_sources(
    data: &terra_voxel::blocks::BlockSection,
    journal: &EditJournal,
    definitions: &BTreeMap<u16, BlockLight>,
) -> Result<Vec<([i64; 3], u16, BlockLight)>, terra_voxel::blocks::BlockError> {
    let origin = data.position().origin()?;
    let cells = effective_cells(data, journal)?;
    // Most newly streamed sections contain no emitters. Slice membership keeps
    // those scans contiguous instead of doing a tree lookup for every voxel.
    if !definitions.keys().any(|material| cells.contains(material)) {
        return Ok(Vec::new());
    }
    let mut sources = Vec::new();
    for (index, &material) in cells.iter().enumerate() {
        let Some(&light) = definitions.get(&material) else {
            continue;
        };
        let p = [
            origin[0] + (index % 16) as i64,
            origin[1] + (index / 16 % 16) as i64,
            origin[2] + (index / 256) as i64,
        ];
        if !journal.object_state(p).is_some_and(|s| s.secondary) {
            sources.push((p, material, light));
        }
    }
    Ok(sources)
}

fn exposed_sources(
    sources: &[([i64; 3], u16, BlockLight)],
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Vec<([i64; 3], BlockLight)> {
    sources
        .iter()
        .filter_map(|&(p, material, light)| {
            (matches!(material, 40 | terra_core::lantern::LANTERN)
                || NEIGHBORS.into_iter().any(|d| {
                    let q = std::array::from_fn(|i| p[i] + d[i]);
                    chunks.block_at(q).map(|m| journal.block(q).unwrap_or(m)) == Some(0)
                }))
            .then_some((p, light))
        })
        .collect()
}

fn effective_cells<'a>(
    data: &'a terra_voxel::blocks::BlockSection,
    journal: &EditJournal,
) -> Result<Cow<'a, [u16]>, terra_voxel::blocks::BlockError> {
    if journal.revision(data.position()) == 0 {
        return Ok(Cow::Borrowed(data.blocks()));
    }
    // Eight KiB of scratch only on a cache miss, not 4096 journal lookups
    // per unchanged section or a wait for an asynchronous mesh replacement.
    let mut cells = data.blocks().to_vec();
    for (p, material) in journal.snapshot(data.position())? {
        let (owner, [x, y, z]) = global_to_section(p);
        if owner == data.position() {
            cells[x + y * 16 + z * 256] = material;
        }
    }
    Ok(Cow::Owned(cells))
}

fn visible(from: Vec3, to: Vec3, mut block: impl FnMut([i64; 3]) -> Option<u16>) -> bool {
    let delta = to.as_dvec3() - from.as_dvec3();
    let distance = delta.length();
    if !distance.is_finite() {
        return false;
    }
    if distance <= 0.05 {
        return true;
    }
    let direction = delta / distance;
    let mut travelled = 0.0;
    // The shared voxel picker accepts rays up to 128 m. Split the sight line
    // so a longer, rejected query cannot accidentally report an open tunnel.
    while travelled < distance - 0.05 {
        let length = (distance - 0.05 - travelled).min(128.0);
        let hit = terra_voxel::blocks::raycast_blocks(
            (from.as_dvec3() + direction * travelled).to_array(),
            direction.to_array(),
            length,
            |p| {
                Some(u16::from(block(p).is_none_or(|id| {
                    terra_voxel::MaterialId::from_raw(id)
                        .is_none_or(terra_voxel::MaterialId::is_opaque)
                })))
            },
        );
        if hit.is_some() {
            return false;
        }
        travelled += length;
    }
    true
}

fn location(p: [i64; 3], torch: bool, mut block: impl FnMut([i64; 3]) -> Option<u16>) -> Vec3 {
    let center = Vec3::from_array(p.map(|v| v as f32)) + Vec3::splat(0.5);
    if torch {
        // The opaque emissive flame reaches y=.915: an emitter inside it
        // shadows itself completely, despite the flame looking luminous.
        return center + Vec3::Y * 0.48;
    }
    let normal = [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ]
    .into_iter()
    .find(|d| block(std::array::from_fn(|i| p[i] + d[i])) == Some(0))
    .unwrap_or([0, 0, 0]);
    center + Vec3::from_array(normal.map(|v| v as f32)) * 1.0
}

fn exposure_gain(exposure: f32) -> f32 {
    2.0_f32.powf(exposure - 6.0).clamp(1.0, 256.0)
}
fn cave_gain(light: BlockLight, blend: f32) -> f32 {
    1.0 + (light.cave_intensity - 1.0) * blend.clamp(0.0, 1.0)
}

fn source_shadows(light: BlockLight, local: bool, graphics: Option<&GraphicsConfig>) -> bool {
    local && light.shadows && crate::meshing::shadows::enabled(graphics)
}

fn flicker(
    time: Res<Time>,
    cameras: Query<&bevy::camera::Exposure, With<terra_player::TerraCamera>>,
    mut lamps: Query<(&VoxelLamp, &mut PointLight, &mut Transform)>,
    cave: Res<super::CaveState>,
    soft_core: Res<TorchLightingStatus>,
    graphics: Option<Res<GraphicsConfig>>,
    sunlight: Option<Res<sunlight::State>>,
) {
    let gain = exposure_gain(cameras.iter().next().map_or(6.0, |e| e.ev100));
    for (lamp, mut light, mut transform) in &mut lamps {
        let phase = (lamp.0[0] as f32 * 0.71 + lamp.0[2] as f32 * 0.39) % 10.0;
        let t = time.elapsed_secs();
        let intensity = lamp.1.intensity
            * soft_core.flux(lamp.1)
            * gain
            * cave_gain(lamp.1, cave.blend)
            * (1.0
                + lamp.1.flicker
                    * ((t * 4.1 + phase).sin() * 0.65 + (t * 7.3 + phase).sin() * 0.35));
        let color = Color::srgb_from_array(lamp.1.color);
        let radius = soft_core.radius(lamp.1);
        let shadow_maps_enabled = source_shadows(lamp.1, true, graphics.as_deref())
            && sunlight.as_ref().is_none_or(|state| state.shadows(lamp.0));
        if light.intensity != intensity {
            light.intensity = intensity;
        }
        if light.color != color {
            light.color = color;
        }
        if light.range != lamp.1.range {
            light.range = lamp.1.range;
        }
        if light.radius != radius {
            light.radius = radius;
        }
        if light.shadow_maps_enabled != shadow_maps_enabled {
            light.shadow_maps_enabled = shadow_maps_enabled;
        }
        let translation = lamp.2
            + Vec3::new(
                (t * 2.7 + phase).sin() * 0.045,
                (t * 5.1 + phase).sin() * 0.018,
                (t * 3.3 + phase).cos() * 0.045,
            ) * if lamp.1.flicker > 0.0 { 1.0 } else { 0.0 };
        if transform.translation != translation {
            transform.translation = translation;
        }
    }
}

#[derive(Component)]
struct HeldLamp(Option<terra_core::PlayerId>);
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn held(
    mut commands: Commands,
    assets: Res<BlockAssets>,
    access: Res<WorldAccess>,
    session: Option<Res<terra_network::client::ClientSession>>,
    selected: Option<Res<terra_network::client::SelectedInventorySlot>>,
    remotes: Option<Res<terra_network::client::ReplicatedPlayers>>,
    bodies: Query<&Transform, (With<terra_player::PlayerBody>, Without<HeldLamp>)>,
    cameras: Query<
        (&Transform, &bevy::camera::Exposure),
        (With<terra_player::TerraCamera>, Without<HeldLamp>),
    >,
    mut lights: Query<
        (Entity, &HeldLamp, &mut Transform, &mut PointLight),
        Without<terra_player::PlayerBody>,
    >,
    time: Res<Time>,
    mut sources: ResMut<TorchSources>,
    cache: Res<Lamps>,
    lamps: Query<&VoxelLamp>,
    cave: Res<super::CaveState>,
    (soft_core, graphics): (Res<TorchLightingStatus>, Option<Res<GraphicsConfig>>),
) {
    sources.0.clear();
    let mut desired = Vec::new();
    if access.ready() {
        if let Some(session) = session.as_ref().filter(|session| session.ready) {
            sources.0.extend(
                session
                    .furnaces
                    .iter()
                    .filter(|furnace| furnace.lit)
                    .map(|furnace| {
                        Vec3::from_array(furnace.position.map(|v| v as f32))
                            + Vec3::new(0.5, 0.55, 0.5)
                    })
                    .chain(
                        session
                            .campfires
                            .iter()
                            .filter(|fire| fire.lit)
                            .map(|fire| {
                                Vec3::from_array(fire.position.map(|v| v as f32))
                                    + Vec3::new(0.5, 0.4, 0.5)
                            }),
                    )
                    .filter(|position| {
                        bodies.iter().next().is_some_and(|body| {
                            position.distance_squared(body.translation) < 14.0_f32.powi(2)
                        })
                    }),
            );
        }
        sources
            .0
            .extend(lamps.iter().filter(|l| l.1.flicker > 0.0).map(|l| l.2));
        // Foley follows the body even when a different lamp wins the camera's GPU budget.
        if let Some(body) = bodies.iter().next() {
            sources.0.extend(
                cache
                    .emitters
                    .iter()
                    .filter(|(_, light)| light.flicker > 0.0)
                    .filter(|(p, _)| !lamps.iter().any(|lamp| lamp.0 == *p))
                    .map(|(p, _)| Vec3::from_array(p.map(|v| v as f32)) + Vec3::new(0.5, 0.98, 0.5))
                    .filter(|p| p.distance_squared(body.translation) < 14.0_f32.powi(2)),
            );
        }
        if let (Some(session), Some(selected), Some(body)) =
            (session.as_ref(), selected.as_ref(), bodies.iter().next())
            && session.ready
            && !session.dead()
            && session.game_mode != terra_core::GameMode::Spectator
            && selected
                .0
                .and_then(|slot| session.inventory.slot(usize::from(slot)))
                .is_some_and(|s| {
                    matches!(
                        s.item,
                        terra_core::ItemKind::Block(40 | terra_core::lantern::LANTERN)
                    )
                })
        {
            desired.push((None, body.translation + Vec3::new(0.3, 0.45, -0.25)));
        }
        if let Some(remotes) = remotes.as_ref() {
            let eye = cameras
                .iter()
                .next()
                .map_or(Vec3::ZERO, |(t, _)| t.translation);
            let mut remote: Vec<_> = remotes
                .players
                .iter()
                .filter(|(_, t)| t.current.held_torch)
                .map(|(&id, t)| (Some(id), t.position() + Vec3::new(0.3, 0.45, -0.25)))
                .collect();
            remote.sort_by(|a, b| {
                a.1.distance_squared(eye)
                    .total_cmp(&b.1.distance_squared(eye))
            });
            desired.extend(remote.into_iter().take(3));
        }
    }
    // Carried lights are deliberately absent from positional fire Foley.
    sources.0.sort_by(|a, b| {
        a.x.total_cmp(&b.x)
            .then(a.y.total_cmp(&b.y))
            .then(a.z.total_cmp(&b.z))
    });
    sources.0.dedup_by(|a, b| a.abs_diff_eq(*b, 0.02));
    let gain = exposure_gain(cameras.iter().next().map_or(6.0, |(_, e)| e.ev100));
    let Some((_, definition)) = assets.lights().find(|(material, _)| *material == 40) else {
        return;
    };
    let intensity = definition.intensity
        * soft_core.flux(definition)
        * gain
        * cave_gain(definition, cave.blend)
        * (1.0 + definition.flicker * (time.elapsed_secs() * 5.7).sin());
    for (entity, id, mut transform, mut light) in &mut lights {
        if let Some(index) = desired.iter().position(|(key, _)| *key == id.0) {
            let (_, position) = desired.remove(index);
            transform.translation = position;
            light.intensity = intensity;
            light.radius = soft_core.radius(definition);
            light.shadow_maps_enabled =
                source_shadows(definition, id.0.is_none(), graphics.as_deref());
        } else {
            commands.entity(entity).despawn();
        }
    }
    for (id, position) in desired {
        commands.spawn((
            Name::new("Carried torch light"),
            HeldLamp(id),
            bevy::camera::visibility::RenderLayers::default()
                .with(crate::meshing::shadows::LAYER)
                .with(crate::transitions::QUEUE_SHADOW_LAYER),
            PointLight {
                color: Color::srgb(
                    definition.color[0],
                    definition.color[1],
                    definition.color[2],
                ),
                intensity,
                range: definition.range,
                radius: soft_core.radius(definition),
                shadow_maps_enabled: source_shadows(definition, id.is_none(), graphics.as_deref()),
                ..default()
            },
            Transform::from_translation(position),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mounted_lantern_lights_follow_the_authored_glass_for_every_mount() {
        let mut journal = EditJournal::default();
        let p = [-16, 19, -1];
        journal.record(p, terra_core::lantern::LANTERN).unwrap();
        let definition = light_definitions()[&terra_core::lantern::LANTERN];
        for facing in [0, 4, 5, 6, 7, 8] {
            journal
                .set_object_state(
                    p,
                    Some(terra_world::edits::BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
                .unwrap();
            let point = emitter_location(&journal, p, definition, Vec3::ZERO);
            let expected = Vec3::from_array(p.map(|v| v as f32))
                + Vec3::from_array(terra_core::lantern::emission(facing).map(|v| v as f32));
            assert!(point.distance(expected) < 1e-5);
            assert!(!definition.shadows);
        }
    }

    #[derive(Resource, Default)]
    struct SelectedLamps(BTreeMap<[i64; 3], BlockLight>);

    fn apply_selected_lamps(
        mut commands: Commands,
        mut lamps: LampEntities,
        selected: Res<SelectedLamps>,
        chunks: Res<ChunkStore>,
        journal: Res<EditJournal>,
        soft_core: Res<TorchLightingStatus>,
    ) {
        sync_lamps(
            &mut commands,
            &mut lamps,
            selected.0.clone(),
            &chunks,
            &journal,
            &soft_core,
            None,
        );
    }

    #[test]
    fn eight_light_pool_slots_survive_source_changes_and_only_shrink_when_needed() {
        fn slots(world: &mut World) -> BTreeMap<[i64; 3], Entity> {
            world
                .query::<(Entity, &VoxelLamp)>()
                .iter(world)
                .map(|(entity, lamp)| (lamp.0, entity))
                .collect()
        }
        let definition = light_definitions()[&40];
        assert!(definition.shadows);
        let initial: BTreeMap<_, _> = (0..8).map(|x| ([x, 2, 3], definition)).collect();
        let mut journal = EditJournal::default();
        for x in (0..8).chain(32..40).chain(80..84) {
            journal.record([x, 2, 3], 40).unwrap();
        }
        let mut app = App::new();
        app.init_resource::<ChunkStore>()
            .insert_resource(journal)
            .init_resource::<TorchLightingStatus>()
            .insert_resource(SelectedLamps(initial))
            .add_systems(Update, apply_selected_lamps);
        app.update();
        let original = slots(app.world_mut());
        let original_ids: std::collections::HashSet<_> = original.values().copied().collect();
        assert_eq!(original_ids.len(), 8);

        let replacement = BlockLight {
            range: definition.range + 2.0,
            intensity: definition.intensity * 0.8,
            color: [0.9, 0.6, 0.3],
            ..definition
        };
        let selected: BTreeMap<_, _> = (0..8).map(|x| ([x + 32, 2, 3], replacement)).collect();
        app.world_mut().resource_mut::<SelectedLamps>().0 = selected.clone();
        app.update();
        let reassigned = slots(app.world_mut());
        assert_eq!(
            reassigned.keys().collect::<Vec<_>>(),
            selected.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            reassigned
                .values()
                .copied()
                .collect::<std::collections::HashSet<_>>(),
            original_ids,
            "new emitter positions must reuse the same eight shadow-view owners"
        );
        for (&position, &entity) in &reassigned {
            let light = app.world().get::<PointLight>(entity).unwrap();
            assert!(light.shadow_maps_enabled);
            assert_eq!(light.range, replacement.range);
            assert_eq!(light.intensity, replacement.intensity);
            assert_eq!(light.color, Color::srgb_from_array(replacement.color));
            assert_eq!(
                app.world().get::<Transform>(entity).unwrap().translation,
                Vec3::from_array(position.map(|v| v as f32)) + Vec3::new(0.5, 0.98, 0.5)
            );
        }

        let retained: BTreeMap<_, _> = reassigned.iter().take(2).map(|(&p, &e)| (p, e)).collect();
        let smaller: BTreeMap<_, _> = retained
            .keys()
            .copied()
            .chain([[80, 2, 3], [81, 2, 3]])
            .map(|p| (p, replacement))
            .collect();
        app.world_mut().resource_mut::<SelectedLamps>().0 = smaller;
        app.update();
        let shrunk = slots(app.world_mut());
        assert_eq!(shrunk.len(), 4);
        assert!(shrunk.values().all(|entity| original_ids.contains(entity)));
        assert!(
            retained
                .iter()
                .all(|(position, entity)| shrunk.get(position) == Some(entity))
        );

        app.world_mut()
            .resource_mut::<SelectedLamps>()
            .0
            .extend([([82, 2, 3], replacement), ([83, 2, 3], replacement)]);
        app.update();
        let grown = slots(app.world_mut());
        assert_eq!(grown.len(), 6);
        assert!(
            shrunk
                .iter()
                .all(|(position, entity)| grown.get(position) == Some(entity))
        );
        app.world_mut().resource_mut::<SelectedLamps>().0.clear();
        app.update();
        assert!(slots(app.world_mut()).is_empty());
    }

    fn light_definitions() -> BTreeMap<u16, BlockLight> {
        crate::blocks::definitions::BlockLibrary::load(
            &terra_core::asset_directory().join("terra/blocks"),
        )
        .unwrap()
        .blocks
        .into_iter()
        .filter_map(|b| b.light.map(|l| (b.id, l)))
        .collect()
    }

    #[test]
    fn machine_and_scarecrow_lanterns_use_authored_rotated_anchors_and_cached_prop_lifetimes() {
        use terra_world::props::{PropId, PropKind, WorldProp};
        let definitions = light_definitions();
        let crusher = definitions[&99];
        assert_eq!(crusher.range, 4.0);
        assert!(!crusher.shadows && crusher.intensity <= 80.0);
        let support = [4, 2, 4];
        let mut journal = EditJournal::default();
        let mut chunks = ChunkStore::default();
        let mut data = terra_voxel::BlockSection::empty(SectionPos::default()).unwrap();
        data.set_block(support.map(|v| v as usize), 3).unwrap();
        chunks.loaded.insert(
            SectionPos::default(),
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut cache = Lamps::default();
        discover(
            &mut cache,
            &chunks,
            &journal,
            &definitions,
            Vec3::ZERO,
            32.0,
        );
        assert!(cache.emitters.is_empty());
        for facing in 0..4 {
            let prop = WorldProp {
                id: PropId(facing + 1),
                kind: PropKind::Scarecrow,
                position: [4.5, 3.0, 4.5],
                rotation: facing as f32 * std::f32::consts::FRAC_PI_2,
                scale: 1.0,
                variant: 0,
                section: SectionPos::default(),
                removed: false,
            };
            let (p, _, light) = scarecrow_source(&prop).unwrap();
            let expected = Vec3::from_array(prop.position.map(|v| v as f32))
                + Quat::from_rotation_y(prop.rotation)
                    * Vec3::new(0.64, 0.84, 0.0)
                    * prop.visual_scale();
            assert!(emitter_location(&journal, p, light, Vec3::ZERO).abs_diff_eq(expected, 0.0001));
            assert!(!light.shadows && light.flicker == 0.0 && light.range == 4.0);
            assert!(journal.place_prop(prop).unwrap());
            discover(
                &mut cache,
                &chunks,
                &journal,
                &definitions,
                Vec3::ZERO,
                32.0,
            );
            assert_eq!(
                cache.emitters.len(),
                1,
                "placed prop epoch invalidates an empty cached section"
            );
            let retained = cache.sections[&SectionPos::default()].epoch;
            discover(
                &mut cache,
                &chunks,
                &journal,
                &definitions,
                Vec3::ZERO,
                32.0,
            );
            assert_eq!(cache.sections[&SectionPos::default()].epoch, retained);
            journal.remove_prop(prop.section, prop.id).unwrap();
            discover(
                &mut cache,
                &chunks,
                &journal,
                &definitions,
                Vec3::ZERO,
                32.0,
            );
            assert!(cache.emitters.is_empty());
        }
        chunks.loaded.clear();
        discover(
            &mut cache,
            &chunks,
            &journal,
            &definitions,
            Vec3::ZERO,
            32.0,
        );
        assert!(cache.sections.is_empty() && cache.emitters.is_empty());
    }

    #[test]
    fn bounded_discovery_finishes_and_compact_emitters_follow_edits_and_unloads() {
        let definitions = light_definitions();
        let mut chunks = ChunkStore::default();
        for x in -64..=64 {
            let position = SectionPos::new(x, 0, 0);
            let mut data = terra_voxel::BlockSection::empty(position).unwrap();
            // Every section has a real source: source-free stone and sky now
            // take the constant-time path and do not consume this budget.
            data.set_block([1, 0, 1], 3).unwrap();
            data.set_block([2, 3, 4], 40).unwrap();
            chunks.loaded.insert(
                position,
                terra_world::streaming::LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        let mut cache = Lamps::default();
        let mut journal = EditJournal::default();
        discover(
            &mut cache,
            &chunks,
            &journal,
            &definitions,
            Vec3::ZERO,
            2000.0,
        );
        assert_eq!(cache.sections.len(), 128);
        assert!(
            cache.dirty,
            "Remaining discovery work must survive the timer"
        );
        assert_eq!(cache.emitters.len(), 128);
        discover(
            &mut cache,
            &chunks,
            &journal,
            &definitions,
            Vec3::ZERO,
            2000.0,
        );
        assert_eq!(cache.sections.len(), 129);
        assert!(!cache.dirty);
        assert_eq!(cache.emitters.len(), 129);
        assert!(cache.emitters.iter().any(|(p, _)| *p == [2, 3, 4]));
        journal.record([2, 3, 4], 0).unwrap();
        discover(
            &mut cache,
            &chunks,
            &journal,
            &definitions,
            Vec3::ZERO,
            2000.0,
        );
        assert_eq!(cache.emitters.len(), 128);
        assert!(cache.emitters.iter().all(|(p, _)| *p != [2, 3, 4]));
        journal.record([2, 3, 4], 40).unwrap();
        discover(
            &mut cache,
            &chunks,
            &journal,
            &definitions,
            Vec3::ZERO,
            2000.0,
        );
        assert_eq!(cache.emitters.len(), 129);
        assert!(cache.emitters.iter().any(|(p, _)| *p == [2, 3, 4]));
        chunks.loaded.remove(&SectionPos::default());
        discover(
            &mut cache,
            &chunks,
            &journal,
            &definitions,
            Vec3::ZERO,
            2000.0,
        );
        assert_eq!(cache.sections.len(), 128);
        assert_eq!(cache.emitters.len(), 128);
        assert!(cache.emitters.iter().all(|(p, _)| *p != [2, 3, 4]));
    }

    #[test]
    fn replacing_a_lamp_at_the_same_position_refreshes_every_light_property() {
        let definitions = light_definitions();
        let p = [-3, 8, 4];
        let mut lamp = VoxelLamp(p, definitions[&40], Vec3::ZERO);
        let mut selected = BTreeMap::from([(p, definitions[&28])]);
        assert!(retain_definition(&mut lamp, &mut selected));
        assert!(selected.is_empty());
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<TorchLightingStatus>()
            .init_resource::<super::super::CaveState>()
            .add_systems(Update, flicker);
        let entity = app
            .world_mut()
            .spawn((lamp, PointLight::default(), Transform::default()))
            .id();
        app.update();
        let light = app.world().get::<PointLight>(entity).unwrap();
        assert_eq!(light.range, 4.0);
        assert_eq!(light.color, Color::srgb_from_array(definitions[&28].color));
        assert!(!light.shadow_maps_enabled);
        assert_eq!(light.intensity, definitions[&28].intensity);
    }

    #[test]
    fn static_lamps_keep_change_detection_clean_but_respond_to_exposure_changes() {
        #[derive(Resource, Default)]
        struct Changes {
            lights: usize,
            transforms: usize,
        }
        fn observe(
            lights: Query<(), (With<VoxelLamp>, Changed<PointLight>)>,
            transforms: Query<(), (With<VoxelLamp>, Changed<Transform>)>,
            mut changes: ResMut<Changes>,
        ) {
            changes.lights = lights.iter().count();
            changes.transforms = transforms.iter().count();
        }
        let mut definition = light_definitions()[&40];
        definition.flicker = 0.0;
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<TorchLightingStatus>()
            .init_resource::<super::super::CaveState>()
            .init_resource::<Changes>()
            .add_systems(Update, (flicker, observe.after(flicker)));
        let camera = app
            .world_mut()
            .spawn((
                terra_player::TerraCamera,
                bevy::camera::Exposure { ev100: 6.0 },
            ))
            .id();
        let position = Vec3::new(2.0, 3.0, 4.0);
        let lamp = app
            .world_mut()
            .spawn((
                VoxelLamp([2, 3, 4], definition, position),
                PointLight::default(),
                Transform::default(),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<PointLight>(lamp).unwrap().intensity,
            definition.intensity
        );
        assert_eq!(
            app.world().get::<Transform>(lamp).unwrap().translation,
            position
        );
        assert_eq!(app.world().resource::<Changes>().lights, 1);
        assert_eq!(app.world().resource::<Changes>().transforms, 1);
        for _ in 0..2 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(100));
            app.update();
            assert_eq!(app.world().resource::<Changes>().lights, 0);
            assert_eq!(app.world().resource::<Changes>().transforms, 0);
        }
        app.world_mut()
            .get_mut::<bevy::camera::Exposure>(camera)
            .unwrap()
            .ev100 = 7.0;
        app.update();
        assert_eq!(
            app.world().get::<PointLight>(lamp).unwrap().intensity,
            definition.intensity * 2.0
        );
        assert_eq!(app.world().resource::<Changes>().lights, 1);
        assert_eq!(app.world().resource::<Changes>().transforms, 0);
    }

    #[test]
    fn shadow_quality_switches_existing_lamps_without_removing_or_dimming_them() {
        let mut definition = light_definitions()[&40];
        definition.flicker = 0.0;
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<GraphicsConfig>()
            .init_resource::<TorchLightingStatus>()
            .init_resource::<super::super::CaveState>()
            .add_systems(Update, flicker);
        let lamp = app
            .world_mut()
            .spawn((
                VoxelLamp([2, 3, 4], definition, Vec3::ZERO),
                PointLight::default(),
                Transform::default(),
            ))
            .id();
        for quality in [
            terra_core::ShadowQuality::High,
            terra_core::ShadowQuality::Off,
            terra_core::ShadowQuality::Low,
            terra_core::ShadowQuality::Off,
            terra_core::ShadowQuality::High,
        ] {
            let graphics = GraphicsConfig {
                shadow_quality: quality,
                ..default()
            };
            let enabled = quality != terra_core::ShadowQuality::Off;
            assert_eq!(source_shadows(definition, true, Some(&graphics)), enabled);
            assert!(!source_shadows(definition, false, Some(&graphics)));
            let mut unshadowed = definition;
            unshadowed.shadows = false;
            assert!(!source_shadows(unshadowed, true, Some(&graphics)));
            app.insert_resource(graphics);
            // More than one frame detects an unconditional flicker reset.
            for _ in 0..3 {
                app.update();
                let light = app.world().get::<PointLight>(lamp).unwrap();
                assert_eq!(light.shadow_maps_enabled, enabled);
                assert_eq!(light.intensity, definition.intensity);
                assert_eq!(light.range, definition.range);
                assert_eq!(light.color, Color::srgb_from_array(definition.color));
            }
            assert_eq!(
                app.world_mut()
                    .query::<&VoxelLamp>()
                    .iter(app.world())
                    .count(),
                1
            );
        }
    }

    #[test]
    fn quality_budgets_keep_the_nearest_loaded_emitters_and_ignore_duplicate_sources() {
        let definition = light_definitions()[&40];
        let position = SectionPos::default();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            position,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut emitters: Vec<_> = (0..32)
            .rev()
            .map(|i| ([i % 8, 1, i / 8], definition))
            .collect();
        emitters.extend_from_within(..);
        emitters.push(([-1, 0, 0], definition));
        let journal = EditJournal::default();
        for budget in [4, 8, 12, 16] {
            let selected = select_emitters(
                &emitters,
                &chunks,
                &journal,
                Vec3::ZERO,
                Vec3::NEG_Z,
                budget,
            );
            assert_eq!(selected.len(), budget);
            assert!(!selected.contains_key(&[-1, 0, 0]));
            let mut expected: Vec<_> = emitters
                .iter()
                .map(|(p, _)| *p)
                .filter(|p| p[0] >= 0)
                .collect();
            expected.sort_by_key(|p| (p.iter().map(|v| v * v).sum::<i64>(), *p));
            expected.dedup();
            expected.truncate(budget);
            assert_eq!(
                selected
                    .keys()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>(),
                expected.into_iter().collect()
            );
        }
    }

    #[test]
    fn cemetery_sources_are_discovered_but_never_exceed_the_shared_light_budget() {
        let definitions = light_definitions();
        let ids = [358, 369, 370, 371, 372, 373, 386, 395, 396, 397];
        let position = SectionPos::default();
        let mut data = terra_voxel::BlockSection::empty(position).unwrap();
        for (x, id) in ids.into_iter().enumerate() {
            data.set_block([x + 1, 1, 1], id).unwrap();
        }
        data.set_block([12, 1, 1], 399).unwrap();
        let journal = EditJournal::default();
        let discovered = discover_sources(&data, &journal, &definitions).unwrap();
        assert_eq!(discovered.len(), ids.len());
        let emitters: Vec<_> = discovered
            .into_iter()
            .map(|(p, _, light)| (p, light))
            .collect();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            position,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut app = App::new();
        app.insert_resource(chunks)
            .insert_resource(journal)
            .init_resource::<TorchLightingStatus>()
            .init_resource::<SelectedLamps>()
            .add_systems(Update, apply_selected_lamps);
        // Keep even the short-range candle eligible. From the section origin
        // it is correctly culled behind the other lamps, beyond range + 2 m.
        let focus = Vec3::new(6.0, 2.0, 4.0);
        for budget in [4, 8, 12, 16] {
            let selected = select_emitters(
                &emitters,
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
                focus,
                Vec3::NEG_Z,
                budget,
            );
            assert_eq!(selected.len(), budget.min(ids.len()));
            app.world_mut().resource_mut::<SelectedLamps>().0 = selected;
            app.update();
            let mut query = app
                .world_mut()
                .query::<(&PointLight, Has<bevy::light::VolumetricLight>)>();
            let lights: Vec<_> = query.iter(app.world()).collect();
            assert_eq!(lights.len(), budget.min(ids.len()));
            assert!(
                lights
                    .iter()
                    .all(|(light, volume)| !light.shadow_maps_enabled && !volume)
            );
        }
    }

    #[test]
    fn visible_street_lights_are_not_starved_by_closer_sealed_household_lamps() {
        let definition = light_definitions()[&40];
        let position = SectionPos::default();
        let mut data = terra_voxel::BlockSection::empty(position).unwrap();
        for x in 0..16 {
            for y in 0..6 {
                data.set_block([x, y, 4], 3).unwrap();
            }
        }
        let street = [14, 1, 1];
        let mut emitters: Vec<_> = (2..10).map(|x| ([x, 1, 7], definition)).collect();
        emitters.push((street, definition));
        for (p, _) in &emitters {
            data.set_block(p.map(|v| v as usize), 40).unwrap();
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            position,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        for budget in [4, 8] {
            let selected = select_emitters(
                &emitters,
                &chunks,
                &EditJournal::default(),
                Vec3::new(0.5, 1.9, 0.5),
                Vec3::NEG_Z,
                budget,
            );
            assert_eq!(selected.len(), budget);
            assert!(selected.contains_key(&street));
            assert!(
                selected.contains_key(&[2, 1, 7]),
                "nearby corner light remains eligible"
            );
        }
    }

    #[test]
    fn discovery_borrows_pristine_sections_but_detects_imported_and_pending_lamps() {
        let definitions = light_definitions();
        let mut data = terra_voxel::BlockSection::empty(SectionPos::new(-1, 0, 0)).unwrap();
        let mut journal = EditJournal::default();
        assert!(matches!(
            effective_cells(&data, &journal).unwrap(),
            Cow::Borrowed(_)
        ));
        assert!(
            discover_sources(&data, &journal, &definitions)
                .unwrap()
                .is_empty()
        );
        let p = [-14, 3, 4];
        // Revision-zero journal does not imply that imported voxel data has no lamps.
        data.set_block([2, 3, 4], 40).unwrap();
        let imported = discover_sources(&data, &journal, &definitions).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].0, p);
        journal.record(p, 0).unwrap();
        assert!(
            discover_sources(&data, &journal, &definitions)
                .unwrap()
                .is_empty()
        );
        journal.record([-13, 3, 4], 40).unwrap();
        let placed = discover_sources(&data, &journal, &definitions).unwrap();
        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0].0, [-13, 3, 4]);
    }

    #[test]
    fn emitter_prefilter_preserves_sparse_imports_and_pending_edits() {
        let definitions = light_definitions();
        let section = SectionPos::new(-3, -1, 2);
        let mut blocks = vec![3; 4096];
        let mut expected = Vec::new();
        for (index, &material) in definitions.keys().enumerate() {
            let cell = [index % 16, index / 16 % 16, index / 256];
            blocks[cell[0] + cell[1] * 16 + cell[2] * 256] = material;
            expected.push((section.global_block(cell).unwrap(), material));
        }
        let data = terra_voxel::BlockSection::from_blocks(section, 0, blocks).unwrap();
        let mut journal = EditJournal::default();
        let found = discover_sources(&data, &journal, &definitions).unwrap();
        assert_eq!(found.len(), expected.len());
        for (position, material) in &expected {
            assert!(
                found
                    .iter()
                    .any(|&(p, m, _)| p == *position && m == *material)
            );
        }
        for (position, _) in expected {
            journal.record(position, 0).unwrap();
        }
        assert!(
            discover_sources(&data, &journal, &definitions)
                .unwrap()
                .is_empty()
        );
        let position = section.global_block([15, 0, 0]).unwrap();
        journal.record(position, 40).unwrap();
        let found = discover_sources(&data, &journal, &definitions).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].0, found[0].1), (position, 40));
    }

    #[test]
    fn neighbor_loading_rechecks_cached_emitters_without_losing_buried_sources() {
        let definitions = light_definitions();
        let owner = SectionPos::new(-1, 0, 0);
        let neighbor = SectionPos::new(0, 0, 0);
        let mut blocks = vec![3; 4096];
        blocks[15 + 16 + 256] = 28;
        let data = terra_voxel::BlockSection::from_blocks(owner, 0, blocks).unwrap();
        let journal = EditJournal::default();
        let sources = discover_sources(&data, &journal, &definitions).unwrap();
        assert_eq!(sources.len(), 1);
        let loaded = |data| terra_world::streaming::LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        };
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(owner, loaded(data));
        assert!(exposed_sources(&sources, &chunks, &journal).is_empty());
        assert_eq!(neighbor_mask(owner, &chunks), 0);
        chunks.loaded.insert(
            neighbor,
            loaded(terra_voxel::BlockSection::empty(neighbor).unwrap()),
        );
        assert_eq!(exposed_sources(&sources, &chunks, &journal).len(), 1);
        assert_eq!(neighbor_mask(owner, &chunks), 1);
        chunks.loaded.remove(&neighbor);
        assert!(exposed_sources(&sources, &chunks, &journal).is_empty());
        assert_eq!(
            sources.len(),
            1,
            "occlusion must not erase source discovery"
        );
    }

    #[test]
    fn cave_torches_are_forty_percent_dimmer_without_changing_surface_gain() {
        let library = crate::blocks::definitions::BlockLibrary::load(
            &terra_core::asset_directory().join("terra/blocks"),
        )
        .unwrap();
        let light = library
            .blocks
            .iter()
            .find(|b| b.id == 40)
            .unwrap()
            .light
            .unwrap();
        assert_eq!(cave_gain(light, 0.0), 1.0);
        assert!((cave_gain(light, 1.0) - 0.6).abs() < 1e-6);
        assert!((cave_gain(light, 0.5) - 0.8).abs() < 1e-6);
    }
    #[test]
    fn bench_lantern_follows_double_width_and_all_four_rotations() {
        let library = crate::blocks::definitions::BlockLibrary::load(
            &terra_core::asset_directory().join("terra/blocks"),
        )
        .unwrap();
        let light = library
            .blocks
            .iter()
            .find(|b| b.id == 28)
            .unwrap()
            .light
            .unwrap();
        let p = [-16, 20, -1];
        for facing in 0..4 {
            let mut journal = EditJournal::default();
            journal.record(p, 28).unwrap();
            let mut state = terra_world::edits::BlockObjectState {
                facing,
                ..default()
            };
            let right = state.right();
            state.peer = Some(std::array::from_fn(|i| p[i] + right[i]));
            journal.set_object_state(p, Some(state)).unwrap();
            let source = emitter_location(&journal, p, light, Vec3::ZERO);
            let local = Quat::from_rotation_y(-f32::from(facing) * std::f32::consts::FRAC_PI_2)
                * (source - Vec3::from_array(p.map(|v| v as f32)) - Vec3::new(0.5, 0.0, 0.5));
            assert!(local.abs_diff_eq(Vec3::new(-0.18, 0.87, -0.32), 0.0001));
        }
    }
    #[test]
    fn outdoor_exposure_does_not_make_an_identical_torch_disappear() {
        assert_eq!(exposure_gain(6.0), 1.0);
        assert!((exposure_gain(10.7) / 2.0_f32.powf(10.7) - 1.0 / 64.0).abs() < 0.0001);
        assert!(exposure_gain(13.7) < 256.0);
    }

    #[test]
    fn running_day_clock_cannot_apply_outdoor_torch_gain_to_cave_exposure() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<terra_core::WorldClock>()
            .init_resource::<terra_core::GameConfig>()
            .init_resource::<GraphicsConfig>()
            .init_resource::<super::super::CaveState>()
            .init_resource::<TorchLightingStatus>();
        super::super::super::daylight::install(&mut app);
        configure_light_updates(&mut app);
        // Register the reader first: production ordering must resolve this,
        // rather than relying on incidental system registration/execution order.
        app.add_systems(PostUpdate, flicker.in_set(ApplyLocalLights))
            .add_systems(PostUpdate, super::super::exposure);
        app.world_mut().resource_mut::<TorchLightingStatus>().ready = true;
        let camera = app
            .world_mut()
            .spawn((
                terra_player::TerraCamera,
                bevy::camera::Exposure::default(),
                AmbientLight::default(),
                bevy::light::AtmosphereEnvironmentMapLight::default(),
            ))
            .id();
        let mut definition = light_definitions()[&40];
        definition.flicker = 0.0;
        let lamp = app
            .world_mut()
            .spawn((
                VoxelLamp([0, 0, 0], definition, Vec3::ZERO),
                PointLight::default(),
                Transform::default(),
            ))
            .id();
        for brightness in [0.5, 1.0, 2.0] {
            app.world_mut().resource_mut::<GraphicsConfig>().brightness = brightness;
            for blend in [0.0, 0.5, 1.0] {
                app.world_mut()
                    .resource_mut::<super::super::CaveState>()
                    .blend = blend;
                for phase in [0.0, 0.35, 0.5, 0.8] {
                    app.world_mut()
                        .resource_mut::<terra_core::WorldClock>()
                        .phase = phase;
                    app.update();
                    let ev = app
                        .world()
                        .get::<bevy::camera::Exposure>(camera)
                        .unwrap()
                        .ev100;
                    let expected = definition.intensity
                        * app
                            .world()
                            .resource::<TorchLightingStatus>()
                            .flux(definition)
                        * exposure_gain(ev)
                        * cave_gain(definition, blend);
                    let actual = app.world().get::<PointLight>(lamp).unwrap().intensity;
                    assert!((actual - expected).abs() < expected * 0.0001);
                    if blend == 1.0 && brightness == 1.0 {
                        assert!((ev - super::super::CAVE_EXPOSURE).abs() < 0.0001);
                        // Darkening the unlit cave must not alter the perceived
                        // local lamp energy or reintroduce outdoor EV gain.
                        assert!((actual * 2.0_f32.powf(-ev) - 8700.0 / 64.0).abs() < 0.0001);
                    }
                    // Pausing the clock cannot silently repair a different
                    // brightness than the same cave rendered while time moves.
                    app.update();
                    assert_eq!(
                        app.world().get::<PointLight>(lamp).unwrap().intensity,
                        actual
                    );
                }
            }
        }
    }
    #[test]
    fn torch_edits_reach_the_light_cache_before_mesh_completion() {
        let mut data = terra_voxel::blocks::BlockSection::empty(SectionPos::default()).unwrap();
        let mut journal = EditJournal::default();
        let p = [2, 3, 4];
        let index = 2 + 3 * 16 + 4 * 256;
        journal.record(p, 40).unwrap();
        assert_eq!(effective_cells(&data, &journal).unwrap()[index], 40);
        data.set_block([2, 3, 4], 40).unwrap();
        journal.record(p, 0).unwrap();
        assert_eq!(effective_cells(&data, &journal).unwrap()[index], 0);
    }
    #[test]
    fn sealed_and_unloaded_rooms_cull_lights_but_glass_and_air_do_not() {
        let a = Vec3::new(0.5, 0.5, 0.5);
        let b = Vec3::new(12.5, 0.5, 0.5);
        assert!(visible(a, b, |_| Some(0)));
        assert!(visible(a, b, |_| Some(15)));
        assert!(!visible(a, b, |p| Some(if p[0] == 6 { 3 } else { 0 })));
        assert!(!visible(a, b, |p| (p[0] != 6).then_some(0)));
        let far = Vec3::new(225.5, 0.5, 0.5);
        assert!(visible(a, far, |_| Some(0)));
        assert!(!visible(a, far, |p| Some(if p[0] == 150 { 3 } else { 0 })));
    }
    #[test]
    fn emitters_clear_their_opaque_flame_and_crystal_geometry() {
        let p = location([-1, 0, 0], false, |p| Some(if p[0] == 0 { 0 } else { 3 }));
        assert_eq!(p.x, 0.5);
        assert_eq!(
            location([-1, 0, 0], true, |_| Some(0)),
            Vec3::new(-0.5, 0.98, 0.5)
        );
    }
}
