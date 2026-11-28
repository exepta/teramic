use super::{Behavior, Creature, Habitat, Kind, distance, navigation, random};
use terra_core::creatures::{CreatureNeeds, definitions, intentions::IntentReason, poultry_id};
use terra_voxel::MaterialId;

const MIN_HENS: usize = 3;
const MAX_HENS: usize = 8;
pub const FLOCK_RADIUS: f64 = 12.0;

fn flock_biome(biome: &str) -> bool {
    matches!(biome, "plains" | "forest" | "highland")
}

pub(crate) fn natural_flock(
    habitat: &Habitat,
    home: [i64; 2],
    available: usize,
) -> Result<Option<Vec<Creature>>, ()> {
    let seed = random(
        poultry_id(home, 0).ok_or(())?,
        habitat.terrain.config().seed.0,
    );
    if available <= MIN_HENS || !seed.is_multiple_of(7) {
        return Ok(None);
    }
    let x = (home[0] * 32 + 11 + ((seed >> 8) % 10) as i64) as f64 + 0.5;
    let z = (home[1] * 32 + 11 + ((seed >> 16) % 10) as i64) as f64 + 0.5;
    let surface = habitat.terrain.surface_column(x, z);
    if !flock_biome(super::biome_slug(habitat.terrain.surface_biome(&surface))) {
        return Ok(None);
    }
    let hens =
        (MIN_HENS + ((seed >> 24) % (MAX_HENS - MIN_HENS + 1) as u64) as usize).min(available - 1);
    let mut flock = Vec::with_capacity(1 + hens);
    let slots = [
        [0, 0],
        [0, -2],
        [2, -2],
        [2, 0],
        [2, 2],
        [0, 2],
        [-2, 2],
        [-2, 0],
        [-2, -2],
        [-4, 0],
        [4, 0],
        [0, 4],
        [0, -4],
    ];
    for [dx, dz] in slots {
        if flock.len() > hens {
            break;
        }
        let slot = flock.len() as u8;
        let kind = if slot == 0 { Kind::Rooster } else { Kind::Hen };
        let id = poultry_id(home, slot).expect("validated poultry habitat");
        let variation = random(id, seed);
        let x = x + f64::from(dx);
        let z = z + f64::from(dz);
        let surface = habitat.terrain.surface_column(x, z);
        let biome = super::biome_slug(habitat.terrain.surface_biome(&surface));
        if !flock_biome(biome) {
            continue;
        }
        let feet = surface.height.floor() + 1.0;
        let yaw = (variation % 628) as f32 * 0.01;
        if !super::spawn_volume_loaded(habitat, [x, feet - 2.0, z], [1.5, 4.0, 1.5]) {
            return Err(());
        }
        let Some(position) = habitat.ground_rotated([x, feet, z], kind, yaw.into()) else {
            continue;
        };
        let soil = [
            x.floor() as i64,
            position[1].floor() as i64 - 1,
            z.floor() as i64,
        ];
        if !forage_ground(habitat, soil)
            && !(biome == "highland"
                && habitat
                    .block(soil)
                    .and_then(MaterialId::from_raw)
                    .is_some_and(|m| {
                        matches!(m, MaterialId::HighlandCliffStone | MaterialId::Stone)
                    }))
        {
            continue;
        }
        flock.push(Creature {
            id,
            kind,
            home,
            position,
            yaw,
            velocity: [0.0; 3],
            health: kind.health(),
            behavior: Behavior::Rest,
            timer: (variation % 30) as f32 * 0.1,
            age: 0.0,
            variant: (variation % u64::from(kind.variant_count())) as u8,
            needs: CreatureNeeds::full(kind),
        });
    }
    Ok((flock.len() > MIN_HENS).then_some(flock))
}

#[cfg(test)]
#[test]
fn natural_flocks_prefer_plains_forest_and_highlands() {
    for biome in terra_core::BiomeKind::ALL {
        let slug = super::biome_slug(biome);
        assert_eq!(
            flock_biome(slug),
            matches!(
                biome,
                terra_core::BiomeKind::Plains
                    | terra_core::BiomeKind::Forest
                    | terra_core::BiomeKind::Highland
            )
        );
    }
}

pub(super) fn forage_ground(habitat: &Habitat, ground: [i64; 3]) -> bool {
    matches!(
        habitat.block(ground).and_then(MaterialId::from_raw),
        Some(
            MaterialId::Grass
                | MaterialId::Dirt
                | MaterialId::SavannaTurf
                | MaterialId::DrySavannaTurf
        )
    )
}

pub(super) fn startle_radius(animal: &Creature) -> f64 {
    match (
        animal.kind,
        animal.behavior == Behavior::Flee && animal.timer > 0.0,
    ) {
        (Kind::Hen, false) => 3.6,
        (Kind::Hen, true) => 5.0,
        (_, true) => 2.5,
        (_, false) => 0.95,
    }
}

fn chest(a: &Creature) -> [f64; 3] {
    [
        a.position[0],
        a.position[1] + a.size()[1] * 0.6,
        a.position[2],
    ]
}

pub fn follow(
    bird: &mut Creature,
    leader: [f64; 3],
    habitat: &Habitat,
    dt: f32,
    search: bool,
    nav: &mut navigation::Navigation,
) -> bool {
    let separation: f64 = if bird.kind == Kind::Hen { 4.5 } else { 6.0 };
    if bird.health <= 0.0
        || !matches!(
            bird.behavior,
            Behavior::Rest | Behavior::Wander | Behavior::Graze
        )
        || bird.needs.goal.is_some()
        || distance(bird.position, leader) <= separation.powi(2)
        || habitat
            .supported_sized(bird.position, bird.size(), bird.yaw.into())
            .is_none()
    {
        return false;
    }
    bird.age += f64::from(dt);
    bird.behavior = Behavior::Wander;
    bird.timer = 1.0;
    super::needs::update(bird, habitat, dt, search);
    if bird.health > 0.0 {
        nav.intend(bird, IntentReason::Explore, Some(leader));
        super::forest::travel(bird, habitat, leader, dt, search, Some(nav));
    }
    true
}

pub fn ready_to_strike(rooster: &Creature, target: [f64; 3], habitat: &Habitat) -> bool {
    definitions::get(Kind::Rooster)
        .combat
        .is_some_and(|combat| {
            rooster.kind == Kind::Rooster
                && rooster.health > 0.0
                && distance(chest(rooster), target) <= combat.reach.powi(2)
                && super::perception::in_melee_arc(rooster.yaw, rooster.position, target)
                && super::perception::visible(habitat, chest(rooster), target)
        })
}

/// Return damage only at the configured impact, after turning and checking cover.
pub fn defend(
    rooster: &mut Creature,
    habitat: &Habitat,
    target: [f64; 3],
    same_target: bool,
    dt: f32,
    search: bool,
    nav: &mut navigation::Navigation,
) -> Option<f32> {
    if rooster.kind != Kind::Rooster
        || rooster.health <= 0.0
        || !(0.0 < dt && dt <= 0.25)
        || habitat
            .block(rooster.position.map(|v| v.floor() as i64))
            .is_none()
    {
        return None;
    }
    let combat = definitions::get(rooster.kind).combat?;
    rooster.age += f64::from(dt);
    let previous = rooster.timer;
    rooster.timer = (previous - dt).max(0.0);
    rooster.needs.goal = None;
    super::needs::update(rooster, habitat, dt, false);
    if rooster.health <= 0.0 {
        return None;
    }
    if super::support::swimming(rooster, habitat) {
        super::support::swim_toward(rooster, habitat, target, dt, rooster.kind.speed());
        return None;
    }
    if habitat
        .supported_sized(rooster.position, rooster.size(), rooster.yaw.into())
        .is_none()
    {
        super::support::settle(rooster, habitat, dt);
        return None;
    }
    let yaw = (rooster.position[0] - target[0]).atan2(rooster.position[2] - target[2]);
    if habitat.clear_swimmer_rotated(rooster.position, rooster.size(), yaw) {
        rooster.yaw = (yaw as f32).rem_euclid(std::f32::consts::TAU);
    }
    if ready_to_strike(rooster, target, habitat) {
        let entering = !same_target || rooster.behavior != Behavior::Attack;
        rooster.behavior = Behavior::Attack;
        rooster.velocity = [0.0; 3];
        nav.intend(rooster, IntentReason::Attack, Some(target));
        return combat
            .melee_contact(entering, previous, &mut rooster.timer)
            .then_some(combat.damage);
    }
    rooster.behavior = Behavior::Hunt;
    nav.intend(rooster, IntentReason::Hunt, Some(target));
    super::forest::travel(rooster, habitat, target, dt, search, Some(nav));
    None
}

pub fn alert_hen(hen: &mut Creature, habitat: &Habitat, attacker: [f64; 3]) -> bool {
    if hen.health <= 0.0
        || distance(chest(hen), attacker) > 6.0_f64.powi(2)
        || !super::perception::visible(habitat, chest(hen), attacker)
    {
        return false;
    }
    let yaw = (attacker[0] - hen.position[0]).atan2(attacker[2] - hen.position[2]);
    if habitat.clear_swimmer_rotated(hen.position, hen.size(), yaw) {
        hen.yaw = (yaw as f32).rem_euclid(std::f32::consts::TAU);
    }
    hen.behavior = Behavior::Flee;
    hen.timer = 2.5;
    true
}
