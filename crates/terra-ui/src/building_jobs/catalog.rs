//! Presentation of the authority's validated catalog; no local cost guesses.
use super::*;
use terra_core::building_jobs::template::{TemplateCategory, TemplateSummary};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TemplateRow {
    pub id: String,
    pub name: String,
    pub category: String,
    pub dimensions: String,
    pub preview: String,
    pub selected: bool,
    pub category_path: String,
}

pub(super) struct CatalogView {
    pub rows: Vec<TemplateRow>,
    pub selected: bool,
    pub name: String,
    pub category: String,
    pub dimensions: String,
    pub steps: String,
    pub preview: String,
    pub materials: Vec<BuildingMaterialRow>,
    pub error: String,
}

fn category(template: &TemplateSummary) -> &'static str {
    match template.category {
        TemplateCategory::Residential => "Wohngebäude",
    }
}
fn dimensions(template: &TemplateSummary) -> String {
    let [x, y, z] = template.dimensions;
    format!("{x} × {y} × {z}")
}
fn preview(id: &str) -> String {
    let file = match id {
        terra_core::building_jobs::HOUSE_TEMPLATE => Some("temperate-house.svg"),
        "spruce-cottage-v1" => Some("spruce-cottage.svg"),
        _ => None,
    };
    file.map_or_else(
        || "tilt-icon:home@64".into(),
        |file| {
            format!(
                "{}/../../src-ui/assets/buildings/{file}",
                env!("CARGO_MANIFEST_DIR")
            )
        },
    )
}

pub(super) fn name(session: &ClientSession, id: &str) -> String {
    session
        .buildings
        .templates
        .iter()
        .find(|template| template.id == id)
        .map_or_else(|| id.to_owned(), |template| template.display_name.clone())
}

fn materials(
    world: &World,
    session: &ClientSession,
    kingdom: Option<KingdomId>,
    template: &TemplateSummary,
) -> Vec<BuildingMaterialRow> {
    let palette = world.get_resource::<terra_player::interaction::BlockPalette>();
    template
        .materials
        .iter()
        .map(|stack| {
            let (supply, sufficient) =
                super::stock::supply(session, kingdom, stack.item, u32::from(stack.count));
            BuildingMaterialRow {
                identifier: stack.item.identifier(),
                label: material_label(stack.item, palette),
                count: stack.count.to_string(),
                preview_key: crate::inventory::preview_key(stack.item),
                supply,
                sufficient,
            }
        })
        .collect()
}

/// Block inputs already identify their authored species through the material ID.
/// Keep its exact metadata name so the normal locale projection can translate it.
pub(super) fn material_label(
    item: terra_core::ItemKind,
    palette: Option<&terra_player::interaction::BlockPalette>,
) -> String {
    let Some(palette) = palette else {
        return item.identifier();
    };
    if let terra_core::ItemKind::Block(id) = item
        && let Some(choice) = palette.choices.iter().find(|choice| choice.id == id)
    {
        return choice.name.clone();
    }
    crate::inventory::item_label(item, palette)
}

pub(super) fn project(
    world: &World,
    session: &ClientSession,
    selected: &str,
    kingdom: Option<KingdomId>,
) -> CatalogView {
    let rows = session
        .buildings
        .templates
        .iter()
        .filter(|template| template.valid())
        .map(|template| TemplateRow {
            id: template.id.clone(),
            name: template.display_name.clone(),
            category: category(template).into(),
            dimensions: dimensions(template),
            preview: preview(&template.id),
            selected: template.id == selected,
            category_path: template.category_path.clone(),
        })
        .collect();
    let selected = session
        .buildings
        .templates
        .iter()
        .find(|template| template.id == selected && template.valid());
    let error = if session.buildings.catalog_error.is_some() {
        "Bauvorlagen konnten nicht geladen werden"
    } else if session.buildings.templates.is_empty() {
        "Bauvorlagen werden geladen …"
    } else if selected.is_none() {
        "Diese Bauvorlage ist nicht mehr verfügbar. Bitte eine andere wählen."
    } else {
        ""
    };
    CatalogView {
        rows,
        selected: selected.is_some() && error.is_empty(),
        name: selected.map_or_else(String::new, |template| template.display_name.clone()),
        category: selected.map_or_else(String::new, |template| category(template).into()),
        dimensions: selected.map_or_else(String::new, dimensions),
        steps: selected.map_or_else(String::new, |template| template.steps.to_string()),
        preview: selected.map_or_else(String::new, |template| preview(&template.id)),
        materials: selected.map_or_else(Vec::new, |template| {
            materials(world, session, kingdom, template)
        }),
        error: error.into(),
    }
}
