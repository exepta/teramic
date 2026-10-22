//! A guarded extension of Bevy's clustered point-light attenuation, not another
//! lighting pass. One reserved radius value marks Terra's flame emitters; every
//! ordinary point/spot light retains Bevy's exact inverse-square path.
use super::*;
use crate::blocks::definitions::LightFalloff;
use bevy::shader::{Shader, ShaderImport, Source};

// Exact binary fraction, reserved internally; never exposed as a physical size.
// Bevy 0.19.1: range drives bounds/shadow frusta; only the three decoded PBR
// radius reads consume this marker. Keep PCSS and source-radius gizmos disabled
// on these lights. They deliberately do not participate in volumetric fog.
const TORCH_RADIUS_CODE: f32 = 128.375;
const WIDENED_FLUX: f32 = 8.0;
const WGSL: &str = include_str!("soft_core.wgsl");
const ATTENUATION: &str = "let rangeAttenuation = getDistanceAttenuation(distance_square, (*light).color_inverse_square_range.w);";
const RADIUS_ARGUMENT: &str = "(*light).position_radius.w,";
const RADIUS_LOCAL: &str = "let light_radius = (*light).position_radius.w;";
const RADIUS_READ: &str = "(*light).position_radius.w";
const IMPORT_PATH: &str = "bevy_pbr::lighting";

#[derive(Resource, Default)]
pub struct TorchLightingStatus {
    pub ready: bool,
    failed: bool,
}

impl TorchLightingStatus {
    pub fn radius(&self, light: BlockLight) -> f32 {
        if self.ready && light.falloff == LightFalloff::SoftCore {
            TORCH_RADIUS_CODE
        } else {
            0.38
        }
    }
    pub fn flux(&self, light: BlockLight) -> f32 {
        if self.ready && light.falloff == LightFalloff::SoftCore {
            WIDENED_FLUX
        } else {
            1.0
        }
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<TorchLightingStatus>()
        .add_systems(PreUpdate, prepare);
}

fn prepare(mut shaders: ResMut<Assets<Shader>>, mut state: ResMut<TorchLightingStatus>) {
    if state.ready || state.failed {
        return;
    }
    let found = shaders.iter().find_map(|(id, shader)| {
        matches!(&shader.import_path, ShaderImport::Custom(path) if path == IMPORT_PATH)
            .then_some(id)
    });
    let Some(id) = found else {
        return;
    };
    let Some(shader) = shaders.get(id) else {
        return;
    };
    let Source::Wgsl(source) = &shader.source else {
        state.failed = true;
        error!("Torch soft-core lighting requires Bevy's WGSL PBR library");
        return;
    };
    match extend(source) {
        Ok(source) => {
            if let Some(mut shader) = shaders.get_mut(id) {
                shader.source = Source::Wgsl(source.into());
                state.ready = true;
                info!("Torch soft-core attenuation installed in existing clustered PBR path");
            }
        }
        Err(reason) => {
            state.failed = true;
            error!(%reason, "Torch attenuation extension refused incompatible Bevy shader; using stock point lighting");
        }
    }
}

fn extend(source: &str) -> Result<String, &'static str> {
    let start = source
        .find("fn point_light(\n")
        .ok_or("Missing point_light boundary")?;
    let end = source[start..]
        .find("fn spot_light(\n")
        .map(|end| start + end)
        .ok_or("Missing spot_light boundary")?;
    let point = &source[start..end];
    if point.matches(ATTENUATION).count() != 1
        || point.matches(RADIUS_ARGUMENT).count() != 2
        || point.matches(RADIUS_LOCAL).count() != 1
        || point.matches(RADIUS_READ).count() != 3
    {
        return Err("Bevy 0.19 point-light signature changed");
    }
    let point = point.replace(ATTENUATION, "let rangeAttenuation = terra_torch_attenuation(distance_square, (*light).color_inverse_square_range.w, (*light).position_radius.w);")
        .replace(RADIUS_ARGUMENT, "terra_physical_light_radius((*light).position_radius.w),")
        .replace(RADIUS_LOCAL, "let light_radius = terra_physical_light_radius((*light).position_radius.w);");
    Ok(format!(
        "{}\n{WGSL}\n{point}{}",
        &source[..start],
        &source[end..]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_bevys_declared_module_name_not_the_wgsl_filename() {
        let shader = Shader::from_wgsl(
            "#define_import_path bevy_pbr::lighting\n",
            "embedded://bevy_pbr/render/pbr_lighting.wgsl",
        );
        assert!(matches!(shader.import_path, ShaderImport::Custom(path) if path == IMPORT_PATH));
    }
    #[test]
    fn soft_core_is_explicit_and_independent_of_fire_animation() {
        let mut light: BlockLight =
            serde_json::from_str(r#"{"color":[1.0,0.8,0.5],"intensity":1450.0,"range":28.0}"#)
                .unwrap();
        let state = TorchLightingStatus {
            ready: true,
            failed: false,
        };
        assert_eq!(light.falloff, LightFalloff::InverseSquare);
        light.flicker = 0.09;
        assert_eq!(state.flux(light), 1.0);
        assert_eq!(state.radius(light), 0.38);
        light.falloff = LightFalloff::SoftCore;
        light.flicker = 0.0;
        assert_eq!(state.flux(light), WIDENED_FLUX);
        assert_eq!(state.radius(light), TORCH_RADIUS_CODE);
        assert_eq!(TorchLightingStatus::default().flux(light), 1.0);
        assert_eq!(TorchLightingStatus::default().radius(light), 0.38);
    }
    fn attenuation(distance: f32, soft: bool) -> f32 {
        let range = (1.0 - (distance / 28.0).powi(4)).clamp(0.0, 1.0).powi(2);
        let t = ((distance - 1.0) / 3.0).clamp(0.0, 1.0);
        let spill = if soft {
            1.0 + 0.2 * t * t * (3.0 - 2.0 * t)
        } else {
            1.0
        };
        spill * range / (distance * distance + if soft { 16.0 } else { 0.0 }).max(0.0001)
    }
    #[test]
    fn torch_spill_is_twenty_percent_brighter_without_strengthening_the_hotspot() {
        for distance in [0.25_f32, 0.5, 1.0, 4.0, 10.0, 18.0] {
            let original = (1.0 - (distance / 28.0).powi(4)).clamp(0.0, 1.0).powi(2)
                / (distance * distance + 16.0);
            let expected = if distance <= 1.0 { 1.0 } else { 1.2 };
            assert!((attenuation(distance, true) / original - expected).abs() < 0.000_01);
        }
        assert!(WGSL.contains("0.2 * smoothstep(1.0, 4.0, sqrt(distance_squared))"));
    }
    #[test]
    fn broad_spill_reduces_the_hotspot_without_more_lights_or_a_hard_range_edge() {
        assert!(WIDENED_FLUX * attenuation(0.5, true) < attenuation(0.5, false) * 0.13);
        assert!(WIDENED_FLUX * attenuation(1.0, true) < attenuation(1.0, false) * 0.50);
        for distance in [6.0, 10.0, 18.0] {
            assert!(
                WIDENED_FLUX * attenuation(distance, true) > attenuation(distance, false) * 5.0
            );
        }
        assert_eq!(attenuation(28.0, true), 0.0);
        assert!(attenuation(27.9, true) < attenuation(27.0, true));
        assert!(WGSL.contains("encoded_radius == 128.375"));
    }
    #[test]
    fn shader_extension_is_strict_and_preserves_other_lighting_functions() {
        let source = format!(
            "// untouched header\nfn point_light(\n{ATTENUATION}\n{RADIUS_ARGUMENT}\n{RADIUS_ARGUMENT}\n{RADIUS_LOCAL}\n)\nfn spot_light(\n// untouched tail"
        );
        let patched = extend(&source).unwrap();
        assert!(patched.starts_with("// untouched header"));
        assert!(patched.ends_with("fn spot_light(\n// untouched tail"));
        assert_eq!(
            patched
                .matches("terra_physical_light_radius((*light).position_radius.w)")
                .count(),
            3
        );
        assert!(extend(&source.replace(ATTENUATION, "new API")).is_err());
        assert!(
            extend(&source.replace(
                RADIUS_LOCAL,
                &format!("{RADIUS_LOCAL}\nlet new_radius_read = {RADIUS_READ};")
            ))
            .is_err()
        );
        assert!(extend(&patched).is_err());
    }

    #[test]
    fn preparation_patches_only_the_lighting_module_once_and_fails_closed() {
        let source = format!(
            "#define_import_path {IMPORT_PATH}\nfn point_light(\n{ATTENUATION}\n{RADIUS_ARGUMENT}\n{RADIUS_ARGUMENT}\n{RADIUS_LOCAL}\n)\nfn spot_light(\n// untouched tail"
        );
        let mut app = App::new();
        app.init_resource::<Assets<Shader>>()
            .init_resource::<TorchLightingStatus>()
            .add_systems(Update, prepare);
        let other = app
            .world_mut()
            .resource_mut::<Assets<Shader>>()
            .add(Shader::from_wgsl(
                "#define_import_path unrelated::lighting\n",
                "other.wgsl",
            ));
        app.update();
        assert!(!app.world().resource::<TorchLightingStatus>().ready);
        let lighting = app
            .world_mut()
            .resource_mut::<Assets<Shader>>()
            .add(Shader::from_wgsl(source, "pbr_lighting.wgsl"));
        app.update();
        assert!(app.world().resource::<TorchLightingStatus>().ready);
        for _ in 0..3 {
            app.update();
        }
        let shaders = app.world().resource::<Assets<Shader>>();
        let Source::Wgsl(patched) = &shaders.get(&lighting).unwrap().source else {
            panic!("Expected WGSL")
        };
        assert_eq!(patched.matches("fn terra_torch_attenuation(").count(), 1);
        let Source::Wgsl(unrelated) = &shaders.get(&other).unwrap().source else {
            panic!("Expected WGSL")
        };
        assert_eq!(&**unrelated, "#define_import_path unrelated::lighting\n");

        let mut app = App::new();
        app.init_resource::<Assets<Shader>>()
            .init_resource::<TorchLightingStatus>()
            .add_systems(Update, prepare);
        let incompatible = format!("#define_import_path {IMPORT_PATH}\n// changed engine API\n");
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Shader>>()
            .add(Shader::from_wgsl(incompatible.clone(), "new_engine.wgsl"));
        app.update();
        let status = app.world().resource::<TorchLightingStatus>();
        assert!(!status.ready && status.failed);
        let Source::Wgsl(unchanged) = &app
            .world()
            .resource::<Assets<Shader>>()
            .get(&handle)
            .unwrap()
            .source
        else {
            panic!("Expected WGSL")
        };
        assert_eq!(&**unchanged, incompatible);
    }
}
