//! Cliff-side exploration sites. The open approach faces downhill and the mine
//! and treasury penetrate into the uphill shoulder. All blocks belong to one
//! immutable surface patch, including the inventories and oriented fittings.
use super::{Cell, Habitat, TILE_SIZE, TerraGenerator, Tile, ticket};
use crate::{edits::BlockObjectState, generation::structures::StructureObject};
use std::collections::BTreeMap;
use terra_core::{Inventory, ItemKind};
use terra_voxel::MaterialId as M;

mod mine;
mod tower;
mod vault;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrystalLandmarkKind {
    CrystalMine,
    MageTower,
    CrystalVault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalLandmarkSite {
    pub kind: CrystalLandmarkKind,
    pub center: [i64; 3],
    pub turn: u8,
}

impl CrystalLandmarkSite {
    pub fn world(self, x: i64, y: i64, z: i64) -> [i64; 3] {
        let (dx, dz) = match self.turn {
            1 => (-z, x),
            2 => (-x, -z),
            3 => (z, -x),
            _ => (x, z),
        };
        [self.center[0] + dx, self.center[1] + y, self.center[2] + dz]
    }
}

pub(super) fn candidate(terrain: &TerraGenerator, tile: Tile) -> Option<CrystalLandmarkKind> {
    if tile.iter().any(|v| v.unsigned_abs() > 31_250_000) {
        return None;
    }
    let config = terrain.config();
    if config.ecology_revision < 21 || !config.biomes || config.structures_revision == 0 {
        return None;
    }
    let region = tile.map(|v| v.div_euclid(3));
    let key = ticket(config.seed.0, region, 8581);
    if tile.map(|v| v.rem_euclid(3)) != [i64::from(key % 3), i64::from((key >> 10) % 3)] {
        return None;
    }
    let center = tile.map(|v| (v * TILE_SIZE + 16) as f64 + 0.5);
    if terrain
        .biome_at(center[0], center[1])
        .weights
        .crystal_highlands
        < 0.52
    {
        return None;
    }
    // Cycle through the three sites across each alpine province, keeping its
    // first few hundred metres from containing only a single type of building.
    match (region[0] + 2 * region[1] + (config.seed.0 % 3) as i64).rem_euclid(3) {
        0 if config.structures.mines => Some(CrystalLandmarkKind::CrystalMine),
        1 if config.structures.ruins => Some(CrystalLandmarkKind::MageTower),
        2 if config.structures.ruins => Some(CrystalLandmarkKind::CrystalVault),
        _ => None,
    }
}

impl TerraGenerator {
    // Called by the tree planner before surface patches are assembled. Reserving
    // the selected site grid here avoids a generator recursion through natural_patch.
    pub(in crate::generation) fn crystal_building_clearing(&self, x: i64, z: i64) -> bool {
        [[x, z], [x - 4, z], [x + 4, z], [x, z - 4], [x, z + 4]]
            .into_iter()
            .any(|p| candidate(self, p.map(|c| c.div_euclid(TILE_SIZE))).is_some())
    }

    /// Coordinates are 32-metre patch indices, as in surface_flora.
    pub fn crystal_landmark_at(&self, tile: Tile) -> Option<CrystalLandmarkSite> {
        candidate(self, tile)?;
        self.natural_patch(tile).crystal_landmark
    }
}

pub(super) fn build(
    h: &mut Habitat<'_>,
    tile: Tile,
    blocks: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<StructureObject>,
) -> Option<CrystalLandmarkSite> {
    let kind = candidate(h.terrain, tile)?;
    let [cx, cz] = tile.map(|v| v * TILE_SIZE + 16);
    if h.reserved([cx, cz], 15) {
        return None;
    }
    let up = [(0, 1), (-1, 0), (0, -1), (1, 0)];
    let mut best = (f64::NEG_INFINITY, 0);
    for (turn, (dx, dz)) in up.into_iter().enumerate() {
        let rise = h.column([cx + dx * 10, cz + dz * 10]).height
            - h.column([cx - dx * 10, cz - dz * 10]).height;
        if rise > best.0 {
            best = (rise, turn as u8);
        }
    }
    let mut site = CrystalLandmarkSite {
        kind,
        turn: best.1,
        center: [cx, 0, cz],
    };
    let mut ground = [[0i64; 31]; 31];
    for z in -15..=15 {
        for x in -15..=15 {
            let [wx, _, wz] = site.world(x, 0, z);
            let col = h.column([wx, wz]);
            if col.lava_level.is_some() || col.water_level.is_some() {
                return None;
            }
            ground[(z + 15) as usize][(x + 15) as usize] = (col.height - 0.5).floor() as i64;
        }
    }
    let elevation = |x: i64, z: i64| ground[(z + 15) as usize][(x + 15) as usize];
    let entry = elevation(0, -13);
    let base = match kind {
        CrystalLandmarkKind::CrystalMine | CrystalLandmarkKind::CrystalVault => {
            let vault = kind == CrystalLandmarkKind::CrystalVault;
            let half_width = if vault { 6 } else { 4 };
            let roof = if vault { 11 } else { 7 };
            // The whole underground volume must fit under the real hillside.
            // A low corner lowers the floor; an isolated boulder cannot qualify
            // a flat clearing as a cliff. Never manufacture an artificial hill.
            let cover = (2..=14)
                .flat_map(|z| (-half_width..=half_width).map(move |x| elevation(x, z)))
                .min()?;
            if best.0 < if vault { 5.5 } else { 3.5 } {
                return None;
            }
            let floor = (cover - roof - 2).min(elevation(0, -2)).min(entry + 2);
            if floor < entry - if vault { 8 } else { 3 } {
                return None;
            }
            floor
        }
        CrystalLandmarkKind::MageTower => {
            // Survey the whole drum and its projecting study: a centre-line
            // slope can miss a tall cliff directly underneath a corner.
            let mut low = i64::MAX;
            let mut high = i64::MIN;
            for z in -3_i64..=10 {
                for x in -5_i64..=11 {
                    if x > 5 && !(0..=8).contains(&z) || x < 4 && x.abs() + (z - 3).abs() > 9 {
                        continue;
                    }
                    let ground = elevation(x, z);
                    low = low.min(ground);
                    high = high.max(ground);
                }
            }
            // Two courses of plinth meet native ground. The rear can sit
            // inside the shoulder up to the height of the lower windows.
            let lower = high - 10;
            let upper = (low + 2).min(entry + 5);
            if lower > upper || upper < entry - 7 {
                return None;
            }
            (elevation(0, -3) + 1).clamp(lower, upper)
        }
    };
    site.center[1] = base;
    // Whole approach, roof silhouette and both shoulders belong to this
    // site. Another formation can never leave an unsupported tip over it.
    blocks.retain(|&[x, _, z], _| (x - cx).abs() > 15 || (z - cz).abs() > 15);
    objects.retain(|o| (o.position[0] - cx).abs() > 15 || (o.position[2] - cz).abs() > 15);
    let mut frame = Frame {
        site,
        ground,
        blocks,
        objects,
    };
    match kind {
        CrystalLandmarkKind::CrystalMine => mine::build(&mut frame),
        CrystalLandmarkKind::MageTower => tower::build(&mut frame),
        CrystalLandmarkKind::CrystalVault => vault::build(&mut frame),
    }
    Some(site)
}

struct Frame<'a> {
    site: CrystalLandmarkSite,
    ground: [[i64; 31]; 31],
    blocks: &'a mut BTreeMap<Cell, u16>,
    objects: &'a mut Vec<StructureObject>,
}

impl Frame<'_> {
    fn floor(&self, x: i64, z: i64) -> i64 {
        self.ground[(z + 15) as usize][(x + 15) as usize] - self.site.center[1]
    }
    fn put(&mut self, x: i64, y: i64, z: i64, mat: M) {
        self.blocks.insert(self.site.world(x, y, z), mat as u16);
    }
    fn column(&mut self, x: i64, z: i64, bottom: i64, top: i64, mat: M) {
        for y in bottom..=top {
            self.put(x, y, z, mat);
        }
    }
    fn oriented(&mut self, x: i64, y: i64, z: i64, mat: M, facing: u8) {
        self.put(x, y, z, mat);
        self.objects.push(StructureObject {
            position: self.site.world(x, y, z),
            material: mat as u16,
            state: Some(BlockObjectState {
                facing: (facing + 4 - self.site.turn) % 4,
                ..Default::default()
            }),
            loot: None,
            crop: None,
        });
    }
    fn chest(&mut self, x: i64, y: i64, z: i64, item: M, count: u16) {
        self.put(x, y, z, M::Chest);
        let mut loot = Inventory::default();
        loot.insert(ItemKind::Block(item as u16), count);
        loot.insert(ItemKind::Block(M::CrystalShardCluster as u16), 2);
        self.objects.push(StructureObject {
            position: self.site.world(x, y, z),
            material: M::Chest as u16,
            state: Some(BlockObjectState {
                facing: (6 - self.site.turn) % 4,
                ..Default::default()
            }),
            loot: Some(loot),
            crop: None,
        });
    }
    /// A terrace has foundations to native rock and actual air above each tread.
    fn terrace(&mut self, x: i64, z: i64, level: i64, mat: M) {
        let ground = self.floor(x, z);
        self.column(
            x,
            z,
            ground.min(level) - 1,
            level - 1,
            M::HighlandCliffStone,
        );
        self.put(x, level, z, mat);
        self.column(x, z, level + 1, ground.max(level) + 3, M::Air);
    }
    fn approach(&mut self, half_width: i64, landing: i64, end_z: i64) {
        let front = -14;
        let start = self
            .floor(0, front)
            .clamp(landing - (end_z - front), landing + (end_z - front));
        for z in front..=end_z {
            let level = start + (landing - start) * (z - front) / (end_z - front);
            for x in -half_width..=half_width {
                let edging = x.abs() == half_width;
                self.terrace(
                    x,
                    z,
                    level,
                    if edging {
                        M::CrystalAshlar
                    } else {
                        M::PolishedHighlandRock
                    },
                );
                if !edging {
                    let next = start + (landing - start) * (z + 1 - front) / (end_z - front);
                    let prev = start + (landing - start) * (z - 1 - front) / (end_z - front);
                    if z < end_z && next > level {
                        self.oriented(x, level + 1, z, M::StoneStairs, 0);
                    } else if z > front && prev > level {
                        self.oriented(x, level + 1, z, M::StoneStairs, 2);
                    }
                }
            }
            if (z - front) % 5 == 1 {
                for x in [-half_width, half_width] {
                    self.put(x, level + 1, z, M::CrystalAshlar);
                    self.put(x, level + 2, z, M::CrystalLantern);
                }
            }
        }
    }
    /// The cut faces meet across voxel boundaries, so an intact prism reads as
    /// one crystal while each of its sections can still be mined individually.
    fn prism(&mut self, x: i64, y: i64, z: i64, height: i64) {
        self.put(x, y, z, M::AmethystPrismFoot);
        self.column(x, z, y + 1, y + height - 2, M::AmethystPrismShaft);
        self.put(x, y + height - 1, z, M::AmethystPrismTip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SectionGenerator;
    use std::collections::BTreeSet;
    use terra_core::WorldConfig;
    use terra_voxel::global_to_section;

    #[test]
    fn natural_sites_have_open_entrances_persistent_loot_and_complete_mine_tracks() {
        let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        for (tile, kind) in [
            ([-36, 1], CrystalLandmarkKind::CrystalMine),
            ([-45, 8], CrystalLandmarkKind::MageTower),
            ([-39, 0], CrystalLandmarkKind::CrystalVault),
        ] {
            let site = terrain.crystal_landmark_at(tile).unwrap();
            assert_eq!(site.kind, kind);
            let patch = terrain.natural_patch(tile);
            let mut points = BTreeSet::new();
            let mut chests = 0;
            for obj in &patch.objects {
                assert!(
                    points.insert(obj.position),
                    "duplicate fixture at {:?}",
                    obj.position
                );
                assert_eq!(patch.block_at(obj.position), Some(obj.material), "{obj:?}");
                if let Some(state) = obj.state {
                    assert!(state.valid_for(obj.position, obj.material), "{obj:?}");
                }
                if obj.material == M::Chest as u16 {
                    assert!(obj.loot.is_some());
                    chests += 1;
                }
            }
            assert!(chests > 0);
            let at = |x, y, z| generator.block_at(site.world(x, y, z)).unwrap();
            if kind != CrystalLandmarkKind::MageTower {
                let (width, cover) = if kind == CrystalLandmarkKind::CrystalVault {
                    (6, 12)
                } else {
                    (4, 8)
                };
                for x in -width..=width {
                    for z in 2..=14 {
                        let top = site.world(x, cover, z);
                        assert!(
                            terrain.surface_height(top[0] as f64 + 0.5, top[2] as f64 + 0.5)
                                > top[1] as f64,
                            "exposed roof at {top:?}"
                        );
                    }
                }
            }
            assert!(
                patch
                    .cells
                    .iter()
                    .all(|(_, id)| !terra_core::quartz_prisms::is_quartz(*id))
            );

            let mark = match kind {
                CrystalLandmarkKind::CrystalMine => {
                    for z in -9..=14 {
                        assert!(terra_core::rails::is_rail(at(0, 1, z)), "rail gap at {z}");
                        let overhead = if z == 14 { M::RailBuffer } else { M::Air };
                        assert_eq!(at(0, 2, z), overhead as u16);
                        assert_eq!(at(0, 3, z), 0);
                    }
                    for entry in patch
                        .objects
                        .iter()
                        .filter(|o| terra_core::rails::is_rail(o.material))
                    {
                        let piece = crate::rails::piece_from_state(
                            entry.position,
                            entry.material,
                            entry.state.unwrap(),
                        )
                        .unwrap();
                        for p in crate::rails::cells(piece) {
                            assert!(
                                patch.objects.iter().any(|o| o.position == p
                                    && o.state == Some(crate::rails::object_state(piece, p))),
                                "incomplete rail {p:?}"
                            );
                        }
                    }
                    site.world(7, 1, -3)
                }
                CrystalLandmarkKind::MageTower => {
                    for z in -3..=2 {
                        for y in [3, 4] {
                            assert_eq!(at(0, y, z), 0, "closed tower door at {z}");
                        }
                    }
                    assert_eq!(at(-2, 3, 1), M::StoneStairs as u16);
                    assert_eq!(at(2, 18, 0), M::StoneStairs as u16);
                    assert_eq!(at(2, 19, 0), 0);
                    for x in 3..=5 {
                        assert_eq!(at(x, 3, 3), 0, "passage to the study");
                    }
                    for (tread, next) in [
                        ([-2, 3, 1], [-2, 4, 2]),
                        ([-1, 9, 6], [0, 10, 6]),
                        ([2, 13, 5], [2, 14, 4]),
                    ] {
                        let [x, y, z] = tread;
                        let start = site.world(x, y, z);
                        let [x, y, z] = next;
                        let end = site.world(x, y, z);
                        let step = patch.objects.iter().find(|o| o.position == start).unwrap();
                        let forward =
                            terra_core::construction::stair_basis(step.state.unwrap().facing)[2];
                        assert_eq!(i64::from(forward[0]), end[0] - start[0]);
                        assert_eq!(i64::from(forward[2]), end[2] - start[2]);
                    }
                    site.world(0, 19, 3)
                }
                CrystalLandmarkKind::CrystalVault => {
                    for z in -3..=8 {
                        for y in [3, 4] {
                            assert_eq!(at(0, y, z), 0, "closed aisle at {z}");
                        }
                    }
                    assert_eq!(at(0, 5, 11), M::AltarCrystalFoot as u16);
                    assert_eq!(at(0, 6, 11), M::AltarCrystalHeart as u16);
                    assert_eq!(at(0, 7, 11), M::AltarCrystalCrown as u16);
                    assert_eq!(at(-2, 6, 11), M::TempleCrystalCluster as u16);
                    assert_eq!(at(2, 7, 11), M::AmethystPrismTip as u16);
                    site.world(0, 11, -1)
                }
            };
            let (owner, local) = global_to_section(mark);
            assert_eq!(
                generator.generate(owner).unwrap().block(local).unwrap(),
                generator.block_at(mark).unwrap()
            );
        }
    }

    #[test]
    fn saved_ecology_and_disabled_structures_gate_crystal_landmarks() {
        for (old, mines, ruins) in [
            (true, true, true),
            (false, false, true),
            (false, true, false),
        ] {
            let mut config = WorldConfig::default();
            if old {
                config.ecology_revision = 20;
            }
            config.structures.mines = mines;
            config.structures.ruins = ruins;
            let terrain = TerraGenerator::new(config).unwrap();
            if old || !mines {
                assert!(terrain.crystal_landmark_at([-36, 1]).is_none());
            }
            if old || !ruins {
                assert!(terrain.crystal_landmark_at([-39, 0]).is_none());
            }
        }
    }
}
