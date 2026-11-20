//! Export the existing village layout as editable validated JSON, without loot or NPCs.
use super::planning::point;
use super::*;
use crate::generation::structures::StructureObject;
use std::collections::BTreeSet;
use terra_core::ItemStack;
use terra_core::building_jobs::template::{
    BuildingTemplate, TEMPLATE_SCHEMA, TemplateCell, TemplateStep, material_totals,
};

pub(super) fn captured(origin: [i64; 3], facing: u8) -> Result<Vec<BuildStep>, String> {
    let objects = crate::generation::structures::default_house_template()?;
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for object in objects.iter().filter(|c| c.material != 0) {
        if !seen.insert(object.position) {
            continue;
        }
        let state = object.state.unwrap_or_default();
        if state.secondary {
            continue;
        }
        let p = object.position;
        let phase = if p[1] < -1 {
            BuildPhase::Foundation
        } else if p[1] == -1 {
            BuildPhase::Floor
        } else if object.state.is_some()
            && (state.peer.is_some()
                || matches!(object.material, 29 | 40 | 71 | 73 | 105 | 107 | 108)
                || object.material == terra_core::lantern::LANTERN
                || object.material == terra_core::flower_pot::MATERIAL
                || terra_core::textiles::is_carpet(object.material))
        {
            BuildPhase::Furnishing
        } else if p[1] >= 4 {
            BuildPhase::Roof
        } else {
            BuildPhase::Walls
        };
        let cell = |o: &StructureObject| {
            let s = o.state.unwrap_or_default();
            BuildCell {
                position: point(origin, o.position, facing),
                material: o.material,
                facing: if o.state.is_none()
                    || terra_core::construction::is_slab(o.material)
                    || o.material == terra_core::flower_pot::MATERIAL
                    || o.material == terra_core::lantern::LANTERN
                {
                    s.facing
                } else {
                    (s.facing + facing) % 4
                },
                peer: s.peer.map(|p| point(origin, p, facing)),
                secondary: s.secondary,
            }
        };
        let mut cells = vec![cell(object)];
        if let Some(peer) = state.peer {
            let other = objects
                .iter()
                .find(|o| o.position == peer && o.material == object.material)
                .ok_or("Default authored house has an unmatched object pair")?;
            cells.push(cell(other));
            seen.insert(peer);
        }
        let mut costs = vec![ItemStack {
            item: ItemKind::Block(object.material),
            count: if object.material == 29 && cells.len() == 2 {
                2
            } else {
                1
            },
        }];
        if object.material == terra_core::flower_pot::MATERIAL
            && let Some(item) = terra_core::flower_pot::contents(state.facing)
        {
            costs.push(ItemStack { item, count: 1 });
        }
        result.push(BuildStep {
            phase,
            cells,
            costs,
        });
    }
    result.sort_by_key(|s| {
        (
            s.phase,
            s.cells[0].position[1],
            s.cells[0].position[2],
            s.cells[0].position[0],
        )
    });
    Ok(result)
}

pub fn export_default_house_template() -> Result<BuildingTemplate, String> {
    let objects = crate::generation::structures::default_house_template()?;
    let cardinal: BTreeSet<_> = objects
        .iter()
        .filter(|object| {
            object.state.is_some()
                && !terra_core::construction::is_slab(object.material)
                && object.material != terra_core::flower_pot::MATERIAL
                && object.material != terra_core::lantern::LANTERN
        })
        .map(|object| object.position)
        .collect();
    let steps: Vec<_> = captured([0; 3], 0)?
        .into_iter()
        .map(|step| TemplateStep {
            phase: step.phase,
            costs: step.costs,
            cells: step
                .cells
                .into_iter()
                .map(|cell| TemplateCell {
                    position: cell.position,
                    material: cell.material,
                    facing: cell.facing,
                    rotate_facing: cardinal.contains(&cell.position),
                    peer: cell.peer,
                    secondary: cell.secondary,
                })
                .collect(),
        })
        .collect();
    let mut minimum = [i64::MAX; 3];
    let mut maximum = [i64::MIN; 3];
    for cell in steps.iter().flat_map(|step| &step.cells) {
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(cell.position[axis]);
            maximum[axis] = maximum[axis].max(cell.position[axis]);
        }
    }
    let template = BuildingTemplate {
        schema_version: TEMPLATE_SCHEMA,
        id: HOUSE_TEMPLATE.into(),
        display_name: "Temperate village house".into(),
        category: terra_core::building_jobs::template::TemplateCategory::Residential,
        anchor: [0; 3],
        bounds: BuildArea::new(minimum, maximum)
            .map_err(|error| format!("Invalid authored house bounds: {error:?}"))?,
        materials: material_totals(&steps)?,
        steps,
    };
    template.validate()?;
    Ok(template)
}

#[cfg(test)]
mod tests;
