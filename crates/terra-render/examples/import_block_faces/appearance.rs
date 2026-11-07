use super::{Pixels, decode, fingerprint, workspace_path};
use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, Image, ImageFormat, ImageSampler, ImageType},
    math::Vec3,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

const MAX_DIMENSION: u32 = 4096;
const MAX_FILE_BYTES: u64 = 80 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Settings {
    pub(super) base_color: PathBuf,
    pub(super) normal: PathBuf,
    #[serde(default = "normal_strength")]
    pub(super) normal_strength: f32,
    #[serde(default = "roughness")]
    pub(super) roughness: f32,
}

fn normal_strength() -> f32 {
    0.45
}
fn roughness() -> f32 {
    0.88
}

pub(super) struct Prepared {
    pub(super) base_png: Vec<u8>,
    pub(super) normal_png: Vec<u8>,
    pub(super) orm_png: Vec<u8>,
    pub(super) report: Value,
}

pub(super) fn validate(settings: &Settings) -> Result<(), String> {
    for (name, path) in [
        ("base_color", &settings.base_color),
        ("normal", &settings.normal),
    ] {
        if path.as_os_str().is_empty() {
            return Err(format!("Appearance {name} path must not be empty"));
        }
        workspace_path(Path::new("."), path)?;
    }
    for (name, value) in [
        ("normal_strength", settings.normal_strength),
        ("roughness", settings.roughness),
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(format!("Appearance {name} must be finite and within 0..=1"));
        }
    }
    Ok(())
}

fn read_png(root: &Path, path: &Path) -> Result<Vec<u8>, String> {
    let workspace = root
        .canonicalize()
        .map_err(|e| format!("Cannot resolve appearance workspace: {e}"))?;
    let file = workspace_path(root, path)?
        .canonicalize()
        .map_err(|e| format!("Cannot resolve appearance {}: {e}", path.display()))?;
    if !file.starts_with(&workspace) {
        return Err(format!(
            "Appearance path escapes the workspace: {}",
            path.display()
        ));
    }
    let metadata = fs::metadata(&file).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return Err(format!(
            "Appearance {} is not a regular PNG within the 80 MiB budget",
            path.display()
        ));
    }
    fs::read(file).map_err(|e| format!("Cannot read appearance {}: {e}", path.display()))
}

fn dimensions(bytes: &[u8]) -> Result<[u32; 2], String> {
    // Check the fixed PNG IHDR before the image decoder can allocate a pixel buffer.
    if bytes.len() < 33
        || bytes[..8] != [137, 80, 78, 71, 13, 10, 26, 10]
        || u32::from_be_bytes(bytes[8..12].try_into().unwrap()) != 13
        || &bytes[12..16] != b"IHDR"
    {
        return Err("Appearance requires a PNG with a valid leading IHDR".into());
    }
    let result = [
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    ];
    if result.into_iter().any(|v| v == 0 || v > MAX_DIMENSION) {
        return Err("Appearance dimensions must be within 1..=4096 pixels per axis".into());
    }
    Ok(result)
}

fn decode_bounded(bytes: &[u8], size: [u32; 2]) -> Result<Pixels, String> {
    if size.into_iter().all(|v| v <= 2048) {
        return decode(bytes);
    }
    let image = Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::MAIN_WORLD,
    )
    .map_err(|e| e.to_string())?;
    if [image.width(), image.height()] != size {
        return Err("Appearance decoder dimensions differ from the validated IHDR".into());
    }
    let rgba = image
        .try_into_dynamic()
        .map_err(|e| e.to_string())?
        .into_rgba8();
    Ok(Pixels {
        width: size[0],
        height: size[1],
        data: rgba.into_raw(),
    })
}

fn encode(width: u32, height: u32, data: Vec<u8>) -> Result<Vec<u8>, String> {
    let image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        // This is a CPU byte container only. glTF selects linear sampling for
        // the generated normal/ORM data, without applying a gamma conversion.
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );
    let mut destination = Cursor::new(Vec::new());
    image
        .try_into_dynamic()
        .map_err(|e| e.to_string())?
        .write_to(
            &mut destination,
            ImageFormat::Png.as_image_crate_format().unwrap(),
        )
        .map_err(|e| e.to_string())?;
    Ok(destination.into_inner())
}

fn prepare_bytes(
    base_png: Vec<u8>,
    normal_source: &[u8],
    settings: &Settings,
) -> Result<Prepared, String> {
    validate(settings)?;
    let base_size = dimensions(&base_png)?;
    let normal_size = dimensions(normal_source)?;
    if base_size != normal_size {
        return Err(format!(
            "Appearance base/normal dimensions must match: {base_size:?} != {normal_size:?}"
        ));
    }
    let base = decode_bounded(&base_png, base_size)?;
    let mut normal = decode_bounded(normal_source, normal_size)?;
    if [base.width, base.height] != base_size || [normal.width, normal.height] != normal_size {
        return Err("Appearance decoded dimensions do not match the PNG headers".into());
    }
    let mut clamped_z_pixels = 0_u64;
    for rgba in normal.data.chunks_exact_mut(4) {
        let decoded = [rgba[0], rgba[1], rgba[2]].map(|v| f32::from(v) / 127.5 - 1.0);
        if decoded[2] < 0.0 {
            clamped_z_pixels += 1;
        }
        let vector = Vec3::new(
            decoded[0] * settings.normal_strength,
            decoded[1] * settings.normal_strength,
            decoded[2].max(1.0 / 255.0),
        )
        .normalize_or(Vec3::Z);
        for (destination, component) in rgba[..3].iter_mut().zip(vector.to_array()) {
            *destination = ((component * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        rgba[3] = 255;
    }
    let normal_png = encode(normal.width, normal.height, normal.data)?;
    let roughness_byte = (settings.roughness * 255.0).round() as u8;
    let orm_png = encode(1, 1, vec![255, roughness_byte, 0, 255])?;
    let report = json!({
        "base_color":settings.base_color,"normal":settings.normal,
        "dimensions":base_size,"base_bytes_preserved":true,
        "base_fingerprint":fingerprint(&base_png),
        "normal_source_fingerprint":fingerprint(normal_source),
        "normal_output_fingerprint":fingerprint(&normal_png),
        "normal_strength":settings.normal_strength,
        "normal_processing":"linear XY strength, positive-Z hemisphere, normalized RGB vectors",
        "normal_clamped_z_pixels":clamped_z_pixels,
        "orm_dimensions":[1,1],"orm_rgba":[255,roughness_byte,0,255],
        "roughness":settings.roughness,"metallic":0,
        "material_factors":{"normal_scale":1.0,"roughness_factor":1.0,"metallic_factor":0.0}
    });
    Ok(Prepared {
        base_png,
        normal_png,
        orm_png,
        report,
    })
}

pub(super) fn prepare(root: &Path, settings: &Settings) -> Result<Prepared, String> {
    validate(settings)?;
    let base = read_png(root, &settings.base_color)?;
    let normal = read_png(root, &settings.normal)?;
    prepare_bytes(base, &normal, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            base_color: "assets/base.png".into(),
            normal: "assets/normal.png".into(),
            normal_strength: normal_strength(),
            roughness: roughness(),
        }
    }

    #[test]
    fn paths_cannot_escape_the_workspace() {
        for invalid in [
            "../base.png",
            "/tmp/base.png",
            "assets/../base.png",
            "./base.png",
            "",
        ] {
            assert!(
                validate(&Settings {
                    base_color: invalid.into(),
                    ..settings()
                })
                .is_err(),
                "{invalid}"
            );
        }
        assert!(validate(&settings()).is_ok());
    }

    #[test]
    fn base_bytes_are_unchanged_and_normals_are_linear_unit_vectors() {
        let base = encode(2, 2, [95, 144, 37, 255].repeat(4)).unwrap();
        let source = encode(
            2,
            2,
            vec![
                128, 128, 255, 255, 220, 100, 230, 255, 64, 200, 220, 255, 100, 140, 80, 255,
            ],
        )
        .unwrap();
        let output = prepare_bytes(base.clone(), &source, &settings()).unwrap();
        assert_eq!(output.base_png, base);
        let normal = decode(&output.normal_png).unwrap();
        assert_eq!([normal.width, normal.height], [2, 2]);
        for rgba in normal.data.chunks_exact(4) {
            let v =
                Vec3::from_array([rgba[0], rgba[1], rgba[2]].map(|v| f32::from(v) / 127.5 - 1.0));
            assert!((v.length() - 1.0).abs() < 0.01);
            assert!(v.z >= 0.0);
            assert_eq!(rgba[3], 255);
        }
        assert_eq!(output.report["normal_clamped_z_pixels"], 1);
        let orm = decode(&output.orm_png).unwrap();
        assert_eq!([orm.width, orm.height], [1, 1]);
        assert_eq!(orm.data, [255, 224, 0, 255]);
    }

    #[test]
    fn dimensions_must_match_and_stay_within_the_import_budget() {
        let base = encode(2, 2, [95, 144, 37, 255].repeat(4)).unwrap();
        let normal = encode(1, 1, vec![128, 128, 255, 255]).unwrap();
        assert!(prepare_bytes(base.clone(), &normal, &settings()).is_err());
        let mut invalid = base;
        invalid[16..20].copy_from_slice(&4097_u32.to_be_bytes());
        assert!(dimensions(&invalid).is_err());
        invalid[16..20].copy_from_slice(&0_u32.to_be_bytes());
        assert!(dimensions(&invalid).is_err());
    }

    #[test]
    fn strength_zero_produces_flat_normals_and_invalid_floats_are_rejected() {
        let base = encode(1, 1, vec![95, 144, 37, 255]).unwrap();
        let source = encode(1, 1, vec![220, 100, 230, 255]).unwrap();
        let output = prepare_bytes(
            base,
            &source,
            &Settings {
                normal_strength: 0.0,
                ..settings()
            },
        )
        .unwrap();
        assert_eq!(
            decode(&output.normal_png).unwrap().data,
            [128, 128, 255, 255]
        );
        assert!(
            validate(&Settings {
                normal_strength: f32::NAN,
                ..settings()
            })
            .is_err()
        );
        assert!(
            validate(&Settings {
                roughness: 1.1,
                ..settings()
            })
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_must_not_escape_the_workspace() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "teramic-appearance-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(directory.join("workspace")).unwrap();
        fs::write(directory.join("outside.png"), b"outside").unwrap();
        std::os::unix::fs::symlink(
            directory.join("outside.png"),
            directory.join("workspace/escape.png"),
        )
        .unwrap();
        let result = read_png(&directory.join("workspace"), Path::new("escape.png"));
        fs::remove_dir_all(directory).unwrap();
        assert!(result.unwrap_err().contains("escapes"));
    }
}
