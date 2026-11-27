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
    let leg = (a.age / 42.0) as u64;
    let seed = random(a.id, leg ^ 0x6465_7074);
    let choice = (seed % 1001) as f64 / 1000.;
    let above_bed = if (leg + a.id % 3).is_multiple_of(3) {
        (high - low) * (0.28 + choice * 0.42)
    } else {
        // Feed among the fronds even in deep ocean, where a fraction of the
        // full column can be many metres above the tallest seagrass.
        0.12 + choice * (1.1 + a.size()[0] * 0.35)
    };
    Some(low + above_bed.min(high - low))
}

pub(super) fn course(a: &Creature, h: &Habitat, mut target: [f64; 3]) -> [f64; 3] {
    if let Some(y) = goal(a, h) {
        // A bounded vertical component still leaves enough horizontal motion
        // to round obstacles. Deeper loaded regions are reached gradually.
        target[1] = a.position[1] + (y - a.position[1]).clamp(-6.0, 6.0);
    }
    target
}
