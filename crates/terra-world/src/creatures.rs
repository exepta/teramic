//! Budgeted local steering over loaded, edited voxels. Unknown terrain is a barrier.
use crate::{TerraGenerator, edits::EditJournal, streaming::ChunkStore};
use terra_core::creatures::{
    Creature, CreatureBehavior as Behavior, CreatureKind as Kind, CreaturePopulation, creature_id,
};
pub mod aquatics;
pub mod bears;
pub mod bees;
pub mod ecology;
mod horses;
pub mod navigation;
#[path = "creature_needs.rs"]
mod needs;
pub mod spawning;
mod support;
pub use needs::{Forage, apply_forage};

pub const ACTIVE_RADIUS: f64 = 150.0;
pub const UPDATE_BUDGET: usize = 128;

pub struct Habitat<'a> {
    pub terrain: &'a TerraGenerator,
    pub chunks: &'a ChunkStore,
    pub journal: &'a EditJournal,
}
impl Habitat<'_> {
    fn boxes(
        &self,
        cell: [i64; 3],
        mut visit: impl FnMut(terra_core::block_shapes::ShapeBox),
    ) -> bool {
        static PROFILES: std::sync::OnceLock<terra_core::block_properties::BlockPropertiesCatalog> =
            std::sync::OnceLock::new();
        crate::interaction_shapes::visit_boxes(
            self.chunks,
            self.journal,
            PROFILES.get_or_init(Default::default),
            cell,
            &mut visit,
        )
    }
    fn obstructs(
        &self,
        cell: [i64; 3],
        lower: [f64; 3],
        upper: [f64; 3],
        pose: ([f64; 3], [f64; 3], f64),
    ) -> bool {
        if !self.solid(cell) {
            return false;
        }
        let mut blocked = false;
        let mut intersects = |shape: terra_core::block_shapes::ShapeBox| {
            if !shape.intersects(lower, upper) {
                return;
            }
            let (p, size, yaw) = pose;
            let (sin, cos) = yaw.sin_cos();
            let dx = (shape.minimum[0] + shape.maximum[0]) * 0.5 - p[0];
            let dz = (shape.minimum[2] + shape.maximum[2]) * 0.5 - p[2];
            let x = (shape.maximum[0] - shape.minimum[0]) * 0.5;
            let z = (shape.maximum[2] - shape.minimum[2]) * 0.5;
            // The two local rectangle axes complete the horizontal SAT test.
            blocked |= (dx * cos - dz * sin).abs()
                < size[0] * 0.5 + x * cos.abs() + z * sin.abs() - 0.000001
                && (dx * sin + dz * cos).abs()
                    < size[2] * 0.5 + x * sin.abs() + z * cos.abs() - 0.000001;
        };
        if !self.boxes(cell, &mut intersects) {
            intersects(terra_core::block_shapes::ShapeBox {
                minimum: cell.map(|v| v as f64),
                maximum: cell.map(|v| v as f64 + 1.0),
                hinged: false,
            });
        }
        blocked
    }
    fn support_at(&self, cell: [i64; 3], point: [f64; 3]) -> Option<f64> {
        let mut top: Option<f64> = None;
        let shaped = self.boxes(cell, |shape| {
            if point[0] >= shape.minimum[0]
                && point[0] < shape.maximum[0]
                && point[2] >= shape.minimum[2]
                && point[2] < shape.maximum[2]
            {
                top = Some(top.map_or(shape.maximum[1], |v| v.max(shape.maximum[1])));
            }
        });
        if shaped {
            top
        } else {
            Some(cell[1] as f64 + 1.0)
        }
    }
    pub fn block(&self, p: [i64; 3]) -> Option<u16> {
        // Loading is still required even when a journal override exists.
        self.chunks
            .block_at(p)
            .map(|b| self.journal.block(p).unwrap_or(b))
    }
    fn solid(&self, p: [i64; 3]) -> bool {
        self.block(p)
            .is_none_or(|b| terra_voxel::MaterialId::from_raw(b).is_none_or(|b| b.is_solid()))
    }
    pub fn wet(&self, p: [f64; 3]) -> bool {
        let cell = p.map(|v| v.floor() as i64);
        self.block(cell).is_some_and(|b| {
            if crate::water::space::partial(b) {
                return self
                    .chunks
                    .water_column_at_point(self.terrain, self.journal, p)
                    .is_some_and(|water| p[1] < water.level);
            }
            let fluid = self
                .chunks
                .fluid_cell_at(self.terrain, self.journal, cell, b);
            fluid.amount > 0 && p[1] < cell[1] as f64 + fluid.height()
        })
    }
    /// The contiguous local fluid column, including partial flowing cells.
    fn water_surface(&self, p: [f64; 3]) -> Option<f64> {
        let first = p.map(|v| v.floor() as i64);
        if self.block(first).is_some_and(crate::water::space::partial) {
            return self
                .chunks
                .water_column_at_point(self.terrain, self.journal, p)
                .map(|water| water.level);
        }
        let mut cell = p.map(|v| v.floor() as i64);
        let mut surface = None;
        for _ in 0..16 {
            let block = self.block(cell)?;
            let fluid = self
                .chunks
                .fluid_cell_at(self.terrain, self.journal, cell, block);
            if fluid.amount == 0 {
                return surface;
            }
            let top = cell[1] as f64 + fluid.height();
            surface = Some(top);
            if fluid.height() < 1.0 {
                return surface;
            }
            cell[1] += 1;
        }
        surface
    }
    pub fn clear(&self, p: [f64; 3], size: [f64; 3], fish: bool) -> bool {
        self.clear_rotated(p, size, fish, 0.0)
    }
    pub fn clear_rotated(&self, p: [f64; 3], size: [f64; 3], fish: bool, yaw: f64) -> bool {
        let (sin, cos) = yaw.sin_cos();
        let half = [
            (size[0] * cos.abs() + size[2] * sin.abs()) * 0.5,
            (size[0] * sin.abs() + size[2] * cos.abs()) * 0.5,
        ];
        let lower = [p[0] - half[0], p[1] + 0.04, p[2] - half[1]];
        let upper = [
            p[0] + half[0] - 0.000001,
            p[1] + size[1] - 0.02,
            p[2] + half[1] - 0.000001,
        ];
        let min = lower.map(|v| v.floor() as i64);
        let max = upper.map(|v| v.floor() as i64);
        // Visit every intersected cell once. Three fixed samples can skip an
        // entire voxel inside a broad animal, depending on its sub-block offset.
        for x in min[0]..=max[0] {
            for z in min[2]..=max[2] {
                for y in min[1]..=max[1] {
                    let cell = [x, y, z];
                    let Some(block) = self.block(cell) else {
                        return false;
                    };
                    if self.obstructs(cell, lower, upper, (p, size, yaw)) {
                        return false;
                    }
                    let fluid = self
                        .chunks
                        .fluid_cell_at(self.terrain, self.journal, cell, block);
                    let height = if fluid.amount > 0 {
                        y as f64 + fluid.height()
                    } else {
                        f64::NEG_INFINITY
                    };
                    let flooded = if fish {
                        upper[1].min(y as f64 + 1.0 - 0.000001) <= height
                    } else {
                        lower[1].max(y as f64) < height
                    };
                    if flooded != fish {
                        return false;
                    }
                }
            }
        }
        true
    }
    pub fn clear_swimmer(&self, p: [f64; 3], size: [f64; 3]) -> bool {
        self.clear_swimmer_rotated(p, size, 0.0)
    }
    pub fn clear_swimmer_rotated(&self, p: [f64; 3], size: [f64; 3], yaw: f64) -> bool {
        let (sin, cos) = yaw.sin_cos();
        let half = [
            (size[0] * cos.abs() + size[2] * sin.abs()) * 0.5,
            (size[0] * sin.abs() + size[2] * cos.abs()) * 0.5,
        ];
        let lower = [p[0] - half[0], p[1] + 0.02, p[2] - half[1]];
        let upper = [
            p[0] + half[0] - 0.000001,
            p[1] + size[1] - 0.01,
            p[2] + half[1] - 0.000001,
        ];
        let min = lower.map(|v| v.floor() as i64);
        let max = upper.map(|v| v.floor() as i64);
        for x in min[0]..=max[0] {
            for z in min[2]..=max[2] {
                for y in min[1]..=max[1] {
                    let cell = [x, y, z];
                    if self.block(cell).is_none()
                        || self.obstructs(cell, lower, upper, (p, size, yaw))
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
    pub fn ground(&self, p: [f64; 3], kind: Kind) -> Option<[f64; 3]> {
        self.ground_rotated(p, kind, 0.0)
    }
    pub fn ground_rotated(&self, p: [f64; 3], kind: Kind, yaw: f64) -> Option<[f64; 3]> {
        let (sin, cos) = yaw.sin_cos();
        let half = [kind.size()[0] * 0.5 - 0.0001, kind.size()[2] * 0.5 - 0.0001];
        let mut support = f64::NEG_INFINITY;
        let mut lowest = f64::INFINITY;
        let mut dry_hooves = 0;
        for dx in [-half[0], half[0]] {
            for dz in [-half[1], half[1]] {
                let [dx, dz] = [dx * cos + dz * sin, -dx * sin + dz * cos];
                let mut found = None;
                let mut in_water = false;
                for dy in (-2..=0).rev() {
                    let q = [
                        (p[0] + dx).floor() as i64,
                        p[1].floor() as i64 + dy,
                        (p[2] + dz).floor() as i64,
                    ];
                    let block = self.block(q)?;
                    if self.solid(q)
                        && let Some(top) = self.support_at(q, [p[0] + dx, p[1], p[2] + dz])
                    {
                        found = Some(top);
                        break;
                    }
                    let fluid = self
                        .chunks
                        .fluid_cell_at(self.terrain, self.journal, q, block);
                    if fluid.amount > 0 {
                        in_water = true;
                        break;
                    }
                }
                if in_water {
                    continue;
                }
                let height = found?;
                dry_hooves += 1;
                support = support.max(height);
                lowest = lowest.min(height);
            }
        }
        let next = [p[0], support, p[2]];
        // Permit a one-voxel walking step, but never bridge a deeper air gap
        // simply because a distant body corner touches the upper ledge.
        // Two real contacts can climb a bank while the rear hooves still swim;
        // the fluid surface itself never contributes a support height.
        (dry_hooves >= 2
            && (support - lowest) <= 1.001
            && (support - p[1]).abs() <= 1.01
            && self.clear_rotated(next, kind.size(), false, yaw))
        .then_some(next)
    }
}

fn random(id: u64, salt: u64) -> u64 {
    let mut x = id.wrapping_add(salt.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

pub(super) fn biome_slug(biome: crate::generation::biomes::BiomeKind) -> &'static str {
    use crate::generation::biomes::BiomeKind;
    match biome {
        BiomeKind::Ocean => "ocean",
        BiomeKind::Plains => "plains",
        BiomeKind::Forest => "forest",
        BiomeKind::Desert => "desert",
        BiomeKind::Highland => "highland",
        BiomeKind::MushroomPark => "mushroom_park",
        BiomeKind::ClifftedDesert => "cliffted_desert",
    }
}

pub fn spawn_cell(population: &mut CreaturePopulation, habitat: &Habitat, home: [i64; 2]) -> bool {
    spawn_cell_using(
        population,
        habitat,
        home,
        terra_core::creatures::definitions::get,
    )
}

fn spawn_cell_using<'a>(
    population: &mut CreaturePopulation,
    habitat: &Habitat,
    home: [i64; 2],
    definition: impl Fn(Kind) -> &'a terra_core::creatures::definitions::MobDefinition,
) -> bool {
    if population.explored.contains(&home)
        || population.explored.len() >= terra_core::creatures::MAX_SPAWN_CELLS
        || population.animals.len() + 4 > terra_core::creatures::MAX_CREATURES
        || creature_id(home, 0).is_none()
    {
        return false;
    }
    let mut candidates = Vec::new();
    let mut bear_habitat = false;
    for slot in 0..4 {
        if slot > 0 && bear_habitat {
            break;
        }
        let Some(id) = creature_id(home, slot) else {
            return false;
        };
        let seed = random(id, habitat.terrain.config().seed.0);
        let group_seed = random(
            creature_id(home, 0).unwrap_or(id),
            habitat.terrain.config().seed.0,
        );
        let x = (home[0] * 32 + 8 + (group_seed % 8) as i64 + i64::from(slot % 2) * 3) as f64 + 0.5;
        let z = (home[1] * 32 + 8 + ((group_seed >> 8) % 8) as i64 + i64::from(slot / 2) * 3)
            as f64
            + 0.5;
        if x.abs() > 998_900.0 || z.abs() > 998_900.0 {
            continue;
        }
        let surface = habitat.terrain.surface_column(x, z);
        let water = habitat.terrain.water_column(x, z);
        let fish = water.is_some_and(|w| w.level - w.bed > 2.0);
        let mut y = if fish {
            water.map_or(surface.height, |w| w.level - 1.5)
        } else {
            surface.height.floor() + 1.0
        };
        if habitat
            .block([x.floor() as i64, y.floor() as i64, z.floor() as i64])
            .is_none()
        {
            return false;
        }
        // Use different hash bits for occupancy and species; otherwise rejecting
        // residue zero would exclude every sheep herd.
        // Existing recorded identities, including deaths, are never replaced.
        if (group_seed >> 32).is_multiple_of(3) {
            continue;
        }
        let biome = biome_slug(surface.biome.weights.dominant());
        let kind = if fish {
            let ocean = surface.biome.weights.ocean > 0.55;
            let lake = habitat
                .terrain
                .lake_at(x, z)
                .is_some_and(|l| l.distance <= l.radius);
            let water_kind = if ocean {
                terra_core::creatures::definitions::SpawnWater::Ocean
            } else if lake {
                terra_core::creatures::definitions::SpawnWater::Lake
            } else {
                terra_core::creatures::definitions::SpawnWater::River
            };
            let Some(kind) = aquatics::select_spawn_kind(
                group_seed,
                water_kind,
                water.map_or(0.0, |w| w.depth),
                biome,
                &definition,
            ) else {
                continue;
            };
            kind
        } else if slot == 0
            && (group_seed >> 40).is_multiple_of(7)
            && bears::near_water(habitat, [x, y, z])
        {
            bear_habitat = true;
            Kind::Bear
        } else {
            if surface.biome.weights.plains > 0.35 && (group_seed >> 48).is_multiple_of(5) {
                Kind::Horse
            } else {
                Kind::ALL[(random(
                    creature_id(home, 0).unwrap_or(id),
                    habitat.terrain.config().seed.0,
                ) % 3) as usize]
            }
        };
        let legacy_group = if fish {
            2 + (group_seed % 3) as u8
        } else {
            3 + (group_seed % 2) as u8
        };
        let species = definition(kind);
        if slot >= species.spawn_group_size(group_seed, biome, legacy_group) {
            continue;
        }
        let position = if fish {
            let Some(water) = water else {
                continue;
            };
            let Some(depth) = aquatics::spawn_depth_for(species, seed, water.depth) else {
                continue;
            };
            y = if species.aquatic_spawn().depth_below_surface.is_some() {
                (water.level - depth).max(water.bed + 0.2)
            } else if kind == Kind::Fish {
                y
            } else {
                // Keep legacy floating-point placement as well as its hash decisions.
                (water.level - kind.size()[1] - 0.7 - (seed % 300) as f64 * 0.01)
                    .max(water.bed + 0.2)
            };
            let p = [x, y, z];
            // Unknown sections are not evidence of an invalid habitat. Wait for
            // the entire body before committing this cell's persistent identities.
            if !spawn_volume_loaded(habitat, p, kind.size()) {
                return false;
            }
            if !habitat.clear(p, kind.size(), true) {
                continue;
            }
            p
        } else {
            let Some(p) = habitat.ground([x, y, z], kind) else {
                continue;
            };
            let support = habitat.block([x.floor() as i64, p[1] as i64 - 1, z.floor() as i64]);
            if !(matches!(support, Some(1 | 2)) || kind == Kind::Bear && support == Some(4)) {
                continue;
            }
            p
        };
        candidates.push(Creature {
            id,
            kind,
            home,
            position,
            velocity: [0.0; 3],
            yaw: (seed % 628) as f32 * 0.01,
            health: kind.health(),
            behavior: if fish { Behavior::Swim } else { Behavior::Rest },
            timer: 1.0 + (seed % 40) as f32 * 0.1,
            age: 0.0,
            variant: (seed % if kind == Kind::Horse { 16 } else { 4 }) as u8,
            needs: terra_core::creatures::CreatureNeeds::full(kind),
        });
    }
    population.explored.insert(home);
    for creature in candidates {
        population.animals.entry(creature.id).or_insert(creature);
    }
    true
}

fn spawn_volume_loaded(habitat: &Habitat, position: [f64; 3], size: [f64; 3]) -> bool {
    let minimum = [
        position[0] - size[0] * 0.5,
        position[1] + 0.04,
        position[2] - size[2] * 0.5,
    ];
    let maximum = [
        position[0] + size[0] * 0.5 - 0.000001,
        position[1] + size[1] - 0.02,
        position[2] + size[2] * 0.5 - 0.000001,
    ];
    let minimum = terra_voxel::global_to_section(minimum.map(|v| v.floor() as i64)).0;
    let maximum = terra_voxel::global_to_section(maximum.map(|v| v.floor() as i64)).0;
    (minimum.x..=maximum.x).all(|x| {
        (minimum.y..=maximum.y).all(|y| {
            (minimum.z..=maximum.z).all(|z| {
                habitat
                    .chunks
                    .loaded
                    .contains_key(&terra_voxel::SectionPos::new(x, y, z))
            })
        })
    })
}

pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
}

pub fn step(animal: &mut Creature, habitat: &Habitat, players: &[[f64; 3]], dt: f32) -> Forage {
    step_budgeted(animal, habitat, players, dt, true)
}

pub fn step_budgeted(
    animal: &mut Creature,
    habitat: &Habitat,
    players: &[[f64; 3]],
    dt: f32,
    search: bool,
) -> Forage {
    step_with_flowers(animal, habitat, players, dt, search, None, None)
}
pub(super) fn step_with_flowers(
    animal: &mut Creature,
    habitat: &Habitat,
    players: &[[f64; 3]],
    dt: f32,
    search: bool,
    ecology: Option<&terra_core::creatures::ecology::Ecology>,
    mut navigation: Option<&mut navigation::Navigation>,
) -> Forage {
    if animal.health <= 0.0
        || !dt.is_finite()
        || dt <= 0.0
        || dt > 0.25
        || habitat
            .block(animal.position.map(|v| v.floor() as i64))
            .is_none()
    {
        return Forage::None;
    }
    if animal.kind == Kind::Bear {
        bears::step_with_navigation(
            animal,
            habitat,
            players,
            &bears::PreyIndex::default(),
            dt,
            search,
            false,
            navigation,
        );
        return Forage::None;
    }
    if animal.kind.is_aquatic() && animal.kind != Kind::Fish {
        aquatics::step(animal, habitat, players, &bears::PreyIndex::default(), dt);
        return Forage::None;
    }
    let previous_yaw = animal.yaw;
    animal.age += f64::from(dt);
    animal.timer = (animal.timer - dt).max(0.0);
    if animal.kind == Kind::Horse && animal.behavior == Behavior::Leap {
        horses::advance(animal, habitat, dt);
        return Forage::None;
    }
    if animal.kind == Kind::Bee {
        animal.needs.search_in = (animal.needs.search_in - dt).max(0.0);
        bees::step(animal, habitat, dt, search, ecology, navigation);
        return Forage::None;
    }
    if animal.kind != Kind::Fish && support::swimming(animal, habitat) {
        let forage = needs::update(animal, habitat, dt, search);
        let target = [
            animal.position[0] - f64::from(animal.yaw.sin()) * 4.0,
            animal.position[1],
            animal.position[2] - f64::from(animal.yaw.cos()) * 4.0,
        ];
        let moved = support::swim_toward(animal, habitat, target, dt, animal.kind.speed());
        if !moved && animal.timer <= 0.0 {
            animal.yaw = (animal.yaw + 0.9).rem_euclid(std::f32::consts::TAU);
            animal.timer = 1.8;
        }
        return forage;
    }
    let nearest = players
        .iter()
        .min_by(|a, b| distance(**a, animal.position).total_cmp(&distance(**b, animal.position)));
    if let Some(player) = nearest.filter(|p| {
        animal.kind != Kind::Fish
            && distance(**p, animal.position)
                < if animal.behavior == Behavior::Flee && animal.timer > 0.0 {
                    2.5_f64.powi(2)
                } else {
                    0.95_f64.powi(2)
                }
    }) {
        animal.yaw =
            ((player[0] - animal.position[0]).atan2(player[2] - animal.position[2])) as f32;
        animal.behavior = Behavior::Flee;
        animal.timer = 2.5;
    }
    if animal.timer <= 0.0 && animal.needs.goal.is_none() {
        let seed = random(animal.id, (animal.age * 10.0) as u64);
        animal.yaw = (seed % 628) as f32 * 0.01;
        animal.behavior = if animal.kind == Kind::Fish {
            Behavior::Swim
        } else {
            match seed % if animal.kind == Kind::Pig { 5 } else { 4 } {
                0 => Behavior::Rest,
                1 if animal.kind == Kind::Sheep => Behavior::Shake,
                _ => Behavior::Wander,
            }
        };
        animal.timer = if animal.behavior == Behavior::Shake {
            1.2
        } else {
            2.0 + (seed % 60) as f32 * 0.1
        };
    }
    if matches!(animal.behavior, Behavior::Rest | Behavior::Shake)
        && let Some(player) = nearest.filter(|p| distance(**p, animal.position) < 6.0_f64.powi(2))
    {
        animal.yaw = (animal.position[0] - player[0]).atan2(animal.position[2] - player[2]) as f32;
    }
    if animal.kind == Kind::Fish && !habitat.wet(animal.position) {
        animal.behavior = Behavior::Stranded;
        animal.health = (animal.health - dt * 0.4).max(0.0);
    } else if animal.kind == Kind::Fish && animal.behavior == Behavior::Stranded {
        animal.behavior = Behavior::Swim;
    }
    let forage = needs::update(animal, habitat, dt, search);
    if animal.yaw != previous_yaw
        && !habitat.clear_swimmer_rotated(animal.position, animal.size(), f64::from(animal.yaw))
    {
        animal.yaw = previous_yaw;
    }
    let moving = matches!(
        animal.behavior,
        Behavior::Wander
            | Behavior::Flee
            | Behavior::Swim
            | Behavior::SeekFood
            | Behavior::SeekWater
    );
    let speed = if moving {
        animal.kind.speed()
            * if animal.behavior == Behavior::Flee {
                2.8
            } else {
                1.0
            }
    } else {
        0.0
    };
    let mut target = animal.position;
    target[0] -= f64::from(animal.yaw.sin()) * speed * f64::from(dt);
    target[2] -= f64::from(animal.yaw.cos()) * speed * f64::from(dt);
    let grounded = animal.kind == Kind::Fish
        || habitat
            .ground_rotated(animal.position, animal.kind, f64::from(animal.yaw))
            .is_some_and(|p| p[1] <= animal.position[1] + 0.001);
    if animal.kind == Kind::Horse
        && grounded
        && speed > 0.0
        && navigation
            .as_deref_mut()
            .is_none_or(|nav| nav.horse_probe(animal.id, animal.age))
    {
        // Start the ascent before the wide body contacts the ledge, not after
        // it is wedged into a corner. Failed probes share the navigation budget.
        for distance in [2.0, 2.8] {
            for rise in [1.0, 2.0] {
                let probe = [
                    animal.position[0] - f64::from(animal.yaw.sin()) * distance,
                    animal.position[1] + rise,
                    animal.position[2] - f64::from(animal.yaw.cos()) * distance,
                ];
                if let Some(landing) =
                    habitat.ground_rotated(probe, animal.kind, f64::from(animal.yaw))
                    && horses::launch(animal, habitat, landing)
                {
                    if let Some(nav) = navigation.as_deref_mut() {
                        nav.horse_jumped(animal.id, animal.age);
                    }
                    return forage;
                }
            }
        }
    }
    let destination = animal.needs.goal.map_or_else(
        || {
            [
                animal.position[0] - f64::from(animal.yaw.sin()) * 4.0,
                animal.position[1],
                animal.position[2] - f64::from(animal.yaw.cos()) * 4.0,
            ]
        },
        |g| g.position.map(|v| v as f64 + 0.5),
    );
    let move_to_waypoint = |waypoint| {
        let length = distance(waypoint, animal.position).sqrt().max(0.001);
        let p = std::array::from_fn(|i| {
            animal.position[i]
                + (waypoint[i] - animal.position[i]) / length * (speed * f64::from(dt)).min(length)
        });
        if animal.kind == Kind::Fish {
            habitat.clear(p, animal.kind.size(), true).then_some(p)
        } else {
            let yaw = (animal.position[0] - p[0]).atan2(animal.position[2] - p[2]);
            habitat.ground_rotated(p, animal.kind, yaw)
        }
    };
    // A free direct step does not mean a detour has cleared its obstacle.
    let detour = if animal.kind != Kind::Fish
        && grounded
        && speed > 0.0
        && navigation
            .as_ref()
            .is_some_and(|nav| nav.following_ground_route(animal.id))
    {
        navigation::ground_waypoint(
            animal,
            habitat,
            destination,
            false,
            navigation.as_deref_mut(),
        )
        .and_then(move_to_waypoint)
    } else {
        None
    };
    let mut next = if detour.is_some() {
        detour
    } else if animal.kind == Kind::Fish && animal.behavior != Behavior::Stranded {
        let flow = crate::water::fluid_motion_at(
            habitat.terrain,
            habitat.chunks,
            habitat.journal,
            animal.position,
        );
        for (axis, value) in target.iter_mut().enumerate() {
            // Fish actively swim against moderate current instead of being passive debris.
            *value += f64::from(flow.velocity[axis].clamp(-0.45, 0.45) * 0.2 * dt);
        }
        target[1] += (animal.age * 0.6 + animal.id as f64 % 11.0).sin() * 0.2 * f64::from(dt);
        habitat
            .clear(target, animal.kind.size(), true)
            .then_some(target)
    } else if grounded {
        habitat.ground_rotated(target, animal.kind, f64::from(animal.yaw))
    } else {
        None
    };
    if animal.kind != Kind::Fish
        && detour.is_none()
        && next.is_some()
        && let Some(nav) = navigation.as_deref_mut()
    {
        nav.ground_unblocked(animal.id);
    }
    if grounded && next.is_none() && speed > 0.0 {
        let waypoint = if animal.kind == Kind::Fish {
            navigation::waypoint(habitat, animal.position, destination, animal.kind)
        } else {
            navigation::ground_waypoint(animal, habitat, destination, search, navigation)
        };
        if let Some(waypoint) = waypoint {
            next = move_to_waypoint(waypoint);
            if next.is_some() {
                animal.yaw = (-(waypoint[0] - animal.position[0]))
                    .atan2(-(waypoint[2] - animal.position[2])) as f32;
            }
        }
    }
    if detour.is_some()
        && let Some(step) = next
    {
        animal.yaw = (animal.position[0] - step[0]).atan2(animal.position[2] - step[2]) as f32;
    }
    if grounded && next.is_none() && speed > 0.0 {
        let sign = if animal.id.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        for turn in [0.7, -0.7, 1.4, -1.4, 2.2] {
            let yaw = animal.yaw + turn * sign;
            let mut probe = animal.position;
            probe[0] -= f64::from(yaw.sin()) * 0.7;
            probe[2] -= f64::from(yaw.cos()) * 0.7;
            let candidate = if animal.kind == Kind::Fish {
                habitat
                    .clear(probe, animal.kind.size(), true)
                    .then_some(probe)
            } else {
                habitat.ground_rotated(probe, animal.kind, f64::from(yaw))
            };
            if candidate.is_some() {
                let mut step = animal.position;
                step[0] -= f64::from(yaw.sin()) * speed * f64::from(dt);
                step[2] -= f64::from(yaw.cos()) * speed * f64::from(dt);
                next = if animal.kind == Kind::Fish {
                    habitat
                        .clear(step, animal.kind.size(), true)
                        .then_some(step)
                } else {
                    habitat.ground_rotated(step, animal.kind, f64::from(yaw))
                };
                if next.is_some() {
                    animal.yaw = yaw.rem_euclid(std::f32::consts::TAU);
                    break;
                }
            }
        }
    }
    if animal.kind == Kind::Horse && next.is_some_and(|p| p[1] > animal.position[1] + 0.3) {
        next = None;
    }
    let before = animal.position;
    if let Some(next) = next {
        animal.position = next;
    } else {
        if animal.kind != Kind::Fish || animal.behavior == Behavior::Stranded {
            support::settle(animal, habitat, dt);
        }
        // Pause when boxed in; changing heading every tick caused stationary spinning.
        animal.behavior = if animal.kind == Kind::Fish {
            Behavior::Swim
        } else {
            Behavior::Rest
        };
        animal.needs.goal = None;
        animal.needs.search_in = 2.0;
        animal.timer = 2.0 + (random(animal.id, animal.age as u64) % 20) as f32 * 0.1;
    }
    animal.velocity = std::array::from_fn(|i| {
        ((animal.position[i] - before[i]) / f64::from(dt.max(0.001))) as f32
    });
    if animal.health <= 0.0 {
        animal.behavior = Behavior::Dead;
        animal.velocity = [0.0; 3];
    }
    forage
}

/// Slab picking against the same conservative articulated-body bounds as navigation.
pub fn ray_distance(
    animal: &Creature,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<f64> {
    if animal.health <= 0.0 || animal.behavior == Behavior::InNest {
        return None;
    }
    let size = animal.size();
    let (origin, direction, min, max) = if animal.kind.is_aquatic() {
        let [x, y, z] = std::array::from_fn(|i| origin[i] - animal.position[i]);
        let (sin, cos) = f64::from(animal.yaw).sin_cos();
        (
            [cos * x - sin * z, y, sin * x + cos * z],
            [
                cos * direction[0] - sin * direction[2],
                direction[1],
                sin * direction[0] + cos * direction[2],
            ],
            [-size[0] * 0.5, 0.0, -size[2] * 0.5],
            [size[0] * 0.5, size[1], size[2] * 0.5],
        )
    } else {
        let r = size[0].max(size[2]) * 0.5;
        let min = [
            animal.position[0] - r,
            animal.position[1],
            animal.position[2] - r,
        ];
        let max = [
            animal.position[0] + r,
            animal.position[1] + size[1],
            animal.position[2] + r,
        ];
        (origin, direction, min, max)
    };
    let mut near: f64 = 0.0;
    let mut far = reach;
    for axis in 0..3 {
        if direction[axis].abs() < 1e-9 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
        } else {
            let a = (min[axis] - origin[axis]) / direction[axis];
            let b = (max[axis] - origin[axis]) / direction[axis];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    Some(near)
}
