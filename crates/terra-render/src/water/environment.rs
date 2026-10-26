use crate::scene::{SURFACE_AMBIENT, SURFACE_FOG, SURFACE_SATURATION, SURFACE_TEMPERATURE};
use bevy::{prelude::*, render::view::ColorGrading};
use terra_core::{GameConfig, WaterQuality};
use terra_player::TerraCamera;
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldTerrain},
};

#[derive(Resource, Default, Debug)]
pub struct UnderwaterState {
    pub submerged: bool,
    pub blend: f32,
    pub depth: f32,
}
type Cameras<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static mut DistanceFog,
        &'static mut ColorGrading,
        &'static mut AmbientLight,
        &'static mut crate::scene::caves::shader::CaveSettings,
    ),
    With<TerraCamera>,
>;
type ViewEnvironment<'w> = (
    Res<'w, Time>,
    Res<'w, crate::scene::daylight::Daylight>,
    Res<'w, crate::scene::caves::CaveState>,
    Option<Res<'w, crate::weather::WeatherVisual>>,
    Option<Res<'w, terra_network::client::ClientSession>>,
);

#[derive(Default)]
pub(crate) struct WaterProbe {
    key: Option<([i64; 3], u64, usize)>,
    next: f32,
    surface: Option<f32>,
}

/// Uses prepared fluid data only, never procedural noise on the render thread.
pub(crate) fn water_surface(
    terrain: &WorldTerrain,
    chunks: &ChunkStore,
    journal: &EditJournal,
    cell: [i64; 3],
) -> Option<f32> {
    chunks.prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])?;
    let fluid_at = |y| {
        let cell = [cell[0], y, cell[2]];
        let raw = chunks.block_at(cell)?;
        let fluid = chunks.fluid_cell_at(&terrain.0, journal, cell, raw);
        (fluid.amount > 0).then_some(fluid)
    };
    let mut fluid = fluid_at(cell[1])?;
    let mut y = cell[1];
    // Below this limit the optical model is already fully in its deep-water range.
    for _ in 0..64 {
        if fluid.amount < terra_world::water::FLUID_CAPACITY {
            break;
        }
        let Some(next) = fluid_at(y + 1) else { break };
        fluid = next;
        y += 1;
    }
    Some(y as f32 + f32::from(fluid.amount) / f32::from(terra_world::water::FLUID_CAPACITY))
}

impl WaterProbe {
    fn sample(
        &mut self,
        now: f32,
        cell: [i64; 3],
        terrain: &WorldTerrain,
        chunks: &ChunkStore,
        journal: &EditJournal,
    ) -> Option<f32> {
        let (section, _) = terra_voxel::global_to_section(cell);
        let pointer = chunks
            .prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])
            .map_or(0, |column| std::sync::Arc::as_ptr(&column) as usize);
        let key = (cell, journal.water_mesh_epoch(section), pointer);
        if self.key != Some(key) || now >= self.next {
            self.key = Some(key);
            self.next = now + 0.20;
            self.surface = water_surface(terrain, chunks, journal, cell).or_else(|| {
                water_surface(terrain, chunks, journal, [cell[0], cell[1] - 1, cell[2]])
            });
        }
        self.surface
    }
}

pub(crate) fn update(
    (time, daylight, cave, weather, session): ViewEnvironment,
    (config, terrain, chunks, journal): (
        Res<GameConfig>,
        Res<WorldTerrain>,
        Res<ChunkStore>,
        Res<EditJournal>,
    ),
    mut state: ResMut<UnderwaterState>,
    mut cameras: Cameras,
    mut probe: Local<WaterProbe>,
    mut previous_vision: Local<f32>,
) {
    let vision = session
        .as_ref()
        .filter(|session| session.ready)
        .and_then(|session| {
            session
                .stats
                .survival
                .effects
                .get(terra_core::EffectKind::NightVision)
        })
        .map_or(0.0, |effect| {
            (2.0 + f32::from(effect.level) * 2.0) * (effect.remaining_seconds / 3.0).clamp(0.0, 1.0)
        });
    let vision_changed = *previous_vision != vision;
    *previous_vision = vision;
    for (camera, mut fog, mut grading, mut ambient, mut optical) in &mut cameras {
        let p = camera.translation;
        let old_depth = state.depth;
        let cell = p.floor().as_i64vec3().to_array();
        state.depth = if config.world.water
            && chunks
                .block_at(cell)
                .is_some_and(terra_world::water::can_hold_fluid)
        {
            if chunks.block_at(cell).is_some_and(|id| {
                terra_core::construction::is_slab(id) || terra_core::construction::is_stair(id)
            }) {
                chunks
                    .water_column_at_point(&terrain.0, &journal, p.as_dvec3().to_array())
                    .map_or(-1.0, |water| water.level as f32 - p.y)
            } else {
                probe
                    .sample(time.elapsed_secs(), cell, &terrain, &chunks, &journal)
                    .map_or(-1.0, |level| level - p.y)
            }
        } else {
            -1.0
        };
        state.submerged = state.depth > if state.submerged { -0.03 } else { 0.06 };
        let target = f32::from(u8::from(state.submerged));
        let stable = state.blend == target
            && state.depth == old_depth
            && !config.is_changed()
            && !daylight.is_changed()
            && !cave.is_changed()
            && !vision_changed
            && weather.as_ref().is_none_or(|weather| !weather.is_changed());
        state.blend += (target - state.blend) * (1.0 - (-14.0 * time.delta_secs()).exp());
        if (target - state.blend).abs() < 0.001 {
            state.blend = target;
        }
        let blend = state.blend;
        let biome_color = chunks
            .prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])
            .and_then(|column| {
                column
                    .surface_at(cell[0], cell[2])
                    .map(|surface| super::biome::biome_tint(surface.biome))
            })
            .unwrap_or(super::biome::DEFAULT_TINT);
        let strength = match config.client.water_quality {
            WaterQuality::Low => 0.0,
            WaterQuality::Medium => 0.55,
            WaterQuality::High => 1.0,
        };
        let water = Vec4::new(
            blend * strength,
            state.depth.max(0.0),
            if blend > 0.001 {
                time.elapsed_secs()
            } else {
                0.0
            },
            strength,
        );
        let scattering = medium_scattering(biome_color, daylight.daylight, cave.blend);
        let medium_color = scattering.truncate();
        let tint_changed = optical.scattering != scattering;
        let world_from_view = if blend > 0.001 {
            Mat4::from_scale_rotation_translation(camera.scale, camera.rotation, camera.translation)
        } else {
            Mat4::IDENTITY
        };
        if optical.underwater != water {
            optical.underwater = water;
        }
        if tint_changed {
            optical.scattering = scattering;
        }
        if optical.world_from_view != world_from_view {
            optical.world_from_view = world_from_view;
        }
        if stable && !tint_changed {
            continue;
        }
        let range = f32::from(config.client.view_distance.max(1)) * 16.0;
        apply_environment(
            blend,
            state.depth,
            crate::scene::surface_fog_range(range, config.client.fog_distance),
            &mut fog,
            &mut grading,
            &mut ambient,
        );
        fog.color = medium_fog(daylight.fog, medium_color, blend);
        ambient.brightness *= daylight.ambient / SURFACE_AMBIENT;
        ambient.color = Color::srgb(
            daylight.ambient_color.x,
            daylight.ambient_color.y,
            daylight.ambient_color.z,
        );
        if let Some(weather) = weather.as_deref() {
            crate::weather::apply_environment(
                weather,
                blend,
                cave.blend,
                &mut fog,
                &mut grading,
                &mut ambient,
            );
        }
        crate::scene::caves::apply(cave.blend, blend, &mut fog, &mut grading, &mut ambient);
        apply_night_vision(
            vision,
            (1.0 - daylight.daylight).max(cave.blend),
            &mut ambient,
        );
    }
}

/// View-local fill preserves actual world lights, shadows and other players' night vision.
fn apply_night_vision(strength: f32, darkness: f32, ambient: &mut AmbientLight) {
    let strength = strength.clamp(0.0, 8.0) * darkness.clamp(0.0, 1.0);
    if strength > 0.0 {
        ambient.brightness *= 1.0 + strength;
        ambient.color = ambient
            .color
            .mix(&Color::srgb(0.83, 0.93, 1.0), strength / 10.0);
    }
}

fn underwater_color(tint: [f32; 3]) -> Vec3 {
    Vec3::from_array(tint) * 0.62 + Vec3::new(0.010, 0.044, 0.093)
}

fn medium_scattering(tint: [f32; 3], daylight: f32, cave: f32) -> Vec4 {
    let skylight = crate::scene::caves::sky_visibility(cave);
    // A cave lake has no sunlit marine backplate or solar caustics. Real
    // point lights still illuminate the geometry through the PBR path.
    (underwater_color(tint) * (0.23 + 0.77 * daylight) * skylight).extend(daylight * skylight)
}

fn medium_fog(surface_srgb: Vec3, water_linear: Vec3, blend: f32) -> Color {
    // Biome water scattering is already linear HDR. Encoding it as sRGB again
    // crushed the submerged horizon into the ACES toe and made it look black.
    let surface = Color::srgb(surface_srgb.x, surface_srgb.y, surface_srgb.z)
        .to_linear()
        .to_vec4()
        .truncate();
    let mixed = surface.lerp(water_linear, blend);
    Color::linear_rgb(mixed.x, mixed.y, mixed.z)
}

fn apply_environment(
    blend: f32,
    depth: f32,
    surface_range: (f32, f32),
    fog: &mut DistanceFog,
    grading: &mut ColorGrading,
    ambient: &mut AmbientLight,
) {
    let color = SURFACE_FOG.lerp(Vec3::new(0.035, 0.22, 0.20), blend);
    fog.color = Color::srgb(color.x, color.y, color.z);
    fog.directional_light_color = Color::NONE;
    let underwater_range = (36.0 - depth.max(0.0) * 0.45).clamp(17.0, 36.0);
    fog.falloff = FogFalloff::Linear {
        start: surface_range.0 * (1.0 - blend) + 1.5 * blend,
        end: surface_range.1 * (1.0 - blend) + underwater_range * blend,
    };
    grading.global.temperature = SURFACE_TEMPERATURE * (1.0 - blend) - 0.012 * blend;
    grading.global.tint = -0.004 * blend;
    grading.global.exposure = -0.12 * blend;
    grading.global.post_saturation = SURFACE_SATURATION * (1.0 - blend) + 1.01 * blend;
    ambient.brightness =
        SURFACE_AMBIENT * (1.0 - blend) + (565.0 - depth.clamp(0.0, 48.0) * 3.0) * blend;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_vision_is_local_level_scaled_and_does_not_brighten_daytime() {
        for base in [25.2, 300.0, 760.0] {
            let initial = AmbientLight {
                brightness: base,
                ..default()
            };
            let mut daylight = initial.clone();
            apply_night_vision(6.0, 0.0, &mut daylight);
            assert_eq!(daylight.brightness, initial.brightness);
            assert_eq!(daylight.color, initial.color);
            let mut first = initial.clone();
            apply_night_vision(4.0, 1.0, &mut first);
            let mut second = initial.clone();
            apply_night_vision(6.0, 1.0, &mut second);
            assert_eq!(first.brightness, base * 5.0);
            assert_eq!(second.brightness, base * 7.0);
            let mut expired = initial.clone();
            apply_night_vision(0.0, 1.0, &mut expired);
            assert_eq!(expired.brightness, initial.brightness);
            assert_eq!(expired.color, initial.color);
        }
    }

    #[test]
    fn enclosed_cave_water_has_no_solar_caustics_or_luminous_marine_backplate() {
        for daylight in [0.0, 0.5, 1.0] {
            assert_eq!(
                medium_scattering(super::super::biome::DEFAULT_TINT, daylight, 1.0),
                Vec4::ZERO
            );
            let open = medium_scattering(super::super::biome::DEFAULT_TINT, daylight, 0.0);
            let entrance = medium_scattering(super::super::biome::DEFAULT_TINT, daylight, 0.5);
            assert_eq!(entrance, open * 0.5);
            assert_eq!(open.w, daylight);
        }
    }

    #[test]
    fn emerging_restores_surface_style_and_fog() {
        let mut fog = DistanceFog::default();
        let mut grading = crate::scene::surface_grading();
        let baseline = grading.clone();
        let mut ambient = AmbientLight::default();
        let range = crate::scene::surface_fog_range(192.0, 1.0);
        apply_environment(1.0, 5.0, range, &mut fog, &mut grading, &mut ambient);
        assert!(grading.global.temperature < 0.0);
        assert!(ambient.brightness < SURFACE_AMBIENT);
        apply_environment(0.0, -1.0, range, &mut fog, &mut grading, &mut ambient);
        assert_eq!(grading.global.temperature, baseline.global.temperature);
        assert_eq!(
            grading.global.post_saturation,
            baseline.global.post_saturation
        );
        assert_eq!(grading.global.exposure, baseline.global.exposure);
        assert_eq!(grading.midtones, baseline.midtones);
        assert_eq!(ambient.brightness, SURFACE_AMBIENT);
        assert_eq!(
            fog.color,
            Color::srgb(SURFACE_FOG.x, SURFACE_FOG.y, SURFACE_FOG.z)
        );
        assert!(matches!(fog.falloff, FogFalloff::Linear { start, end }
            if (start - 192.0 * 0.65).abs() < 1e-5 && (end - 192.0 * 0.93).abs() < 1e-5));
    }

    #[test]
    fn depth_preserves_near_reef_color_and_reduces_distant_visibility() {
        let range = crate::scene::surface_fog_range(192.0, 1.0);
        let mut fog = DistanceFog::default();
        let mut grading = crate::scene::surface_grading();
        let mut ambient = AmbientLight::default();
        apply_environment(1.0, 1.0, range, &mut fog, &mut grading, &mut ambient);
        let FogFalloff::Linear {
            start,
            end: shallow,
        } = fog.falloff
        else {
            panic!("Linear water fog")
        };
        let clear_fill = ambient.brightness;
        assert_eq!(start, 1.5);
        assert!(shallow >= 35.0 && grading.global.post_saturation >= 1.0);
        apply_environment(1.0, 64.0, range, &mut fog, &mut grading, &mut ambient);
        let FogFalloff::Linear { end: deep, .. } = fog.falloff else {
            panic!("Linear water fog")
        };
        assert!(deep >= 17.0 && deep < shallow);
        assert!(ambient.brightness < clear_fill);
        let purple = underwater_color(super::super::biome::COLORS[5]);
        assert!(purple.z > purple.y && purple.x > purple.y);
        assert!(
            water_surface(
                &WorldTerrain(terra_world::TerraGenerator::new(Default::default()).unwrap()),
                &ChunkStore::default(),
                &EditJournal::default(),
                [-1, 0, -17]
            )
            .is_none()
        );
    }

    #[test]
    fn medium_scattering_stays_linear_without_changing_dry_surface_color() {
        let water = underwater_color(super::super::biome::DEFAULT_TINT);
        let submerged = medium_fog(SURFACE_FOG, water, 1.0).to_linear();
        assert!(
            Vec3::new(submerged.red, submerged.green, submerged.blue).abs_diff_eq(water, 0.00001)
        );
        assert!(submerged.green > 0.15 && submerged.blue > 0.18);
        let dry = medium_fog(SURFACE_FOG, water, 0.0).to_linear();
        let expected = Color::srgb(SURFACE_FOG.x, SURFACE_FOG.y, SURFACE_FOG.z).to_linear();
        assert_eq!(dry, expected);
        let halfway = medium_fog(SURFACE_FOG, water, 0.5).to_linear();
        assert!((halfway.blue - (expected.blue + submerged.blue) * 0.5).abs() < 0.00001);
    }
}
