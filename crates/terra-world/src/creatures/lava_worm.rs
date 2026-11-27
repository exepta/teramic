//! The worm patrols the connected free surface of loaded lava and dives to turn in shallows.
use super::{Behavior as B, Creature, Habitat, Kind, bears::Effect, distance, perception};
use terra_core::creatures::{NeedGoal, NeedKind, definitions, lava_worm as pose};
use terra_voxel::MaterialId;

pub(super) fn is_lava(h: &Habitat, p: [f64; 3]) -> bool {
    h.block(p.map(|v| v.floor() as i64)) == Some(MaterialId::Lava as u16)
}

fn open_pool_cell(h: &Habitat, x: i64, y: i64, z: i64) -> bool {
    h.block([x, y, z]) == Some(MaterialId::Lava as u16)
        && crate::lava::level(h.journal, [x, y, z]) >= 0.85
        && h.block([x, y + 1, z]) == Some(0)
}

pub(super) fn anchor(h: &Habitat, p: [f64; 3]) -> Option<[f64; 3]> {
    let x = p[0].floor() as i64;
    let y = p[1].floor() as i64;
    let z = p[2].floor() as i64;
    // Find the free surface of a loaded pool in the supplied column (also
    // supports the administrator command probing from above or within lava).
    for dy in [0, -1, 1] {
        let level = y + dy;
        let fits =
            (-1..=1).all(|dx| (-1..=1).all(|dz| open_pool_cell(h, x + dx, level - 1, z + dz)));
        if fits
            && h.block([x, level - 2, z]) == Some(MaterialId::Lava as u16)
            && h.clear([p[0], level as f64, p[2]], Kind::LavaWorm.size(), false)
        {
            return Some([p[0], level as f64, p[2]]);
        }
    }
    None
}

/// Sweep across both flanks of the swimming body. Its widest plates and the
/// travelling lateral wave fit inside this envelope even between server ticks.
fn channel(h: &Habitat, position: [f64; 3], yaw: f32, full_body: bool) -> bool {
    let (sin, cos) = f64::from(yaw).sin_cos();
    let y = position[1].round() as i64 - 1;
    if h.block([
        position[0].floor() as i64,
        y - 1,
        position[2].floor() as i64,
    ]) != Some(MaterialId::Lava as u16)
    {
        return false;
    }
    let sections = if full_body { 11 } else { 3 };
    (0..sections).all(|n| {
        let back = if full_body {
            -1.52 + n as f64 * 0.76
        } else {
            -0.65 + n as f64 * 0.65
        };
        let width = if full_body { 0.96 } else { 0.63 };
        [-width, 0., width].into_iter().all(|side| {
            let x = (position[0] + back * sin + side * cos).floor() as i64;
            let z = (position[2] + back * cos - side * sin).floor() as i64;
            open_pool_cell(h, x, y, z)
        })
    })
}

fn patrol(a: &mut Creature, h: &Habitat, targets: &[[f64; 3]], dt: f32) {
    let handedness = if a.id & 1 == 0 { -1. } else { 1. };
    let drift = ((a.age * 0.54 + (a.id % 97) as f64).sin() as f32) * 0.45 + handedness * 0.12;
    let turn = targets
        .iter()
        .filter(|p| distance(a.position, **p) < 18.0_f64.powi(2))
        .min_by(|a1, b| distance(a.position, **a1).total_cmp(&distance(a.position, **b)))
        .map_or(drift, |p| {
            let desired = (a.position[0] - p[0]).atan2(a.position[2] - p[2]) as f32;
            let offset = desired - a.yaw;
            offset.sin().atan2(offset.cos()).clamp(-1., 1.)
        });
    let surfaced = a.behavior == B::Swim;
    let speed = a.kind.speed() as f32 * if surfaced { 1. } else { 0.83 };
    for steering in [turn, handedness * 1.4, -handedness * 1.4] {
        let yaw = (a.yaw + steering * dt).rem_euclid(std::f32::consts::TAU);
        let (sin, cos) = yaw.sin_cos();
        let velocity = [-speed * sin, 0., -speed * cos];
        let at = |t: f32| std::array::from_fn(|i| a.position[i] + f64::from(velocity[i] * t));
        if channel(h, at(dt), yaw, surfaced) && channel(h, at(dt + 0.26), yaw, false) {
            a.position = at(dt);
            a.velocity = velocity;
            a.yaw = yaw;
            return;
        }
    }
    // A long surfaced body cannot pivot through a bank. Slip below the crust
    // and search from the same pool until the next heading clears the shore.
    if surfaced {
        begin(a, B::Burrowed, 2.2);
    } else {
        a.yaw = (a.yaw + handedness * dt * 2.15).rem_euclid(std::f32::consts::TAU);
    }
}

/// A submerged worm can roll its long body into the channel before breaching.
fn surfacing_heading(a: &Creature, h: &Habitat) -> Option<f32> {
    let side = if a.id.is_multiple_of(2) { 1. } else { -1. };
    [
        0.,
        0.45,
        -0.45,
        0.96,
        -0.96,
        1.57,
        -1.57,
        std::f32::consts::PI,
    ]
    .into_iter()
    .map(|turn| (a.yaw + turn * side).rem_euclid(std::f32::consts::TAU))
    .find(|&yaw| {
        let (sin, cos) = f64::from(yaw).sin_cos();
        let next = [
            a.position[0] - sin * 0.75,
            a.position[1],
            a.position[2] - cos * 0.75,
        ];
        channel(h, a.position, yaw, true) && channel(h, next, yaw, true)
    })
}

fn begin(a: &mut Creature, b: B, timer: f32) {
    a.behavior = b;
    a.timer = timer;
    a.needs.goal = None;
}
fn bolt(a: &mut Creature, h: &Habitat, targets: &[[f64; 3]], previous: f32) -> Effect {
    let Some(direction) = pose::bolt_direction(a) else {
        return Effect::None;
    };
    let elapsed = (pose::SPIT - a.timer - pose::RELEASE) as f64;
    if elapsed < 0. {
        return Effect::None;
    }
    let speed = f64::from(pose::BOLT_SPEED);
    let from = pose::muzzle(a);
    let near = f64::from((pose::SPIT - previous - pose::RELEASE).max(0.)) * speed;
    let far = elapsed * speed;
    let point = |r: f64| std::array::from_fn(|i| from[i] + direction[i] * r);
    let hit = targets
        .iter()
        .enumerate()
        .filter_map(|(i, &p)| {
            let along = (0..3)
                .map(|j| (p[j] - from[j]) * direction[j])
                .sum::<f64>()
                .clamp(near, far.max(near));
            (distance(point(along), p) < 0.94_f64.powi(2)).then_some((i, along))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some(position) =
        perception::obstruction(h, point(near), point(hit.map_or(far, |(_, along)| along)))
    {
        a.needs.goal = None;
        if h.block(position)
            .and_then(MaterialId::from_raw)
            .is_some_and(MaterialId::flammable)
        {
            return Effect::Ignite { position };
        }
    } else if let Some((index, _)) = hit {
        a.needs.goal = None;
        return Effect::StrikePlayer {
            index,
            damage: definitions::get(a.kind).combat.unwrap().damage * 0.8,
        };
    }
    Effect::None
}
pub fn step(a: &mut Creature, h: &Habitat, targets: &[[f64; 3]], dt: f32) -> Effect {
    if a.kind != Kind::LavaWorm
        || !dt.is_finite()
        || !(0.0..=0.25).contains(&dt)
        || dt == 0.
        || a.health <= 0.
    {
        return Effect::None;
    }
    let mut cell = a.position.map(|v| v.floor() as i64);
    cell[1] -= 1;
    a.velocity = [0.; 3];
    if h.block(cell).is_none() {
        return Effect::None;
    }
    a.age += f64::from(dt);
    let previous = a.timer;
    a.timer = (a.timer - dt).max(0.);
    // Player edits and spreading lava are read through the loaded world. If
    // stranded it stays in place, retracts and withers rather than pursuing.
    if !open_pool_cell(h, cell[0], cell[1], cell[2]) {
        begin(a, B::Burrowed, 2.);
        a.health = (a.health - dt * 2.).max(0.);
        if a.health == 0. {
            a.behavior = B::Dead;
        }
        return Effect::None;
    }
    if a.behavior == B::Spit {
        let hit = bolt(a, h, targets, previous);
        if hit != Effect::None {
            return hit;
        }
    }
    if a.behavior == B::Attack
        && previous > pose::SLAM - pose::IMPACT
        && a.timer <= pose::SLAM - pose::IMPACT
    {
        let combat = definitions::get(a.kind).combat.unwrap();
        let (sin, cos) = f64::from(a.yaw).sin_cos();
        let impact = [
            a.position[0] - sin * combat.reach,
            a.position[1] + 0.65,
            a.position[2] - cos * combat.reach,
        ];
        if let Some((index, _)) =
            perception::nearest_visible(h, pose::muzzle(a), impact, targets, 2., |_| true)
        {
            return Effect::StrikePlayer {
                index,
                damage: combat.damage * 1.4,
            };
        }
    }
    if a.timer > 0. {
        if matches!(a.behavior, B::Burrowed | B::Swim) {
            patrol(a, h, targets, dt);
        }
        return Effect::None;
    }
    match a.behavior {
        B::Burrowed => {
            let cruising = targets
                .iter()
                .all(|p| distance(a.position, *p) > 16.0_f64.powi(2))
                && !(a.age as u64 / 12 + a.id % 3).is_multiple_of(3);
            if cruising && let Some(yaw) = surfacing_heading(a, h) {
                a.yaw = yaw;
                begin(a, B::Swim, pose::SWIM);
            } else if anchor(h, a.position).is_some() {
                begin(a, B::Emerge, pose::RISE);
            } else {
                a.timer = 3.;
            }
        }
        B::Emerge => begin(a, B::Hold, 1.8),
        B::Hold => {
            let combat = definitions::get(a.kind).combat.unwrap();
            if let Some((_, target)) = perception::nearest_visible(
                h,
                pose::muzzle(a),
                a.position,
                targets,
                combat.aggro_radius,
                |_| true,
            ) {
                a.yaw = ((a.position[0] - target[0]).atan2(a.position[2] - target[2]) as f32)
                    .rem_euclid(std::f32::consts::TAU);
                if distance(a.position, target) < (combat.reach + 1.2).powi(2) {
                    begin(a, B::Attack, pose::SLAM);
                } else {
                    begin(a, B::Spit, pose::SPIT);
                    a.needs.goal = Some(NeedGoal {
                        kind: NeedKind::AttackAim,
                        position: target.map(|v| v.floor() as i64),
                    });
                }
            } else {
                begin(a, B::Retreat, pose::DIVE);
            }
        }
        B::Retreat if channel(h, a.position, a.yaw, true) => begin(a, B::Swim, pose::SWIM),
        B::Retreat | B::Swim => begin(a, B::Burrowed, 3.8 + (a.id % 17) as f32 * 0.14),
        _ => begin(a, B::Retreat, pose::DIVE),
    }
    Effect::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TerraGenerator,
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    use terra_core::{
        WorldConfig,
        creatures::{CreatureNeeds, creature_id, spawning::SpawnMob},
    };
    use terra_voxel::{BlockSection, SectionPos};

    fn pool() -> (TerraGenerator, ChunkStore, EditJournal, Creature) {
        let terrain = TerraGenerator::new(WorldConfig {
            water: false,
            biomes: false,
            caves: false,
            trees: false,
            ..Default::default()
        })
        .unwrap();
        let mut chunks = ChunkStore::default();
        let pos = SectionPos::new(0, 0, 0);
        let mut data = BlockSection::empty(pos).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..4 {
                    data.set_block(
                        [x, y, z],
                        if y > 1 && (5..=10).contains(&x) && (6..=11).contains(&z) {
                            MaterialId::Lava as u16
                        } else {
                            MaterialId::Stone as u16
                        },
                    )
                    .unwrap();
                }
            }
        }
        chunks.loaded.insert(
            pos,
            LoadedSection {
                data,
                props: vec![],
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
            },
        );
        let kind = Kind::LavaWorm;
        let animal = Creature {
            id: creature_id([0, 0], 0).unwrap(),
            kind,
            home: [0, 0],
            position: [7.5, 4., 8.5],
            yaw: 0.,
            velocity: [0.; 3],
            behavior: B::Burrowed,
            timer: 0.,
            health: kind.health(),
            age: 0.,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        };
        (terrain, chunks, EditJournal::default(), animal)
    }
    #[test]
    fn patrolling_worm_stays_in_lava_and_attacks_from_its_replicated_pose() {
        let (terrain, chunks, journal, a) = pool();
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        assert_eq!(anchor(&habitat, a.position), Some(a.position));
        assert!(anchor(&habitat, [2., 4., 2.]).is_none());
        assert!(
            terra_core::creatures::hitboxes::for_creature(&a)
                .iter()
                .next()
                .is_none()
        );
        for (target, attack) in [([7.5, 4.9, 5.8], B::Attack), ([7.5, 5., 1.5], B::Spit)] {
            let mut worm = a.clone();
            let mut phases = Vec::new();
            let mut hits = 0;
            for _ in 0..130 {
                if phases.last() != Some(&worm.behavior) {
                    phases.push(worm.behavior);
                }
                let effect = step(&mut worm, &habitat, &[target], 0.05);
                if let Effect::StrikePlayer { index, damage } = effect {
                    assert_eq!(index, 0);
                    assert!(damage >= 3.);
                    hits += 1;
                }
                assert!(is_lava(
                    &habitat,
                    [worm.position[0], worm.position[1] - 0.1, worm.position[2]]
                ));
                assert!(worm.valid());
            }
            assert!(
                phases.contains(&attack)
                    && phases.contains(&B::Emerge)
                    && phases.contains(&B::Retreat),
                "{phases:?}"
            );
            assert_eq!(hits, 1);
        }
    }

    #[test]
    fn a_patrol_surfaces_its_long_body_and_turns_inside_an_irregular_pool() {
        let (terrain, mut chunks, journal, mut a) = pool();
        let section = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
        for x in 1..15 {
            for z in 1..15 {
                if x < 10 || z > 6 {
                    for y in [2, 3] {
                        section
                            .set_block([x, y, z], MaterialId::Lava as u16)
                            .unwrap();
                    }
                }
            }
        }
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        a.position = [7.5, 4., 7.5];
        a.behavior = B::Swim;
        a.timer = pose::SWIM;
        let mut distance_travelled = 0.;
        let mut farthest = 0.0_f64;
        let mut surfaced_distance = 0.;
        let start = a.position;
        for _ in 0..850 {
            let before = a.position;
            step(&mut a, &h, &[], 0.05);
            let distance = distance(before, a.position).sqrt();
            assert!(distance < 0.13);
            distance_travelled += distance;
            farthest = farthest.max(super::distance(start, a.position).sqrt());
            assert!(is_lava(&h, [a.position[0], 3.5, a.position[2]]));
            if a.behavior == B::Swim {
                surfaced_distance += distance;
                assert!(channel(&h, a.position, a.yaw, true));
                let (sin, cos) = f64::from(a.yaw).sin_cos();
                for i in 0..pose::SEGMENTS {
                    let p = pose::center(a.behavior, a.timer, a.age, i);
                    let x = a.position[0] + f64::from(p.x) * cos + f64::from(p.z) * sin;
                    let z = a.position[2] - f64::from(p.x) * sin + f64::from(p.z) * cos;
                    assert!(is_lava(&h, [x, 3.8, z]));
                }
            }
            assert!(a.valid());
        }
        assert!(
            farthest > 4. && distance_travelled > 15. && surfaced_distance > 2.,
            "excursion {farthest}, path {distance_travelled}, surfaced {surfaced_distance}"
        );
    }

    #[test]
    fn released_fireball_respects_cover_and_drained_worm_cannot_move_on_land() {
        let (terrain, chunks, mut journal, mut a) = pool();
        let victim = [7.5, 5., 1.5];
        a.needs.goal = Some(NeedGoal {
            kind: NeedKind::AttackAim,
            position: [7, 5, 1],
        });
        a.behavior = B::Spit;
        a.timer = pose::SPIT;
        for y in 4..9 {
            journal.record([7, y, 4], MaterialId::Stone as u16).unwrap();
        }
        {
            let h = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            for _ in 0..26 {
                assert_eq!(step(&mut a, &h, &[victim], 0.08), Effect::None);
            }
            assert!(a.needs.goal.is_none());
        }
        // A second shot hits the first exposed timber exactly once.
        for y in 4..9 {
            journal.record([7, y, 4], MaterialId::Wood as u16).unwrap();
        }
        a.behavior = B::Spit;
        a.timer = pose::SPIT;
        a.needs.goal = Some(NeedGoal {
            kind: NeedKind::AttackAim,
            position: [7, 5, 1],
        });
        {
            let h = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            let mut ignitions = 0;
            for _ in 0..26 {
                match step(&mut a, &h, &[victim], 0.08) {
                    Effect::Ignite { position } => {
                        assert_eq!([position[0], position[2]], [7, 4]);
                        ignitions += 1;
                    }
                    Effect::None => {}
                    effect => panic!("unexpected impact behind timber: {effect:?}"),
                }
            }
            assert_eq!(ignitions, 1);
        }
        journal.record([7, 3, 8], MaterialId::Stone as u16).unwrap();
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let root = a.position;
        for _ in 0..24 {
            step(&mut a, &h, &[victim], 0.2);
        }
        assert_eq!(a.behavior, B::Burrowed);
        assert_eq!(a.position, root);
        assert!(a.health < Kind::LavaWorm.health());
        assert!(
            super::super::spawning::near(
                &Default::default(),
                &h,
                SpawnMob {
                    kind: a.kind,
                    variant: 0
                },
                [1., 4., 1.],
                0.
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod generated_habitats {
    use super::*;
    use crate::{SectionGenerator, TerraGenerator, edits::EditJournal};
    use terra_core::{
        WorldConfig,
        creatures::{CreaturePopulation, creature_id, skeleton_id},
    };

    use super::super::test_habitat::load_surface;
    #[test]
    fn natural_worm_and_night_graves_reach_all_three_overworld_biomes() {
        let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        let journal = EditJournal::default();
        let mut visited = std::collections::BTreeSet::new();
        let mut worm_found = false;
        for z in (-8192i64..=8192).step_by(128) {
            for x in (-8192i64..=8192).step_by(128) {
                let home = [x / 32, z / 32];
                let seed =
                    super::super::random(creature_id(home, 0).unwrap(), terrain.config().seed.0);
                let px = (x + 8 + (seed % 8) as i64) as f64 + 0.5;
                let pz = (z + 8 + ((seed >> 8) % 8) as i64) as f64 + 0.5;
                let s = terrain.surface_column(px, pz);
                let biome = super::super::biome_slug(terrain.surface_biome(&s));
                let grave_seed =
                    super::super::random(skeleton_id(home, 0).unwrap(), terrain.config().seed.0);
                let grave = matches!(biome, "ashlands" | "spirit_blossom" | "blood_forest")
                    && !visited.contains(biome)
                    && !grave_seed.is_multiple_of(3)
                    && (grave_seed >> 20).is_multiple_of(4)
                    && s.lava_level.is_none();
                let worm = !worm_found
                    && biome == "ashlands"
                    && !(seed >> 32).is_multiple_of(3)
                    && s.lava_level.is_some_and(|level| level - s.height > 2.5);
                if !(worm || grave) {
                    continue;
                }
                let chunks = load_surface(&generator, home);
                let h = Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal,
                };
                let mut pop = CreaturePopulation::default();
                if worm {
                    super::super::spawn_cell(&mut pop, &h, home);
                    if let Some(a) = pop.animals.values().find(|a| a.kind == Kind::LavaWorm) {
                        assert_eq!(anchor(&h, a.position), Some(a.position));
                        assert!(a.valid());
                        eprintln!(
                            "Natural lava worm, seed {}: {:?}",
                            terrain.config().seed.0,
                            a.position
                        );
                        worm_found = true;
                    }
                }
                if grave {
                    let spawn = super::super::forest::graves::spawn;
                    assert_eq!(spawn(&mut pop, &h, home, s.height, false, 2), 0);
                    if spawn(&mut pop, &h, home, s.height, true, 2) > 0 {
                        let a = pop
                            .animals
                            .values()
                            .find(|a| a.kind == Kind::Skeleton)
                            .unwrap();
                        assert_eq!(a.behavior, B::Emerge);
                        assert!(a.valid());
                        visited.insert(biome);
                    }
                }
                if visited.len() == 3 && worm_found {
                    return;
                }
            }
        }
        panic!("missing natural habitats: graves {visited:?}, lava worm {worm_found}");
    }
}
