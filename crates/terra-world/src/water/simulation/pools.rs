//! Bounded resting shallow pools; source creation stays confined to closed 2x2 basins.
use super::*;
use crate::water::space::FluidSpace;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use terra_core::{
    block_shapes::{ShapeBox, builtin_boxes},
    plumbing::Cell,
};

const EPSILON: f64 = 1e-8;
const FULL: FluidCell = FluidCell {
    amount: FLUID_CAPACITY,
    flow: [0; 2],
    source: false,
};

fn cells(origin: Cell) -> [Cell; 4] {
    [
        [origin[0], origin[1], origin[2]],
        [origin[0] + 1, origin[1], origin[2]],
        [origin[0], origin[1], origin[2] + 1],
        [origin[0] + 1, origin[1], origin[2] + 1],
    ]
}

/// Physical support cannot be inferred from can_hold_fluid: open machinery and
/// tanks intentionally seal terrain fluid voxels, but do not provide a full roof.
fn solid_space(sampler: &mut Sampler<'_>, journal: &EditJournal, p: Cell) -> Option<FluidSpace> {
    let material = sampler.chunks.block_at(p)?;
    if crate::water::space::partial(material) {
        return Some(sampler.space(journal, p).clone());
    }
    if terra_core::plumbing::is_part(material) {
        return Some(FluidSpace {
            solids: crate::plumbing::plumbing_boxes(
                journal,
                p,
                material,
                journal.object_state(p).unwrap_or_default(),
                |q| sampler.chunks.block_at(q),
            ),
        });
    }
    let bounds = builtin_boxes(material);
    let solids = if !bounds.is_empty() {
        let facing = journal.object_state(p).unwrap_or_default().facing;
        bounds
            .iter()
            .map(|bounds| bounds.rotated_y(facing))
            .collect()
    } else if terra_voxel::MaterialId::from_raw(material)
        .is_some_and(|material| material.is_solid())
    {
        vec![ShapeBox {
            minimum: [0.; 3],
            maximum: [1.; 3],
            hinged: false,
        }]
    } else {
        vec![]
    };
    Some(FluidSpace { solids })
}

pub(super) const MAX_POOL_CELLS: usize = 64;
const STEP_POOL_BUDGET: usize = 256;

#[derive(Default)]
pub(super) struct Resting {
    handled: HashSet<Cell>,
    checked: HashSet<Cell>,
    examined: usize,
}

struct Proof {
    positions: Vec<Cell>,
    ready: bool,
    waiting: Option<terra_voxel::SectionPos>,
    deferred: bool,
    walls: Vec<(Cell, Cell, FluidSpace)>,
}
impl Resting {
    pub(super) fn handled(&self, p: Cell) -> bool {
        self.handled.contains(&p)
    }

    fn geometry(&mut self, sampler: &mut Sampler<'_>, journal: &EditJournal, p: Cell) -> Proof {
        let mut proof = Proof {
            positions: vec![],
            ready: false,
            waiting: None,
            deferred: false,
            walls: vec![],
        };
        if self.examined >= STEP_POOL_BUDGET {
            proof.deferred = true;
            return proof;
        }
        let mut seen = HashSet::from([p]);
        let mut queue = VecDeque::from([p]);
        proof.positions.push(p);
        self.examined += 1;
        while let Some(p) = queue.pop_front() {
            if !matches!(sampler.chunks.block_at(p), Some(0 | 27))
                || !sampler.space(journal, p).solids.is_empty()
            {
                return proof;
            }
            let above = [p[0], p[1] + 1, p[2]];
            match sampler.cell(journal, above) {
                Some(cell) if cell.amount > 0 => {
                    return Proof {
                        waiting: None,
                        ..proof
                    };
                }
                Some(_) => {}
                None => {
                    proof.waiting.get_or_insert(global_to_section(above).0);
                }
            }
            let below = [p[0], p[1] - 1, p[2]];
            match solid_space(sampler, journal, below) {
                Some(floor)
                    if floor
                        .wet_boxes(1.)
                        .iter()
                        .any(|void| void.maximum[1] >= 1. - EPSILON) =>
                {
                    return Proof {
                        waiting: None,
                        ..proof
                    };
                }
                Some(_) => {}
                None => {
                    proof.waiting.get_or_insert(global_to_section(below).0);
                }
            }
            for direction in [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]] {
                let q: Cell = std::array::from_fn(|axis| p[axis] + direction[axis]);
                if seen.contains(&q) {
                    continue;
                }
                let Some(material) = sampler.chunks.block_at(q) else {
                    proof.waiting.get_or_insert(global_to_section(q).0);
                    continue;
                };
                if matches!(material, 0 | 27) && sampler.space(journal, q).solids.is_empty() {
                    if self.checked.contains(&q) || seen.len() == MAX_POOL_CELLS {
                        // All discovered members share this open horizontal
                        // region, even those still waiting in the BFS queue.
                        return Proof {
                            waiting: None,
                            ..proof
                        };
                    }
                    if self.examined >= STEP_POOL_BUDGET {
                        proof.deferred = true;
                        return proof;
                    }
                    seen.insert(q);
                    proof.positions.push(q);
                    self.examined += 1;
                    queue.push_back(q);
                } else {
                    let wall = solid_space(sampler, journal, q).unwrap();
                    proof.walls.push((q, direction, wall));
                }
            }
        }
        let mut total = 0u16;
        let mut supplied = false;
        for &cell in &proof.positions {
            let fluid = sampler.cell(journal, cell).unwrap();
            total += u16::from(fluid.amount);
            supplied |= fluid.source;
        }
        // A lower slab is watertight below its lip. Testing every wall at
        // FULL8 would reject a supported half-height pool and discard its
        // finite water despite there being no physically wet escape route.
        let mut level = if supplied || total == 0 {
            FLUID_CAPACITY
        } else {
            total.div_ceil(proof.positions.len() as u16) as u8
        };
        for (q, _, _) in &proof.walls {
            level = level.max(sampler.cell(journal, *q).map_or(0, |cell| cell.amount));
        }
        let wet = FluidCell {
            amount: level,
            ..FULL
        };
        for (position, direction, wall) in &proof.walls {
            if sampler
                .chunks
                .block_at(*position)
                .is_some_and(terra_core::construction::is_shape)
            {
                continue;
            }
            if (FluidSpace { solids: vec![] }).connected(wet, wall, wet, *direction) {
                return proof;
            }
        }
        proof.positions.sort_unstable_by_key(|p| (p[2], p[0]));
        proof.ready = proof.waiting.is_none();
        proof
    }

    /// A complete basin is processed before ordinary attenuating flow, including
    /// its dry members, so conservation does not depend on work queue order.
    pub(super) fn settle(
        &mut self,
        sampler: &mut Sampler<'_>,
        journal: &mut EditJournal,
        p: Cell,
        step: &mut FluidStep,
    ) -> Result<bool, FluidError> {
        if !matches!(sampler.chunks.block_at(p), Some(0 | 27)) || self.checked.contains(&p) {
            return Ok(false);
        }
        // Constant local checks remain available after the BFS budget runs out.
        // A falling column or a member of an already rejected open region must
        // continue receding instead of being requeued as an undecided basin.
        let above = [p[0], p[1] + 1, p[2]];
        let below = [p[0], p[1] - 1, p[2]];
        let locally_open = sampler
            .cell(journal, above)
            .is_some_and(|cell| cell.amount > 0)
            || solid_space(sampler, journal, below).is_some_and(|floor| {
                floor
                    .wet_boxes(1.)
                    .iter()
                    .any(|void| void.maximum[1] >= 1. - EPSILON)
            })
            || [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]]
                .into_iter()
                .any(|direction| {
                    let q = std::array::from_fn(|axis| p[axis] + direction[axis]);
                    self.checked.contains(&q) && matches!(sampler.chunks.block_at(q), Some(0 | 27))
                });
        if locally_open {
            self.checked.insert(p);
            return Ok(false);
        }
        let proof = self.geometry(sampler, journal, p);
        if proof.deferred || proof.waiting.is_some() {
            // No extent/support proof means no finite water is discarded while
            // waiting for a chunk or the next bounded authority step.
            for q in proof.positions.into_iter().chain([p]) {
                self.handled.insert(q);
                if let Some(section) = proof.waiting.filter(|_| !proof.deferred) {
                    journal.sleep_fluid(q, section);
                } else {
                    journal.wake_fluid(q);
                }
                if sampler.cell(journal, q).is_some_and(|cell| cell.amount > 0) {
                    step.wetted.push(q);
                }
            }
            return Ok(true);
        }
        self.checked.extend(proof.positions.iter().copied());
        if !proof.ready {
            return Ok(false);
        }
        // A complete constructed rim fills only to its lowest solid crest.
        // This keeps a full bucket inside a lower-slab ring rather than putting
        // a hovering water sheet on top of the slabs.
        let bank_capacity = if !proof.walls.is_empty()
            && proof.walls.iter().all(|(q, _, _)| {
                sampler
                    .chunks
                    .block_at(*q)
                    .is_some_and(terra_core::construction::is_shape)
            }) {
            proof
                .walls
                .iter()
                .map(|(_, _, wall)| {
                    (wall
                        .solids
                        .iter()
                        .map(|b| b.maximum[1])
                        .fold(0.0_f64, f64::max)
                        .min(1.)
                        * 8.)
                        .round() as u8
                })
                .min()
                .unwrap_or(FLUID_CAPACITY)
                .max(1)
        } else {
            FLUID_CAPACITY
        };
        let positions = proof.positions;
        let old: Vec<_> = positions
            .iter()
            .map(|&p| sampler.cell(journal, p).unwrap())
            .collect();
        if old
            .iter()
            .any(|cell| cell.source && cell.amount < bank_capacity)
        {
            return Ok(false);
        }
        let origin = [
            positions.iter().map(|p| p[0]).min().unwrap(),
            p[1],
            positions.iter().map(|p| p[2]).min().unwrap(),
        ];
        let two_by_two = positions.as_slice() == cells(origin).as_slice();
        let promote =
            two_by_two && ((old[0].source && old[3].source) || (old[1].source && old[2].source));
        let total: u16 = old.iter().map(|cell| u16::from(cell.amount)).sum();
        let supplied = old.iter().any(|cell| cell.source);
        let count = positions.len() as u16;
        let next: Vec<_> = old
            .iter()
            .enumerate()
            .map(|(index, old)| FluidCell {
                amount: if supplied {
                    bank_capacity
                } else {
                    (total / count + u16::from(index < (total % count) as usize)) as u8
                },
                source: promote || old.source,
                flow: [0; 2],
            })
            .collect();
        let updates: Vec<_> = positions
            .iter()
            .copied()
            .zip(&old)
            .zip(&next)
            .filter_map(|((p, old), next)| (old != next).then_some((p, *next)))
            .collect();
        preflight(journal, &updates)?;
        for &(p, cell) in &updates {
            step.changed |= journal.set_simulated_fluid(p, cell)?;
        }
        let after = next.iter().map(|cell| u64::from(cell.amount)).sum::<u64>();
        let added = after.saturating_sub(u64::from(total));
        step.drained += u64::from(total).saturating_sub(after);
        step.sourced += added;
        step.transferred += added;
        self.handled.extend(positions.iter().copied());
        step.wetted.extend(
            positions
                .into_iter()
                .zip(next)
                .filter_map(|(p, cell)| (cell.amount > 0).then_some(p)),
        );
        Ok(true)
    }
}

fn preflight(journal: &EditJournal, updates: &[(Cell, FluidCell)]) -> Result<(), FluidError> {
    let mut fluid = BTreeMap::<_, u64>::new();
    let mut meshes = BTreeMap::<_, u64>::new();
    for &(p, _) in updates {
        let (section, local) = global_to_section(p);
        *fluid.entry(section).or_default() += 1;
        let offsets = local.map(|v| {
            if v == 0 {
                -1
            } else if v == 15 {
                1
            } else {
                0
            }
        });
        let mut affected = BTreeSet::from([section]);
        for x in [0, offsets[0]] {
            for y in [0, offsets[1]] {
                for z in [0, offsets[2]] {
                    affected.insert(section.offset([x, y, z])?);
                }
            }
        }
        for section in affected {
            *meshes.entry(section).or_default() += 1;
        }
    }
    if fluid
        .iter()
        .any(|(&section, &count)| journal.fluid_epoch(section).checked_add(count).is_none())
        || meshes.iter().any(|(&section, &count)| {
            journal
                .water_mesh_epoch(section)
                .checked_add(count)
                .is_none()
        })
    {
        return Err(terra_voxel::blocks::BlockError::RevisionOverflow.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
