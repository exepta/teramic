use terra_core::creatures::{CreatureBehavior as B, CreatureKind as Kind};

pub(super) const FIRST: usize = super::volcanic::FIRST + super::volcanic::CLIPS.len();
pub(super) const CLIPS: [&str; 4] = [
    "animals/elephant-trumpet.wav",
    "animals/elephant-contact.wav",
    "animals/zebra-bark.wav",
    "animals/zebra-bray.wav",
];

pub(super) fn call(
    kind: Kind,
    behavior: B,
    previous: B,
    id: u64,
    now: f32,
    next: &mut f32,
) -> Option<(usize, f32)> {
    if behavior == B::Sleep {
        *next = now + 4.;
        return None;
    }
    let startled = behavior == B::Flee && previous != B::Flee;
    if !startled && now < *next {
        return None;
    }
    let take = (id ^ u64::from(now.to_bits())).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    *next = now + 14. + (take >> 55) as f32 * 0.045;
    match kind {
        Kind::Elephant => Some((
            FIRST + usize::from(!startled && !take.is_multiple_of(4)),
            0.48,
        )),
        Kind::Zebra => Some((
            FIRST + 2 + usize::from(!startled && take.is_multiple_of(3)),
            0.37,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waking_and_alarm_use_only_the_species_own_calls() {
        for (kind, pair) in [(Kind::Elephant, FIRST), (Kind::Zebra, FIRST + 2)] {
            let mut next = 0.;
            assert!(call(kind, B::Sleep, B::Sleep, 43, 20., &mut next).is_none());
            let (sound, _) = call(kind, B::Flee, B::Rest, 43, 21., &mut next).unwrap();
            assert_eq!(sound, pair);
            assert!(call(kind, B::Flee, B::Flee, 43, 21.05, &mut next).is_none());
            let mut heard = [false; 2];
            for id in 0..12 {
                let mut due = 0.;
                let (index, _) = call(kind, B::Rest, B::Rest, id, 46., &mut due).unwrap();
                heard[index - pair] = true;
            }
            assert_eq!(heard, [true; 2]);
        }
    }
}
