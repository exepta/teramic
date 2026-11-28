//! Compile discharge paths once per streamed terrain column. Vertical
//! section workers share exact endpoints independently of simulation interest.
use super::*;
use crate::water::{can_hold_fluid, falls::Fall};
use std::collections::HashMap;
use terra_voxel::MaterialId as M;

impl SectionGenerator {
    pub(super) fn prepare_falls(
        &self,
        start: [i64; 3],
        size: usize,
        columns: &mut [SurfaceColumn],
        sites: &[Arc<super::super::structures::Site>],
        features: &[Arc<super::super::natural_features::NaturalPatch>],
        trees: &[Tree],
    ) {
        if !self.terrain().config().water {
            return;
        }
        let world_floor = if self.terrain().config().bounded_world {
            -128
        } else {
            -MAX_SAMPLE_COORDINATE
        };
        let idx = |x: i64, z: i64| {
            let lx = x - start[0];
            let lz = z - start[2];
            ((0..size as i64).contains(&lx) && (0..size as i64).contains(&lz))
                .then(|| lx as usize + size * lz as usize)
        };
        let mut border = HashMap::new();
        let sample = |x: i64, z: i64| {
            let mut c = self
                .terrain()
                .surface_column(x as f64 + 0.5, z as f64 + 0.5);
            for site in sites.iter().filter(|s| s.summary.kind.dragon().is_some()) {
                site.clip_water(&mut c, x, z);
            }
            c
        };
        let material = |p: [i64; 3], c: SurfaceColumn| {
            sites
                .iter()
                .find_map(|s| s.block_at(p))
                .or_else(|| features.iter().find_map(|f| f.block_at(p)))
                .unwrap_or_else(|| {
                    let raw = self.terrain_block(p, c);
                    if raw != 0 {
                        return raw;
                    }
                    trees
                        .iter()
                        .find_map(|t| t.wood_at(p))
                        .or_else(|| trees.iter().find_map(|t| t.leaves_at(p)))
                        .map_or(0, |m| m as u16)
                })
        };
        for i in 0..columns.len() {
            let x = start[0] + (i % size) as i64;
            let z = start[2] + (i / size) as i64;
            let c = columns[i];
            let own = c.water_columns();
            let mut spans = [None; 3];
            let mut neighbor_at = |[dx, dz]: [i64; 2]| {
                let (nx, nz) = (x + dx, z + dz);
                let c = idx(nx, nz)
                    .map(|j| &columns[j])
                    .unwrap_or_else(|| border.entry([nx, nz]).or_insert_with(|| sample(nx, nz)));
                (c.water_level.is_some() || c.cave.water.is_some()).then(|| *c)
            };
            for (dx, dz) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x + dx;
                let nz = z + dz;
                let Some(n) = neighbor_at([dx, dz]) else {
                    continue;
                };
                for (layer, source) in n.water_columns().intervals[..2].iter().enumerate() {
                    let Some(w) = source else { continue };
                    let under = dx == 0 && dz == 0;
                    let bed = w.bed.ceil() as i64;
                    let crest = w.level.ceil() as i64 - 1;
                    let lower = own.intervals[layer];
                    if !under && lower.is_some_and(|l| l.bed <= w.bed && l.level >= w.level) {
                        continue;
                    }
                    let floor = if layer == 0 {
                        c.water_bed()
                    } else {
                        c.cave.floor
                    };
                    let bank = layer == 0
                        && n.water_flow != [0.; 2]
                        && c.water_flow == [0.; 2]
                        && lower.is_none();
                    let lip = !bank
                        && w.level - lower.map_or(floor, |l| l.level) >= 1.
                        && !(layer == 0
                            && lower.is_some_and(|l| {
                                crate::water::outlets::shares_channel_edge(
                                    [dx, dz],
                                    l.level,
                                    w.level,
                                    |p| neighbor_at(p).and_then(|c| c.water_level),
                                )
                            }));
                    let open = |top: i64, head: i64| {
                        own.at(top).is_none()
                            && can_hold_fluid(material([x, top, z], c))
                            && can_hold_fluid(material([nx, head, nz], n))
                    };
                    let (top, head) = if under {
                        if !open(bed - 1, bed) {
                            continue;
                        }
                        (bed - 1, bed)
                    } else if lip && open(crest, crest) {
                        (crest, crest)
                    } else if layer == 0 {
                        // A flooded wall can break into a lower gallery, even when
                        // the actual hole lies several cells below a sealed crest.
                        let last = crest.min((c.height - 0.5).ceil() as i64 - 1);
                        let Some(y) = (bed..=last).rev().find(|&y| open(y, y)) else {
                            continue;
                        };
                        (y, y)
                    } else {
                        continue;
                    };
                    let mut bottom = top;
                    while bottom > world_floor
                        && own.at(bottom - 1).is_none()
                        && can_hold_fluid(material([x, bottom - 1, z], c))
                    {
                        bottom -= 1;
                    }
                    let f = Fall::new(bottom, top, head, [dx as i8, dz as i8]).fed_by(w.level);
                    if spans[layer].is_none_or(|s: Fall| (f.top, -f.bottom) > (s.top, -s.bottom)) {
                        spans[layer] = Some(f);
                    }
                }
            }
            columns[i].falls = spans;
        }
        for (p, f) in features.iter().flat_map(|f| &f.streams) {
            if let Some(i) = idx(p[0], p[2]) {
                columns[i].falls[2] = Some(*f);
            }
        }
        for &([x, z], span) in features.iter().flat_map(|f| &f.geothermal_spring) {
            if let Some(i) = idx(x, z) {
                columns[i].falls[2] = Some(span);
            }
        }
        for source in features
            .iter()
            .flat_map(|f| &f.objects)
            .chain(sites.iter().flat_map(|s| {
                s.sections
                    .iter()
                    .filter(|(p, _)| {
                        p.x >= start[0].div_euclid(16)
                            && p.x <= (start[0] + size as i64 - 1).div_euclid(16)
                            && p.z >= start[2].div_euclid(16)
                            && p.z <= (start[2] + size as i64 - 1).div_euclid(16)
                    })
                    .flat_map(|(_, s)| &s.objects)
            }))
            .filter(|o| o.material == M::FluidWater as u16)
        {
            let [x, y, z] = source.position;
            let Some(i) = idx(x, z) else { continue };
            let c = columns[i];
            if c.falls[2].is_some() {
                continue;
            }
            let mut bottom = y;
            while bottom > world_floor
                && c.water_columns().at(bottom - 1).is_none()
                && can_hold_fluid(material([x, bottom - 1, z], c))
            {
                bottom -= 1;
            }
            let mut span = Fall::new(bottom, y, y, [0, 0]);
            span.authored = true;
            columns[i].falls[2] = Some(span);
        }
    }
}
