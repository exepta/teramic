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
