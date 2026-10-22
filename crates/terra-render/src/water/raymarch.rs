//! Keep Bevy's SSR traversal, using its native hardware-depth filtering path
//! with the equivalent linear/clamp sampler already bound for transmission.
use bevy::{
    pbr::MeshPipelineKey,
    prelude::*,
    shader::{Shader, ShaderImport, Source},
};

const ORIGINAL_PATH: &str = "#define_import_path bevy_pbr::raymarch";
const WATER_PATH: &str = "#define_import_path terra::water_raymarch";
const LINEAR: &str = "fn depth_sample_linear(uv: vec2<f32>, tex_size: vec2<f32>) -> f32 {
#ifdef USE_DEPTH_SAMPLERS
    return textureSampleLevel(depth_prepass_texture, depth_linear_sampler, uv, 0u);
#else
    return depth_sample_bilinear_clamped(uv, tex_size);
#endif
}";
const HARDWARE_LINEAR: &str = "fn depth_sample_linear(uv: vec2<f32>, tex_size: vec2<f32>) -> f32 {
    return textureSampleLevel(depth_prepass_texture, view_transmission_sampler, uv, 0u);
}";
const NEAREST: &str = "return depth_sample_nearest_clamped(uv, tex_size);";

#[derive(Resource, Default)]
pub(super) struct WaterRaymarch {
    pub ready: bool,
    finished: bool,
    shader: Option<Handle<Shader>>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<WaterRaymarch>()
        .add_systems(PreUpdate, prepare);
}

fn prepare(mut shaders: ResMut<Assets<Shader>>, mut state: ResMut<WaterRaymarch>) {
    if state.finished {
        return;
    }
    let source = shaders.iter().find_map(|(_, shader)| {
        matches!(&shader.import_path, ShaderImport::Custom(path) if path == "bevy_pbr::raymarch")
            .then_some(&shader.source)
    });
    let Some(source) = source else {
        return;
    };
    let Source::Wgsl(source) = source else {
        state.finished = true;
        return;
    };
    let hardware = !(cfg!(target_arch = "wasm32")
        || std::env::var("TERRA_BENCH_WATER_SOFTWARE_DEPTH").is_ok_and(|v| v == "1")
            && std::env::args().any(|arg| arg.starts_with("--smoke-test")));
    let (source, ready, fallback) = variant(source, hardware);
    // Bevy resolves imports before evaluating conditional shader definitions.
    // Even views using stock sampling need this alias registered, or all water
    // pipelines can remain pending without reporting a shader error.
    state.shader = Some(shaders.add(Shader::from_wgsl(
        source,
        "terra/water_native_raymarch.wgsl",
    )));
    state.ready = ready;
    if ready {
        info!(
            "Water SSR hardware depth filtering ready; multisampled views retain the original path"
        );
    } else if let Some(reason) = fallback {
        warn!(%reason, "Water SSR retains Bevy's original depth sampling");
    }
    state.finished = true;
}

fn variant(source: &str, hardware: bool) -> (String, bool, Option<&'static str>) {
    let fallback = if hardware {
        match extend(source) {
            Ok(source) => return (source, true, None),
            Err(reason) => Some(reason),
        }
    } else {
        None
    };
    // Only rename the module. Helper drift disables the optimization without
    // changing the stock traversal or modifying Bevy's shared shader asset.
    let body = source
        .lines()
        .filter(|line| line.split_whitespace().next() != Some("#define_import_path"))
        .collect::<Vec<_>>()
        .join("\n");
    (format!("{WATER_PATH}\n{body}"), false, fallback)
}

fn extend(source: &str) -> Result<String, &'static str> {
    if source.matches(ORIGINAL_PATH).count() != 1
        || source.matches(LINEAR).count() != 1
        || source.matches(NEAREST).count() != 1
    {
        return Err("Unrecognized Bevy depth sampler helpers");
    }
    Ok(source
        .replace(
            ORIGINAL_PATH,
            &format!(
                "{WATER_PATH}\n#import bevy_pbr::mesh_view_bindings::view_transmission_sampler"
            ),
        )
        .replace(LINEAR, HARDWARE_LINEAR))
}

pub(super) fn enabled(ready: bool, key: MeshPipelineKey) -> bool {
    ready
        && !cfg!(target_arch = "wasm32")
        && key.msaa_samples() == 1
        && key.contains(
            MeshPipelineKey::DEPTH_PREPASS | MeshPipelineKey::READS_VIEW_TRANSMISSION_TEXTURE,
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_variant_keeps_the_ray_traversal_and_nearest_sample_unchanged() {
        let traversal = "fn depth_ray_march_march() { let steps = 28u; }";
        let source = format!("{ORIGINAL_PATH}\n{LINEAR}\n{NEAREST}\n{traversal}");
        let native = extend(&source).unwrap();
        assert!(native.contains(WATER_PATH));
        assert!(!native.contains(ORIGINAL_PATH));
        assert!(native.contains(HARDWARE_LINEAR));
        assert!(native.contains(NEAREST));
        assert!(native.ends_with(traversal));
        assert!(
            source.contains(ORIGINAL_PATH),
            "Never mutate Bevy's shared library"
        );
    }

    #[test]
    fn incompatible_shader_helpers_cannot_enable_a_partial_patch() {
        let source = format!("{ORIGINAL_PATH}\n{LINEAR}\n{NEAREST}");
        assert!(extend(&source.replace(NEAREST, "changed")).is_err());
        assert!(extend(&source.replace(LINEAR, "changed")).is_err());
        assert!(extend(&format!("{source}\n{ORIGINAL_PATH}")).is_err());
    }

    #[test]
    fn software_and_incompatible_variants_still_register_the_required_import() {
        let traversal = "fn depth_ray_march_march() { let steps = 28u; }";
        let source = format!("{ORIGINAL_PATH}\n{LINEAR}\n{NEAREST}\n{traversal}");
        for (source, hardware, warned) in [
            (source.clone(), false, false),
            (source.replace(NEAREST, "changed_helper"), true, true),
            (source.replace(LINEAR, "changed_linear_helper"), true, true),
            (source.replace(ORIGINAL_PATH, ""), true, true),
        ] {
            let (alias, ready, fallback) = variant(&source, hardware);
            assert!(!ready);
            assert_eq!(fallback.is_some(), warned);
            let shader = Shader::from_wgsl(alias.clone(), "water_fallback.wgsl");
            assert!(matches!(
                shader.import_path,
                ShaderImport::Custom(path) if path == "terra::water_raymarch"
            ));
            assert_eq!(alias.matches(WATER_PATH).count(), 1);
            assert!(!alias.contains(ORIGINAL_PATH));
            assert!(alias.ends_with(traversal));
            assert!(!alias.contains("view_transmission_sampler"));
            for line in source
                .lines()
                .filter(|line| line.split_whitespace().next() != Some("#define_import_path"))
            {
                assert!(
                    alias.contains(line),
                    "Fallback must preserve the stock shader"
                );
            }
        }
    }

    #[test]
    fn recognized_native_variant_enables_only_the_complete_optimization() {
        let source = format!("{ORIGINAL_PATH}\n{LINEAR}\n{NEAREST}");
        let (alias, ready, fallback) = variant(&source, true);
        assert!(ready);
        assert!(fallback.is_none());
        assert!(alias.contains(HARDWARE_LINEAR));
        assert!(alias.contains(NEAREST));
        assert_eq!(alias.matches(WATER_PATH).count(), 1);
        assert!(source.contains(LINEAR));
    }

    #[test]
    fn delayed_stock_library_registers_an_alias_even_when_helpers_are_incompatible() {
        let mut app = App::new();
        app.init_resource::<Assets<Shader>>();
        install(&mut app);
        app.update();
        assert!(!app.world().resource::<WaterRaymarch>().finished);
        let source = format!("{ORIGINAL_PATH}\nfn changed_helper() {{}}");
        let original = app
            .world_mut()
            .resource_mut::<Assets<Shader>>()
            .add(Shader::from_wgsl(source.clone(), "stock.wgsl"));
        app.update();
        let state = app.world().resource::<WaterRaymarch>();
        assert!(state.finished);
        assert!(!state.ready);
        let shaders = app.world().resource::<Assets<Shader>>();
        let alias = shaders.get(state.shader.as_ref().unwrap()).unwrap();
        assert!(matches!(
            &alias.import_path,
            ShaderImport::Custom(path) if path == "terra::water_raymarch"
        ));
        assert!(matches!(
            &shaders.get(&original).unwrap().source,
            Source::Wgsl(stock) if stock.as_ref() == source
        ));
        app.update();
        assert_eq!(app.world().resource::<Assets<Shader>>().len(), 2);
    }

    #[test]
    fn unavailable_library_msaa_and_non_transmissive_views_use_original_sampler() {
        let key = MeshPipelineKey::DEPTH_PREPASS
            | MeshPipelineKey::READS_VIEW_TRANSMISSION_TEXTURE
            | MeshPipelineKey::from_msaa_samples(1);
        assert_eq!(enabled(true, key), !cfg!(target_arch = "wasm32"));
        assert!(!enabled(false, key));
        for samples in [2, 4, 8] {
            assert!(!enabled(
                true,
                key | MeshPipelineKey::from_msaa_samples(samples)
            ));
        }
        assert!(!enabled(true, MeshPipelineKey::DEPTH_PREPASS));
        assert!(!enabled(
            true,
            MeshPipelineKey::READS_VIEW_TRANSMISSION_TEXTURE
        ));
    }
}
