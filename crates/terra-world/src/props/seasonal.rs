//! Reversible seasonal appearance, shared by batched rendering and authority picking.
use super::{PropKind, WorldProp};

pub const FLOWER_WITHER_SECONDS: f32 = 10.0;
pub const FLOWER_RANKS: usize = 256;
const ALIVE: f32 = -1.0;
const DORMANT: f32 = -2.0;

/// One bounded clock per quantized flower rank, never one entry per plant.
/// This is transient appearance state: loading an already dormant world does
/// not bring its flowers back just to replay their death animation.
#[derive(Clone, Debug)]
pub struct FlowerLifecycle {
    starts: [f32; FLOWER_RANKS],
    now: f32,
    initialized: bool,
}

impl Default for FlowerLifecycle {
    fn default() -> Self {
        Self {
            starts: [ALIVE; FLOWER_RANKS],
            now: 0.0,
            initialized: false,
        }
    }
}

impl FlowerLifecycle {
    pub fn advance(&mut self, loss: f32, seconds: f32) {
        let loss = if loss.is_finite() {
            loss.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if seconds.is_finite() && seconds > 0.0 {
            self.now += seconds;
        }
        for (rank, start) in self.starts.iter_mut().enumerate().take(254).skip(2) {
            if rank as f32 / 255.0 > loss {
                *start = ALIVE;
            } else if !self.initialized {
                *start = DORMANT;
            } else if *start == ALIVE {
                *start = self.now;
            }
        }
        self.initialized = true;
    }

    pub fn initialized(&self) -> bool {
        self.initialized
    }

    pub fn shader_times(&self) -> &[f32; FLOWER_RANKS] {
        &self.starts
    }

    pub fn shader_time(&self) -> f32 {
        self.now
    }

    pub fn progress(&self, prop: &WorldProp, season: terra_core::SeasonSample) -> f32 {
        if prop.kind != PropKind::Flower {
            return 0.0;
        }
        if !self.initialized {
            return 1.0 - flower_coverage(prop, season);
        }
        let start = self.starts[(flower_rank(prop) * 255.0).round() as usize];
        if start == ALIVE {
            0.0
        } else if start == DORMANT {
            1.0
        } else {
            ((self.now - start) / FLOWER_WITHER_SECONDS).clamp(0.0, 1.0)
        }
    }

    pub fn coverage(&self, prop: &WorldProp, season: terra_core::SeasonSample) -> f32 {
        if !self.initialized {
            return flower_coverage(prop, season);
        }
        let progress = self.progress(prop, season);
        1.0 - progress * progress * (3.0 - 2.0 * progress)
    }
}

#[cfg(feature = "bevy")]
pub(crate) fn tick(
    config: bevy::prelude::Res<terra_core::GameConfig>,
    clock: Option<bevy::prelude::Res<terra_core::WorldClock>>,
    time: bevy::prelude::Res<bevy::prelude::Time<bevy::prelude::Real>>,
    mut journal: bevy::prelude::ResMut<crate::edits::EditJournal>,
) {
    use bevy::prelude::DetectChangesMut;
    let season = journal
        .clock
        .or_else(|| clock.as_deref().copied())
        .unwrap_or_default()
        .season(config.world.seasons);
    journal
        .bypass_change_detection()
        .flowers
        .advance(season.flower_loss(), time.delta_secs());
}

pub fn flower_rank(prop: &WorldProp) -> f32 {
    if prop.kind != PropKind::Flower {
        return 1.0;
    }
    let mut hash = prop.id.0 ^ 0xd395_187a_2c04_6ef1;
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    let rank = ((hash ^ (hash >> 31)) % 252 + 2) as u8;
    f32::from(rank) / 255.0
}

pub fn flower_coverage(prop: &WorldProp, season: terra_core::SeasonSample) -> f32 {
    let rank = flower_rank(prop);
    if rank == 1.0 {
        return 1.0;
    }
    let lower = (rank - 0.04).max(0.0);
    let upper = (rank + 0.04).min(1.0);
    let t = ((season.flower_loss() - lower) / (upper - lower)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

pub fn snow_sensitive(prop: &WorldProp) -> bool {
    !prop.wall_attached()
        && matches!(
            prop.kind,
            PropKind::Grass
                | PropKind::Flower
                | PropKind::Bush
                | PropKind::Fern
                | PropKind::Mushroom
                | PropKind::SmallPlant
                | PropKind::SmallStones
                | PropKind::Wheat
                | PropKind::Corn
                | PropKind::Lettuce
        )
}

#[cfg(feature = "bevy")]
pub fn snow_buried(
    prop: &WorldProp,
    chunks: &crate::streaming::ChunkStore,
    journal: &crate::edits::EditJournal,
) -> bool {
    if !snow_sensitive(prop) {
        return false;
    }
    let support = super::placement::support(prop);
    let cap = [support[0], support[1] + 1, support[2]];
    match journal.block(cap).or_else(|| chunks.block_at(cap)) {
        Some(5) => true,
        Some(98) => journal
            .object_state(cap)
            .is_some_and(|state| terra_core::construction::snow_layers(state.facing) >= 3),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::{PropId, PropPose, raycast_props_with_pose};

    fn flower(id: u64) -> WorldProp {
        WorldProp {
            id: PropId(id),
            kind: PropKind::Flower,
            position: [-0.5, 2.0, -0.5],
            rotation: 0.0,
            scale: 0.65,
            variant: 0,
            section: terra_voxel::SectionPos::new(-1, 0, -1),
            removed: false,
        }
    }

    fn winter() -> terra_core::SeasonSample {
        terra_core::WorldClock {
            day: 21,
            ..Default::default()
        }
        .season(terra_core::SeasonConfig::default())
    }

    #[test]
    fn seasonal_threshold_starts_one_bounded_ten_real_second_wither_without_season_clock_progress()
    {
        let prop = flower(7);
        let rank = flower_rank(&prop);
        let mut flowers = FlowerLifecycle::default();
        flowers.advance(0.0, 0.0);
        flowers.advance(rank, 0.2);
        assert_eq!(flowers.progress(&prop, winter()), 0.0);
        for step in 1..=10 {
            flowers.advance(rank, 1.0);
            assert!((flowers.progress(&prop, winter()) - step as f32 / 10.0).abs() < 1e-5);
            assert_eq!(flowers.shader_times().len(), 256);
        }
        assert_eq!(flowers.coverage(&prop, winter()), 0.0);
        flowers.advance(rank, 600.0);
        assert_eq!(flowers.coverage(&prop, winter()), 0.0);
        assert!(!prop.removed);
    }

    #[test]
    fn dormant_reload_does_not_flash_and_clock_jumps_reverse_and_restart_only_live_ranks() {
        let prop = flower(17);
        let mut restored = FlowerLifecycle::default();
        restored.advance(1.0, 0.0);
        assert_eq!(restored.progress(&prop, winter()), 1.0);
        restored.advance(1.0, 3.0);
        assert_eq!(restored.progress(&prop, winter()), 1.0);
        restored.advance(0.0, 0.0);
        assert_eq!(restored.progress(&prop, winter()), 0.0);
        restored.advance(1.0, 1200.0);
        assert_eq!(restored.progress(&prop, winter()), 0.0);
        restored.advance(1.0, 5.0);
        assert!((restored.progress(&prop, winter()) - 0.5).abs() < 1e-5);
        let cloned = restored.clone();
        assert_eq!(
            cloned.progress(&prop, winter()),
            restored.progress(&prop, winter())
        );
        restored.advance(0.0, 0.0);
        assert_eq!(restored.coverage(&prop, winter()), 1.0);
        restored.advance(1.0, 0.0);
        assert_eq!(restored.coverage(&prop, winter()), 1.0);
        restored.advance(1.0, 10.0);
        assert_eq!(restored.coverage(&prop, winter()), 0.0);
    }

    #[test]
    fn timed_picking_shrinks_at_the_root_then_removes_hits_without_removing_the_saved_flower() {
        let prop = flower(7);
        let mut flowers = FlowerLifecycle::default();
        flowers.advance(0.0, 0.0);
        flowers.advance(1.0, 0.0);
        let ray = (1..100)
            .rev()
            .map(|step| [-0.5, 2.0 + f64::from(step) * 0.01, 1.0])
            .find(|origin| {
                raycast_props_with_pose(*origin, [0.0, 0.0, -1.0], 4.0, [&prop], PropPose::Rest)
                    .is_some()
            })
            .unwrap();
        let pose = PropPose::Rest.with_season(winter());
        assert!(
            super::super::raycast_props_with_lifecycle(
                ray,
                [0.0, 0.0, -1.0],
                4.0,
                [&prop],
                pose,
                Some(&flowers)
            )
            .is_some()
        );
        flowers.advance(1.0, 5.0);
        assert!(
            super::super::raycast_props_with_lifecycle(
                ray,
                [0.0, 0.0, -1.0],
                4.0,
                [&prop],
                pose,
                Some(&flowers)
            )
            .is_none(),
            "the old full-height geometry must not remain selectable"
        );
        let smaller_ray = [
            ray[0],
            prop.position[1] + (ray[1] - prop.position[1]) * 0.5,
            ray[2],
        ];
        assert!(
            super::super::raycast_props_with_lifecycle(
                smaller_ray,
                [0.0, 0.0, -1.0],
                4.0,
                [&prop],
                pose,
                Some(&flowers)
            )
            .is_some(),
            "the shrinking flower stays rooted and selectable at its actual size"
        );
        flowers.advance(1.0, 5.0);
        assert!(
            super::super::raycast_props_with_lifecycle(
                ray,
                [0.0, 0.0, -1.0],
                4.0,
                [&prop],
                pose,
                Some(&flowers)
            )
            .is_none()
        );
        flowers.advance(0.0, 0.0);
        assert!(
            super::super::raycast_props_with_lifecycle(
                ray,
                [0.0, 0.0, -1.0],
                4.0,
                [&prop],
                pose,
                Some(&flowers)
            )
            .is_some()
        );
        assert!(!prop.removed);
    }

    #[cfg(feature = "bevy")]
    #[test]
    fn lifecycle_tick_does_not_dirty_the_edit_journal_or_any_section_epoch() {
        use bevy::prelude::*;
        #[derive(Resource, Default)]
        struct Changed(bool);
        let mut app = App::new();
        app.init_resource::<terra_core::GameConfig>()
            .init_resource::<Time<Real>>()
            .init_resource::<crate::edits::EditJournal>()
            .init_resource::<Changed>()
            .add_systems(
                Update,
                (
                    tick,
                    |journal: Res<crate::edits::EditJournal>, mut changed: ResMut<Changed>| {
                        changed.0 = journal.is_changed()
                    },
                )
                    .chain(),
            );
        app.update();
        app.update();
        assert!(!app.world().resource::<Changed>().0);
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_secs(5));
        app.update();
        assert!(!app.world().resource::<Changed>().0);
        let journal = app.world().resource::<crate::edits::EditJournal>();
        assert_eq!(journal.changed_sections().count(), 0);
        assert!(journal.flowers.initialized());
    }

    #[test]
    fn flower_ranks_survive_reload_and_stagger_a_continuous_autumn_fade() {
        let config = terra_core::SeasonConfig::default();
        let mut clock = terra_core::WorldClock {
            day: 14,
            phase: 0.0,
            ..Default::default()
        };
        let first = clock.season(config);
        clock.day = 17;
        clock.phase = 0.5;
        let middle = clock.season(config);
        clock.day = 21;
        let winter = clock.season(config);
        let mut hidden = 0;
        let mut fading = 0;
        let mut visible = 0;
        for id in 1..=256 {
            let prop = flower(id);
            assert_eq!(flower_rank(&prop), flower_rank(&prop));
            assert_eq!(
                (flower_rank(&prop) * 255.0).round() / 255.0,
                flower_rank(&prop)
            );
            assert_eq!(flower_coverage(&prop, first), 1.0);
            assert_eq!(flower_coverage(&prop, winter), 0.0);
            let coverage = flower_coverage(&prop, middle);
            hidden += usize::from(coverage == 0.0);
            visible += usize::from(coverage == 1.0);
            fading += usize::from(coverage > 0.0 && coverage < 1.0);
        }
        assert!(hidden > 60 && visible > 60 && fading > 0);
        let mut prop = flower(9);
        prop.kind = PropKind::Mushroom;
        assert_eq!(flower_rank(&prop), 1.0);
        assert_eq!(flower_coverage(&prop, winter), 1.0);
    }

    #[test]
    fn fully_dormant_flowers_do_not_create_invisible_prop_hits_and_regrow_in_spring() {
        let prop = flower(7);
        let config = terra_core::SeasonConfig::default();
        let spring = terra_core::WorldClock::default().season(config);
        let winter = terra_core::WorldClock {
            day: 21,
            ..Default::default()
        }
        .season(config);
        let ray = (1..100)
            .map(|step| [-0.5, 2.0 + f64::from(step) * 0.01, 1.0])
            .find(|origin| {
                raycast_props_with_pose(*origin, [0.0, 0.0, -1.0], 4.0, [&prop], PropPose::Rest)
                    .is_some()
            })
            .expect("real flower mesh is selectable");
        for pose in [PropPose::Rest, PropPose::Swept] {
            assert!(
                raycast_props_with_pose(
                    ray,
                    [0.0, 0.0, -1.0],
                    4.0,
                    [&prop],
                    pose.with_season(winter)
                )
                .is_none()
            );
        }
        assert!(
            raycast_props_with_pose(
                ray,
                [0.0, 0.0, -1.0],
                4.0,
                [&prop],
                PropPose::Rest.with_season(spring)
            )
            .is_some()
        );
        assert!(!prop.removed);
    }
}
