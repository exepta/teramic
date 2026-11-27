//! Catch preprocessor and import syntax regressions before starting the GPU.

#[path = "shader_imprint_tests.rs"]
mod imprint;

#[test]
fn all_custom_shader_modules_parse_as_wesl() {
    let directory = terra_core::asset_directory().join("shaders");
    let mut checked = 0;
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "wesl") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        source
            .parse::<wesl::syntax::TranslationUnit>()
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(!source.contains("#import"), "{}", path.display());
        assert!(!source.contains("#ifdef"), "{}", path.display());
        assert!(!source.contains("#{"), "{}", path.display());
        checked += 1;
    }
    assert_eq!(checked, 28); // Includes the breaking surf material.
}

#[test]
fn footprint_depth_and_color_use_identical_bounded_cutout_math_in_both_binding_modes() {
    use wesl::{
        pass::{Features, condcomp},
        syntax::{GlobalDeclaration, TranslationUnit},
    };
    let color = include_str!("../../../assets/shaders/terra_wet_surface.wesl");
    let depth = include_str!("../../../assets/shaders/terra_imprint_prepass.wesl");
    for bindless in [false, true] {
        let mut bodies = Vec::new();
        for source in [color, depth] {
            let mut module = source.parse::<TranslationUnit>().unwrap();
            let mut features = Features::new();
            features.set("BINDLESS", bindless);
            features.set("VERTEX_UVS_B", true);
            condcomp(&mut module, &features).unwrap();
            let function = module
                .global_declarations
                .iter()
                .find_map(|d| match &**d {
                    GlobalDeclaration::Function(f)
                        if f.ident.name().as_str() == "imprint_cutout" =>
                    {
                        Some(f.body.to_string())
                    }
                    _ => None,
                })
                .unwrap();
            assert!(function.contains("i < 4u"));
            assert!(function.contains("% 128u"));
            assert!(function.contains("abs(p.y - a.z) < 0.006"));
            bodies.push(function);
        }
        assert_eq!(bodies[0], bodies[1]);
    }
}

#[test]
fn instance_rgb_ribbon_keeps_forward_deferred_and_default_shadow_interfaces_separate() {
    use wesl::{
        pass::{Features, condcomp},
        syntax::TranslationUnit,
    };
    let source = include_str!("../../../assets/shaders/terra_ribbon_tint.wesl");
    for deferred in [false, true] {
        for dither in [false, true] {
            let mut module = source.parse::<TranslationUnit>().unwrap();
            let mut features = Features::new();
            features.set("PREPASS_PIPELINE", deferred);
            features.set("VISIBILITY_RANGE_DITHER", dither);
            condcomp(&mut module, &features).unwrap();
            let text = module.to_string();
            assert_eq!(text.contains("import bevy_pbr::prepass::io"), deferred);
            assert_eq!(
                text.contains("import bevy_pbr::render::forward_io"),
                !deferred
            );
            assert_eq!(text.contains("return deferred_output"), deferred);
            assert_eq!(text.contains("out.color = apply_pbr_lighting"), !deferred);
            assert_eq!(text.contains("visibility_range_dither(input"), dither);
            assert!(text.contains("get_tag(input.instance_index)"));
            for contract in [
                "srgb <= 0.04045",
                "srgb / 12.92",
                "pow((srgb + 0.055) / 1.055, 2.4)",
                "linear_channel(srgb.r)",
                "linear_channel(srgb.g)",
                "linear_channel(srgb.b)",
            ] {
                assert!(
                    text.contains(contract),
                    "ribbon RGB must use the same sRGB transfer as CPU tints: {contract}"
                );
            }
            assert!(text.contains("alpha_discard"));
            assert!(text.contains("apply_decals"));
            assert!(
                !text.contains("@group"),
                "RGB never introduces a per-color binding"
            );
            assert!(
                !text.lines().any(|line| line.trim() == "discard;"),
                "RGB must not modify opaque coverage"
            );
        }
    }
}

#[test]
fn water_reads_the_opaque_snapshot_only_when_the_view_binds_one() {
    use wesl::{
        pass::{Features, condcomp},
        syntax::{GlobalDeclaration, TranslationUnit},
    };
    let source = include_str!("../../../assets/shaders/terra_water.wesl");
    for transmission in [false, true] {
        for prepass in [false, true] {
            for native in [false, true] {
                let mut module = source.parse::<TranslationUnit>().unwrap();
                let mut features = Features::new();
                features.set("VIEW_TRANSMISSION_TEXTURE", transmission);
                features.set("DEPTH_PREPASS", prepass);
                features.set("WATER_NATIVE_DEPTH_FILTER", native);
                condcomp(&mut module, &features).unwrap();
                let resolved = module.to_string();
                assert_eq!(
                    resolved.contains(
                        "import bevy_pbr::transmission::fetch_transmissive_background_non_rough"
                    ),
                    transmission
                );
                assert_eq!(
                    resolved.contains("import terra::water_raymarch"),
                    transmission && prepass && native
                );
                assert_eq!(
                    resolved.contains("import bevy_pbr::ssr::raymarch"),
                    transmission && prepass && !native
                );
                let function = |name: &str| {
                    module
                        .global_declarations
                        .iter()
                        .find_map(|declaration| match &**declaration {
                            GlobalDeclaration::Function(function)
                                if function.ident.name().as_str() == name =>
                            {
                                Some(function.body.to_string())
                            }
                            _ => None,
                        })
                        .unwrap()
                };
                let fragment = function("fragment");
                let reflection = function("scene_reflection");
                assert_eq!(
                    fragment.contains("fetch_transmissive_background_non_rough"),
                    transmission
                );
                assert_eq!(
                    reflection.contains("fetch_transmissive_background_non_rough"),
                    transmission && prepass
                );
                assert_eq!(fragment.contains("prepass_depth(input.position"), prepass);
                for operation in [
                    "apply_pbr_lighting",
                    "main_pass_post_lighting_processing",
                    "scatter",
                    "foam",
                ] {
                    assert!(
                        fragment.contains(operation),
                        "all views retain water lighting: {operation}"
                    );
                }
                if !transmission {
                    for unavailable in [
                        "fetch_transmissive_background",
                        "depth_ray_march",
                        "view_transmission_",
                    ] {
                        assert!(
                            !resolved.contains(unavailable),
                            "secondary view references absent snapshot resources: {unavailable}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn dry_surface_variant_preserves_stock_pbr_without_detail_or_weather_work() {
    use wesl::{
        pass::{Features, condcomp},
        syntax::{GlobalDeclaration, TranslationUnit},
    };
    let source = include_str!("../../../assets/shaders/terra_wet_surface.wesl");
    for prepass in [false, true] {
        for payload in [false, true] {
            let mut module = source.parse::<TranslationUnit>().unwrap();
            let mut features = Features::new();
            features.set("VERTEX_UVS_B", payload);
            features.set("VERTEX_COLORS", true);
            features.set("PREPASS_PIPELINE", prepass);
            condcomp(&mut module, &features).unwrap();
            let fragment = module
                .global_declarations
                .iter()
                .find_map(|declaration| match &**declaration {
                    GlobalDeclaration::Function(function)
                        if function.ident.name().as_str() == "fragment" =>
                    {
                        Some(function)
                    }
                    _ => None,
                })
                .unwrap();
            let body = fragment.body.to_string();
            for operation in [
                "pbr_input_from_standard_material",
                "alpha_discard",
                "apply_decals",
            ] {
                assert!(
                    body.contains(operation),
                    "Missing stock PBR step {operation}"
                );
            }
            if prepass {
                assert!(body.contains("deferred_output"));
            } else {
                assert!(body.contains("apply_pbr_lighting"));
                assert!(body.contains("main_pass_post_lighting_processing"));
            }
            if payload {
                for operation in ["block_profile", "saved_water", "saved_snow", "film"] {
                    assert!(body.contains(operation), "Lost surface work {operation}");
                }
            } else {
                for operation in [
                    "terra_detail",
                    "terra_weather",
                    "block_profile",
                    "dpdx",
                    "dpdy",
                    "uv_b",
                    "perceptual_roughness",
                ] {
                    assert!(
                        !body.contains(operation),
                        "Dry color path unexpectedly performs {operation}"
                    );
                }
            }
        }
    }
}

#[test]
fn distant_and_disabled_detail_skip_noise_while_derivatives_remain_reconverged() {
    use wesl::{
        pass::{Features, condcomp},
        syntax::{GlobalDeclaration, Statement, TranslationUnit},
    };
    let source = include_str!("../../../assets/shaders/terra_wet_surface.wesl");
    for bindless in [false, true] {
        for prepass in [false, true] {
            let mut module = source.parse::<TranslationUnit>().unwrap();
            let mut features = Features::new();
            for (name, value) in [
                ("BINDLESS", bindless),
                ("PREPASS_PIPELINE", prepass),
                ("VERTEX_UVS_B", true),
                ("VERTEX_COLORS", true),
            ] {
                features.set(name, value);
            }
            condcomp(&mut module, &features).unwrap();
            let fragment = module
                .global_declarations
                .iter()
                .find_map(|declaration| match &**declaration {
                    GlobalDeclaration::Function(function)
                        if function.ident.name().as_str() == "fragment" =>
                    {
                        Some(function)
                    }
                    _ => None,
                })
                .unwrap();
            let detail = fragment
                .body
                .statements
                .iter()
                .find_map(|statement| match &**statement {
                    Statement::If(statement)
                        if statement
                            .if_clause
                            .expression
                            .to_string()
                            .contains("terra_detail.x > 0.5") =>
                    {
                        Some(&statement.if_clause.body)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(detail.to_string().matches("block_profile(").count(), 1);
            let (gate_index, gate) = detail
                .statements
                .iter()
                .enumerate()
                .find_map(|(index, statement)| match &**statement {
                    Statement::If(statement)
                        if statement
                            .if_clause
                            .body
                            .to_string()
                            .contains("block_profile(") =>
                    {
                        Some((index, statement))
                    }
                    _ => None,
                })
                .expect("costly profile must be inside a fade/output gate");
            let condition = gate.if_clause.expression.to_string();
            assert!(condition.contains("detail_fade > 0.0001"));
            assert!(condition.contains("terra_detail.y > 0.0 || terra_detail.z > 0.0"));
            let guarded = gate.if_clause.body.to_string();
            for forbidden in ["dpdx(", "dpdy(", "texture", "return", "discard"] {
                assert!(
                    !guarded.contains(forbidden),
                    "divergent profile gate contains {forbidden}"
                );
            }
            assert!(gate.else_if_clauses.is_empty() && gate.else_clause.is_none());
            let before = detail.statements[..gate_index]
                .iter()
                .map(ToString::to_string)
                .collect::<String>();
            let after = detail.statements[gate_index + 1..]
                .iter()
                .map(ToString::to_string)
                .collect::<String>();
            assert!(before.contains("dpdx(uv)") && before.contains("dpdy(uv)"));
            assert!(
                before.contains("vec3<f32>(0.0, 1.0, 1.0)"),
                "far/disabled detail must have neutral height and tint"
            );
            assert!(after.contains("dpdx(height)") && after.contains("dpdy(height)"));
            // Every function reachable from the divergent profile call is pure
            // arithmetic: it cannot smuggle implicit sampling/derivatives into it.
            for name in ["block_profile", "wet_noise", "wet_hash"] {
                let body = module
                    .global_declarations
                    .iter()
                    .find_map(|declaration| match &**declaration {
                        GlobalDeclaration::Function(function)
                            if function.ident.name().as_str() == name =>
                        {
                            Some(function.body.to_string())
                        }
                        _ => None,
                    })
                    .unwrap();
                for forbidden in ["dpdx(", "dpdy(", "texture"] {
                    assert!(
                        !body.contains(forbidden),
                        "{name} contains divergent {forbidden}"
                    );
                }
            }
        }
    }
}

fn authored_grass_palette_parameters(module: &wesl::syntax::TranslationUnit) -> ([f32; 3], f32) {
    use wesl::syntax::{Expression, GlobalDeclaration, LiteralExpression, Statement};
    let function = module
        .global_declarations
        .iter()
        .find_map(|declaration| match &**declaration {
            GlobalDeclaration::Function(function)
                if function.ident.name().as_str() == "authored_grass_top_palette" =>
            {
                Some(function)
            }
            _ => None,
        })
        .unwrap();
    // Inspect the AST, rather than its printer's spelling of 0.30 versus 0.3.
    // A pure mix must preserve the source texel and contain no texture/lighting work.
    assert_eq!(function.body.statements.len(), 1);
    let Statement::Return(returned) = &*function.body.statements[0] else {
        panic!("Palette must return its mix");
    };
    let Expression::FunctionCall(mix) = &**returned.expression.as_ref().unwrap() else {
        panic!("Palette must mix source and target");
    };
    assert_eq!(mix.ty.ident.name().as_str(), "mix");
    assert_eq!(mix.arguments.len(), 3);
    let Expression::TypeOrIdentifier(source) = &*mix.arguments[0] else {
        panic!("Palette must retain source texels");
    };
    assert_eq!(source.ident.name().as_str(), "color");
    let Expression::FunctionCall(target) = &*mix.arguments[1] else {
        panic!("Palette target must be a color");
    };
    assert_eq!(target.ty.ident.name().as_str(), "vec3");
    assert_eq!(target.arguments.len(), 3);
    let scalar = |expression: &wesl::syntax::ExpressionNode| match &**expression {
        Expression::Literal(LiteralExpression::AbstractFloat(value)) => *value as f32,
        Expression::Literal(LiteralExpression::F32(value)) => *value,
        _ => panic!("Palette coefficients must be constant linear albedos"),
    };
    (
        std::array::from_fn(|axis| scalar(&target.arguments[axis])),
        scalar(&mix.arguments[2]),
    )
}

#[test]
fn authored_grass_top_palette_preserves_tints_normals_and_weather_order() {
    use wesl::{
        pass::{Features, condcomp},
        syntax::{GlobalDeclaration, TranslationUnit},
    };
    let source = include_str!("../../../assets/shaders/terra_wet_surface.wesl");
    for colors in [false, true] {
        for prepass in [false, true] {
            let mut module = source.parse::<TranslationUnit>().unwrap();
            let mut features = Features::new();
            features.set("VERTEX_UVS_B", true);
            features.set("VERTEX_COLORS", colors);
            features.set("PREPASS_PIPELINE", prepass);
            condcomp(&mut module, &features).unwrap();
            let function = |name: &str| {
                module
                    .global_declarations
                    .iter()
                    .find_map(|declaration| match &**declaration {
                        GlobalDeclaration::Function(function)
                            if function.ident.name().as_str() == name =>
                        {
                            Some(function.body.to_string())
                        }
                        _ => None,
                    })
                    .unwrap()
            };
            let palette = function("authored_grass_top_palette");
            let (target, weight) = authored_grass_palette_parameters(&module);
            for (actual, expected) in target.into_iter().zip([0.17_f32, 0.30, 0.045]) {
                assert!((actual - expected).abs() < 1e-6);
            }
            assert!((weight - 0.32).abs() < 1e-6);
            for forbidden in ["pbr", "texture", "noise", "emissive"] {
                assert!(!palette.contains(forbidden));
            }
            let body = function("fragment");
            assert_eq!(body.contains("input.color.rgb"), colors);
            assert!(body.contains("normalize(input.world_normal).y"));
            assert!(body.contains("round(input.world_position.y)"));
            assert!(body.contains("green_texel && (top_face || side_face)"));
            assert!(body.contains("side_face && !green_texel"));
            assert!(body.contains("source_color * soil_shade"));
            assert!(body.contains("* vertex_tint"));
            assert!(body.contains("calendar_turf = true"));
            assert!(body.contains("calendar_turf = turf"));
            assert!(body.contains("if calendar_turf"));
            let palette_call = body
                .find("authored_grass_top_palette(source_color)")
                .unwrap();
            let season = body.find("terra_weather.grass.rgb").unwrap();
            let snow = body.find("snow_cover").unwrap();
            assert!(palette_call < season && season < snow);
            assert!(body.contains("pbr.N + vec3<f32>(ripple"));
            assert!(body.contains("pbr.material.perceptual_roughness"));
        }
    }
}

#[test]
fn authored_grass_top_palette_brightens_and_limits_albedo_variation() {
    let module = include_str!("../../../assets/shaders/terra_wet_surface.wesl")
        .parse::<wesl::syntax::TranslationUnit>()
        .unwrap();
    let (target_color, weight) = authored_grass_palette_parameters(&module);
    let retained_weight = 1.0 - weight;
    let palette = |color: [f32; 3]| color.map(|v| v * retained_weight);
    let target = target_color.map(|v| v * weight);
    let apply = |color| {
        let retained = palette(color);
        std::array::from_fn::<_, 3, _>(|axis| retained[axis] + target[axis])
    };
    let luminance = |color: [f32; 3]| color[0] * 0.2126 + color[1] * 0.7152 + color[2] * 0.0722;
    let low = [0.08, 0.18, 0.03];
    let high = [0.20, 0.38, 0.07];
    for color in [low, [0.12, 0.28, 0.04], high] {
        let output = apply(color);
        assert!((luminance(output) - luminance(color)).abs() < 0.04);
        assert!(output[1] > output[0] * 1.12 && output[1] > output[2] * 1.12);
        for tint in [[1.0_f32; 3], [0.8, 1.3, 0.7], [1.3, 0.9, 0.75]] {
            let tinted: [f32; 3] = std::array::from_fn(|axis| color[axis] * tint[axis]);
            let restored: [f32; 3] = std::array::from_fn(|axis| tinted[axis] / tint[axis]);
            let result = apply(restored);
            for axis in 0..3 {
                assert!((result[axis] * tint[axis] - output[axis] * tint[axis]).abs() < 1e-6);
            }
        }
    }
    for axis in 0..3 {
        let variation = apply(high)[axis] - apply(low)[axis];
        assert!((variation - (high[axis] - low[axis]) * retained_weight).abs() < 1e-6);
    }
}
