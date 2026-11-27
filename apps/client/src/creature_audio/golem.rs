use terra_core::creatures::{CreatureBehavior as B, definitions::MobCombat, golem};

pub(super) const FIRST: usize = super::savanna::FIRST + super::savanna::CLIPS.len();
pub(super) const CLIPS: [&str; 7] = [
    "golem/wake.wav",
    "golem/hurt.wav",
    "golem/punch.wav",
    "golem/slam.wav",
    "golem/fall.wav",
    "golem/step-left.wav",
    "golem/step-right.wav",
];

pub(super) fn foot(alternate: bool) -> usize {
    FIRST + 5 + usize::from(alternate)
}

#[derive(Default)]
pub(super) struct Cadence {
    last: Option<(B, f32, f64)>,
}

impl Cadence {
    pub(super) fn action(&mut self, behavior: B, timer: f32, age: f64) -> Option<(usize, f32)> {
        let phase = golem::phase(behavior, timer);
        let (before, prior, seen) = self.last.replace((behavior, phase, age))?;
        if age < seen || age - seen > 1.5 {
            return None;
        }
        let (contact, sound, level) = match behavior {
            B::Emerge => (0.08, 0, 0.47),
            B::Shake => (0.04, 1, 0.30),
            B::Attack => (MobCombat::MELEE_IMPACT_PHASE, 2, 0.57),
            B::Tremor => (golem::SLAM_IMPACT, 3, 0.71),
            B::Dead => (0., 4, 0.56),
            _ => return None,
        };
        (phase >= contact && (before != behavior || phase + 0.15 < prior || prior < contact))
            .then_some((FIRST + sound, level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replayed_guardian_snapshots_give_one_contact_after_the_windup() {
        let mut c = Cadence::default();
        assert_eq!(c.action(B::Sleep, 9., 0.), None);
        assert_eq!(
            c.action(B::Emerge, golem::WAKE_SECONDS * 0.84, 0.4),
            Some((FIRST, 0.47))
        );
        assert_eq!(c.action(B::Emerge, golem::WAKE_SECONDS * 0.8, 0.5), None);
        assert_eq!(c.action(B::Tremor, golem::SLAM_SECONDS * 0.6, 1.0), None);
        assert_eq!(
            c.action(B::Tremor, golem::SLAM_SECONDS * 0.40, 1.40),
            Some((FIRST + 3, 0.71))
        );
        assert_eq!(c.action(B::Tremor, golem::SLAM_SECONDS * 0.39, 1.42), None);
        assert_eq!(c.action(B::Dead, 0., 1.7), Some((FIRST + 4, 0.56)));
        assert_eq!(c.action(B::Dead, 0., 1.8), None);
        assert_ne!(foot(false), foot(true));
    }
}
