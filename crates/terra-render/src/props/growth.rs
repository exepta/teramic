use std::collections::HashMap;
use terra_world::props::{PropId, WorldProp};

pub(super) const SECONDS: f32 = 2.4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Pose {
    pub scale: f32,
    pub ratio: f32,
    pub start: f32,
    pub pending: bool,
}

impl Pose {
    pub fn factor(self, time: f32) -> f32 {
        let t = ((time - self.start) / SECONDS).clamp(0.0, 1.0);
        self.ratio + (1.0 - self.ratio) * t * t * (3.0 - 2.0 * t)
    }

    pub fn begin(&mut self, previous: Option<Self>, time: f32) {
        if !self.pending {
            return;
        }
        if let Some(previous) = previous {
            self.ratio = (previous.scale * previous.factor(time) / self.scale).clamp(0.0, 1.0);
        }
        self.start = time;
        self.pending = false;
    }
}

pub(super) fn snapshot(
    props: &[WorldProp],
    previous: Option<&HashMap<PropId, Pose>>,
    time: f32,
) -> HashMap<PropId, Pose> {
    props
        .iter()
        .filter(|prop| !prop.removed && prop.growing_grass())
        .map(|prop| {
            let scale = prop.visual_scale();
            let pose = if let Some(old) = previous.and_then(|previous| previous.get(&prop.id)) {
                if old.scale == scale {
                    *old
                } else {
                    Pose {
                        scale,
                        ratio: (old.scale * old.factor(time) / scale).clamp(0.0, 1.0),
                        start: -1.0,
                        pending: true,
                    }
                }
            } else if previous.is_some() && !prop.hidden_growth() {
                Pose {
                    scale,
                    ratio: 0.0,
                    start: -1.0,
                    pending: true,
                }
            } else {
                // A first section upload may restore a saved mature sprout. Do not
                // replay growth whenever the player reloads or turns toward it.
                Pose {
                    scale,
                    ratio: 1.0,
                    start: 0.0,
                    pending: false,
                }
            };
            (prop.id, pose)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::SectionPos;
    use terra_world::props::PropKind;

    fn sprout(scale: f32, variant: u8) -> WorldProp {
        WorldProp {
            id: PropId(99),
            kind: PropKind::Grass,
            position: [-0.5, 16.0, -0.5],
            rotation: 0.0,
            scale,
            variant,
            section: SectionPos::new(-1, 0, -1),
            removed: false,
        }
    }

    #[test]
    fn saved_first_upload_is_mature_but_new_sprouts_grow_continuously_from_root() {
        let prop = sprout(0.12, 64);
        let restored = snapshot(&[prop], None, 4000.0)[&prop.id];
        assert_eq!(restored.factor(4000.0), 1.0);
        assert!(!restored.pending);
        let mut fresh = snapshot(&[prop], Some(&HashMap::new()), 4000.0)[&prop.id];
        assert!(fresh.pending);
        fresh.start = 4000.0;
        assert_eq!(fresh.factor(4000.0), 0.0);
        assert!((fresh.factor(4001.2) - 0.5).abs() < 0.001);
        assert_eq!(
            fresh.factor(7601.2),
            1.0,
            "one-hour wrap must not replay old growth"
        );
        for pair in (0..24).map(|step| [step as f32 * 0.1, (step + 1) as f32 * 0.1]) {
            assert!(fresh.factor(4000.0 + pair[0]) <= fresh.factor(4000.0 + pair[1]));
        }
    }

    #[test]
    fn scale_changes_are_continuous_and_lod_rebuilds_do_not_restart_them() {
        let first = sprout(0.25, 65);
        let old = Pose {
            scale: first.visual_scale(),
            ratio: 0.2,
            start: 20.0,
            pending: false,
        };
        let previous = HashMap::from([(first.id, old)]);
        assert_eq!(snapshot(&[first], Some(&previous), 21.0)[&first.id], old);
        let next = sprout(0.43, 66);
        let mut growing = snapshot(&[next], Some(&previous), 21.0)[&next.id];
        growing.start = 21.0;
        assert!((growing.scale * growing.factor(21.0) - old.scale * old.factor(21.0)).abs() < 1e-6);
        assert_eq!(first.template_variant(), next.template_variant());
        assert_eq!(next.template_variant(), 0);
    }

    #[test]
    fn delayed_upload_rebases_pending_growth_on_the_still_visible_pose() {
        let first = sprout(0.12, 64);
        let old = Pose {
            scale: first.visual_scale(),
            ratio: 0.2,
            start: 20.0,
            pending: false,
        };
        let next = sprout(0.25, 65);
        let dispatch = 20.5;
        let now = dispatch + 15.0;
        let mut pose =
            snapshot(&[next], Some(&HashMap::from([(first.id, old)])), dispatch)[&next.id];
        let stale_ratio = pose.ratio;
        pose.begin(Some(old), now);
        assert!(pose.ratio > stale_ratio);
        assert!((pose.scale * pose.factor(now) - old.scale * old.factor(now)).abs() < 1e-6);
        assert!(!pose.pending);
        assert_eq!(pose.start, now);
        assert_eq!(
            snapshot(&[next], Some(&HashMap::from([(next.id, pose)])), now + 15.0)[&next.id],
            pose
        );
        let mut restored = snapshot(&[next], None, now)[&next.id];
        let saved = restored;
        restored.begin(Some(old), now + 15.0);
        assert_eq!(
            restored, saved,
            "restored poses do not begin again at upload"
        );
    }

    #[test]
    fn motion_vectors_use_previous_growth_for_both_position_and_wind() {
        let shader = include_str!("../../../../assets/shaders/terra_props.wgsl");
        assert!(shader.contains("grown_local(input, previous_clock)"));
        assert!(shader.contains("previous_weights = input.uv;"));
        assert!(shader.contains("previous_weights.y *= growth_factor(input, previous_clock);"));
        assert!(
            shader.contains(
                "wind_position(previous, previous_weights, time - prop_globals.delta_time)"
            )
        );
        let pose = Pose {
            scale: 1.0,
            ratio: 0.2,
            start: 20.0,
            pending: false,
        };
        assert!(pose.factor(21.0 - 1.0 / 60.0) < pose.factor(21.0));
    }
}
