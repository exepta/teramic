//! Bounded mechanical crushing; shared rules, authority-owned elapsed time and private slots.
use crate::{INVENTORY_SLOTS, Inventory, ItemKind, ItemStack};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const MATERIAL: u16 = 99;
pub const CRANK_MATERIAL: u16 = 100;
pub const HAND_CRANK_MATERIAL: u16 = CRANK_MATERIAL;
pub const INPUT: usize = 0;
pub const OUTPUT: usize = 1;
pub const SLOTS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DustKind {
    Iron,
    Gold,
    Coal,
    Diamond,
    Salt,
    Ruby,
}
impl DustKind {
    pub const ALL: [Self; 6] = [
        Self::Iron,
        Self::Gold,
        Self::Coal,
        Self::Diamond,
        Self::Salt,
        Self::Ruby,
    ];
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Iron => "iron_dust",
            Self::Gold => "gold_dust",
            Self::Coal => "coal_dust",
            Self::Diamond => "diamond_dust",
            Self::Salt => "salt_dust",
            Self::Ruby => "ruby_dust",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrusherRecipe {
    pub id: u64,
    pub key: String,
    pub name: String,
    pub input: ItemKind,
    pub output: ItemStack,
    pub seconds: f32,
}
impl CrusherRecipe {
    pub fn valid(&self) -> bool {
        self.id > 0
            && self.key.starts_with("teramic::")
            && !self.name.is_empty()
            && self.input.valid()
            && self.output.valid()
            && self.input != self.output.item
            && self.seconds.is_finite()
            && (0.5..=600.0).contains(&self.seconds)
    }
}
const RECIPES: [(&str, &str); 19] = [
    (
        "pink_flower",
        include_str!("../../../assets/crushing/pink_flower.json"),
    ),
    (
        "black_dye",
        include_str!("../../../assets/crushing/black_dye.json"),
    ),
    (
        "white_dye",
        include_str!("../../../assets/crushing/white_dye.json"),
    ),
    ("iron", include_str!("../../../assets/crushing/iron.json")),
    ("gold", include_str!("../../../assets/crushing/gold.json")),
    ("coal", include_str!("../../../assets/crushing/coal.json")),
    (
        "diamond",
        include_str!("../../../assets/crushing/diamond.json"),
    ),
    ("salt", include_str!("../../../assets/crushing/salt.json")),
    ("ruby", include_str!("../../../assets/crushing/ruby.json")),
    (
        "red_dye",
        include_str!("../../../assets/crushing/red_dye.json"),
    ),
    (
        "red_flower",
        include_str!("../../../assets/crushing/red_flower.json"),
    ),
    (
        "yellow_flower",
        include_str!("../../../assets/crushing/yellow_flower.json"),
    ),
    (
        "white_flower",
        include_str!("../../../assets/crushing/white_flower.json"),
    ),
    (
        "blue_flower",
        include_str!("../../../assets/crushing/blue_flower.json"),
    ),
    (
        "purple_flower",
        include_str!("../../../assets/crushing/purple_flower.json"),
    ),
    (
        "orange_flower",
        include_str!("../../../assets/crushing/orange_flower.json"),
    ),
    (
        "green_flower",
        include_str!("../../../assets/crushing/green_flower.json"),
    ),
    (
        "iron_ingot",
        include_str!("../../../assets/crushing/iron_ingot.json"),
    ),
    (
        "gold_ingot",
        include_str!("../../../assets/crushing/gold_ingot.json"),
    ),
];
pub const fn flower_color(variant: u8) -> crate::beds::DyeColor {
    use crate::beds::DyeColor::*;
    [Red, Yellow, White, Blue, Pink, Purple, Orange, Green][(variant % 8) as usize]
}
pub const fn flower_variant(color: crate::beds::DyeColor) -> u8 {
    use crate::beds::DyeColor::*;
    match color {
        Red => 0,
        Yellow => 1,
        White => 2,
        Blue => 3,
        Pink => 4,
        Purple => 5,
        Orange => 6,
        Green => 7,
        Black | Lime | Gold | Silver | LightBlue => 0,
    }
}
pub fn recipes() -> &'static [CrusherRecipe] {
    static BOOK: OnceLock<Vec<CrusherRecipe>> = OnceLock::new();
    BOOK.get_or_init(|| {
        RECIPES
            .into_iter()
            .map(|(name, source)| {
                let builtin: CrusherRecipe =
                    serde_json::from_str(source).expect("embedded crusher recipe");
                assert!(builtin.valid());
                std::fs::read_to_string(
                    crate::asset_directory()
                        .join("crushing")
                        .join(format!("{name}.json")),
                )
                .ok()
                .and_then(|source| serde_json::from_str::<CrusherRecipe>(&source).ok())
                .filter(|recipe| {
                    recipe.valid()
                        && recipe.id == builtin.id
                        && recipe.key == builtin.key
                        && recipe.input == builtin.input
                })
                .unwrap_or(builtin)
            })
            .collect()
    })
}
pub fn recipe_for(item: ItemKind) -> Option<&'static CrusherRecipe> {
    recipes().iter().find(|recipe| recipe.input == item)
}
pub fn catalog_fingerprint() -> u64 {
    static HASH: OnceLock<u64> = OnceLock::new();
    *HASH.get_or_init(|| {
        serde_json::to_vec(recipes())
            .expect("finite crusher recipes")
            .into_iter()
            .fold(0xcbf29ce484222325, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            })
    })
}
pub fn slot_capacity(slot: usize, item: ItemKind) -> u16 {
    if slot == INPUT && recipe_for(item).is_some() {
        item.max_stack()
    } else {
        0
    }
}
pub fn valid_inventory(inventory: &Inventory) -> bool {
    inventory.valid()
        && inventory.cursor.is_none()
        && inventory.slots.len() <= SLOTS
        && inventory.slots.iter().enumerate().all(|(slot, stack)| {
            *stack == ItemStack::EMPTY
                || if slot == OUTPUT {
                    recipes()
                        .iter()
                        .any(|recipe| recipe.output.item == stack.item)
                } else {
                    stack.count <= slot_capacity(slot, stack.item)
                }
        })
}
pub fn apply_gesture_reserved(
    player: &mut Inventory,
    inventory: &mut Inventory,
    gesture: crate::containers::ContainerGesture,
    reserved: [u16; INVENTORY_SLOTS],
) -> bool {
    crate::containers::apply_crusher_filtered(
        player,
        std::slice::from_mut(inventory),
        gesture,
        reserved,
    )
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrusherState {
    pub progress_seconds: f32,
    pub recipe: Option<u64>,
    pub completed: u64,
}
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrusherView {
    pub progress: f32,
    pub duration: f32,
    pub recipe: Option<u64>,
    pub working: bool,
}
impl CrusherView {
    pub fn valid(&self) -> bool {
        self.progress.is_finite()
            && (0.0..=1.0).contains(&self.progress)
            && self.duration.is_finite()
            && (0.0..=600.0).contains(&self.duration)
            && self
                .recipe
                .is_none_or(|id| recipes().iter().any(|recipe| recipe.id == id))
    }
}
impl CrusherState {
    pub fn valid(&self) -> bool {
        self.progress_seconds.is_finite()
            && (0.0..=600.0).contains(&self.progress_seconds)
            && self.recipe.map_or(self.progress_seconds == 0.0, |id| {
                recipes()
                    .iter()
                    .any(|recipe| recipe.id == id && self.progress_seconds <= recipe.seconds)
            })
    }
    pub fn can_process(&self, inventory: &Inventory) -> bool {
        inventory
            .slot(INPUT)
            .and_then(|stack| recipe_for(stack.item))
            .is_some_and(|recipe| {
                inventory.slot(OUTPUT).is_none_or(|stack| {
                    stack.item == recipe.output.item
                        && stack.count <= recipe.output.item.max_stack() - recipe.output.count
                })
            })
            && self.completed < u64::MAX
            && inventory.revision < u64::MAX
    }
    pub fn view(&self, inventory: &Inventory) -> CrusherView {
        let recipe = self
            .recipe
            .and_then(|id| recipes().iter().find(|recipe| recipe.id == id));
        CrusherView {
            progress: recipe.map_or(0.0, |recipe| {
                (self.progress_seconds / recipe.seconds).clamp(0.0, 1.0)
            }),
            duration: recipe.map_or(0.0, |recipe| recipe.seconds),
            recipe: self.recipe,
            working: self.can_process(inventory),
        }
    }
}
/// No offline catchup. At most one second and two completions per authority call.
pub fn advance(state: &mut CrusherState, inventory: &mut Inventory, elapsed: f32) -> bool {
    if !state.valid()
        || !valid_inventory(inventory)
        || !elapsed.is_finite()
        || elapsed <= 0.0
        || inventory.revision == u64::MAX
        || state.completed == u64::MAX
    {
        return false;
    }
    let before_state = state.clone();
    let before_inventory = inventory.clone();
    let mut remaining = elapsed.min(1.0);
    for _ in 0..3 {
        let recipe = inventory
            .slot(INPUT)
            .and_then(|stack| recipe_for(stack.item));
        if state.recipe != recipe.map(|recipe| recipe.id) {
            state.recipe = recipe.map(|recipe| recipe.id);
            state.progress_seconds = 0.0;
        }
        let Some(recipe) = recipe.filter(|_| state.can_process(inventory)) else {
            break;
        };
        let delta = remaining.min((recipe.seconds - state.progress_seconds).max(0.0));
        state.progress_seconds += delta;
        remaining -= delta;
        if state.progress_seconds + 0.00001 >= recipe.seconds {
            inventory.slots.resize(SLOTS, ItemStack::EMPTY);
            let input = &mut inventory.slots[INPUT];
            input.count -= 1;
            if input.count == 0 {
                *input = ItemStack::EMPTY;
            }
            let count = inventory.slot(OUTPUT).map_or(0, |stack| stack.count) + recipe.output.count;
            inventory.slots[OUTPUT] = ItemStack {
                item: recipe.output.item,
                count,
            };
            state.progress_seconds = 0.0;
            state.completed += 1;
        }
        if remaining <= 0.00001 || state.completed == u64::MAX {
            break;
        }
    }
    if inventory.slots != before_inventory.slots {
        inventory.revision = before_inventory.revision + 1;
    }
    *state != before_state || *inventory != before_inventory
}

#[cfg(test)]
mod tests;
