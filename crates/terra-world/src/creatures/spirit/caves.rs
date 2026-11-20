//! Persistent cave Glooms inspect loaded rock/ruin floors only, never worldgen.
use super::super::{Behavior, Creature, CreaturePopulation, Habitat, Kind, distance, random};
use terra_core::creatures::{CreatureNeeds, gloom, gloom_id};

pub fn spawn(
    population: &mut CreaturePopulation,
    h: &Habitat,
    home: [i64; 2],
    player_y: f64,
    available: usize,
) -> usize {
    let Some(id) = gloom_id(home) else {
        return 0;
    };
    let seed = random(id, h.terrain.config().seed.0);
    if available == 0
        || population.animals.len() >= terra_core::creatures::MAX_CREATURES
        || population.animals.contains_key(&id)
        || !terra_core::creatures::definitions::get(Kind::SpiritGloom).permits_cave_spawn(seed, 4)
    {
        return 0;
    }
    let mut sections: Vec<_> = h
        .chunks
        .loaded
        .keys()
        .filter(|p| p.x.div_euclid(2) == home[0] && p.z.div_euclid(2) == home[1])
        .copied()
        .collect();
    sections.sort_unstable_by(|a, b| {
        ((a.y * 16) as f64 - player_y)
            .abs()
            .total_cmp(&((b.y * 16) as f64 - player_y).abs())
            .then(a.cmp(b))
    });
    for section in sections.into_iter().take(4) {
        for dy in 0..16 {
            for probe in 0..4 {
                let hash = random(id, probe);
                let cell = [
                    section.x * 16 + 2 + (hash % 12) as i64,
                    section.y * 16 + dy,
                    section.z * 16 + 2 + ((hash >> 8) % 12) as i64,
                ];
                let Some(raw) = h.block(cell) else { continue };
                let floor = terra_voxel::MaterialId::from_raw(raw)
                    .is_some_and(|m| m.is_natural_ground())
                    || (337..=343).contains(&raw);
                if !floor {
                    continue;
                }
                let position = [
                    cell[0] as f64 + 0.5,
                    cell[1] as f64 + 1.4,
                    cell[2] as f64 + 0.5,
                ];
                let surface = h.terrain.surface_height(position[0], position[2]);
                let depth = surface - position[1];
                if depth < 8.0
                    || !h.clear(position, Kind::SpiritGloom.size(), false)
                    || h.wet(position)
                    || !super::super::navigation::caves::covered(h, position, 0.8)
                    || population
                        .animals
                        .values()
                        .any(|a| a.health > 0.0 && distance(a.position, position) < 9.0)
                {
                    continue;
                }
                population.animals.insert(
                    id,
                    Creature {
                        id,
                        kind: Kind::SpiritGloom,
                        home,
                        position,
                        velocity: [0.0; 3],
                        yaw: 0.0,
                        health: Kind::SpiritGloom.health(),
                        behavior: Behavior::Fly,
                        timer: 2.0,
                        age: 0.0,
                        variant: gloom::cave_variant(depth, seed >> 8),
                        needs: CreatureNeeds::full(Kind::SpiritGloom),
                    },
                );
                return 1;
            }
        }
    }
    0
}

#[cfg(test)]
#[path = "caves_tests.rs"]
mod tests;
