use std::collections::HashMap;
use terra_world::props::{PropId, WorldProp};

pub(super) const SECONDS: f32 = 2.4;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Pose {
    pub scale: f32,
    pub ratio: f32,
    pub start: f32,
    pub pending: bool,
    pub variant: u8,
    pub previous_variant: Option<u8>,
}

pub(super) fn animated(prop: &WorldProp) -> bool {
    prop.growing_grass()
        || prop.kind.crop().is_some()
        || prop.kind == terra_world::props::PropKind::Flower
}

pub(super) fn extent(prop: &WorldProp) -> f32 {
    if prop.kind.crop().is_some() {
        (prop.bounds()[1][1] - prop.position[1]).max(0.02) as f32
    } else {
        prop.visual_scale()
    }
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
        .filter(|prop| !prop.removed && animated(prop))
        .map(|prop| {
            let scale = extent(prop);
            let variant = if prop.kind.crop().is_some() {
                prop.variant
            } else {
                0
            };
            let pose = if let Some(old) = previous.and_then(|previous| previous.get(&prop.id)) {
                if old.scale == scale && old.variant == variant {
                    let mut pose = *old;
                    if !pose.pending && time >= pose.start + SECONDS {
                        pose.previous_variant = None;
                    }
                    pose
                } else {
                    Pose {
                        scale,
                        ratio: (old.scale * old.factor(time) / scale).clamp(0.0, 1.0),
                        start: -1.0,
                        pending: true,
                        variant,
                        previous_variant: prop.kind.crop().map(|_| old.variant),
                    }
                }
            } else if previous.is_some()
                && !prop.hidden_growth()
                && prop.kind != terra_world::props::PropKind::Flower
            {
                Pose {
                    scale,
                    ratio: 0.0,
                    start: -1.0,
                    pending: true,
                    variant,
                    previous_variant: None,
                }
            } else {
                // A first section upload may restore a saved mature sprout. Do not
                // replay growth whenever the player reloads or turns toward it.
                Pose {
                    scale,
                    ratio: 1.0,
                    start: 0.0,
                    pending: false,
                    variant,
                    previous_variant: None,
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
    fn seasonal_flowers_keep_root_anchors_without_starting_growth_or_replaying_on_reload() {
        let mut flower = sprout(0.65, 0);
        flower.kind = PropKind::Flower;
        assert!(animated(&flower));
        for previous in [None, Some(&HashMap::new())] {
            let poses = snapshot(&[flower], previous, 500.0);
            let pose = poses[&flower.id];
            assert_eq!(pose.ratio, 1.0);
            assert!(!pose.pending);
            assert_eq!(pose.previous_variant, None);
            assert_eq!(snapshot(&[flower], Some(&poses), 9000.0)[&flower.id], pose);
        }
        let shader = include_str!("../../../../assets/shaders/terra_props.wgsl");
        assert!(shader.contains("(input.position - input.growth_anchor) * scale"));
        assert!(shader.contains("growth_factor(input, time) * flower_scale(input, seconds_ago)"));
        assert!(
            shader.contains("previous_weights.y *= flower_scale(input, prop_globals.delta_time);")
        );
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
        };
        assert!(pose.factor(21.0 - 1.0 / 60.0) < pose.factor(21.0));
    }

    #[test]
    fn crop_stages_crossfade_from_the_old_rooted_size_and_retire_old_geometry() {
        for kind in [PropKind::Wheat, PropKind::Corn, PropKind::Lettuce] {
            let mut prop = WorldProp {
                kind,
                variant: 0,
                scale: 1.0,
                ..sprout(1.0, 0)
            };
            let restored = snapshot(&[prop], None, 100.0);
            assert_eq!(restored[&prop.id].ratio, 1.0);
            assert_eq!(restored[&prop.id].previous_variant, None);
            let mut previous = restored;
            for variant in 1..8 {
                let old = previous[&prop.id];
                prop.variant = variant;
                let now = 100.0 + f32::from(variant) * 10.0;
                let mut pose = snapshot(&[prop], Some(&previous), now)[&prop.id];
                assert_eq!(pose.previous_variant, Some(variant - 1));
                assert!(pose.pending);
                pose.begin(Some(old), now);
                assert!((pose.scale * pose.factor(now) - old.scale).abs() < 0.001);
                assert!(pose.factor(now + SECONDS * 0.5) >= pose.factor(now));
                let current = HashMap::from([(prop.id, pose)]);
                assert_eq!(
                    snapshot(&[prop], Some(&current), now + 0.5)[&prop.id],
                    pose,
                    "LOD rebuild must preserve fade clock"
                );
                let settled = snapshot(&[prop], Some(&current), now + SECONDS + 0.01);
                assert_eq!(settled[&prop.id].previous_variant, None);
                assert_eq!(settled[&prop.id].factor(now + SECONDS), 1.0);
                previous = settled;
            }
        }
        let shader = include_str!("../../../../assets/shaders/terra_prop_fragment.wgsl");
        assert!(shader.contains("crop_coverage(input);"));
        assert!(shader.contains("threshold < eased"));
        assert!(shader.contains("threshold >= eased"));
        assert!(shader.contains("prepass_alpha_discard"));
    }
}
