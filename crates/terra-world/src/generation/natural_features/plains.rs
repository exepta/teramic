//! Coherent flower meadows and small fishing landings along open rivers.
use super::{Cell, Habitat, TILE_SIZE, Tile, ticket};
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

pub(super) fn build(h: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    let origin = tile.map(|i| i * TILE_SIZE);
    let middle = origin.map(|i| i + TILE_SIZE / 2);
    if h.terrain
        .biome_at(middle[0] as f64, middle[1] as f64)
        .weights
        .plains
        < 0.30
    {
        return;
    }
    let world_seed = h.terrain.config().seed.0;
    let shore_ticket = ticket(world_seed, tile, 3062);
    if h.terrain.config().structures.villages
        && h.terrain.config().structures_revision > 0
        && h.terrain.config().water
        && shore_ticket.is_multiple_of(5)
        && !h.reserved(middle, 17)
    {
        for [dx, dz] in [[-5, 0], [5, 0], [0, 5], [0, -5]] {
            let p = [middle[0] + dx, middle[1] + dz];
            if [[1, 0], [-1, 0], [0, 1], [0, -1]]
                .into_iter()
                .any(|d| river_landing(h, p, d, cells))
            {
                break;
            }
        }
    }
    for gz in 0..8 {
        for gx in 0..8 {
            let grid = [tile[0] * 8 + gx, tile[1] * 8 + gz];
            let hash = ticket(world_seed, grid, 3061);
            let p = [
                origin[0] + gx * 4 + i64::from(hash & 3),
                origin[1] + gz * 4 + i64::from((hash >> 2) & 3),
            ];
            let c = h.column(p);
            if c.biome.weights.plains < 0.46
                || c.cave.open_to_sky()
                || c.river_cavity.is_some()
                || h.reserved(p, 1)
            {
                continue;
            }
            if let Some(level) = c.water_level {
                if level - c.height > 0.5
                    && level - c.height < 2.8
                    && c.water_flow[0].hypot(c.water_flow[1]) < 0.75
                    && c.ocean.ice_depth == 0
                    && hash.is_multiple_of(3)
                {
                    let y = level.ceil() as i64;
                    if !cells.contains_key(&[p[0], y + 1, p[1]]) {
                        cells.entry([p[0], y, p[1]]).or_insert(M::LilyPad as u16);
                    }
                }
                continue;
            }
            if c.surface_material != M::Grass || c.slope > 0.9 {
                continue;
            }
            let x = p[0] as f64 + 0.5;
            let z = p[1] as f64 + 0.5;
            let [glade, meadow, shrub, _] = h.terrain.climate.vegetation_masks(x, z);
            let abundance = (0.12 + 0.88 * meadow * glade) * c.biome.weights.plains;
            if f64::from(hash >> 16) / 65536.0 >= abundance
                || h.terrain.climate.ground_masks(x, z).1 < 0.42
            {
                continue;
            }
            let Some((floor, _)) = h.ground(p) else {
                continue;
            };
            let cell = [p[0], floor + 1, p[1]];
            if h.tree_occupied(cell) || cells.contains_key(&[p[0], floor, p[1]]) {
                continue;
            }
            // Broad waves of related colours with occasional companion flowers.
            let flowers = if shrub > 0.65 {
                [M::RoseFoxgloves, M::Lupine, M::FloweringShrub]
            } else if meadow > 0.56 {
                [M::MeadowDaisies, M::MeadowSunflowers, M::RoseFoxgloves]
            } else {
                [M::MeadowSunflowers, M::Lupine, M::MeadowDaisies]
            };
            let species = match (hash >> 4) % 8 {
                0 => flowers[2],
                1..=2 => flowers[1],
                _ => flowers[0],
            };
            if !h.terrain.in_prop_patch(p, species as u16) {
                continue;
            }
            cells.entry(cell).or_insert(species as u16);
        }
    }
}

fn river_landing(
    h: &mut Habitat<'_>,
    bank: Tile,
    d: Tile,
    cells: &mut BTreeMap<Cell, u16>,
) -> bool {
    let Some((ground, shore)) = h.ground(bank) else {
        return false;
    };
    if shore.biome.weights.plains < 0.55 || shore.slope > 0.8 {
        return false;
    }
    let position = |t, w| [bank[0] + d[0] * t - d[1] * w, bank[1] + d[1] * t + d[0] * w];
    let tip = position(5, 0);
    let tip_col = h.column(tip);
    let Some(water) = tip_col.water_level else {
        return false;
    };
    if tip_col.river_cavity.is_some() || !(1.0..5.0).contains(&(water - tip_col.height)) {
        return false;
    }
    let deck = water.ceil() as i64;
    if !(0..=1).contains(&(deck - ground)) {
        return false;
    }
    let mut plan = Vec::new();
    for t in 0..=5 {
        for w in -1_i64..=1 {
            let [x, z] = position(t, w);
            let c = h.column([x, z]);
            let floor = (c.height - 0.5).floor() as i64;
            if c.cave.open_to_sky()
                || c.river_cavity.is_some()
                || floor > deck
                || (t == 0 && (ground - floor).abs() > 1)
                || deck - floor > 6
                || (deck..=deck + 3)
                    .any(|y| h.tree_occupied([x, y, z]) || cells.contains_key(&[x, y, z]))
            {
                return false;
            }
            plan.push(([x, deck, z], M::OakPlanks));
            if (t == 0 || t == 5) && w.abs() == 1 {
                for y in floor..=deck + 1 {
                    plan.push(([x, y, z], M::Wood));
                }
                if t == 5 {
                    plan.push(([x, deck + 2, z], M::Lantern));
                }
            }
        }
    }
    cells.extend(plan.into_iter().map(|(cell, m)| (cell, m as u16)));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, TerraGenerator};
    use terra_core::{BiomeDefinitions, WorldConfig, WorldSeed};
    use terra_voxel::global_to_section;

    fn landscape(revision: u8) -> TerraGenerator {
        TerraGenerator::new(WorldConfig {
            seed: WorldSeed(42042),
            ecology_revision: revision,
            biome_definitions: Some(
                BiomeDefinitions::load_directory(terra_core::asset_directory().join("biome"))
                    .unwrap(),
            ),
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn rooted_plains_flower_beds_survive_section_generation_and_are_revision_gated() {
        let modern = landscape(13);
        let old = landscape(12);
        let sections = SectionGenerator::new(modern.clone());
        let mut species = std::collections::BTreeSet::new();
        for tile in [[6, 6], [4, 20]] {
            let patch = modern.natural_patch(tile);
            let old_patch = old.natural_patch(tile);
            for &(cell, flower) in patch
                .cells
                .iter()
                .filter(|(_, m)| [M::MeadowSunflowers as u16, M::RoseFoxgloves as u16].contains(m))
            {
                if !species.insert(flower) {
                    continue;
                }
                assert_ne!(old_patch.block_at(cell), Some(flower));
                assert_eq!(
                    sections.block_at([cell[0], cell[1] - 1, cell[2]]).unwrap(),
                    M::Grass as u16
                );
                let (owner, local) = global_to_section(cell);
                assert_eq!(
                    sections.generate(owner).unwrap().block(local).unwrap(),
                    flower
                );
            }
        }
        assert_eq!(species.len(), 2);
    }

    #[test]
    fn legacy_plains_village_keeps_its_mill_and_hilltop_ruin() {
        let t = landscape(13);
        let sections = SectionGenerator::new(t.clone());
        let village = t.structure_site([1, 1]).unwrap();
        assert_eq!(
            village.kind,
            crate::generation::structures::StructureKind::Village
        );
        let hub = [663, 40, 619];
        assert_eq!(sections.block_at(hub).unwrap(), M::Wood as u16);
        for (dx, dy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let sail = [hub[0] + 4 * dx - dy, hub[1] + 4 * dy + dx, hub[2]];
            assert_eq!(sections.block_at(sail).unwrap(), M::Wool as u16);
        }
        let ruin = t.structure_site([1, 2]).unwrap();
        let [x, y, z] = ruin.buildings[0].entrance;
        assert_eq!(sections.block_at([x, y + 1, z + 2]).unwrap(), M::Air as u16);
        assert!(matches!(
            M::from_raw(sections.block_at([x, y + 8, z + 2]).unwrap()),
            Some(M::StructureStone | M::MeadowLimestone | M::MossyStone)
        ));
        assert_eq!(ruin.containers.len(), 1);
    }
    #[test]
    fn current_plains_village_keeps_its_houses_without_fields_or_mill() {
        use crate::generation::structures::{BuildingRole, StructureKind};
        let terrain = landscape(14);
        let village = terrain.structure_site([1, 1]).unwrap();
        let [x, _, z] = village.center;
        assert_eq!(village.kind, StructureKind::Village);
        assert_eq!(
            terrain.biome_at(x as f64, z as f64).weights.dominant(),
            terra_core::BiomeKind::Plains
        );
        assert!(
            village
                .buildings
                .iter()
                .any(|plot| plot.role == BuildingRole::House)
        );
        assert!(
            village
                .buildings
                .iter()
                .all(|plot| plot.role != BuildingRole::Farm)
        );
        let column = [x.div_euclid(16), z.div_euclid(16)];
        let site = terrain
            .structures_for_column(column, 0)
            .into_iter()
            .find(|site| site.summary.id == village.id)
            .unwrap();
        assert!(
            site.sections
                .values()
                .all(|s| s.objects.iter().all(|object| object.crop.is_none()))
        );
        assert!(!terrain.village_residents([1, 1]).is_empty());
        let sections = SectionGenerator::new(terrain.clone());
        assert_ne!(sections.block_at([663, 40, 619]).unwrap(), M::Wood as u16);
        assert_ne!(sections.block_at([666, 45, 619]).unwrap(), M::Wool as u16);
        assert_eq!(
            terrain.structure_site([1, 2]).unwrap().kind,
            StructureKind::Ruin
        );
    }
}
