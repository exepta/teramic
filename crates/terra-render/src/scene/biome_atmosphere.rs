//! View-local atmospheric color, sampled from cached streamed columns only.
use bevy::prelude::*;
use terra_player::TerraCamera;
use terra_world::streaming::{ChunkStore, WorldAccess};

#[derive(Resource, Default, Clone, Copy)]
pub(crate) struct BiomeAtmosphere {
    pub cherry: f32,
    pub spirit: f32,
    pub blood: f32,
    pub mushroom: f32,
    pub ashlands: f32,
    pub swamp: f32,
    pub petrified: f32,
    pub crystal: f32,
    pub coast: f32,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<BiomeAtmosphere>()
        .add_systems(Update, sample.before(super::daylight::update));
}
fn sample(
    time: Res<Time>,
    access: Option<Res<WorldAccess>>,
    chunks: Option<Res<ChunkStore>>,
    cameras: Query<&Transform, With<TerraCamera>>,
    mut state: ResMut<BiomeAtmosphere>,
) {
    let target = access
        .filter(|a| a.ready())
        .and_then(|_| {
            let p = cameras.iter().next()?.translation;
            let c = chunks
                .as_ref()?
                .surface_column_at(p.x.floor() as i64, p.z.floor() as i64)?;
            // The atmosphere is outdoors: no pink sky/fill leaking into deep caves.
            let surface = ((p.y - c.height as f32 + 4.) / 8.).clamp(0., 1.);
            Some([
                c.biome.weights.cherry_forest as f32 * surface,
                c.biome.weights.spirit_blossom as f32 * surface,
                c.biome.weights.blood_forest as f32 * surface,
                c.biome.weights.mushroom_park as f32 * surface,
                c.biome.weights.swamp as f32 * surface,
                c.biome.weights.ashlands as f32 * ((p.y - c.height as f32 + 5.) / 5.).clamp(0., 1.),
                c.biome.weights.petrified_forest as f32 * surface,
                c.biome.weights.crystal_highlands as f32 * surface,
                c.biome.weights.coastal_land_fraction() as f32 * surface,
            ])
        })
        .unwrap_or([0.; 9]);
    let blend = 1. - (-2.5 * time.delta_secs()).exp();
    let approach = |current: f32, target: f32| {
        let next = current + (target - current) * blend;
        if (next - target).abs() < 0.0001 {
            target
        } else {
            next
        }
    };
    let next = [
        approach(state.cherry, target[0]),
        approach(state.spirit, target[1]),
        approach(state.blood, target[2]),
        approach(state.mushroom, target[3]),
        approach(state.swamp, target[4]),
        approach(state.ashlands, target[5]),
        approach(state.petrified, target[6]),
        approach(state.crystal, target[7]),
        approach(state.coast, target[8]),
    ];
    if next
        != [
            state.cherry,
            state.spirit,
            state.blood,
            state.mushroom,
            state.swamp,
            state.ashlands,
            state.petrified,
            state.crystal,
            state.coast,
        ]
    {
        state.cherry = next[0];
        state.spirit = next[1];
        state.blood = next[2];
        state.mushroom = next[3];
        state.swamp = next[4];
        state.ashlands = next[5];
        state.petrified = next[6];
        state.crystal = next[7];
        state.coast = next[8];
    }
}
impl BiomeAtmosphere {
    pub(super) fn apply(self, day: &mut super::daylight::Daylight) {
        // Blend the existing distance atmosphere and ambient sky fill; direct
        // lamps and sun intensity keep their authored colors and energy.
        let pink = Vec3::new(0.78, 0.46, 0.66) * (0.10 + 0.90 * day.daylight);
        let moonlit = Vec3::new(0.12, 0.19, 0.29).lerp(Vec3::new(0.56, 0.71, 0.87), day.daylight);
        let crimson = Vec3::new(0.60, 0.34, 0.46) * (0.10 + 0.90 * day.daylight);
        day.fog = day
            .fog
            .lerp(
                Vec3::new(0.79, 0.72, 0.83) * (0.09 + 0.91 * day.daylight),
                self.mushroom * 0.40,
            )
            .lerp(
                Vec3::new(0.12, 0.13, 0.25).lerp(Vec3::new(0.67, 0.66, 0.87), day.daylight),
                self.crystal * 0.78,
            )
            .lerp(
                Vec3::new(0.12, 0.17, 0.22).lerp(Vec3::new(0.64, 0.72, 0.76), day.daylight),
                self.coast * 0.86,
            )
            .lerp(pink, self.cherry * 0.65)
            .lerp(moonlit, self.spirit * 0.48)
            .lerp(crimson, self.blood * 0.68)
            .lerp(
                Vec3::new(0.17, 0.16, 0.23).lerp(Vec3::new(0.83, 0.76, 0.71), day.daylight),
                self.petrified * 0.77,
            )
            .lerp(
                Vec3::new(0.53, 0.55, 0.49) * (0.12 + 0.88 * day.daylight),
                self.swamp * 0.88,
            )
            .lerp(
                Vec3::new(0.21, 0.097, 0.16).lerp(Vec3::new(0.65, 0.39, 0.49), day.daylight),
                self.ashlands * 0.98,
            );
        day.ambient_color = day
            .ambient_color
            .lerp(Vec3::new(1.0, 0.94, 0.86), self.mushroom * 0.19)
            .lerp(Vec3::new(0.82, 0.95, 1.0), self.spirit * 0.34)
            .lerp(Vec3::new(1.0, 0.73, 0.88), self.cherry * 0.40)
            // A nearly white rose sky preserves diffuse fill beneath the canopy;
            // tinting that fill saturated red removed too much green/blue energy.
            .lerp(Vec3::new(1.0, 0.89, 0.93), self.blood * 0.18)
            .lerp(Vec3::new(0.96, 0.76, 0.79), self.ashlands * 0.54)
            .lerp(Vec3::new(1.0, 0.93, 0.82), self.petrified * 0.46)
            .lerp(Vec3::new(0.87, 0.83, 1.0), self.crystal * 0.52)
            .lerp(Vec3::new(0.88, 0.95, 1.0), self.coast * 0.38);
        day.humidity = self.swamp.max(self.coast * 0.85);
        // Ash clouds scatter daylight into basalt shadows. The pale fossil beds
        // also reflect warm skylight onto the trunks and the carved facades.
        day.ambient *= 1.0
            + self.ashlands * (0.42 + 2.6 * day.daylight)
            + self.petrified * (0.12 + 0.85 * day.daylight)
            + self.crystal * (0.15 + 0.84 * day.daylight)
            + self.coast * (0.18 + 1.05 * day.daylight)
            - self.swamp * 0.12;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_palette_fades_without_mutating_light_energy_or_other_biomes() {
        let original = super::super::daylight::Daylight::at(0.5);
        let mut unchanged = original;
        BiomeAtmosphere::default().apply(&mut unchanged);
        assert_eq!(unchanged.fog, original.fog);
        assert_eq!(unchanged.ambient_color, original.ambient_color);
        let mut pink = original;
        BiomeAtmosphere {
            cherry: 1.,
            spirit: 0.,
            blood: 0.,
            ..default()
        }
        .apply(&mut pink);
        assert!(pink.fog.x > pink.fog.y && pink.fog.z > pink.fog.y);
        assert_eq!(pink.ambient, original.ambient);
        assert_eq!(pink.exposure, original.exposure);
        assert_eq!(pink.sun, original.sun);
        let mut blood = original;
        BiomeAtmosphere {
            blood: 1.,
            ..default()
        }
        .apply(&mut blood);
        assert!(blood.fog.x / blood.fog.y > original.fog.x / original.fog.y);
        assert_eq!(blood.sun, original.sun);
        assert_eq!(blood.ambient, original.ambient);
        assert_eq!(blood.exposure, original.exposure);
        let mut previous = original.fog;
        for i in 0..=100 {
            let mut day = original;
            BiomeAtmosphere {
                cherry: i as f32 / 100.,
                spirit: 0.,
                blood: 0.,
                ..default()
            }
            .apply(&mut day);
            assert!(day.fog.distance(previous) < 0.01);
            previous = day.fog;
        }
    }

    #[test]
    fn marsh_haze_leaves_the_foreground_visible_and_follows_the_camera_offset() {
        let mut day = super::super::daylight::Daylight::at(0.5);
        let dry = day;
        for (weight, offset, expected) in [
            (0., 0., (100., 225.)),
            (1., 0., (14., 110.)),
            (1., 50., (64., 160.)),
            (0.5, 0., (57., 167.5)),
        ] {
            day = dry;
            BiomeAtmosphere {
                swamp: weight,
                ..Default::default()
            }
            .apply(&mut day);
            assert_eq!(day.fog_range((100., 225.), offset), expected);
        }
    }

    #[test]
    fn blood_canopy_fill_retains_linear_luminance_through_the_day() {
        let luminance = |color: Vec3| {
            let linear = Color::srgb(color.x, color.y, color.z).to_linear();
            0.2126 * linear.red + 0.7152 * linear.green + 0.0722 * linear.blue
        };
        for step in 0..=96 {
            let original = super::super::daylight::Daylight::at(f64::from(step) / 96.);
            for weight in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let mut tinted = original;
                BiomeAtmosphere {
                    blood: weight,
                    ..default()
                }
                .apply(&mut tinted);
                assert!(
                    luminance(tinted.ambient_color) >= luminance(original.ambient_color) * 0.98
                );
                assert_eq!(tinted.ambient, original.ambient);
                assert_eq!(tinted.exposure, original.exposure);
                assert_eq!(tinted.illuminance(true), original.illuminance(true));
                assert_eq!(tinted.illuminance(false), original.illuminance(false));
            }
        }
    }
}
