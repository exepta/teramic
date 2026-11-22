//! Rare, seed-stable boss territories carved and stamped through the normal structure pipeline.
use super::*;
use crate::generation::biomes::BiomeKind;
use terra_core::creatures::CreatureKind;
use terra_voxel::MaterialId as M;

impl StructureKind {
    pub fn dragon(self) -> Option<CreatureKind> {
        match self {
            Self::DarkDragonLair => Some(CreatureKind::DarkDragon),
            Self::EarthDragonLair => Some(CreatureKind::EarthDragon),
            Self::WaterDragonLair => Some(CreatureKind::WaterDragon),
            _ => None,
        }
    }
}

impl SiteSummary {
    /// The daylit entrance extends further from the platform than the side passages.
    pub fn dragon_entrance(&self) -> Option<[i64; 2]> {
        self.kind.dragon()?;
        let [x, _, z] = self.center;
        let [lo, hi] = self.bounds;
        [
            (hi[0] - x, [1, 0]),
            (x - lo[0], [-1, 0]),
            (hi[2] - z, [0, 1]),
            (z - lo[2], [0, -1]),
        ]
        .into_iter()
        .max_by_key(|(d, _)| *d)
        .map(|(_, direction)| direction)
    }
}

impl TerraGenerator {
    /// Cheap candidate filter shared by the planner, encounter lookup and /locate.
    pub fn dragon_lair_candidate(&self, region: [i64; 2]) -> bool {
        self.config().structures_revision >= 11
            && self.config().biomes
            && self.config().caves
            && self.config().structures.ruins
            && region.iter().all(|v| v.unsigned_abs() <= 2500)
            && region != [0, 0]
            && layer_seed(
                u64::from(regional_seed(self.config().seed.0, region)),
                0xd2a6_0ace,
            )
            .is_multiple_of(13)
    }

    /// A nest belongs to exactly one 32-block ecology cell even across negative regions.
    pub fn dragon_nest(&self, home: [i64; 2]) -> Option<(CreatureKind, [i64; 3])> {
        let (kind, center, _) = self.dragon_roost(home)?;
        Some((kind, center))
    }

    /// Return the territory's doorway as well as the spot its guardian rests on.
    pub fn dragon_roost(&self, home: [i64; 2]) -> Option<(CreatureKind, [i64; 3], [i64; 2])> {
        let region = home.map(|v| v.div_euclid(REGION_SIZE / 32));
        if !self.dragon_lair_candidate(region)
            || home.iter().any(|v| {
                let cells = if self.config().structures_revision >= 13 {
                    3..=8
                } else {
                    5..=6
                };
                !cells.contains(&v.rem_euclid(REGION_SIZE / 32))
            })
        {
            return None;
        }
        let site = self.planned_site(region)?;
        let center = site.summary.center;
        ([center[0], center[2]].map(|v| v.div_euclid(32)) == home).then_some((
            site.summary.kind.dragon()?,
            center,
            site.summary.dragon_entrance()?,
        ))
    }
}

fn choose(terrain: &TerraGenerator, x: i64, z: i64) -> Option<StructureKind> {
    let column = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
    let biome = terrain.surface_biome(&column);
    let flooded = column
        .water_level
        .is_some_and(|level| level > column.height);
    if !flooded {
        match biome {
            BiomeKind::BloodForest => return Some(StructureKind::DarkDragonLair),
            BiomeKind::Highland
                if column.height > terrain.config().sea_level + 32.0
                    && layer_seed(
                        terrain.config().seed.0 ^ (x as u64).rotate_left(19),
                        z as u64,
                    )
                    .is_multiple_of(5) =>
            {
                return Some(StructureKind::EarthDragonLair);
            }
            BiomeKind::Desert | BiomeKind::ClifftedDesert => {
                return Some(StructureKind::EarthDragonLair);
            }
            _ => {}
        }
    }
    // Sample actual coast on both sides. Deep ocean and inland lakes cannot claim a water dragon.
    if !terrain.config().water || (column.height - terrain.config().sea_level).abs() > 13.0 {
        return None;
    }
    let mut sea = false;
    let mut land = false;
    for (dx, dz) in [
        (0, 0),
        (52, 0),
        (-52, 0),
        (0, 52),
        (0, -52),
        (36, 36),
        (-36, -36),
        (36, -36),
        (-36, 36),
    ] {
        let sample = terrain.surface_column((x + dx) as f64 + 0.5, (z + dz) as f64 + 0.5);
        sea |= (terrain.surface_biome(&sample) == BiomeKind::Ocean
            || sample.biome.weights.ocean > 0.42)
            && sample.water_level.is_some_and(|w| {
                (w - terrain.config().sea_level).abs() < 1.0 && w > sample.height + 1.0
            });
        land |= sample.height > terrain.config().sea_level + 0.5;
    }
    (sea && land).then_some(StructureKind::WaterDragonLair)
}

/// Select a coastal bank that can actually contain the chamber above tide.
/// The saddle in front must remain dry all the way to the landing ledge.
fn coastal_bank(
    terrain: &TerraGenerator,
    middle: [i64; 2],
    seed: u32,
) -> Option<([i64; 2], [i64; 2], i64)> {
    let sea = terrain.config().sea_level;
    let tide = sea.ceil() as i64;
    let ground = |x: i64, z: i64| terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5);
    let mut best = None;
    let mut best_score = f64::INFINITY;
    for dz in (-84..=84).step_by(28) {
        for dx in (-84..=84).step_by(28) {
            let [x, z] = [middle[0] + dx, middle[1] + dz];
            let high = ground(x, z);
            if high < sea + 14.0 {
                continue;
            }
            let floor = (high.floor() as i64 - 20).max(tide + 1);
            for i in 0..4 {
                let [fx, fz] = [[1, 0], [0, 1], [-1, 0], [0, -1]][(i + seed as usize % 4) % 4];
                let h = |u, v| ground(x + u * fz + v * fx, z - u * fx + v * fz);
                let bank = h(-12, -8).min(h(12, -8)).min(h(0, -17));
                let lip = h(0, 27);
                let pad = h(0, 54);
                let beyond = h(0, 70);
                if bank < floor as f64 + 12.0
                    || pad < sea + 1.
                    || beyond < sea + 0.5
                    || pad > high + 1.
                {
                    continue;
                }
                let cover = (floor as f64 + 20. - bank).max(0.);
                let score = cover * 4.0
                    + (lip - (floor as f64 + 11.)).abs() * 0.8
                    + (pad - (floor as f64 + 3.)).abs()
                    + (beyond - pad).abs() * 0.6
                    + (dx as f64).hypot(dz as f64) * 0.03;
                if score < best_score {
                    best_score = score;
                    best = Some(([x, z], [fx, fz], floor));
                }
            }
        }
    }
    best
}

pub(super) fn plan(terrain: &TerraGenerator, region: [i64; 2], seed: u32) -> Option<Site> {
    if !terrain.dragon_lair_candidate(region) {
        return None;
    }
    let [rx, rz] = region.map(|v| REGION_SIZE * v + REGION_SIZE / 2);
    let x = rx + i64::from((seed >> 6) % 33) - 16;
    let z = rz + i64::from((seed >> 19) % 33) - 16;
    let kind = choose(terrain, x, z)?;
    let attempts: &[[i64; 2]] = if terrain.config().structures_revision >= 15 {
        &[[0, 0], [32, -24], [-32, 24], [24, 32], [-24, -32]]
    } else {
        &[[0, 0]]
    };
    for &[dx, dz] in attempts {
        let alternative = dx != 0 || dz != 0;
        if alternative
            && (kind == StructureKind::WaterDragonLair
                || choose(terrain, x + dx, z + dz) != Some(kind))
        {
            continue;
        }
        if let Some(site) = plan_at(terrain, region, seed, [x + dx, z + dz], kind) {
            return Some(site);
        }
    }
    None
}

fn plan_at(
    terrain: &TerraGenerator,
    region: [i64; 2],
    seed: u32,
    [x, z]: [i64; 2],
    kind: StructureKind,
) -> Option<Site> {
    let directions = [[1, 0], [0, 1], [-1, 0], [0, -1]];
    let ground = terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5);
    let coast =
        kind == StructureKind::WaterDragonLair && terrain.config().structures_revision >= 13;
    let ([x, z], forward, y) = if coast {
        coastal_bank(
            terrain,
            region.map(|v| REGION_SIZE * v + REGION_SIZE / 2),
            seed,
        )?
    } else {
        let forward = (0..4)
            .map(|i| directions[(i + seed as usize % 4) % 4])
            .min_by_key(|[dx, dz]| {
                if terrain.config().structures_revision < 12 {
                    return (terrain.surface_height((x + dx * 32) as f64, (z + dz * 32) as f64)
                        * 32.0) as i64;
                }
                // Prefer a walkable bank. Dropping straight into deep ocean or a ravine
                // makes a wide opening every bit as inaccessible as a narrow doorway.
                let outside =
                    terrain.surface_column((x + dx * 64) as f64 + 0.5, (z + dz * 64) as f64 + 0.5);
                let beyond =
                    terrain.surface_height((x + dx * 70) as f64 + 0.5, (z + dz * 70) as f64 + 0.5);
                let depth = outside
                    .water_level
                    .map_or(0.0, |level| (level - outside.height).max(0.0));
                ((outside.height - ground).abs() * 2.0
                    + (outside.height - beyond).abs() * 3.0
                    + depth * 5.0) as i64
            })?;
        let y = if kind == StructureKind::WaterDragonLair {
            terrain.config().sea_level.ceil() as i64 + 4
        } else {
            ground.floor() as i64
                - if terrain.config().structures_revision >= 13 {
                    19
                } else {
                    5
                }
        };
        ([x, z], forward, y)
    };
    if !(-95..=315).contains(&y) {
        return None;
    }
    let style = if kind == StructureKind::EarthDragonLair {
        VillageStyle::Desert
    } else {
        VillageStyle::Temperate
    };
    let mut builder = Builder {
        terrain,
        summary: SiteSummary {
            id: (u64::from(seed) << 32) ^ (region[0] as u64).rotate_left(8) ^ region[1] as u64,
            kind,
            style,
            center: [x, y, z],
            bounds: [[0; 3]; 2],
            buildings: Vec::new(),
            containers: Vec::new(),
            grave: None,
        },
        seed,
        writes: 0,
        failed: false,
        patches: BTreeMap::new(),
        objects: BTreeMap::new(),
    };
    carve(&mut builder, forward);
    builder.finish()
}

#[derive(Clone, Copy)]
enum Hall {
    Oval,
    Crescent,
    TwinVault,
    Cathedral,
}

impl Hall {
    fn dimensions(self, seed: u32) -> [f64; 2] {
        let spread = f64::from((seed >> 8) % 5);
        match self {
            Self::Oval => [24. + spread, 29.],
            Self::Crescent => [25. + spread, 34.],
            Self::TwinVault => [29. + spread, 33.],
            Self::Cathedral => [33. + spread, 40.],
        }
    }

    fn nest_radius(self, a: f64, d: f64) -> f64 {
        match self {
            Self::Oval => (a / 12.3).hypot(d / 10.5),
            Self::Crescent => ((a - 2.1 * (d / 11.).powi(2)) / 13.).hypot(d / 11.),
            Self::TwinVault => ((a + d * 0.17) / 15.).hypot(d / 10.8),
            Self::Cathedral => (a / 16.5).hypot(d / 13.5),
        }
    }

    fn wall(self, a: f64, d: f64, [width, depth]: [f64; 2]) -> f64 {
        match self {
            Self::Oval | Self::Cathedral => (a / width).hypot(d / depth),
            Self::Crescent => ((a - 8.5 * (d / depth).powi(2)) / width).hypot(d / depth),
            Self::TwinVault => {
                let central = (a / (width * 0.78)).hypot(d / depth);
                let side = ((a + width * 0.64) / (width * 0.52)).hypot((d + 10.) / (depth * 0.73));
                central.min(side)
            }
        }
    }
}

struct Palette {
    stone: M,
    seam: M,
    floor: M,
    trim: M,
    nest: M,
    root: M,
    crystal: M,
    accent: M,
    egg: M,
}

fn palette(kind: StructureKind) -> Palette {
    match kind {
        StructureKind::DarkDragonLair => Palette {
            stone: M::DarkGraveStone,
            seam: M::VeinedStone,
            floor: M::BloodStone,
            trim: M::BloodMoss,
            nest: M::BloodDirt,
            root: M::BloodRoots,
            crystal: M::SoulCrystalLarge,
            accent: M::BloodBones,
            egg: M::DarkDragonEgg,
        },
        StructureKind::WaterDragonLair => Palette {
            stone: M::Stone,
            seam: M::SmoothStone,
            floor: M::SaltStone,
            trim: M::MossyStone,
            nest: M::Gravel,
            root: M::CaveRoot,
            crystal: M::TideGeode,
            accent: M::MossyStone,
            egg: M::WaterDragonEgg,
        },
        _ => Palette {
            stone: M::Sandstone,
            seam: M::Dripstone,
            floor: M::Sandstone,
            trim: M::MossyStone,
            nest: M::Dirt,
            root: M::CaveRoot,
            crystal: M::AmberGeode,
            accent: M::BonePile,
            egg: M::EarthDragonEgg,
        },
    }
}

fn carve(b: &mut Builder<'_>, [fx, fz]: [i64; 2]) {
    let [cx, cy, cz] = b.summary.center;
    let kind = b.summary.kind;
    let mut p = palette(kind);
    if b.terrain.config().structures_revision >= 13 {
        match kind {
            StructureKind::DarkDragonLair => p.stone = M::BloodStone,
            StructureKind::EarthDragonLair => p.trim = M::Sand,
            _ => p.seam = M::MossyStone,
        }
    }
    let at = |u: i64, h: i64, v: i64| [cx + u * fz + v * fx, cy + h, cz - u * fx + v * fz];
    let phase = f64::from(b.seed & 255) * 0.0245;
    let varied = b.terrain.config().structures_revision >= 15;
    let hall = if varied {
        [Hall::Crescent, Hall::TwinVault, Hall::Cathedral][(b.seed.rotate_left(11) % 3) as usize]
    } else {
        Hall::Oval
    };
    let dimensions = hall.dimensions(b.seed);
    let wet = kind == StructureKind::WaterDragonLair;
    let accessible = b.terrain.config().structures_revision >= 12;
    let concealed = b.terrain.config().structures_revision >= 13;
    let dressed = b.terrain.config().structures_revision >= 14;
    let coastal = concealed && wet;
    let portal_lip = |a: f64| {
        if varied {
            // One continuous scalloped arch across the approach. A local
            // rock-height threshold can puncture isolated holes in a thin roof.
            23. + 7. * (a / 13.).powi(2).min(1.) + 0.9 * (a * 0.24 + phase).sin()
        } else {
            30. + 3.3 * (a * 0.26 + phase).sin()
        }
    };
    let length = if accessible { 64 } else { 49 };
    let outer = at(0, 0, length);
    let outer_floor = b.ground(outer[0], outer[2]) - cy - 1;
    let back = if varied {
        -66
    } else if concealed {
        -49
    } else {
        -33
    };
    let side = if varied {
        68
    } else if concealed {
        52
    } else {
        40
    };
    // The runway is graded after the vault. Remember the actual exposed floor
    // or roof so the second pass can join its banks without hollow fill or a cut lip.
    let mut bank_heights = BTreeMap::new();
    for v in back..=length {
        for u in -side..=side {
            let (a, d) = (u as f64, v as f64);
            let radial = hall.wall(a, d, dimensions);
            let shoulder = radial + 0.027 * (a * 0.31 + phase).sin() * (d * 0.22).cos();
            let aisle = (a - 2.4 * (d * 0.12 + phase).sin()).abs();
            // A broad flared mouth admits daylight; a level central tread connects the
            // nest to the local hillside. Save manifests before revision 12 keep their mouth.
            let entry = if concealed {
                v >= 0 && aisle < 10.7 + 2.1 * (d * 0.095 + phase).sin() + (d / 56.0).min(1.0) * 1.1
            } else if accessible {
                v >= 0
                    && a.abs()
                        < 15.0
                            + 4.5 * ((d - 18.0) / 26.0).clamp(0.0, 1.0)
                            + 1.5 * (d * 0.13 + phase).sin()
            } else {
                v >= 5 && aisle < (11.5 - d * 0.06).max(7.0)
            };
            let tunnel =
                (13..=52).contains(&u) && (d + 7.0 - 3.2 * (a * 0.24 + phase).sin()).abs() < 4.7;
            if shoulder > if concealed { 1.62 } else { 1.07 } && !entry && !tunnel {
                continue;
            }
            let point = at(u, 0, v);
            let column = b
                .terrain
                .surface_column(point[0] as f64 + 0.5, point[2] as f64 + 0.5);
            let local_ground = (column.height - 0.5).floor() as i64 + 1 - cy;
            // Raise a rounded bluff only where the chamber would break through a low
            // hillside. Its sides taper down to the unchanged biome surface.
            let shoulder_weight = ((1.58 - shoulder) / 0.69).clamp(0.0, 1.0);
            let weight = shoulder_weight * shoulder_weight * (3.0 - 2.0 * shoulder_weight);
            let vault_rise = if matches!(hall, Hall::Cathedral) && !coastal {
                4.0
            } else {
                0.0
            };
            let brow = (if coastal { 18.5 } else { 27.0 })
                + vault_rise
                + 2.7 * (a * 0.098 + d * 0.064 + phase).sin();
            let cap =
                ((brow - local_ground as f64).max(0.0) * weight).round() as i64 + local_ground - 1;
            // A new lair never excavates an existing watercourse. Leave its
            // sloping bank alone and try another site if the hall would cut it.
            if varied && column.water_level.is_some() {
                if shoulder <= 1.07 || entry || tunnel {
                    b.failed = true;
                    return;
                }
                continue;
            }
            if concealed && shoulder > 1.07 && !entry && !tunnel {
                if varied && v >= 22 {
                    bank_heights.insert([u, v], cap);
                }
                dress_slope(
                    b,
                    |height| at(u, height, v),
                    local_ground,
                    cap,
                    p.stone,
                    column.surface_material,
                );
                continue;
            }
            let noise = layer_seed(u64::from(b.seed) ^ (u as u64).rotate_left(25), v as u64);
            let nest_radius = hall.nest_radius(a, d)
                + 0.055 * (a * 0.37 + d * 0.21 + phase).sin()
                + 0.035 * (d * 0.55).sin();
            let channel = a - 17.0 - (d * 0.14 + phase).sin() * 2.7;
            let basin_radius = ((a - 15.0) / 6.3).hypot((d - 8.5) / 8.7);
            let pool =
                wet && (channel.abs() < 2.4 || basin_radius < 1.0) && shoulder < 0.99 && d > -25.0;
            let shelf = if nest_radius <= 1.0 {
                -1
            } else if nest_radius < 1.22 {
                -2
            } else if nest_radius < 1.5 {
                -3
            } else {
                -4
            };
            let mut floor = if pool { -6 } else { shelf };
            // Keep auxiliary passages below the surface until their tread can
            // meet the hillside. A straight chimney through a high bank has no exit.
            if dressed && tunnel && a > 27. {
                let rise = a - 27.;
                floor +=
                    ((local_ground as f64 - 1. - f64::from(floor)).clamp(0., rise)).round() as i32;
            }
            // Bedrock ribs and slumped terraced banks accumulate around the sides of
            // the chamber. The central hatch platform and the front route stay clear.
            if !pool && nest_radius > 1.20 && !(entry && v > 8) && !tunnel {
                let banks = ((a + 18.0) / 8.0).hypot((d + 6.0) / 16.0);
                let ledge = ((a - 8.0) / 15.0).hypot((d + 22.0) / 7.0);
                let uplift = (1.0 - banks).max(0.0) * 8.5 + (1.0 - ledge).max(0.0) * 6.0;
                floor +=
                    (uplift * (0.79 + 0.21 * (a * 0.68 + d * 0.28 + phase).sin())).round() as i32;
            }
            if accessible && entry {
                let rise = ((d - 8.0) / 50.0).clamp(0.0, 1.0);
                let path = -1.0 + (outer_floor as f64 + 1.0) * rise;
                let apron = ((d - 53.0) / 11.0).clamp(0.0, 1.0);
                let path = path + (local_ground as f64 - 1.0 - path) * apron;
                let fill = ((16.0 - a.abs()) / 6.0).clamp(0.0, 1.0);
                floor = (f64::from(floor) + (path - f64::from(floor)) * fill).round() as i32;
            } else if entry && v > 19 {
                let blend = ((d - 19.0) / 30.0).clamp(0.0, 1.0);
                floor = (f64::from(floor) + (local_ground as f64 - 1.0 - f64::from(floor)) * blend)
                    .round() as i32;
            }
            let floor = i64::from(floor);
            let inside = shoulder < 0.965 || entry || tunnel;
            let roof = if coastal && entry {
                let arch = 17.7
                    - 3.7 * (a.abs() / 11.5).min(1.).powi(2)
                    - 4.2 * ((d - 7.) / 23.).clamp(0., 1.)
                    + 0.9 * (a * 0.24 + d * 0.11 + phase).sin();
                floor + arch.round() as i64
            } else if accessible && entry {
                // The center vault accommodates the folded wingtips and long head even
                // on uphill approaches; the arch tapers only at its rock shoulders.
                let vault = 22.0 - 5.0 * (a.abs() / 18.0).min(1.0).powi(2);
                // An ascending exit needs less overhead than the brooding chamber.
                // Fade to an arch under the hillside instead of raising an artificial
                // sandstone fin high above the surrounding dune at the mouth.
                let vault = if varied {
                    let daylight = ((d - 13.) / 16.).clamp(0., 1.);
                    vault + (14. - vault) * daylight
                } else {
                    vault
                };
                floor + vault.round() as i64
            } else if entry && v > 16 {
                // The mouth reaches daylight even if this seed opens onto a steep bank.
                if v > 31 {
                    local_ground.max(floor + 8) + 1
                } else {
                    floor + 13 + ((v - 16) / 3)
                }
            } else if tunnel && shoulder > 0.92 {
                floor + 6
            } else {
                let height = if coastal { 12.0 } else { 16.0 + vault_rise };
                (7.0 + height * (1.0 - shoulder * shoulder).max(0.0).sqrt()).round() as i64
            };
            let lining = (a * 0.47 + 1.35 * (d * 0.43 + phase).sin()).sin()
                + 0.65 * (d * 0.74 - a * 0.14).cos();
            let bedding = if dressed {
                nest_radius < 0.68 && (lining > 0.44 || nest_radius < 0.15)
            } else {
                nest_radius < 0.84 && (lining > -0.43 || nest_radius < 0.15)
            };
            let surface = if concealed && bedding {
                M::StrawBlock
            } else if concealed && v > 32 && entry {
                column.surface_material
            } else if nest_radius < 0.96 {
                p.nest
            } else if !pool
                && ((d * 0.3 + (a * 0.2 + phase).sin()).sin() > 0.78 && nest_radius > 1.4)
            {
                p.trim
            } else if floor > 0 {
                p.seam
            } else {
                p.floor
            };
            let bottom = (floor - 3).min(local_ground - 1).max(floor - 32);
            // Discontinuous mineral lenses follow the fold of the rock. The same
            // field crosses the wall/ceiling junction without tiling a horizontal band.
            let fold =
                5.0 + 4.2 * (a * 0.14 + d * 0.092 + phase).sin() + 2.7 * (d * 0.19 - phase).cos();
            let exposure = (a * 0.23 - d * 0.11 + phase).cos() + 0.6 * (d * 0.29 + phase).sin();
            let rock = |h: i64| {
                if exposure > 0.54 && (h as f64 - fold).abs() < 0.65 + exposure * 0.7 {
                    p.seam
                } else {
                    p.stone
                }
            };
            for h in bottom..floor {
                b.set(at(u, h, v), p.stone as u16);
            }
            b.set(at(u, floor, v), surface as u16);
            if !inside {
                let top = (8.0 + 6.0 * (1.07 - shoulder) * 8.0).round() as i64;
                if varied && v >= 22 {
                    bank_heights.insert([u, v], top.max(cap));
                }
                for h in floor + 1..=top {
                    b.set(at(u, h, v), rock(h) as u16);
                }
                if concealed {
                    dress_slope(
                        b,
                        |height| at(u, height, v),
                        (top + 1).max(local_ground),
                        cap.max(top),
                        p.stone,
                        column.surface_material,
                    );
                }
                continue;
            }
            for h in floor + 1..roof {
                b.set(at(u, h, v), M::Air as u16);
            }
            if matches!(hall, Hall::TwinVault) && u < -13 {
                for h in floor + 1..roof {
                    let t = (h - floor) as f64 / (roof - floor) as f64;
                    let waist = 1.5 + 3.3 * (2. * t - 1.).powi(2);
                    let rib = ((a + 20. - t * 2.) / waist).hypot((d + 4.) / (waist * 1.45));
                    if rib < 1. {
                        b.set(at(u, h, v), rock(h) as u16);
                    }
                }
            }
            // Springs in the channel use the same persistent source contract as
            // village wells. The fluid simulation fills the spaces between them.
            if pool
                && (channel.abs() <= 0.55 && v.rem_euclid(4) == 0
                    || basin_radius < 0.89 && v.rem_euclid(2) == 0 && u.rem_euclid(2) == 0)
            {
                b.object(at(u, -4, v), M::FluidWater as u16, None, None);
            }
            // Exposed slots admit skylight to the nest; the portal and the near end of the side passage stay open.
            let skylight = ((a + 4.0) / 8.4).hypot((d + 6.0) / 7.4) < 1.0
                || ((a - 16.0) / 4.2).hypot((d + 13.0) / 6.8) < 1.0;
            let opening = if concealed {
                (entry
                    && (d > portal_lip(a)
                        || !varied && coastal && d > 17.0 && roof + 1 >= local_ground))
                    || (tunnel && u > 34 && (!dressed || local_ground <= floor + 2))
                    || (!dressed && ((a - 16.0) / 2.6).hypot((d + 13.0) / 3.7) < 1.0)
            } else {
                (entry && v > 31) || (tunnel && u > 34) || skylight
            };
            if varied && v >= 22 {
                bank_heights.insert([u, v], if opening { floor } else { cap.max(roof + 3) });
            }
            if opening {
                for h in roof..=local_ground + 2 {
                    b.set(at(u, h, v), M::Air as u16);
                }
            } else {
                for h in roof..=roof + 2 {
                    b.set(at(u, h, v), rock(h) as u16);
                }
                if concealed {
                    dress_slope(
                        b,
                        |height| at(u, height, v),
                        (roof + 3).max(local_ground),
                        cap.max(roof + 3),
                        p.stone,
                        column.surface_material,
                    );
                }
                if !entry && v < -9 && shoulder > 0.74 && noise.is_multiple_of(57) {
                    b.set(at(u, roof - 1, v), M::DripstoneFormation as u16);
                }
            }
            if !pool
                && shoulder < 0.92
                && nest_radius > 1.1
                && !(accessible && entry && u.abs() <= 9)
            {
                if noise.is_multiple_of(127) {
                    b.set(at(u, floor + 1, v), p.crystal as u16);
                } else if noise.is_multiple_of(79) {
                    b.set(at(u, floor + 1, v), p.accent as u16);
                }
            }
            if dressed && !pool && (0.24..0.86).contains(&nest_radius) {
                // Windrows follow the bowl and leave its middle clear for the guardian.
                let windrow = (nest_radius * 24. + (a * 0.27 + phase).sin() + d * 0.17).sin();
                let midden = ((a + 7.) / 2.9).hypot((d - 1.5) / 1.7) < 1.
                    || ((a - 5.5) / 2.2).hypot((d + 4.5) / 1.7) < 1.;
                if midden && noise.is_multiple_of(3) {
                    let bone = if kind == StructureKind::DarkDragonLair {
                        M::BloodBones
                    } else {
                        M::BonePile
                    };
                    b.set(at(u, floor + 1, v), bone as u16);
                } else if windrow > 0.43 && !noise.is_multiple_of(4) {
                    b.set(at(u, floor + 1, v), M::NestStraw as u16);
                }
            }
            // Woven, broken rim. Two gaps towards the entrance let hatchlings walk out.
            if (0.88..1.05).contains(&nest_radius)
                && !(v > 4 && u.abs() < if accessible { 10 } else { 7 })
            {
                if dressed {
                    // Loose bedding catches on a few exposed roots and stones.
                    // Keep the skull-bearing piles only in the two nearby middens.
                    if noise.is_multiple_of(5) {
                        b.set(at(u, floor + 1, v), p.root as u16);
                    } else if (a * 0.34 + d * 0.20 + phase).sin() > 0.20 {
                        b.set(at(u, floor + 1, v), M::NestStraw as u16);
                    }
                    continue;
                }
                let rim = if (a * 0.34 + d * 0.2 + phase).sin() > 0.15 {
                    p.root
                } else {
                    p.accent
                };
                b.set(at(u, floor + 1, v), rim as u16);
            }
        }
    }
    // A grassy/sandy opening in the canopy is the runway. Keep the native
    // topsoil and ease the clearing back into the bank at its irregular edge.
    if concealed {
        let pad = at(0, 0, 54);
        let lift_ground = b.ground(pad[0], pad[2]) - cy - 1;
        for v in (if varied { 22 } else { 34 })..=84 {
            let half_width = if varied { 39 } else { 31 };
            for u in -half_width..=half_width {
                let a = u as f64;
                let d = v as f64;
                let aisle = (a - 2.4 * (d * 0.12 + phase).sin()).abs();
                let daylight = d - portal_lip(a);
                let bank = varied && daylight > 0. && aisle < 38. && v < 74;
                let radial =
                    (a / 30.).hypot((d - 57.) / 28.) + 0.038 * (a * 0.26 + d * 0.17 + phase).sin();
                if radial >= 1. && !bank {
                    continue;
                }
                let q = at(u, 0, v);
                let column = b
                    .terrain
                    .surface_column(q[0] as f64 + 0.5, q[2] as f64 + 0.5);
                if varied && column.water_level.is_some() {
                    // The landing apron narrows into wet terrain. Its central path
                    // was already required to be dry when selecting the exit.
                    if u.abs() <= 6 && v <= 70 {
                        b.failed = true;
                        return;
                    }
                    continue;
                }
                let old = (column.height - 0.5).floor() as i64 - cy;
                let smooth = |t: f64| {
                    let t = t.clamp(0., 1.);
                    t * t * (3. - 2. * t)
                };
                let weight = if varied {
                    smooth((1. - radial) / 0.64) * smooth((d - 34.) / 16.)
                } else {
                    ((1. - radial) / 0.26).clamp(0., 1.) * ((d - 34.) / 12.).clamp(0., 1.)
                };
                let approach =
                    (lift_ground as f64 + 1.) * ((v as f64 - 8.) / 46.).clamp(0., 1.) - 1.;
                let road = v <= length
                    && aisle < 10.7 + 2.1 * (d * 0.095 + phase).sin() + (d / 56.0).min(1.0) * 1.1;
                let path = -1.0 + (outer_floor as f64 + 1.0) * ((d - 8.0) / 50.0).clamp(0.0, 1.0);
                let previous_top = bank_heights.get(&[u, v]).copied().unwrap_or(old);
                let original = if varied {
                    let bank_weight = if bank {
                        smooth((38. - aisle) / 30.)
                            * smooth((74. - d) / 12.)
                            * if road { 1. } else { smooth(daylight / 9.) }
                    } else {
                        0.
                    };
                    previous_top as f64 + (path - previous_top as f64) * bank_weight
                } else if road {
                    let apron = ((d - 53.0) / 11.0).clamp(0.0, 1.0);
                    let path = path + (old as f64 - path) * apron;
                    -4.0 + (path + 4.0) * ((16.0 - a.abs()) / 6.0).clamp(0.0, 1.0)
                } else {
                    old as f64
                };
                let top = (original + (approach - original) * weight).round() as i64;
                if varied && weight <= 0. && top == previous_top {
                    continue;
                }
                let base = previous_top.min(top) - 1;
                for y in base..top {
                    b.set(at(u, y, v), p.stone as u16);
                }
                b.set(at(u, top, v), column.surface_material as u16);
                let crown = if kind == StructureKind::DarkDragonLair {
                    62
                } else {
                    38
                };
                let clearing = if varied {
                    (crown as f64 * smooth((1. - radial) / 0.27)).ceil() as i64
                } else {
                    crown
                };
                for y in top + 1..=previous_top.max(old).max(top) + clearing {
                    b.set(at(u, y, v), M::Air as u16);
                }
            }
        }
    }
    // A natural stone bridge across the rear gallery frames a second opening. Its
    // unequal abutments transition into the ledges and expose a jagged mineral vein.
    for u in -21_i64..=14 {
        let t = (u + 21) as f64 / 35.0;
        let back = match hall {
            Hall::Oval => -16,
            Hall::Crescent => -18,
            Hall::TwinVault => -20,
            Hall::Cathedral => -25,
        };
        let v = back + (2.8 * (t * 5.8 + phase).sin()).round() as i64;
        let curve = (std::f64::consts::PI * t).sin().max(0.0).powf(0.61);
        let low = (-3.0 + 13.5 * curve).round() as i64;
        let high = low + 2 + (1.7 * (t * 8.0 + phase).sin().abs()).round() as i64;
        let depth = if !(0.08..0.92).contains(&t) { 3 } else { 2 };
        for h in low..=high {
            for edge in -depth..=depth {
                b.set(
                    at(u, h, v + edge),
                    if h == high && edge.abs() < depth && t > 0.5 {
                        p.trim
                    } else if (u + h).rem_euclid(9) == 0 {
                        p.seam
                    } else {
                        p.stone
                    } as u16,
                );
            }
        }
        if u.rem_euclid(5) == 2 {
            b.set(at(u, high + 1, v), p.crystal as u16);
        }
        if u.rem_euclid(7) == 1 {
            b.set(
                at(u, low - 1, v),
                if wet { M::CaveVine } else { p.root } as u16,
            );
        }
    }
    // Group mineral outcrops into coherent spurs. Taller spires rise at their
    // sheltered steep side with small satellites in the surrounding rubble.
    for (u, v, height) in [(-15, 6, 5_i64), (13, -15, 6), (-18, -12, 5), (9, 20, 3)] {
        for (dx, dz, n) in [
            (0, 0, height),
            (-1, 0, height - 2),
            (1, 1, height - 1),
            (0, 2, 2),
        ] {
            for h in -4..n {
                let drift = if h > height / 2 { 1 } else { 0 };
                b.set(
                    at(u + dx + drift, h, v + dz),
                    if h > n - 3 { p.seam } else { p.stone } as u16,
                );
            }
            b.set(
                at(u + dx + i64::from(n - 1 > height / 2), n, v + dz),
                p.crystal as u16,
            );
        }
    }
    // Older sites used block-built fossil ribs. New beds expose the actual bone piles.
    if !wet && !dressed {
        for (u, v, sign) in [(-9, -4, -1), (8, -7, 1), (-6, -9, -1), (11, 1, 1)] {
            let bone = if kind == StructureKind::DarkDragonLair {
                M::SaltStone
            } else {
                M::Sandstone
            };
            for (du, h, dv) in [(0, 0, 0), (1, 1, 0), (1, 2, -1), (1, 3, -1), (0, 4, -2)] {
                b.set(at(u + du * sign, h, v + dv), bone as u16);
            }
            b.set(
                at(u, 5, v - 2),
                if kind == StructureKind::DarkDragonLair {
                    M::BloodBones
                } else {
                    M::BonePile
                } as u16,
            );
        }
    }
    // Waterfalls have support behind the entire drop and feed the channel pool below.
    if wet {
        let v = -12;
        let u = (17.0 + (-12.0_f64 * 0.14 + phase).sin() * 2.7).round() as i64;
        for h in -5..=5 {
            for du in 0..=1 {
                b.set(at(u + du, h, v - 1), p.stone as u16);
                if h == 5 {
                    b.object(at(u + du, h, v), M::FluidWater as u16, None, None);
                } else {
                    b.set(at(u + du, h, v), M::Air as u16);
                }
            }
        }
        b.set(at(u, 6, v - 1), p.crystal as u16);
    }
    let clutch = (b.seed >> 12) % 4;
    for &(u, v) in &[(-8, -5), (-7, -6), (-9, -4)][..clutch as usize] {
        b.set(at(u, 0, v), p.egg as u16);
    }
}

/// Restore the local surface on newly exposed shoulders. Never paint a second
/// grass or sand horizon inside an already covered hill.
fn dress_slope(
    b: &mut Builder<'_>,
    at: impl Fn(i64) -> [i64; 3],
    start: i64,
    cap: i64,
    rock: M,
    skin: M,
) {
    for h in start..=cap {
        let material = if h == cap {
            skin
        } else if h >= cap - 2 {
            match skin {
                M::Grass => M::Dirt,
                M::BloodGrass => M::BloodDirt,
                _ => skin,
            }
        } else {
            rock
        };
        b.set(at(h), material as u16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_dark_lair_displaces_the_crossing_river_in_mesh_and_physics() {
        use crate::generation::SectionGenerator;
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            seed: terra_core::WorldSeed(3809574271049500506),
            structures_revision: 14,
            rivers: true,
            biome_definitions: Some(
                terra_core::BiomeDefinitions::load_directory(
                    terra_core::asset_directory().join("biome"),
                )
                .unwrap(),
            ),
            ..Default::default()
        })
        .unwrap();
        let site = terrain.planned_site([14, 12]).unwrap();
        assert_eq!(site.summary.center, [5568, 20, 4805]);
        let [lo, hi] = site.summary.bounds;
        let (x, z, y) = (lo[2]..=hi[2])
            .find_map(|z| {
                (lo[0]..=hi[0]).find_map(|x| {
                    let w = terrain.natural_surface_water(x as f64 + 0.5, z as f64 + 0.5)?;
                    let y = w.level.ceil() as i64 - 1;
                    (site.block_at([x, y, z]) == Some(0) && site.displaces_water(x, z, w))
                        .then_some((x, z, y))
                })
            })
            .expect("actual stamped opening intersected by the original river");
        let (sx, sz) = (x as f64 + 0.5, z as f64 + 0.5);
        assert!(terrain.water_columns(sx, sz).at(y).is_none());
        assert!(terrain.water_column(sx, sz).is_none());
        let g = SectionGenerator::new(terrain);
        let (owner, local) = terra_voxel::global_to_section([x, y, z]);
        let column = g.prepare_column([owner.x, owner.z], true).unwrap();
        assert!(
            column
                .local_surface(local[0], local[2])
                .unwrap()
                .water_columns()
                .at(y)
                .is_none()
        );
        let section = g
            .build_prepared(owner, 0, &Default::default(), &column)
            .unwrap();
        if let Some(water) = section.water {
            for tri in water.indices.chunks_exact(3) {
                let center = [0, 2].map(|axis| {
                    tri.iter()
                        .map(|i| water.positions[*i as usize][axis])
                        .sum::<f32>()
                        / 3.
                });
                assert_ne!(center.map(|v| v.floor() as usize), [local[0], local[2]]);
            }
        }
    }

    #[test]
    fn blood_forest_side_passage_meets_its_bank_without_a_vertical_pit() {
        // This hillside in the reported Test world used to have a 23-block
        // sheer drop where the end of the side passage met untouched ground.
        let mut config = terra_core::WorldConfig::default();
        config.seed.0 = 12210528271206938340;
        let terrain = TerraGenerator::new(config).unwrap();
        let site = terrain.planned_site([-3, 3]).unwrap();
        assert_eq!(site.summary.kind, StructureKind::DarkDragonLair);
        let (x, z) = (-1004, 1345);
        let surface = (terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64;
        assert!(
            site.block_at([x, surface, z])
                .is_some_and(|m| m != M::Air as u16)
        );
        assert_eq!(site.block_at([x, surface + 1, z]), Some(M::Air as u16));
    }

    #[test]
    fn lair_aprons_rejoin_the_hillside_on_both_sides_with_supported_treads() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        for region in [[-1, 3], [-1, -5], [-6, 5]] {
            let site = terrain.planned_site(region).unwrap();
            let [cx, cy, cz] = site.summary.center;
            let [fx, fz] = site.summary.dragon_entrance().unwrap();
            for d in [38, 48, 58, 68] {
                let mut previous: Option<i64> = None;
                for u in -24..=24 {
                    let [x, z] = [cx + d * fx + u * fz, cz + d * fz - u * fx];
                    let old = (terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor()
                        as i64;
                    let solid = |y| site.block_at([x, y, z]).map_or(y <= old, |id| id != 0);
                    let top = (cy - 16..=old.max(cy + 42))
                        .rev()
                        .find(|y| solid(*y))
                        .unwrap();
                    assert!(solid(top - 1), "{region:?} unsupported bank at {u}/{d}");
                    assert!(
                        previous.is_none_or(|last| (top - last).abs() <= 3),
                        "{region:?} cut at {u}/{d}: {previous:?} -> {top}"
                    );
                    previous = Some(top);
                }
            }
        }
    }

    #[test]
    fn earth_lair_mouth_meets_both_banks_without_a_sheer_cut() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let site = terrain.planned_site([-1, 3]).unwrap();
        let [cx, cy, cz] = site.summary.center;
        let [fx, fz] = site.summary.dragon_entrance().unwrap();
        let mut last: Option<i64> = None;
        for u in -16..=16 {
            let x = cx + u * fz + 34 * fx;
            let z = cz - u * fx + 34 * fz;
            let old = (terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64;
            let ground = (cy - 5..=cy + 45)
                .rev()
                .find(|&y| site.block_at([x, y, z]).map_or(y <= old, |m| m != 0))
                .unwrap();
            assert!(
                last.is_none_or(|previous| (ground - previous).abs() <= 3),
                "abrupt portal bank at lateral offset {u}: {last:?} -> {ground}"
            );
            last = Some(ground);
        }
    }

    #[test]
    fn dragon_lairs_link_generated_grottos_to_unique_ecology_cells() {
        use terra_core::WorldConfig;
        let generator = TerraGenerator::new(WorldConfig::default()).unwrap();
        let samples = [
            ([-1, 3], StructureKind::EarthDragonLair),
            ([-1, -5], StructureKind::DarkDragonLair),
            ([-6, 5], StructureKind::WaterDragonLair),
        ];
        for (region, kind) in samples {
            assert!(generator.dragon_lair_candidate(region));
            let seed = regional_seed(generator.config().seed.0, region);
            let site = generator.planned_site(region).expect("a carved lair");
            assert_eq!(site.summary.kind, kind);
            let [cx, y, cz] = site.summary.center;
            let home = [cx.div_euclid(32), cz.div_euclid(32)];
            assert_eq!(
                generator.dragon_nest(home),
                Some((kind.dragon().unwrap(), [cx, y, cz]))
            );
            assert!(generator.dragon_nest([home[0] + 1, home[1]]).is_none());
            assert_eq!(site.block_at([cx, y, cz]), Some(0));
            let center_column =
                crate::generation::sections::SectionGenerator::new(generator.clone())
                    .prepare_column([cx.div_euclid(16), cz.div_euclid(16)], true)
                    .unwrap();
            assert!(center_column.in_dragon_lair([cx, y, cz]));
            assert!(!center_column.in_dragon_lair([cx, site.summary.bounds[1][1] + 1, cz]));
            assert_eq!(site.block_at([cx, y - 1, cz]), Some(M::StrawBlock as u16));
            assert!(site.sections.len() > 8);
            let [fx, fz] = site.summary.dragon_entrance().unwrap();
            if kind == StructureKind::WaterDragonLair {
                let natural = generator.surface_height(cx as f64 + 0.5, cz as f64 + 0.5);
                assert!(natural >= y as f64 + 13. && y as f64 > generator.config().sea_level);
                for corner in site.summary.bounds {
                    assert_eq!(
                        [corner[0], corner[2]].map(|v| v.div_euclid(REGION_SIZE)),
                        region
                    );
                }
                // All generated sections, including the uphill side beyond the
                // usual village envelope, must be discoverable by both lookups.
                for (&sec, patch) in &site.sections {
                    let &(index, material) = patch.cells.first().expect("a lair section");
                    let index = usize::from(index);
                    let pos = sec
                        .global_block([index % 16, (index / 16) % 16, index / 256])
                        .unwrap();
                    assert_eq!(generator.structure_block(pos), Some(material));
                    assert!(
                        generator
                            .structures_for_column([sec.x, sec.z], 0)
                            .iter()
                            .any(|entry| entry.summary.id == site.summary.id)
                    );
                }
            }
            for v in [0, 8, 16] {
                assert!(
                    (y + 7..=y + 34).any(|h| site
                        .block_at([cx + fx * v, h, cz + fz * v])
                        .is_some_and(|id| id != 0)),
                    "nest and inner passage are sheltered from an overhead view"
                );
            }
            let straw = site
                .sections
                .values()
                .flat_map(|s| &s.cells)
                .filter(|(_, m)| *m == M::StrawBlock as u16)
                .count();
            assert!((40..400).contains(&straw));
            for ornament in [
                M::NestStraw,
                if kind == StructureKind::DarkDragonLair {
                    M::BloodBones
                } else {
                    M::BonePile
                },
            ] {
                assert!(
                    site.sections
                        .values()
                        .flat_map(|s| &s.cells)
                        .any(|(_, m)| *m == ornament as u16),
                    "missing {ornament:?} from {kind:?}"
                );
            }
            let eggs = site
                .sections
                .values()
                .flat_map(|section| &section.cells)
                .filter(|(_, m)| *m == palette(kind).egg as u16)
                .count();
            assert_eq!(eggs, (seed >> 12) as usize % 4);
            #[cfg(feature = "bevy")]
            {
                use crate::generation::sections::SectionGenerator;
                use crate::{
                    creatures::{Habitat, spawn_cell_away_from_players},
                    edits::EditJournal,
                    streaming::{ChunkStore, LoadedSection},
                };
                let mut population = terra_core::creatures::CreaturePopulation::default();
                let mut journal = EditJournal::default();
                let mut chunks = ChunkStore::default();
                let sections = SectionGenerator::new(generator.clone());
                let dragon = kind.dragon().unwrap();
                let mut size = (0..dragon.variant_count())
                    .map(|v| terra_core::creatures::dragons::occupied_size(dragon, v))
                    .max_by(|a, b| a[1].total_cmp(&b[1]))
                    .unwrap();
                if fx != 0 {
                    size.swap(0, 2);
                }
                let reach_x = (size[0] * 0.5).ceil() as i64;
                let reach_z = (size[2] * 0.5).ceil() as i64;
                let lower = terra_voxel::global_to_section([cx - reach_x, y, cz - reach_z]).0;
                let upper = terra_voxel::global_to_section([
                    cx + reach_x,
                    y + size[1].ceil() as i64,
                    cz + reach_z,
                ])
                .0;
                for (&section, patch) in &site.sections {
                    journal.initialize_structures(&patch.objects).unwrap();
                    if !(lower.x..=upper.x).contains(&section.x)
                        || !(lower.y..=upper.y).contains(&section.y)
                        || !(lower.z..=upper.z).contains(&section.z)
                    {
                        continue;
                    }
                    let column = sections
                        .prepare_column([section.x, section.z], true)
                        .unwrap();
                    let generated = sections
                        .build_prepared(section, 0, &Default::default(), &column)
                        .unwrap();
                    chunks.loaded.insert(
                        section,
                        LoadedSection {
                            data: generated.data,
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: generated.water,
                            props: vec![],
                        },
                    );
                }
                if kind == StructureKind::WaterDragonLair {
                    let springs: Vec<_> = site
                        .sections
                        .values()
                        .flat_map(|p| &p.objects)
                        .filter(|o| o.material == M::FluidWater as u16)
                        .collect();
                    assert!(springs.len() >= 3);
                    assert!(
                        springs
                            .iter()
                            .all(|o| journal.fluid(o.position).is_some_and(|cell| cell.source))
                    );
                }
                let habitat = Habitat {
                    terrain: &generator,
                    chunks: &chunks,
                    journal: &journal,
                };
                let position = [cx as f64 + 0.5, y as f64, cz as f64 + 0.5];
                assert!(
                    habitat.clear(position, size, false),
                    "largest {kind:?} lacks clearance at {position:?} with {size:?}"
                );
                let visitors = [[cx as f64 + 13.0, y as f64, cz as f64]];
                assert!(spawn_cell_away_from_players(
                    &mut population,
                    &habitat,
                    home,
                    &visitors,
                    1
                ));
                assert_eq!(population.animals.len(), 1);
                assert!(population.valid());
                assert_eq!(
                    population.animals.values().next().unwrap().kind,
                    kind.dragon().unwrap()
                );
                spawn_cell_away_from_players(&mut population, &habitat, home, &visitors, 1);
                assert_eq!(population.animals.len(), 1);
            }
        }
    }
}
