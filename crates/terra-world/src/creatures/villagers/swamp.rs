//! Discover hut inhabitants once their actual floor and headroom have streamed in.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use terra_core::creatures::villagers::swamp_id;

type HutFloor = ([i64; 3], u8);

#[derive(Default)]
pub(super) struct HutResidents {
    tiles: BTreeMap<[i64; 2], Vec<HutFloor>>,
}

impl HutResidents {
    pub(super) fn discover(
        &mut self,
        population: &mut CreaturePopulation,
        habitat: &Habitat,
        players: &[[f64; 3]],
    ) {
        if habitat.terrain.config().ecology_revision < 18 {
            return;
        }
        let mut nearby = BTreeSet::new();
        for p in players {
            let tile = [p[0], p[2]].map(|c| (c.floor() as i64).div_euclid(32));
            for x in tile[0] - 2..=tile[0] + 2 {
                for z in tile[1] - 2..=tile[1] + 2 {
                    nearby.insert([x, z]);
                }
            }
        }
        self.tiles.retain(|tile, _| nearby.contains(tile));
        for tile in nearby {
            let positions = self
                .tiles
                .entry(tile)
                .or_insert_with(|| habitat.terrain.swamp_residents(tile));
            for &(floor, role) in positions.iter() {
                let Some(id) = swamp_id(tile, role) else {
                    continue;
                };
                if population.animals.contains_key(&id)
                    || population.animals.len() >= terra_core::creatures::MAX_CREATURES
                {
                    continue;
                }
                let position = [
                    floor[0] as f64 + 0.5,
                    floor[1] as f64,
                    floor[2] as f64 + 0.5,
                ];
                let Some(position) = habitat
                    .ground(position, Kind::Villager)
                    .filter(|p| (p[1] - position[1]).abs() < 0.55)
                else {
                    continue;
                };
                population.animals.insert(
                    id,
                    Creature {
                        id,
                        kind: Kind::Villager,
                        home: tile,
                        position,
                        velocity: [0.; 3],
                        yaw: 0.,
                        health: Kind::Villager.health(),
                        behavior: Behavior::Rest,
                        timer: 1.,
                        age: 0.,
                        variant: role,
                        needs: CreatureNeeds::full(Kind::Villager),
                    },
                );
            }
        }
    }

    pub(super) fn anchor(&self, resident: &Creature) -> Option<[f64; 3]> {
        self.tiles
            .get(&resident.home)?
            .iter()
            .find_map(|&(p, role)| {
                (swamp_id(resident.home, role) == Some(resident.id)).then_some([
                    p[0] as f64 + 0.5,
                    p[1] as f64,
                    p[2] as f64 + 0.5,
                ])
            })
    }
}

pub(super) fn step(
    r: &mut Creature,
    h: &Habitat,
    anchor: [f64; 3],
    dt: f32,
    search: bool,
    nav: &mut navigation::Navigation,
) {
    // Keep walks short on elevated boardwalks; destinations must lie on the hut's level.
    let cycle = ((r.age + (r.id % 17) as f64) / 7.) as usize;
    let offset = [[0., 0.], [0., 1.], [1., 1.], [1., 0.], [0., 0.], [-1., 0.]][cycle % 6];
    let target = [anchor[0] + offset[0], anchor[1], anchor[2] + offset[1]];
    let dry_deck = |p: [f64; 3]| {
        (p[1] - anchor[1]).abs() < 0.6 && !h.wet_footprint(p, Kind::Villager.size(), 0.)
    };
    if let Some(target) = h.ground(target, Kind::Villager).filter(|p| dry_deck(*p))
        && !navigation::ground_arrived(r, h, target, 0.32)
    {
        walk_with_access(r, h, target, dt, search, false, nav, dry_deck);
    } else {
        r.velocity[0] = 0.;
        r.velocity[2] = 0.;
        r.behavior = Behavior::Rest;
        support::settle(r, h, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SectionGenerator, TerraGenerator, creatures::test_habitat::load_surface, edits::EditJournal,
    };
    use terra_core::{
        WorldConfig,
        creatures::villagers::{SWAMP_HERMIT, SWAMP_WARLOCK, SWAMP_WITCH},
    };
    use terra_voxel::MaterialId;

    #[test]
    fn real_huts_have_their_own_residents_and_keep_the_same_saved_identities() {
        let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        let mut residents = super::super::Residents::default();
        let mut population = CreaturePopulation::default();
        for (tile, roles) in [
            ([164, -229], &[SWAMP_WITCH, SWAMP_WARLOCK][..]),
            ([6, -354], &[SWAMP_HERMIT][..]),
        ] {
            let journal = EditJournal::default();
            let chunks = load_surface(&generator, tile);
            let h = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            let floors = terrain.swamp_residents(tile);
            assert_eq!(floors.len(), roles.len());
            let player = floors[0].0.map(|v| v as f64 + 0.5);
            residents.discover(&mut population, &h, &[player]);
            for &role in roles {
                let id = swamp_id(tile, role).unwrap();
                let r = &mut population
                    .animals
                    .get_mut(&id)
                    .expect("resident at the hut");
                assert!(r.valid());
                assert_eq!(r.variant, role);
                let [x, y, z] = r.position.map(|v| v.floor() as i64);
                assert_eq!(
                    h.block([x, y - 1, z]),
                    Some(MaterialId::WillowPlanks as u16)
                );
                r.health = 0.;
                residents.discover(&mut population, &h, &[player]);
                assert_eq!(population.animals[&id].health, 0.);
            }
        }
        assert_eq!(population.animals.len(), 3);
    }
}
