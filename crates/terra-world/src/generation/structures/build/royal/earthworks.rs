//! Local revision-ten foundations and roads; no settlement-sized terrain pad.
use super::*;
use crate::generation::structures::build::graves::adapted::ground;

impl Royal<'_, '_> {
    pub(super) fn terrain_adapted(&self) -> bool {
        self.builder.terrain.config().structures_revision >= 10
    }
    pub(super) fn local_floor(&mut self, x: i64, z: i64, w: i64, d: i64, legacy: i64) -> i64 {
        if !self.terrain_adapted() {
            return legacy;
        }
        let low = self.point([x, 0, z]);
        let high = self.point([x + w, 0, z + d]);
        match ground::fitted_floor(self.builder, [low[0], low[2]], [high[0], high[2]], 3) {
            Some(y) => y - self.origin[1],
            None => {
                self.builder.failed = true;
                legacy
            }
        }
    }
    pub(super) fn bounded_foundation(&mut self, x: i64, z: i64, w: i64, d: i64, y: i64) {
        for dz in 0..=d {
            for dx in 0..=w {
                let p = self.point([x + dx, y - 1, z + dz]);
                let (owner, local) = global_to_section(p);
                let existing = self
                    .builder
                    .patches
                    .get(&owner)
                    .map(|cells| cells[local[0] + 16 * (local[1] + 16 * local[2])]);
                if existing.is_some_and(|id| id != 0 && id != u16::MAX) {
                    continue;
                }
                let natural = self.natural_floor(x + dx, z + dz);
                let column = self
                    .builder
                    .terrain
                    .surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
                if (natural - y).abs() > 4
                    || !crate::generation::structures::build::foundation_supported(
                        self.builder.terrain,
                        p[0],
                        p[2],
                        y + self.origin[1],
                        column,
                    )
                {
                    self.builder.failed = true;
                    return;
                }
            }
        }
        for dz in 0..=d {
            for dx in 0..=w {
                let p = self.point([x + dx, y - 1, z + dz]);
                let (owner, local) = global_to_section(p);
                let supported = self.builder.patches.get(&owner).is_some_and(|cells| {
                    let id = cells[local[0] + 16 * (local[1] + 16 * local[2])];
                    id != 0 && id != u16::MAX
                });
                if !supported {
                    let natural = self.natural_floor(x + dx, z + dz);
                    self.fill(
                        [x + dx, natural.min(y) - 1, z + dz],
                        [x + dx, y - 1, z + dz],
                        43,
                    );
                }
            }
        }
    }
    pub(super) fn natural_paths(&mut self) {
        let routes = if self.outpost {
            vec![
                ([0, -6], [0, 28]),
                ([-17, 7], [18, 7]),
                ([-14, 20], [18, 20]),
            ]
        } else {
            vec![
                ([0, -25], [0, 70]),
                ([-56, 16], [56, 16]),
                ([-56, 32], [56, 32]),
                ([-56, 52], [56, 52]),
            ]
        };
        for (start, end) in routes {
            let start = self.point([start[0], self.natural_floor(start[0], start[1]), start[1]]);
            let end = self.point([end[0], self.natural_floor(end[0], end[1]), end[1]]);
            if ground::path(self.builder, start, end).is_none() {
                self.builder.failed = true;
            }
        }
    }
    pub(super) fn natural_access(&mut self, door: [i64; 3]) {
        let end = [
            door[0],
            self.natural_floor(door[0], door[2] + 6),
            door[2] + 6,
        ];
        let start = self.point([door[0], door[1], door[2] + 2]);
        let end = self.point(end);
        if ground::path(self.builder, start, end).is_none() {
            self.builder.failed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::WorldConfig;

    fn terrain(hills: f64) -> TerraGenerator {
        TerraGenerator::new(WorldConfig {
            structures_revision: 10,
            landscape_revision: 0,
            geology_revision: 0,
            ocean_revision: 0,
            base_height: 32.0,
            hill_amplitude: hills,
            mountain_amplitude: 0.0,
            terrace_strength: 0.0,
            biomes: false,
            water: false,
            rivers: false,
            caves: false,
            trees: false,
            sea_level: 0.0,
            ..Default::default()
        })
        .unwrap()
    }
    #[test]
    fn revised_kingdom_and_outpost_leave_the_ground_between_buildings_untouched() {
        for seed in [522, 261] {
            let terrain = terrain(0.0);
            let site =
                plan(&terrain, [0, 0], seed).expect("flat land must support a complete settlement");
            let radius = if seed == 261 { 26 } else { 64 };
            let mut original = 0;
            let mut total = 0;
            for z in -radius..=radius {
                for x in -radius..=radius {
                    total += 1;
                    if site.block_at([192 + x, 30, 192 + z]).is_none() {
                        original += 1;
                    }
                }
            }
            assert!(
                original * 100 / total > 85,
                "settlement-wide pad: {original}/{total} natural subsoil cells"
            );
            assert!(site.settlement.is_some());
            assert!(
                site.summary
                    .buildings
                    .iter()
                    .any(|building| building.role == BuildingRole::House
                        || building.role == BuildingRole::Barracks)
            );
            assert!(
                site.summary
                    .containers
                    .iter()
                    .any(|p| site.block_at(*p) == Some(29))
            );
        }
    }
    #[test]
    fn hillside_roadworks_never_construct_a_filled_royal_circle() {
        let terrain = terrain(8.0);
        let mut b = Builder {
            terrain: &terrain,
            summary: SiteSummary {
                id: 1,
                kind: StructureKind::Kingdom,
                style: VillageStyle::Temperate,
                center: [192, 32, 192],
                bounds: [[0; 3]; 2],
                buildings: Vec::new(),
                containers: Vec::new(),
                grave: None,
            },
            patches: BTreeMap::new(),
            objects: BTreeMap::new(),
            writes: 0,
            failed: false,
            seed: 522,
        };
        let mut royal = Royal {
            builder: &mut b,
            origin: [192, 32, 192],
            winter: false,
            outpost: false,
        };
        let sentinel = royal.point([25, royal.floor(25, 30) - 1, 30]);
        royal.terrain();
        let site = b.finish().unwrap();
        assert_eq!(site.block_at(sentinel), None);
        for (owner, patch) in &site.sections {
            let origin = owner.origin().unwrap();
            for &(index, material) in &patch.cells {
                if material == 0 {
                    continue;
                }
                let index = i64::from(index);
                let p = [
                    origin[0] + index % 16,
                    origin[1] + index / 16 % 16,
                    origin[2] + index / 256,
                ];
                let ground = (terrain.surface_height(p[0] as f64 + 0.5, p[2] as f64 + 0.5) - 0.5)
                    .floor() as i64
                    + 1;
                assert!(
                    p[1] < ground + 2,
                    "excessive road fill at {p:?}, natural={ground}"
                );
            }
        }
    }
}
