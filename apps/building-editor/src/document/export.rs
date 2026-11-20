use super::*;
use std::collections::BTreeSet;
use terra_core::building_jobs::{
    BuildArea, BuildPhase,
    template::{
        TEMPLATE_SCHEMA, TemplateCell, TemplateStep, cell_inputs, material_totals, rotated_facing,
    },
};

fn reserved([x, y, z]: [i64; 3]) -> bool {
    x == -2 && (0..=3).contains(&y) && ((-2 + y)..=1).contains(&z)
        || y == 3 && (-2..=2).contains(&x) && (-2..=2).contains(&z) && !(x == -2 && z <= 0)
}
fn phase(document: &Document, position: [i64; 3], cell: &AuthoredCell) -> BuildPhase {
    let local = std::array::from_fn(|axis| position[axis] - document.anchor[axis]);
    if reserved(local) {
        BuildPhase::Furnishing
    } else if local[1] < -1 {
        BuildPhase::Foundation
    } else if local[1] == -1 {
        BuildPhase::Floor
    } else if cell.state.peer.is_some()
        || matches!(
            cell.material,
            29 | 40 | 59 | 71..=73 | 99 | 100 | 105 | 107 | 108
        )
        || terra_core::profession_stations::is_station(cell.material)
        || cell.material == terra_core::flower_pot::MATERIAL
        || terra_core::lantern::is_part(cell.material)
        || terra_core::textiles::is_carpet(cell.material)
    {
        BuildPhase::Furnishing
    } else if local[1] >= 4 {
        BuildPhase::Roof
    } else {
        BuildPhase::Walls
    }
}
fn rotate_facing(cell: &AuthoredCell) -> bool {
    cell.material != terra_core::flower_pot::MATERIAL
        && !terra_core::lantern::is_part(cell.material)
        && !terra_core::textiles::is_carpet(cell.material)
        && (cell.state.peer.is_some()
            || terra_world::placement::furniture(cell.material)
            || matches!(cell.material, 40 | 99 | 100))
}
pub(super) fn template(document: &Document) -> Result<BuildingTemplate, String> {
    if document.cells.is_empty() {
        return Err("Place at least one block before saving / Vor dem Speichern mindestens einen Block platzieren".into());
    }
    let mut seen = BTreeSet::new();
    let mut steps = Vec::new();
    for (&position, cell) in &document.cells {
        // Validate every half, including secondary cells visited before primary.
        if !document.contains(position) || cell.material == 0 || !cell.valid_at(position) {
            return Err(format!("Invalid authored voxel at {position:?}"));
        }
        if cell.state.wood != native_wood(cell.material) || cell.state.rail_curve.is_some() {
            return Err(format!(
                "The schema cannot preserve this non-native wood/rail state at {position:?}"
            ));
        }
        if !seen.insert(position) {
            continue;
        }
        if cell.state.secondary {
            continue;
        }
        let to_cell = |position, cell: &AuthoredCell| TemplateCell {
            position,
            material: cell.material,
            facing: cell.state.facing,
            rotate_facing: rotate_facing(cell),
            peer: cell.state.peer,
            secondary: cell.state.secondary,
        };
        let mut cells = vec![to_cell(position, cell)];
        let mut build_phase = phase(document, position, cell);
        if let Some(peer) = cell.state.peer {
            let other = document
                .cells
                .get(&peer)
                .ok_or_else(|| format!("Missing paired half at {peer:?}"))?;
            if other.material != cell.material
                || other.state.peer != Some(position)
                || !other.state.secondary
            {
                return Err(format!("Broken object pair at {position:?}"));
            }
            cells.push(to_cell(peer, other));
            seen.insert(peer);
            build_phase = BuildPhase::Furnishing;
        }
        for cell in &cells {
            let _ = rotated_facing(cell, 0)?;
        }
        let costs = cell_inputs(&cells)?;
        steps.push(TemplateStep {
            phase: build_phase,
            cells,
            costs,
        });
    }
    if seen.len() != document.cells.len()
        || steps.iter().map(|step| step.cells.len()).sum::<usize>() != document.cells.len()
    {
        return Err("An orphaned secondary object half cannot be exported".into());
    }
    steps.sort_by_key(|step| {
        (
            step.phase,
            step.cells[0].position[1],
            step.cells[0].position[2],
            step.cells[0].position[0],
        )
    });
    let mut min = document.anchor;
    let mut max = document.anchor;
    for p in document.cells.keys() {
        for axis in 0..3 {
            min[axis] = min[axis].min(p[axis]);
            max[axis] = max[axis].max(p[axis]);
        }
    }
    let template = BuildingTemplate {
        schema_version: TEMPLATE_SCHEMA,
        id: document.id.clone(),
        display_name: document.name.clone(),
        category: document.category,
        anchor: document.anchor,
        bounds: BuildArea::new(min, max).map_err(|error| format!("{error:?}"))?,
        materials: material_totals(&steps)?,
        steps,
    };
    template.validate()?;
    Ok(template)
}
