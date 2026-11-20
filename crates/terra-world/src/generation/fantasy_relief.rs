//! Revision-six landscape shapes, evaluated only at cached terrain field nodes.
//! Ridges and shelves belong to the height field, so they remain rooted bedrock.

#[derive(Clone, Copy)]
pub(super) struct Fields {
    crest: f64,
    teeth: f64,
    valley: f64,
    shelf: f64,
}

#[derive(Clone, Copy)]
pub(super) enum Style {
    Plains,
    Hills,
    Forest,
    Mountain,
    Alpine,
    Grove,
    Spirit,
    Desert,
}

fn smooth(start: f64, end: f64, value: f64) -> f64 {
    let t = ((value - start) / (end - start)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Fields {
    pub(super) fn new(axis: f64, serration: f64, drainage: f64, shelf: f64) -> Self {
        // One steep rock face and one longer shoulder give a ridge direction.
        // The crest eases to zero slope, including where its two sides meet.
        let flank = if axis < 0.0 { 0.32 } else { 0.13 };
        let crest = (1.0 - smooth(0.012, flank, axis.abs())).powf(1.25);
        let teeth = (serration * 0.5 + 0.5).clamp(0.0, 1.0);
        let valley = 1.0 - smooth(0.025, 0.22, drainage.abs());
        let shelf = (shelf * 0.5 + 0.5).clamp(0.0, 1.0);
        let level = shelf * 4.0;
        let shelf = (level.floor() + smooth(0.22, 0.78, level.fract())) * 0.25;
        Self {
            crest,
            teeth,
            valley,
            shelf,
        }
    }

    pub(super) fn shape(self, style: Style, original: f64, broad: f64, erosion: f64) -> f64 {
        let erosion = erosion.clamp(0.0, 1.0);
        let valley_floor = 0.18 + broad * 0.065 + self.shelf * 0.065;
        let serrated = self.crest * (0.50 + self.teeth.powi(2) * 0.40);
        let shoulder = self.shelf * (1.0 - self.crest * 0.65);
        let (target, valley_strength) = match style {
            Style::Plains => (
                original * 0.82 + (0.30 + self.shelf * 0.28 + broad * 0.06) * 0.18,
                0.20,
            ),
            Style::Hills => (
                original * 0.42 + 0.17 + shoulder * 0.31 + serrated * 0.23,
                0.44,
            ),
            Style::Forest => (
                original * 0.60 + 0.10 + shoulder * 0.26 + serrated * 0.16,
                0.40,
            ),
            Style::Mountain => (
                0.12 + serrated * (0.99 - erosion * 0.16) + shoulder * 0.24 + broad * 0.025,
                0.78,
            ),
            Style::Alpine => (
                0.20 + serrated * 0.82 + shoulder * 0.23 + original * 0.06,
                0.66,
            ),
            Style::Grove => (
                original * 0.42 + 0.15 + shoulder * 0.36 + serrated * 0.12,
                0.44,
            ),
            Style::Spirit => (
                0.17 + original * 0.18 + shoulder * 0.44 + serrated * 0.42,
                0.60,
            ),
            Style::Desert => (
                original * 0.63 + 0.10 + shoulder * 0.28 + serrated * 0.12,
                0.36,
            ),
        };
        // Broad coherent valleys leave walking and river corridors between the
        // tall folds; local broad/shelf fields prevent a constant-height floor.
        let strength = self.valley * valley_strength * (0.72 + erosion * 0.28);
        (target + (valley_floor - target) * strength).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ridges_have_unequal_flanks_serrated_summits_and_open_valley_gaps() {
        let mountain = |axis, teeth, drainage| {
            Fields::new(axis, teeth, drainage, 0.0).shape(Style::Mountain, 0.5, 0.0, 0.5)
        };
        assert!(mountain(-0.12, 0.8, 0.8) > mountain(0.12, 0.8, 0.8) + 0.20);
        assert!(mountain(0.0, 0.8, 0.8) > mountain(0.0, -0.8, 0.8) + 0.20);
        assert!(mountain(0.0, 0.8, 0.8) > mountain(0.0, 0.8, 0.0) + 0.35);
        assert!(mountain(0.0, 0.8, 0.8) > mountain(0.6, 0.8, 0.8) + 0.65);
    }

    #[test]
    fn all_landforms_are_bounded_continuous_and_valley_floors_keep_relief() {
        let styles = [
            Style::Plains,
            Style::Hills,
            Style::Forest,
            Style::Mountain,
            Style::Alpine,
            Style::Grove,
            Style::Spirit,
            Style::Desert,
        ];
        for style in styles {
            for i in -1000..=1000 {
                let x = f64::from(i) / 1000.0;
                let shape = |x: f64| {
                    Fields::new(x, x, x * 0.75, x).shape(style, 0.5 + x * 0.25, x * 0.5, 0.5)
                };
                let height = shape(x);
                assert!(height.is_finite() && (0.0..=1.0).contains(&height));
                assert!((height - shape(x + 0.000001)).abs() < 0.0001);
            }
            let valley = |broad| Fields::new(0.6, 0.0, 0.0, 0.0).shape(style, 0.5, broad, 0.5);
            assert!((valley(0.5) - valley(-0.5)).abs() > 0.005);
        }
    }
}
