use super::*;
use terra_core::creatures::{CreatureNeeds, CreaturePopulation, skeleton_id};

/// Crypt-floor encounters only inspect loaded, edited cells. Stable separate IDs
/// prevent replacing surface fauna or respawning defeated guards on a reload.
pub fn spawn(
    population: &mut CreaturePopulation,
    h: &Habitat,
    home: [i64; 2],
    player_y: f64,
    available: usize,
) -> usize {
    let species = definitions::get(Kind::Skeleton);
    if !species.permits_spawn_biome("spirit_blossom") {
        return 0;
    }
    if available == 0 || population.animals.len() >= terra_core::creatures::MAX_CREATURES {
        return 0;
    }
    let x = (home[0] * 32 + 16) as f64;
    let z = (home[1] * 32 + 16) as f64;
    if super::super::biome_slug(h.terrain.biome_at(x, z).weights.dominant()) != "spirit_blossom" {
        return 0;
    }
    let Some(first_id) = skeleton_id(home, 0) else {
        return 0;
    };
    let seed = random(first_id, h.terrain.config().seed.0);
    if seed.is_multiple_of(3) {
        return 0;
    }
    let mut sections: Vec<_> = h
        .chunks
        .loaded
        .keys()
        .filter(|p| p.x.div_euclid(2) == home[0] && p.z.div_euclid(2) == home[1])
        .copied()
        .collect();
    sections.sort_by(|a, b| {
        ((a.y * 16) as f64 - player_y)
            .abs()
            .total_cmp(&((b.y * 16) as f64 - player_y).abs())
            .then(a.cmp(b))
    });
    let mut spawned = 0;
    for slot in 0..species.spawn_group_size(seed, "spirit_blossom", 2).min(2) {
        if spawned >= available || population.animals.len() >= terra_core::creatures::MAX_CREATURES
        {
            break;
        }
        let Some(id) = skeleton_id(home, slot) else {
            break;
        };
        if population.animals.contains_key(&id) {
            continue;
        }
        let mut candidate = None;
        'section: for section in sections.iter().take(4) {
            for dy in 0..16 {
                for probe in 0..12 {
                    let hash = random(id, probe);
                    let cell = [
                        section.x * 16 + 2 + (hash % 12) as i64,
                        section.y * 16 + dy,
                        section.z * 16 + 2 + ((hash >> 8) % 12) as i64,
                    ];
                    if !matches!(h.block(cell), Some(337..=343)) {
                        continue;
                    }
                    let p = [
                        cell[0] as f64 + 0.5,
                        cell[1] as f64 + 1.0,
                        cell[2] as f64 + 0.5,
                    ];
                    if h.clear(p, Kind::Skeleton.size(), false)
                        && !h.wet(p)
                        && population
                            .animals
                            .values()
                            .all(|a| a.health <= 0.0 || distance(a.position, p) > 4.0)
                    {
                        candidate = Some(p);
                        break 'section;
                    }
                }
            }
        }
        if let Some(position) = candidate {
            population.animals.insert(
                id,
                Creature {
                    id,
                    kind: Kind::Skeleton,
                    home,
                    position,
                    velocity: [0.0; 3],
                    yaw: 0.0,
                    health: Kind::Skeleton.health(),
                    behavior: Behavior::Rest,
                    timer: 1.5,
                    age: 0.0,
                    variant: (random(id, seed) % 3) as u8,
                    needs: CreatureNeeds::full(Kind::Skeleton),
                },
            );
            spawned += 1;
        }
    }
    spawned
}
