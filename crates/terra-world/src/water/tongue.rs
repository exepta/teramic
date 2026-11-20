//! Footprint of an advancing pour on a solid bed. Rendering and immersion
//! sample the same triangulated surface, including the dry corners at the toe.
use super::can_hold_fluid;
use terra_core::fluid_surface::Front;

/// A film can advance across its last dry cell right up to a solid bank.
/// Waterloggable shapes and open fixtures need their clipped surface instead.
pub(super) fn front_boundary(material: u16) -> bool {
    matches!(material, 0 | 27)
        || (terra_voxel::MaterialId::from_raw(material).is_some_and(|id| id.is_solid())
            && !terra_core::plumbing::is_part(material)
            && material != terra_core::brewing::MATERIAL
            && !can_hold_fluid(material))
}

pub(super) const BANDS: usize = 4;

pub(super) struct Tongue {
    pub front: Front,
    pub exposed: [bool; 2],
    pub tip_exposed: bool,
    bow: f32,
}

impl Tongue {
    pub fn on_bed(
        front: Front,
        p: [i64; 3],
        block: impl Fn([i64; 3]) -> Option<u16>,
        wet: impl Fn([i64; 3]) -> bool,
    ) -> Option<Self> {
        if can_hold_fluid(block([p[0], p[1] - 1, p[2]])?) || !matches!(block(p)?, 0 | 27) {
            return None;
        }
        let mut exposed = [false; 2];
        for (i, side) in exposed.iter_mut().enumerate() {
            let mut q = p;
            q[if front.axis == 0 { 2 } else { 0 }] += if i == 0 { -1 } else { 1 };
            let m = block(q)?;
            // A wet neighbour owns a shared edge. A waterlogged construction
            // has its own clipped mesh. Preserve those joins.
            if wet(q) || (can_hold_fluid(m) && !matches!(m, 0 | 27)) {
                return None;
            }
            *side = can_hold_fluid(m);
        }
        // These five lateral samples have zero trapezoidal mean. The rounded
        // nose redistributes the available water without adding volume.
        let bow = if exposed == [true; 2] {
            0.4 * front.length.min(1. - front.length)
        } else {
            0.
        };
        let mut ahead = p;
        ahead[front.axis * 2] += if front.positive { 1 } else { -1 };
        Some(Self {
            front,
            exposed,
            tip_exposed: matches!(block(ahead)?, 0 | 27),
            bow,
        })
    }
    fn reach(&self, band: usize) -> f32 {
        let s = band as f32 / BANDS as f32;
        self.front.length + self.bow * (8. * s * (1. - s) - 1.25)
    }
    fn height(&self, row: usize) -> f32 {
        let t = row as f32 / BANDS as f32;
        self.front.head + (self.front.tip - self.front.head) * t * t * (3. - 2. * t)
    }
    pub fn point(&self, row: usize, band: usize) -> [f32; 3] {
        let t = row as f32 / BANDS as f32;
        let along = t * self.reach(band);
        let along = if self.front.positive {
            along
        } else {
            1. - along
        };
        let across = band as f32 / BANDS as f32;
        let h = self.height(row);
        if self.front.axis == 0 {
            [along, h, across]
        } else {
            [across, h, along]
        }
    }
    pub fn normal(&self, row: usize, band: usize) -> [f32; 3] {
        let t = row as f32 / BANDS as f32;
        let dh = (self.front.tip - self.front.head) * 6. * t * (1. - t) / self.reach(band);
        let slope = if self.front.positive { dh } else { -dh };
        let side = band as f32 / BANDS as f32;
        let lateral = dh * t * self.bow * (8. - 16. * side);
        let n = if self.front.axis == 0 {
            [-slope, 1., lateral]
        } else {
            [lateral, 1., -slope]
        };
        let len = (1. + slope * slope + lateral * lateral).sqrt();
        n.map(|v| v / len)
    }
    pub fn height_at(&self, xz: [f32; 2]) -> Option<f32> {
        let along = xz[self.front.axis];
        let d = if self.front.positive {
            along
        } else {
            1. - along
        };
        let s = xz[1 - self.front.axis].clamp(0., 1.) * BANDS as f32;
        let col = (s as usize).min(BANDS - 1);
        let u = s - col as f32;
        let (l, r) = (self.reach(col), self.reach(col + 1));
        let reach = l + u * (r - l);
        if d > reach || d < 0. {
            return None;
        }
        let row = ((d / reach * BANDS as f32) as usize).min(BANDS - 1);
        let low = row as f32 * reach;
        let offset = d * BANDS as f32 - low;
        // The diagonal joins (row,col) to (row+1,col+1). Both triangles
        // interpolate height linearly, with a different longitudinal span.
        let blend = if offset <= u * r {
            offset / r
        } else {
            1. - (reach - offset) / l
        };
        Some(self.height(row) + blend * (self.height(row + 1) - self.height(row)))
    }
}
