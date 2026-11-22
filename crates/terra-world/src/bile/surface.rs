//! A read-only halo of a poured section. The same free-surface mesher builds
//! water and acid, including the shared corners, advancing toe and falling jet.
use super::*;
use crate::water::{
    FluidCell, FluidOccluders, FluidSnapshot, WaterPatch, append_fluid_patch,
    front::inlet_width,
    tongue::{self, Tongue},
};
use std::collections::HashMap;
use terra_voxel::{BlockSection, SectionPos};

#[derive(Clone)]
pub struct BileSurface {
    section: SectionPos,
    padded: Vec<u16>,
    cells: FluidSnapshot,
    levels: HashMap<[i64; 3], f64>,
}

fn volume(journal: &EditJournal, p: [i64; 3]) -> u8 {
    journal.object_state(p).map_or(8, |state| {
        state.liquid.map_or((state.facing.min(7) + 1) * 8, |s| {
            bile::fine_volume(s.volume, s.age)
        })
    })
}

fn incoming(p: [i64; 3], own: u8, mut sample: impl FnMut([i64; 3]) -> u8) -> [i8; 2] {
    let mut best = own;
    let mut flow = [0; 2];
    for d in SIDES {
        let head = sample(std::array::from_fn(|i| p[i] + d[i]));
        if head > best {
            best = head;
            flow = [-d[0] as i8, -d[2] as i8];
        }
    }
    flow
}

/// Contact height of the same advancing tongue and falling inlet drawn by the mesher.
pub(crate) fn height_at(
    chunks: &ChunkStore,
    journal: &EditJournal,
    point: [f64; 3],
) -> Option<f64> {
    let p = point.map(|v| v.floor() as i64);
    let wet = |q| chunks.block_at(q) == Some(bile::FLUID);
    if !wet(p) {
        return None;
    }
    let filled = |q| if wet(q) { volume(journal, q) } else { 0 };
    let occupied = |q| {
        chunks
            .block_at(q)
            .map(|id| if id == bile::FLUID { 0 } else { id })
    };
    let amount = filled(p);
    let height = f64::from(amount) / f64::from(bile::CAPACITY);
    let xz = [0, 2].map(|i| (point[i] - p[i] as f64) as f32);
    if wet([p[0], p[1] + 1, p[2]]) && amount < bile::CAPACITY {
        let width = |q| {
            inlet_width(
                q,
                FluidCell {
                    amount: filled(q).div_ceil(8),
                    ..Default::default()
                },
                wet,
            )
        };
        let top = width(p);
        let down = [p[0], p[1] - 1, p[2]];
        let bottom = if wet(down) { width(down) } else { top };
        let radius = xz
            .into_iter()
            .map(|v| 2. * f64::from((v - 0.5).abs()))
            .fold(0., f64::max);
        let y = point[1] - p[1] as f64;
        if radius > bottom + (top - bottom) * y {
            return None;
        }
        return Some(
            p[1] as f64
                + if radius > top {
                    (bottom - radius) / (bottom - top)
                } else {
                    1.
                },
        );
    }
    let flow = incoming(p, amount, filled);
    if flow != [0; 2] {
        let axis = usize::from(flow[0] == 0);
        let mut behind = p;
        let mut ahead = p;
        behind[axis * 2] -= i64::from(flow[axis]);
        ahead[axis * 2] += i64::from(flow[axis]);
        if !wet(ahead)
            && occupied(ahead).is_some_and(tongue::front_boundary)
            && !tongue::wet_flank(p, axis, wet)
            && let Some(front) = terra_core::fluid_surface::Front::new(
                height as f32,
                f32::from(filled(behind)) / 64.,
                axis,
                flow[axis] > 0,
            )
        {
            let sampled = if let Some(shape) = Tongue::on_bed(front, p, occupied, wet) {
                shape.height_at(xz)
            } else {
                front.height_at(xz[axis])
            };
            return sampled.map(|h| p[1] as f64 + f64::from(h));
        }
    }
    Some(p[1] as f64 + height)
}

impl BileSurface {
    pub fn capture(
        section: &BlockSection,
        journal: &EditJournal,
        mut neighbor: impl FnMut([i64; 3]) -> Option<u16>,
    ) -> Option<Self> {
        if !section.may_contain(bile::FLUID) || !section.blocks().contains(&bile::FLUID) {
            return None;
        }
        let origin = section.position().origin().ok()?;
        let mut block = |p: [i64; 3]| {
            let local = std::array::from_fn::<_, 3, _>(|i| p[i] - origin[i]);
            if local.iter().all(|i| (0..16).contains(i)) {
                let [x, y, z] = local.map(|i| i as usize);
                Some(section.blocks()[x + 16 * (y + 16 * z)])
            } else {
                neighbor(p)
            }
        };
        let mut padded = Vec::with_capacity(18 * 18 * 18);
        let mut cells = FluidSnapshot::new();
        let mut levels = HashMap::new();
        for z in -1..=16 {
            for y in -1..=16 {
                for x in -1..=16 {
                    let p = [origin[0] + x, origin[1] + y, origin[2] + z];
                    let material = block(p);
                    padded.push(if matches!(material, Some(0 | bile::FLUID)) {
                        0
                    } else {
                        3
                    });
                    if material != Some(bile::FLUID) {
                        continue;
                    }
                    let amount = volume(journal, p);
                    let source = journal
                        .object_state(p)
                        .and_then(|s| s.liquid)
                        .is_some_and(|s| s.source);
                    let flow = incoming(p, amount, |q| {
                        if block(q) == Some(bile::FLUID) {
                            volume(journal, q)
                        } else {
                            0
                        }
                    });
                    cells.insert(
                        p,
                        FluidCell {
                            amount: amount.div_ceil(8),
                            source,
                            flow,
                        },
                    );
                    levels.insert(
                        p,
                        p[1] as f64 + f64::from(amount) / f64::from(bile::CAPACITY),
                    );
                }
            }
        }
        Some(Self {
            section: section.position(),
            padded,
            cells,
            levels,
        })
    }

    pub fn mesh(&self) -> Option<WaterPatch> {
        let mut patch = None;
        append_fluid_patch(
            &mut patch,
            self.section,
            &[],
            &self.padded,
            &self.cells,
            (false, &FluidOccluders::default(), &self.levels),
        );
        patch
    }
}
