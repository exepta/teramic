//! Authoritative, bounded planted-tree growth. Occupied cells defer growth; nothing is replaced.
mod appearance;
use crate::{
    edits::EditJournal,
    props::{PropId, WorldProp, placement},
    streaming::{ChunkStore, WorldAccess, WorldTerrain},
};
pub(crate) use appearance::AppearanceCache;
pub use appearance::invalidate_tree_visuals;
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{PlayerUuid, TreeRecord, TreeSpecies, TreeStage};
use terra_voxel::global_to_section;

pub const CHECK_BUDGET: usize = 16;
pub const GROWTH_BUDGET: usize = 1;
#[derive(Default)]
pub(crate) struct GrowthWork {
    next: f64,
    cursor: u64,
    grow_cursor: u64,
    visited: BTreeMap<u64, f64>,
    pending: Option<PendingGrowth>,
}

pub fn soil(material: u16) -> bool {
    matches!(material, 1 | 2 | terra_core::farming::FARMLAND)
}
/// Saplings need a clear outdoor shaft or nearby artificial light. Unknown loaded cells
/// are never guessed to be empty. Night delays outdoor growth, not placement.
pub fn light(chunks: &ChunkStore, root: [i64; 3], daylight: bool) -> bool {
    if daylight && (0..=12).all(|dy| chunks.block_at([root[0], root[1] + dy, root[2]]) == Some(0)) {
        return true;
    }
    (-3..=3).any(|x: i64| {
        (-3..=3).any(|y: i64| {
            (-3..=3).any(|z: i64| {
                x.abs() + y.abs() + z.abs() <= 4
                    && matches!(
                        chunks.block_at([root[0] + x, root[1] + y, root[2] + z]),
                        Some(40 | 193)
                    )
                    && (1..=x.abs().max(y.abs()).max(z.abs())).all(|n| {
                        let length = x.abs().max(y.abs()).max(z.abs());
                        let p = [
                            root[0] + x * n / length,
                            root[1] + y * n / length,
                            root[2] + z * n / length,
                        ];
                        chunks
                            .block_at(p)
                            .is_some_and(|m| m == 0 || matches!(m, 40 | 193))
                    })
            })
        })
    })
}
/// Construct the same aligned miniature for preview and authoritative placement.
pub fn placement_prop(support: [i64; 3], species: TreeSpecies, epoch: u64) -> WorldProp {
    let id = placement_seed(support, species.sapling(), epoch);
    let mut prop = WorldProp {
        id: PropId(id),
        kind: species.sapling().into(),
        position: [
            support[0] as f64 + 0.5,
            support[1] as f64 + 1.,
            support[2] as f64 + 0.5,
        ],
        rotation: 0.,
        scale: 1.,
        variant: (id % 6) as u8,
        section: global_to_section(support).0,
        removed: false,
    };
    placement::align(&mut prop, support, 0);
    prop
}
/// Only the actual miniature footprint matters here. Light, neighbour trees and
/// the eventual full crown are independent checks when the tree later grows.
pub fn planting_environment_valid(
    chunks: &ChunkStore,
    journal: &EditJournal,
    support: [i64; 3],
    species: TreeSpecies,
) -> bool {
    placement_check(
        chunks,
        journal,
        placement_prop(
            support,
            species,
            journal.prop_epoch(global_to_section(support).0),
        ),
        None,
    )
    .is_ok()
}
pub fn placement_check(
    chunks: &ChunkStore,
    journal: &EditJournal,
    prop: WorldProp,
    owner: Option<PlayerUuid>,
) -> Result<(), terra_core::trees::SaplingPlacementError> {
    planting_plan(chunks, journal, prop, owner).map(|_| ())
}
pub fn placement_check_in_world(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &crate::TerraGenerator,
    prop: WorldProp,
    owner: Option<PlayerUuid>,
) -> Result<(), terra_core::trees::SaplingPlacementError> {
    placement_check(chunks, journal, prop, owner)?;
    let bounds = prop.bounds();
    let from = bounds[0].map(|v| (v + 0.001).floor() as i64);
    let to = bounds[1].map(|v| (v - 0.001).ceil() as i64);
    if (from[0]..to[0]).any(|x| {
        (from[1]..to[1]).any(|y| {
            (from[2]..to[2])
                .any(|z| chunks.fluid_cell_at(terrain, journal, [x, y, z], 0).amount > 0)
        })
    }) {
        return Err(terra_core::trees::SaplingPlacementError::Water);
    }
    Ok(())
}
struct PlantingPlan {
    prop: WorldProp,
    soft: BTreeSet<(terra_voxel::SectionPos, PropId)>,
}
fn removal_allowed(
    chunks: &ChunkStore,
    journal: &EditJournal,
    prop: &WorldProp,
    owner: PlayerUuid,
) -> bool {
    let allowed = |p| {
        crate::kingdom_access::can_use(
            &journal.gameplay().kingdoms,
            chunks,
            owner,
            terra_core::kingdom::KingdomPermission::Break,
            p,
        )
    };
    let bounds = prop.bounds();
    let from = bounds[0].map(|v| v.floor() as i64);
    let to = bounds[1].map(|v| v.ceil() as i64);
    allowed(placement::support(prop))
        && (from[0]..to[0])
            .all(|x| (from[1]..to[1]).all(|y| (from[2]..to[2]).all(|z| allowed([x, y, z]))))
}
fn planting_plan(
    chunks: &ChunkStore,
    journal: &EditJournal,
    mut prop: WorldProp,
    owner: Option<PlayerUuid>,
) -> Result<PlantingPlan, terra_core::trees::SaplingPlacementError> {
    use terra_core::trees::SaplingPlacementError as Error;
    prop.kind.sapling().ok_or(Error::Occupied)?;
    let support = placement::support(&prop);
    let material = chunks.block_at(support).ok_or(Error::Unloaded)?;
    if !soil(material) {
        return Err(Error::Soil);
    }
    if owner.is_some_and(|owner| owner.is_nil()) {
        return Err(Error::Protected);
    }
    placement::align(&mut prop, support, 0);
    let bounds = prop.bounds();
    let from = bounds[0].map(|v| (v + 0.001).floor() as i64);
    let to = bounds[1].map(|v| (v - 0.001).ceil() as i64);
    let allowed = |p| {
        owner.is_none_or(|owner| {
            crate::kingdom_access::can_use(
                &journal.gameplay().kingdoms,
                chunks,
                owner,
                terra_core::kingdom::KingdomPermission::Build,
                p,
            )
        })
    };
    if !allowed(support) {
        return Err(Error::Protected);
    }
    for x in from[0]..to[0] {
        for y in from[1]..to[1] {
            for z in from[2]..to[2] {
                let p = [x, y, z];
                let material = chunks.block_at(p).ok_or(Error::Unloaded)?;
                if matches!(material, 14 | 27 | 50)
                    || journal.fluid(p).is_some_and(|f| f.amount > 0)
                {
                    return Err(Error::Water);
                }
                if material != 0 || journal.object_state(p).is_some() {
                    return Err(Error::Occupied);
                }
                if !allowed(p) {
                    return Err(Error::Protected);
                }
            }
        }
    }
    let mut soft = BTreeSet::new();
    let web_halo = (crate::props::webs::MAX_SPAN.ceil() as i64 + 16).div_euclid(16) + 1;
    for other in chunks
        .loaded
        .iter()
        .filter_map(|(owner, section)| {
            let distance = (owner.x - prop.section.x)
                .abs()
                .max((owner.y - prop.section.y).abs())
                .max((owner.z - prop.section.z).abs());
            (distance <= web_halo).then_some((distance, section))
        })
        .flat_map(|(distance, section)| {
            section
                .props
                .iter()
                .filter(move |p| distance <= 1 || crate::props::webs::is_construction(p))
        })
        .filter(|p| !p.removed)
    {
        if other.id == prop.id {
            return Err(Error::Changed);
        }
        let b = other.bounds();
        let overlap = placement::support(other) == support
            || (0..3).all(|a| b[1][a] > bounds[0][a] + 0.001 && b[0][a] < bounds[1][a] - 0.001);
        if !overlap {
            continue;
        }
        if !soft_natural(other, journal) {
            return Err(Error::Occupied);
        }
        if owner.is_some_and(|owner| !removal_allowed(chunks, journal, other, owner)) {
            return Err(Error::Protected);
        }
        soft.insert((other.section, other.id));
    }
    if soft.len() > 64
        || !journal.sapling_prop_fits(&prop, &soft)
        || journal.gameplay().trees.trees.len() >= terra_core::trees::MAX_TREES
        || journal.gameplay().trees.revision > u64::MAX - 2
    {
        return Err(Error::Limit);
    }
    if journal.gameplay().trees.get(prop.id.0).is_some() {
        return Err(Error::Changed);
    }
    Ok(PlantingPlan { prop, soft })
}
pub fn plant(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    prop: WorldProp,
    owner: PlayerUuid,
) -> Result<bool, String> {
    let Some(species) = prop.kind.sapling() else {
        return Ok(false);
    };
    let Ok(plan) = planting_plan(chunks, journal, prop, Some(owner)) else {
        return Ok(false);
    };
    let prop = plan.prop;
    let support = placement::support(&prop);
    let root = [support[0], support[1] + 1, support[2]];
    let tree = TreeRecord {
        id: prop.id.0,
        root,
        species,
        seed: prop.id.0,
        planted_by: Some(owner),
        stage: TreeStage::Sapling,
        growth_seconds: 0.,
        bounds: None,
        wood: vec![],
        leaves: vec![],
        fruit: None,
        revision: 0,
    };
    if !tree.valid() || !journal.place_sapling_prop(prop, &plan.soft) {
        return Ok(false);
    }
    for &(section, id) in &plan.soft {
        let loaded = chunks
            .loaded
            .get_mut(&section)
            .expect("validated natural prop section");
        if let Some(p) = loaded.props.iter_mut().find(|p| p.id == id) {
            p.removed = true;
        }
    }
    chunks
        .loaded
        .get_mut(&prop.section)
        .expect("validated sapling support section")
        .props
        .push(prop);
    for section in std::iter::once(prop.section)
        .chain(plan.soft.iter().map(|(p, _)| *p))
        .collect::<BTreeSet<_>>()
    {
        placement::reflow(
            &mut chunks
                .loaded
                .get_mut(&section)
                .expect("validated prop section")
                .props,
        );
    }
    assert!(journal.gameplay_mut().trees.insert(tree));
    Ok(true)
}

/// All newly grown trees use the existing generator's capsules/crowns.
pub fn shape(species: TreeSpecies, seed: u64) -> (Vec<[i8; 3]>, Vec<[i8; 3]>) {
    crate::generation::planted_shape(species, seed).unwrap_or_default()
}
/// Shared identity for authority placement and headless room preparation.
pub fn placement_seed(support: [i64; 3], kind: terra_core::ResourcePropKind, epoch: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ epoch.rotate_left(23) ^ kind as u64;
    for value in support {
        hash = (hash ^ value as u64).wrapping_mul(0x100_0000_01b3);
    }
    hash
}
fn prop_live(chunks: &ChunkStore, tree: &TreeRecord) -> bool {
    let support = [tree.root[0], tree.root[1] - 1, tree.root[2]];
    chunks
        .loaded
        .get(&global_to_section(support).0)
        .is_some_and(|s| {
            s.props
                .iter()
                .any(|p| p.id.0 == tree.id && !p.removed && p.kind.sapling() == Some(tree.species))
        })
}
/// Only authentic unsaved ground-cover grass/flowers can yield to a trunk/crown.
/// User-placed flowers/grass remain in placed_props and always block growth.
pub(crate) fn soft_natural(p: &WorldProp, journal: &EditJournal) -> bool {
    matches!(
        p.kind,
        crate::props::PropKind::Grass | crate::props::PropKind::Flower
    ) && !p.growing_grass()
        && !journal
            .placed_props(p.section)
            .any(|saved| saved.id == p.id)
}
fn prop_space(
    chunks: &ChunkStore,
    journal: &EditJournal,
    id: u64,
    cells: &[([i64; 3], u16)],
) -> Option<BTreeSet<(terra_voxel::SectionPos, PropId)>> {
    let occupied: std::collections::HashSet<_> = cells.iter().map(|(p, _)| *p).collect();
    let low: [i64; 3] = std::array::from_fn(|a| cells.iter().map(|(p, _)| p[a]).min().unwrap());
    let high: [i64; 3] =
        std::array::from_fn(|a| cells.iter().map(|(p, _)| p[a] + 1).max().unwrap());
    let mut soft = BTreeSet::new();
    let actor = journal.gameplay().trees.get(id)?.planted_by?;
    // Props can span several owner sections (large silk webs); test their real
    // bounds, then a clipped integer footprint. No world/terrain sampling occurs.
    for p in chunks
        .loaded
        .values()
        .flat_map(|s| &s.props)
        .filter(|p| !p.removed && p.id.0 != id)
    {
        let b = p.bounds();
        if (0..3).any(|a| b[1][a] <= low[a] as f64 + 0.001 || b[0][a] >= high[a] as f64 - 0.001) {
            continue;
        }
        let from: [i64; 3] =
            std::array::from_fn(|a| (b[0][a] + 0.001).floor().max(low[a] as f64) as i64);
        let to: [i64; 3] =
            std::array::from_fn(|a| (b[1][a] - 0.001).ceil().min(high[a] as f64) as i64);
        let hit = (from[0]..to[0]).any(|x| {
            (from[1]..to[1]).any(|y| (from[2]..to[2]).any(|z| occupied.contains(&[x, y, z])))
        });
        if hit {
            if soft_natural(p, journal) {
                let allowed = |cell| {
                    crate::kingdom_access::can_use(
                        &journal.gameplay().kingdoms,
                        chunks,
                        actor,
                        terra_core::kingdom::KingdomPermission::Break,
                        cell,
                    )
                };
                let from = b[0].map(|v| v.floor() as i64);
                let to = b[1].map(|v| v.ceil() as i64);
                if !allowed(placement::support(p))
                    || !(from[0]..to[0]).all(|x| {
                        (from[1]..to[1]).all(|y| (from[2]..to[2]).all(|z| allowed([x, y, z])))
                    })
                {
                    return None;
                }
                soft.insert((p.section, p.id));
            } else {
                return None;
            }
        }
    }
    Some(soft)
}
fn begin_growth(
    chunks: &mut ChunkStore,
    journal: &EditJournal,
    id: u64,
    wood: Vec<[i8; 3]>,
    leaves: Vec<[i8; 3]>,
) -> Option<crate::edits::TreeGrowthBatch> {
    let tree = journal.gameplay().trees.get(id)?;
    if tree.stage != TreeStage::Sapling
        || tree.growth_seconds < tree.species.growth_seconds()
        || !prop_live(chunks, tree)
        || wood.is_empty()
    {
        return None;
    }
    let support = [tree.root[0], tree.root[1] - 1, tree.root[2]];
    if !chunks.block_at(support).is_some_and(soil) {
        return None;
    }
    let cells: Vec<_> = wood
        .iter()
        .map(|p| (tree.global(*p), tree.species.wood()))
        .chain(
            leaves
                .iter()
                .map(|p| (tree.global(*p), tree.species.leaves())),
        )
        .collect();
    let sections: BTreeSet<_> = cells.iter().map(|(p, _)| global_to_section(*p).0).collect();
    if sections.iter().any(|s| !chunks.loaded.contains_key(s)) {
        chunks.request_sections(sections);
        return None;
    }
    let soft = prop_space(chunks, journal, id, &cells)?;
    crate::edits::TreeGrowthBatch::begin(journal, chunks, id, wood, leaves, soft)
}
/// Synchronous setup/test API uses exactly the same staged transaction. Runtime
/// calls the bounded worker path below, rather than building an entire tree here.
pub fn grow(chunks: &mut ChunkStore, journal: &mut EditJournal, id: u64) -> Result<bool, String> {
    let Some(tree) = journal.gameplay().trees.get(id) else {
        return Ok(false);
    };
    if tree.stage != TreeStage::Sapling || tree.growth_seconds < tree.species.growth_seconds() {
        return Ok(false);
    }
    let (wood, leaves) = shape(tree.species, tree.seed);
    let Some(mut batch) = begin_growth(chunks, journal, id, wood, leaves) else {
        return Ok(false);
    };
    loop {
        match batch.advance(journal, chunks, CELL_BUDGET) {
            Ok(true) => break,
            Ok(false) => {}
            Err(_) => return Ok(false),
        }
    }
    if prop_space(chunks, journal, id, &batch.cells).as_ref() != Some(&batch.soft) {
        return Ok(false);
    }
    batch.publish(journal, chunks)
}
pub const CELL_BUDGET: usize = 512;
type ShapeCells = (Vec<[i8; 3]>, Vec<[i8; 3]>);
enum PendingGrowth {
    Shape {
        record: TreeRecord,
        task: bevy::tasks::Task<ShapeCells>,
    },
    Ready {
        record: TreeRecord,
        wood: Vec<[i8; 3]>,
        leaves: Vec<[i8; 3]>,
    },
    Staging {
        id: u64,
        batch: Box<crate::edits::TreeGrowthBatch>,
    },
}
fn advance_pending(work: &mut GrowthWork, chunks: &mut ChunkStore, journal: &mut EditJournal) {
    use bevy::tasks::futures_lite::future;
    let Some(pending) = work.pending.take() else {
        return;
    };
    work.pending = match pending {
        PendingGrowth::Shape { record, mut task } => {
            match future::block_on(future::poll_once(&mut task)) {
                Some((wood, leaves)) => Some(PendingGrowth::Ready {
                    record,
                    wood,
                    leaves,
                }),
                None => Some(PendingGrowth::Shape { record, task }),
            }
        }
        PendingGrowth::Ready {
            record,
            wood,
            leaves,
        } => {
            if journal.gameplay().trees.get(record.id) != Some(&record)
                || !prop_live(chunks, &record)
            {
                None
            } else {
                let sections: BTreeSet<_> = wood
                    .iter()
                    .chain(&leaves)
                    .map(|p| global_to_section(record.global(*p)).0)
                    .collect();
                if sections.iter().any(|s| !chunks.loaded.contains_key(s)) {
                    chunks.request_sections(sections);
                    Some(PendingGrowth::Ready {
                        record,
                        wood,
                        leaves,
                    })
                } else {
                    begin_growth(chunks, journal, record.id, wood, leaves).map(|batch| {
                        PendingGrowth::Staging {
                            id: record.id,
                            batch: Box::new(batch),
                        }
                    })
                }
            }
        }
        PendingGrowth::Staging { id, mut batch } => {
            chunks.request_sections(batch.requested_sections().collect::<Vec<_>>());
            match batch.advance(journal, chunks, CELL_BUDGET) {
                Ok(false) => Some(PendingGrowth::Staging { id, batch }),
                Ok(true) => {
                    if prop_space(chunks, journal, id, &batch.cells).as_ref() == Some(&batch.soft)
                        && let Err(e) = (*batch).publish(journal, chunks)
                    {
                        tracing::error!(%e,"Tree growth publish failed");
                    }
                    None
                }
                Err(_) => None,
            }
        }
    };
}
pub(crate) fn tick(
    time: Res<Time>,
    access: Res<WorldAccess>,
    terrain: Res<WorldTerrain>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    mut work: Local<GrowthWork>,
) {
    if !access.authoritative() {
        return;
    }
    advance_pending(&mut work, &mut chunks, &mut journal);
    if time.elapsed_secs_f64() < work.next {
        return;
    }
    let now = time.elapsed_secs_f64();
    work.next = now + 2.;
    let daylight = (0.23..=0.77).contains(&journal.clock.unwrap_or_default().phase);
    let ids: Vec<_> = journal
        .gameplay()
        .trees
        .trees
        .range(work.cursor..)
        .chain(journal.gameplay().trees.trees.range(..work.cursor))
        .filter(|(_, tree)| tree.stage == TreeStage::Sapling)
        .take(CHECK_BUDGET)
        .map(|(id, _)| *id)
        .collect();
    let mut ready = Vec::new();
    for id in ids {
        work.cursor = id.saturating_add(1);
        let elapsed = work
            .visited
            .insert(id, now)
            .map_or(0., |last| (now - last).clamp(0., 60.) as f32);
        let Some(tree) = journal.gameplay().trees.get(id).cloned() else {
            continue;
        };
        if !prop_live(&chunks, &tree)
            || chunks
                .fluid_cell_at(&terrain.0, &journal, tree.root, 0)
                .amount
                > 0
            || !light(&chunks, tree.root, daylight)
        {
            continue;
        }
        if tree.growth_seconds < tree.species.growth_seconds() && elapsed > 0. {
            let registry = &mut journal.gameplay_mut().trees;
            let tree = registry.get_mut(id).unwrap();
            let before = (tree.growth_seconds / tree.species.growth_seconds() * 100.) as u8;
            tree.growth_seconds =
                (tree.growth_seconds + elapsed).min(tree.species.growth_seconds());
            if before != (tree.growth_seconds / tree.species.growth_seconds() * 100.) as u8 {
                tree.revision = tree.revision.saturating_add(1);
                registry.changed();
            }
        }
        if journal
            .gameplay()
            .trees
            .get(id)
            .is_some_and(|tree| tree.growth_seconds >= tree.species.growth_seconds())
        {
            ready.push(id);
        }
    }
    ready.sort_unstable();
    let split = ready.partition_point(|id| *id < work.grow_cursor);
    if work.pending.is_none()
        && let Some(id) = ready[split..].iter().chain(&ready[..split]).next()
    {
        work.grow_cursor = id.saturating_add(1);
        let record = journal.gameplay().trees.get(*id).unwrap().clone();
        let species = record.species;
        let seed = record.seed;
        let task =
            bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { shape(species, seed) });
        work.pending = Some(PendingGrowth::Shape { record, task });
    }
}
#[cfg(test)]
mod tests;

/// One-in-sixteen attributed leaves return their own species' sapling. Placed leaf
/// blocks are excluded; full-tree felling separately guarantees one replacement.
pub fn leaf_drop(
    chunks: &ChunkStore,
    journal: &EditJournal,
    cell: [i64; 3],
    material: u16,
) -> Option<TreeSpecies> {
    if !matches!(material, 7..=9 | 132 | 133 | 195) {
        return None;
    }
    let hash = cell.into_iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, v| {
        (h ^ v as u64).wrapping_mul(0x100_0000_01b3)
    });
    if hash % 16 != 0 {
        return None;
    }
    if let Some(tree) = journal.gameplay().trees.owner(cell, material) {
        return Some(tree.species);
    }
    if journal.block(cell).is_some() {
        return None;
    }
    let column = chunks.prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])?;
    let root = column.felling_owner(cell, material)?;
    if let Some(species) = column.felling_species(root) {
        return Some(species);
    }
    let materials = column.felling_materials(root)?;
    Some(match materials {
        (6, 195) => TreeSpecies::Apple,
        (6, 7 | 8) => TreeSpecies::Oak,
        (10, 9) => TreeSpecies::Birch,
        (36, 7) => TreeSpecies::Spruce,
        (134, 132 | 133) => TreeSpecies::Cherry,
        _ => return None,
    })
}
