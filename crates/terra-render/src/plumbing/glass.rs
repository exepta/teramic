//! Confirmed pipe contents share the existing bounded hydraulic draw batch.
use super::Geometry;
use crate::meshing::custom::plumbing::pipe;
use bevy::prelude::*;
use std::collections::HashMap;
use terra_core::plumbing::{self as core, Cell};
use terra_world::{edits::EditJournal, streaming::ChunkStore};

#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct GlassPipeRenderStatus {
    /// Accepted snapshot used to construct this bounded render batch.
    pub plumbing_sequence: u64,
    pub nodes: Vec<GlassPipeNodeRenderStatus>,
    pub pipes: usize,
    pub filled: usize,
    pub units: u16,
    /// Geometry belonging to stored units; transit has its own counter below.
    pub vertices: usize,
    /// Confirmed transfer through a pipe can leave no stored units at tick end.
    pub flowing: usize,
    pub transit_vertices: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlassPipeNodeRenderStatus {
    pub position: Cell,
    pub amount: u16,
    pub flowing: bool,
    pub vertices: usize,
}

#[derive(Default, Clone, Copy)]
pub(super) struct Liquid {
    pub amount: u16,
    pub flowing: bool,
    /// Confirmed neighboring network amount and transfer flag, ordered like DIRECTIONS.
    pub neighbors: [Option<(u8, bool)>; 6],
}

#[derive(Default)]
pub(super) struct Shapes {
    levels: HashMap<(u8, u8, [u32; 6]), Geometry>,
    bounds: HashMap<u8, (f32, f32)>,
}

pub(super) fn append(
    geometry: &mut Geometry,
    shapes: &mut Shapes,
    position: Cell,
    liquid: Liquid,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> usize {
    if liquid.amount == 0 && !liquid.flowing {
        return 0;
    }
    let block_at = |p| {
        chunks
            .block_at(p)
            .map(|raw| journal.block(p).unwrap_or(raw))
    };
    let facing = journal.object_state(position).unwrap_or_default().facing;
    let mask = terra_world::plumbing::connection_mask(
        journal,
        position,
        core::GLASS_PIPE,
        facing,
        block_at,
    );
    let mut neighbor_levels = [None; 6];
    for (side, d) in core::DIRECTIONS.into_iter().enumerate() {
        let Some((amount, flowing)) = liquid.neighbors[side] else {
            continue;
        };
        if amount == 0 && !flowing {
            continue;
        }
        let neighbor = std::array::from_fn(|axis| position[axis] + d[axis]);
        let Some(material) = block_at(neighbor) else {
            continue;
        };
        let facing = journal.object_state(neighbor).unwrap_or_default().facing;
        if !core::is_pipe(material) || !core::connects(material, facing, core::DIRECTIONS[side ^ 1])
        {
            continue;
        }
        let neighbor_mask =
            terra_world::plumbing::connection_mask(journal, neighbor, material, facing, block_at);
        let (low, high) = *shapes.bounds.entry(neighbor_mask).or_insert_with(|| {
            let shape = pipe::contents(neighbor_mask, neighbor_mask);
            let low = shape
                .positions
                .iter()
                .map(|p| p[1])
                .fold(f32::INFINITY, f32::min);
            let high = shape
                .positions
                .iter()
                .map(|p| p[1])
                .fold(f32::NEG_INFINITY, f32::max);
            (low, high)
        });
        if amount > 0 {
            neighbor_levels[side] =
                Some(d[1] as f32 + low + (high - low) * f32::from(amount.min(8)) / 8.);
        }
    }
    // Zero-storage transit is a narrow stream through the entire actual route,
    // including vertical sections. It has no invented 1/8-height stored level.
    let units = liquid.amount.min(u16::from(core::CELL_CAPACITY)) as u8;
    let transit_neighbors: [bool; 6] = std::array::from_fn(|side| {
        let d = core::DIRECTIONS[side];
        let neighbor = std::array::from_fn(|axis| position[axis] + d[axis]);
        liquid.neighbors[side].is_some_and(|(amount, flowing)| amount > 0 || flowing)
            && block_at(neighbor).is_some_and(|material| {
                core::connects(
                    material,
                    journal.object_state(neighbor).unwrap_or_default().facing,
                    core::DIRECTIONS[side ^ 1],
                )
            })
    });
    let signature = if units == 0 {
        transit_neighbors.map(u32::from)
    } else {
        neighbor_levels.map(|level| level.map_or(u32::MAX, f32::to_bits))
    };
    let key = (mask, units, signature);
    if shapes.levels.len() >= 512 && !shapes.levels.contains_key(&key) {
        shapes.levels.clear();
    }
    let shape = shapes.levels.entry(key).or_insert_with(|| {
        if units == 0 {
            let connected = transit_neighbors
                .iter()
                .enumerate()
                .fold(0, |ports, (side, &wet)| {
                    ports | if wet { 1 << side } else { 0 }
                });
            let stream = pipe::transit_contents(mask, mask & !connected);
            Geometry {
                positions: stream.positions,
                normals: stream.normals,
                indices: stream.indices,
                ..Default::default()
            }
        } else {
            super::level::clipped(
                &pipe::contents(mask, mask),
                f32::from(units) / 8.,
                neighbor_levels,
            )
        }
    });
    let anchor = position.map(|v| v as f32);
    let base = geometry.positions.len() as u32;
    geometry.positions.extend(
        shape
            .positions
            .iter()
            .map(|p| std::array::from_fn(|axis| anchor[axis] + p[axis])),
    );
    geometry.normals.extend_from_slice(&shape.normals);
    let low = shape
        .positions
        .iter()
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min);
    let high = shape
        .positions
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let depth = (high - low).clamp(0.015, 0.22);
    geometry.colors.extend(
        shape
            .positions
            .iter()
            .map(|_| [depth, if liquid.flowing { 1. } else { 0. }, 0., 7.]),
    );
    geometry
        .indices
        .extend(shape.indices.iter().map(|i| base + i));
    shape.positions.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, global_to_section};
    use terra_world::streaming::LoadedSection;

    #[test]
    fn empty_glass_has_no_liquid_and_confirmed_contents_follow_all_three_pipe_axes() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let position = [-1, 5, -1];
        let (owner, local) = global_to_section(position);
        let mut section = BlockSection::empty(owner).unwrap();
        section.set_block(local, core::GLASS_PIPE).unwrap();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: section,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        journal.record(position, core::GLASS_PIPE).unwrap();
        let mut shapes = Shapes::default();
        for facing in 0..6 {
            journal
                .set_object_state(
                    position,
                    Some(terra_world::edits::BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
                .unwrap();
            let mut geometry = Geometry::default();
            assert_eq!(
                append(
                    &mut geometry,
                    &mut shapes,
                    position,
                    Liquid::default(),
                    &chunks,
                    &journal
                ),
                0
            );
            assert!(geometry.indices.is_empty());
            let count = append(
                &mut geometry,
                &mut shapes,
                position,
                Liquid {
                    amount: 8,
                    ..Default::default()
                },
                &chunks,
                &journal,
            );
            assert!(count > 0 && count <= 4000);
            let axis = terra_core::kinetic::axis(facing)
                .iter()
                .position(|v| *v != 0)
                .unwrap();
            let min = geometry
                .positions
                .iter()
                .map(|p| p[axis])
                .fold(f32::INFINITY, f32::min);
            let max = geometry
                .positions
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((min - position[axis] as f32).abs() < 1e-5);
            assert!((max - position[axis] as f32 - 1.).abs() < 1e-5);
            for point in &geometry.positions {
                let mut radial = Vec3::from_array(*point)
                    - Vec3::from_array(position.map(|v| v as f32))
                    - Vec3::splat(0.5);
                radial[axis] = 0.;
                assert!(radial.length() < 0.118);
            }
            assert_eq!(geometry.positions.len(), geometry.colors.len());
            assert_eq!(geometry.positions.len(), geometry.normals.len());
            let revision = journal.gameplay().revision;
            let delta = journal.section_delta(owner);
            let mut transit = Geometry::default();
            assert!(
                append(
                    &mut transit,
                    &mut shapes,
                    position,
                    Liquid {
                        amount: 0,
                        flowing: true,
                        ..Default::default()
                    },
                    &chunks,
                    &journal
                ) > 0
            );
            assert!(
                transit
                    .colors
                    .iter()
                    .all(|color| color[3] == 7. && color[1] == 1. && color[0] <= 0.22)
            );
            let transit_min = transit
                .positions
                .iter()
                .map(|p| p[axis])
                .fold(f32::INFINITY, f32::min);
            let transit_max = transit
                .positions
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((transit_min - position[axis] as f32).abs() < 1e-5);
            assert!(
                (transit_max - position[axis] as f32 - 1.).abs() < 1e-5,
                "confirmed transit must span a whole vertical or horizontal pipe, not an invented stored level"
            );
            for p in &transit.positions {
                let mut radial = Vec3::from_array(*p)
                    - Vec3::from_array(position.map(|v| v as f32))
                    - Vec3::splat(0.5);
                radial[axis] = 0.;
                assert!(radial.length() <= 0.03501);
            }
            let mut later = Geometry::default();
            append(
                &mut later,
                &mut shapes,
                position,
                Liquid {
                    amount: 0,
                    flowing: true,
                    ..Default::default()
                },
                &chunks,
                &journal,
            );
            assert_eq!(transit.positions, later.positions);
            assert_eq!(
                transit.colors, later.colors,
                "the shared water shader animates currents without rebuilding geometry"
            );
            let mut stopped = Geometry::default();
            assert_eq!(
                append(
                    &mut stopped,
                    &mut shapes,
                    position,
                    Liquid::default(),
                    &chunks,
                    &journal
                ),
                0
            );
            assert!(stopped.indices.is_empty());
            assert_eq!(journal.gameplay().revision, revision);
            assert_eq!(journal.section_delta(owner), delta);
            assert!(journal.gameplay().inventories.is_empty());
            assert!(
                journal.plumbing_at(position).is_none(),
                "visual transit must not invent stored water"
            );
        }
    }
}
