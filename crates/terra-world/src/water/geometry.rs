use crate::SurfaceColumn;
use terra_voxel::{SECTION_SIZE, SectionPos};

#[derive(Debug, Clone, PartialEq)]
pub struct WaterPatch {
    pub normals: Vec<[f32; 3]>,
    pub flow: Vec<[f32; 2]>,
    pub positions: Vec<[f32; 3]>,
    /// Bed depth and shoreline coverage, shared at tile boundaries.
    pub depth_shore: Vec<[f32; 2]>,
    /// Crest distance, impact distance, fall height, effect mask (0 normal, 1 curtain,
    /// 2 impact, 3 crest). For horizontal effects the first channel is edge distance.
    pub surface_effects: Vec<[f32; 4]>,
    /// Shared corner displacement weights; waterfall attachments and edited
    /// fluid boundaries stay pinned while neighbouring surface waves taper in.
    pub wave_weights: Vec<f32>,
    pub indices: Vec<u32>,
}
impl WaterPatch {
    pub(crate) fn append(&mut self, mut other: Self) {
        let base = self.positions.len() as u32;
        self.positions.append(&mut other.positions);
        self.normals.append(&mut other.normals);
        self.flow.append(&mut other.flow);
        self.depth_shore.append(&mut other.depth_shore);
        self.surface_effects.append(&mut other.surface_effects);
        self.wave_weights.append(&mut other.wave_weights);
        self.indices
            .extend(other.indices.into_iter().map(|i| i + base));
    }
}

pub(crate) fn build_water_patch(
    position: SectionPos,
    origin: [i64; 3],
    level: f64,
    columns: &[SurfaceColumn],
    padded: &[u16],
) -> Option<WaterPatch> {
    let plane = level.floor() as i64;
    if plane.div_euclid(SECTION_SIZE as i64) != position.y {
        return None;
    }
    const SIZE: usize = SECTION_SIZE + 2;
    let y = (plane - origin[1]) as usize + 1;
    let wet = |x: usize, z: usize| {
        let c = columns[x + SIZE * z];
        if c.water_level != Some(level) {
            return false;
        }
        let bed = c.water_bed();
        let block = padded[x + SIZE * (y + SIZE * z)];
        let volume = if (level - plane as f64).abs() < 1e-8 && y > 0 {
            padded[x + SIZE * (y - 1 + SIZE * z)]
        } else {
            block
        };
        super::can_hold_fluid(block)
            && !super::space::partial(block)
            && super::can_hold_fluid(volume)
            && !super::space::partial(volume)
            && bed < level
    };
    let mut patch = WaterPatch {
        normals: Vec::new(),
        flow: Vec::new(),
        positions: Vec::with_capacity(17 * 17),
        depth_shore: Vec::with_capacity(17 * 17),
        surface_effects: Vec::new(),
        wave_weights: Vec::new(),
        indices: Vec::new(),
    };
    for z in 0..=SECTION_SIZE {
        for x in 0..=SECTION_SIZE {
            let mut count = 0;
            let mut depth = 0.0;
            for dz in 0..=1 {
                for dx in 0..=1 {
                    if wet(x + dx, z + dz) {
                        count += 1;
                        let bed = (columns[x + dx + SIZE * (z + dz)].height - 0.5).floor() + 1.0;
                        depth += level - bed;
                    }
                }
            }
            patch
                .positions
                .push([x as f32, (level - origin[1] as f64) as f32, z as f32]);
            patch.depth_shore.push([
                if count == 0 {
                    0.0
                } else {
                    (depth / f64::from(count)) as f32
                },
                1.0 - count as f32 * 0.25,
            ]);
        }
    }
    for z in 0..SECTION_SIZE {
        for x in 0..SECTION_SIZE {
            if wet(x + 1, z + 1) {
                let a = (x + 17 * z) as u32;
                patch
                    .indices
                    .extend([a, a + 17, a + 1, a + 1, a + 17, a + 18]);
            }
        }
    }
    assign_wave_weights(&mut patch, origin, columns, None, padded);
    (!patch.indices.is_empty()).then_some(patch)
}

pub(super) fn assign_wave_weights(
    patch: &mut WaterPatch,
    origin: [i64; 3],
    columns: &[SurfaceColumn],
    fluids: Option<&super::FluidSnapshot>,
    padded: &[u16],
) {
    let surface_effects = &patch.surface_effects;
    let normals = &patch.normals;
    let horizontal = |i: usize| {
        let mask = surface_effects.get(i).map_or(0.0, |v| v[3]);
        let up = normals.get(i).map_or(1.0, |n| n[1]);
        up > 0.001 && (mask < 0.5 || (1.5..3.5).contains(&mask))
    };
    patch.wave_weights = vec![0.0; patch.positions.len()];
    if !(0..patch.positions.len()).any(horizontal) {
        return;
    }
    let mut corners = [0.0; 17 * 17];
    let mut corner_flow = [[0.; 2]; 17 * 17];
    for z in 0..=16 {
        for x in 0..=16 {
            corners[x + z * 17] = super::channels::shared_wave_weight(columns, x, z);
            // The vertex shader also derives swell amplitude from flow. Every
            // owner of this corner must supply the same four-column value;
            // a per-cell flow would split moving and still surfaces despite
            // identical displacement weights and world-space wave phase.
            for column in [
                columns[x + 18 * z],
                columns[x + 1 + 18 * z],
                columns[x + 18 * (z + 1)],
                columns[x + 1 + 18 * (z + 1)],
            ] {
                for (shared, local) in corner_flow[x + z * 17].iter_mut().zip(column.water_flow) {
                    *shared += local * 0.25;
                }
            }
        }
    }
    patch.flow.resize(patch.positions.len(), [0.; 2]);
    for i in 0..patch.positions.len() {
        if !horizontal(i) {
            continue;
        }
        let p = patch.positions[i];
        let x = p[0].round();
        let z = p[2].round();
        if !(0.0..=16.0).contains(&x)
            || !(0.0..=16.0).contains(&z)
            || (x - p[0]).abs() > 0.001
            || (z - p[2]).abs() > 0.001
        {
            continue;
        }
        let global_x = origin[0] + x as i64;
        let global_z = origin[2] + z as i64;
        let y = origin[1] + (f64::from(p[1]) - 0.001).floor() as i64;
        let edited = fluids.is_some_and(|fluids| {
            (0..=1).any(|dy| {
                (-1..=0).any(|dz| {
                    (-1..=0).any(|dx| fluids.contains_key(&[global_x + dx, y + dy, global_z + dz]))
                })
            })
        });
        // Live terrain/pipe edits may differ from the immutable height field.
        // Every owner samples the same padded four cells, including negative
        // section borders. Pin the contact rather than lowering the whole lake.
        let touches_solid = (0..=1).any(|dy| {
            (-1..=0).any(|dz| {
                (-1..=0).any(|dx| {
                    let local = [x as i64 + dx + 1, y - origin[1] + dy + 1, z as i64 + dz + 1];
                    if !local.iter().all(|v| (0..18).contains(v)) {
                        return false;
                    }
                    let [lx, ly, lz] = local.map(|v| v as usize);
                    let material = padded[lx + 18 * (ly + 18 * lz)];
                    !super::can_hold_fluid(material) || super::space::partial(material)
                })
            })
        });
        if !edited && !touches_solid {
            patch.wave_weights[i] = corners[x as usize + z as usize * 17];
            if patch.wave_weights[i] > 0. {
                patch.flow[i] = corner_flow[x as usize + z as usize * 17];
            }
        }
    }
}
