//! Real embedded PBR imports plus the runtime-only storage mesh-binding adapter.
use bevy::{
    asset::io::Reader,
    prelude::*,
    render::{RenderPlugin, settings::WgpuSettings},
};
use std::{borrow::Cow, cell::RefCell, collections::HashMap, path::PathBuf};
use wesl::{
    Resolver,
    error::ResolveError,
    syntax::{GlobalDeclaration, GlobalDirective, ModulePath, PathOrigin, TranslationUnit},
};

struct EmbeddedShaders {
    server: AssetServer,
    root: String,
    sources: RefCell<HashMap<ModulePath, String>>,
}

impl Resolver for EmbeddedShaders {
    fn resolve_source(&self, path: &ModulePath) -> Result<Cow<'_, str>, ResolveError> {
        let canonical = self.canonical_path(path);
        let path = &canonical;
        if path == &"terra::imprint".parse::<ModulePath>().unwrap() {
            return Ok(Cow::Borrowed(&self.root));
        }
        if path == &"constants".parse::<ModulePath>().unwrap() {
            return Ok(Cow::Borrowed(
                "const MATERIAL_BIND_GROUP = 3u; const MAX_DIRECTIONAL_LIGHTS = 10u; \
                 const MAX_CASCADES_PER_LIGHT = 4u; const MAX_RECT_LIGHTS = 64u;",
            ));
        }
        if path
            == &"bevy_pbr::render::mesh_bindings"
                .parse::<ModulePath>()
                .unwrap()
        {
            // Bevy registers this module during MeshRenderPlugin::finish only
            // with a RenderDevice. Match its storage-buffer variant, importing
            // the real embedded types; all PBR functions remain engine-owned.
            return Ok(Cow::Borrowed(
                "import package::render::mesh_types::Mesh; \
                 import bevy_render::mesh::metadata_types::MeshMetadata; \
                 @group(2) @binding(0) var<storage> mesh: array<Mesh>; \
                 @group(2) @binding(9) var<storage> metadata: array<MeshMetadata>;",
            ));
        }
        if let Some(source) = self.sources.borrow().get(path) {
            return Ok(Cow::Owned(source.clone()));
        }
        // Runtime shaders use Bevy's auto-loaded absolute asset imports. This
        // test root is named terra::imprint, so package:: resolves to terra here.
        if (matches!(&path.origin, PathOrigin::Absolute)
            || matches!(&path.origin, PathOrigin::Package(package) if package == "terra"))
            && path
                .components
                .first()
                .is_some_and(|value| value == "shaders")
        {
            let local = terra_core::asset_directory()
                .join(path.components.join("/"))
                .with_extension("wesl");
            return std::fs::read_to_string(local)
                .map(Cow::Owned)
                .map_err(|error| ResolveError::ModuleNotFound(path.clone(), error.to_string()));
        }
        let PathOrigin::Package(package) = &path.origin else {
            return Err(ResolveError::ModuleNotFound(
                path.clone(),
                "expected an embedded engine package".into(),
            ));
        };
        let mut asset_path = PathBuf::from(package);
        for component in &path.components {
            asset_path.push(component);
        }
        asset_path.set_extension("wesl");
        let source = self.server.get_source("embedded").unwrap();
        let mut reader = bevy::tasks::block_on(source.reader().read(&asset_path))
            .map_err(|error| ResolveError::ModuleNotFound(path.clone(), error.to_string()))?;
        let mut bytes = Vec::new();
        bevy::tasks::block_on(reader.read_to_end(&mut bytes)).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        self.sources.borrow_mut().insert(path.clone(), text.clone());
        Ok(Cow::Owned(text))
    }

    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        // Match Bevy's ShaderResolver: cross-package imports can arrive with
        // the importing package prepended by WESL.
        match &path.origin {
            PathOrigin::Package(package) if package.contains('/') => ModulePath {
                origin: PathOrigin::Package(package.rsplit('/').next().unwrap().into()),
                components: path.components.clone(),
            },
            _ => path.clone(),
        }
    }
}

#[test]
fn only_the_new_reconstructed_surface_uses_active_domain_coverage() {
    let path = "shaders/terra_particle_surface.wesl";
    let source = std::fs::read_to_string(terra_core::asset_directory().join(path)).unwrap();
    let shader = bevy::shader::Shader::from_wesl(source, path);
    assert!(shader.imports.iter().any(|import| matches!(import,
        bevy::shader::ShaderImport::AssetPath(path) if path == "/shaders/terra_fluid_coverage"
    )));
    for file in ["terra_water", "terra_wet_surface", "terra_imprint_prepass"] {
        let source = std::fs::read_to_string(
            terra_core::asset_directory().join(format!("shaders/{file}.wesl")),
        )
        .unwrap();
        assert!(
            !source.contains("particle_surface_pixel"),
            "{file} must never expose a second surface outside the near region"
        );
    }
    let optics = include_str!("../../../assets/shaders/terra_caves.wesl");
    assert!(!optics.contains("fn water_path"));
    assert!(!optics.contains("let ripple ="));
}

fn engine_shaders() -> EmbeddedShaders {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        RenderPlugin {
            render_creation: WgpuSettings {
                backends: None,
                ..default()
            }
            .into(),
            ..default()
        },
        ImagePlugin::default(),
        bevy::mesh::MeshPlugin,
        bevy::pbr::PbrPlugin {
            gltf_enable_standard_materials: false,
            ..default()
        },
    ));
    EmbeddedShaders {
        server: app.world().resource::<AssetServer>().clone(),
        root: include_str!("../../../assets/shaders/terra_imprint_prepass.wesl").into(),
        sources: RefCell::default(),
    }
}

fn has_binding_arrays(module: &TranslationUnit) -> bool {
    module.global_declarations.iter().any(|declaration| {
        matches!(&**declaration, GlobalDeclaration::Declaration(declaration)
            if declaration.ty.as_ref().is_some_and(|ty| ty.ident.name().as_str() == "binding_array"))
    })
}

fn enables_binding_arrays(module: &TranslationUnit) -> bool {
    module.global_directives.iter().any(|directive| {
        matches!(directive, GlobalDirective::Enable(enable)
            if enable.extensions.iter().any(|extension| extension == "wgpu_binding_array"))
    })
}

#[test]
fn actual_imprint_prepass_composition_enables_arrays_even_without_stock_texture_access() {
    let mut resolver = engine_shaders();
    let path = "terra::imprint".parse::<ModulePath>().unwrap();
    for bindless in [false, true] {
        for (discard, normal_motion, normal_map, uv_b, unclipped) in [
            (false, false, false, false, false),
            (true, false, false, false, false),
            (false, true, false, false, false),
            (false, true, true, false, false),
            (true, true, true, true, false),
            (false, false, false, false, true),
        ] {
            let mut options = wesl::CompileOptions {
                visibility: false,
                ..default()
            };
            for (feature, enabled) in [
                ("BINDLESS", bindless),
                ("PREPASS_PIPELINE", true),
                ("PREPASS_FRAGMENT", normal_motion || unclipped),
                ("NORMAL_PREPASS", normal_motion),
                ("NORMAL_PREPASS_OR_DEFERRED_PREPASS", normal_motion),
                ("MOTION_VECTOR_PREPASS", normal_motion),
                ("MAY_DISCARD", discard),
                ("VERTEX_OUTPUT_INSTANCE_INDEX", true),
                ("VERTEX_NORMALS", true),
                ("VERTEX_UVS", true),
                ("VERTEX_UVS_A", true),
                ("VERTEX_UVS_B", true),
                ("VERTEX_TANGENTS", true),
                ("STANDARD_MATERIAL_NORMAL_MAP", normal_map),
                ("STANDARD_MATERIAL_NORMAL_MAP_UV_B", uv_b),
                ("UNCLIPPED_DEPTH_ORTHO_EMULATION", unclipped),
            ] {
                options.features.set(feature, enabled);
            }
            let compiled = wesl::compile(&path, &options, &resolver).unwrap();
            assert_eq!(has_binding_arrays(&compiled.syntax), bindless);
            assert_eq!(enables_binding_arrays(&compiled.syntax), bindless);
            assert!(
                compiled
                    .modules
                    .iter()
                    .any(|module| module.path.to_string() == "bevy_pbr::prepass::io")
            );

            if bindless && !discard && !normal_motion && !unclipped {
                // Reproduce the original hole: stripping unused stock texture
                // imports also strips their enable directive, but not our buffer.
                let original = resolver.root.clone();
                let mut broken = original.parse::<TranslationUnit>().unwrap();
                broken.global_directives.clear();
                resolver.root = broken.to_string();
                let compiled = wesl::compile(&path, &options, &resolver).unwrap();
                assert!(has_binding_arrays(&compiled.syntax));
                assert!(!enables_binding_arrays(&compiled.syntax));
                resolver.root = original;
            }
        }
    }
}

#[test]
fn actual_liquid_vertex_compiles_forward_depth_and_motion_prepasses_in_both_binding_modes() {
    let mut resolver = engine_shaders();
    resolver.root = include_str!("../../../assets/shaders/terra_liquid_vertex.wesl").into();
    let path = "terra::imprint".parse::<ModulePath>().unwrap();
    for bindless in [false, true] {
        for liquid in [false, true] {
            for (prepass, normal_motion, deferred, unclipped) in [
                (false, false, false, false),
                (true, false, false, false),
                (true, true, false, false),
                (true, true, true, true),
            ] {
                let mut options = wesl::CompileOptions {
                    visibility: false,
                    ..default()
                };
                for (feature, enabled) in [
                    ("BINDLESS", bindless),
                    ("LIQUID_MOTION", liquid),
                    ("PREPASS_PIPELINE", prepass),
                    ("NORMAL_PREPASS_OR_DEFERRED_PREPASS", normal_motion),
                    ("NORMAL_PREPASS", normal_motion && !deferred),
                    ("DEFERRED_PREPASS", deferred),
                    ("MOTION_VECTOR_PREPASS", normal_motion),
                    ("VERTEX_POSITIONS", true),
                    ("VERTEX_NORMALS", true),
                    ("VERTEX_UVS", true),
                    ("VERTEX_UVS_A", true),
                    ("VERTEX_UVS_B", true),
                    ("VERTEX_TANGENTS", true),
                    ("VERTEX_COLORS", true),
                    ("VERTEX_OUTPUT_INSTANCE_INDEX", true),
                    ("VISIBILITY_RANGE_DITHER", true),
                    ("UNCLIPPED_DEPTH_ORTHO_EMULATION", unclipped),
                ] {
                    options.features.set(feature, enabled);
                }
                let compiled = wesl::compile(&path, &options, &resolver).unwrap();
                let wgsl = compiled.syntax.to_string();
                let module = naga::front::wgsl::parse_str(&wgsl)
                    .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&wgsl)));
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                )
                .validate(&module)
                .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&wgsl)));

                if liquid && normal_motion && !deferred {
                    // Conditional WESL blocks flatten into one WGSL scope. The
                    // original native failure reused the current-position name
                    // in the previous-position branch; real WGSL validation must
                    // reject that regression in both material binding modes.
                    let fixed = resolver.root.clone();
                    resolver.root = fixed.replace("previous_phase", "phase");
                    if let Ok(broken) = wesl::compile(&path, &options, &resolver) {
                        assert!(naga::front::wgsl::parse_str(&broken.syntax.to_string()).is_err());
                    }
                    resolver.root = fixed;
                }
            }
        }
    }
}

#[test]
fn actual_particle_solver_and_reconstruction_compile_all_gpu_entry_points() {
    let mut resolver = engine_shaders();
    let path = "terra::imprint".parse::<ModulePath>().unwrap();
    for source in [
        include_str!("../../../assets/shaders/terra_particle_solver.wesl"),
        include_str!("../../../assets/shaders/terra_particle_field.wesl"),
    ] {
        resolver.root = source.into();
        let options = wesl::CompileOptions {
            visibility: false,
            keep_main: true,
            ..default()
        };
        let compiled = wesl::compile(&path, &options, &resolver).unwrap();
        let wgsl = compiled.syntax.to_string();
        let module = naga::front::wgsl::parse_str(&wgsl)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&wgsl)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&wgsl)));
        assert!(
            module
                .entry_points
                .iter()
                .all(|entry| entry.stage == naga::ShaderStage::Compute)
        );
        assert!(module.entry_points.len() >= 3);
    }
}

#[test]
fn particle_surface_beacon_and_sky_compile_with_and_without_opaque_depth_clipping() {
    let mut resolver = engine_shaders();
    let path = "terra::imprint".parse::<ModulePath>().unwrap();
    for (source, entry_points) in [
        (
            include_str!("../../../assets/shaders/terra_particle_surface.wesl"),
            2,
        ),
        (include_str!("../../../assets/shaders/terra_beacon.wesl"), 2),
        (
            include_str!("../../../assets/shaders/terra_horizon.wesl"),
            1,
        ),
    ] {
        resolver.root = source.into();
        for depth in [false, true] {
            let mut options = wesl::CompileOptions {
                visibility: false,
                ..default()
            };
            for (name, value) in [("DEPTH_PREPASS", depth), ("VERTEX_POSITIONS", true)] {
                options.features.set(name, value);
            }
            let compiled = wesl::compile(&path, &options, &resolver).unwrap();
            let wgsl = compiled.syntax.to_string();
            let module = naga::front::wgsl::parse_str(&wgsl)
                .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&wgsl)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&wgsl)));
            assert_eq!(module.entry_points.len(), entry_points);
        }
    }
}

#[test]
fn actual_fluid_handoff_fragments_compile_in_both_binding_modes() {
    let mut resolver = engine_shaders();
    let path = "terra::imprint".parse::<ModulePath>().unwrap();
    for file in ["terra_water", "terra_wet_surface", "terra_imprint_prepass"] {
        for bindless in [false, true] {
            for depth in [false, true] {
                resolver.root = std::fs::read_to_string(
                    terra_core::asset_directory().join(format!("shaders/{file}.wesl")),
                )
                .unwrap();
                let mut options = wesl::CompileOptions {
                    visibility: false,
                    ..default()
                };
                for (feature, value) in [
                    ("BINDLESS", bindless),
                    ("VERTEX_POSITIONS", true),
                    ("VERTEX_NORMALS", true),
                    ("VERTEX_UVS", true),
                    ("VERTEX_UVS_A", true),
                    ("VERTEX_UVS_B", true),
                    ("VERTEX_TANGENTS", true),
                    ("VERTEX_COLORS", true),
                    ("VERTEX_OUTPUT_INSTANCE_INDEX", true),
                    ("DEPTH_PREPASS", depth),
                    ("PREPASS_PIPELINE", file == "terra_imprint_prepass"),
                    ("PREPASS_FRAGMENT", file == "terra_imprint_prepass"),
                    ("NORMAL_PREPASS", file == "terra_imprint_prepass"),
                    (
                        "NORMAL_PREPASS_OR_DEFERRED_PREPASS",
                        file == "terra_imprint_prepass",
                    ),
                    ("MAY_DISCARD", true),
                ] {
                    options.features.set(feature, value);
                }
                let compiled = wesl::compile(&path, &options, &resolver).unwrap();
                let wgsl = compiled.syntax.to_string();
                let module = naga::front::wgsl::parse_str(&wgsl).unwrap_or_else(|e| {
                    panic!(
                        "{file} bindless{bindless} depth{depth}: {}",
                        e.emit_to_string(&wgsl)
                    )
                });
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                )
                .validate(&module)
                .unwrap_or_else(|e| {
                    panic!(
                        "{file} bindless{bindless} depth{depth}: {}",
                        e.emit_to_string(&wgsl)
                    )
                });
            }
        }
    }
}

#[test]
fn certified_oak_paint_omits_alpha_texture_fetch_in_both_shadow_binding_modes() {
    let mut resolver = engine_shaders();
    resolver.root = include_str!("../../../assets/shaders/terra_foliage_prepass.wesl").into();
    let path = "terra::imprint".parse::<ModulePath>().unwrap();
    for bindless in [false, true] {
        for (opaque, profile) in [(false, false), (true, false), (true, true)] {
            let mut options = wesl::CompileOptions {
                visibility: false,
                ..default()
            };
            for feature in [
                "PREPASS_PIPELINE",
                "MAY_DISCARD",
                "VERTEX_UVS",
                "VERTEX_UVS_A",
                "VERTEX_COLORS",
                "VERTEX_OUTPUT_INSTANCE_INDEX",
            ] {
                options.features.set(feature, true);
            }
            options.features.set("BINDLESS", bindless);
            options.features.set("OPAQUE_LEAF_PAINT", opaque);
            options.features.set("LEAF_SHADOW_PROFILE", profile);
            let wgsl = wesl::compile(&path, &options, &resolver)
                .unwrap()
                .syntax
                .to_string();
            let module = naga::front::wgsl::parse_str(&wgsl)
                .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&wgsl)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap();
            // This variant outputs depth only, so the sole purpose of any
            // remaining image sample is coverage. Both paths still discard.
            assert_eq!(
                module
                    .functions
                    .iter()
                    .any(|(_, function)| function.expressions.iter().any(
                        |(_, expression)| matches!(
                            expression,
                            naga::Expression::ImageSample { .. }
                        )
                    )),
                !opaque
            );
            assert!(wgsl.contains("discard;"));
        }
    }
}

#[test]
fn canopy_opaque_surfaces_and_surf_translate_to_direct3d_and_vulkan() {
    let mut resolver = engine_shaders();
    let path = "terra::imprint".parse::<ModulePath>().unwrap();
    let leaf = include_str!("../../../assets/shaders/terra_foliage.wesl");
    let liquid = include_str!("../../../assets/shaders/terra_liquid_vertex.wesl");
    for (source, prepass, fragment, compressed, deferred, flame, dry_oak) in [
        (
            include_str!("../../../assets/shaders/terra_surf.wesl"),
            false,
            true,
            false,
            false,
            false,
            false,
        ),
        (leaf, false, false, false, false, false, false),
        (leaf, false, false, false, false, false, true),
        (leaf, true, false, true, false, false, false),
        (leaf, true, false, true, false, false, true),
        (leaf, true, false, true, true, false, false),
        (leaf, true, false, true, true, false, true),
        (leaf, true, false, false, false, true, false),
        (
            include_str!("../../../assets/shaders/terra_foliage_prepass.wesl"),
            true,
            true,
            false,
            false,
            false,
            false,
        ),
        (liquid, false, false, true, false, false, false),
        (liquid, true, false, false, false, false, false),
    ] {
        resolver.root = source.into();
        let mut options = wesl::CompileOptions {
            visibility: false,
            ..default()
        };
        for (name, enabled) in [
            ("PREPASS_PIPELINE", prepass),
            ("DEFERRED_PREPASS", deferred),
            ("FLAME_GEOMETRY", flame),
            ("DRY_OAK_WIND", dry_oak),
            ("PREPASS_FRAGMENT", fragment),
            ("NORMAL_PREPASS", prepass),
            ("NORMAL_PREPASS_OR_DEFERRED_PREPASS", prepass),
            ("MOTION_VECTOR_PREPASS", prepass),
            ("MAY_DISCARD", fragment),
            ("VERTEX_POSITIONS_COMPRESSED", compressed),
            ("VERTEX_NORMALS_COMPRESSED", compressed),
            ("VERTEX_POSITIONS", true),
            ("VERTEX_NORMALS", true),
            ("VERTEX_TANGENTS", true),
            ("VERTEX_COLORS", true),
            ("PACKED_LEAF_COLOR", true),
            ("VERTEX_UVS", true),
            ("VERTEX_UVS_A", true),
            ("VERTEX_UVS_B", !dry_oak),
            ("VERTEX_OUTPUT_INSTANCE_INDEX", true),
            ("VISIBILITY_RANGE_DITHER", true),
        ] {
            options.features.set(name, enabled);
        }
        let wgsl = wesl::compile(&path, &options, &resolver)
            .unwrap()
            .syntax
            .to_string();
        if source == leaf {
            assert_eq!(
                wgsl.contains("let climate_tag"),
                !flame && (!prepass || deferred)
            );
            assert_eq!(wgsl.contains("let seed = clamp"), !flame);
        }
        let module = naga::front::wgsl::parse_str(&wgsl)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&wgsl)));
        if source == leaf && !flame {
            let wind = module
                .functions
                .iter()
                .map(|(_, f)| f)
                .find(|f| {
                    f.name
                        .as_deref()
                        .is_some_and(|n| n.contains("wind_position"))
                })
                .unwrap();
            let sines = wind
                .expressions
                .iter()
                .filter(|(_, e)| {
                    matches!(
                        e,
                        naga::Expression::Math {
                            fun: naga::MathFunction::Sin,
                            ..
                        }
                    )
                })
                .count();
            assert_eq!(sines, if dry_oak { 3 } else { 5 });
            assert_eq!(
                module
                    .global_variables
                    .iter()
                    .any(|(_, v)| v.binding.as_ref().is_some_and(|b| b.binding == 100)),
                !dry_oak
            );
        }
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        let hlsl_options = naga::back::hlsl::Options {
            shader_model: naga::back::hlsl::ShaderModel::V6_0,
            ..default()
        };
        let mut hlsl = String::new();
        let written = naga::back::hlsl::Writer::new(&mut hlsl, &hlsl_options, &default())
            .write(&module, &info, None)
            .unwrap();
        assert!(written.entry_point_names.iter().all(Result::is_ok));
        assert!(!hlsl.is_empty());
        let spirv = naga::back::spv::write_vec(&module, &info, &default(), None).unwrap();
        assert_eq!(spirv[0], 0x0723_0203);
    }
}
