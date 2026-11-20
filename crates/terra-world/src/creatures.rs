//! Budgeted local steering over loaded, edited voxels. Unknown terrain is a barrier.
use crate::{TerraGenerator, edits::EditJournal, streaming::ChunkStore};
use terra_core::creatures::{
    Creature, CreatureBehavior as Behavior, CreatureKind as Kind, CreaturePopulation, creature_id,
};
pub mod aquatics;
pub mod bears;
pub mod bee_lifecycle;
pub mod bees;
pub mod breeding;
pub mod companions;
pub mod ecology;
pub mod forest;
mod horses;
pub mod navigation;
#[path = "creature_needs.rs"]
mod needs;
mod perception;
pub mod riding;
pub mod sandworm;
pub mod spawning;
pub mod spider_hunting;
pub mod spiders;
pub mod spirit;
pub mod stalker;
mod support;
pub mod villagers;
mod work;
pub use needs::{Forage, apply_forage};
pub use work::{work_rest, work_rest_with_access, work_step, work_step_with_access};

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
        crate::interaction_shapes::visit_collision_boxes_with(
            self.journal,
            PROFILES.get_or_init(Default::default),
            cell,
            |position| self.block(position),
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
            blocked |= Self::shape_obstructs(shape, lower, upper, pose);
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
    fn shape_obstructs(
        shape: terra_core::block_shapes::ShapeBox,
        lower: [f64; 3],
        upper: [f64; 3],
        (p, size, yaw): ([f64; 3], [f64; 3], f64),
    ) -> bool {
        if !shape.intersects(lower, upper) {
            return false;
        }
        let (sin, cos) = yaw.sin_cos();
        let dx = (shape.minimum[0] + shape.maximum[0]) * 0.5 - p[0];
        let dz = (shape.minimum[2] + shape.maximum[2]) * 0.5 - p[2];
        let x = (shape.maximum[0] - shape.minimum[0]) * 0.5;
        let z = (shape.maximum[2] - shape.minimum[2]) * 0.5;
        // The two local rectangle axes complete the horizontal SAT test.
        (dx * cos - dz * sin).abs() < size[0] * 0.5 + x * cos.abs() + z * sin.abs() - 0.000001
            && (dx * sin + dz * cos).abs()
                < size[2] * 0.5 + x * sin.abs() + z * cos.abs() - 0.000001
    }
    fn support_at(&self, cell: [i64; 3], point: [f64; 3]) -> Option<f64> {
        let mut top: Option<f64> = None;
        let shaped = self.boxes(cell, |shape| {
            if point[0] >= shape.minimum[0]
                && point[0] < shape.maximum[0]
                && point[2] >= shape.minimum[2]
                && point[2] < shape.maximum[2]
                && shape.maximum[1] <= point[1] + 1.01
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
    pub(super) fn wet_footprint(&self, p: [f64; 3], size: [f64; 3], yaw: f64) -> bool {
        let immersed =
            |feet: [f64; 3]| self.wet(feet) || self.wet([feet[0], feet[1] - 0.08, feet[2]]);
        if immersed(p) {
            return true;
        }
        let (sin, cos) = yaw.sin_cos();
        let half = [size[0] * 0.5, size[2] * 0.5];
        [-1.0, 0.0, 1.0].into_iter().any(|x| {
            [-1.0, 0.0, 1.0].into_iter().any(|z| {
                (x != 0.0 || z != 0.0)
                    && immersed([
                        p[0] + x * half[0] * cos + z * half[1] * sin,
                        p[1],
                        p[2] - x * half[0] * sin + z * half[1] * cos,
                    ])
            })
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
                    // The enclosing AABB has dry/unloaded corner cells that a
                    // rotated long body never occupies. Apply the same SAT
                    // intersection used for solid shapes before testing fluid.
                    if !Self::shape_obstructs(
                        terra_core::block_shapes::ShapeBox {
                            minimum: cell.map(|v| v as f64),
                            maximum: cell.map(|v| v as f64 + 1.0),
                            hinged: false,
                        },
                        lower,
                        upper,
                        (p, size, yaw),
                    ) {
                        continue;
                    }
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
    /// Walking cannot skip an intervening wall, unsupported gap or unloaded cell.
    pub fn walk_step(
        &self,
        start: [f64; 3],
        target: [f64; 3],
        kind: Kind,
        yaw: f64,
    ) -> Option<[f64; 3]> {
        self.walk_step_sized(start, target, kind.size(), yaw)
    }
    pub fn walk_step_sized(
        &self,
        start: [f64; 3],
        target: [f64; 3],
        size: [f64; 3],
        yaw: f64,
    ) -> Option<[f64; 3]> {
        self.swept_step(start, target, size, yaw, false)
    }
    /// Actual locomotion may enter verified water; a fluid surface is never
    /// promoted to ground support. The existing buoyancy resolver takes over.
    pub fn travel_step(
        &self,
        start: [f64; 3],
        target: [f64; 3],
        kind: Kind,
        yaw: f64,
    ) -> Option<[f64; 3]> {
        self.travel_step_sized(start, target, kind.size(), yaw)
    }
    pub fn travel_step_sized(
        &self,
        start: [f64; 3],
        target: [f64; 3],
        size: [f64; 3],
        yaw: f64,
    ) -> Option<[f64; 3]> {
        self.swept_step(start, target, size, yaw, true)
    }
    fn swept_step(
        &self,
        start: [f64; 3],
        target: [f64; 3],
        size: [f64; 3],
        yaw: f64,
        water_entry: bool,
    ) -> Option<[f64; 3]> {
        let distance = (target[0] - start[0]).hypot(target[2] - start[2]);
        if !yaw.is_finite()
            || !start.iter().chain(target.iter()).all(|v| v.is_finite())
            || size
                .iter()
                .any(|v| !v.is_finite() || *v <= 0.0 || *v > 24.0)
            || distance > 11.52
            || !self.clear_swimmer_rotated(start, size, yaw)
        {
            return None;
        }
        let steps = (distance / 0.18).ceil().clamp(1.0, 64.0) as usize;
        let mut current = start;
        for step in 1..=steps {
            let t = step as f64 / steps as f64;
            let probe = [
                start[0] + (target[0] - start[0]) * t,
                current[1],
                start[2] + (target[2] - start[2]) * t,
            ];
            let next = if water_entry {
                self.supported_sized(probe, size, yaw)
            } else {
                self.ground_sized(probe, size, yaw)
            }
            .or_else(|| {
                if !water_entry || !self.clear_swimmer_rotated(probe, size, yaw) {
                    return None;
                }
                self.wet_footprint(probe, size, yaw).then_some(probe)
            })?;
            if !self.clear_step_transition(current, next, size, yaw) {
                return None;
            }
            current = next;
        }
        Some(current)
    }

    /// Lift above a riser before advancing, then descend at the destination.
    /// Checking only final poses can step vertically through a low ceiling.
    fn clear_step_transition(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        size: [f64; 3],
        yaw: f64,
    ) -> bool {
        if (start[1] - end[1]).abs() < 0.000001 {
            return true;
        }
        let height = start[1].max(end[1]);
        let raised_start = [start[0], height, start[2]];
        let raised_end = [end[0], height, end[2]];
        [
            (start, raised_start),
            (raised_start, raised_end),
            (raised_end, end),
        ]
        .into_iter()
        .all(|(from, to)| {
            let length = distance(from, to).sqrt();
            let steps = (length / 0.125).ceil().clamp(1.0, 16.0) as usize;
            (1..=steps).all(|step| {
                self.clear_swimmer_rotated(
                    std::array::from_fn(|axis| {
                        from[axis] + (to[axis] - from[axis]) * step as f64 / steps as f64
                    }),
                    size,
                    yaw,
                )
            })
        })
    }
    pub fn ground_rotated(&self, p: [f64; 3], kind: Kind, yaw: f64) -> Option<[f64; 3]> {
        self.ground_sized(p, kind.size(), yaw)
    }
    pub fn ground_sized(&self, p: [f64; 3], size: [f64; 3], yaw: f64) -> Option<[f64; 3]> {
        self.ground_profile(p, size, yaw, false)
    }
    /// Physical contact may span several descending steps while the rear feet
    /// still support the body. Path planning deliberately keeps its stricter
    /// four-foot probe so this cannot create routes across unsupported gaps.
    pub(super) fn supported_sized(
        &self,
        p: [f64; 3],
        size: [f64; 3],
        yaw: f64,
    ) -> Option<[f64; 3]> {
        self.ground_profile(p, size, yaw, true)
            .or_else(|| self.contact_ground(p, size, yaw))
    }
    fn contact_ground(&self, p: [f64; 3], size: [f64; 3], yaw: f64) -> Option<[f64; 3]> {
        let (sin, cos) = yaw.sin_cos();
        let radius = [
            (size[0] * cos.abs() + size[2] * sin.abs()) * 0.5,
            (size[0] * sin.abs() + size[2] * cos.abs()) * 0.5,
        ];
        let mut best: Option<[f64; 3]> = None;
        for x in (p[0] - radius[0]).floor() as i64..=(p[0] + radius[0] - 0.000001).floor() as i64 {
            for z in
                (p[2] - radius[1]).floor() as i64..=(p[2] + radius[1] - 0.000001).floor() as i64
            {
                for y in (p[1].floor() as i64 - 2)..=p[1].floor() as i64 {
                    let cell = [x, y, z];
                    self.block(cell)?;
                    if !self.solid(cell) {
                        continue;
                    }
                    let mut visit = |shape: terra_core::block_shapes::ShapeBox| {
                        let top = shape.maximum[1];
                        if (top - p[1]).abs() > 1.01 || best.is_some_and(|old| top <= old[1]) {
                            return;
                        }
                        let lower = [p[0] - radius[0], top - 0.001, p[2] - radius[1]];
                        let upper = [p[0] + radius[0], top + 0.001, p[2] + radius[1]];
                        let probe = [p[0], top, p[2]];
                        if Self::shape_obstructs(shape, lower, upper, (probe, size, yaw))
                            && self.clear_rotated(probe, size, false, yaw)
                        {
                            best = Some(probe);
                        }
                    };
                    if !self.boxes(cell, &mut visit) {
                        visit(terra_core::block_shapes::ShapeBox {
                            minimum: cell.map(|v| v as f64),
                            maximum: cell.map(|v| v as f64 + 1.0),
                            hinged: false,
                        });
                    }
                }
            }
        }
        best
    }
    fn ground_profile(
        &self,
        p: [f64; 3],
        size: [f64; 3],
        yaw: f64,
        partial: bool,
    ) -> Option<[f64; 3]> {
        let (sin, cos) = yaw.sin_cos();
        let half = [size[0] * 0.5 - 0.0001, size[2] * 0.5 - 0.0001];
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
                let Some(height) = found else {
                    if partial {
                        continue;
                    }
                    return None;
                };
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
            && (partial || (support - lowest) <= 1.001)
            && (support - p[1]).abs() <= 1.01
            && self.clear_rotated(next, size, false, yaw))
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
        BiomeKind::CherryForest => "cherry_forest",
        BiomeKind::SpiritBlossom => "spirit_blossom",
        BiomeKind::BloodForest => "blood_forest",
    }
}

pub fn spawn_cell(population: &mut CreaturePopulation, habitat: &Habitat, home: [i64; 2]) -> bool {
    if population.explored.contains(&home) {
        return sandworm::recover_missing_spawn(population, habitat, home);
    }
    spawn_cell_using(
        population,
        habitat,
        home,
        terra_core::creatures::definitions::get,
    )
}

/// Keep natural arrivals outside the immediate vicinity of every nearby player.
/// A deferred habitat is not marked explored and can be retried after movement.
pub fn spawn_cell_away_from_players(
    population: &mut CreaturePopulation,
    habitat: &Habitat,
    home: [i64; 2],
    players: &[[f64; 3]],
    available: usize,
) -> bool {
    if available == 0 {
        return false;
    }
    let center = [home[0] as f64 * 32.0 + 16.0, home[1] as f64 * 32.0 + 16.0];
    if players.iter().take(128).any(|player| {
        player.iter().all(|v| v.is_finite())
            && (player[0] - center[0]).hypot(player[2] - center[1]) < 40.0
    }) {
        return false;
    }
    if population.explored.contains(&home) {
        sandworm::recover_missing_spawn(population, habitat, home)
    } else {
        spawn_cell_budgeted_using(
            population,
            habitat,
            home,
            terra_core::creatures::definitions::get,
            available.min(4),
        )
    }
}

fn spawn_cell_using<'a>(
    population: &mut CreaturePopulation,
    habitat: &Habitat,
    home: [i64; 2],
    definition: impl Fn(Kind) -> &'a terra_core::creatures::definitions::MobDefinition,
) -> bool {
    spawn_cell_budgeted_using(population, habitat, home, definition, 4)
}

fn spawn_cell_budgeted_using<'a>(
    population: &mut CreaturePopulation,
    habitat: &Habitat,
    home: [i64; 2],
    definition: impl Fn(Kind) -> &'a terra_core::creatures::definitions::MobDefinition,
    available: usize,
) -> bool {
    let available = available
        .min(terra_core::creatures::MAX_CREATURES.saturating_sub(population.animals.len()));
    if population.explored.contains(&home)
        || population.explored.len() >= terra_core::creatures::MAX_SPAWN_CELLS
        || available == 0
        || creature_id(home, 0).is_none()
    {
        return false;
    }
    let mut candidates = Vec::new();
    let mut bear_habitat = false;
    for slot in 0..4 {
        if candidates.len() >= available {
            break;
        }
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
        let biome = biome_slug(habitat.terrain.surface_biome(&surface));
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
        } else if !matches!(biome, "spirit_blossom" | "blood_forest")
            && slot == 0
            && (group_seed >> 40).is_multiple_of(7)
            && bears::near_water(habitat, [x, y, z])
        {
            // Keep the existing rare solitary shore-bear slot. Ambient
            // butterflies replace herd candidates, never this habitat's bear.
            bear_habitat = true;
            Kind::Bear
        } else if let Some(kind) = sandworm::surface_species(biome, group_seed)
            .or_else(|| spirit::surface_species(biome, group_seed))
            .or_else(|| forest::surface_species(biome, group_seed))
        {
            kind
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
        } else if kind.is_flying() {
            let p = [x, y + 1.2 + (seed % 100) as f64 * 0.01, z];
            if !spawn_volume_loaded(habitat, p, kind.size()) {
                return false;
            }
            if !habitat.clear(p, kind.size(), false) {
                continue;
            }
            p
        } else if kind == Kind::SandWorm {
            let candidate = [x, y, z];
            if !spawn_volume_loaded(habitat, candidate, kind.size())
                || (1..=3).any(|depth| {
                    habitat
                        .block([x.floor() as i64, y.floor() as i64 - depth, z.floor() as i64])
                        .is_none()
                })
            {
                return false;
            }
            let Some(p) = sandworm::spawn_surface(habitat, candidate) else {
                continue;
            };
            p
        } else {
            let Some(p) =
                habitat.ground_rotated([x, y, z], kind, f64::from((seed % 628) as f32 * 0.01))
            else {
                continue;
            };
            let support = habitat.block([x.floor() as i64, p[1] as i64 - 1, z.floor() as i64]);
            if !(matches!(support, Some(1 | 2))
                || kind == Kind::Bear && support == Some(4)
                || kind == Kind::SpiritFox
                    && support == Some(terra_voxel::MaterialId::SpiritBlossomGrass as u16)
                || biome == "blood_forest"
                    && matches!(support, Some(400 | 401 | 402 | 403 | 407 | 408)))
            {
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
            behavior: if kind == Kind::SandWorm {
                Behavior::Burrowed
            } else if fish {
                Behavior::Swim
            } else if kind.is_flying() {
                Behavior::Fly
            } else {
                Behavior::Rest
            },
            timer: 1.0 + (seed % 40) as f32 * 0.1,
            age: 0.0,
            variant: if kind == Kind::SpiritGloom {
                terra_core::creatures::gloom::surface_variant(
                    biome,
                    surface.biome.temperature,
                    y,
                    seed,
                )
            } else {
                (seed % u64::from(kind.variant_count())) as u8
            },
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
    if animal.kind.is_spider() {
        spiders::step_with_navigation(animal, habitat, players, dt, search, true, navigation);
        return Forage::None;
    }
    if animal.kind.is_aquatic() && animal.kind != Kind::Fish {
        aquatics::step(animal, habitat, players, &bears::PreyIndex::default(), dt);
        return Forage::None;
    }
    if animal.kind == Kind::SandWorm {
        sandworm::step(animal, habitat, players, dt);
        return Forage::None;
    }
    if forest::hostile(animal.kind) {
        forest::step(animal, habitat, players, dt, search, navigation);
        return Forage::None;
    }
    if matches!(
        animal.kind,
        Kind::Butterfly | Kind::SpiritGloom | Kind::Raven
    ) {
        spirit::fly(animal, habitat, players, dt);
        return Forage::None;
    }
    let previous_yaw = animal.yaw;
    if animal.kind == Kind::Fish {
        animal.needs.search_in = (animal.needs.search_in - dt).max(0.0);
    }
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
        if animal.health <= 0.0 {
            return forage;
        }
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
    let nearest = perception::nearest_visible(
        habitat,
        [
            animal.position[0],
            animal.position[1] + animal.size()[1] * 0.65,
            animal.position[2],
        ],
        animal.position,
        players,
        6.0,
        |_| true,
    )
    .map(|(index, _)| &players[index]);
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
    if animal.yaw != previous_yaw {
        let desired_yaw = animal.yaw;
        // Timer/need steering proposes a turn. Sweep from the previous pose;
        // testing only its final angle misses diagonal wall intersections.
        animal.yaw = previous_yaw;
        let clear = if animal.kind == Kind::Fish && animal.behavior != Behavior::Stranded {
            aquatics::swim_step(animal, habitat, animal.position, desired_yaw)
        } else {
            habitat.clear_swimmer_rotated(animal.position, animal.size(), f64::from(desired_yaw))
        };
        if clear {
            animal.yaw = desired_yaw;
        }
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
            * animal.movement_scale()
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
            .supported_sized(animal.position, animal.size(), f64::from(animal.yaw))
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
                    habitat.ground_sized(probe, animal.size(), f64::from(animal.yaw))
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
    let swim_depth = (animal.kind == Kind::Fish && animal.behavior != Behavior::Stranded)
        .then(|| aquatics::depth_goal(animal, habitat))
        .flatten();
    let destination = animal.needs.goal.map_or_else(
        || {
            [
                animal.position[0] - f64::from(animal.yaw.sin()) * 4.0,
                swim_depth.unwrap_or(animal.position[1]),
                animal.position[2] - f64::from(animal.yaw.cos()) * 4.0,
            ]
        },
        |g| g.position.map(|v| v as f64 + 0.5),
    );
    if let Some(nav) = navigation.as_deref_mut() {
        nav.intend(
            animal,
            if !grounded {
                terra_core::creatures::intentions::IntentReason::Unsupported
            } else {
                terra_core::creatures::intentions::CreatureIntent::state(animal).reason
            },
            moving.then_some(destination),
        );
    }
    let move_to_waypoint = |waypoint| {
        let length = distance(waypoint, animal.position).sqrt().max(0.001);
        let p = std::array::from_fn(|i| {
            animal.position[i]
                + (waypoint[i] - animal.position[i]) / length * (speed * f64::from(dt)).min(length)
        });
        if animal.kind == Kind::Fish {
            let yaw = (-(waypoint[0] - animal.position[0]))
                .atan2(-(waypoint[2] - animal.position[2])) as f32;
            aquatics::swim_step(animal, habitat, p, yaw).then_some(p)
        } else {
            let yaw = (animal.position[0] - p[0]).atan2(animal.position[2] - p[2]);
            habitat.travel_step_sized(animal.position, p, animal.size(), yaw)
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
        if let Some(depth) = swim_depth {
            let vertical_step = speed * 0.65 * f64::from(dt);
            target[1] += (depth - animal.position[1]).clamp(-vertical_step, vertical_step);
        }
        aquatics::swim_step(animal, habitat, target, animal.yaw).then_some(target)
    } else if grounded {
        habitat.travel_step_sized(
            animal.position,
            target,
            animal.size(),
            f64::from(animal.yaw),
        )
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
    let fish_search =
        grounded && next.is_none() && speed > 0.0 && animal.kind == Kind::Fish && search;
    if grounded && next.is_none() && speed > 0.0 {
        let waypoint = if animal.kind == Kind::Fish {
            if search {
                navigation::waypoint(habitat, animal.position, destination, animal.kind)
            } else {
                None
            }
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
    if fish_search {
        animal.needs.search_in = 2.0;
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
                aquatics::swim_step(animal, habitat, probe, yaw).then_some(probe)
            } else {
                habitat.travel_step_sized(animal.position, probe, animal.size(), f64::from(yaw))
            };
            if candidate.is_some() {
                let mut step = animal.position;
                step[0] -= f64::from(yaw.sin()) * speed * f64::from(dt);
                step[2] -= f64::from(yaw.cos()) * speed * f64::from(dt);
                next = if animal.kind == Kind::Fish {
                    aquatics::swim_step(animal, habitat, step, yaw).then_some(step)
                } else {
                    habitat.travel_step_sized(animal.position, step, animal.size(), f64::from(yaw))
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
        if animal.kind != Kind::Fish {
            animal.needs.search_in = 2.0;
        }
        animal.timer = 2.0 + (random(animal.id, animal.age as u64) % 20) as f32 * 0.1;
    }
    let vertical = animal.velocity[1];
    animal.velocity = std::array::from_fn(|i| {
        ((animal.position[i] - before[i]) / f64::from(dt.max(0.001))) as f32
    });
    if animal.kind != Kind::Fish {
        // Walking down a stair changes support height, not ballistic velocity.
        // Gravity already produced the real falling velocity when unsupported.
        animal.velocity[1] = if next.is_some() { 0.0 } else { vertical };
        if next.is_none() {
            // A local penetration correction is not locomotion, even when it
            // moves the saved pose sideways at a high simulation frequency.
            animal.velocity[0] = 0.0;
            animal.velocity[2] = 0.0;
        }
    }
    if animal.health <= 0.0 {
        animal.behavior = Behavior::Dead;
        animal.velocity = [0.0; 3];
    }
    forage
}

/// Client targeting and authoritative interaction share the anatomical volumes;
/// navigation clearance must not turn empty space around a long body into a hit.
pub fn ray_distance(
    animal: &Creature,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<f64> {
    if animal.health <= 0.0 || animal.behavior == Behavior::InNest {
        return None;
    }
    terra_core::creatures::hitboxes::for_creature(animal).ray_distance(origin, direction, reach)
}
