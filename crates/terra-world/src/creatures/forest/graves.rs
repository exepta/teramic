use super::*;
use terra_core::creatures::{CreatureNeeds, CreaturePopulation, skeleton_id};

/// Night visitors share persistent grave slots with crypt guards. Already
/// explored daytime habitats are revisited by the bounded night scan.
pub fn spawn(
    population: &mut CreaturePopulation,
    h: &Habitat,
    home: [i64; 2],
    player_y: f64,
    night: bool,
    available: usize,
) -> usize {
    let species = definitions::get(Kind::Skeleton);
    if !night {
        return 0;
    }
    if available == 0 || population.animals.len() >= terra_core::creatures::MAX_CREATURES {
        return 0;
    }
    let x = (home[0] * 32 + 16) as f64;
    let z = (home[1] * 32 + 16) as f64;
    let biome = super::super::biome_slug(h.terrain.surface_biome(&h.terrain.surface_column(x, z)));
    if !matches!(biome, "ashlands" | "spirit_blossom" | "blood_forest")
        || !species.permits_spawn_biome(biome)
    {
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
    for slot in 0..species.spawn_group_size(seed, biome, 2).min(2) {
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
        'section: for section in sections
            .iter()
            .filter(|_| biome == "spirit_blossom")
            .take(4)
        {
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
        // Surface slots are independent of daytime herbivore exploration. Probe only
        // installed ground near this player, accounting for excavation and dry banks.
        if candidate.is_none() && (seed >> 20).is_multiple_of(4) {
            for probe in 0..6 {
                let hash = random(id, probe + 230);
                let x = (home[0] * 32 + 4 + (hash % 24) as i64) as f64 + 0.5;
                let z = (home[1] * 32 + 4 + ((hash >> 8) % 24) as i64) as f64 + 0.5;
                let surface = h.terrain.surface_column(x, z);
                let ground = surface.height.floor() + 1.;
                if (ground - player_y).abs() > 28.
                    || super::super::biome_slug(h.terrain.surface_biome(&surface)) != biome
                {
                    continue;
                }
                if let Some(p) = h.ground([x, ground, z], Kind::Skeleton)
                    && !h.wet(p)
                    && !super::super::lava_worm::is_lava(h, p)
                    && population
                        .animals
                        .values()
                        .all(|a| a.health <= 0. || distance(a.position, p) > 4.)
                {
                    candidate = Some(p);
                    break;
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
                    behavior: Behavior::Emerge,
                    timer: terra_core::creatures::SKELETON_RISE_SECONDS,
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
