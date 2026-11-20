//! Stable tomb identities and navigation landmarks. These are derived from the
//! site seed, so saves/network messages need no second mutable source of truth.
use super::{SiteSummary, StructureKind};

mod layout;
pub(super) use layout::adapted_layout;

pub const GRAVE_LEVEL_HEIGHT: i64 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraveVariant {
    Knight,
    Priestess,
    Scholar,
}
impl GraveVariant {
    pub fn levels(self) -> u8 {
        match self {
            Self::Knight => 3,
            Self::Priestess | Self::Scholar => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraveDungeonInfo {
    pub name: String,
    pub variant: GraveVariant,
    /// Feet positions, including the exterior approach and each floor landing.
    pub entrance: [i64; 3],
    pub levels: Vec<[i64; 3]>,
    /// Full, connected walkable route from the entrance to every treasury.
    /// Each step has <=1m rise/drop and a three-voxel high clear corridor.
    pub route: Vec<[i64; 3]>,
    pub treasures: Vec<[i64; 3]>,
    /// Individually bounded rooms on each floor; empty for the preserved legacy plan.
    pub floors: Vec<GraveFloor>,
}

impl SiteSummary {
    pub fn grave_dungeon(&self) -> Option<GraveDungeonInfo> {
        if let Some(info) = &self.grave {
            return Some(info.clone());
        }
        if self.kind != StructureKind::GraveDungeon {
            return None;
        }
        let variant = match (self.id >> 32) % 3 {
            0 => GraveVariant::Knight,
            1 => GraveVariant::Priestess,
            _ => GraveVariant::Scholar,
        };
        let names = match variant {
            GraveVariant::Knight => [
                "Eldric", "Valtor", "Aldren", "Caelan", "Tharic", "Roderan", "Osric", "Gareth",
            ],
            GraveVariant::Priestess => [
                "Maelis",
                "Seraphine",
                "Liora",
                "Evelis",
                "Ysara",
                "Aurelia",
                "Naeris",
                "Selene",
            ],
            GraveVariant::Scholar => [
                "Corvin", "Elowen", "Vaelor", "Orien", "Mireth", "Alaric", "Theron", "Merovan",
            ],
        };
        let name = names[(self.id.rotate_right(11) as usize) % names.len()].to_owned();
        let [x, y, z] = self.center;
        let entrance = [x, y, z - 20];
        let mut route = vec![entrance];
        let mut levels = Vec::new();
        let mut treasures = Vec::new();
        for level in 0..variant.levels() {
            let top = y - i64::from(level) * GRAVE_LEVEL_HEIGHT;
            let side = if level % 2 == 0 { -10 } else { 10 };
            connect(&mut route, [x, top, z - 10]);
            connect(&mut route, [x + side, top, z - 10]);
            for step in 1..=GRAVE_LEVEL_HEIGHT {
                route.push([x + side, top - step, z - 10 + step]);
            }
            let bottom = top - GRAVE_LEVEL_HEIGHT;
            levels.push([x + side, bottom, z - 2]);
            connect(&mut route, [x, bottom, z - 2]);
            connect(&mut route, [x, bottom, z + 9]);
            connect(&mut route, [x + 5, bottom, z + 9]);
            treasures.push([x + 7, bottom, z + 9]);
            // Return to the central aisle before the next stair flight.
            connect(&mut route, [x, bottom, z + 9]);
        }
        Some(GraveDungeonInfo {
            name,
            variant,
            entrance,
            levels,
            route,
            treasures,
            floors: Vec::new(),
        })
    }
}

fn connect(route: &mut Vec<[i64; 3]>, target: [i64; 3]) {
    let mut cursor = route.last().copied().unwrap_or(target);
    // Floors are horizontal; vertical changes belong only to explicit stairs.
    debug_assert_eq!(cursor[1], target[1]);
    for axis in [2, 0] {
        while cursor[axis] != target[axis] {
            cursor[axis] += (target[axis] - cursor[axis]).signum();
            route.push(cursor);
        }
    }
}

impl crate::generation::TerraGenerator {
    /// Cheap locate/map prefilter. The full planner still checks dry ground,
    /// footprint biome weights, slope and allocation budgets before acceptance.
    pub fn grave_site_candidate(&self, region: [i64; 2]) -> Option<StructureKind> {
        let config = self.config();
        if config.spirit_blossom_revision < 2
            || config.structures_revision == 0
            || !config.structures.ruins
            || region.iter().any(|v| v.unsigned_abs() > 2500)
        {
            return None;
        }
        let seed = super::regional_seed(config.seed.0, region);
        if seed % 5 == 4 {
            return None;
        }
        let x = region[0] * super::REGION_SIZE + 192 + i64::from((seed >> 7) % 33) - 16;
        let z = region[1] * super::REGION_SIZE + 192 + i64::from((seed >> 15) % 33) - 16;
        if self
            .climate_biome_at(x as f64, z as f64)
            .weights
            .spirit_blossom
            < 0.78
        {
            return None;
        }
        Some(if seed.is_multiple_of(4) {
            StructureKind::Cemetery
        } else {
            StructureKind::GraveDungeon
        })
    }
}

/// Gameplay-facing architectural purpose, without implying unimplemented quests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraveRoomPurpose {
    GuardHall,
    Armory,
    Ossuary,
    FamilyCrypt,
    BurialHall,
    ProcessionalHall,
    Cloister,
    Reliquary,
    Sanctum,
    Library,
    Archive,
    Study,
    CodexVault,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraveRoom {
    pub purpose: GraveRoomPurpose,
    /// Inclusive floor-space bounds; Y denotes clear walking level through ceiling.
    pub bounds: [[i64; 3]; 2],
}
impl GraveRoom {
    pub fn center(&self) -> [i64; 3] {
        [
            (self.bounds[0][0] + self.bounds[1][0]) / 2,
            self.bounds[0][1],
            (self.bounds[0][2] + self.bounds[1][2]) / 2,
        ]
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraveFloor {
    pub rooms: Vec<GraveRoom>,
    pub landing: [i64; 3],
    /// A clear interior vantage in the primary room, also used by native acceptance captures.
    pub view_eye: [i64; 3],
    pub view_target: [i64; 3],
}
