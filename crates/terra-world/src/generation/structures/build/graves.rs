//! Spirit-only cemeteries and tombs, stamped into the ordinary voxel stream.
use super::*;
use crate::generation::structures::{GRAVE_LEVEL_HEIGHT, GraveVariant};

pub(super) mod adapted;

const STONE: u16 = 337;
const DARK: u16 = 338;
const MOSS: u16 = 341;
const BRICK: u16 = 343;
const COLUMN: u16 = 349;
const CAPITAL: u16 = 351;
const ARCH: u16 = 353;
const LANTERN: u16 = 369;
const CANDLES: u16 = 371;
const URN: u16 = 376;
const RADIUS: i64 = 26;

pub(super) fn plan(terrain: &TerraGenerator, region: [i64; 2], seed: u32) -> Option<Site> {
    if terrain.config().structures_revision >= 10 {
        return adapted::plan(terrain, region, seed);
    }
    if terrain.config().spirit_blossom_revision < 2
        || !terrain.config().structures.ruins
        || seed % 5 == 4
    {
        return None;
    }
    let x = region[0] * REGION_SIZE + 192 + i64::from((seed >> 7) % 33) - 16;
    let z = region[1] * REGION_SIZE + 192 + i64::from((seed >> 15) % 33) - 16;
    if terrain
        .climate_biome_at(x as f64, z as f64)
        .weights
        .spirit_blossom
        < 0.78
    {
        return None;
    }
    // Validate the whole footprint on a dense grid before any allocation. Tombs
    // never replace lakes, oceans, rivers, neighboring biomes or steep cliffs.
    let mut low = i64::MAX;
    let mut high = i64::MIN;
    for dz in (-RADIUS..=RADIUS).step_by(4) {
        for dx in (-RADIUS..=RADIUS).step_by(4) {
            let column = terrain.surface_column((x + dx) as f64 + 0.5, (z + dz) as f64 + 0.5);
            if column.biome.weights.spirit_blossom < 0.70
                || column.water_level.is_some()
                || column.river_cavity.is_some()
            {
                return None;
            }
            let ground = (column.height - 0.5).floor() as i64 + 1;
            low = low.min(ground);
            high = high.max(ground);
        }
    }
    if high - low > 14 || !(-70..=260).contains(&high) {
        return None;
    }
    let kind = if seed.is_multiple_of(4) {
        StructureKind::Cemetery
    } else {
        StructureKind::GraveDungeon
    };
    let mut builder = Builder {
        terrain,
        summary: SiteSummary {
            id: (u64::from(seed) << 32) ^ region[0] as u64 ^ (region[1] as u64).rotate_left(17),
            kind,
            style: VillageStyle::Highland,
            center: [x, high, z],
            bounds: [[0; 3]; 2],
            buildings: Vec::new(),
            containers: Vec::new(),
            grave: None,
        },
        patches: BTreeMap::new(),
        objects: BTreeMap::new(),
        writes: 0,
        failed: false,
        seed,
    };
    courtyard(&mut builder);
    if kind == StructureKind::GraveDungeon {
        dungeon(&mut builder);
    } else {
        cemetery(&mut builder);
    }
    builder.summary.buildings.push(BuildingSummary {
        role: BuildingRole::Ruin,
        entrance: [x, high, z - RADIUS],
        bounds: [
            [x - RADIUS, high - 40, z - RADIUS],
            [x + RADIUS, high + 18, z + RADIUS],
        ],
    });
    builder.finish()
}

fn courtyard(b: &mut Builder<'_>) {
    let [x, y, z] = b.summary.center;
    // Foundation slopes are filled only where needed; the raised perimeter is
    // terraced into the hill, with an open, stepped entrance toward the south.
    b.fill(
        [x - RADIUS, y, z - RADIUS],
        [x + RADIUS, y + 18, z + RADIUS],
        0,
    );
    for dz in -RADIUS..=RADIUS {
        for dx in -RADIUS..=RADIUS {
            let ground = b.ground(x + dx, z + dz);
            b.fill(
                [x + dx, ground.min(y) - 2, z + dz],
                [x + dx, y - 1, z + dz],
                if (dx * 13 + dz * 7).rem_euclid(9) == 0 {
                    MOSS
                } else {
                    STONE
                },
            );
        }
    }
    for d in -RADIUS..=RADIUS {
        for [dx, dz] in [[d, -RADIUS], [d, RADIUS], [-RADIUS, d], [RADIUS, d]] {
            if dz == -RADIUS && dx.abs() <= 3 {
                continue;
            }
            b.set([x + dx, y, z + dz], DARK);
            b.set(
                [x + dx, y + 1, z + dz],
                if d.rem_euclid(6) == 0 { STONE } else { 391 },
            );
            if d.rem_euclid(6) == 0 {
                b.set([x + dx, y + 2, z + dz], CAPITAL);
                b.set([x + dx, y + 3, z + dz], LANTERN);
            }
        }
    }
    // The approach starts with the first descending tread at courtyard height.
    // Stop at real terrain instead of descending below the lowest site sample.
    b.fill(
        [x - 2, y - 1, z - RADIUS],
        [x + 2, y - 1, z + RADIUS],
        BRICK,
    );
    for step in 0..=16 {
        let pz = z - RADIUS - step;
        let feet = y - step;
        let ground = b.ground(x, pz);
        if ground >= feet {
            if ground > y {
                b.failed = true;
                return;
            }
            level_approach_landing(b, step, ground);
            return;
        }
        for dx in -3..=3 {
            // Facing 0 rises toward +Z, back toward the courtyard.
            b.state([x + dx, feet - 1, pz], 77, 0);
            b.fill([x + dx, feet, pz], [x + dx, y + 3, pz], 0);
        }
    }
    // Do not publish a floating approach that fails to meet natural terrain.
    b.failed = true;
}

/// A rising terrain cell may meet us above the next proposed tread. Replace
/// the last treads with a level landing instead of leaving an upward block lip.
fn level_approach_landing(b: &mut Builder<'_>, last_step: i64, ground: i64) {
    let [x, y, z] = b.summary.center;
    for step in y - ground..=last_step {
        let pz = z - RADIUS - step;
        for dx in -3..=3 {
            b.set([x + dx, ground - 1, pz], BRICK);
            b.fill([x + dx, ground, pz], [x + dx, y + 3, pz], 0);
        }
    }
}

fn grave(b: &mut Builder<'_>, x: i64, y: i64, z: i64, variant: u32) {
    b.fill([x - 1, y - 1, z - 1], [x + 1, y - 1, z + 3], DARK);
    b.set([x, y, z], 362 + (variant % 3) as u16);
    b.set(
        [x, y, z + 2],
        if variant.is_multiple_of(3) { 367 } else { 366 },
    );
    b.set([x - 1, y, z], CANDLES);
    if variant.is_multiple_of(2) {
        b.set([x + 1, y, z + 3], 382);
    }
}

fn cemetery(b: &mut Builder<'_>) {
    let [x, y, z] = b.summary.center;
    for (row, dz) in [-17, -9, -1, 7, 15].into_iter().enumerate() {
        for (col, dx) in [-18, -10, 10, 18].into_iter().enumerate() {
            grave(
                b,
                x + dx,
                y,
                z + dz,
                b.seed.wrapping_add((row * 4 + col) as u32),
            );
        }
    }
    // Small roofed memorial chapel, distinct from the large dungeon entrance.
    b.fill([x - 5, y, z + 14], [x + 5, y + 6, z + 23], STONE);
    b.fill([x - 4, y, z + 15], [x + 4, y + 5, z + 22], 0);
    b.fill([x - 1, y, z + 14], [x + 1, y + 3, z + 14], 0);
    for dx in -6_i64..=6 {
        b.fill(
            [x + dx, y + 7 + (6 - dx.abs()) / 2, z + 13],
            [x + dx, y + 7 + (6 - dx.abs()) / 2, z + 24],
            DARK,
        );
    }
    b.set([x, y + 11, z + 14], 365);
    b.set([x, y, z + 20], 368);
    for dx in [-3, 3] {
        b.set([x + dx, y, z + 19], CANDLES);
    }
}

fn dungeon(b: &mut Builder<'_>) {
    let Some(info) = b.summary.grave_dungeon() else {
        return;
    };
    let [x, y, z] = b.summary.center;
    let wall = match info.variant {
        GraveVariant::Knight => BRICK,
        GraveVariant::Priestess => 340,
        GraveVariant::Scholar => STONE,
    };
    // Solid shell first, then floors/chambers. The lowest floor remains sealed
    // against terrain caves; doors and stair flights are carved last.
    let bottom = y - i64::from(info.variant.levels()) * GRAVE_LEVEL_HEIGHT;
    b.fill([x - 17, bottom - 2, z - 17], [x + 17, y - 1, z + 17], wall);
    for level in 1..=info.variant.levels() {
        let floor = y - i64::from(level) * GRAVE_LEVEL_HEIGHT;
        b.fill([x - 15, floor, z - 15], [x + 15, floor + 6, z + 15], 0);
        b.fill(
            [x - 15, floor - 1, z - 15],
            [x + 15, floor - 1, z + 15],
            DARK,
        );
        // Two chambers, connected by a wide real doorway under an arch.
        b.fill([x - 15, floor, z + 2], [x + 15, floor + 6, z + 2], wall);
        b.fill([x - 2, floor, z + 2], [x + 2, floor + 4, z + 2], 0);
        for dx in -2..=2 {
            b.set([x + dx, floor + 5, z + 2], ARCH);
        }
        for dx in [-7, 7] {
            for dz in [-11, 7, 13] {
                b.fill([x + dx, floor, z + dz], [x + dx, floor + 5, z + dz], COLUMN);
                b.set([x + dx, floor + 6, z + dz], CAPITAL);
                b.set([x + dx, floor + 3, z + dz - 1], LANTERN);
            }
        }
        b.set(
            [x, floor, z + 13],
            match info.variant {
                GraveVariant::Knight => 367,
                GraveVariant::Priestess => 368,
                GraveVariant::Scholar => 378,
            },
        );
        for dx in [-12, 12] {
            for dz in [6, 10, 14] {
                b.set(
                    [x + dx, floor, z + dz],
                    match info.variant {
                        GraveVariant::Knight => 374,
                        GraveVariant::Priestess => 376,
                        GraveVariant::Scholar => 378,
                    },
                );
                b.set([x + dx, floor, z + dz - 1], CANDLES);
            }
        }
        if info.variant == GraveVariant::Priestess {
            for dx in [-3, 3] {
                b.set([x + dx, floor, z + 13], 395);
            }
        }
        let treasure = info.treasures[usize::from(level - 1)];
        b.object(
            treasure,
            377,
            Some(BlockObjectState::default()),
            Some(super::super::loot::inventory(
                BuildingRole::Ruin,
                b.seed.wrapping_add(u32::from(level)),
            )),
        );
        b.summary.containers.push(treasure);
    }
    mausoleum(b, wall, info.variant);
    // Establish all floors and headroom first, then replace the departure cell
    // of each descending step. The stair's high end meets the preceding floor;
    // putting it in the arrival cell leaves a full-block lip at both landings.
    for p in &info.route {
        for dx in -1..=1 {
            b.set([p[0] + dx, p[1] - 1, p[2]], DARK);
            b.fill([p[0] + dx, p[1], p[2]], [p[0] + dx, p[1] + 2, p[2]], 0);
        }
    }
    for pair in info.route.windows(2) {
        if pair[1][1] < pair[0][1] {
            let p = pair[0];
            for dx in -1..=1 {
                // Facing 2 rises toward -Z; the route descends toward +Z.
                b.state([p[0] + dx, p[1] - 1, p[2]], 77, 2);
            }
        }
    }
    for p in &info.levels {
        b.set([p[0] + 3, p[1], p[2] + 3], URN);
    }
}

fn mausoleum(b: &mut Builder<'_>, wall: u16, variant: GraveVariant) {
    let [x, y, z] = b.summary.center;
    b.fill([x - 13, y, z - 14], [x + 13, y + 8, z + 14], wall);
    b.fill([x - 12, y, z - 13], [x + 12, y + 7, z + 13], 0);
    b.fill([x - 2, y, z - 14], [x + 2, y + 5, z - 14], 0);
    for dx in -3..=3 {
        b.set([x + dx, y + 6, z - 15], ARCH);
    }
    for dx in [-11, -5, 5, 11] {
        b.fill([x + dx, y, z - 15], [x + dx, y + 7, z - 15], COLUMN);
        b.set([x + dx, y + 8, z - 15], CAPITAL);
        b.set([x + dx, y + 4, z - 16], LANTERN);
    }
    for dx in -14_i64..=14 {
        let rise = (14 - dx.abs()) / 2;
        b.fill(
            [x + dx, y + 9 + rise, z - 15],
            [x + dx, y + 9 + rise, z + 15],
            DARK,
        );
        if rise > 0 {
            for edge in [-14, 14] {
                b.fill(
                    [x + dx, y + 8, z + edge],
                    [x + dx, y + 8 + rise, z + edge],
                    wall,
                );
            }
        }
    }
    b.set([x, y + 17, z - 14], 365);
    b.set(
        [x, y + 9, z - 15],
        match variant {
            GraveVariant::Knight => 356,
            GraveVariant::Priestess => 357,
            GraveVariant::Scholar => 355,
        },
    );
    for dx in [-20, 20] {
        for dz in [-18, -8, 2, 12, 21] {
            grave(b, x + dx, y, z + dz, b.seed.wrapping_add((dx + dz) as u32));
        }
    }
}

#[cfg(test)]
mod tests;
