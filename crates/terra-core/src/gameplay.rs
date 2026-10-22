//! Small authoritative inventories and persistent loose resources.
use crate::PlayerUuid;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

mod inventory_ops;

pub const INVENTORY_SLOTS: usize = 27;
pub const STACK_LIMIT: u16 = 128;
pub const DROP_LIFETIME_SECONDS: f32 = 300.0;
pub const MAX_WORLD_DROPS: usize = 2048;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
    Spectator,
}
impl GameMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Survival => "Survival",
            Self::Creative => "Creative",
            Self::Spectator => "Spectator",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourcePropKind {
    Grass,
    Flower,
    Bush,
    Fern,
    Mushroom,
    SmallPlant,
    BeeNest,
    Seagrass,
    SeaCucumber,
    BranchingCoral,
    TubeCoral,
    PlateCoral,
    SeaAnemone,
    SmallStones,
}
impl ResourcePropKind {
    pub const ALL: [Self; 14] = [
        Self::Grass,
        Self::Flower,
        Self::Bush,
        Self::Fern,
        Self::Mushroom,
        Self::SmallPlant,
        Self::BeeNest,
        Self::Seagrass,
        Self::SeaCucumber,
        Self::BranchingCoral,
        Self::TubeCoral,
        Self::PlateCoral,
        Self::SeaAnemone,
        Self::SmallStones,
    ];
    pub fn slug(self) -> &'static str {
        match self {
            Self::Grass => "grass_tuft",
            Self::Flower => "flower",
            Self::Bush => "bush",
            Self::Fern => "fern",
            Self::Mushroom => "mushroom",
            Self::SmallPlant => "small_plant",
            Self::BeeNest => "bee_nest",
            Self::Seagrass => "seagrass",
            Self::SeaCucumber => "sea_cucumber",
            Self::BranchingCoral => "branching_coral",
            Self::TubeCoral => "tube_coral",
            Self::PlateCoral => "plate_coral",
            Self::SeaAnemone => "sea_anemone",
            Self::SmallStones => "small_stones",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Grass => "Grass",
            Self::Flower => "Flower",
            Self::Bush => "Bush",
            Self::Fern => "Fern",
            Self::Mushroom => "Mushroom",
            Self::SmallPlant => "Small plant",
            Self::BeeNest => "Bee nest",
            Self::Seagrass => "Seagrass",
            Self::SeaCucumber => "Sea cucumber",
            Self::BranchingCoral => "Branching coral",
            Self::TubeCoral => "Tube coral",
            Self::PlateCoral => "Plate coral",
            Self::SeaAnemone => "Sea anemone",
            Self::SmallStones => "Small stones",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Block(u16),
    Prop(ResourcePropKind),
    Tool(ToolKind),
    Stick,
    WoodenBlock {
        material: u16,
        wood: WoodSpecies,
    },
    Mineral(crate::MineralItem),
    Animal(crate::creatures::AnimalProduct),
    Bellows(BellowsKind),
    Dye(crate::beds::DyeColor),
    DyedBed {
        material: u16,
        color: crate::beds::DyeColor,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BellowsKind {
    Stone,
    Iron,
    Gold,
    Ruby,
}
impl BellowsKind {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Stone => "stone_bellows",
            Self::Iron => "iron_bellows",
            Self::Gold => "gold_bellows",
            Self::Ruby => "ruby_bellows",
        }
    }
    pub const fn speed_multiplier(self) -> f32 {
        match self {
            Self::Stone | Self::Gold => 1.10,
            Self::Iron => 1.20,
            Self::Ruby => 1.15,
        }
    }
    pub const fn double_chance(self) -> f32 {
        match self {
            Self::Gold => 0.05,
            Self::Ruby => 0.075,
            _ => 0.0,
        }
    }
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum WoodSpecies {
    #[default]
    Oak,
    Birch,
    Spruce,
}
impl WoodSpecies {
    pub fn tint(self) -> [f32; 4] {
        match self {
            Self::Oak => [1.0, 1.0, 1.0, 1.0],
            // Compensate the warm oak grain in linear space without new materials.
            Self::Birch => [1.35, 3.6, 16.0, 1.0],
            Self::Spruce => [0.57, 0.63, 0.64, 1.0],
        }
    }
    pub fn slug(self) -> &'static str {
        match self {
            Self::Oak => "oak",
            Self::Birch => "birch",
            Self::Spruce => "spruce",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    WoodPickaxe,
    StonePickaxe,
    StoneAxe,
    StoneShovel,
    IronPickaxe,
    WoodAxe,
    WoodShovel,
    WoodSword,
    StoneSword,
}
impl ToolKind {
    pub fn slug(self) -> &'static str {
        match self {
            Self::WoodPickaxe => "wood_pickaxe",
            Self::StonePickaxe => "stone_pickaxe",
            Self::StoneAxe => "stone_axe",
            Self::StoneShovel => "stone_shovel",
            Self::IronPickaxe => "iron_pickaxe",
            Self::WoodAxe => "wood_axe",
            Self::WoodShovel => "wood_shovel",
            Self::WoodSword => "wood_sword",
            Self::StoneSword => "stone_sword",
        }
    }
    pub fn speed(self, material: u16) -> f32 {
        match (self, material) {
            (Self::WoodPickaxe, 3 | 11..=13 | 30..=35 | 37..=39 | 43) => 2.0,
            (Self::StonePickaxe, 3 | 11..=13 | 30..=35 | 37..=39 | 43) => 4.0,
            (Self::IronPickaxe, 3 | 11..=13 | 30..=35 | 37..=39 | 43) => 6.0,
            (Self::StoneAxe, 6 | 10 | 16..=18 | 28..=29 | 36) => 4.0,
            (Self::WoodAxe, 6 | 10 | 16..=18 | 28..=29 | 36) => 2.0,
            (Self::StoneShovel, 1..=2 | 4..=5 | 19 | 42 | 48) => 4.0,
            (Self::WoodShovel, 1..=2 | 4..=5 | 19 | 42 | 48) => 2.0,
            _ => 1.0,
        }
    }
}
impl ItemKind {
    pub fn block_material(self) -> Option<u16> {
        match self {
            Self::Block(material)
            | Self::WoodenBlock { material, .. }
            | Self::DyedBed { material, .. } => Some(material),
            Self::Prop(ResourcePropKind::Seagrass) => Some(50),
            Self::Mineral(crate::MineralItem::SaltCrystal) => Some(51),
            _ => None,
        }
    }
    pub fn wood_species(self) -> Option<WoodSpecies> {
        match self {
            Self::Block(6 | 16 | 28 | 29) => Some(WoodSpecies::Oak),
            Self::Block(10 | 17) => Some(WoodSpecies::Birch),
            Self::Block(18 | 36) => Some(WoodSpecies::Spruce),
            Self::WoodenBlock { wood, .. } => Some(wood),
            _ => None,
        }
    }
    pub fn for_wood(material: u16, wood: WoodSpecies) -> Self {
        if wood == WoodSpecies::Oak || !matches!(material, 28..=29) {
            Self::Block(material)
        } else {
            Self::WoodenBlock { material, wood }
        }
    }
    pub const EXTRA: [Self; 41] = [
        Self::Dye(crate::beds::DyeColor::Red),
        Self::Dye(crate::beds::DyeColor::Yellow),
        Self::Dye(crate::beds::DyeColor::Blue),
        Self::Dye(crate::beds::DyeColor::Orange),
        Self::Dye(crate::beds::DyeColor::Green),
        Self::Dye(crate::beds::DyeColor::Purple),
        Self::Animal(crate::creatures::AnimalProduct::Mutton),
        Self::Animal(crate::creatures::AnimalProduct::Beef),
        Self::Animal(crate::creatures::AnimalProduct::Pork),
        Self::Animal(crate::creatures::AnimalProduct::Fish),
        Self::Animal(crate::creatures::AnimalProduct::Wool),
        Self::Stick,
        Self::Tool(ToolKind::WoodPickaxe),
        Self::Tool(ToolKind::StonePickaxe),
        Self::Tool(ToolKind::StoneAxe),
        Self::Tool(ToolKind::StoneShovel),
        Self::Tool(ToolKind::IronPickaxe),
        Self::Mineral(crate::MineralItem::SaltCrystal),
        Self::Mineral(crate::MineralItem::RawIron),
        Self::Mineral(crate::MineralItem::Coal),
        Self::Mineral(crate::MineralItem::RawGold),
        Self::Mineral(crate::MineralItem::Diamond),
        Self::Mineral(crate::MineralItem::Ruby),
        Self::Mineral(crate::MineralItem::IronIngot),
        Self::Mineral(crate::MineralItem::GoldIngot),
        Self::Animal(crate::creatures::AnimalProduct::CookedBeef),
        Self::Animal(crate::creatures::AnimalProduct::CookedPork),
        Self::Animal(crate::creatures::AnimalProduct::CookedMutton),
        Self::Animal(crate::creatures::AnimalProduct::CookedFish),
        Self::Animal(crate::creatures::AnimalProduct::Clownfish),
        Self::Animal(crate::creatures::AnimalProduct::CookedClownfish),
        Self::Animal(crate::creatures::AnimalProduct::Leather),
        Self::Animal(crate::creatures::AnimalProduct::Honey),
        Self::Tool(ToolKind::WoodAxe),
        Self::Tool(ToolKind::WoodShovel),
        Self::Tool(ToolKind::WoodSword),
        Self::Tool(ToolKind::StoneSword),
        Self::Bellows(BellowsKind::Stone),
        Self::Bellows(BellowsKind::Iron),
        Self::Bellows(BellowsKind::Gold),
        Self::Bellows(BellowsKind::Ruby),
    ];
    pub fn can_harvest(self, material: u16) -> bool {
        if let Some(ore) = crate::OreKind::from_material(material) {
            let level = match self {
                Self::Tool(ToolKind::WoodPickaxe) => 1,
                Self::Tool(ToolKind::StonePickaxe) => 2,
                Self::Tool(ToolKind::IronPickaxe) => 3,
                _ => 0,
            };
            return level >= ore.mining_level();
        }
        !matches!(material, 12..=13)
            || matches!(
                self,
                Self::Tool(ToolKind::WoodPickaxe | ToolKind::StonePickaxe | ToolKind::IronPickaxe)
            )
    }
    pub fn identifier(self) -> String {
        match self {
            Self::Dye(color) => format!("teramic::{}_dye", color.slug()),
            Self::DyedBed { material, color } => {
                format!("{}#{}", Self::Block(material).identifier(), color.slug())
            }
            Self::WoodenBlock { material, wood } => {
                format!("{}#{}", Self::Block(material).identifier(), wood.slug())
            }
            Self::Tool(tool) => format!("teramic::{}", tool.slug()),
            Self::Stick => "teramic::stick".into(),
            Self::Mineral(mineral) => format!("teramic::{}", mineral.slug()),
            Self::Animal(product) => format!("teramic::{}", product.slug()),
            Self::Bellows(kind) => format!("teramic::{}", kind.slug()),
            Self::Block(id) => terra_voxel::MaterialId::from_raw(id).map_or_else(
                || format!("teramic::custom_{id}"),
                |m| m.identifier().to_owned(),
            ),
            Self::Prop(kind) => format!("teramic::{}", kind.slug()),
        }
    }
    pub fn valid(self) -> bool {
        match self {
            Self::Dye(_) => true,
            Self::DyedBed { material, .. } => crate::beds::is_bed(material),
            Self::WoodenBlock { material, wood } => {
                matches!(material, 28..=29) && wood != WoodSpecies::Oak
            }
            Self::Block(id) => terra_voxel::MaterialId::from_raw(id)
                .is_some_and(|material| material != terra_voxel::MaterialId::Air),
            Self::Prop(_)
            | Self::Tool(_)
            | Self::Stick
            | Self::Mineral(_)
            | Self::Animal(_)
            | Self::Bellows(_) => true,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub item: ItemKind,
    pub count: u16,
}
impl ItemStack {
    pub const EMPTY: Self = Self {
        item: ItemKind::Block(0),
        count: 0,
    };
    pub fn valid(self) -> bool {
        self.item.valid() && (1..=STACK_LIMIT).contains(&self.count)
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub slots: Vec<ItemStack>,
    #[serde(default)]
    pub cursor: Option<ItemStack>,
    #[serde(default)]
    pub revision: u64,
}
impl Inventory {
    pub fn valid(&self) -> bool {
        self.slots.len() <= INVENTORY_SLOTS
            && self.cursor.is_none_or(ItemStack::valid)
            && self
                .slots
                .iter()
                .all(|s| *s == ItemStack::EMPTY || s.valid())
    }
    pub fn slot(&self, index: usize) -> Option<ItemStack> {
        self.slots.get(index).copied().filter(|s| s.valid())
    }
    pub fn occupied_slots(&self) -> usize {
        self.slots.iter().filter(|s| s.valid()).count()
    }
    pub fn has_capacity(&self, item: ItemKind) -> bool {
        self.occupied_slots() < INVENTORY_SLOTS
            || self
                .slots
                .iter()
                .any(|s| s.item == item && s.count < STACK_LIMIT)
    }
    fn changed(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }
    fn set(&mut self, index: usize, stack: ItemStack) {
        self.slots
            .resize(self.slots.len().max(index + 1), ItemStack::EMPTY);
        self.slots[index] = stack;
    }
    pub fn creative_set(&mut self, slot: usize, stack: ItemStack) -> bool {
        if slot >= INVENTORY_SLOTS || !stack.valid() || self.slot(slot) == Some(stack) {
            return false;
        }
        self.set(slot, stack);
        self.changed();
        true
    }
    /// Stable slot indices: merge equal items, or swap complete unlike stacks.
    pub fn move_slot(&mut self, from: usize, to: usize, count: u16) -> bool {
        if from == to || from >= INVENTORY_SLOTS || to >= INVENTORY_SLOTS || count == 0 {
            return false;
        }
        let Some(mut source) = self.slot(from) else {
            return false;
        };
        if count > source.count {
            return false;
        }
        let target = self.slot(to);
        if let Some(target) = target
            && target.item != source.item
        {
            if count != source.count {
                return false;
            }
            self.set(from, target);
            self.set(to, source);
        } else {
            let existing = target.map_or(0, |s| s.count);
            let moved = count.min(STACK_LIMIT - existing);
            if moved == 0 {
                return false;
            }
            source.count -= moved;
            self.set(
                to,
                ItemStack {
                    item: source.item,
                    count: existing + moved,
                },
            );
            self.set(
                from,
                if source.count == 0 {
                    ItemStack::EMPTY
                } else {
                    source
                },
            );
        }
        self.changed();
        true
    }
    pub fn count(&self, item: ItemKind) -> u16 {
        self.slots
            .iter()
            .filter(|s| s.item == item)
            .map(|s| s.count)
            .sum()
    }
    /// Returns the unaccepted amount; full inventories never destroy resources.
    pub fn insert(&mut self, item: ItemKind, mut count: u16) -> u16 {
        if !item.valid() {
            return count;
        }
        let original = count;
        for stack in self
            .slots
            .iter_mut()
            .filter(|s| s.item == item && s.valid())
        {
            let amount = count.min(STACK_LIMIT - stack.count);
            stack.count += amount;
            count -= amount;
        }
        for slot in 0..INVENTORY_SLOTS {
            if count == 0 {
                break;
            }
            if self.slot(slot).is_some() {
                continue;
            }
            let amount = count.min(STACK_LIMIT);
            self.set(
                slot,
                ItemStack {
                    item,
                    count: amount,
                },
            );
            count -= amount;
        }
        if count != original {
            self.changed();
        }
        count
    }
    pub fn take_one(&mut self, item: ItemKind) -> bool {
        let Some(index) = self
            .slots
            .iter()
            .position(|s| s.item == item && s.count > 0)
        else {
            return false;
        };
        self.take_from(index, item)
    }
    pub fn take_from(&mut self, index: usize, item: ItemKind) -> bool {
        if self.slot(index).is_none_or(|stack| stack.item != item) {
            return false;
        }
        self.slots[index].count -= 1;
        if self.slots[index].count == 0 {
            self.slots[index] = ItemStack::EMPTY;
        }
        self.changed();
        true
    }
    pub fn craft(&mut self, slots: [Option<u8>; 4]) -> bool {
        crate::crafting::RecipeBook::builtins().craft_cells(
            self,
            slots,
            crate::crafting::CraftDestination::Inventory,
        )
    }
}

/// Each recipe cell consumes one resource, including repeated references to a stack.
pub fn crafting_preview(bag: &Inventory, cells: [Option<u8>; 4]) -> Option<ItemStack> {
    let book = crate::crafting::RecipeBook::builtins();
    let (key, _) = book.grid_match(bag, cells)?;
    Some(book.get(key)?.output)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldDrop {
    pub id: u64,
    pub position: [f64; 3],
    pub stack: ItemStack,
    #[serde(default)]
    pub velocity: [f32; 3],
    #[serde(default)]
    pub age_seconds: f32,
    /// Quaternion in XYZW order, shared by snapshots and the item renderer.
    #[serde(default = "identity_rotation")]
    pub rotation: [f32; 4],
    #[serde(default)]
    pub angular_velocity: [f32; 3],
    #[serde(default)]
    pub grounded: bool,
    #[serde(default)]
    pub pickup_delay_seconds: f32,
}
fn identity_rotation() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}
impl WorldDrop {
    pub fn valid(&self) -> bool {
        self.id != 0
            && self.stack.valid()
            && self
                .velocity
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 100.0)
            && self.age_seconds.is_finite()
            && self.rotation.iter().all(|v| v.is_finite())
            && (self.rotation.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 0.01
            && self
                .angular_velocity
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 100.0)
            && self.pickup_delay_seconds.is_finite()
            && (0.0..=10.0).contains(&self.pickup_delay_seconds)
            && (0.0..=DROP_LIFETIME_SECONDS).contains(&self.age_seconds)
            && self
                .position
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerStats {
    pub game_mode: Option<GameMode>,
    pub blocks_broken: u64,
    pub blocks_placed: u64,
    pub props_harvested: u64,
    pub deaths: u64,
    pub survival: crate::SurvivalVitals,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GameplayState {
    pub creatures: crate::creatures::CreaturePopulation,
    pub inventories: BTreeMap<PlayerUuid, Inventory>,
    pub player_stats: BTreeMap<PlayerUuid, PlayerStats>,
    pub drops: Vec<WorldDrop>,
    pub next_drop: u64,
    pub revision: u64,
}
impl GameplayState {
    pub fn valid(&self) -> bool {
        let mut ids = HashSet::new();
        self.creatures.valid()
            && self.inventories.len() <= 4096
            && self.player_stats.len() <= 4096
            && self
                .player_stats
                .iter()
                .all(|(id, stats)| !id.is_nil() && stats.survival.valid())
            && self
                .inventories
                .iter()
                .all(|(id, bag)| !id.is_nil() && bag.valid())
            && self.drops.len() <= MAX_WORLD_DROPS
            && self
                .drops
                .iter()
                .all(|d| d.valid() && d.id <= self.next_drop && ids.insert(d.id))
    }
    pub fn can_drop(&self) -> bool {
        self.drops.len() < MAX_WORLD_DROPS && self.next_drop < u64::MAX
    }
    pub fn drop_one(&mut self, position: [f64; 3], item: ItemKind) -> bool {
        self.drop_stack(
            position,
            ItemStack { item, count: 1 },
            [0.0, 1.8, 0.0],
            0.35,
        )
    }
    pub fn drop_stack(
        &mut self,
        position: [f64; 3],
        stack: ItemStack,
        velocity: [f32; 3],
        pickup_delay_seconds: f32,
    ) -> bool {
        if !self.can_drop()
            || !stack.valid()
            || velocity.iter().any(|v| !v.is_finite() || v.abs() > 100.0)
            || !pickup_delay_seconds.is_finite()
            || !(0.0..=10.0).contains(&pickup_delay_seconds)
            || position
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        {
            return false;
        }
        self.next_drop += 1;
        self.drops.push(WorldDrop {
            id: self.next_drop,
            position,
            stack,
            velocity,
            age_seconds: 0.0,
            rotation: identity_rotation(),
            angular_velocity: [2.4, (self.next_drop % 11) as f32 * 0.7 + 0.4, -1.7],
            grounded: false,
            pickup_delay_seconds,
        });
        self.revision = self.revision.saturating_add(1);
        true
    }
    pub fn pickup(&mut self, player: PlayerUuid, id: u64) -> bool {
        if player.is_nil()
            || (!self.inventories.contains_key(&player) && self.inventories.len() >= 4096)
        {
            return false;
        }
        let Some(index) = self.drops.iter().position(|d| d.id == id) else {
            return false;
        };
        let stack = self.drops[index].stack;
        let remaining = self
            .inventories
            .entry(player)
            .or_default()
            .insert(stack.item, stack.count);
        if remaining == stack.count {
            return false;
        }
        if remaining == 0 {
            self.drops.remove(index);
        } else {
            self.drops[index].stack.count = remaining;
        }
        self.revision = self.revision.saturating_add(1);
        true
    }
    pub fn consume(&mut self, player: PlayerUuid, item: ItemKind) -> bool {
        self.consume_from(player, item, None)
    }
    pub fn consume_from(&mut self, player: PlayerUuid, item: ItemKind, slot: Option<u8>) -> bool {
        if self
            .inventories
            .get_mut(&player)
            .is_some_and(|bag| match slot {
                Some(slot) => bag.take_from(usize::from(slot), item),
                None => bag.take_one(item),
            })
        {
            self.revision = self.revision.saturating_add(1);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeable_salt_segments_do_not_shadow_the_existing_harvest_resource_identifier() {
        assert_ne!(
            super::ItemKind::Block(51).identifier(),
            super::ItemKind::Mineral(crate::MineralItem::SaltCrystal).identifier()
        );
    }
    use super::*;
    #[test]
    fn slot_indices_survive_moves_merges_empty_cells_and_legacy_toml() {
        let mut bag: Inventory =
            toml::from_str("slots = [{item = {block = 3}, count = 64}]").unwrap();
        assert_eq!(bag.revision, 0);
        assert!(bag.move_slot(0, 8, 32));
        assert_eq!(bag.slot(8).unwrap().count, 32);
        assert!(bag.move_slot(0, 8, 32));
        assert!(bag.slot(0).is_none());
        assert_eq!(bag.slot(8).unwrap().count, 64);
        assert_eq!(bag.insert(ItemKind::Block(3), 100), 0);
        assert_eq!(bag.slot(8).unwrap().count, 128);
        assert_eq!(bag.slot(0).unwrap().count, 36);
        assert!(bag.move_slot(0, 26, 36));
        let encoded = toml::to_string(&bag).unwrap();
        assert_eq!(toml::from_str::<Inventory>(&encoded).unwrap(), bag);
        assert!(bag.valid());
        assert!(!bag.move_slot(26, 27, 1));
        assert!(!bag.move_slot(26, 0, 37));
    }

    #[test]
    fn crafting_consumes_actual_resources_and_is_atomic_when_full() {
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Block(6), 2);
        assert_eq!(
            crafting_preview(&bag, [Some(0), None, None, None])
                .unwrap()
                .count,
            4
        );
        assert!(bag.craft([Some(0), None, None, None]));
        assert_eq!(bag.count(ItemKind::Block(6)), 1);
        assert_eq!(bag.count(ItemKind::Block(16)), 4);
        let mut sand = Inventory::default();
        sand.insert(ItemKind::Block(4), 3);
        assert!(crafting_preview(&sand, [Some(0); 4]).is_none());
        sand.insert(ItemKind::Block(4), 1);
        assert!(sand.craft([Some(0); 4]));
        assert_eq!(sand.count(ItemKind::Block(4)), 0);
        assert_eq!(sand.count(ItemKind::Block(11)), 1);
        let mut full = Inventory::default();
        for slot in 0..INVENTORY_SLOTS {
            full.creative_set(
                slot,
                ItemStack {
                    item: ItemKind::Block(3),
                    count: 128,
                },
            );
        }
        full.creative_set(
            0,
            ItemStack {
                item: ItemKind::Block(6),
                count: 2,
            },
        );
        let before = full.clone();
        assert!(!full.craft([Some(0), None, None, None]));
        assert_eq!(full, before);
    }

    #[test]
    fn old_drop_snapshots_get_stationary_unexpired_defaults() {
        let drop: WorldDrop =
            toml::from_str("id=1\nposition=[1.0,2.0,3.0]\n[stack]\nitem={block=3}\ncount=1")
                .unwrap();
        assert_eq!(drop.velocity, [0.0; 3]);
        assert_eq!(drop.age_seconds, 0.0);
        assert_eq!(drop.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(drop.angular_velocity, [0.0; 3]);
        assert_eq!(drop.pickup_delay_seconds, 0.0);
        assert!(drop.valid());
    }
    #[test]
    fn all_registered_block_items_roundtrip_without_changing_ids() {
        let mut state = GameplayState::default();
        for id in 1..=terra_voxel::MaterialId::LAST {
            let uuid = PlayerUuid::from_u128(u128::from((id - 1) / INVENTORY_SLOTS as u16) + 1);
            assert!(state.drop_one([1.0, 2.0, 3.0], ItemKind::Block(id)));
            assert!(state.pickup(uuid, u64::from(id)));
        }
        assert!(state.valid());
        assert!(!ItemKind::Block(0).valid());
        assert!(!ItemKind::Block(terra_voxel::MaterialId::LAST + 1).valid());
        let encoded = toml::to_string(&state).unwrap();
        let restored: GameplayState = toml::from_str(&encoded).unwrap();
        assert_eq!(state, restored);
    }
    #[test]
    fn full_inventory_keeps_drop_and_partial_pickup_keeps_remainder() {
        let uuid = PlayerUuid::new_v4();
        let item = ItemKind::Block(3);
        let mut state = GameplayState::default();
        let mut bag = Inventory::default();
        let capacity = INVENTORY_SLOTS as u16 * STACK_LIMIT;
        assert_eq!(bag.insert(item, capacity), 0);
        state.inventories.insert(uuid, bag);
        assert!(state.drop_one([0.0; 3], item));
        let id = state.drops[0].id;
        assert!(!state.pickup(uuid, id));
        assert!(state.consume(uuid, item));
        state.drops[0].stack.count = 2;
        assert!(state.pickup(uuid, id));
        assert_eq!(state.drops[0].stack.count, 1);
        assert_eq!(state.inventories[&uuid].count(item), capacity);
        assert!(!state.pickup(uuid, id));
        assert!(state.valid());
    }
    #[test]
    fn duplicate_pickup_cannot_duplicate_resources() {
        let uuid = PlayerUuid::new_v4();
        let mut state = GameplayState::default();
        let item = ItemKind::Prop(ResourcePropKind::Flower);
        assert!(state.drop_one([2.0, 3.0, 4.0], item));
        assert!(state.pickup(uuid, 1));
        assert!(!state.pickup(uuid, 1));
        assert_eq!(state.inventories[&uuid].count(item), 1);
        assert!(state.drop_one([2.0, 3.0, 4.0], item));
        assert_eq!(state.drops[0].id, 2);
    }
}
