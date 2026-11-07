//! RGB travels with the existing ribbon instance, not a color-sized material cache.
use super::*;
use bevy::{
    mesh::MeshTag,
    pbr::{ExtendedMaterial, MaterialExtension},
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub(crate) struct RibbonTint {}

impl MaterialExtension for RibbonTint {
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_ribbon_tint.wesl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        Self::fragment_shader()
    }
}

pub(crate) type Material = ExtendedMaterial<StandardMaterial, RibbonTint>;

pub(super) fn tag(rgb: [u8; 3]) -> MeshTag {
    MeshTag::with_type::<RibbonTint>(
        (1 << 24) | u32::from(rgb[0]) | (u32::from(rgb[1]) << 8) | (u32::from(rgb[2]) << 16),
    )
}

pub(super) fn material(base: StandardMaterial) -> Material {
    ExtendedMaterial {
        base,
        extension: RibbonTint {},
    }
}

pub(super) fn cached_material(
    cache: &mut HashMap<bevy::asset::AssetId<StandardMaterial>, Handle<Material>>,
    source: &Handle<StandardMaterial>,
    bases: &Assets<StandardMaterial>,
    materials: &mut Assets<Material>,
) -> Option<Handle<Material>> {
    if let Some(handle) = cache.get(&source.id()) {
        return Some(handle.clone());
    }
    let handle = materials.add(material(bases.get(source)?.clone()));
    cache.insert(source.id(), handle.clone());
    Some(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_rgb_tags_include_black_without_overloading_missing_tags() {
        for value in 0..65536u32 {
            let rgb = [value as u8, (value >> 8) as u8, 184];
            let packed = tag(rgb).value;
            assert_ne!(packed & (1 << 24), 0);
            assert_eq!(
                [packed as u8, (packed >> 8) as u8, (packed >> 16) as u8],
                rgb
            );
        }
        assert_ne!(tag([0; 3]).value, 0);
    }

    #[test]
    fn wrapping_keeps_all_standard_surface_shadow_and_texture_properties() {
        let base = StandardMaterial {
            base_color: Color::linear_rgba(0.6, 0.7, 0.8, 1.0),
            perceptual_roughness: 0.73,
            metallic: 0.04,
            cull_mode: None,
            double_sided: true,
            uv_transform: bevy::math::Affine2::from_translation(Vec2::new(0.0, 0.25)),
            ..default()
        };
        let wrapped = material(base.clone());
        assert_eq!(wrapped.base.base_color, base.base_color);
        assert_eq!(wrapped.base.perceptual_roughness, base.perceptual_roughness);
        assert_eq!(wrapped.base.metallic, base.metallic);
        assert_eq!(wrapped.base.alpha_mode, base.alpha_mode);
        assert_eq!(wrapped.base.cull_mode, base.cull_mode);
        assert_eq!(wrapped.base.double_sided, base.double_sided);
        assert_eq!(wrapped.base.uv_transform, base.uv_transform);
    }

    #[test]
    fn a_thousand_instance_colors_share_one_ribbon_material_and_do_not_touch_wood() {
        let mut bases = Assets::<StandardMaterial>::default();
        let source = bases.add(StandardMaterial::default());
        let wood = bases.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.4, 0.2),
            ..default()
        });
        let before = bases.get(&wood).unwrap().base_color;
        let mut materials = Assets::<Material>::default();
        let mut cache = HashMap::new();
        let first = cached_material(&mut cache, &source, &bases, &mut materials).unwrap();
        for value in 0..1000u16 {
            let _tag = tag([value as u8, (value >> 8) as u8, 71]);
            assert_eq!(
                cached_material(&mut cache, &source, &bases, &mut materials).unwrap(),
                first
            );
        }
        assert_eq!(materials.len(), 1);
        assert_eq!(cache.len(), 1);
        assert_eq!(bases.len(), 2);
        assert_eq!(bases.get(&wood).unwrap().base_color, before);
    }
}
