//! Stable depth courses through the actual loaded, edited water column. ID and
//! simulation age already persist, so a reload needs no new goal/save fields.
use super::*;

pub(in crate::creatures) fn goal(a: &Creature, h: &Habitat) -> Option<f64> {
    let water = h
        .chunks
        .water_column_at_point(h.terrain, h.journal, center(a))?;
    let low = water.bed + 0.20;
    let high = water.level - a.size()[1] - 0.15;
    if high < low {
        return None;
    }
    // Hold a depth preference for 32 seconds: unlike a small sine offset about
    // the current Y, this creates sustained dives through the water column.
    let seed = random(a.id, (a.age / 32.0) as u64 ^ 0x6465_7074);
    let fraction = 0.12 + (seed % 1001) as f64 / 1000.0 * 0.62;
    Some(low + (high - low) * fraction)
}

pub(super) fn course(a: &Creature, h: &Habitat, mut target: [f64; 3]) -> [f64; 3] {
    if let Some(y) = goal(a, h) {
        // A bounded vertical component still leaves enough horizontal motion
        // to round obstacles. Deeper loaded regions are reached gradually.
        target[1] = a.position[1] + (y - a.position[1]).clamp(-6.0, 6.0);
    }
    target
}
