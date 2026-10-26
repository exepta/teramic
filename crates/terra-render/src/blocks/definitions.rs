use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Component, Path},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolMetadata {
    item: terra_core::ItemKind,
    name: String,
    description: String,
    #[serde(default)]
    food: Option<terra_core::FoodDefinition>,
    #[serde(default)]
    equipment: Option<terra_core::EquipmentDefinition>,
}
pub(super) fn load_tool_metadata(
    path: &Path,
) -> Result<Vec<(terra_core::ItemKind, String, String)>, String> {
    let definitions: Vec<ToolMetadata> = read_definitions(path)?;
    let mut seen = HashSet::new();
    for definition in &definitions {
        if !terra_core::ItemKind::EXTRA.contains(&definition.item)
            || !seen.insert(definition.item)
            || definition.name.trim().is_empty()
            || definition.name.len() > 96
            || definition.description.is_empty()
            || definition.description.len() > 2048
            || definition
                .food
                .is_some_and(|food| !food.valid() || food.remainder == Some(definition.item))
            || definition.equipment.is_some_and(|equipment| {
                !equipment.valid()
                    || terra_core::equipment_definition(definition.item)
                        .is_none_or(|registered| registered.slot != equipment.slot)
            })
        {
            return Err("Invalid tool metadata".into());
        }
    }
    if seen.len() != terra_core::ItemKind::EXTRA.len() {
        return Err("Incomplete tool metadata".into());
    }
    Ok(definitions
        .into_iter()
        .map(|d| (d.item, d.name, d.description))
        .collect())
}

fn read_definitions<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>, String> {
    let mut files = std::fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .map(|entry| entry.map(|entry| entry.path()).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    files
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|p| {
            serde_json::from_str(&std::fs::read_to_string(&p).map_err(|e| e.to_string())?)
                .map_err(|e| format!("{}: {e}", p.display()))
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PropItemDefinition {
    kind: terra_core::ResourcePropKind,
    description: String,
    #[serde(default)]
    food: Option<terra_core::FoodDefinition>,
    #[serde(default = "nature_category")]
    creative_category: String,
    #[serde(default, rename = "hud_stats")]
    _hud_stats: terra_core::block_properties::HudStats,
}

pub(super) struct PropItemMetadata {
    pub descriptions: HashMap<terra_core::ResourcePropKind, String>,
    pub categories: HashMap<terra_core::ItemKind, String>,
}

fn nature_category() -> String {
    "nature".into()
}

pub(super) fn load_prop_metadata(path: &Path) -> Result<PropItemMetadata, String> {
    let items: Vec<PropItemDefinition> = read_definitions(path)?;
    let mut descriptions = HashMap::new();
    let mut categories = HashMap::new();
    for item in items {
        if item.description.trim().is_empty()
            || item.description.len() > 2048
            || !valid_category_id(&item.creative_category)
            || item.creative_category == "all"
            || item.food.is_some_and(|food| {
                !food.valid() || food.remainder == Some(terra_core::ItemKind::Prop(item.kind))
            })
            || descriptions.insert(item.kind, item.description).is_some()
        {
            return Err(format!(
                "{}: empty, oversized or duplicated prop description",
                path.display()
            ));
        }
        categories.insert(
            terra_core::ItemKind::Prop(item.kind),
            item.creative_category,
        );
    }
    if descriptions.len() != terra_core::ResourcePropKind::ALL.len() {
        return Err(format!(
            "{}: descriptions for all registered prop kinds are required",
            path.display()
        ));
    }
    Ok(PropItemMetadata {
        descriptions,
        categories,
    })
}

pub(super) fn valid_category_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':'))
        && id.bytes().any(|byte| byte.is_ascii_alphanumeric())
}

#[path = "creative.rs"]
mod creative;
pub(super) use creative::{item_category, load_creative_categories};

/// In-memory registry discovered from independent block JSON files.
#[derive(Clone, Debug)]
pub struct BlockLibrary {
    pub blocks: Vec<BlockDefinition>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockDefinition {
    pub schema_version: u32,
    pub id: u16,
    #[serde(default)]
    pub light: Option<BlockLight>,
    pub name: String,
    #[serde(flatten)]
    pub gameplay: terra_core::block_properties::BlockProperties,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub creative_category: Option<String>,
    #[serde(default)]
    pub render_kind: BlockRenderKind,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub collision_boxes: Vec<terra_core::block_shapes::ShapeBox>,
    #[serde(default)]
    pub model_layout: ModelLayout,
    /// Width of the model's bevel band, in one-meter cell coordinates.
    #[serde(default)]
    pub bevel_width: f32,
    /// Optional visual-only, inward-cut surface detail for cube blocks.
    #[serde(default)]
    pub surface: Option<CubeSurface>,
    #[serde(default)]
    pub textures: BTreeMap<String, String>,
    #[serde(default)]
    pub materials: BTreeMap<String, CubeMaterialDefinition>,
    #[serde(default)]
    pub face_materials: Option<[String; 6]>,
    #[serde(default)]
    pub side_cap: Option<SideCapDefinition>,
    /// Optional joined window frame, emitted only on exposed pane boundaries.
    #[serde(default)]
    pub frame_material: Option<String>,
    #[serde(default)]
    pub crystal_material: Option<String>,
    #[serde(default = "face_names")]
    pub faces: [String; 6],
    #[serde(default)]
    pub details: Vec<DetailDefinition>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockLight {
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    #[serde(default)]
    pub falloff: LightFalloff,
    #[serde(default)]
    pub flicker: f32,
    #[serde(default = "unit_light_gain")]
    pub cave_intensity: f32,
    #[serde(default)]
    pub offset: Option<[f32; 3]>,
    #[serde(default = "light_shadows")]
    pub shadows: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LightFalloff {
    #[default]
    InverseSquare,
    SoftCore,
}

fn unit_light_gain() -> f32 {
    1.0
}
fn light_shadows() -> bool {
    true
}

/// Missing render kinds retain the original schema-v1 GLB interpretation.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockRenderKind {
    Cube,
    Segment,
    /// Shared textured box geometry, authored by the same collision profile.
    Boxes,
    #[default]
    CustomMesh,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CubeSurface {
    Wool,
    Turf,
    Earth,
    Rock,
    Sand,
    Snow,
    Wood,
    Mushroom,
    CoralRed,
    CoralBlue,
    CoralYellow,
    CoralPurple,
    Farmland,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelLayout {
    #[default]
    FaceGroups,
    /// Entire default glTF scene, baked into one static, non-occluding block model.
    Scene,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CubeMaterialDefinition {
    #[serde(default)]
    pub emission: [f32; 3],
    /// Linear RGBA, matching glTF's baseColorFactor convention.
    #[serde(default = "white")]
    pub base_color: [f32; 4],
    #[serde(default = "roughness")]
    pub roughness: f32,
    #[serde(default = "reflectance")]
    pub reflectance: f32,
    #[serde(default)]
    pub metallic: f32,
    #[serde(default)]
    pub double_sided: bool,
    #[serde(default)]
    pub texture_pattern: Option<TexturePattern>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TexturePattern {
    Wool,
    PathTop,
    PathSide,
    Turf,
    Earth,
    Farmland,
    FarmlandSide,
    Canopy,
    BirchCanopy,
    Gravel,
    StructureStone,
    Mineral,
    BirchBark,
    SpruceBark,
    SpruceEnd,
    Dripstone,
    Sand,
    Sandstone,
    Stone,
    SmoothStone,
    MossyStone,
    Planks,
    Clay,
    MushroomStem,
    MushroomGills,
    PurpleMushroomCap,
    RedMushroomCap,
    YellowMushroomCap,
    GreenMushroomCap,
    BlueMushroomCap,
    WhiteMushroomCap,
    CoralRed,
    CoralBlue,
    CoralYellow,
    CoralPurple,
    Mud,
    Ice,
    BlossomCanopy,
    CherryBark,
    CherryEnd,
}

fn white() -> [f32; 4] {
    [1.0; 4]
}

fn roughness() -> f32 {
    0.92
}

fn reflectance() -> f32 {
    0.5
}

impl Default for CubeMaterialDefinition {
    fn default() -> Self {
        Self {
            emission: [0.0; 3],
            base_color: white(),
            roughness: roughness(),
            reflectance: reflectance(),
            metallic: 0.0,
            double_sided: false,
            texture_pattern: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SideCapDefinition {
    pub material: String,
    pub height: f32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailDefinition {
    pub mesh: String,
    pub probability: f32,
    /// Restrict shore vegetation to a top-face height above the configured water level.
    #[serde(default)]
    pub water_clearance: Option<[f32; 2]>,
    #[serde(default)]
    pub density_field: Option<DetailDensity>,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailDensity {
    Grass,
    Bush,
    Flower,
    Fern,
}

fn face_names() -> [String; 6] {
    ["West", "East", "Down", "Up", "North", "South"].map(String::from)
}

fn relative_asset(path: &str, extension: &str) -> bool {
    !path.contains(['#', '?'])
        && Path::new(path).extension().is_some_and(|e| e == extension)
        && Path::new(path)
            .components()
            .all(|p| matches!(p, Component::Normal(_)))
}

impl BlockLibrary {
    pub fn load(directory: &Path) -> Result<Self, String> {
        let entries =
            std::fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))?;
        let mut blocks = Vec::new();
        for entry in entries {
            let path = entry
                .map_err(|e| format!("{}: {e}", directory.display()))?
                .path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let source =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let block: BlockDefinition =
                serde_json::from_str(&source).map_err(|e| format!("{}: {e}", path.display()))?;
            block
                .validate()
                .map_err(|e| format!("{}: {e}", path.display()))?;
            blocks.push(block);
        }
        blocks.sort_by_key(|b| b.id);
        let library = Self { blocks };
        library.validate()?;
        Ok(library)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.blocks.is_empty() {
            return Err("No block JSON definitions found".into());
        }
        let mut ids = HashSet::new();
        for block in &self.blocks {
            block.validate()?;
            if !ids.insert(block.id) {
                return Err(format!("Duplicate block ID {} ({})", block.id, block.name));
            }
        }
        Ok(())
    }
}

impl BlockDefinition {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.light.is_some_and(|light| {
                light
                    .color
                    .iter()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
                    || !light.intensity.is_finite()
                    || !(0.0..=100_000.0).contains(&light.intensity)
                    || !light.range.is_finite()
                    || !(0.1..=32.0).contains(&light.range)
                    || !light.flicker.is_finite()
                    || !(0.0..=0.15).contains(&light.flicker)
                    || !light.cave_intensity.is_finite()
                    || !(0.0..=1.0).contains(&light.cave_intensity)
                    || light.offset.is_some_and(|p| {
                        p.into_iter()
                            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
                    })
            })
            || self.materials.values().any(|m| {
                m.emission
                    .iter()
                    .any(|v| !v.is_finite() || !(0.0..=16.0).contains(v))
            })
            || self.description.len() > 2048
            || self
                .creative_category
                .as_deref()
                .is_some_and(|id| !valid_category_id(id) || id == "all")
            || self.id == 0
            || self.name.trim().is_empty()
            || !self.gameplay.valid()
            || self.collision_boxes.len() > 128
            || !self
                .collision_boxes
                .iter()
                .all(|shape| shape.valid_for(self.id))
            || (!self.model.is_empty()
                && !relative_asset(&self.model, "glb")
                && !relative_asset(&self.model, "gltf"))
            || (self.render_kind == BlockRenderKind::CustomMesh && self.model.is_empty())
            || (self.render_kind == BlockRenderKind::Boxes
                && (self.collision_boxes.is_empty() || !self.model.is_empty()))
            || (self.render_kind == BlockRenderKind::Segment
                && (!(50..=56).contains(&self.id) || !self.model.is_empty()))
            || !self.bevel_width.is_finite()
            || !(0.0..=0.25).contains(&self.bevel_width)
            || (self.surface.is_some() && self.render_kind != BlockRenderKind::Cube)
            || (self.model_layout == ModelLayout::Scene
                && (self.render_kind != BlockRenderKind::CustomMesh
                    || self.bevel_width != 0.0
                    || !self.details.is_empty()))
            || self
                .textures
                .iter()
                .any(|(material, path)| material.trim().is_empty() || !relative_asset(path, "png"))
            || self.faces.iter().any(|s| s.trim().is_empty())
            || self.materials.iter().any(|(name, material)| {
                name.trim().is_empty()
                    || (material.texture_pattern.is_some() && self.textures.contains_key(name))
                    || material
                        .base_color
                        .iter()
                        .chain([
                            &material.roughness,
                            &material.metallic,
                            &material.reflectance,
                        ])
                        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            })
            || (matches!(
                self.render_kind,
                BlockRenderKind::Cube | BlockRenderKind::Segment | BlockRenderKind::Boxes
            ) && (self
                .face_materials
                .as_ref()
                .is_none_or(|faces| faces.iter().any(|name| !self.materials.contains_key(name)))
                || self
                    .textures
                    .keys()
                    .any(|name| !self.materials.contains_key(name))))
            || self.side_cap.as_ref().is_some_and(|cap| {
                self.render_kind != BlockRenderKind::Cube
                    || !self.materials.contains_key(&cap.material)
                    || !cap.height.is_finite()
                    || !(0.01..=0.25).contains(&cap.height)
            })
            || self.frame_material.as_ref().is_some_and(|name| {
                self.render_kind != BlockRenderKind::Cube || !self.materials.contains_key(name)
            })
            || self.crystal_material.as_ref().is_some_and(|name| {
                self.render_kind != BlockRenderKind::Cube || !self.materials.contains_key(name)
            })
            || self.details.iter().any(|d| {
                d.mesh.trim().is_empty()
                    || !d.probability.is_finite()
                    || !(0.0..=1.0).contains(&d.probability)
                    || d.water_clearance.is_some_and(|[low, high]| {
                        !low.is_finite()
                            || !high.is_finite()
                            || low < 0.0
                            || high < low
                            || high > 16.0
                    })
            })
        {
            return Err(format!(
                "Invalid block definition for ID {} ({})",
                self.id, self.name
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod surface_tests {
    use super::*;

    #[test]
    fn surface_styles_are_optional_for_legacy_assets_and_invalid_for_custom_meshes() {
        let legacy: BlockDefinition = serde_json::from_str(
            r#"{"schema_version":1,"id":101,"name":"Legacy","model":"legacy.glb"}"#,
        )
        .expect("schema v1");
        assert!(legacy.surface.is_none());
        assert!(legacy.validate().is_ok());
        let mut invalid = legacy;
        invalid.surface = Some(CubeSurface::Rock);
        assert!(invalid.validate().is_err());
    }
}
