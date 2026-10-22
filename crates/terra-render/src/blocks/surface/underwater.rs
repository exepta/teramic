use super::CubeSurface;

/// Four cached coral heightfields. All recesses stop inside the solid cube;
/// the caller preserves its perimeter and stitches every height discontinuity.
pub(super) fn depth(style: CubeSurface, x: usize, y: usize, variant: usize) -> Option<f32> {
    let (x, y) = if variant & 1 == 0 { (x, y) } else { (y, x) };
    let dx = x.abs_diff(if x <= 3 { 2 } else { 5 });
    let dy = y.abs_diff(if y <= 3 { 2 } else { 5 });
    Some(match style {
        CubeSurface::CoralRed => {
            if dx == 0 && dy == 0 {
                0.035
            } else if dx + dy == 1 {
                0.012
            } else {
                0.105
            }
        }
        CubeSurface::CoralBlue => {
            if dx == 0 && dy == 0 {
                0.12
            } else if dx.max(dy) == 1 {
                0.015
            } else {
                0.085
            }
        }
        CubeSurface::CoralYellow => {
            let lane = (y + variant / 2) % 3;
            if lane == 0 {
                0.105
            } else if lane == 1 && (2..=5).contains(&x) {
                0.012
            } else {
                0.048
            }
        }
        CubeSurface::CoralPurple => {
            let seed = super::hash(x / 2, y / 2, variant + 239);
            if dx == 0 && dy == 0 {
                0.025
            } else if (dx + dy + (seed % 3) as usize).is_multiple_of(3) {
                0.10
            } else {
                0.045
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tube_centres_are_hollow_but_coral_never_cuts_through_the_cube() {
        assert!(
            depth(CubeSurface::CoralBlue, 2, 2, 0).unwrap()
                > depth(CubeSurface::CoralBlue, 1, 2, 0).unwrap()
        );
        for style in [
            CubeSurface::CoralRed,
            CubeSurface::CoralBlue,
            CubeSurface::CoralYellow,
            CubeSurface::CoralPurple,
        ] {
            for variant in 0..4 {
                for x in 1..7 {
                    for y in 1..7 {
                        assert!((0.0..=0.12).contains(&depth(style, x, y, variant).unwrap()));
                    }
                }
            }
        }
        assert!(depth(CubeSurface::Earth, 2, 2, 0).is_none());
    }
}
