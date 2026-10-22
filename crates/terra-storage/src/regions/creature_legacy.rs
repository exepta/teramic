use super::*;
use terra_core::creatures::*;

#[derive(Serialize, Deserialize)]
pub(super) struct PreviousCreature {
    pub id: u64,
    pub kind: CreatureKind,
    pub home: [i64; 2],
    pub position: [f64; 3],
    pub velocity: [f32; 3],
    pub yaw: f32,
    pub health: f32,
    pub behavior: CreatureBehavior,
    pub timer: f32,
    pub age: f64,
    pub variant: u8,
}
#[derive(Serialize, Deserialize)]
pub(super) struct PreviousPopulation {
    pub animals: BTreeMap<u64, PreviousCreature>,
    pub explored: BTreeSet<[i64; 2]>,
}
#[derive(Serialize, Deserialize)]
pub(super) struct PreviousCreatureRegion {
    pub region: Region,
    pub creatures: PreviousPopulation,
}

pub(super) fn migrate(data: PreviousCreatureRegion) -> Result<CreatureRegion, String> {
    if data.creatures.animals.len() > MAX_CREATURES
        || data.creatures.explored.len() > MAX_SPAWN_CELLS
    {
        return Err("Legacy creature population exceeds its budget".into());
    }
    let mut population = CreaturePopulation::default();
    for (id, old) in data.creatures.animals {
        let max = match old.kind {
            CreatureKind::Cow => 20.0,
            CreatureKind::Sheep | CreatureKind::Pig => 12.0,
            CreatureKind::Fish => 4.0,
            CreatureKind::Bee => 2.0,
            _ => {
                return Err("New species is not valid in a legacy creature region".into());
            }
        };
        if !old.health.is_finite()
            || !(0.0..=max).contains(&old.health)
            || !data.creatures.explored.contains(&old.home)
        {
            return Err("Invalid legacy creature state".into());
        }
        let animal = Creature {
            id: old.id,
            kind: old.kind,
            home: old.home,
            position: old.position,
            velocity: old.velocity,
            yaw: old.yaw,
            health: old.health / max * old.kind.health(),
            behavior: old.behavior,
            timer: old.timer,
            age: old.age,
            variant: old.variant,
            needs: CreatureNeeds::full(old.kind),
        };
        if id != animal.id || !animal.valid() {
            return Err("Invalid legacy creature identity".into());
        }
        population.explored.insert(animal.home);
        population.animals.insert(id, animal);
    }
    // Revisit previously empty habitats once under the denser herd rules. Homes
    // with any recorded creature, including deaths, retain their spawn tombstones.
    Ok(CreatureRegion {
        region: data.region,
        creatures: population,
    })
}
