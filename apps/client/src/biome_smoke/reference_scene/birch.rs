//! A real generated birch, with attributed shape extrema checked against the
//! ordinary voxel generator and again after streaming. No fixture edits.
use super::*;

pub(super) struct Specimen {
    origin: [i64; 3],
    pub(super) bounds: [[i64; 3]; 2],
    base_width: i64,
    waist_width: i64,
    crown_width: i64,
    checked_shape_extrema: usize,
}

impl Specimen {
    pub(super) fn report(&self) -> serde_json::Value {
        serde_json::json!({"origin":self.origin,"bounds":self.bounds,
            "base_width":self.base_width,"waist_width":self.waist_width,
            "crown_width":self.crown_width,"checked_shape_extrema":self.checked_shape_extrema})
    }
}

pub(super) fn candidate(
    terrain: &TerraGenerator,
    generator: &SectionGenerator,
    tile: [i64; 2],
) -> Option<Candidate> {
    let appearance = terrain.tree_appearance([tile[0] * 32, 0, tile[1] * 32]);
    let roots: Vec<_> = appearance.tree_roots().collect();
    for &(root, kind) in &roots {
        if kind != TreeAppearance::Birch
            || [root[0].div_euclid(32), root[2].div_euclid(32)] != tile
            || roots.iter().any(|(other, _)| {
                *other != root
                    && (other[0] - root[0]).pow(2) + (other[2] - root[2]).pow(2) < 14 * 14
            })
        {
            continue;
        }
        // The existing appearance snapshot queries prepared capsules/crowns,
        // avoiding thousands of repeated terrain-generation calls in a finder.
        let mut cells = Vec::new();
        let mut extrema = [[i64::MAX, i64::MIN]; 3];
        let mut lowest = [i64::MAX; 3];
        let mut highest = [i64::MIN; 3];
        let mut waist_leaves = false;
        for y in 0..=24 {
            for z in -8..=8 {
                for x in -8..=8 {
                    let point = [root[0] + x, root[1] + y, root[2] + z];
                    let material = if appearance.at(point, 10) == Some(TreeAppearance::Birch) {
                        10
                    } else if appearance.at(point, 9) == Some(TreeAppearance::Birch) {
                        9
                    } else {
                        continue;
                    };
                    waist_leaves |= material == 9 && (4..=6).contains(&y);
                    for axis in 0..3 {
                        lowest[axis] = lowest[axis].min(point[axis]);
                        highest[axis] = highest[axis].max(point[axis]);
                    }
                    let band = if y <= 2 {
                        0
                    } else if (4..=6).contains(&y) {
                        1
                    } else if y >= 9 {
                        2
                    } else {
                        3
                    };
                    if band < 3 {
                        extrema[band][0] = extrema[band][0].min(point[0]);
                        extrema[band][1] = extrema[band][1].max(point[0]);
                    }
                    cells.push((point, material, band));
                }
            }
        }
        if waist_leaves || extrema.iter().any(|e| e[0] > e[1]) {
            continue;
        }
        let widths = extrema.map(|e| e[1] - e[0] + 1);
        if widths[0] <= widths[1] || widths[2] < widths[1] * 3 {
            continue;
        }
        // Include each band's silhouette endpoints and samples throughout the
        // authored structure, plus an explicit root. These are all real cells.
        let mut probes = vec![(root, 10)];
        for (band, edges) in extrema.iter().enumerate() {
            for &edge in edges {
                if let Some((point, material, _)) =
                    cells.iter().find(|(p, _, b)| *b == band && p[0] == edge)
                {
                    probes.push((*point, *material));
                }
            }
        }
        probes.extend(
            cells
                .iter()
                .step_by((cells.len() / 40).max(1))
                .map(|(p, m, _)| (*p, *m)),
        );
        probes.sort_unstable();
        probes.dedup();
        if probes
            .iter()
            .any(|(p, m)| generator.block_at(*p).ok() != Some(*m))
        {
            continue;
        }
        let specimen = Specimen {
            origin: root,
            bounds: [lowest, highest],
            base_width: widths[0],
            waist_width: widths[1],
            crown_width: widths[2],
            checked_shape_extrema: 6,
        };
        let target = Vec3::new(
            root[0] as f32 + 0.5,
            (root[1] + highest[1]) as f32 * 0.5,
            root[2] as f32 + 0.5,
        );
        return Some((target, probes, None, Some(specimen)));
    }
    None
}
