//! Deterministic dry-land selection without creating islands or changing saves.
use crate::{SectionGenerator, TerraGenerator, edits::EditJournal, water::fluid_cell_at};
use terra_core::BiomeKind;
use terra_voxel::MaterialId;

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod tests;

// Stay inside the network/character precision budget, not just the noise budget.
const LIMIT: i64 = 999_000;
const SEARCH_STEPS: [i64; 5] = [1, 16, 128, 1024, 8192];
const SEARCH_RINGS: i64 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct WorldSpawn {
    pub ground: [i64; 3],
}

impl WorldSpawn {
    /// Feet are centered over one full logical cell. Physics adds its own height.
    pub fn feet(self) -> [f64; 3] {
        [
            self.ground[0] as f64 + 0.5,
            self.ground[1] as f64 + 1.0,
            self.ground[2] as f64 + 0.5,
        ]
    }

    pub fn is_valid(&self, terrain: &TerraGenerator, journal: &EditJournal) -> bool {
        valid(*self, &SectionGenerator::new(terrain.clone()), journal)
    }
}

/// Search nearby cells first, then wider rings. At most 5441 candidate columns
/// are examined. An optional saved biome preference filters this same bounded
/// search; it never changes terrain or the validity of saved player positions.
/// A world without suitable dry land fails instead of spawning in water or
/// silently building a platform under the player.
pub fn find_land_spawn(
    terrain: &TerraGenerator,
    journal: &EditJournal,
    origin: [i64; 2],
) -> Result<WorldSpawn, String> {
    let preferred = terrain.config().preferred_spawn_biome;
    let failure = || match preferred {
        Some(biome) => format!(
            "No safe spawn found for the selected biome ({biome:?}). Try another seed or biome."
        ),
        None => "No safe dry-land spawn found. Check world water/terrain settings.".to_string(),
    };
    if origin.iter().any(|v| !(-LIMIT..=LIMIT).contains(v)) {
        return Err(failure());
    }
    let config = terrain.config();
    if preferred.is_some_and(|biome| {
        (!config.biomes && biome != BiomeKind::Forest)
            || (biome == BiomeKind::Ocean && !config.water)
    }) {
        return Err(failure());
    }
    if config.water
        && config.hill_amplitude == 0.0
        && config.mountain_amplitude == 0.0
        && (config.base_height - 0.5).floor() + 1.0 < config.sea_level
    {
        return Err(failure());
    }
    let generator = SectionGenerator::new(terrain.clone());
    let candidate = |x: i64, z: i64| {
        if !terrain.settlement_start_clear([x, z]) {
            return None;
        }
        let ground = surface_ground(terrain, x, z)?;
        if preferred.is_some_and(|biome| !matches_preferred_biome(terrain, x, z, biome)) {
            return None;
        }
        let spawn = WorldSpawn { ground };
        valid(spawn, &generator, journal).then_some(spawn)
    };
    if let Some(spawn) = candidate(origin[0], origin[1]) {
        return Ok(spawn);
    }
    for step in SEARCH_STEPS {
        for ring in 1..=SEARCH_RINGS {
            for offset in -ring..=ring {
                for [dx, dz] in [[offset, -ring], [offset, ring]] {
                    if let Some(spawn) = candidate(origin[0] + dx * step, origin[1] + dz * step) {
                        return Ok(spawn);
                    }
                }
            }
            for offset in (-ring + 1)..ring {
                for [dx, dz] in [[-ring, offset], [ring, offset]] {
                    if let Some(spawn) = candidate(origin[0] + dx * step, origin[1] + dz * step) {
                        return Ok(spawn);
                    }
                }
            }
        }
    }
    Err(failure())
}

/// Dry land uses the same visible biome policy as SurfaceColumn: marine
/// climate alone does not turn a dry island into an ocean floor. The Ocean
/// preference therefore means a safe shore within 64 m of real marine water.
fn matches_preferred_biome(terrain: &TerraGenerator, x: i64, z: i64, preferred: BiomeKind) -> bool {
    if preferred == BiomeKind::Ocean {
        for distance in [8, 32, 64] {
            for direction in [
                [-1, -1],
                [0, -1],
                [1, -1],
                [-1, 0],
                [1, 0],
                [-1, 1],
                [0, 1],
                [1, 1],
            ] {
                let point = [x + direction[0] * distance, z + direction[1] * distance];
                if point.iter().any(|v| !(-LIMIT..=LIMIT).contains(v)) {
                    continue;
                }
                let [sx, sz] = point.map(|value| value as f64 + 0.5);
                if terrain.biome_at(sx, sz).weights.ocean > 0.45
                    && terrain.water_column(sx, sz).is_some_and(|water| {
                        (water.level - terrain.config().sea_level).abs() < 0.01
                            && water.bed < water.level
                            && terrain.surface_height(sx, sz) < water.level
                    })
                {
                    return true;
                }
            }
        }
        return false;
    }
    let mut weights = terrain.biome_at(x as f64 + 0.5, z as f64 + 0.5).weights;
    weights.ocean = 0.0;
    let visible = if weights.values().iter().all(|value| *value <= f64::EPSILON) {
        BiomeKind::Plains
    } else {
        weights.dominant()
    };
    visible == preferred
}

fn surface_ground(terrain: &TerraGenerator, x: i64, z: i64) -> Option<[i64; 3]> {
    if !(-LIMIT..=LIMIT).contains(&x) || !(-LIMIT..=LIMIT).contains(&z) {
        return None;
    }
    let sx = x as f64 + 0.5;
    let sz = z as f64 + 0.5;
    // Match SectionGenerator's center-sampled voxel surface, not float height.
    let y = (terrain.surface_height(sx, sz) - 0.5).floor() as i64;
    if !(-LIMIT..=LIMIT).contains(&y)
        || terrain
            .water_column(sx, sz)
            .is_some_and(|w| w.level > (y + 1) as f64)
    {
        return None;
    }
    Some([x, y, z])
}

fn valid(spawn: WorldSpawn, generator: &SectionGenerator, journal: &EditJournal) -> bool {
    if spawn.ground.iter().any(|v| !(-LIMIT..=LIMIT).contains(v)) {
        return false;
    }
    let terrain = generator.terrain();
    let block = |p| journal.block(p).or_else(|| generator.block_at(p).ok());
    let [x, y, z] = spawn.ground;
    if surface_ground(terrain, x, z) != Some(spawn.ground) {
        return false;
    }
    // A small walkable shore margin avoids single-cell ledges and steep banks.
    for dz in -1..=1 {
        for dx in -1..=1 {
            let Some(ground) = surface_ground(terrain, x + dx, z + dz) else {
                return false;
            };
            if (ground[1] - y).abs() > 1
                || !block(ground)
                    .and_then(MaterialId::from_raw)
                    .is_some_and(MaterialId::is_natural_ground)
                || !block([ground[0], ground[1] - 1, ground[2]])
                    .and_then(MaterialId::from_raw)
                    .is_some_and(MaterialId::is_natural_ground)
            {
                return false;
            }
            let above = [ground[0], ground[1] + 1, ground[2]];
            if fluid_cell_at(terrain, journal, above, block(above).unwrap_or(u16::MAX)).amount > 0 {
                return false;
            }
        }
    }
    // Actual generated wood/leaves, edited blocks and saved fluids all matter.
    for height in 1..=3 {
        let position = [x, y + height, z];
        if block(position) != Some(0) || fluid_cell_at(terrain, journal, position, 0).amount > 0 {
            return false;
        }
    }
    true
}
