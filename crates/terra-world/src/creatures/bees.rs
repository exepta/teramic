//! Sparse forest ecology using the existing persistent creatures and section props.
use super::{Habitat, distance, random};
use crate::{
    TerraGenerator,
    edits::EditJournal,
    props::{PropId, PropKind, WorldProp},
    streaming::ChunkStore,
};
use terra_core::creatures::{
    Creature, CreatureBehavior as Behavior, CreatureKind as Kind, CreatureNeeds,
    CreaturePopulation, NeedGoal, NeedKind, bee_id,
};

fn nesting_biome(biome: crate::generation::biomes::BiomeKind) -> bool {
    use crate::generation::biomes::BiomeKind;
    matches!(
        biome,
        BiomeKind::Forest | BiomeKind::CherryForest | BiomeKind::Plains | BiomeKind::Highland
    )
}

/// Returns false only while the deterministic candidate cells are still streaming.
pub fn spawn(
    population: &mut CreaturePopulation,
    terrain: &TerraGenerator,
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    home: [i64; 2],
) -> Result<bool, String> {
    let Some(id) = bee_id(home, 0) else {
        return Ok(true);
    };
    if population.animals.values().any(|a| {
        a.kind == Kind::Bee
            && a.home == home
            && !terra_core::creatures::spawning::is_command_id(a.id)
    }) || population.animals.len() + 4 > terra_core::creatures::MAX_CREATURES
        || population.explored.len() >= terra_core::creatures::MAX_SPAWN_CELLS
    {
        return Ok(true);
    }
    let seed = random(id, terrain.config().seed.0);
    // One candidate nest at most in a 32x32 habitat (= four chunk columns).
    if !seed.is_multiple_of(4) {
        return Ok(true);
    }
    let center = [home[0] * 32 + 16, home[1] * 32 + 16];
    let biome = terrain
        .biome_at(center[0] as f64, center[1] as f64)
        .weights
        .dominant();
    let definition = terra_core::creatures::definitions::get(Kind::Bee);
    if !nesting_biome(biome) || !definition.permits_spawn_biome(super::biome_slug(biome)) {
        return Ok(true);
    }
    let mut candidate = None;
    'search: for probe in 0..64 {
        let r = random(seed, probe);
        let x = home[0] * 32 + (r & 31) as i64;
        let z = home[1] * 32 + ((r >> 8) & 31) as i64;
        let surface = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
        if !nesting_biome(surface.biome.weights.dominant())
            || !definition.permits_spawn_biome(super::biome_slug(surface.biome.weights.dominant()))
        {
            continue;
        }
        let ground = surface.height.floor() as i64;
        for dy in 3..=6 {
            let trunk = [x, ground + dy, z];
            let Some(material) = chunks
                .block_at(trunk)
                .map(|m| journal.block(trunk).unwrap_or(m))
            else {
                return Ok(false);
            };
            if !matches!(material, 6 | 10 | 36 | 134) {
                continue;
            }
            let side = ((r >> 16) % 4) as usize;
            let [dx, dz] = [[0, 1], [1, 0], [0, -1], [-1, 0]][side];
            let front = [x + dx, trunk[1], z + dz];
            if chunks
                .block_at(front)
                .map(|m| journal.block(front).unwrap_or(m))
                != Some(0)
            {
                continue;
            }
            let section = terra_voxel::global_to_section(trunk).0;
            let nest = WorldProp {
                id: PropId(random(id, 0xbee0) | (1 << 62)),
                kind: PropKind::BeeNest,
                position: [
                    x as f64 + 0.5 + dx as f64 * 0.64,
                    trunk[1] as f64 + 0.15,
                    z as f64 + 0.5 + dz as f64 * 0.64,
                ],
                rotation: side as f32 * std::f32::consts::FRAC_PI_2,
                scale: 1.0,
                variant: 128,
                section,
                removed: false,
            };
            if journal.prop_removed(section, nest.id) {
                return Ok(true);
            }
            candidate = Some(nest);
            break 'search;
        }
    }
    let Some(nest) = candidate else {
        return Ok(true);
    };
    let habitat = Habitat {
        terrain,
        chunks,
        journal,
    };
    let mut bees = Vec::new();
    let biome = super::biome_slug(
        terrain
            .biome_at(nest.position[0], nest.position[2])
            .weights
            .dominant(),
    );
    for slot in 0..definition.spawn_group_size(seed, biome, definition.group_size) {
        let Some(id) = bee_id(home, slot) else {
            continue;
        };
        let (sin, cos) = f64::from(nest.rotation).sin_cos();
        let p = [
            nest.position[0] + sin * 0.5,
            nest.position[1] + 0.20 + f64::from(slot) * 0.15,
            nest.position[2] + cos * 0.5,
        ];
        if !habitat.clear(p, Kind::Bee.size(), false) {
            continue;
        }
        bees.push(Creature {
            id,
            kind: Kind::Bee,
            home,
            position: p,
            velocity: [0.0; 3],
            yaw: nest.rotation,
            health: Kind::Bee.health(),
            behavior: Behavior::Fly,
            timer: 1.0,
            age: 0.0,
            variant: slot,
            needs: CreatureNeeds::full(Kind::Bee),
        });
    }
    if bees.is_empty() {
        return Ok(true);
    }
    if !journal.place_prop(nest).map_err(|e| e.to_string())? {
        return Ok(true);
    }
    if let Some(section) = chunks.loaded.get_mut(&nest.section) {
        section.props.push(nest);
    }
    population.explored.insert(home);
    population.ecology.nests.entry(nest.id.0).or_insert(
        terra_core::creatures::ecology::NestState {
            position: nest.position,
            nectar: 0,
            honey: 0,
            progress: 0.0,
        },
    );
    for bee in bees {
        population.ecology.bees.insert(
            bee.id,
            terra_core::creatures::ecology::BeeState {
                nest: nest.id.0,
                ..Default::default()
            },
        );
        population.animals.insert(bee.id, bee);
    }
    Ok(true)
}

pub(super) fn nearby<'a>(
    animal: &Creature,
    habitat: &'a Habitat,
) -> impl Iterator<Item = &'a WorldProp> {
    let owner = terra_voxel::global_to_section(animal.position.map(|v| v.floor() as i64)).0;
    let season = habitat
        .journal
        .clock
        .unwrap_or_default()
        .season(habitat.terrain.config().seasons);
    (-1..=1)
        .flat_map(|y| (-1..=1).flat_map(move |z| (-1..=1).map(move |x| [x, y, z])))
        .filter_map(move |d| {
            owner
                .offset(d)
                .ok()
                .and_then(|p| habitat.chunks.loaded.get(&p))
        })
        .flat_map(|s| s.props.iter())
        .filter(move |p| {
            !p.removed
                && !habitat.journal.prop_removed(p.section, p.id)
                && habitat.journal.flowers.coverage(p, season) > 0.0
                && crate::props::placement::supported(p, |p| habitat.chunks.block_at(p))
                    == Some(true)
                && !crate::props::seasonal::snow_buried(p, habitat.chunks, habitat.journal)
        })
}

pub(super) fn step(
    animal: &mut Creature,
    habitat: &Habitat,
    dt: f32,
    search: bool,
    ecology: Option<&terra_core::creatures::ecology::Ecology>,
    navigation: Option<&mut super::navigation::Navigation>,
) {
    let old = animal.position;
    if animal.behavior == Behavior::Land && animal.timer <= 0.0 && animal.needs.goal.is_some() {
        animal.behavior = Behavior::Crawl;
        animal.timer = 1.5;
    }
    if matches!(animal.behavior, Behavior::Nectar | Behavior::Land) && animal.timer > 0.0 {
        animal.velocity = [0.0; 3];
        return;
    }
    if animal.behavior == Behavior::Crawl && animal.timer > 0.0 {
        if let Some(goal) = animal.needs.goal {
            let center = goal.position.map(|v| v as f64 + 0.5);
            let axis = (0..3)
                .max_by(|&a, &b| {
                    (old[a] - center[a])
                        .abs()
                        .total_cmp(&(old[b] - center[b]).abs())
                })
                .unwrap_or(1);
            let mut p = old;
            let move_axis = if axis == 1 { 0 } else { 1 };
            p[move_axis] += f64::from(dt) * 0.12;
            let support = std::array::from_fn(|i| {
                if i == axis {
                    goal.position[i]
                } else {
                    p[i].floor() as i64
                }
            });
            if habitat.block(support).is_some_and(|m| {
                terra_voxel::MaterialId::from_raw(m).is_some_and(terra_voxel::MaterialId::is_opaque)
            }) && habitat.clear(p, Kind::Bee.size(), false)
            {
                animal.position = p;
                animal.needs.goal = Some(NeedGoal {
                    position: support,
                    ..goal
                });
                animal.velocity = std::array::from_fn(|i| ((p[i] - old[i]) / f64::from(dt)) as f32);
                return;
            }
        }
        animal.timer = 0.0;
    }
    if animal.timer <= 0.0 || animal.needs.goal.is_none() {
        if !search {
            animal.needs.goal = None;
            animal.velocity = [0.0; 3];
            return;
        }
        let seed = random(animal.id, (animal.age / 3.0) as u64);
        let flower = nearby(animal, habitat)
            .filter(|p| {
                p.kind == PropKind::Flower
                    && ecology.is_none_or(|eco| eco.nectar(p.id.0) > 0)
                    && ecology.is_none_or(|eco| {
                        !eco.bees
                            .iter()
                            .any(|(&id, b)| id != animal.id && b.flower == Some(p.id.0))
                    })
                    && distance(p.position, old) < 24.0_f64.powi(2)
                    && ecology
                        .and_then(|eco| {
                            eco.bees
                                .get(&animal.id)
                                .and_then(|b| eco.nests.get(&b.nest))
                        })
                        .is_none_or(|nest| distance(p.position, nest.position) < 100.0_f64.powi(2))
                    && distance(p.position, old) > 0.5
            })
            .min_by(|a, b| {
                distance(a.position, old)
                    .total_cmp(&distance(b.position, old))
                    .then(a.id.cmp(&b.id))
            });
        animal.needs.goal = flower.map(|p| NeedGoal {
            kind: NeedKind::Food,
            position: p.position.map(|v| v.floor() as i64),
        });
        if animal.needs.goal.is_none() {
            let nest = nearby(animal, habitat).find(|p| p.kind == PropKind::BeeNest);
            let center = nest.map_or(
                [
                    animal.home[0] as f64 * 32.0 + 16.0,
                    old[1],
                    animal.home[1] as f64 * 32.0 + 16.0,
                ],
                |p| p.position,
            );
            let p = [
                center[0] + (seed % 900) as f64 / 100.0 - 4.5,
                center[1] + 0.2 + ((seed >> 12) % 250) as f64 / 100.0,
                center[2] + ((seed >> 24) % 900) as f64 / 100.0 - 4.5,
            ];
            animal.needs.goal = Some(NeedGoal {
                kind: NeedKind::Water,
                position: p.map(|v| v.floor() as i64),
            });
        }
        animal.behavior = Behavior::Fly;
        animal.timer = 12.0;
    }
    let Some(goal) = animal.needs.goal else {
        return;
    };
    let target = if goal.kind == NeedKind::Food {
        nearby(animal, habitat)
            .find(|p| {
                p.kind == PropKind::Flower
                    && ecology.is_none_or(|eco| eco.nectar(p.id.0) > 0)
                    && ecology.is_none_or(|eco| {
                        eco.bees
                            .get(&animal.id)
                            .and_then(|b| b.flower)
                            .is_none_or(|id| id == p.id.0)
                    })
                    && p.position.map(|v| v.floor() as i64) == goal.position
            })
            .map(|p| {
                [
                    p.position[0],
                    p.position[1] + f64::from(p.visual_scale()) * 0.85,
                    p.position[2],
                ]
            })
    } else {
        Some(goal.position.map(|v| v as f64 + 0.5))
    };
    let Some(target) = target else {
        animal.needs.goal = None;
        return;
    };
    let length = distance(target, old).sqrt();
    if length < 0.10 {
        if goal.kind == NeedKind::Water && random(animal.id, animal.age as u64).is_multiple_of(5) {
            let cell = old.map(|v| v.floor() as i64);
            for d in [
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ] {
                let support = std::array::from_fn(|i| cell[i] + d[i]);
                if matches!(habitat.block(support), Some(6 | 10 | 36 | 134)) {
                    animal.behavior = Behavior::Land;
                    animal.timer = 0.3;
                    animal.needs.goal = Some(NeedGoal {
                        kind: NeedKind::Water,
                        position: support,
                    });
                    animal.velocity = [0.0; 3];
                    return;
                }
            }
        }
        animal.behavior = if goal.kind == NeedKind::Food {
            Behavior::Nectar
        } else {
            Behavior::Fly
        };
        animal.timer = if animal.behavior == Behavior::Nectar {
            2.0
        } else {
            0.0
        };
        animal.needs.goal = None;
        animal.velocity = [0.0; 3];
        return;
    }
    let proposed = target;
    if ecology
        .and_then(|eco| {
            eco.bees
                .get(&animal.id)
                .and_then(|b| eco.nests.get(&b.nest))
        })
        .is_some_and(|nest| distance(proposed, nest.position) > 100.0_f64.powi(2))
    {
        animal.needs.goal = None;
        animal.velocity = [0.0; 3];
        return;
    }
    if !super::navigation::fly_cached(animal, habitat, target, dt, navigation) {
        // A blocked flight is not a landing request. Release unreachable food
        // and retry after the bounded search cooldown instead of piling up on a trunk.
        animal.needs.goal = None;
        animal.timer = 0.0;
        animal.needs.search_in = 1.0 + (animal.id % 7) as f32 * 0.1;
    }
    animal.velocity =
        std::array::from_fn(|i| ((animal.position[i] - old[i]) / f64::from(dt)) as f32);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supported_tree_biomes_include_plains_and_highland_but_not_desert_or_ocean() {
        use crate::generation::biomes::BiomeKind;
        for biome in [BiomeKind::Forest, BiomeKind::Plains, BiomeKind::Highland] {
            assert!(nesting_biome(biome));
        }
        for biome in [BiomeKind::Desert, BiomeKind::Ocean] {
            assert!(!nesting_biome(biome));
        }
    }
    use crate::streaming::LoadedSection;
    use terra_voxel::{SectionPos, blocks::BlockSection};

    fn empty(chunks: &mut ChunkStore, p: [i64; 3]) {
        let owner = terra_voxel::global_to_section(p).0;
        chunks.loaded.entry(owner).or_insert_with(|| LoadedSection {
            data: BlockSection::empty(owner).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        });
    }
    #[test]
    fn forest_nest_uses_existing_journal_and_never_duplicates_or_rerolls_bees() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let home = (-30..=30)
            .find_map(|z| {
                (-30..=30).find_map(|x| {
                    let home = [x, z];
                    let seed = random(bee_id(home, 0).unwrap(), terrain.config().seed.0);
                    (seed.is_multiple_of(4)
                        && terrain
                            .biome_at((x * 32 + 16) as f64, (z * 32 + 16) as f64)
                            .weights
                            .dominant()
                            == crate::generation::biomes::BiomeKind::Forest)
                        .then_some(home)
                })
            })
            .unwrap();
        let seed = random(bee_id(home, 0).unwrap(), terrain.config().seed.0);
        let r = random(seed, 0);
        let x = home[0] * 32 + (r & 31) as i64;
        let z = home[1] * 32 + ((r >> 8) & 31) as i64;
        let y = terrain
            .surface_height(x as f64 + 0.5, z as f64 + 0.5)
            .floor() as i64
            + 3;
        let mut chunks = ChunkStore::default();
        for dx in -2..=2 {
            for dy in -2..=2 {
                for dz in -2..=2 {
                    empty(&mut chunks, [x + dx, y + dy, z + dz]);
                }
            }
        }
        let (owner, local) = terra_voxel::global_to_section([x, y, z]);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, 6)
            .unwrap();
        let mut journal = EditJournal::default();
        let mut pop = CreaturePopulation::default();
        let command_id = terra_core::creatures::spawning::next_command_id(&pop).unwrap();
        pop.animals.insert(
            command_id,
            Creature {
                id: command_id,
                home,
                kind: Kind::Bee,
                position: [x as f64, y as f64, z as f64],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: Kind::Bee.health(),
                behavior: Behavior::Fly,
                timer: 1.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(Kind::Bee),
            },
        );
        assert!(spawn(&mut pop, &terrain, &mut chunks, &mut journal, home).unwrap());
        assert_eq!(pop.animals.len(), 5);
        assert!(pop.animals.contains_key(&command_id));
        assert!(pop.valid());
        let nest = *chunks
            .loaded
            .values()
            .flat_map(|s| s.props.iter())
            .next()
            .unwrap();
        assert!(nest.valid() && nest.wall_attached());
        let delta = journal.section_delta(nest.section);
        assert_eq!(delta.placed_props, vec![nest]);
        assert!(spawn(&mut pop, &terrain, &mut chunks, &mut journal, home).unwrap());
        assert_eq!(pop.animals.len(), 5);
        journal.remove_prop(nest.section, nest.id).unwrap();
        let mut reloaded = Vec::new();
        journal.apply_prop_removals(nest.section, &mut reloaded);
        assert!(reloaded.is_empty());
        spawn(&mut pop, &terrain, &mut chunks, &mut journal, home).unwrap();
        assert!(journal.prop_removed(nest.section, nest.id));
    }
    #[test]
    fn nectar_is_collected_without_consuming_flower_and_unloaded_cells_are_barriers() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let owner = SectionPos::default();
        let mut chunks = ChunkStore::default();
        empty(&mut chunks, [0, 0, 0]);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([3, 0, 2], 3)
            .unwrap();
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .props
            .push(WorldProp {
                id: PropId(12),
                kind: PropKind::Flower,
                position: [3.5, 1.0, 2.5],
                rotation: 0.0,
                scale: 0.6,
                variant: 0,
                section: owner,
                removed: false,
            });
        let journal = EditJournal::default();
        let mut animal = Creature {
            id: bee_id([0, 0], 0).unwrap(),
            kind: Kind::Bee,
            home: [0, 0],
            position: [1.5, 1.8, 2.5],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 2.0,
            behavior: Behavior::Fly,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(Kind::Bee),
        };
        let mut nectar = false;
        for _ in 0..100 {
            super::super::step(
                &mut animal,
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal,
                },
                &[],
                0.1,
            );
            nectar |= animal.behavior == Behavior::Nectar;
            assert!(animal.valid());
        }
        assert!(nectar);
        assert!(!chunks.loaded[&owner].props[0].removed);
        animal.position = [15.8, 2.0, 2.5];
        animal.needs.goal = Some(NeedGoal {
            kind: NeedKind::Water,
            position: [18, 2, 2],
        });
        animal.behavior = Behavior::Fly;
        animal.timer = 5.0;
        super::super::step(
            &mut animal,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            &[],
            0.2,
        );
        assert!(animal.position[0] < 16.0);
    }
}
