//! Deterministic, recipe-driven preparation of a static PBR cube for terrain batching.
use bevy::{
    asset::RenderAssetUsages,
    gltf::gltf::{self, binary::Glb},
    image::{CompressedImageFormats, Image, ImageFormat, ImageSampler, ImageType},
    math::Vec3,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    io::Cursor,
    path::{Component, Path, PathBuf},
};

const SIDES: [&str; 6] = ["West", "East", "Down", "Up", "North", "South"];
const AXES: [[usize; 2]; 6] = [[2, 1], [2, 1], [0, 2], [0, 2], [0, 1], [0, 1]];

#[path = "import_block_faces/appearance.rs"]
mod appearance;
#[path = "import_block_faces/relief.rs"]
mod relief;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recipe {
    schema_version: u32,
    source: PathBuf,
    output: PathBuf,
    provenance: PathBuf,
    translation: [f32; 3],
    minimum_normal_dominance: f64,
    join_band: f64,
    maximum_uv_residual: f64,
    lod_tile_size: u32,
    expected_vertices: usize,
    expected_triangles: usize,
    #[serde(default)]
    relief: Option<relief::Settings>,
    #[serde(default)]
    appearance: Option<appearance::Settings>,
}

#[derive(Default, Clone)]
struct Group {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    tangents: Vec<[f32; 4]>,
    indices: Vec<u32>,
    remap: BTreeMap<u32, u32>,
    dominance: f64,
}

struct Pixels {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

#[derive(Default)]
struct Buffer {
    bytes: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Buffer {
    fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while !self.bytes.len().is_multiple_of(4) {
            self.bytes.push(0);
        }
        let index = self.views.len();
        let mut view = json!({"buffer":0,"byteOffset":self.bytes.len(),"byteLength":bytes.len()});
        if let Some(target) = target {
            view["target"] = target.into();
        }
        self.views.push(view);
        self.bytes.extend_from_slice(bytes);
        index
    }

    fn floats<const N: usize>(&mut self, data: &[[f32; N]], kind: &str, bounds: bool) -> usize {
        let bytes: Vec<_> = data
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let view = self.view(&bytes, Some(34962));
        let mut accessor =
            json!({"bufferView":view,"componentType":5126,"count":data.len(),"type":kind});
        if bounds {
            accessor["min"] = json!(
                (0..N)
                    .map(|i| data.iter().map(|p| p[i]).fold(f32::INFINITY, f32::min))
                    .collect::<Vec<_>>()
            );
            accessor["max"] = json!(
                (0..N)
                    .map(|i| data.iter().map(|p| p[i]).fold(f32::NEG_INFINITY, f32::max))
                    .collect::<Vec<_>>()
            );
        }
        let index = self.accessors.len();
        self.accessors.push(accessor);
        index
    }

    fn primitive(&mut self, group: &Group, material: usize) -> Value {
        let mut attributes = json!({
            "POSITION":self.floats(&group.positions,"VEC3",true),
            "NORMAL":self.floats(&group.normals,"VEC3",false),
            "TEXCOORD_0":self.floats(&group.uvs,"VEC2",false)
        });
        if !group.tangents.is_empty() {
            attributes["TANGENT"] = self.floats(&group.tangents, "VEC4", false).into();
        }
        let (bytes, component) = if group.positions.len() <= 65536 {
            (
                group
                    .indices
                    .iter()
                    .flat_map(|&i| (i as u16).to_le_bytes())
                    .collect::<Vec<_>>(),
                5123,
            )
        } else {
            (
                group
                    .indices
                    .iter()
                    .flat_map(|i| i.to_le_bytes())
                    .collect::<Vec<_>>(),
                5125,
            )
        };
        let view = self.view(&bytes, Some(34963));
        let index = self.accessors.len();
        self.accessors.push(json!({"bufferView":view,"componentType":component,"count":group.indices.len(),"type":"SCALAR"}));
        json!({"attributes":attributes,"indices":index,"material":material,"mode":4})
    }
}

fn fingerprint(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    format!("fnv1a64:{hash:016x}")
}

fn side(normal: Vec3, minimum: f64) -> Result<(usize, f64), String> {
    if !normal.is_finite() || normal.length_squared() < 1e-20 {
        return Err("Degenerate/nonfinite triangle".into());
    }
    let n = normal.normalize().to_array().map(f64::from);
    let axis = (1..3).fold(0, |a, b| if n[b].abs() > n[a].abs() { b } else { a });
    let dominance = n[axis].abs();
    if dominance < minimum {
        return Err(format!(
            "Triangle is not unambiguously face-oriented: {dominance}"
        ));
    }
    Ok((axis * 2 + usize::from(n[axis] > 0.0), dominance))
}

fn partition(source: &gltf::Gltf, recipe: &Recipe) -> Result<[Group; 6], String> {
    if source.nodes().len() != 1
        || source.meshes().len() != 1
        || source.materials().len() != 1
        || source.animations().next().is_some()
        || source.skins().next().is_some()
    {
        return Err(
            "Importer requires one static mesh/node/material, without animation or skin".into(),
        );
    }
    let node = source.nodes().next().unwrap();
    let identity = [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ];
    if node.transform().matrix() != identity || node.children().next().is_some() {
        return Err("Bake node transforms before face import".into());
    }
    let mesh = source.meshes().next().unwrap();
    if mesh.primitives().len() != 1 {
        return Err("Importer requires one indexed primitive".into());
    }
    let primitive = mesh.primitives().next().unwrap();
    if primitive.mode() != gltf::mesh::Mode::Triangles
        || primitive.morph_targets().next().is_some()
        || primitive.attributes().any(|(s, _)| {
            !matches!(
                s,
                gltf::Semantic::Positions
                    | gltf::Semantic::Normals
                    | gltf::Semantic::Tangents
                    | gltf::Semantic::TexCoords(0)
            )
        })
    {
        return Err("Unsupported topology, morph target or extra vertex attribute".into());
    }
    let reader = primitive.reader(|buffer| match buffer.source() {
        gltf::buffer::Source::Bin => source.blob.as_deref(),
        _ => None,
    });
    let positions: Vec<_> = reader
        .read_positions()
        .ok_or("Missing positions")?
        .collect();
    let normals: Vec<_> = reader.read_normals().ok_or("Missing normals")?.collect();
    let uvs: Vec<_> = reader
        .read_tex_coords(0)
        .ok_or("Missing UV0")?
        .into_f32()
        .collect();
    let tangents: Option<Vec<_>> = reader.read_tangents().map(Iterator::collect);
    let indices: Vec<_> = reader
        .read_indices()
        .ok_or("Missing indices")?
        .into_u32()
        .collect();
    if positions.len() != recipe.expected_vertices
        || indices.len() != recipe.expected_triangles * 3
        || normals.len() != positions.len()
        || uvs.len() != positions.len()
        || tangents
            .as_ref()
            .is_some_and(|t| t.len() != positions.len())
        || indices.iter().any(|&i| i as usize >= positions.len())
        || positions
            .iter()
            .chain(&normals)
            .flatten()
            .any(|v| !v.is_finite())
        || uvs.iter().flatten().any(|v| !v.is_finite())
        || tangents
            .as_ref()
            .is_some_and(|t| t.iter().flatten().any(|v| !v.is_finite()))
    {
        return Err("Source vertex/index contract changed".into());
    }
    let mut groups = std::array::from_fn(|_| Group {
        dominance: 1.0,
        ..Default::default()
    });
    for triangle in indices.chunks_exact(3) {
        let p = [triangle[0], triangle[1], triangle[2]]
            .map(|i| Vec3::from_array(positions[i as usize]));
        let (side, dominance) = side(
            (p[1] - p[0]).cross(p[2] - p[0]),
            recipe.minimum_normal_dominance,
        )?;
        let group = &mut groups[side];
        group.dominance = group.dominance.min(dominance);
        for &index in triangle {
            let local = if let Some(&mapped) = group.remap.get(&index) {
                mapped
            } else {
                let source = index as usize;
                let mapped = group.positions.len() as u32;
                let position = (Vec3::from_array(positions[source])
                    + Vec3::from_array(recipe.translation))
                .to_array();
                if position.iter().any(|v| !(0.0..=1.0).contains(v)) {
                    return Err("Translated geometry exceeds its unit cell".into());
                }
                group.positions.push(position);
                group.normals.push(normals[source]);
                group.uvs.push(uvs[source]);
                if let Some(tangents) = &tangents {
                    group.tangents.push(tangents[source]);
                }
                group.remap.insert(index, mapped);
                mapped
            };
            group.indices.push(local);
        }
    }
    if groups.iter().any(|g| g.indices.is_empty()) {
        return Err("Every cube side needs geometry".into());
    }
    Ok(groups)
}

fn snap(value: f32, band: f64) -> f64 {
    let value = f64::from(value);
    if value <= band {
        0.0
    } else if value >= 1.0 - band {
        1.0
    } else {
        value
    }
}

fn solve(mut matrix: [[f64; 4]; 3]) -> Result<[f64; 3], String> {
    for column in 0..3 {
        let pivot = (column..3)
            .max_by(|&a, &b| matrix[a][column].abs().total_cmp(&matrix[b][column].abs()))
            .unwrap();
        matrix.swap(column, pivot);
        let divisor = matrix[column][column];
        if divisor.abs() < 1e-12 {
            return Err("Singular face UV fit".into());
        }
        for entry in &mut matrix[column][column..] {
            *entry /= divisor;
        }
        let pivot_row = matrix[column];
        for (index, row) in matrix.iter_mut().enumerate() {
            if index == column {
                continue;
            }
            let factor = row[column];
            for entry in column..4 {
                row[entry] -= factor * pivot_row[entry];
            }
        }
    }
    Ok(matrix.map(|row| row[3]))
}

fn fit_uv(group: &Group, side: usize, recipe: &Recipe) -> Result<([[f64; 3]; 2], f64), String> {
    let axes = AXES[side];
    let rows: Vec<_> = group
        .positions
        .iter()
        .map(|p| {
            [
                snap(p[axes[0]], recipe.join_band),
                snap(p[axes[1]], recipe.join_band),
                1.0,
            ]
        })
        .collect();
    let fit = [0, 1].map(|channel| {
        let mut matrix = [[0.0; 4]; 3];
        for (row, uv) in rows.iter().zip(&group.uvs) {
            for a in 0..3 {
                for b in 0..3 {
                    matrix[a][b] += row[a] * row[b];
                }
                matrix[a][3] += row[a] * f64::from(uv[channel]);
            }
        }
        solve(matrix)
    });
    let fit = [fit[0].clone()?, fit[1].clone()?];
    let residual = rows
        .iter()
        .zip(&group.uvs)
        .map(|(row, uv)| {
            (0..2)
                .map(|c| {
                    (fit[c].iter().zip(row).map(|(a, b)| a * b).sum::<f64>() - f64::from(uv[c]))
                        .powi(2)
                })
                .sum::<f64>()
                .sqrt()
        })
        .fold(0.0, f64::max);
    let basis = uv_basis(fit)?;
    if residual > recipe.maximum_uv_residual || basis.iter().flatten().any(|v| !v.is_finite()) {
        return Err(format!(
            "{} UV fit is not affine after documented gutter snap: {residual}",
            SIDES[side]
        ));
    }
    Ok((fit, residual))
}

fn uv_basis(fit: [[f64; 3]; 2]) -> Result<[[f64; 2]; 2], String> {
    let mut basis = [[0.0; 2]; 2];
    for row in 0..2 {
        let length = fit[row][0].hypot(fit[row][1]);
        if length < 0.01 {
            return Err("Invalid atlas UV scale".into());
        }
        basis[row] = [fit[row][0] / length, fit[row][1] / length];
        if basis[row][0].abs().min(basis[row][1].abs()) > 1e-5 {
            return Err("UV bake requires an axis-aligned atlas rotation/reflection".into());
        }
    }
    if (basis[0][0] * basis[1][0] + basis[0][1] * basis[1][1]).abs() > 1e-5 {
        return Err("UV atlas axes are not orthogonal".into());
    }
    Ok(basis)
}

fn decode(bytes: &[u8]) -> Result<Pixels, String> {
    let image = Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        // CPU PNG bytes only: try_into_dynamic supports the sRGB-tagged RGBA
        // container; this flag does not transform the decoded channel bytes.
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::MAIN_WORLD,
    )
    .map_err(|e| e.to_string())?;
    if image.width() > 2048 || image.height() > 2048 {
        return Err("Source atlas exceeds 2048x2048 import budget".into());
    }
    let image = image
        .try_into_dynamic()
        .map_err(|e| e.to_string())?
        .into_rgba8();
    Ok(Pixels {
        width: image.width(),
        height: image.height(),
        data: image.into_raw(),
    })
}

fn linear(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(value: f64) -> f64 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        value.powf(1.0 / 2.4) * 1.055 - 0.055
    }
}

fn sample(image: &Pixels, uv: [f64; 2], color: bool) -> [f64; 4] {
    let x = (uv[0] * f64::from(image.width) - 0.5).clamp(0.0, f64::from(image.width - 1));
    let y = (uv[1] * f64::from(image.height) - 0.5).clamp(0.0, f64::from(image.height - 1));
    let mut output = [0.0; 4];
    for (dy, wy) in [(0, 1.0 - y.fract()), (1, y.fract())] {
        for (dx, wx) in [(0, 1.0 - x.fract()), (1, x.fract())] {
            let pixel = ((y as u32 + dy).min(image.height - 1) * image.width
                + (x as u32 + dx).min(image.width - 1)) as usize
                * 4;
            for (channel, target) in output.iter_mut().enumerate() {
                let value = f64::from(image.data[pixel + channel]) / 255.0;
                *target += if color && channel < 3 {
                    linear(value) * wx * wy
                } else {
                    value * wx * wy
                };
            }
        }
    }
    if color {
        for channel in &mut output[..3] {
            *channel = srgb(*channel);
        }
    }
    output
}

fn bake(
    image: &Pixels,
    fit: [[f64; 3]; 2],
    size: u32,
    normal: bool,
    color: bool,
) -> Result<Vec<u8>, String> {
    let basis = uv_basis(fit)?;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let q = [
                (f64::from(x) + 0.5) / f64::from(size),
                (f64::from(y) + 0.5) / f64::from(size),
                1.0,
            ];
            let uv = fit.map(|row| row.iter().zip(q).map(|(a, b)| a * b).sum());
            let mut rgba = sample(image, uv, color);
            if normal {
                let n = [rgba[0], rgba[1], rgba[2]].map(|v| v * 2.0 - 1.0);
                // Change tangent basis as well as pixel placement for mirrored atlas islands.
                let target = [
                    basis[0][0] * n[0] + basis[1][0] * n[1],
                    basis[0][1] * n[0] + basis[1][1] * n[1],
                    n[2],
                ];
                let length = target.iter().map(|v| v * v).sum::<f64>().sqrt().max(1e-9);
                for channel in 0..3 {
                    rgba[channel] = target[channel] / length * 0.5 + 0.5;
                }
            }
            pixels.extend(rgba.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
        }
    }
    let image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        // Encoding copies RGBA bytes. glTF assigns linear/sRGB sampling when
        // these PNGs are loaded as normal/ORM/base-color textures respectively.
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );
    let mut png = Cursor::new(Vec::new());
    image
        .try_into_dynamic()
        .map_err(|e| e.to_string())?
        .write_to(&mut png, ImageFormat::Png.as_image_crate_format().unwrap())
        .map_err(|e| e.to_string())?;
    Ok(png.into_inner())
}

fn quad(side: usize) -> Group {
    let mut group = Group::default();
    let axis = side / 2;
    let [u, v] = AXES[side];
    let mut normal = [0.0; 3];
    normal[axis] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    for uv in [[0., 0.], [1., 0.], [1., 1.], [0., 1.]] {
        let mut point = [0.0; 3];
        point[axis] = (side % 2) as f32;
        point[u] = uv[0];
        point[v] = uv[1];
        group.positions.push(point);
        group.normals.push(normal);
        group.uvs.push(uv);
    }
    group.indices = vec![0, 1, 2, 0, 2, 3];
    let p = group
        .positions
        .iter()
        .map(|p| Vec3::from_array(*p))
        .collect::<Vec<_>>();
    if (p[1] - p[0])
        .cross(p[2] - p[0])
        .dot(Vec3::from_array(normal))
        < 0.0
    {
        for t in group.indices.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
    }
    group
}

fn import(bytes: &[u8], recipe: &Recipe) -> Result<(Vec<u8>, Value), String> {
    let source = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    let glb = Glb::from_slice(bytes).map_err(|e| e.to_string())?;
    let original: Value = serde_json::from_slice(&glb.json).map_err(|e| e.to_string())?;
    if source.buffers().len() != 1 || original.get("extensionsUsed").is_some() {
        return Err("Only self-contained, extension-free GLBs are supported".into());
    }
    let mut groups = partition(&source, recipe)?;
    // Measure the original atlas before sculpting its near-only relief.
    let fits: Vec<_> = groups
        .iter()
        .enumerate()
        .map(|(side, group)| fit_uv(group, side, recipe))
        .collect::<Result<_, _>>()?;
    let mut buffer = Buffer::default();
    let mut images = Vec::new();
    let mut pixels = Vec::new();
    let mut image_reports = Vec::new();
    for image in source.images() {
        let gltf::image::Source::View { view, mime_type } = image.source() else {
            return Err("External images are not supported".into());
        };
        if mime_type != "image/png" {
            return Err("Importer expects embedded PNG textures".into());
        }
        let start = view.offset();
        let payload = source
            .blob
            .as_deref()
            .and_then(|b| b.get(start..start + view.length()))
            .ok_or("Invalid embedded image range")?;
        let target = buffer.view(payload, None);
        let mut definition = original["images"][image.index()].clone();
        definition["bufferView"] = target.into();
        images.push(definition);
        pixels.push(decode(payload)?);
        image_reports.push(
            json!({"name":image.name(),"bytes":payload.len(),"fingerprint":fingerprint(payload)}),
        );
    }
    let mut materials = original["materials"]
        .as_array()
        .ok_or("Missing material")?
        .clone();
    let mut textures = original["textures"]
        .as_array()
        .ok_or("Missing textures")?
        .clone();
    let mut samplers = original["samplers"].as_array().cloned().unwrap_or_default();
    let repeat = samplers.len();
    samplers.push(json!({"magFilter":9729,"minFilter":9987,"wrapS":10497,"wrapT":10497}));
    let appearance_report = if let Some(settings) = &recipe.appearance {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let prepared = appearance::prepare(&root, settings)?;
        let mut overrides = Vec::new();
        for (name, png) in [
            ("ReferenceHdBaseColor", prepared.base_png),
            ("ReferenceHdNormal", prepared.normal_png),
            ("ReferenceHdOrm", prepared.orm_png),
        ] {
            let view = buffer.view(&png, None);
            let image = images.len();
            images.push(json!({"name":name,"mimeType":"image/png","bufferView":view}));
            pixels.push(decode(&png)?);
            overrides.push(textures.len());
            textures.push(json!({"sampler":0,"source":image}));
        }
        let material = &mut materials[0];
        material["name"] = "FiverrGrassReferenceHD".into();
        material["pbrMetallicRoughness"]["baseColorTexture"]["index"] = overrides[0].into();
        material["normalTexture"]["index"] = overrides[1].into();
        material["normalTexture"]["scale"] = 1.0.into();
        material["pbrMetallicRoughness"]["metallicRoughnessTexture"]["index"] = overrides[2].into();
        material["occlusionTexture"]["index"] = overrides[2].into();
        material["pbrMetallicRoughness"]["roughnessFactor"] = 1.0.into();
        material["pbrMetallicRoughness"]["metallicFactor"] = 0.0.into();
        prepared.report
    } else {
        Value::Null
    };
    let source_material = materials[0].clone();
    let bindings = [
        ["pbrMetallicRoughness", "baseColorTexture"],
        ["normalTexture", ""],
        ["pbrMetallicRoughness", "metallicRoughnessTexture"],
    ];
    let texture_index = |channel: usize| -> Result<usize, String> {
        let binding = if bindings[channel][1].is_empty() {
            &source_material[bindings[channel][0]]
        } else {
            &source_material[bindings[channel][0]][bindings[channel][1]]
        };
        if binding.get("texCoord").and_then(Value::as_u64).unwrap_or(0) != 0
            || binding.get("extensions").is_some()
        {
            return Err("Only untransformed UV0 texture bindings are supported".into());
        }
        binding["index"]
            .as_u64()
            .map(|i| i as usize)
            .ok_or("Missing original PBR texture".into())
    };
    let source_textures = [texture_index(0)?, texture_index(1)?, texture_index(2)?];
    let occlusion = &source_material["occlusionTexture"];
    if occlusion["index"].as_u64() != Some(source_textures[2] as u64)
        || occlusion
            .get("texCoord")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            != 0
        || occlusion.get("extensions").is_some()
    {
        return Err("Expected original shared ORM texture".into());
    }
    let relief_report = if let Some(settings) = &recipe.relief {
        relief::validate(settings)?;
        let source_image = textures[source_textures[0]]["source"]
            .as_u64()
            .ok_or("Base color texture has no embedded image")? as usize;
        let transforms = std::array::from_fn(|side| fits[side].0);
        relief::apply(
            &mut groups,
            &transforms,
            pixels
                .get(source_image)
                .ok_or("Base color image unavailable")?,
            settings,
        )?
    } else {
        Value::Null
    };
    let mut meshes = Vec::new();
    let mut reports = Vec::new();
    for (side, group) in groups.iter().enumerate() {
        meshes.push(json!({"name":SIDES[side],"primitives":[buffer.primitive(group,0)]}));
    }
    for (side, group) in groups.iter().enumerate() {
        let (fit, residual) = fits[side];
        let mut target = Vec::new();
        for (channel, source_texture) in source_textures.into_iter().enumerate() {
            let source_image = textures[source_texture]["source"]
                .as_u64()
                .ok_or("PBR texture has no embedded image")?
                as usize;
            let png = bake(
                pixels
                    .get(source_image)
                    .ok_or("PBR image index unavailable")?,
                fit,
                recipe.lod_tile_size,
                channel == 1,
                channel == 0,
            )?;
            let view = buffer.view(&png, None);
            let image = images.len();
            images.push(json!({"name":format!("{}Lod{}",SIDES[side],["BaseColor","Normal","ORM"][channel]),"mimeType":"image/png","bufferView":view}));
            target.push(textures.len());
            textures.push(json!({"sampler":repeat,"source":image}));
        }
        let mut material = source_material.clone();
        material["name"] = format!("{}Lod", SIDES[side]).into();
        material["pbrMetallicRoughness"]["baseColorTexture"]["index"] = target[0].into();
        material["normalTexture"]["index"] = target[1].into();
        material["pbrMetallicRoughness"]["metallicRoughnessTexture"]["index"] = target[2].into();
        material["occlusionTexture"]["index"] = target[2].into();
        materials.push(material);
        meshes.push(json!({"name":format!("{}Lod",SIDES[side]),"primitives":[buffer.primitive(&quad(side),side+1)]}));
        reports.push(json!({"name":SIDES[side],"vertices":group.positions.len(),"triangles":group.indices.len()/3,"minimum_normal_dominance":group.dominance,"lod_uv_from_positive_world_axes":fit,"max_uv_residual":residual,"lod_normal_basis":uv_basis(fit)?}));
    }
    let output = json!({
        "asset":{"version":"2.0","generator":"TERAMIC import_block_faces v1"},
        "scene":0,"scenes":[{"name":"Near","nodes":[0,1,2,3,4,5]},
            {"name":"Coarse","nodes":[6,7,8,9,10,11]}],
        "nodes":meshes.iter().enumerate().map(|(i,m)|json!({"mesh":i,"name":m["name"]})).collect::<Vec<_>>(),
        "meshes":meshes,"materials":materials,"textures":textures,"samplers":samplers,"images":images,
        "buffers":[{"byteLength":buffer.bytes.len()}],"bufferViews":buffer.views,"accessors":buffer.accessors
    });
    let glb = Glb {
        header: gltf::binary::Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: Cow::Owned(serde_json::to_vec(&output).map_err(|e| e.to_string())?),
        bin: Some(Cow::Owned(buffer.bytes)),
    }
    .to_vec()
    .map_err(|e| e.to_string())?;
    gltf::Gltf::from_slice(&glb).map_err(|e| format!("Generated GLB is invalid: {e}"))?;
    let provenance = json!({
        "schema_version":1,"importer":"crates/terra-render/examples/import_block_faces.rs",
        "recipe_schema":recipe.schema_version,"source":recipe.source,"source_bytes":bytes.len(),"source_fingerprint":fingerprint(bytes),
        "output":recipe.output,"output_bytes":glb.len(),"output_fingerprint":fingerprint(&glb),
        "translation":recipe.translation,"join_band":recipe.join_band,"lod_tile_size":recipe.lod_tile_size,
        "near_relief":relief_report,
        "appearance":appearance_report,
        "normal_bake":"transpose of normalized source-UV Jacobian; mirrored components corrected and vectors renormalized",
        "original_image_payloads_preserved":image_reports,"faces":reports
    });
    Ok((glb, provenance))
}

fn workspace_path(root: &Path, path: &Path) -> Result<PathBuf, String> {
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Recipe paths must be workspace-relative without parent traversal".into());
    }
    Ok(root.join(path))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut args = std::env::args().skip(1);
    let recipe_path = args
        .next()
        .unwrap_or_else(|| "tools/assets/import_fiverr_grass.json".into());
    let check = match args.next().as_deref() {
        None => false,
        Some("--check") => true,
        _ => return Err("Usage: import_block_faces [recipe.json] [--check]".into()),
    };
    if args.next().is_some() {
        return Err("Too many arguments".into());
    }
    let recipe: Recipe = serde_json::from_slice(&std::fs::read(workspace_path(
        &root,
        Path::new(&recipe_path),
    )?)?)?;
    if recipe.schema_version != 1
        || !(0.8..=1.0).contains(&recipe.minimum_normal_dominance)
        || !(0.0..=0.1).contains(&recipe.join_band)
        || !recipe.maximum_uv_residual.is_finite()
        || !(1e-9..=1e-4).contains(&recipe.maximum_uv_residual)
        || !(64..=1024).contains(&recipe.lod_tile_size)
        || recipe.expected_vertices > 100_000
        || recipe.expected_triangles > 200_000
    {
        return Err("Invalid import recipe limits".into());
    }
    let source = workspace_path(&root, &recipe.source)?;
    let output = workspace_path(&root, &recipe.output)?;
    let provenance_path = workspace_path(&root, &recipe.provenance)?;
    if source == output || source == provenance_path || output == provenance_path {
        return Err("Source/output/provenance paths must be distinct".into());
    }
    let bytes = std::fs::read(&source)?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Source GLB exceeds 64MiB import budget".into());
    }
    let (derived, provenance) = import(&bytes, &recipe)?;
    let mut provenance = serde_json::to_vec_pretty(&provenance)?;
    provenance.push(b'\n');
    for (path, data) in [(&output, derived), (&provenance_path, provenance)] {
        if check {
            if std::fs::read(path)? != data {
                return Err(format!("Generated asset is stale: {}", path.display()).into());
            }
        } else {
            std::fs::create_dir_all(path.parent().ok_or("Missing output parent")?)?;
            if std::fs::read(path).ok().as_deref() != Some(data.as_slice()) {
                std::fs::write(path, data)?;
            }
        }
    }
    println!(
        "{} {} (source unchanged)",
        if check { "Verified" } else { "Prepared" },
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe() -> Recipe {
        serde_json::from_str(include_str!(
            "../../../tools/assets/import_fiverr_grass.json"
        ))
        .unwrap()
    }

    #[test]
    fn original_triangles_partition_without_losing_attributes_and_uvs_fit_after_gutter_snap() {
        let bytes = include_bytes!("../../../FIVERR/VoxelGrassBlock glb file.glb");
        let source = gltf::Gltf::from_slice(bytes).unwrap();
        let recipe = recipe();
        let groups = partition(&source, &recipe).unwrap();
        assert_eq!(
            groups.iter().map(|g| g.positions.len()).sum::<usize>(),
            1494
        );
        for (side, group) in groups.iter().enumerate() {
            assert_eq!(group.indices.len(), 384);
            assert_eq!(group.normals.len(), group.positions.len());
            assert_eq!(group.uvs.len(), group.positions.len());
            assert!(fit_uv(group, side, &recipe).unwrap().1 < 1e-6);
        }
        assert!(side(Vec3::ONE, 0.95).is_err());
    }

    #[test]
    fn coarse_quads_have_positive_world_axis_uvs_and_outward_winding() {
        for side in 0..6 {
            let group = quad(side);
            for (p, uv) in group.positions.iter().zip(&group.uvs) {
                assert_eq!(*uv, [p[AXES[side][0]], p[AXES[side][1]]]);
            }
            for t in group.indices.chunks_exact(3) {
                let p = [t[0], t[1], t[2]].map(|i| Vec3::from_array(group.positions[i as usize]));
                assert!(
                    (p[1] - p[0])
                        .cross(p[2] - p[0])
                        .dot(Vec3::from_array(group.normals[0]))
                        > 0.99
                );
            }
        }
    }

    #[test]
    fn mirrored_normal_bake_changes_components_not_only_pixel_locations() {
        let pixels = Pixels {
            width: 1,
            height: 1,
            data: vec![204, 128, 230, 255],
        };
        let png = bake(
            &pixels,
            [[-0.5, 0.0, 0.75], [0.0, 0.5, 0.0]],
            64,
            true,
            false,
        )
        .unwrap();
        let decoded = decode(&png).unwrap();
        assert!(decoded.data[0] < 80 && decoded.data[1].abs_diff(128) < 2 && decoded.data[2] > 220);

        let rotated = decode(
            &bake(
                &pixels,
                [[0.0, 0.5, 0.0], [-0.5, 0.0, 0.75]],
                64,
                true,
                false,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(
            rotated.data[0].abs_diff(128) < 2 && rotated.data[1] > 190 && rotated.data[2] > 220
        );
    }

    #[test]
    fn derived_asset_preserves_original_pbr_and_embeds_six_repeatable_coarse_materials() {
        let original = include_bytes!("../../../FIVERR/VoxelGrassBlock glb file.glb");
        let mut recipe = recipe();
        recipe.lod_tile_size = 64;
        recipe.appearance = None;
        let (derived, report) = import(original, &recipe).unwrap();
        let source = gltf::Gltf::from_slice(original).unwrap();
        let output = gltf::Gltf::from_slice(&derived).unwrap();
        let source_json: Value =
            serde_json::from_slice(&Glb::from_slice(original).unwrap().json).unwrap();
        let output_json: Value =
            serde_json::from_slice(&Glb::from_slice(&derived).unwrap().json).unwrap();
        assert_eq!(source_json["materials"][0], output_json["materials"][0]);
        assert_eq!(output.meshes().len(), 12);
        assert_eq!(output.materials().len(), 7);
        assert_eq!(output.images().len(), 21);
        assert_eq!(output.default_scene().unwrap().nodes().count(), 6);
        assert_eq!(
            output.scenes().len(),
            2,
            "the coarse scene retains its PBR subasset handles"
        );
        assert_eq!(output.scenes().nth(1).unwrap().nodes().count(), 6);
        for image in source.images() {
            let gltf::image::Source::View { view, .. } = image.source() else {
                panic!("Original image should be embedded");
            };
            let target = output.images().nth(image.index()).unwrap();
            let gltf::image::Source::View { view: target, .. } = target.source() else {
                panic!("Derived image should be embedded");
            };
            assert_eq!(
                &source.blob.as_ref().unwrap()[view.offset()..view.offset() + view.length()],
                &output.blob.as_ref().unwrap()[target.offset()..target.offset() + target.length()]
            );
        }
        for (side, name) in SIDES.iter().enumerate() {
            let near = output.meshes().nth(side).unwrap();
            assert_eq!(near.name(), Some(*name));
            assert_eq!(
                near.primitives().next().unwrap().material().index(),
                Some(0)
            );
            let coarse = output.meshes().nth(side + 6).unwrap();
            assert_eq!(coarse.name(), Some(format!("{name}Lod").as_str()));
            let primitive = coarse.primitives().next().unwrap();
            assert_eq!(primitive.material().index(), Some(side + 1));
            let texture = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_texture()
                .unwrap()
                .texture();
            assert_eq!(
                texture.sampler().wrap_s(),
                gltf::texture::WrappingMode::Repeat
            );
            assert_eq!(
                texture.sampler().wrap_t(),
                gltf::texture::WrappingMode::Repeat
            );
            assert!(report["faces"][side]["max_uv_residual"].as_f64().unwrap() < 1e-6);
        }
    }

    #[test]
    fn hd_appearance_preserves_the_original_images_and_replaces_both_lod_materials() {
        let original = include_bytes!("../../../FIVERR/VoxelGrassBlock glb file.glb");
        let mut recipe = recipe();
        recipe.lod_tile_size = 64;
        let (derived, report) = import(original, &recipe).unwrap();
        let source = gltf::Gltf::from_slice(original).unwrap();
        let output = gltf::Gltf::from_slice(&derived).unwrap();
        let embedded = |asset: &gltf::Gltf, index: usize| {
            let gltf::image::Source::View { view, .. } =
                asset.images().nth(index).unwrap().source()
            else {
                panic!("embedded PNG required");
            };
            asset.blob.as_ref().unwrap()[view.offset()..view.offset() + view.length()].to_vec()
        };
        assert_eq!(output.materials().len(), 7);
        assert_eq!(output.images().len(), 24);
        for index in 0..3 {
            assert_eq!(embedded(&source, index), embedded(&output, index));
        }
        let material = output.materials().next().unwrap();
        let pbr = material.pbr_metallic_roughness();
        let base = pbr.base_color_texture().unwrap().texture().source().index();
        assert_eq!(base, 3);
        assert_eq!(
            embedded(&output, base),
            include_bytes!("../../../assets/terra/blocks/textures/grass-reference-hd-v1.png")
        );
        let pixels = decode(&embedded(&output, base)).unwrap();
        assert_eq!([pixels.width, pixels.height], [1254, 1254]);
        assert_eq!(pbr.metallic_factor(), 0.0);
        assert_eq!(pbr.roughness_factor(), 1.0);
        assert_eq!(material.normal_texture().unwrap().scale(), 1.0);
        let orm = pbr
            .metallic_roughness_texture()
            .unwrap()
            .texture()
            .source()
            .index();
        let orm = decode(&embedded(&output, orm)).unwrap();
        assert_eq!([orm.width, orm.height], [1, 1]);
        assert_eq!(orm.data, [255, 224, 0, 255]);
        assert_eq!(report["appearance"]["base_bytes_preserved"], true);
        for material in output.materials().skip(1) {
            let pbr = material.pbr_metallic_roughness();
            assert_eq!(pbr.metallic_factor(), 0.0);
            let image = pbr.base_color_texture().unwrap().texture().source().index();
            assert!(image >= 6, "coarse faces use the rebaked HD appearance");
        }
    }

    #[test]
    fn relief_changes_only_near_geometry_not_original_pbr_images_or_coarse_meshes() {
        let original = include_bytes!("../../../FIVERR/VoxelGrassBlock glb file.glb");
        let mut sculpted_recipe = recipe();
        sculpted_recipe.lod_tile_size = 64;
        let (sculpted, report) = import(original, &sculpted_recipe).unwrap();
        let mut plain_recipe = sculpted_recipe;
        plain_recipe.relief = None;
        let (plain, _) = import(original, &plain_recipe).unwrap();
        let sculpted = gltf::Gltf::from_slice(&sculpted).unwrap();
        let plain = gltf::Gltf::from_slice(&plain).unwrap();
        assert!(report["near_relief"].is_object());
        for face in report["near_relief"]["faces"].as_array().unwrap() {
            if face["side"] == 3 {
                continue;
            }
            let features = face["features"].as_array().unwrap();
            for kind in ["grass", "stone"] {
                assert!(
                    features.iter().any(|f| f["kind"] == kind),
                    "the real Fiverr face {} must contain {kind} relief",
                    face["side"]
                );
            }
        }
        for (a, b) in sculpted.images().zip(plain.images()) {
            let gltf::image::Source::View { view: a, .. } = a.source() else {
                panic!("embedded image required");
            };
            let gltf::image::Source::View { view: b, .. } = b.source() else {
                panic!("embedded image required");
            };
            assert_eq!(
                &sculpted.blob.as_ref().unwrap()[a.offset()..a.offset() + a.length()],
                &plain.blob.as_ref().unwrap()[b.offset()..b.offset() + b.length()]
            );
        }
        let mut enlarged = false;
        for side in 0..6 {
            let a = sculpted.meshes().nth(side).unwrap();
            let b = plain.meshes().nth(side).unwrap();
            enlarged |= a.primitives().next().unwrap().indices().unwrap().count()
                > b.primitives().next().unwrap().indices().unwrap().count();
            let a = sculpted.meshes().nth(side + 6).unwrap();
            let b = plain.meshes().nth(side + 6).unwrap();
            let a = a.primitives().next().unwrap();
            let b = b.primitives().next().unwrap();
            let a = a.reader(|_| sculpted.blob.as_deref());
            let b = b.reader(|_| plain.blob.as_deref());
            assert_eq!(
                a.read_positions().unwrap().collect::<Vec<_>>(),
                b.read_positions().unwrap().collect::<Vec<_>>()
            );
            assert_eq!(
                a.read_tex_coords(0).unwrap().into_f32().collect::<Vec<_>>(),
                b.read_tex_coords(0).unwrap().into_f32().collect::<Vec<_>>()
            );
            assert_eq!(
                a.read_indices().unwrap().into_u32().collect::<Vec<_>>(),
                b.read_indices().unwrap().into_u32().collect::<Vec<_>>()
            );
        }
        assert!(
            enlarged,
            "near geometry contains real relief, not just normal mapping"
        );
    }
}
