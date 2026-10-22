//! Shared, validated recipe data and atomic inventory transactions.
use crate::{Inventory, ItemKind, ItemStack, STACK_LIMIT};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path, sync::OnceLock};

pub const MAX_CRAFT_BATCH: u16 = 128;
pub const MAX_RECIPES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CraftStation {
    Hand,
    Workbench,
}

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ingredient {
    pub item: ItemKind,
    pub count: u16,
    #[serde(default)]
    pub wood: bool,
}
impl Ingredient {
    pub fn matches(self, item: ItemKind) -> bool {
        if self.wood {
            matches!(item, ItemKind::Block(16..=18))
        } else if let ItemKind::Block(material @ 71..=73) = self.item {
            item.block_material() == Some(material)
        } else {
            self.item == item
        }
    }
    pub fn available_count(self, inventory: &Inventory) -> u16 {
        inventory
            .slots
            .iter()
            .filter(|s| self.matches(s.item))
            .map(|s| s.count)
            .sum()
    }
}

/// A quantity reserved from an existing inventory slot; no duplicate item storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CraftGridCell {
    pub slot: u8,
    pub count: u16,
}

fn resolved_output(mut output: ItemStack, wood: [u32; 3]) -> ItemStack {
    if let Some(material @ 28..=29) = output.item.block_material() {
        let species = if wood[1] > wood[0] && wood[1] >= wood[2] {
            crate::WoodSpecies::Birch
        } else if wood[2] > wood[0] && wood[2] > wood[1] {
            crate::WoodSpecies::Spruce
        } else {
            crate::WoodSpecies::Oak
        };
        output.item = ItemKind::for_wood(material, species);
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecipePattern {
    Shapeless {
        ingredients: Vec<Ingredient>,
    },
    Shaped {
        width: u8,
        height: u8,
        cells: Vec<Option<Ingredient>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub station: CraftStation,
    pub pattern: RecipePattern,
    pub output: ItemStack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CraftDestination {
    Inventory,
    Cursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftError {
    UnknownRecipe,
    WrongStation,
    InvalidQuantity,
    MissingIngredients,
    NoSpace,
}
impl CraftError {
    pub fn message(self) -> &'static str {
        match self {
            Self::UnknownRecipe => "Recipe is unavailable.",
            Self::WrongStation => "A nearby workbench is required.",
            Self::InvalidQuantity => "Choose between 1 and 128 crafts.",
            Self::MissingIngredients => "Missing ingredients.",
            Self::NoSpace => "No room for the result. Free inventory space or empty the cursor.",
        }
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct RecipeBook {
    recipes: BTreeMap<u64, Recipe>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeFile {
    schema_version: u32,
    recipes: Vec<Recipe>,
}

impl Recipe {
    pub fn key(&self) -> u64 {
        recipe_key(&self.id)
    }
    pub fn ingredients(&self) -> Vec<Ingredient> {
        let cells: Vec<_> = match &self.pattern {
            RecipePattern::Shapeless { ingredients } => ingredients.clone(),
            RecipePattern::Shaped { cells, .. } => cells.iter().flatten().copied().collect(),
        };
        let mut totals = BTreeMap::<(ItemKind, bool), u16>::new();
        for cell in cells {
            *totals.entry((cell.item, cell.wood)).or_default() += cell.count;
        }
        totals
            .into_iter()
            .map(|((item, wood), count)| Ingredient { item, count, wood })
            .collect()
    }
    fn validate(&self) -> Result<(), String> {
        let valid_id = self.id.split_once("::").is_some_and(|(namespace, name)| {
            !namespace.is_empty()
                && !name.is_empty()
                && self.id.len() <= 96
                && namespace
                    .bytes()
                    .chain(name.bytes())
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_-/".contains(&c))
        });
        if !valid_id
            || self.name.trim().is_empty()
            || self.name.len() > 96
            || self.name.chars().any(char::is_control)
            || !self.output.valid()
        {
            return Err(format!("Invalid recipe metadata: {}", self.id));
        }
        let ingredients: Vec<_> = match &self.pattern {
            RecipePattern::Shapeless { ingredients } if (1..=9).contains(&ingredients.len()) => {
                ingredients.iter().collect()
            }
            RecipePattern::Shaped {
                width,
                height,
                cells,
            } if (1..=3).contains(width)
                && (1..=3).contains(height)
                && cells.len() == usize::from(*width) * usize::from(*height)
                && cells[..usize::from(*width)].iter().any(Option::is_some)
                && cells[cells.len() - usize::from(*width)..]
                    .iter()
                    .any(Option::is_some)
                && cells
                    .iter()
                    .step_by(usize::from(*width))
                    .any(Option::is_some)
                && cells
                    .iter()
                    .skip(usize::from(*width) - 1)
                    .step_by(usize::from(*width))
                    .any(Option::is_some) =>
            {
                cells.iter().flatten().collect()
            }
            _ => return Err(format!("Invalid recipe pattern: {}", self.id)),
        };
        if ingredients.iter().any(|i| {
            !i.item.valid()
                || !(1..=STACK_LIMIT).contains(&i.count)
                || i.wood && !matches!(i.item, ItemKind::Block(16..=18))
        }) {
            return Err(format!("Invalid ingredients: {}", self.id));
        }
        Ok(())
    }
}

/// Stable on disk, across platforms and across registry insertion order.
pub fn recipe_key(id: &str) -> u64 {
    id.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn match_shapeless(
    inventory: &Inventory,
    cells: &[Option<u8>],
    ingredients: &[Ingredient],
) -> Option<Vec<(u8, u16)>> {
    let slots: Vec<_> = cells.iter().flatten().copied().collect();
    if slots.len() != ingredients.len() {
        return None;
    }
    // Exact ingredients can overlap generic groups. A greedy first match makes
    // the result depend on cell order; nine cells bound this small search.
    fn assign(
        inventory: &Inventory,
        slots: &[u8],
        ingredients: &[Ingredient],
        used: &mut [bool; 9],
        totals: &mut [u16; crate::INVENTORY_SLOTS],
        ordered: &mut Vec<(u8, u16)>,
    ) -> bool {
        let Some((&slot, rest)) = slots.split_first() else {
            return true;
        };
        let Some(stack) = inventory.slot(usize::from(slot)) else {
            return false;
        };
        for (index, ingredient) in ingredients.iter().enumerate() {
            if used[index]
                || !ingredient.matches(stack.item)
                || (0..index)
                    .any(|previous| !used[previous] && ingredients[previous] == *ingredient)
                || totals[usize::from(slot)] + ingredient.count > stack.count
            {
                continue;
            }
            used[index] = true;
            totals[usize::from(slot)] += ingredient.count;
            ordered.push((slot, ingredient.count));
            if assign(inventory, rest, ingredients, used, totals, ordered) {
                return true;
            }
            ordered.pop();
            totals[usize::from(slot)] -= ingredient.count;
            used[index] = false;
        }
        false
    }
    let mut ordered = Vec::with_capacity(slots.len());
    assign(
        inventory,
        &slots,
        ingredients,
        &mut [false; 9],
        &mut [0; crate::INVENTORY_SLOTS],
        &mut ordered,
    )
    .then_some(ordered)
}

impl RecipeBook {
    pub fn from_json(text: &str) -> Result<Self, String> {
        if text.len() > 1024 * 1024 {
            return Err("Recipe catalog exceeds 1 MiB".into());
        }
        let file: RecipeFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if file.schema_version != 1 {
            return Err("Unsupported recipe schema".into());
        }
        Self::from_recipes(file.recipes)
    }
    pub fn to_json(&self) -> Result<String, String> {
        let text = serde_json::to_string(&RecipeFile {
            schema_version: 1,
            recipes: self.snapshot(),
        })
        .map_err(|e| e.to_string())?;
        if text.len() > 1024 * 1024 {
            return Err("Recipe catalog exceeds 1 MiB".into());
        }
        Ok(text)
    }
    pub fn builtins() -> &'static Self {
        static BOOK: OnceLock<RecipeBook> = OnceLock::new();
        BOOK.get_or_init(|| {
            include!("crafting/builtins.rs")
                .iter()
                .map(|source| serde_json::from_str(source).map_err(|e| e.to_string()))
                .collect::<Result<Vec<Recipe>, String>>()
                .and_then(Self::from_recipes)
                .unwrap_or_else(|error| Self {
                    error: Some(error),
                    ..Self::default()
                })
        })
    }
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let file: RecipeFile = toml::from_str(text).map_err(|error| error.to_string())?;
        if file.schema_version != 1 {
            return Err("Unsupported recipe schema".into());
        }
        Self::from_recipes(file.recipes)
    }
    pub fn from_recipes(recipes: Vec<Recipe>) -> Result<Self, String> {
        let mut book = Self::default();
        book.extend(recipes)?;
        Ok(book)
    }
    pub fn extend(&mut self, recipes: Vec<Recipe>) -> Result<(), String> {
        if self.recipes.len() + recipes.len() > MAX_RECIPES {
            return Err("Too many recipes (maximum 512)".into());
        }
        let mut next = self.recipes.clone();
        for recipe in recipes {
            recipe.validate()?;
            if next.insert(recipe.key(), recipe).is_some() {
                return Err("Duplicate recipe ID or key collision".into());
            }
        }
        self.recipes = next;
        Ok(())
    }
    /// Mod packs add namespaced recipes; a bad file never partly changes the book.
    pub fn load_directory(&mut self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let mut paths = std::fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .map(|entry| entry.map(|e| e.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        let mut recipes = Vec::new();
        for path in paths.into_iter().filter(|p| {
            p.extension()
                .is_some_and(|ext| ext == "toml" || ext == "json")
        }) {
            if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 512 * 1024 {
                return Err("Recipe file exceeds 512 KiB".into());
            }
            let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let file: RecipeFile = if path.extension().is_some_and(|ext| ext == "json") {
                let value: serde_json::Value =
                    serde_json::from_str(&source).map_err(|e| e.to_string())?;
                if value.get("id").is_some() {
                    RecipeFile {
                        schema_version: 1,
                        recipes: vec![serde_json::from_value(value).map_err(|e| e.to_string())?],
                    }
                } else {
                    serde_json::from_value(value).map_err(|e| e.to_string())?
                }
            } else {
                toml::from_str(&source).map_err(|e| format!("{}: {e}", path.display()))?
            };
            if file.schema_version != 1 {
                return Err("Unsupported recipe schema".into());
            }
            recipes.extend(file.recipes);
            if recipes.len() > MAX_RECIPES {
                return Err("Too many mod recipes".into());
            }
        }
        self.extend(recipes)
    }
    pub fn get(&self, key: u64) -> Option<&Recipe> {
        self.recipes.get(&key)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.values()
    }
    pub fn snapshot(&self) -> Vec<Recipe> {
        self.iter().cloned().collect()
    }
    pub fn craft(
        &self,
        inventory: &mut Inventory,
        key: u64,
        count: u16,
        station: CraftStation,
        destination: CraftDestination,
    ) -> Result<(), CraftError> {
        if !(1..=MAX_CRAFT_BATCH).contains(&count) {
            return Err(CraftError::InvalidQuantity);
        }
        let recipe = self.get(key).ok_or(CraftError::UnknownRecipe)?;
        if recipe.station == CraftStation::Workbench && station != CraftStation::Workbench {
            return Err(CraftError::WrongStation);
        }
        let mut next = inventory.clone();
        let mut wood = [0_u32; 3];
        for ingredient in recipe.ingredients() {
            let needed = ingredient
                .count
                .checked_mul(count)
                .ok_or(CraftError::MissingIngredients)?;
            if ingredient.available_count(&next) < needed {
                return Err(CraftError::MissingIngredients);
            }
            let mut remaining = needed;
            for stack in &mut next.slots {
                if ingredient.matches(stack.item) && remaining > 0 {
                    let used = remaining.min(stack.count);
                    if let Some(species) = stack.item.wood_species() {
                        wood[species as usize] += u32::from(used);
                    }
                    stack.count -= used;
                    remaining -= used;
                    if stack.count == 0 {
                        *stack = ItemStack::EMPTY;
                    }
                }
            }
        }
        let amount = recipe.output.count * count;
        let output = resolved_output(recipe.output, wood);
        match destination {
            CraftDestination::Inventory => {
                if next.insert(output.item, amount) > 0 {
                    return Err(CraftError::NoSpace);
                }
            }
            CraftDestination::Cursor => {
                let present = next.cursor.map_or(0, |s| s.count);
                if next.cursor.is_some_and(|s| s.item != output.item)
                    || present + amount > STACK_LIMIT
                {
                    return Err(CraftError::NoSpace);
                }
                next.cursor = Some(ItemStack {
                    item: output.item,
                    count: present + amount,
                });
            }
        }
        next.revision = inventory.revision.saturating_add(1);
        *inventory = next;
        Ok(())
    }
    pub fn max_crafts(
        &self,
        inventory: &Inventory,
        key: u64,
        station: CraftStation,
        destination: CraftDestination,
    ) -> u16 {
        let Some(recipe) = self.get(key) else {
            return 0;
        };
        let max = recipe.ingredients().iter().fold(MAX_CRAFT_BATCH, |max, i| {
            max.min(i.available_count(inventory) / i.count)
        });
        (1..=max)
            .rev()
            .find(|count| {
                self.craft(&mut inventory.clone(), key, *count, station, destination)
                    .is_ok()
            })
            .unwrap_or(0)
    }
    pub fn grid_match(
        &self,
        inventory: &Inventory,
        cells: [Option<u8>; 4],
    ) -> Option<(u64, Vec<(u8, u16)>)> {
        self.grid_match_in(inventory, &cells, 2, CraftStation::Hand)
    }
    fn reserved_inventory(
        &self,
        inventory: &Inventory,
        cells: &[Option<CraftGridCell>],
    ) -> Option<Inventory> {
        let mut reserved = BTreeMap::<u8, u16>::new();
        let mut slots = Vec::with_capacity(cells.len());
        for cell in cells {
            if let Some(cell) = cell {
                if !(1..=STACK_LIMIT).contains(&cell.count) {
                    return None;
                }
                let stack = inventory.slot(usize::from(cell.slot))?;
                let total = reserved.entry(cell.slot).or_default();
                *total = total.checked_add(cell.count)?;
                if *total > stack.count {
                    return None;
                }
                slots.push(ItemStack {
                    item: stack.item,
                    count: cell.count,
                });
            } else {
                slots.push(ItemStack::EMPTY);
            }
        }
        Some(Inventory {
            slots,
            ..Inventory::default()
        })
    }
    pub fn grid_match_reserved(
        &self,
        inventory: &Inventory,
        cells: &[Option<CraftGridCell>],
        size: usize,
        station: CraftStation,
    ) -> Option<(u64, Vec<(u8, u16)>)> {
        let virtual_inventory = self.reserved_inventory(inventory, cells)?;
        let references: Vec<_> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| c.map(|_| i as u8))
            .collect();
        let (key, consumed) = self.grid_match_in(&virtual_inventory, &references, size, station)?;
        let mut totals = BTreeMap::<u8, u16>::new();
        for (cell, count) in consumed {
            *totals.entry(cells[usize::from(cell)]?.slot).or_default() += count;
        }
        Some((key, totals.into_iter().collect()))
    }
    pub fn grid_consumption_reserved(
        &self,
        inventory: &Inventory,
        cells: &[Option<CraftGridCell>],
        size: usize,
        station: CraftStation,
    ) -> Option<Vec<(usize, u16)>> {
        let virtual_inventory = self.reserved_inventory(inventory, cells)?;
        let references: Vec<_> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| c.map(|_| i as u8))
            .collect();
        let (_, consumed) = self.grid_match_in(&virtual_inventory, &references, size, station)?;
        Some(
            consumed
                .into_iter()
                .map(|(cell, count)| (usize::from(cell), count))
                .collect(),
        )
    }
    pub fn output_for_grid(
        &self,
        inventory: &Inventory,
        cells: &[Option<u8>],
        size: usize,
        station: CraftStation,
    ) -> Option<ItemStack> {
        let (key, consumed) = self.grid_match_in(inventory, cells, size, station)?;
        self.output_for_consumed(inventory, key, &consumed)
    }
    fn output_for_consumed(
        &self,
        inventory: &Inventory,
        key: u64,
        consumed: &[(u8, u16)],
    ) -> Option<ItemStack> {
        let mut wood = [0_u32; 3];
        for &(slot, count) in consumed {
            if let Some(species) = inventory.slot(usize::from(slot))?.item.wood_species() {
                wood[species as usize] += u32::from(count);
            }
        }
        Some(resolved_output(self.get(key)?.output, wood))
    }
    pub fn output_for_reserved_grid(
        &self,
        inventory: &Inventory,
        cells: &[Option<CraftGridCell>],
        size: usize,
        station: CraftStation,
    ) -> Option<ItemStack> {
        let (key, consumed) = self.grid_match_reserved(inventory, cells, size, station)?;
        self.output_for_consumed(inventory, key, &consumed)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn craft_reserved_grid(
        &self,
        inventory: &mut Inventory,
        cells: &[Option<CraftGridCell>],
        size: usize,
        station: CraftStation,
        count: u16,
        destination: CraftDestination,
    ) -> bool {
        let Some(virtual_inventory) = self.reserved_inventory(inventory, cells) else {
            return false;
        };
        let references: Vec<_> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| c.map(|_| i as u8))
            .collect();
        let Some((key, consumed)) =
            self.grid_match_in(&virtual_inventory, &references, size, station)
        else {
            return false;
        };
        if consumed.iter().any(|(cell, needed)| {
            needed
                .checked_mul(count)
                .is_none_or(|amount| amount > virtual_inventory.slots[usize::from(*cell)].count)
        }) {
            return false;
        }
        let mut totals = BTreeMap::<u8, u16>::new();
        for (cell, needed) in consumed {
            let Some(original) = cells[usize::from(cell)] else {
                return false;
            };
            *totals.entry(original.slot).or_default() += needed;
        }
        self.craft_matched(
            inventory,
            key,
            totals.into_iter().collect(),
            count,
            destination,
        )
    }
    pub fn grid_match_in(
        &self,
        inventory: &Inventory,
        cells: &[Option<u8>],
        size: usize,
        station: CraftStation,
    ) -> Option<(u64, Vec<(u8, u16)>)> {
        if !inventory.valid()
            || !(2..=3).contains(&size)
            || cells.len() != size * size
            || cells
                .iter()
                .flatten()
                .any(|slot| usize::from(*slot) >= crate::INVENTORY_SLOTS)
        {
            return None;
        }
        self.iter()
            .filter(|r| r.station == CraftStation::Hand || station == CraftStation::Workbench)
            .find_map(|recipe| {
                let ingredients: Vec<_> = match &recipe.pattern {
                    RecipePattern::Shapeless { ingredients } => {
                        match_shapeless(inventory, cells, ingredients)?
                    }
                    RecipePattern::Shaped {
                        width,
                        height,
                        cells: pattern,
                    } => {
                        if usize::from(*width) > size || usize::from(*height) > size {
                            return None;
                        }
                        let occupied: Vec<_> = cells
                            .iter()
                            .enumerate()
                            .filter(|(_, c)| c.is_some())
                            .map(|(i, _)| i)
                            .collect();
                        let x = occupied.iter().map(|i| i % size).min()?;
                        let y = occupied.iter().map(|i| i / size).min()?;
                        if x + usize::from(*width) > size || y + usize::from(*height) > size {
                            return None;
                        }
                        let mut ordered = Vec::new();
                        for (i, slot) in cells.iter().enumerate() {
                            let px = (i % size).checked_sub(x);
                            let py = (i / size).checked_sub(y);
                            let ingredient = px
                                .zip(py)
                                .filter(|(x, y)| {
                                    *x < usize::from(*width) && *y < usize::from(*height)
                                })
                                .and_then(|(x, y)| pattern[y * usize::from(*width) + x]);
                            match (*slot, ingredient) {
                                (Some(slot), Some(ingredient))
                                    if ingredient
                                        .matches(inventory.slot(usize::from(slot))?.item) =>
                                {
                                    ordered.push((slot, ingredient.count))
                                }
                                (None, None) => {}
                                _ => return None,
                            }
                        }
                        ordered
                    }
                };
                let mut totals = BTreeMap::<u8, u16>::new();
                for (slot, count) in ingredients {
                    *totals.entry(slot).or_default() += count;
                }
                totals
                    .iter()
                    .all(|(slot, count)| {
                        inventory
                            .slot(usize::from(*slot))
                            .is_some_and(|s| s.count >= *count)
                    })
                    .then(|| (recipe.key(), totals.into_iter().collect()))
            })
    }
    pub fn craft_cells(
        &self,
        inventory: &mut Inventory,
        cells: [Option<u8>; 4],
        destination: CraftDestination,
    ) -> bool {
        self.craft_grid(inventory, &cells, 2, CraftStation::Hand, 1, destination)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn craft_grid(
        &self,
        inventory: &mut Inventory,
        cells: &[Option<u8>],
        size: usize,
        station: CraftStation,
        count: u16,
        destination: CraftDestination,
    ) -> bool {
        if !(1..=MAX_CRAFT_BATCH).contains(&count) {
            return false;
        }
        let Some((key, consumed)) = self.grid_match_in(inventory, cells, size, station) else {
            return false;
        };
        self.craft_matched(inventory, key, consumed, count, destination)
    }
    fn craft_matched(
        &self,
        inventory: &mut Inventory,
        key: u64,
        consumed: Vec<(u8, u16)>,
        count: u16,
        destination: CraftDestination,
    ) -> bool {
        if !inventory.valid() || !(1..=MAX_CRAFT_BATCH).contains(&count) {
            return false;
        }
        let Some(revision) = inventory.revision.checked_add(1) else {
            return false;
        };
        let Some(recipe) = self.get(key) else {
            return false;
        };
        let mut next = inventory.clone();
        let mut wood = [0_u32; 3];
        for (slot, needed) in consumed {
            let Some(needed) = needed.checked_mul(count) else {
                return false;
            };
            let stack = &mut next.slots[usize::from(slot)];
            if let Some(species) = stack.item.wood_species() {
                wood[species as usize] += u32::from(needed);
            }
            if stack.count < needed {
                return false;
            }
            stack.count -= needed;
            if stack.count == 0 {
                *stack = ItemStack::EMPTY;
            }
        }
        let output = resolved_output(recipe.output, wood);
        match destination {
            CraftDestination::Inventory => {
                if next.insert(output.item, output.count * count) > 0 {
                    return false;
                }
            }
            CraftDestination::Cursor => {
                let present = next.cursor.map_or(0, |s| s.count);
                if next.cursor.is_some_and(|s| s.item != output.item)
                    || present + output.count * count > STACK_LIMIT
                {
                    return false;
                }
                next.cursor = Some(ItemStack {
                    count: present + output.count * count,
                    ..output
                });
            }
        }
        next.revision = revision;
        *inventory = next;
        true
    }
}
