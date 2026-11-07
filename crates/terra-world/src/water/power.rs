//! Mechanical contacts sample loaded liquid, never the camera's visual waves.
use super::fluid_motion_at;
use crate::{TerraGenerator, edits::EditJournal, streaming::ChunkStore};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterDriveKind {
    Current,
    Fall,
    Ocean,
}

impl WaterDriveKind {
    pub fn strength(self, storm: f32) -> f32 {
        let storm = if storm.is_finite() {
            storm.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (calm, rough) = match self {
            Self::Current => (1.0, 1.2),
            Self::Fall => (1.2, 1.35),
            Self::Ocean => (1.1, 1.4),
        };
        calm + (rough - calm) * storm
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WheelMotion {
    pub velocity: [f32; 3],
    pub immersion: f32,
    pub kind: Option<WaterDriveKind>,
    pub strength: f32,
}

pub fn wheel_motion_at(
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [f64; 3],
) -> WheelMotion {
    let motion = fluid_motion_at(terrain, chunks, journal, position);
    if motion.immersion <= 0.0 {
        return WheelMotion::default();
    }
    let weather = journal.weather.unwrap_or_default();
    let wind = weather.wind();
    let speed = wind[0].hypot(wind[1]);
    let storm = ((speed - 2.2) / 4.4).clamp(0.0, 1.0);
    let mut velocity = motion.velocity;
    let kind = if velocity[1] < -0.1 {
        Some(WaterDriveKind::Fall)
    } else if velocity[0].hypot(velocity[2]) > 0.1 {
        Some(WaterDriveKind::Current)
    } else {
        // A lake at sea-level is not automatically an ocean. Require the
        // actual prepared marine column and a contact near its free surface.
        let global = position.map(|v| v.floor() as i64);
        chunks
            .surface_column_at(global[0], global[2])
            .filter(|column| marine_contact(column, terrain.config().sea_level, position[1]))
            .map(|_| {
                velocity = ocean_push(wind, weather.elapsed_total, position);
                WaterDriveKind::Ocean
            })
    };
    WheelMotion {
        velocity,
        immersion: motion.immersion,
        kind,
        strength: kind.map_or(0.0, |kind| kind.strength(storm)),
    }
}

fn marine_contact(column: &crate::SurfaceColumn, sea_level: f64, height: f64) -> bool {
    column.biome_kind(sea_level) == crate::generation::biomes::BiomeKind::Ocean
        && column
            .water_level
            .is_some_and(|level| height >= level - 2.5 && height < level)
}

fn ocean_push(wind: [f32; 2], seconds: f64, position: [f64; 3]) -> [f32; 3] {
    let length = wind[0].hypot(wind[1]);
    let direction = if length > 0.001 {
        wind.map(|v| v / length)
    } else {
        [0.8, 0.6]
    };
    // Net windward crest transport has a stable direction and pulsing strength;
    // unlike a camera-dependent normal map it remains authoritative on servers.
    let phase = ((position[0] * f64::from(direction[0]) + position[2] * f64::from(direction[1]))
        * 0.74
        - seconds * 2.1) as f32;
    let impulse = 0.65 + 0.25 * phase.sin();
    [direction[0] * impulse, 0.0, direction[1] * impulse]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calm_lakes_dry_and_unloaded_cells_cannot_power_a_wheel_but_loaded_currents_do() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            water: true,
            rivers: false,
            sea_level: -100.0,
            ..Default::default()
        })
        .unwrap();
        let mut chunks = ChunkStore::default();
        let owner = terra_voxel::SectionPos::default();
        chunks.loaded.insert(
            owner,
            crate::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut journal = EditJournal::default();
        let p = [7, 3, 7];
        let point = [7.5, 3.4, 7.5];
        assert_eq!(
            wheel_motion_at(&terrain, &chunks, &journal, point).kind,
            None
        );
        let still = super::super::FluidCell {
            amount: 8,
            source: true,
            flow: [0, 0],
        };
        journal.set_fluid(p, still).unwrap();
        assert_eq!(
            wheel_motion_at(&terrain, &chunks, &journal, point).kind,
            None
        );
        journal
            .set_fluid(
                p,
                super::super::FluidCell {
                    flow: [1, 0],
                    ..still
                },
            )
            .unwrap();
        let current = wheel_motion_at(&terrain, &chunks, &journal, point);
        assert_eq!(current.kind, Some(WaterDriveKind::Current));
        assert_eq!(current.strength, 1.0);
        assert!(current.velocity[0] > 0.0);
        let storm = terra_core::WorldWeather {
            from_wind: [6.6, 0.0],
            target_wind: [6.6, 0.0],
            ..Default::default()
        };
        journal.weather = Some(storm);
        assert!(
            (wheel_motion_at(&terrain, &chunks, &journal, point).strength - 1.2).abs() < 0.0001
        );
        chunks.loaded.clear();
        assert_eq!(
            wheel_motion_at(&terrain, &chunks, &journal, point).kind,
            None
        );
    }

    #[test]
    fn sea_level_lake_is_not_a_marine_contact_and_deep_water_cannot_capture_surface_waves() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let mut column = terrain.surface_column(0.0, 0.0);
        column.height = -3.0;
        column.water_level = Some(8.0);
        column.river_cavity = None;
        column.biome.weights = crate::generation::biomes::BiomeWeights {
            ocean: 1.0,
            plains: 0.0,
            forest: 0.0,
            desert: 0.0,
            highland: 0.0,
            mushroom_park: 0.0,
            cliffted_desert: 0.0,
            cherry_forest: 0.0,
        };
        assert!(marine_contact(&column, 8.0, 7.9));
        assert!(!marine_contact(&column, 8.0, 8.0));
        assert!(!marine_contact(&column, 8.0, 4.0));
        column.biome.weights.ocean = 0.0;
        column.biome.weights.plains = 1.0;
        assert!(!marine_contact(&column, 8.0, 7.9));
    }
    #[test]
    fn sources_have_the_requested_calm_and_storm_strengths() {
        for (kind, calm, storm) in [
            (WaterDriveKind::Current, 1.0, 1.2),
            (WaterDriveKind::Fall, 1.2, 1.35),
            (WaterDriveKind::Ocean, 1.1, 1.4),
        ] {
            assert_eq!(kind.strength(0.0), calm);
            assert!((kind.strength(1.0) - storm).abs() < 0.00001);
            assert!(kind.strength(0.5) > calm);
            assert_eq!(kind.strength(f32::NAN), calm);
        }
    }
    #[test]
    fn ocean_crests_are_time_dependent_bounded_and_world_position_deterministic() {
        let p = [-32.5, 7.8, 0.5];
        let a = ocean_push([0.0, 1.0], 1.0, p);
        assert_eq!(a, ocean_push([0.0, 1.0], 1.0, p));
        assert_ne!(a, ocean_push([0.0, 1.0], 2.0, p));
        assert_eq!(a[0], 0.0);
        assert!((0.39..=0.91).contains(&a[2]));
        assert!(ocean_push([0.0, -1.0], 1.0, p)[2] < 0.0);
    }
}
