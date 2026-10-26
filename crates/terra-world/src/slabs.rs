//! Ray-selected halves reuse the existing material IDs and saved facing codes.
use crate::{
    edits::{BlockObjectState, EditJournal},
    streaming::ChunkStore,
};
use terra_core::construction::{slab_bounds, slab_halves};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HalfRemoval {
    pub selected: u8,
    pub removed: u16,
    pub removed_facing: u8,
    pub remaining: u16,
    pub remaining_facing: u8,
}

impl HalfRemoval {
    pub fn remaining_state(self, previous: BlockObjectState) -> BlockObjectState {
        BlockObjectState {
            facing: self.remaining_facing,
            ..previous
        }
    }

    pub fn center(self, position: [i64; 3]) -> [f64; 3] {
        let bounds = slab_bounds(self.removed_facing);
        std::array::from_fn(|axis| {
            position[axis] as f64 + (bounds.minimum[axis] + bounds.maximum[axis]) * 0.5
        })
    }
}

/// Callers still validate the world ray/occlusion and revision. The same nearest
/// half selection is used by client feedback and the server's timed transaction.
pub fn targeted_half(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [i64; 3],
    ray: ([f64; 3], [f64; 3]),
) -> Option<HalfRemoval> {
    if ray.0.iter().chain(&ray.1).any(|v| !v.is_finite()) {
        return None;
    }
    let material = chunks.block_at(position)?;
    let state = journal.object_state(position).unwrap_or_default();
    let halves = slab_halves(material, state.facing)?;
    // Identical pairs retain their established whole-block mining behaviour.
    if halves[0].0 == halves[1].0 {
        return None;
    }
    let local = std::array::from_fn(|axis| ray.0[axis] - position[axis] as f64);
    let selected = halves
        .iter()
        .enumerate()
        .filter_map(|(index, &(_, facing))| {
            slab_bounds(facing)
                .ray(local, ray.1, 8.01)
                .map(|hit| (index, hit.0))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))?
        .0;
    let (removed, removed_facing) = halves[selected];
    let (remaining, remaining_facing) = halves[1 - selected];
    Some(HalfRemoval {
        selected: selected as u8,
        removed,
        removed_facing,
        remaining,
        remaining_facing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::LoadedSection;
    use terra_voxel::{BlockSection, global_to_section};

    #[test]
    fn mixed_slab_rays_select_each_material_and_preserve_the_other_half_on_every_split_plane() {
        for position in [[4, 4, 4], [-17, 15, -16]] {
            for first in [82, 85] {
                for second in [82, 85] {
                    if first == second {
                        continue;
                    }
                    for facing in [0, 4, 5, 6, 7, 8] {
                        let merged =
                            terra_core::construction::merged_slab(first, facing, second).unwrap();
                        let mut chunks = ChunkStore::default();
                        let (section, local) = global_to_section(position);
                        let mut data = BlockSection::empty(section).unwrap();
                        data.set_block(local, first).unwrap();
                        chunks.loaded.insert(
                            section,
                            LoadedSection {
                                data,
                                mesh: None,
                                mesh_epoch: 0,
                                water_epoch: 0,
                                water: None,
                                props: Vec::new(),
                            },
                        );
                        let mut journal = EditJournal::default();
                        journal.record(position, first).unwrap();
                        journal
                            .set_object_state(
                                position,
                                Some(BlockObjectState {
                                    facing: merged,
                                    ..Default::default()
                                }),
                            )
                            .unwrap();
                        let halves = slab_halves(first, merged).unwrap();
                        for index in 0..2 {
                            let bounds = slab_bounds(halves[index].1);
                            let axis = (0..3)
                                .find(|&axis| bounds.maximum[axis] - bounds.minimum[axis] == 0.5)
                                .unwrap();
                            let sign = if bounds.minimum[axis] == 0.5 {
                                1.0
                            } else {
                                -1.0
                            };
                            let mut origin = position.map(|v| v as f64 + 0.5);
                            origin[axis] += sign * 3.0;
                            let mut direction = [0.0; 3];
                            direction[axis] = -sign;
                            let part =
                                targeted_half(&chunks, &journal, position, (origin, direction))
                                    .unwrap();
                            assert_eq!(part.selected, index as u8);
                            assert_eq!((part.removed, part.removed_facing), halves[index]);
                            assert_eq!((part.remaining, part.remaining_facing), halves[1 - index]);
                            let saved =
                                part.remaining_state(journal.object_state(position).unwrap());
                            assert!(saved.valid_for(position, part.remaining));
                            assert!(!terra_core::construction::full_slab(saved.facing));
                        }
                    }
                }
            }
        }
    }
}
