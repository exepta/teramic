//! A bounded near-field view of the real saved fluid volumes and collision shapes.
//! Moving particles keep their slots when liquid moves between voxel owners.
use super::api::*;
use bevy::{
    ecs::{message::MessageCursor, system::SystemParam},
    prelude::*,
};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use terra_core::{block_properties::BlockPropertiesCatalog, block_shapes::ShapeBox};
use terra_voxel::{MaterialId, SectionPos, global_to_section};
use terra_world::{
    edits::EditJournal,
    streaming::{
        ChunkFocus, ChunkStore, SectionEditApplied, SectionPublished, SectionRemoved,
        WaterPublished, WorldTerrain,
    },
};

const SIDE: i64 = DOMAIN_SIZE as i64;
const REBUILD_SECONDS: f64 = 0.15;
const EPS: f32 = 0.00001;

#[derive(Clone, Copy, Debug)]
struct Region {
    minimum: [f32; 3],
    maximum: [f32; 3],
    kind: u32,
    source: bool,
    velocity: [f32; 3],
    flow_origin: [f32; 3],
    flow_radius: f32,
}
impl Region {
    fn volume(self) -> f32 {
        (0..3)
            .map(|i| (self.maximum[i] - self.minimum[i]).max(0.))
            .product()
    }
}

#[derive(Default)]
struct Adapter {
    signature: Option<u64>,
    next_update: f64,
    generation: u32,
    resolution: Resolution,
    anchors: Vec<[f32; 3]>,
}

fn smoke_option(name: &str) -> bool {
    std::env::args().any(|a| a.starts_with("--smoke-test"))
        && std::env::var(name).as_deref() == Ok("1")
}

#[derive(SystemParam)]
struct Source<'w> {
    focus: Res<'w, ChunkFocus>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    terrain: Res<'w, WorldTerrain>,
    properties: Res<'w, BlockPropertiesCatalog>,
    time: Res<'w, Time>,
}
fn update(
    mut input: ResMut<ParticleFluidInput>,
    mut rest: ResMut<RestFluidGrid>,
    source: Source,
    mut state: Local<Adapter>,
) {
    if smoke_option("TERRA_PARTICLE_MANUAL_INPUT") {
        return;
    }
    if smoke_option("TERRA_PARTICLE_FLUIDS_DISABLE") {
        if input.enabled {
            input.enabled = false;
            input.revision += 1;
        }
        return;
    }
    let now = source.time.elapsed_secs_f64();
    if now < state.next_update {
        return;
    }
    state.next_update = now + REBUILD_SECONDS;
    let focus = source.focus.0;
    if focus.iter().any(|v| !v.is_finite() || v.abs() > 990_000.) {
        return;
    }
    let origin = focus.map(|v| ((v - 8.) / 4.).floor() as i64 * 4);
    let signature = signature(&source, origin);
    if state.signature == Some(signature) {
        return;
    }
    state.signature = Some(signature);
    // Undisturbed reservoirs have no particle domain. Moving the camera must
    // never reseed an ocean or turn it into a shrinking box of loose particles.
    if !has_local_dynamic_liquid(&source, origin) {
        if input.enabled || input.particles.iter().any(|p| p.kind < 4) {
            input.enabled = false;
            input.particles.clear();
            input.target_mass = [0.; 4];
            input.solids.clear();
            input.paths.clear();
            input.revision = input.revision.wrapping_add(1);
        }
        return;
    }
    let (regions, mut solids, paths) = capture(&source, origin);
    merge_colliders(&mut solids);
    let paths = merge_paths(paths);
    let mut mass = [0.; 4];
    for region in &regions {
        mass[region.kind as usize] += region.volume();
    }
    // Never silently discard collision geometry to make particles leak through
    // a dense construction. The existing world renderer is the bounded fallback.
    let supported = solids.len() <= MAX_COLLIDERS && paths.len() <= MAX_PATHS;
    if !supported {
        mass = [0.; 4];
    }
    let (wanted, coarsened) = state.resolution.counts(mass);
    // A changed support radius represents a different sampling resolution.
    // Rebuild that kind uniformly instead of carrying its old density into a
    // small entering slice of the camera domain.
    for slot in &mut input.particles {
        if slot.kind < 4 && coarsened[slot.kind as usize] {
            slot.kind = INACTIVE;
        }
    }
    let previous_origin = input.origin;
    let next_origin = origin.map(|v| v as f32);
    let Adapter {
        generation,
        anchors,
        ..
    } = &mut *state;
    let covered = reconcile_with_anchors(
        &mut input.particles,
        &regions,
        wanted,
        previous_origin,
        next_origin,
        generation,
        anchors,
    );
    let (halo, _, _) = capture_range(&source, origin, -1, SIDE + 1);
    let snapshot = sampled_section(&halo, input.revision.wrapping_add(1));
    rest.origin = next_origin;
    rest.revision = input.revision.wrapping_add(1);
    rest.fields = snapshot.fields;
    rest.films = snapshot.films;
    input.origin = next_origin;
    input.solids = if supported { solids } else { Vec::new() };
    input.paths = if supported { paths } else { Vec::new() };
    input.target_mass = mass;
    input.enabled = supported && covered && wanted.iter().any(|&n| n > 0);
    input.revision = input.revision.wrapping_add(1);
}

fn signature(source: &Source, origin: [i64; 3]) -> u64 {
    let mut h = DefaultHasher::new();
    origin.hash(&mut h);
    source.terrain.0.config().water.hash(&mut h);
    let a = global_to_section(origin).0;
    let b = global_to_section(origin.map(|v| v + SIDE - 1)).0;
    for x in a.x..=b.x {
        for y in a.y..=b.y {
            for z in a.z..=b.z {
                let p = SectionPos::new(x, y, z);
                p.hash(&mut h);
                source
                    .chunks
                    .loaded
                    .get(&p)
                    .map(|s| s.data.revision())
                    .hash(&mut h);
                source.journal.mesh_epoch(p).hash(&mut h);
                source.journal.fluid_epoch(p).hash(&mut h);
                source.journal.water_mesh_epoch(p).hash(&mut h);
            }
        }
    }
    h.finish()
}

/// Publication-driven work: camera movement only reorders outstanding uploads.
#[derive(Default)]
struct FarCapture {
    epochs: std::collections::HashMap<SectionPos, u64>,
    pending: std::collections::HashSet<SectionPos>,
    seeded: bool,
    publications: MessageCursor<SectionPublished>,
    water: MessageCursor<WaterPublished>,
    removals: MessageCursor<SectionRemoved>,
    edits: MessageCursor<SectionEditApplied>,
}
#[derive(SystemParam)]
struct Publications<'w> {
    sections: Option<Res<'w, Messages<SectionPublished>>>,
    water: Option<Res<'w, Messages<WaterPublished>>>,
    removed: Option<Res<'w, Messages<SectionRemoved>>>,
    edits: Option<Res<'w, Messages<SectionEditApplied>>>,
}
fn update_far(
    mut output: ResMut<FarFluidSections>,
    source: Source,
    messages: Publications,
    mut state: Local<FarCapture>,
) {
    if smoke_option("TERRA_PARTICLE_MANUAL_INPUT") {
        return;
    }
    if source.chunks.loaded.is_empty() {
        state.epochs.clear();
        state.pending.clear();
        state.seeded = false;
        if !output.sections.is_empty() {
            output.sections.clear();
            output.revision = output.revision.wrapping_add(1);
        }
        return;
    }
    if source.terrain.is_changed() {
        state.seeded = false;
        state.epochs.clear();
    }
    let notified =
        messages.sections.is_some() && messages.water.is_some() && messages.removed.is_some();
    let mut invalidate = Vec::new();
    if let Some(messages) = &messages.sections {
        invalidate.extend(state.publications.read(messages).map(|m| m.position));
    }
    if let Some(messages) = &messages.water {
        invalidate.extend(state.water.read(messages).map(|m| m.position));
    }
    if let Some(messages) = &messages.edits {
        invalidate.extend(state.edits.read(messages).map(|m| m.position));
    }
    let removed: Vec<_> = if let Some(messages) = &messages.removed {
        state.removals.read(messages).map(|m| m.position).collect()
    } else if source.chunks.is_changed() {
        state
            .epochs
            .keys()
            .filter(|p| !source.chunks.loaded.contains_key(*p))
            .copied()
            .collect()
    } else {
        Vec::new()
    };
    for p in removed {
        state.epochs.remove(&p);
        state.pending.remove(&p);
        if output.sections.remove(&p).is_some() {
            output.revision = output.revision.wrapping_add(1);
        }
    }
    if !state.seeded || (!notified && (source.chunks.is_changed() || source.journal.is_changed())) {
        let owners: Vec<_> = source.chunks.loaded.keys().copied().collect();
        state.pending.extend(owners);
        state.seeded = !source.chunks.loaded.is_empty();
    }
    if source.journal.is_changed() {
        state
            .pending
            .extend(source.journal.water_mesh_epochs().map(|(p, _)| p));
        state.pending.extend(source.journal.object_sections());
    }
    // New neighbours can expose a border even without changing its own epoch.
    for p in invalidate {
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let n = SectionPos::new(p.x + x, p.y + y, p.z + z);
                    if !source.chunks.loaded.contains_key(&n) {
                        continue;
                    }
                    // Prepared body topology already includes its padded
                    // neighbours. Loading a nearby dry chunk must not replace
                    // a cached sea surface; SDF fallbacks still need the halo.
                    if n == p
                        || !output
                            .sections
                            .get(&n)
                            .is_some_and(|s| s.published_water.is_some())
                    {
                        state.epochs.remove(&n);
                        state.pending.insert(n);
                    }
                }
            }
        }
    }
    let mut pending: Vec<_> = state.pending.iter().copied().collect();
    pending.sort_by(|a, b| {
        crate::meshing::lod::distance_squared(*a, source.focus.0)
            .total_cmp(&crate::meshing::lod::distance_squared(*b, source.focus.0))
    });
    let mut accepted = 0;
    let mut captures = 0;
    for (examined, p) in pending.into_iter().enumerate() {
        // A first publication of a large view must not monopolize one frame.
        if examined >= 256 {
            break;
        }
        let Some(section) = source.chunks.loaded.get(&p) else {
            state.pending.remove(&p);
            continue;
        };
        let mut h = DefaultHasher::new();
        section.data.revision().hash(&mut h);
        section.water_epoch.hash(&mut h);
        source.journal.mesh_epoch(p).hash(&mut h);
        source.journal.fluid_epoch(p).hash(&mut h);
        source.journal.water_mesh_epoch(p).hash(&mut h);
        let epoch = h.finish();
        if state.epochs.get(&p) == Some(&epoch) {
            state.pending.remove(&p);
            continue;
        }
        if accepted >= 32 {
            break;
        }
        let Ok(origin) = p.origin() else {
            state.pending.remove(&p);
            continue;
        };
        let body = water_body_owner(&source, p);
        let has_viscous = section.data.may_contain(terra_core::bile::FLUID)
            || section.data.may_contain(terra_core::roads::WET_TAR);
        let prepared = body.then(|| section.water.clone()).flatten();
        // Wait for the worker's current shoreline, rather than drawing a stale
        // patch over a confirmed terrain or source edit.
        if body && section.water_epoch != source.journal.water_mesh_epoch(p) {
            continue;
        }
        let mut snapshot = if !has_viscous && body {
            FarFluidSection {
                epoch,
                fields: std::array::from_fn(|_| Vec::new()),
                films: Vec::new(),
                published_water: prepared,
            }
        } else if has_viscous
            || section.water.is_some()
            || source.journal.fluid_epoch(p) > 0
            || source.journal.water_mesh_epoch(p) > 0
            || output.sections.contains_key(&p)
        {
            if captures >= 2 {
                continue;
            }
            captures += 1;
            let (regions, _, _) = capture_range(&source, origin, -1, SIDE + 1);
            output.sampled_cells = output.sampled_cells.wrapping_add((SIDE + 2).pow(3) as u64);
            let mut s = sampled_section(&regions, epoch);
            s.published_water = prepared;
            s
        } else {
            state.pending.remove(&p);
            state.epochs.insert(p, epoch);
            continue;
        };
        if body {
            snapshot.fields[WATER as usize].clear();
            snapshot.fields[BLOOD as usize].clear();
        }
        let wet = snapshot
            .published_water
            .as_ref()
            .is_some_and(|p| !p.indices.is_empty())
            || !snapshot.films.is_empty()
            || snapshot.fields.iter().any(|f| f.iter().any(|&v| v > 0.));
        if wet {
            if snapshot.published_water.is_some() {
                output.body_patches = output.body_patches.wrapping_add(1);
            }
            output.sections.insert(p, snapshot);
        } else {
            output.sections.remove(&p);
        }
        state.pending.remove(&p);
        state.epochs.insert(p, epoch);
        output.revision = output.revision.wrapping_add(1);
        accepted += 1;
    }
    if output.revision == 0 {
        output.revision = 1;
    }
}

/// True for a prepared natural interval, including a surface exactly on a
/// vertical section boundary. Large player-built pools also form stable bodies.
fn water_body_owner(source: &Source, p: SectionPos) -> bool {
    if !source.terrain.0.config().water {
        return false;
    }
    let Ok(origin) = p.origin() else {
        return false;
    };
    if (0..16).any(|z| {
        (0..16).any(|x| {
            source
                .chunks
                .surface_column_at(origin[0] + x, origin[2] + z)
                .is_some_and(|c| {
                    c.water_columns().intervals.into_iter().flatten().any(|w| {
                        w.bed < w.level
                            && w.bed < (origin[1] + 16) as f64
                            && w.level >= origin[1] as f64
                    })
                })
        })
    }) {
        return true;
    }
    let volume: u32 = source
        .journal
        .fluid_cells_in(p)
        .map(|(_, c)| u32::from(c.amount))
        .sum();
    if volume >= 64
        && (source.chunks.prepared_column([p.x, p.z]).is_some()
            || source
                .chunks
                .loaded
                .get(&p)
                .is_some_and(|s| s.water.is_some()))
    {
        return true;
    }
    // The published top belongs to the upper owner when its filled cells end
    // exactly at y=16k. Its bulk may all be stored in the lower section.
    source
        .chunks
        .loaded
        .get(&p)
        .and_then(|s| s.water.as_ref())
        .is_some_and(|patch| patch.positions.iter().any(|v| v[1].abs() < 0.001))
        && source
            .journal
            .fluid_cells_in(SectionPos::new(p.x, p.y - 1, p.z))
            .map(|(_, c)| u32::from(c.amount))
            .sum::<u32>()
            >= 64
}
fn has_local_dynamic_liquid(source: &Source, origin: [i64; 3]) -> bool {
    let a = global_to_section(origin).0;
    let b = global_to_section(origin.map(|v| v + SIDE - 1)).0;
    for x in a.x..=b.x {
        for y in a.y..=b.y {
            for z in a.z..=b.z {
                let owner = SectionPos::new(x, y, z);
                let Some(section) = source.chunks.loaded.get(&owner) else {
                    continue;
                };
                if section.data.may_contain(terra_core::bile::FLUID)
                    || section.data.may_contain(terra_core::roads::WET_TAR)
                {
                    return true;
                }
                if !water_body_owner(source, owner)
                    && source.journal.fluid_cells_in(owner).any(|(p, c)| {
                        c.amount > 0 && (0..3).all(|i| p[i] >= origin[i] && p[i] < origin[i] + SIDE)
                    })
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Shared world-space scalar samples are independent of section ownership.
/// The halo makes adjacent sections evaluate exactly the same border nodes.
fn sampled_section(regions: &[Region], epoch: u64) -> FarFluidSection {
    let mut fields: [Vec<f32>; 4] = std::array::from_fn(|_| vec![-1.; REST_NODES]);
    let mut films = Vec::new();
    for r in regions {
        if r.kind == TAR {
            let minimum = r.minimum.map(|v| v.max(0.));
            let maximum = r.maximum.map(|v| v.min(16.));
            if (0..3).all(|a| maximum[a] > minimum[a]) {
                films.push(FluidFilm {
                    minimum,
                    maximum,
                    kind: TAR,
                });
            }
            continue;
        }
        let first = r.minimum.map(|v| (v - 1.).ceil().clamp(0., 16.) as usize);
        let last = r.maximum.map(|v| (v + 1.).floor().clamp(0., 16.) as usize);
        for z in first[2]..=last[2] {
            for y in first[1]..=last[1] {
                for x in first[0]..=last[0] {
                    let p = [x as f32, y as f32, z as f32];
                    let inside =
                        (0..3).all(|a| p[a] >= r.minimum[a] - EPS && p[a] <= r.maximum[a] + EPS);
                    // At supporting bottoms use the upper free surface's distance.
                    // This preserves thin volume levels; opaque terrain clips the
                    // submerged interface, as in the particle surface renderer.
                    let value = if inside {
                        (r.maximum[1] - p[1]).clamp(0.001, 1.)
                    } else {
                        -(0..3)
                            .map(|a| (p[a] - p[a].clamp(r.minimum[a], r.maximum[a])).powi(2))
                            .sum::<f32>()
                            .sqrt()
                            .min(1.)
                    };
                    let index = x + REST_SIDE * (y + REST_SIDE * z);
                    fields[r.kind as usize][index] = fields[r.kind as usize][index].max(value);
                }
            }
        }
    }
    FarFluidSection {
        epoch,
        fields,
        films,
        published_water: None,
    }
}

fn capture(source: &Source, origin: [i64; 3]) -> (Vec<Region>, Vec<FluidCollider>, Vec<FluidPath>) {
    capture_range(source, origin, 0, SIDE)
}
fn capture_range(
    source: &Source,
    origin: [i64; 3],
    first: i64,
    end: i64,
) -> (Vec<Region>, Vec<FluidCollider>, Vec<FluidPath>) {
    let mut regions = Vec::new();
    let mut solids = Vec::new();
    let mut paths = Vec::new();
    // Only wet columns need climate lookup; reuse it down the column. A dry
    // build or a tar-only road never triggers 256 procedural biome queries.
    let mut blood = std::collections::HashMap::new();
    let mut bodies = std::collections::HashMap::new();
    for y in first..end {
        for z in first..end {
            for x in first..end {
                let local = [x as f32, y as f32, z as f32];
                let p = [origin[0] + x, origin[1] + y, origin[2] + z];
                let Some(material) = source.chunks.block_at(p) else {
                    solids.push(FluidCollider {
                        minimum: local,
                        maximum: local.map(|v| v + 1.),
                        blocked_kinds: ALL_KINDS,
                    });
                    continue;
                };
                let mut cell_solids = Vec::new();
                if matches!(
                    material,
                    terra_world::roads::PATH | terra_core::roads::WET_TAR
                ) {
                    cell_solids.push(ShapeBox {
                        minimum: [0.; 3],
                        maximum: [1., f64::from(terra_core::roads::PATH_HEIGHT), 1.],
                        hinged: false,
                    });
                    paths.push(FluidPath {
                        minimum: [
                            local[0],
                            local[1] + terra_core::roads::PATH_HEIGHT,
                            local[2],
                        ],
                        maximum: local.map(|v| v + 1.),
                    });
                } else if !matches!(material, 0 | 27) && material != terra_core::bile::FLUID {
                    let authored = terra_world::interaction_shapes::visit_collision_boxes(
                        &source.chunks,
                        &source.journal,
                        &source.properties,
                        p,
                        |mut b| {
                            for (i, coordinate) in p.iter().enumerate() {
                                b.minimum[i] -= *coordinate as f64;
                                b.maximum[i] -= *coordinate as f64;
                            }
                            cell_solids.push(b);
                        },
                    );
                    if !authored && MaterialId::from_raw(material).is_none_or(MaterialId::is_solid)
                    {
                        cell_solids.push(ShapeBox {
                            minimum: [0.; 3],
                            maximum: [1.; 3],
                            hinged: false,
                        });
                    }
                }
                for b in &cell_solids {
                    if let Some(collider) = collider(*b, local) {
                        solids.push(collider);
                    }
                }
                let state = source.journal.object_state(p).unwrap_or_default();
                let region = if material == terra_core::bile::FLUID {
                    let volume = state.liquid.map_or((state.facing + 1) * 8, |s| {
                        terra_core::bile::fine_volume(s.volume, s.age)
                    });
                    if volume == 0 {
                        continue;
                    }
                    Region {
                        minimum: local,
                        maximum: [
                            local[0] + 1.,
                            local[1] + f32::from(volume) / 64.,
                            local[2] + 1.,
                        ],
                        kind: GALLE,
                        source: state.liquid.is_none_or(|s| s.source),
                        velocity: [0.; 3],
                        flow_origin: state.liquid.map_or(local, |s| {
                            std::array::from_fn(|i| (s.origin[i] - origin[i]) as f32)
                        }),
                        flow_radius: 4.,
                    }
                } else if material == terra_core::roads::WET_TAR {
                    let volume = state
                        .liquid
                        .map_or(terra_core::roads::tar_amount(state.facing) * 8, |s| {
                            s.volume
                        });
                    if volume == 0 {
                        continue;
                    }
                    Region {
                        minimum: [
                            local[0],
                            local[1] + terra_core::roads::PATH_HEIGHT,
                            local[2],
                        ],
                        maximum: [
                            local[0] + 1.,
                            local[1] + terra_core::roads::tar_fine_height(volume),
                            local[2] + 1.,
                        ],
                        kind: TAR,
                        source: state.liquid.is_none_or(|s| s.origin == p),
                        velocity: [0.; 3],
                        flow_origin: state.liquid.map_or(local, |s| {
                            std::array::from_fn(|i| (s.origin[i] - origin[i]) as f32)
                        }),
                        flow_radius: 4.,
                    }
                } else {
                    if !terra_world::water::can_hold_fluid(material) {
                        continue;
                    }
                    let owner = global_to_section(p).0;
                    if *bodies
                        .entry(owner)
                        .or_insert_with(|| water_body_owner(source, owner))
                    {
                        // Natural water receives finite water, but acid and tar
                        // still cannot replace or pass through its occupied volume.
                        let water = source.chunks.fluid_cell_at(
                            &source.terrain.0,
                            &source.journal,
                            p,
                            material,
                        );
                        if water.amount > 0 {
                            solids.push(FluidCollider {
                                minimum: local,
                                maximum: [
                                    local[0] + 1.,
                                    local[1] + water.height() as f32,
                                    local[2] + 1.,
                                ],
                                blocked_kinds: (1 << GALLE) | (1 << TAR),
                            });
                        }
                        continue;
                    }
                    let water = source.chunks.fluid_cell_at(
                        &source.terrain.0,
                        &source.journal,
                        p,
                        material,
                    );
                    if water.amount == 0 {
                        continue;
                    }
                    let is_blood = *blood.entry([p[0], p[2]]).or_insert_with(|| {
                        source
                            .chunks
                            .surface_column_at(p[0], p[2])
                            .map_or_else(
                                || {
                                    source
                                        .terrain
                                        .0
                                        .biome_at(p[0] as f64 + 0.5, p[2] as f64 + 0.5)
                                },
                                |c| c.biome,
                            )
                            .weights
                            .blood_forest
                            > 0.5
                    });
                    Region {
                        minimum: local,
                        maximum: [
                            local[0] + 1.,
                            local[1] + water.height() as f32,
                            local[2] + 1.,
                        ],
                        kind: if is_blood { BLOOD } else { WATER },
                        source: water.source,
                        velocity: [
                            f32::from(water.flow[0]) * 1.2,
                            0.,
                            f32::from(water.flow[1]) * 1.2,
                        ],
                        flow_origin: local,
                        flow_radius: 0.,
                    }
                };
                // Partial solids are paid only once: sample the same free space that
                // character collision exposes, rather than filling the inside of a slab.
                let mut free = vec![region];
                for solid in &cell_solids {
                    let solid = ShapeBox {
                        minimum: std::array::from_fn(|i| solid.minimum[i] + f64::from(local[i])),
                        maximum: std::array::from_fn(|i| solid.maximum[i] + f64::from(local[i])),
                        hinged: false,
                    };
                    free = free.into_iter().flat_map(|r| subtract(r, solid)).collect();
                }
                for free in free.into_iter().filter(|r| r.volume() > EPS) {
                    // The saved world does not let acid/tar cross water or
                    // other fluids. Preserve those phase boundaries near-field.
                    // Water and blood share one mutually miscible phase.
                    solids.push(FluidCollider {
                        minimum: free.minimum,
                        maximum: free.maximum,
                        blocked_kinds: match free.kind {
                            WATER | BLOOD => (1 << GALLE) | (1 << TAR),
                            GALLE => (1 << WATER) | (1 << BLOOD) | (1 << TAR),
                            _ => (1 << WATER) | (1 << BLOOD) | (1 << GALLE),
                        },
                    });
                    regions.push(free);
                }
            }
        }
    }
    (regions, solids, paths)
}

fn collider(b: ShapeBox, offset: [f32; 3]) -> Option<FluidCollider> {
    let minimum = std::array::from_fn(|i| (b.minimum[i] as f32 + offset[i]).max(0.));
    let maximum = std::array::from_fn(|i| (b.maximum[i] as f32 + offset[i]).min(DOMAIN_SIZE));
    (0..3)
        .all(|i| maximum[i] > minimum[i])
        .then_some(FluidCollider {
            minimum,
            maximum,
            blocked_kinds: ALL_KINDS,
        })
}
fn subtract(region: Region, solid: ShapeBox) -> Vec<Region> {
    let lo: [f32; 3] = std::array::from_fn(|i| region.minimum[i].max(solid.minimum[i] as f32));
    let hi: [f32; 3] = std::array::from_fn(|i| region.maximum[i].min(solid.maximum[i] as f32));
    if (0..3).any(|i| lo[i] >= hi[i]) {
        return vec![region];
    }
    let mut core = region;
    let mut out = Vec::with_capacity(6);
    for axis in 0..3 {
        if core.minimum[axis] < lo[axis] {
            let mut part = core;
            part.maximum[axis] = lo[axis];
            out.push(part);
            core.minimum[axis] = lo[axis];
        }
        if core.maximum[axis] > hi[axis] {
            let mut part = core;
            part.minimum[axis] = hi[axis];
            out.push(part);
            core.maximum[axis] = hi[axis];
        }
    }
    out
}

// Keep particle volume stable as the camera reveals more or less of a lake.
// Filling8192 slots on both sides of a mass change would silently change every
// retained particle's volume and stuff the difference into a narrow boundary.
#[derive(Default)]
struct Resolution {
    quantum: [f32; 4],
}
impl Resolution {
    fn counts(&mut self, mass: [f32; 4]) -> ([usize; 4], [bool; 4]) {
        for (kind, quantum) in self.quantum.iter_mut().enumerate() {
            if mass[kind] <= 0. {
                *quantum = particle_volume(kind as u32);
            } else {
                *quantum = quantum.max(particle_volume(kind as u32));
            }
        }
        let mut out: [usize; 4] =
            std::array::from_fn(|k| (mass[k] / self.quantum[k]).ceil() as usize);
        let mut coarsened = [false; 4];
        while out.iter().sum::<usize>() > MAX_PARTICLES {
            let kind = (0..4).max_by_key(|&k| out[k]).unwrap();
            self.quantum[kind] *= 2.;
            coarsened[kind] = true;
            out[kind] = (mass[kind] / self.quantum[kind]).ceil() as usize;
        }
        (out, coarsened)
    }
}
#[cfg(test)]
fn counts(mass: [f32; 4]) -> [usize; 4] {
    Resolution::default().counts(mass).0
}

// A pool owns its own mass budget. A disconnected source of the same kind is
// not a replacement for the pool from which an existing particle originated.
struct Pool {
    regions: Vec<Region>,
    entering: Vec<Region>,
    wanted: usize,
    kept: usize,
}
fn cell_index(p: [i32; 3]) -> Option<usize> {
    p.iter()
        .all(|v| (0..16).contains(v))
        .then_some(p[0].max(0) as usize + 16 * (p[1].max(0) as usize + 16 * p[2].max(0) as usize))
}
fn same_phase(a: Region, b: Region) -> bool {
    a.kind == b.kind
        && (a.kind < GALLE || (a.flow_origin == b.flow_origin && a.flow_radius == b.flow_radius))
}
fn touching(a: Region, b: Region) -> bool {
    let overlap: [f32; 3] =
        std::array::from_fn(|i| a.maximum[i].min(b.maximum[i]) - a.minimum[i].max(b.minimum[i]));
    // Shared faces connect; touching corners across solid cells do not.
    overlap.iter().all(|&v| v >= -EPS) && overlap.iter().filter(|&&v| v <= EPS).count() <= 1
}
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}
fn pools(regions: &[Region], wanted: [usize; 4], old_domain: ShapeBox) -> Vec<Pool> {
    let mut parent: Vec<_> = (0..regions.len()).collect();
    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); 4096];
    for (i, &r) in regions.iter().enumerate() {
        let cell = std::array::from_fn(|axis| r.minimum[axis].floor() as i32);
        for offset in [
            [0, 0, 0],
            [-1, 0, 0],
            [0, -1, 0],
            [0, 0, -1],
            [1, 0, 0],
            [0, 1, 0],
            [0, 0, 1],
        ] {
            let neighbor = std::array::from_fn(|axis| cell[axis] + offset[axis]);
            if let Some(bucket) = cell_index(neighbor) {
                for &j in &cells[bucket] {
                    if same_phase(r, regions[j]) && touching(r, regions[j]) {
                        let a = root(&mut parent, i);
                        let b = root(&mut parent, j);
                        parent[a] = b;
                    }
                }
            }
        }
        if let Some(bucket) = cell_index(cell) {
            cells[bucket].push(i);
        }
    }
    let mut pool_indices = std::collections::HashMap::new();
    let mut out: Vec<Pool> = Vec::new();
    for (i, &region) in regions.iter().enumerate() {
        let component = root(&mut parent, i);
        let pool = *pool_indices.entry(component).or_insert_with(|| {
            out.push(Pool {
                regions: Vec::new(),
                entering: Vec::new(),
                wanted: 0,
                kept: 0,
            });
            out.len() - 1
        });
        out[pool].regions.push(region);
        out[pool].entering.extend(subtract(region, old_domain));
    }
    // Largest-remainder allocation conserves the bounded total independently for
    // each fluid while neither draining an unrelated basin nor duplicating it.
    for (kind, &budget) in wanted.iter().enumerate() {
        let members: Vec<_> = out
            .iter()
            .enumerate()
            .filter(|(_, p)| p.regions[0].kind as usize == kind)
            .map(|(i, p)| (i, p.regions.iter().map(|r| r.volume()).sum::<f32>()))
            .collect();
        let total: f32 = members.iter().map(|(_, v)| v).sum();
        let mut remainders = Vec::new();
        let mut allocated = 0;
        for &(i, mass) in &members {
            let exact = budget as f64 * f64::from(mass) / f64::from(total.max(EPS));
            out[i].wanted = exact.floor() as usize;
            allocated += out[i].wanted;
            remainders.push((i, exact.fract()));
        }
        remainders.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        for &(i, _) in remainders.iter().take(budget.saturating_sub(allocated)) {
            out[i].wanted += 1;
        }
    }
    out
}
fn distance_squared(p: [f32; 3], r: Region) -> f32 {
    (0..3)
        .map(|i| (p[i] - p[i].clamp(r.minimum[i], r.maximum[i])).powi(2))
        .sum()
}
fn sample(regions: &[Region], ordinal: usize, count: usize, index: u32) -> RegionSample {
    let total: f32 = regions.iter().map(|r| r.volume()).sum();
    let mut mass = total * (ordinal as f32 + 0.5) / count.max(1) as f32;
    let region = *regions
        .iter()
        .find(|r| {
            mass -= r.volume();
            mass <= 0.
        })
        .unwrap_or_else(|| regions.last().expect("A fluid pool contains a region"));
    RegionSample {
        region,
        position: std::array::from_fn(|axis| {
            region.minimum[axis]
                + radical(index, [2, 3, 5][axis]).clamp(0.025, 0.975)
                    * (region.maximum[axis] - region.minimum[axis])
        }),
    }
}
struct RegionSample {
    region: Region,
    position: [f32; 3],
}
fn reconcile_with_anchors(
    slots: &mut Vec<FluidParticleSeed>,
    regions: &[Region],
    wanted: [usize; 4],
    old_origin: [f32; 3],
    origin: [f32; 3],
    generation: &mut u32,
    anchors: &mut Vec<[f32; 3]>,
) -> bool {
    anchors.resize(MAX_PARTICLES, [0.; 3]);
    slots.resize(MAX_PARTICLES, FluidParticleSeed::default());
    let shift: [f32; 3] = std::array::from_fn(|i| old_origin[i] - origin[i]);
    let old_domain = ShapeBox {
        minimum: shift.map(f64::from),
        maximum: shift.map(|v| f64::from(v + DOMAIN_SIZE)),
        hinged: false,
    };
    let mut pools = pools(regions, wanted, old_domain);
    // A tiny separate basin may be smaller than this domain's coarse particle
    // quantum. Keep the ordinary world surface if any wet pool has no samples;
    // the global coverage mask must never hide a real, unrepresented puddle.
    if pools.iter().any(|p| p.wanted == 0) {
        for slot in slots {
            slot.kind = INACTIVE;
        }
        return false;
    }
    // Bounded spatial lookup: anchor matching checks only its27 neighboring cells,
    // rather than scanning every live seed against every region of a large lake.
    let mut cells: Vec<Vec<(usize, usize)>> = vec![Vec::new(); 4096];
    for (pool, p) in pools.iter().enumerate() {
        for (r, region) in p.regions.iter().enumerate() {
            if let Some(bucket) = cell_index(region.minimum.map(|v| v.floor() as i32)) {
                cells[bucket].push((pool, r));
            }
        }
    }
    let mut assigned = vec![None; slots.len()];
    for (slot_index, slot) in slots.iter_mut().enumerate() {
        if slot.kind >= 4 {
            continue;
        }
        let anchor = &mut anchors[slot_index];
        for (i, &delta) in shift.iter().enumerate() {
            slot.position[i] += delta;
            slot.flow_origin[i] += delta;
            anchor[i] += delta;
        }
        // An adjacent transfer is valid inside a stationary domain, but an
        // outgoing anchor cannot spend the incoming strip's particle quota.
        if shift != [0.; 3] && anchor.iter().any(|&v| !(0.0..DOMAIN_SIZE).contains(&v)) {
            slot.kind = INACTIVE;
            continue;
        }
        let cell = anchor.map(|v| v.floor() as i32);
        let mut nearest: Option<(f32, usize, usize)> = None;
        for z in -1..=1 {
            for y in -1..=1 {
                for x in -1..=1 {
                    let neighbor = [cell[0] + x, cell[1] + y, cell[2] + z];
                    let Some(bucket) = cell_index(neighbor) else {
                        continue;
                    };
                    for &(pool, r) in &cells[bucket] {
                        let region = pools[pool].regions[r];
                        if region.kind != slot.kind
                            || (slot.kind >= GALLE
                                && (region.flow_origin != slot.flow_origin
                                    || region.flow_radius != slot.flow_radius))
                        {
                            continue;
                        }
                        let distance = distance_squared(*anchor, region);
                        if distance <= 1.0 + EPS && nearest.is_none_or(|(d, _, _)| distance < d) {
                            nearest = Some((distance, pool, r));
                        }
                    }
                }
            }
        }
        if let Some((_, pool, r)) = nearest
            && pools[pool].kept < pools[pool].wanted
        {
            assigned[slot_index] = Some((pool, r));
            pools[pool].kept += 1;
        } else {
            slot.kind = INACTIVE;
        }
    }
    let mut ordinal = vec![0; pools.len()];
    for (slot_index, (slot, assignment)) in slots.iter_mut().zip(assigned).enumerate() {
        let Some((pool, r)) = assignment else {
            continue;
        };
        let p = &pools[pool];
        let region = p.regions[r];
        let anchor = &mut anchors[slot_index];
        // CPU provenance must remain independent of the GPU fallback candidate:
        // otherwise the following rebase mistakes uniformly sampled candidates
        // for the actual retained mass and retires the wrong boundary slice.
        for (axis, coordinate) in anchor.iter_mut().enumerate() {
            let margin = (region.maximum[axis] - region.minimum[axis]) * 0.025;
            *coordinate =
                coordinate.clamp(region.minimum[axis] + margin, region.maximum[axis] - margin);
        }
        let region = if shift != [0.; 3] && !p.entering.is_empty() {
            // Reconcile in the GPU preserves actual particles still inside the
            // moving cube. Only departed ones restart at these uniform seeds.
            // CPU anchors cannot predict which actual GPU particles left the
            // cube. A fallback samples the whole pool, not the tiny entering
            // band that new CPU particles already fill with their own quota.
            let candidate = sample(&p.regions, ordinal[pool], p.kept, slot.generation);
            slot.position = candidate.position;
            candidate.region
        } else {
            // Adjacent ownership transfers advance only the private anchor;
            // unchanged generation keeps the real GPU motion uninterrupted.
            slot.position = *anchor;
            region
        };
        slot.velocity = region.velocity;
        ordinal[pool] += 1;
    }
    let mut free = slots
        .iter_mut()
        .enumerate()
        .filter(|(_, p)| p.kind == INACTIVE);
    for p in pools {
        let emitters: Vec<_> = p.regions.iter().copied().filter(|r| r.source).collect();
        let add = p.wanted.saturating_sub(p.kept);
        let total: f32 = p.regions.iter().map(|r| r.volume()).sum();
        let incoming: f32 = p.entering.iter().map(|r| r.volume()).sum();
        let entering_quota = if shift != [0.; 3] && p.kept > 0 {
            ((p.wanted as f32 * incoming / total.max(EPS)).round() as usize).min(add)
        } else {
            0
        };
        for i in 0..add {
            let Some((slot_index, slot)) = free.next() else {
                return false;
            };
            *generation = generation.wrapping_add(1).max(1);
            let (sampling, ordinal, amount) = if i < entering_quota {
                (&p.entering, i, entering_quota)
            } else if shift == [0.; 3] && p.kept > 0 && !emitters.is_empty() {
                (&emitters, i, add)
            } else {
                // Initial/coarser resolution and any additional refinement mass
                // must be distributed throughout the pool, never concentrated
                // into its entering strip or a single source.
                (&p.regions, i - entering_quota, add - entering_quota)
            };
            let next = sample(sampling, ordinal, amount, *generation);
            anchors[slot_index] = next.position;
            *slot = FluidParticleSeed {
                position: next.position,
                velocity: next.region.velocity,
                kind: next.region.kind,
                generation: *generation,
                flow_origin: next.region.flow_origin,
                flow_radius: next.region.flow_radius,
            };
        }
    }
    true
}

#[cfg(test)]
fn reconcile(
    slots: &mut Vec<FluidParticleSeed>,
    regions: &[Region],
    wanted: [usize; 4],
    old_origin: [f32; 3],
    origin: [f32; 3],
    generation: &mut u32,
) {
    let mut anchors = slots.iter().map(|s| s.position).collect();
    reconcile_with_anchors(
        slots,
        regions,
        wanted,
        old_origin,
        origin,
        generation,
        &mut anchors,
    );
}

fn radical(mut n: u32, base: u32) -> f32 {
    let mut value = 0.;
    let mut factor = 1. / base as f32;
    while n > 0 {
        value += (n % base) as f32 * factor;
        n /= base;
        factor /= base as f32;
    }
    value
}

fn merge_colliders(boxes: &mut Vec<FluidCollider>) {
    for _ in 0..2 {
        for axis in 0..3 {
            let a = (axis + 1) % 3;
            let b = (axis + 2) % 3;
            boxes.sort_unstable_by(|x, y| {
                x.blocked_kinds
                    .cmp(&y.blocked_kinds)
                    .then(x.minimum[a].total_cmp(&y.minimum[a]))
                    .then(x.maximum[a].total_cmp(&y.maximum[a]))
                    .then(x.minimum[b].total_cmp(&y.minimum[b]))
                    .then(x.maximum[b].total_cmp(&y.maximum[b]))
                    .then(x.minimum[axis].total_cmp(&y.minimum[axis]))
            });
            let mut out: Vec<FluidCollider> = Vec::with_capacity(boxes.len());
            for next in boxes.drain(..) {
                if let Some(last) = out.last_mut()
                    && last.blocked_kinds == next.blocked_kinds
                    && [a, b].into_iter().all(|i| {
                        (last.minimum[i] - next.minimum[i]).abs() < EPS
                            && (last.maximum[i] - next.maximum[i]).abs() < EPS
                    })
                    && next.minimum[axis] <= last.maximum[axis] + EPS
                {
                    last.maximum[axis] = last.maximum[axis].max(next.maximum[axis]);
                } else {
                    out.push(next);
                }
            }
            *boxes = out;
        }
    }
}

fn merge_paths(paths: Vec<FluidPath>) -> Vec<FluidPath> {
    let mut bounds: Vec<_> = paths
        .into_iter()
        .map(|p| FluidCollider {
            minimum: p.minimum,
            maximum: p.maximum,
            blocked_kinds: 0,
        })
        .collect();
    merge_colliders(&mut bounds);
    bounds
        .into_iter()
        .map(|p| FluidPath {
            minimum: p.minimum,
            maximum: p.maximum,
        })
        .collect()
}

#[cfg(test)]
#[path = "world_tests.rs"]
mod tests;
