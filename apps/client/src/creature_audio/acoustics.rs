//! Small loaded-cell probes, independent of cameras and audio-device callbacks.
use bevy::prelude::*;

/// Hear a neighborhood rather than a single point on a region boundary.
/// The center keeps half the influence; four nearby samples share the rest.
pub(super) fn biome_neighborhood(
    center: [f64; 2],
    middle: [f64; 16],
    mut sample: impl FnMut(f64, f64) -> [f64; 16],
) -> [f64; 16] {
    let mut weights = middle.map(|weight| weight * 0.5);
    for [dx, dz] in [[-16.0, 0.0], [16.0, 0.0], [0.0, -16.0], [0.0, 16.0]] {
        for (sum, weight) in weights
            .iter_mut()
            .zip(sample(center[0] + dx, center[1] + dz))
        {
            *sum += weight * 0.125;
        }
    }
    weights
}

/// Rain is heard where it hits nearby surfaces, not as a loop attached to a
/// flying listener. Nine rays at 2 Hz never request or generate terrain.
pub(super) fn rain_surface_proximity(
    listener: Vec3,
    mut receives_rain: impl FnMut([i64; 3]) -> Option<bool>,
) -> f32 {
    let center = listener.floor().as_i64vec3().to_array();
    let mut energy = 0.0;
    let mut weight = 0.0;
    for dx in [-6_i64, 0, 6] {
        for dz in [-6_i64, 0, 6] {
            let influence = if dx == 0 && dz == 0 { 2.0 } else { 1.0 };
            weight += influence;
            let impact = ((center[1] - 36)..=(center[1] + 24))
                .rev()
                .find(|&y| receives_rain([center[0] + dx, y, center[2] + dz]) == Some(true));
            if let Some(y) = impact {
                let p = Vec3::new(
                    (center[0] + dx) as f32 + 0.5,
                    y as f32 + 1.0,
                    (center[2] + dz) as f32 + 0.5,
                );
                energy += influence * (1.0 - listener.distance(p) / 32.0).clamp(0.0, 1.0).powi(2);
            }
        }
    }
    (energy / weight).sqrt()
}

pub(super) fn rain_gain(rain: f32, roof: bool, depth: f32, cave: f32, impact: f32) -> f32 {
    let shelter = if roof { 0.22 } else { 1.0 };
    rain.clamp(0.0, 1.0)
        * 0.52
        * shelter
        * impact.clamp(0.0, 1.0)
        * (1.0 - cave * (depth / 18.0).clamp(0.0, 1.0))
}

pub(super) fn wet_step_mix(rain: f32, wetness: f32, surface: super::clips::Surface) -> f32 {
    use super::clips::Surface;
    let film = ((wetness - 0.06) / 0.65).clamp(0.0, 1.0);
    let raining = (rain / 0.15).clamp(0.0, 1.0);
    let absorption = match surface {
        Surface::Dirt | Surface::Grass | Surface::Leaves => 0.72,
        Surface::Sand | Surface::Gravel => 0.45,
        Surface::Stone | Surface::Wood | Surface::Glass => 0.58,
        Surface::Snow => 0.25,
    };
    film * raining * absorption
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighborhood_preserves_regions_and_introduces_neighbors_before_crossing() {
        let region = |x: f64, _: f64| {
            let mut weights = [0.0; 16];
            weights[if x < 0.0 { 2 } else { 8 }] = 1.0;
            weights
        };
        let inside = biome_neighborhood([-40.0, 0.0], region(-40.0, 0.0), region);
        assert_eq!(inside[2], 1.0);
        assert_eq!(inside[8], 0.0);
        let near = biome_neighborhood([-5.0, 0.0], region(-5.0, 0.0), region);
        assert_eq!(near[2], 0.875);
        assert_eq!(near[8], 0.125);
        assert_eq!(near.iter().sum::<f64>(), 1.0);
    }

    #[test]
    fn rain_requires_real_nearby_impact_surfaces_and_fades_with_altitude() {
        let ground = |p: [i64; 3]| Some(p[1] == 0);
        let close = rain_surface_proximity(Vec3::new(0.5, 2.6, 0.5), ground);
        let flying = rain_surface_proximity(Vec3::new(0.5, 25.0, 0.5), ground);
        assert!(close > 0.75);
        assert!(flying > 0.0 && flying < close * 0.4);
        assert_eq!(rain_surface_proximity(Vec3::Y * 45.0, ground), 0.0);
        assert_eq!(rain_surface_proximity(Vec3::ZERO, |_| None), 0.0);
        assert_eq!(rain_surface_proximity(Vec3::ZERO, |_| Some(false)), 0.0);
    }

    #[test]
    fn rain_is_sheltered_quiet_indoors_and_silent_in_deep_caves() {
        assert_eq!(rain_gain(0.0, false, 0.0, 0.0, 1.0), 0.0);
        assert_eq!(rain_gain(1.0, false, 0.0, 0.0, 1.0), 0.52);
        assert_eq!(rain_gain(1.0, false, 0.0, 0.0, 0.0), 0.0);
        assert!(rain_gain(1.0, true, 0.0, 0.0, 1.0) < 0.13);
        assert_eq!(rain_gain(1.0, true, 20.0, 1.0, 1.0), 0.0);
        assert!(rain_gain(1.0, true, 5.0, 0.8, 1.0) > 0.0);
    }

    #[test]
    fn only_rain_on_already_wet_blocks_adds_contact_water() {
        use super::super::clips::Surface;
        assert_eq!(wet_step_mix(0.0, 1.0, Surface::Grass), 0.0);
        assert_eq!(wet_step_mix(1.0, 0.0, Surface::Grass), 0.0);
        let wet = wet_step_mix(1.0, 1.0, Surface::Grass);
        assert!(wet > 0.6 && wet < 1.0);
        assert!(wet_step_mix(1.0, 0.2, Surface::Grass) < wet);
        assert!(wet_step_mix(1.0, 1.0, Surface::Sand) < wet);
        assert!(wet_step_mix(1.0, 1.0, Surface::Snow) < wet);
    }
}
