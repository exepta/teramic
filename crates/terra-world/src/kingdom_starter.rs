//! One bounded, read-only placement transaction over loaded terrain.
use crate::creatures::Habitat;
use std::collections::{BTreeSet, VecDeque};
use terra_core::{
    creatures::CreatureKind,
    kingdom::{KingdomError, STARTER_COUNT, claim_chunk},
};

const RADIUS: i64 = 16;
const SEARCH_BUDGET: usize = 2048;

#[cfg(test)]
#[path = "kingdom_starter_tests.rs"]
mod tests;

fn bounds(position: [f64; 3], size: [f64; 3]) -> [[f64; 3]; 2] {
    [
        [
            position[0] - size[0] * 0.5,
            position[1] + 0.02,
            position[2] - size[2] * 0.5,
        ],
        [
            position[0] + size[0] * 0.5,
            position[1] + size[1],
            position[2] + size[2] * 0.5,
        ],
    ]
}
fn overlaps(a: [[f64; 3]; 2], b: [[f64; 3]; 2]) -> bool {
    (0..3).all(|axis| a[0][axis] < b[1][axis] + 0.08 && a[1][axis] > b[0][axis] - 0.08)
}
fn terrain_floor(habitat: &Habitat, position: [f64; 3]) -> bool {
    let support = [
        position[0].floor() as i64,
        (position[1] - 0.01).floor() as i64,
        position[2].floor() as i64,
    ];
    // Initial residents require untouched natural ground. Authored floors and
    // even player-placed terrain blocks are excluded, including flat roofs.
    habitat.journal.block(support).is_none()
        && habitat
            .block(support)
            .and_then(terra_voxel::MaterialId::from_raw)
            .is_some_and(terra_voxel::MaterialId::is_natural_ground)
}

fn dry_body(habitat: &Habitat, position: [f64; 3], size: [f64; 3]) -> bool {
    let body = bounds(position, size);
    let low = body[0].map(|v| v.floor() as i64);
    let high = body[1].map(|v| (v - 0.000001).floor() as i64);
    for x in low[0]..=high[0] {
        for y in low[1]..=high[1] {
            for z in low[2]..=high[2] {
                let cell = [x, y, z];
                let Some(block) = habitat.block(cell) else {
                    return false;
                };
                // Persisted water blocks remain unsafe even if procedural water
                // is disabled in the current world's settings.
                if matches!(
                    terra_voxel::MaterialId::from_raw(block),
                    Some(terra_voxel::MaterialId::FluidWater | terra_voxel::MaterialId::SolidWater)
                ) {
                    return false;
                }
                let fluid = habitat.journal.fluid(cell).unwrap_or_else(|| {
                    habitat
                        .chunks
                        .fluid_cell_at(habitat.terrain, habitat.journal, cell, block)
                });
                if fluid.amount > 0 && body[0][1] < y as f64 + fluid.height() {
                    return false;
                }
            }
        }
    }
    for x in [body[0][0] + 0.0001, body[1][0] - 0.0001] {
        for z in [body[0][2] + 0.0001, body[1][2] - 0.0001] {
            if habitat.wet([x, body[0][1], z]) {
                return false;
            }
        }
    }
    true
}

/// Every result is in the new keep's claimed chunk, dry, supported and connected
/// to the founder by verified walking steps. Searches never generate or load chunks.
pub fn plan(
    habitat: &Habitat,
    keep: [i64; 3],
    founder: terra_core::PlayerUuid,
    founder_feet: [f64; 3],
    player_bounds: &[[[f64; 3]; 2]],
) -> Result<[[f64; 3]; STARTER_COUNT], KingdomError> {
    if !terra_core::kingdom::valid_location(keep)
        || founder_feet
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 998_900.)
    {
        return Err(KingdomError::StarterAreaUnavailable);
    }
    let kind = CreatureKind::Villager;
    let seed = (0..=8)
        .find_map(|down| {
            let probe = [
                founder_feet[0],
                founder_feet[1] - f64::from(down),
                founder_feet[2],
            ];
            habitat.ground(probe, kind)
        })
        .ok_or(KingdomError::StarterAreaUnavailable)?;
    let game = habitat.journal.gameplay();
    let area = [
        [
            (keep[0] - RADIUS - 2) as f64,
            -999_000.,
            (keep[2] - RADIUS - 2) as f64,
        ],
        [
            (keep[0] + RADIUS + 2) as f64,
            999_000.,
            (keep[2] + RADIUS + 2) as f64,
        ],
    ];
    let vehicles: Vec<_> = game
        .trains
        .vehicles
        .values()
        .map(|vehicle| vehicle.model_bounds())
        .filter(|bounds| overlaps(area, *bounds))
        .collect();
    let minimum =
        terra_voxel::global_to_section([keep[0] - RADIUS - 2, keep[1], keep[2] - RADIUS - 2]).0;
    let maximum =
        terra_voxel::global_to_section([keep[0] + RADIUS + 2, keep[1], keep[2] + RADIUS + 2]).0;
    let props: Vec<_> = habitat
        .chunks
        .loaded
        .iter()
        .filter(|(section, _)| {
            (minimum.x..=maximum.x).contains(&section.x)
                && (minimum.z..=maximum.z).contains(&section.z)
        })
        .flat_map(|(_, section)| &section.props)
        .filter(|prop| {
            matches!(
                prop.kind,
                crate::props::PropKind::Bush
                    | crate::props::PropKind::BeeNest
                    | crate::props::PropKind::Scarecrow
                    | crate::props::PropKind::SpiderWeb
            )
        })
        .map(|prop| prop.bounds())
        .filter(|bounds| overlaps(area, *bounds))
        .collect();
    let mut reached = BTreeSet::new();
    let mut queue = VecDeque::from([seed]);
    let mut positions: Vec<[f64; 3]> = Vec::with_capacity(STARTER_COUNT);
    while let Some(position) = queue.pop_front() {
        let cell = position.map(|v| v.floor() as i64);
        if (cell[0] - keep[0]).abs() > RADIUS
            || (cell[2] - keep[2]).abs() > RADIUS
            || !crate::kingdom_access::can_use(
                &game.kingdoms,
                habitat.chunks,
                founder,
                terra_core::kingdom::KingdomPermission::NpcWork,
                cell,
            )
            || reached.len() >= SEARCH_BUDGET
            || !reached.insert(cell)
        {
            continue;
        }
        let body = bounds(position, kind.size());
        if !dry_body(habitat, position, kind.size())
            || vehicles
                .iter()
                .chain(&props)
                .any(|other| overlaps(body, *other))
        {
            continue;
        }
        if claim_chunk(cell) == claim_chunk(keep)
            && (position[0] - keep[0] as f64 - 0.5).hypot(position[2] - keep[2] as f64 - 0.5)
                >= 1.65
            && terrain_floor(habitat, position)
            && !player_bounds.iter().any(|other| overlaps(body, *other))
            && !game
                .creatures
                .animals
                .values()
                .filter(|other| other.health > 0.)
                .any(|other| overlaps(body, bounds(other.position, other.size())))
            && !positions
                .iter()
                .any(|other| (position[0] - other[0]).hypot(position[2] - other[2]) < 1.15)
        {
            positions.push(position);
            if positions.len() == STARTER_COUNT {
                break;
            }
        }
        for [dx, dz] in [[1., 0.], [-1., 0.], [0., 1.], [0., -1.]] {
            let target = [position[0] + dx, position[1], position[2] + dz];
            if let Some(next) = habitat.walk_step(position, target, kind, 0.) {
                queue.push_back(next);
            }
        }
    }
    positions
        .try_into()
        .map_err(|_| KingdomError::StarterAreaUnavailable)
}
