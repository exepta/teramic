//! Tile-bounded coastal landmarks. Positive local Z always points inland.
//! Landmarks and their tidal water volumes use the same deterministic plan.
use super::{Cell, Habitat, TILE_SIZE, TerraGenerator, Tile, ticket};
use crate::generation::{biomes::BiomeKind, structures::StructureObject};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::Mutex,
};
use terra_voxel::MaterialId as M;

mod breakers;
mod buildings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoastalLandmarkKind {
    Shipwreck,
    FishingVillage,
    Lighthouse,
    TidalCave,
}
impl CoastalLandmarkKind {
    pub fn enabled(self, config: &terra_core::WorldConfig) -> bool {
        match self {
            Self::FishingVillage => config.structures.villages,
            Self::TidalCave => config.caves,
            _ => config.structures.ruins,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoastalLandmarkSite {
    pub kind: CoastalLandmarkKind,
    pub center: Cell,
    pub turn: u8,
}
impl CoastalLandmarkSite {
    pub fn world(self, x: i64, y: i64, z: i64) -> Cell {
        let (dx, dz) = match self.turn {
            1 => (-z, x),
            2 => (-x, -z),
            3 => (z, -x),
            _ => (x, z),
        };
        [self.center[0] + dx, self.center[1] + y, self.center[2] + dz]
    }

    fn local(self, x: f64, z: f64) -> [f64; 2] {
        let (x, z) = (
            x - self.center[0] as f64 - 0.5,
            z - self.center[2] as f64 - 0.5,
        );
        match self.turn {
            1 => [z, -x],
            2 => [-x, -z],
            3 => [-z, x],
            _ => [x, z],
        }
    }
    pub fn entrance(self) -> Cell {
        match self.kind {
            CoastalLandmarkKind::Shipwreck => self.world(0, 2, 2),
            CoastalLandmarkKind::FishingVillage => self.world(0, 2, 1),
            CoastalLandmarkKind::Lighthouse => self.world(0, 2, -5),
            CoastalLandmarkKind::TidalCave => self.world(3, 2, -3),
        }
    }
}

type Plans = (HashMap<Tile, Option<CoastalLandmarkSite>>, VecDeque<Tile>);
#[derive(Default)]
pub(in crate::generation) struct CoastPlans(Mutex<Plans>);

impl TerraGenerator {
    fn coastal_plan(&self, tile: Tile) -> Option<CoastalLandmarkSite> {
        let c = self.config();
        if c.ecology_revision < 22
            || !self.marine_overhaul()
            || c.structures_revision == 0
            || tile.iter().any(|v| v.unsigned_abs() > 31_250_000)
        {
            return None;
        }
        let sector = tile.map(|v| v.div_euclid(2));
        let draw = ticket(c.seed.0, sector, 0x5a17);
        if tile.map(|v| v.rem_euclid(2)) != [i64::from(draw & 1), i64::from((draw >> 1) & 1)] {
            return None;
        }
        if let Some(plan) = self.coastal_plans.0.lock().unwrap().0.get(&tile).copied() {
            return plan;
        }
        let plan = self.plan_shore(tile, draw).filter(|site| {
            if site.kind != CoastalLandmarkKind::Shipwreck {
                return true;
            }
            // Wrecks can fit almost every successive shelf. Choose the highest
            // ranked wreck within 96 m, while preserving working harbours beside lights.
            let rank = |p: Tile| (ticket(c.seed.0, p, 0x5a17_726b), p);
            for dz in -2..=2 {
                for dx in -2..=2 {
                    let other_sector = [sector[0] + dx, sector[1] + dz];
                    let hash = ticket(c.seed.0, other_sector, 0x5a17);
                    let peer = [
                        other_sector[0] * 2 + i64::from(hash & 1),
                        other_sector[1] * 2 + i64::from((hash >> 1) & 1),
                    ];
                    let delta = [tile[0] - peer[0], tile[1] - peer[1]];
                    if (dx == 0 && dz == 0) || delta[0] * delta[0] + delta[1] * delta[1] >= 9 {
                        continue;
                    }
                    if self.plan_shore(peer, hash).is_some_and(|p| {
                        p.kind == CoastalLandmarkKind::FishingVillage
                            || (p.kind == CoastalLandmarkKind::Shipwreck && rank(peer) > rank(tile))
                    }) {
                        return false;
                    }
                }
            }
            true
        });
        let mut cache = self.coastal_plans.0.lock().unwrap();
        if let Some(old) = cache.0.get(&tile) {
            return *old;
        }
        if cache.1.len() >= 2048
            && let Some(old) = cache.1.pop_front()
        {
            cache.0.remove(&old);
        }
        cache.1.push_back(tile);
        cache.0.insert(tile, plan);
        plan
    }

    fn plan_shore(&self, tile: Tile, draw: u32) -> Option<CoastalLandmarkSite> {
        let [x, z] = tile.map(|v| v * TILE_SIZE + 16);
        let world = |dx: i64, dz: i64| [(x + dx) as f64 + 0.5, (z + dz) as f64 + 0.5];
        let sea = self.config().sea_level;
        let [cx, cz] = world(0, 0);
        let biome = self.climate_biome_at(cx, cz);
        if biome.weights.coastal_land_fraction() < 0.52 {
            return None;
        }
        let elevation = |dx, dz| {
            let [px, pz] = world(dx, dz);
            self.surface_height(px, pz)
        };
        let ground = elevation(0, 0);
        if !(sea - 8.0..=sea + 34.).contains(&ground) {
            return None;
        }
        let directions = [(0, 1), (-1, 0), (0, -1), (1, 0)];
        // A real submerged seaward sample is mandatory even for high ground.
        let mut orientation = None;
        for (turn, (dx, dz)) in directions.into_iter().enumerate() {
            for distance in [13, 26, 48, 80] {
                let [px, pz] = world(-dx * distance, -dz * distance);
                if self.climate_biome_at(px, pz).weights.ocean <= 0.45
                    || self.surface_height(px, pz) > sea - 1.
                {
                    continue;
                }
                let rise = elevation(dx * 10, dz * 10) - elevation(-dx * 13, -dz * 13);
                let score = rise - (distance as f64) * 0.18;
                if orientation.is_none_or(|(v, _, _)| score > v) {
                    orientation = Some((score, turn, distance));
                }
                break;
            }
        }
        let (_, turn, distance) = orientation?;
        let (dx, dz) = directions[turn];
        let back = elevation(dx * 11, dz * 11);
        let front = elevation(-dx * 14, -dz * 14);
        let harbor = ground > sea - 2.5
            && ground < sea + 9.
            && back > sea + 0.75
            && front < sea + 0.5
            && distance <= 26;
        let location = if !(harbor && self.config().structures.villages && !draw.is_multiple_of(3))
            && ground < sea + 0.25
            && front < sea
            && back < sea + 6.
            && ground > sea - 7.
        {
            // Stranded ribs belong to the inner shelf, within reach of dry land.
            if ![22, 45, 72]
                .into_iter()
                .any(|n| elevation(dx * n, dz * n) > sea + 1.)
            {
                return None;
            }
            (
                CoastalLandmarkKind::Shipwreck,
                (ground - 0.5).floor() as i64,
            )
        } else if front < sea
            && distance <= 26
            && back > sea + 6.0
            && self.config().caves
            && (!draw.is_multiple_of(3) || ground > sea + 8.)
        {
            (CoastalLandmarkKind::TidalCave, sea.floor() as i64)
        } else if harbor {
            (CoastalLandmarkKind::FishingVillage, sea.ceil() as i64)
        } else if ground >= sea + 7.
            && (back - ground).abs() < 14.
            && distance <= 80
            && draw.is_multiple_of(3)
        {
            (
                CoastalLandmarkKind::Lighthouse,
                (ground - 0.5).floor() as i64,
            )
        } else {
            return None;
        };
        location
            .0
            .enabled(self.config())
            .then_some(CoastalLandmarkSite {
                kind: location.0,
                center: [x, location.1, z],
                turn: turn as u8,
            })
    }

    /// The same carved interval controls collision, natural water meshing and swimming.
    pub(in crate::generation) fn tidal_cavity(
        &self,
        x: f64,
        z: f64,
        height: f64,
    ) -> Option<[f64; 2]> {
        if !self.config().caves || self.config().ecology_revision < 22 {
            return None;
        }
        let site = self.coastal_plan([x, z].map(|n| (n / TILE_SIZE as f64).floor() as i64))?;
        if site.kind != CoastalLandmarkKind::TidalCave {
            return None;
        }
        let [a, b] = site.local(x, z);
        if !(-15. ..=13.).contains(&b) {
            return None;
        }
        let taper = (1. - ((b - 3.).max(0.) / 11.).powi(2)).max(0.);
        let curve = ((b + 4.) * 0.15).sin() * 1.4;
        let arch = 1. - ((a - curve) / (4.7 + 0.8 * (b * 0.65).cos())).powi(2);
        if arch * taper < 0.10 {
            return None;
        }
        let roof = site.center[1] as f64 + 1.4 + 5.0 * (arch * taper).sqrt();
        let bed = site.center[1] as f64 - 0.3 - 2.5 * (arch * taper);
        // At the mouth the arch opens through the hillside into the sea.
        // Behind it a native stone roof must survive.
        if b > 6. && roof > height - 1.5 {
            return None;
        }
        Some([bed, roof])
    }

    pub(in crate::generation) fn coastal_site_footprint(&self, x: i64, z: i64) -> bool {
        self.coastal_plan([x.div_euclid(TILE_SIZE), z.div_euclid(TILE_SIZE)])
            .is_some_and(|site| {
                matches!(
                    site.kind,
                    CoastalLandmarkKind::Lighthouse | CoastalLandmarkKind::FishingVillage
                )
            })
    }

    pub fn coastal_landmark_at(&self, tile: Tile) -> Option<CoastalLandmarkSite> {
        self.coastal_plan(tile)?;
        self.natural_patch(tile).coastal_landmark
    }
}

pub(super) fn build(
    h: &mut Habitat<'_>,
    tile: Tile,
    cells: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<StructureObject>,
) -> Option<CoastalLandmarkSite> {
    let plan = h.terrain.coastal_plan(tile);
    let landmark = if let Some(site) = plan
        && !h.reserved([site.center[0], site.center[2]], 15)
        && buildings::build(h, site, cells, objects)
    {
        Some(site)
    } else {
        None
    };
    if landmark.is_none() && breakers::build(h, tile, cells) {
        return None;
    }
    let [ox, oz] = tile.map(|n| n * TILE_SIZE);
    let middle = h.column([ox + 16, oz + 16]);
    if middle.biome.weights.coastal_land_fraction() < 0.40 {
        return landmark;
    }
    // Drift lines, storm wrack and salt meadow each occupy a different tidal band.
    for z in 0..4 {
        for x in 0..4 {
            let k = ticket(
                h.terrain.config().seed.0,
                [tile[0] * 4 + x, tile[1] * 4 + z],
                94897,
            );
            let p = [
                ox + 8 * x + 1 + i64::from(k % 6),
                oz + 8 * z + 1 + i64::from((k >> 5) % 6),
            ];
            let c = h.column(p);
            if c.biome.weights.coastal_land_fraction() < 0.45
                || h.reserved(p, 1)
                || c.river_cavity.is_some()
            {
                continue;
            }
            let y = c.water_bed() as i64;
            let sea = h.terrain.config().sea_level;
            let delta = c.height - sea;
            let m = if let Some(level) = c.water_level.filter(|&level| level > y as f64) {
                // Kelp and oysters occupy a full voxel. Retain clear water above
                // the crown even along a shelf that is only one cell deep.
                if c.biome_kind(sea) != BiomeKind::Ocean
                    || (y + 1) as f64 >= level
                    || delta < -6.
                    || c.seagrass_height > 0
                {
                    continue;
                }
                [M::Bladderwrack, M::RibbonKelp, M::ReefOyster, M::GreenAlgae][(k % 4) as usize]
            } else if delta < 3.2 {
                [
                    M::MarineDriftwood,
                    M::SpiralShell,
                    M::ScallopShell,
                    M::BarnacledBoulder,
                    M::ClamShell,
                ][(k % 5) as usize]
            } else if c.slope < 1.3 {
                if k % 5 < 2 {
                    M::SeaThrift
                } else {
                    M::DuneGrassTuft
                }
            } else {
                continue;
            };
            let pos = [p[0], y, p[1]];
            if !h.terrain.in_prop_patch(p, m as u16)
                || h.tree_occupied(pos)
                || cells.contains_key(&pos)
                || cells.contains_key(&[p[0], y - 1, p[1]])
            {
                continue;
            }
            cells.insert(pos, m as u16);
            if matches!(m, M::DuneGrassTuft | M::SeaThrift) {
                // Low dunes carry connected sweeps of marram and thrift.
                // Higher ledges and sheltered woodland support fern and heather.
                let neighbors = [(-2, 0), (1, 2), (3, 1), (-1, -2), (0, 3)];
                for n in 0..(2 + (k >> 14) % 4) {
                    let (dx, dz) = neighbors[(n as usize + (k % 5) as usize) % 5];
                    let q = [p[0] + dx, p[1] + dz];
                    if q[0].div_euclid(TILE_SIZE) != tile[0]
                        || q[1].div_euclid(TILE_SIZE) != tile[1]
                    {
                        continue;
                    }
                    let col = h.column(q);
                    let top = col.water_bed() as i64;
                    let point = [q[0], top, q[1]];
                    if col.height < sea + 2.2
                        || col.slope > 1.2
                        || col.river_cavity.is_some()
                        || col.biome.weights.coastal_land_fraction() < 0.45
                        || (top - y).abs() > 3
                        || cells.contains_key(&point)
                        || cells.contains_key(&[q[0], top - 1, q[1]])
                        || h.reserved(q, 1)
                        || h.tree_occupied(point)
                    {
                        continue;
                    }
                    let plant = match n {
                        0 if delta > 5. && col.slope < 0.8 => M::AlpineHeather,
                        1 if delta > 7. && h.root_distance_squared(q) < 12 * 12 => M::ForestFern,
                        2 if delta < 5. => M::SeaThrift,
                        _ => M::DuneGrassTuft,
                    };
                    cells.insert(point, plant as u16);
                }
            }
        }
    }
    landmark
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SectionGenerator;
    use terra_core::{Config, WorldConfig};
    pub(super) fn config() -> WorldConfig {
        WorldConfig {
            biome_definitions: Some(terra_core::BiomeDefinitions::builtin().clone()),
            ..WorldConfig::from_toml(include_str!("../../../../../configs/world.toml")).unwrap()
        }
    }

    #[test]
    fn tide_reaches_the_cave_and_the_four_sites_have_grounded_fittings() {
        let terrain = TerraGenerator::new(config()).unwrap();
        let sections = SectionGenerator::new(terrain.clone());
        for (tile, kind) in [
            ([-1, 0], CoastalLandmarkKind::Shipwreck),
            ([-30, -39], CoastalLandmarkKind::FishingVillage),
            ([7, -4], CoastalLandmarkKind::Lighthouse),
            ([-26, -19], CoastalLandmarkKind::TidalCave),
        ] {
            let s = terrain.coastal_landmark_at(tile).unwrap();
            assert_eq!(s.kind, kind);
            crate::edits::EditJournal::default()
                .initialize_structures(&terrain.natural_patch(tile).objects)
                .expect("every oriented fitting and chest must stream into the live world");
            let patch = terrain.natural_patch(tile);
            assert!(
                patch
                    .cells
                    .iter()
                    .all(|(p, _)| p[0].div_euclid(TILE_SIZE) == tile[0]
                        && p[2].div_euclid(TILE_SIZE) == tile[1])
            );
            let verify =
                |x, y, z, m: M| assert_eq!(sections.block_at(s.world(x, y, z)).unwrap(), m as u16);
            match kind {
                CoastalLandmarkKind::Shipwreck => {
                    verify(2, 12, -1, M::HarborTimber);
                    verify(-1, 3, 6, M::Chest);
                    let sea = terrain.config().sea_level.floor() as i64;
                    for x in [-3, 3] {
                        let above_water = (sea + 1..=sea + 5).any(|y| {
                            sections.block_at(s.world(x, y - s.center[1], -6)).unwrap()
                                == M::HarborTimber as u16
                        });
                        assert!(above_water, "both bow frames must break the surface");
                    }
                }
                CoastalLandmarkKind::FishingVillage => {
                    let jetty = (-9..=9)
                        .find(|&x| {
                            patch
                                .cells
                                .contains(&(s.world(x + 1, 3, -14), M::Lantern as u16))
                        })
                        .expect("lantern marks the seaward end of the jetty");
                    verify(jetty, 1, -13, M::SprucePlanks);
                    verify(jetty + 1, 0, -14, M::Wood);
                    let [x, y, z] = s.world(jetty, 0, -13);
                    assert!(
                        terrain
                            .surface_column(x as f64 + 0.5, z as f64 + 0.5)
                            .water_columns()
                            .at(y - 1)
                            .is_some()
                    );
                }
                CoastalLandmarkKind::Lighthouse => {
                    verify(0, 19, 0, M::BeaconLens);
                    verify(0, 1, -4, M::Air);
                }
                CoastalLandmarkKind::TidalCave => {
                    let [x, y, z] = s.world(0, 0, 4);
                    let c = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
                    assert!(
                        c.river_cavity
                            .is_some_and(|[bottom, top]| bottom < (y - 1) as f64
                                && top >= (y + 3) as f64
                                && top < c.height)
                    );
                    assert!(c.water_columns().at(y - 1).is_some());
                    verify(0, 2, 4, M::Air);
                }
            }
        }
    }

    #[test]
    fn neighbouring_shallow_cells_cannot_repeat_a_wreck_along_the_same_beach() {
        let generator = TerraGenerator::new(config()).unwrap();
        // This shelf formerly put paired wrecks only 32 m apart on three stretches.
        let mut wrecks = Vec::new();
        for tx in 21..=32 {
            for tz in -8..=14 {
                if let Some(s) = generator.coastal_landmark_at([tx, tz])
                    && s.kind == CoastalLandmarkKind::Shipwreck
                {
                    wrecks.push(s.center);
                }
            }
        }
        assert!(wrecks.len() >= 2);
        for (i, a) in wrecks.iter().enumerate() {
            for b in &wrecks[i + 1..] {
                assert!((a[0] - b[0]).pow(2) + (a[2] - b[2]).pow(2) >= 96 * 96);
            }
        }
    }

    #[test]
    fn no_district_without_an_open_marine_shelf_or_in_historical_snapshots() {
        let full = TerraGenerator::new(config()).unwrap();
        for disabled in [
            WorldConfig {
                ecology_revision: 21,
                ..config()
            },
            WorldConfig {
                water: false,
                ..config()
            },
        ] {
            let old = TerraGenerator::new(disabled).unwrap();
            assert_eq!(old.biome_at(240.5, -112.5).weights.misty_coast, 0.);
            assert!(old.coastal_landmark_at([7, -4]).is_none());
        }
        let mut samples = 0;
        for x in (-700..700).step_by(65) {
            for z in (-700..700).step_by(65) {
                let (x, z) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
                let c = full.surface_column(x, z);
                if c.biome_kind(full.config().sea_level) != BiomeKind::MistyCoast {
                    continue;
                }
                samples += 1;
                // Independent sample using fully evaluated surface columns (lakes
                // at this height only qualify if they are also real ocean biome).
                assert!((-3..=3).any(|dx| (-3..=3).any(|dz| {
                    let (px, pz) = (x + f64::from(dx) * 44., z + f64::from(dz) * 44.);
                    (x - px).hypot(z - pz) <= 174.
                        && full
                            .surface_column(px, pz)
                            .biome_kind(full.config().sea_level)
                            == BiomeKind::Ocean
                })));
            }
        }
        assert!(samples > 3);
    }
}
