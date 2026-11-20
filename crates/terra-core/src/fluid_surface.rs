//! Volume-preserving leading wedge shared by realtime liquid surfaces.
#[derive(Clone, Copy, Debug)]
pub struct Front {
    pub length: f32,
    pub head: f32,
    pub tip: f32,
    pub axis: usize,
    pub positive: bool,
}
impl Front {
    pub fn new(volume: f32, head: f32, axis: usize, positive: bool) -> Option<Self> {
        if !volume.is_finite()
            || !head.is_finite()
            || volume <= 0.
            || volume >= 1.
            || head <= volume + 0.0001
            || axis > 1
        {
            return None;
        }
        let head = head.min(1.);
        let length = (2. * volume / head).min(1.);
        Some(Self {
            length,
            head,
            tip: (2. * volume - head).max(0.),
            axis,
            positive,
        })
    }
    pub fn deform(self, mut p: [f32; 3], base: [f32; 3], old_height: f32) -> [f32; 3] {
        let axis = if self.axis == 0 { 0 } else { 2 };
        let t = (p[axis] - base[axis]).clamp(0., 1.);
        let from_back = if self.positive { t } else { 1. - t };
        p[axis] = base[axis]
            + if self.positive {
                from_back * self.length
            } else {
                1. - from_back * self.length
            };
        p[1] = base[1]
            + (self.head + (self.tip - self.head) * from_back) * (p[1] - base[1])
                / old_height.max(0.0001);
        p
    }
    pub fn height_at(self, coordinate: f32) -> Option<f32> {
        let d = if self.positive {
            coordinate
        } else {
            1. - coordinate
        };
        (d <= self.length).then(|| self.head + (self.tip - self.head) * d / self.length)
    }
    pub fn normal(self) -> [f32; 3] {
        let slope = (self.head - self.tip) / self.length * if self.positive { 1. } else { -1. };
        let mut n = [0., 1., 0.];
        n[if self.axis == 0 { 0 } else { 2 }] = slope;
        let l = (1. + slope * slope).sqrt();
        n.map(|v| v / l)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_front_pays_for_its_fractional_footprint_and_fills_continuously() {
        for units in 1..64 {
            let v = units as f32 / 64.;
            let f = Front::new(v, 1., 0, true).unwrap();
            assert!(((f.head + f.tip) * 0.5 * f.length - v).abs() < 0.00001);
            assert!(f.length > 0. && f.length <= 1.);
            if units < 32 {
                assert!(f.height_at(f.length + 0.01).is_none());
            }
        }
        assert!(Front::new(1., 1., 0, true).is_none());
    }
}
