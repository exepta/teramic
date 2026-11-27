//! Shore plants, inhabited stilt houses and broken arcades rooted in the peat.
use super::{Cell, Habitat, TILE_SIZE, Tile, ticket};
use crate::TerraGenerator;
use std::collections::BTreeMap;
use terra_core::creatures::villagers::{SWAMP_HERMIT, SWAMP_WARLOCK, SWAMP_WITCH};
use terra_voxel::MaterialId as M;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwampLandmark {
    WitchesLodge,
    SunkenRuins,
    AbandonedHut,
}

/// The floor cell and orientation are shared between the authored geometry,
/// its residents, cartography captures and chunk regeneration.
#[derive(Clone, Copy, Debug)]
pub struct SwampSite {
    pub kind: SwampLandmark,
    pub floor: Cell,
    rotation: u32,
}

impl SwampSite {
    fn at(self, x: i64, y: i64, z: i64) -> Cell {
        let [dx, dz] = match self.rotation {
            0 => [x, z],
            1 => [-z, x],
            2 => [-x, -z],
            _ => [z, -x],
        };
        [self.floor[0] + dx, self.floor[1] + y, self.floor[2] + dz]
    }
    /// Last plank on the entrance pier, or the same approach to the arcades.
    pub fn landing(self) -> Cell {
        self.at(0, 0, 12)
    }
    fn resident_cells(self) -> Vec<(Cell, u8)> {
        match self.kind {
            SwampLandmark::WitchesLodge => vec![
                (self.at(-2, 1, 0), SWAMP_WITCH),
                (self.at(2, 1, 1), SWAMP_WARLOCK),
            ],
            SwampLandmark::AbandonedHut => vec![(self.at(0, 1, 0), SWAMP_HERMIT)],
            SwampLandmark::SunkenRuins => Vec::new(),
        }
    }
}

impl TerraGenerator {
    fn swamp_site_candidate(&self, tile: Tile) -> Option<(SwampLandmark, u32)> {
        if self.config().ecology_revision < 18
            || !self.config().biomes
            || !self.config().water
            || tile.iter().any(|v| v.unsigned_abs() > 31_250_000)
        {
            return None;
        }
        // A jittered 96m lattice leaves open marsh between landmarks.
        let cell = tile.map(|v| v.div_euclid(3));
        let key = ticket(self.config().seed.0, cell, 0x5ea502);
        if !key.is_multiple_of(3)
            || tile
                != [
                    cell[0] * 3 + i64::from((key >> 5) % 3),
                    cell[1] * 3 + i64::from((key >> 9) % 3),
                ]
        {
            return None;
        }
        let kind = match (key >> 13) % 3 {
            0 => SwampLandmark::WitchesLodge,
            1 => SwampLandmark::SunkenRuins,
            _ => SwampLandmark::AbandonedHut,
        };
        let settings = self.config().structures;
        if !(match kind {
            SwampLandmark::SunkenRuins => settings.ruins,
            _ => settings.villages,
        }) {
            return None;
        }
        Some((kind, (key >> 18) & 3))
    }
    /// Leave open water and bank around a potential landing, including sites that silted up.
    pub(in crate::generation) fn marsh_clearing(&self, x: i64, z: i64) -> bool {
        let local = [x, z].map(|v| (v.rem_euclid(TILE_SIZE) - 16).abs());
        local[0] <= 12
            && local[1] <= 12
            && self
                .swamp_site_candidate([x.div_euclid(TILE_SIZE), z.div_euclid(TILE_SIZE)])
                .is_some()
    }
    pub fn swamp_site(&self, tile: Tile) -> Option<SwampSite> {
        let (kind, rotation) = self.swamp_site_candidate(tile)?;
        let x = tile[0] * TILE_SIZE + 16;
        let z = tile[1] * TILE_SIZE + 16;
        if self
            .climate_biome_at(x as f64 + 0.5, z as f64 + 0.5)
            .weights
            .swamp
            < 0.72
        {
            return None;
        }
        if !self
            .structures_for_column([tile[0] * 2, tile[1] * 2], 32)
            .is_empty()
        {
            return None;
        }
        let mut wet = 0;
        let mut high = i64::MIN;
        let mut low = i64::MAX;
        for dz in [-7, 0, 7] {
            for dx in [-7, 0, 7] {
                let c = self.surface_column((x + dx) as f64 + 0.5, (z + dz) as f64 + 0.5);
                if c.cave_cavity.is_some_and(|b| b[1] >= c.height - 1.) || c.river_cavity.is_some()
                {
                    return None;
                }
                let bed = (c.height - 0.5).floor() as i64;
                wet += usize::from(c.water_level.is_some_and(|w| {
                    w > bed as f64 + 1. && (w - self.config().sea_level).abs() < 0.01
                }));
                high = high.max(bed);
                low = low.min(bed);
            }
        }
        let water = self.config().sea_level.ceil() as i64;
        if wet == 0 || low < water - 7 || high > water + 4 {
            return None;
        }
        let y = if kind == SwampLandmark::SunkenRuins {
            water - 2
        } else {
            high.max(water) + 1
        };
        Some(SwampSite {
            kind,
            floor: [x, y, z],
            rotation,
        })
    }
    /// Residents have a single owning feature tile and stand on an actual plank floor.
    pub fn swamp_residents(&self, tile: Tile) -> Vec<(Cell, u8)> {
        self.swamp_site(tile)
            .map(SwampSite::resident_cells)
            .unwrap_or_default()
    }
}

pub(super) fn build(h: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    let origin = tile.map(|v| v * TILE_SIZE);
    if h.terrain
        .climate_biome_at((origin[0] + 16) as f64, (origin[1] + 16) as f64)
        .weights
        .swamp
        < 0.22
    {
        return;
    }
    let seed = h.terrain.config().seed.0;
    for iz in 0..11 {
        for ix in 0..11 {
            let key = ticket(seed, [tile[0] * 11 + ix, tile[1] * 11 + iz], 0xbed50);
            let p = [
                origin[0] + (ix * 3 + i64::from(key % 3)).min(31),
                origin[1] + (iz * 3 + i64::from((key >> 4) % 3)).min(31),
            ];
            if h.reserved(p, 1) {
                continue;
            }
            let c = h.column(p);
            if c.biome.weights.swamp < 0.46 || c.cave.open_to_sky() || c.river_cavity.is_some() {
                continue;
            }
            let floor = (c.height - 0.5).floor() as i64;
            let depth = c
                .water_level
                .map_or(0., |w| (w - floor as f64 - 1.).max(0.));
            let roll = (key >> 9) % 100;
            let (y, m) = if depth > 0. {
                if depth < 3.5 && roll < 10 && c.water_flow == [0.; 2] {
                    (
                        c.water_level.unwrap().ceil() as i64,
                        if roll < 3 { M::LotusFlower } else { M::LilyPad },
                    )
                } else if depth <= 1. && roll < 29 {
                    (
                        floor + 1,
                        if roll.is_multiple_of(2) {
                            M::SwampCattails
                        } else {
                            M::SwampReeds
                        },
                    )
                } else {
                    continue;
                }
            } else {
                if c.slope > 1.5 {
                    continue;
                }
                let m = match roll {
                    0..=15 => M::SwampSedge,
                    16..=26 => M::SwampFern,
                    27..=36 => M::SwampReeds,
                    37..=44 => M::SwampCattails,
                    45..=48 => M::BogIris,
                    49..=51 => M::BogLoosestrife,
                    52..=54 => M::SwampMushrooms,
                    55..=56 => M::SwampGlowcaps,
                    57 => M::SwampToadstools,
                    58 => M::MarshFireflies,
                    59 => M::MossyStump,
                    60 => M::FallenWillow,
                    61..=62 => M::SwampBoulder,
                    63 => M::WillowRoots,
                    64 if key.is_multiple_of(4) => M::RootArch,
                    _ => continue,
                };
                (floor + 1, m)
            };
            if !h.terrain.in_prop_patch(p, m as u16) {
                continue;
            }
            let at = [p[0], y, p[1]];
            if !h.tree_occupied(at) {
                cells.entry(at).or_insert(m as u16);
            }
        }
    }
    if let Some(site) = h.terrain.swamp_site(tile) {
        match site.kind {
            SwampLandmark::SunkenRuins => ruins(h, site, seed, cells),
            _ => hut(h, site, cells),
        }
    }
}

fn support(
    h: &mut Habitat<'_>,
    site: SwampSite,
    x: i64,
    z: i64,
    to: i64,
    m: M,
    cells: &mut BTreeMap<Cell, u16>,
) {
    let [px, y, pz] = site.at(x, to, z);
    let base = (h.column([px, pz]).height - 0.5).floor() as i64;
    for level in base.min(y)..=y {
        cells.insert([px, level, pz], m as u16);
    }
}

fn hut(h: &mut Habitat<'_>, s: SwampSite, cells: &mut BTreeMap<Cell, u16>) {
    let lodge = s.kind == SwampLandmark::WitchesLodge;
    let w = if lodge { 5i64 } else { 3 };
    let d = if lodge { 4i64 } else { 3 };
    for x in -w - 1..=w + 1 {
        for z in -d - 1..=d + 2 {
            let eave = x.abs() == w + 1 || z == -d - 1 || z == d + 2;
            for y in 0..=5 + w {
                let p = s.at(x, y, z);
                let wall = x.abs() == w || z.abs() == d;
                let ridge = 4 + w + 1 - x.abs();
                let block = if y == 0 {
                    M::WillowPlanks
                } else if z <= d + 1 && y == ridge {
                    if (x < 0 && (3 * x + z).rem_euclid(9) < 3) || !lodge && z < 0 && x < 1 {
                        M::MossyPeat
                    } else if x.abs() == w + 1 || x == 0 {
                        M::WillowLog
                    } else {
                        M::SprucePlanks
                    }
                } else if !eave && wall && y < ridge {
                    if x.abs() == w && z.abs() == d {
                        M::WillowLog
                    } else if (z == d && x.abs() <= 1 && y < 4)
                        || (y == 2 && (x.abs() == w && z.abs() <= 1 || z == -d && x.abs() <= 2))
                    {
                        M::Air
                    } else {
                        M::WillowPlanks
                    }
                } else {
                    M::Air
                };
                cells.insert(p, block as u16);
            }
        }
    }
    for x in [-w, w] {
        for z in [-d, d, d + 2] {
            support(h, s, x, z, 0, M::MossyWillowLog, cells);
        }
    }
    // A timber-framed gable rises behind the lower, open-sided porch.
    for x in -w..=w {
        let edge = w - x.abs();
        cells.insert(s.at(x, 4 + edge, d), M::WillowLog as u16);
        cells.insert(s.at(x, 4, d), M::WillowLog as u16);
        if x.abs() >= 2 && x.abs() < w {
            cells.insert(s.at(x, 2, d), M::Air as u16);
            cells.insert(s.at(x, 1, d + 1), M::SpruceSlab as u16);
        }
    }
    for x in -1..=1 {
        cells.insert(s.at(x, 6, d), M::Air as u16);
    }
    for x in -(w + 1)..=w + 1 {
        cells.insert(s.at(x, 4, d + 1), M::SpruceSlab as u16);
        cells.insert(s.at(x, 3, d + 2), M::SpruceSlab as u16);
    }
    for x in [-w, w] {
        for y in 1..=3 {
            cells.insert(s.at(x, y, d + 2), M::SpruceFence as u16);
        }
        cells.insert(s.at(x - x.signum(), 2, d + 2), M::Lantern as u16);
        cells.insert(s.at(x, 1, d + 1), M::SwampBanner as u16);
    }
    // The abandoned roof has an exposed rafter. The witches have a stone flue.
    if lodge {
        for y in 1..=9 {
            cells.insert(s.at(3, y, -2), M::SwampRuinedBricks as u16);
        }
        cells.insert(s.at(3, 10, -2), M::StoneSlab as u16);
        cells.insert(s.at(3, 1, -1), M::Cauldron as u16);
        cells.insert(s.at(0, 5, d), M::Lantern as u16);
    } else {
        cells.insert(s.at(-2, 3 + w, -2), M::Air as u16);
        cells.insert(s.at(-2, 3 + w, -1), M::Air as u16);
        cells.insert(s.at(-w + 1, 1, d + 1), M::BarnacledBarrel as u16);
    }
    // A railed jetty with a lower landing is the entrance from the water.
    for z in d + 3..=12 {
        for x in -1..=1 {
            cells.insert(s.at(x, 0, z), M::WillowPlanks as u16);
        }
        for x in [-2, 2] {
            if z < 12 && !(z == 10 && x > 0) {
                cells.insert(s.at(x, 0, z), M::WillowPlanks as u16);
                cells.insert(s.at(x, 1, z), M::SpruceFence as u16);
            }
        }
        if z % 3 == 0 {
            for x in [-2, 2] {
                support(h, s, x, z, 1, M::MossyWillowLog, cells);
            }
        }
    }
    for x in [-2, 2] {
        cells.insert(s.at(x, 1, 12), M::SwampLanternPost as u16);
    }
    let mooring = s.at(4, 0, 11);
    let water = h.column([mooring[0], mooring[2]]).water_level;
    if let Some(water) = water {
        cells.insert(
            [mooring[0], water.ceil() as i64, mooring[2]],
            M::WillowBoat as u16,
        );
    }
    cells.insert(s.at(w - 1, 1, -d + 1), M::Lantern as u16);
    cells.insert(s.at(-w, 3, d + 1), M::SwampBanner as u16);
    if lodge {
        // Open watch platform and lanterns at the side of the witches' house.
        for x in -13i64..=-7 {
            for z in -4..=2 {
                let y = 8 + (3 - (x + 10).abs()).max(0);
                cells.insert(
                    s.at(x, y, z),
                    if x == -13 || x == -7 {
                        M::WillowLog
                    } else {
                        M::SprucePlanks
                    } as u16,
                );
            }
        }
        for x in -12..=-8 {
            for z in -3..=1 {
                cells.insert(s.at(x, 4, z), M::WillowPlanks as u16);
                if x == -12 || z == -3 {
                    cells.insert(s.at(x, 5, z), M::OakFence as u16);
                }
            }
        }
        for x in [-12, -8] {
            for z in [-3, 1] {
                support(h, s, x, z, 8, M::MossyWillowLog, cells);
            }
        }
        for x in -8..=-5 {
            for z in [-1, 0] {
                cells.insert(s.at(x, -x - 4, z), M::WillowPlanks as u16);
            }
        }
        for x in -11..=-9 {
            cells.insert(s.at(x, 4, 2), M::SpruceSlab as u16);
            cells.insert(s.at(x, 5, 1), M::SpruceFence as u16);
        }
        cells.insert(s.at(-10, 7, 1), M::Lantern as u16);
        cells.insert(s.at(-11, 5, 2), M::SwampBanner as u16);
        cells.insert(s.at(0, 1, -d + 1), M::SwampGlowcaps as u16);
    }
}

fn ruins(h: &mut Habitat<'_>, s: SwampSite, seed: u64, cells: &mut BTreeMap<Cell, u16>) {
    for (cx, cz, span, height, sideways) in [
        (0, 0, 4i64, 8, false),
        (-7, -4, 3, 5, true),
        (7, -5, 3, 5, true),
    ] {
        for along in -span..=span {
            for thick in 0..=1 {
                let (x, z) = if sideways {
                    (cx + thick, cz + along)
                } else {
                    (cx + along, cz + thick)
                };
                let a = along.abs();
                // Each stone course steps inward toward the keystone. One side
                // arcade has lost part of its crown and tumbles into the pool.
                let crown = height + 1 - a;
                let spring = if a == span { 0 } else { crown - 1 };
                if cx < 0 && along == 1 && thick == 0 {
                    continue;
                }
                if spring == 0 {
                    support(h, s, x, z, 0, M::MossySwampStone, cells);
                }
                for y in spring..=crown {
                    let wear = ticket(seed, [s.at(x, y, z)[0] + y, s.at(x, y, z)[2] - y], 0xaf5);
                    let m = if wear.is_multiple_of(4) {
                        M::SwampRuinedBricks
                    } else {
                        M::MossySwampStone
                    };
                    cells.insert(s.at(x, y, z), m as u16);
                }
                if a > 1 && thick == 1 && along % 2 == 0 {
                    cells.insert(s.at(x, spring.max(2) - 1, z + 1), M::HangingMoss as u16);
                }
            }
        }
    }
    for (x, z) in [(-4, 2), (4, 2), (-5, 0), (5, 0)] {
        support(h, s, x, z, 2, M::SwampRuinedBricks, cells);
        cells.insert(s.at(x, 3, z), M::MossySwampStone as u16);
    }
    for (x, z, top) in [(-5, 6, 3), (6, 5, 2), (0, -8, 2), (-9, 5, 1)] {
        support(h, s, x, z, top, M::SwampRuinedBricks, cells);
        cells.insert(s.at(x, top + 1, z), M::SwampGlowcaps as u16);
    }
}
