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
        let bed = (columns[x + SIZE * z].height - 0.5).floor() + 1.0;
        let block = padded[x + SIZE * (y + SIZE * z)];
        super::can_hold_fluid(block) && !super::space::partial(block) && bed < level
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
    assign_wave_weights(&mut patch, origin, columns, None);
    (!patch.indices.is_empty()).then_some(patch)
}

pub(super) fn assign_wave_weights(
    patch: &mut WaterPatch,
    origin: [i64; 3],
    columns: &[SurfaceColumn],
    fluids: Option<&super::FluidSnapshot>,
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
    for z in 0..=16 {
        for x in 0..=16 {
            corners[x + z * 17] = super::channels::shared_wave_weight(columns, x, z);
        }
    }
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
            (-1..=1).any(|dy| {
                (-1..=0).any(|dz| {
                    (-1..=0).any(|dx| fluids.contains_key(&[global_x + dx, y + dy, global_z + dz]))
                })
            })
        });
        if !edited {
            patch.wave_weights[i] = corners[x as usize + z as usize * 17];
        }
    }
}
