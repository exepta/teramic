//! Canonical bounded equipment identities and data-driven protection/attribute bonuses.
use crate::{ItemKind, ItemStack};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

pub const EQUIPMENT_SLOTS: usize = 10;
pub const MAX_ARMOR_POINTS: f32 = 40.0;
pub const MAX_EQUIPMENT_BONUS: f32 = 200.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum EquipmentSlot {
    Helmet,
    Earrings,
    Chestplate,
    Necklace,
    Pants,
    Belt,
    Boots,
    Socks,
    Gloves,
    Cape,
}
impl EquipmentSlot {
    pub const ALL: [Self; EQUIPMENT_SLOTS] = [
        Self::Helmet,
        Self::Earrings,
        Self::Chestplate,
        Self::Necklace,
        Self::Pants,
        Self::Belt,
        Self::Boots,
        Self::Socks,
        Self::Gloves,
        Self::Cape,
    ];
    pub const fn index(self) -> usize {
        self as usize
    }
    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }
    pub fn for_item(item: ItemKind) -> Option<Self> {
        equipment_definition(item).map(|definition| definition.slot)
    }
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Helmet => "helmet",
            Self::Earrings => "earrings",
            Self::Chestplate => "chestplate",
            Self::Necklace => "necklace",
            Self::Pants => "pants",
            Self::Belt => "belt",
            Self::Boots => "boots",
            Self::Socks => "socks",
            Self::Gloves => "gloves",
            Self::Cape => "cape",
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Helmet => "Helmet",
            Self::Earrings => "Earrings",
            Self::Chestplate => "Chestplate",
            Self::Necklace => "Necklace",
            Self::Pants => "Pants",
            Self::Belt => "Belt",
            Self::Boots => "Boots",
            Self::Socks => "Socks",
            Self::Gloves => "Gloves",
            Self::Cape => "Cape",
        }
    }
    pub fn accepts(self, item: ItemKind) -> bool {
        Self::for_item(item) == Some(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorMaterial {
    Leather,
    Iron,
    Gold,
    Diamond,
    Ruby,
}
impl ArmorMaterial {
    pub const ALL: [Self; 5] = [
        Self::Leather,
        Self::Iron,
        Self::Gold,
        Self::Diamond,
        Self::Ruby,
    ];
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Leather => "leather",
            Self::Iron => "iron",
            Self::Gold => "gold",
            Self::Diamond => "diamond",
            Self::Ruby => "ruby",
        }
    }
    pub const fn color(self) -> [f32; 4] {
        match self {
            Self::Leather => [0.34, 0.15, 0.055, 1.0],
            Self::Iron => [0.68, 0.72, 0.77, 1.0],
            Self::Gold => [0.96, 0.59, 0.07, 1.0],
            Self::Diamond => [0.04, 0.72, 0.88, 1.0],
            Self::Ruby => [0.77, 0.025, 0.055, 1.0],
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CapeItem {
    pub material: ArmorMaterial,
    pub color: crate::beds::DyeColor,
}
impl CapeItem {
    pub const ALL_WHITE: [Self; 5] = [
        Self {
            material: ArmorMaterial::Leather,
            color: crate::beds::DyeColor::White,
        },
        Self {
            material: ArmorMaterial::Iron,
            color: crate::beds::DyeColor::White,
        },
        Self {
            material: ArmorMaterial::Gold,
            color: crate::beds::DyeColor::White,
        },
        Self {
            material: ArmorMaterial::Diamond,
            color: crate::beds::DyeColor::White,
        },
        Self {
            material: ArmorMaterial::Ruby,
            color: crate::beds::DyeColor::White,
        },
    ];
    pub const ALL: [Self; ArmorMaterial::ALL.len() * crate::beds::DyeColor::ALL.len()] = {
        let mut all =
            [Self::ALL_WHITE[0]; ArmorMaterial::ALL.len() * crate::beds::DyeColor::ALL.len()];
        let mut material = 0;
        while material < ArmorMaterial::ALL.len() {
            let mut color = 0;
            while color < crate::beds::DyeColor::ALL.len() {
                all[material * crate::beds::DyeColor::ALL.len() + color] = Self {
                    material: ArmorMaterial::ALL[material],
                    color: crate::beds::DyeColor::ALL[color],
                };
                color += 1;
            }
            material += 1;
        }
        all
    };
    pub fn slug(self) -> String {
        format!("{}_cape", self.material.slug())
    }
    pub const fn undyed(self) -> Self {
        Self {
            material: self.material,
            color: crate::beds::DyeColor::White,
        }
    }
}

impl ItemKind {
    pub const fn cape(self) -> Option<CapeItem> {
        match self {
            Self::Cape(cape) | Self::HeraldicCape { cape, .. } => Some(cape),
            _ => None,
        }
    }

    /// Only newly acquired undyed capes receive the owner's heraldry. A manually
    /// colored cape and an existing heraldic cape retain their concrete identity.
    pub fn with_default_heraldry(self, heraldry: Option<crate::kingdom::KingdomHeraldry>) -> Self {
        match (self, heraldry) {
            (Self::Cape(cape), Some(heraldry))
                if cape.color == crate::beds::DyeColor::White && heraldry.icon.valid() =>
            {
                Self::HeraldicCape {
                    cape: CapeItem {
                        color: crate::beds::DyeColor::Rgb(heraldry.color),
                        ..cape
                    },
                    heraldry,
                }
            }
            _ => self,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorPiece {
    Helmet,
    Chestplate,
    Gloves,
    Pants,
    Boots,
}
impl ArmorPiece {
    pub const ALL: [Self; 5] = [
        Self::Helmet,
        Self::Chestplate,
        Self::Gloves,
        Self::Pants,
        Self::Boots,
    ];
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Helmet => "helmet",
            Self::Chestplate => "chestplate",
            Self::Gloves => "gloves",
            Self::Pants => "pants",
            Self::Boots => "boots",
        }
    }
    pub const fn slot(self) -> EquipmentSlot {
        match self {
            Self::Helmet => EquipmentSlot::Helmet,
            Self::Chestplate => EquipmentSlot::Chestplate,
            Self::Gloves => EquipmentSlot::Gloves,
            Self::Pants => EquipmentSlot::Pants,
            Self::Boots => EquipmentSlot::Boots,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ArmorItem {
    pub material: ArmorMaterial,
    pub piece: ArmorPiece,
}
impl ArmorItem {
    pub const ALL: [Self; 25] = [
        Self {
            material: ArmorMaterial::Leather,
            piece: ArmorPiece::Helmet,
        },
        Self {
            material: ArmorMaterial::Leather,
            piece: ArmorPiece::Chestplate,
        },
        Self {
            material: ArmorMaterial::Leather,
            piece: ArmorPiece::Gloves,
        },
        Self {
            material: ArmorMaterial::Leather,
            piece: ArmorPiece::Pants,
        },
        Self {
            material: ArmorMaterial::Leather,
            piece: ArmorPiece::Boots,
        },
        Self {
            material: ArmorMaterial::Iron,
            piece: ArmorPiece::Helmet,
        },
        Self {
            material: ArmorMaterial::Iron,
            piece: ArmorPiece::Chestplate,
        },
        Self {
            material: ArmorMaterial::Iron,
            piece: ArmorPiece::Gloves,
        },
        Self {
            material: ArmorMaterial::Iron,
            piece: ArmorPiece::Pants,
        },
        Self {
            material: ArmorMaterial::Iron,
            piece: ArmorPiece::Boots,
        },
        Self {
            material: ArmorMaterial::Gold,
            piece: ArmorPiece::Helmet,
        },
        Self {
            material: ArmorMaterial::Gold,
            piece: ArmorPiece::Chestplate,
        },
        Self {
            material: ArmorMaterial::Gold,
            piece: ArmorPiece::Gloves,
        },
        Self {
            material: ArmorMaterial::Gold,
            piece: ArmorPiece::Pants,
        },
        Self {
            material: ArmorMaterial::Gold,
            piece: ArmorPiece::Boots,
        },
        Self {
            material: ArmorMaterial::Diamond,
            piece: ArmorPiece::Helmet,
        },
        Self {
            material: ArmorMaterial::Diamond,
            piece: ArmorPiece::Chestplate,
        },
        Self {
            material: ArmorMaterial::Diamond,
            piece: ArmorPiece::Gloves,
        },
        Self {
            material: ArmorMaterial::Diamond,
            piece: ArmorPiece::Pants,
        },
        Self {
            material: ArmorMaterial::Diamond,
            piece: ArmorPiece::Boots,
        },
        Self {
            material: ArmorMaterial::Ruby,
            piece: ArmorPiece::Helmet,
        },
        Self {
            material: ArmorMaterial::Ruby,
            piece: ArmorPiece::Chestplate,
        },
        Self {
            material: ArmorMaterial::Ruby,
            piece: ArmorPiece::Gloves,
        },
        Self {
            material: ArmorMaterial::Ruby,
            piece: ArmorPiece::Pants,
        },
        Self {
            material: ArmorMaterial::Ruby,
            piece: ArmorPiece::Boots,
        },
    ];
    pub fn slug(self) -> String {
        format!("{}_{}", self.material.slug(), self.piece.slug())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessoryKind {
    Earrings,
    Necklace,
    Belt,
    Socks,
}
impl AccessoryKind {
    pub const ALL: [Self; 4] = [Self::Earrings, Self::Necklace, Self::Belt, Self::Socks];
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Earrings => "earrings",
            Self::Necklace => "necklace",
            Self::Belt => "belt",
            Self::Socks => "socks",
        }
    }
    pub const fn slot(self) -> EquipmentSlot {
        match self {
            Self::Earrings => EquipmentSlot::Earrings,
            Self::Necklace => EquipmentSlot::Necklace,
            Self::Belt => EquipmentSlot::Belt,
            Self::Socks => EquipmentSlot::Socks,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EquipmentBonuses {
    pub armor_points: f32,
    pub health_bonus: f32,
    pub stamina_bonus: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquipmentDefinition {
    pub slot: EquipmentSlot,
    pub armor_points: f32,
    pub health_bonus: f32,
    pub stamina_bonus: f32,
}
impl EquipmentDefinition {
    pub fn valid(self) -> bool {
        self.armor_points.is_finite()
            && (0.0..=MAX_ARMOR_POINTS).contains(&self.armor_points)
            && [self.health_bonus, self.stamina_bonus]
                .into_iter()
                .all(|n| n.is_finite() && (0.0..=MAX_EQUIPMENT_BONUS).contains(&n))
    }
}
#[derive(Deserialize)]
struct GearFile {
    item: ItemKind,
    equipment: EquipmentDefinition,
}
const SOURCES: [(&str, &str); 34] = [
    (
        "leather_helmet",
        include_str!("../../../assets/terra/items/leather_helmet.json"),
    ),
    (
        "leather_chestplate",
        include_str!("../../../assets/terra/items/leather_chestplate.json"),
    ),
    (
        "leather_gloves",
        include_str!("../../../assets/terra/items/leather_gloves.json"),
    ),
    (
        "leather_pants",
        include_str!("../../../assets/terra/items/leather_pants.json"),
    ),
    (
        "leather_boots",
        include_str!("../../../assets/terra/items/leather_boots.json"),
    ),
    (
        "iron_helmet",
        include_str!("../../../assets/terra/items/iron_helmet.json"),
    ),
    (
        "iron_chestplate",
        include_str!("../../../assets/terra/items/iron_chestplate.json"),
    ),
    (
        "iron_gloves",
        include_str!("../../../assets/terra/items/iron_gloves.json"),
    ),
    (
        "iron_pants",
        include_str!("../../../assets/terra/items/iron_pants.json"),
    ),
    (
        "iron_boots",
        include_str!("../../../assets/terra/items/iron_boots.json"),
    ),
    (
        "gold_helmet",
        include_str!("../../../assets/terra/items/gold_helmet.json"),
    ),
    (
        "gold_chestplate",
        include_str!("../../../assets/terra/items/gold_chestplate.json"),
    ),
    (
        "gold_gloves",
        include_str!("../../../assets/terra/items/gold_gloves.json"),
    ),
    (
        "gold_pants",
        include_str!("../../../assets/terra/items/gold_pants.json"),
    ),
    (
        "gold_boots",
        include_str!("../../../assets/terra/items/gold_boots.json"),
    ),
    (
        "diamond_helmet",
        include_str!("../../../assets/terra/items/diamond_helmet.json"),
    ),
    (
        "diamond_chestplate",
        include_str!("../../../assets/terra/items/diamond_chestplate.json"),
    ),
    (
        "diamond_gloves",
        include_str!("../../../assets/terra/items/diamond_gloves.json"),
    ),
    (
        "diamond_pants",
        include_str!("../../../assets/terra/items/diamond_pants.json"),
    ),
    (
        "diamond_boots",
        include_str!("../../../assets/terra/items/diamond_boots.json"),
    ),
    (
        "ruby_helmet",
        include_str!("../../../assets/terra/items/ruby_helmet.json"),
    ),
    (
        "ruby_chestplate",
        include_str!("../../../assets/terra/items/ruby_chestplate.json"),
    ),
    (
        "ruby_gloves",
        include_str!("../../../assets/terra/items/ruby_gloves.json"),
    ),
    (
        "ruby_pants",
        include_str!("../../../assets/terra/items/ruby_pants.json"),
    ),
    (
        "ruby_boots",
        include_str!("../../../assets/terra/items/ruby_boots.json"),
    ),
    (
        "earrings",
        include_str!("../../../assets/terra/items/earrings.json"),
    ),
    (
        "necklace",
        include_str!("../../../assets/terra/items/necklace.json"),
    ),
    (
        "belt",
        include_str!("../../../assets/terra/items/belt.json"),
    ),
    (
        "socks",
        include_str!("../../../assets/terra/items/socks.json"),
    ),
    (
        "leather_cape",
        include_str!("../../../assets/terra/items/leather_cape.json"),
    ),
    (
        "iron_cape",
        include_str!("../../../assets/terra/items/iron_cape.json"),
    ),
    (
        "gold_cape",
        include_str!("../../../assets/terra/items/gold_cape.json"),
    ),
    (
        "diamond_cape",
        include_str!("../../../assets/terra/items/diamond_cape.json"),
    ),
    (
        "ruby_cape",
        include_str!("../../../assets/terra/items/ruby_cape.json"),
    ),
];
fn parse(source: &str, expected: ItemKind) -> Result<EquipmentDefinition, String> {
    let file: GearFile = serde_json::from_str(source).map_err(|e| e.to_string())?;
    let slot = match expected {
        ItemKind::Armor(armor) => armor.piece.slot(),
        ItemKind::Accessory(kind) => kind.slot(),
        ItemKind::Cape(_) => EquipmentSlot::Cape,
        _ => return Err("Not an equipment identity".into()),
    };
    if file.item != expected || file.equipment.slot != slot || !file.equipment.valid() {
        return Err("Invalid equipment identity, slot or bonuses".into());
    }
    Ok(file.equipment)
}
pub fn equipment_definition(item: ItemKind) -> Option<&'static EquipmentDefinition> {
    let item = if let Some(cape) = item.cape() {
        ItemKind::Cape(cape.undyed())
    } else {
        item
    };
    static DEFINITIONS: OnceLock<BTreeMap<ItemKind, EquipmentDefinition>> = OnceLock::new();
    DEFINITIONS.get_or_init(||{
        let root=crate::asset_directory().join("terra/items");
        SOURCES.into_iter().map(|(name,source)|{
            let file: GearFile=serde_json::from_str(source).expect("embedded equipment JSON");
            let fallback=parse(source,file.item).expect("valid embedded equipment definition");
            let path=root.join(format!("{name}.json"));
            let definition=match std::fs::read_to_string(&path) {
                Ok(text)=>match parse(&text,file.item) {Ok(definition)=>definition,Err(error)=>{eprintln!("Invalid equipment definition {}: {error}; using builtin values",path.display());fallback}},
                Err(_)=>fallback,
            };
            (file.item,definition)
        }).collect()
    }).get(&item)
}
pub fn equipment_catalog_fingerprint() -> u64 {
    static HASH: OnceLock<u64> = OnceLock::new();
    *HASH.get_or_init(|| {
        let mut hash = 0xcbf29ce484222325_u64;
        for item in ArmorItem::ALL
            .map(ItemKind::Armor)
            .into_iter()
            .chain(AccessoryKind::ALL.map(ItemKind::Accessory))
            .chain(CapeItem::ALL_WHITE.map(ItemKind::Cape))
        {
            let definition = equipment_definition(item).expect("builtin equipment identity");
            for byte in
                serde_json::to_vec(&(item, definition)).expect("finite equipment definition")
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        hash
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EquipmentSlots {
    pub slots: [ItemStack; EQUIPMENT_SLOTS],
}
impl Default for EquipmentSlots {
    fn default() -> Self {
        Self {
            slots: [ItemStack::EMPTY; EQUIPMENT_SLOTS],
        }
    }
}
impl EquipmentSlots {
    pub fn valid(self) -> bool {
        EquipmentSlot::ALL.into_iter().all(|slot| {
            let stack = self.slots[slot.index()];
            stack == ItemStack::EMPTY
                || stack.count == 1 && stack.valid() && slot.accepts(stack.item)
        })
    }
    pub fn get(self, slot: EquipmentSlot) -> Option<ItemKind> {
        self.slot(slot.index())
            .map(|stack| stack.item)
            .filter(|item| slot.accepts(*item))
    }
    pub fn slot(self, index: usize) -> Option<ItemStack> {
        self.slots
            .get(index)
            .copied()
            .filter(|stack| stack.valid() && stack.count == 1)
    }
    pub fn set(&mut self, slot: EquipmentSlot, item: Option<ItemKind>) -> bool {
        if item.is_some_and(|item| !slot.accepts(item)) {
            return false;
        }
        let stack = item.map_or(ItemStack::EMPTY, |item| ItemStack { item, count: 1 });
        if self.slots[slot.index()] == stack {
            return false;
        }
        self.slots[slot.index()] = stack;
        true
    }
    pub fn iter(self) -> impl Iterator<Item = (EquipmentSlot, ItemKind)> {
        EquipmentSlot::ALL
            .into_iter()
            .filter_map(move |slot| self.get(slot).map(|item| (slot, item)))
    }
    pub fn bonuses(self) -> EquipmentBonuses {
        let mut total = EquipmentBonuses::default();
        for (_, item) in self.iter() {
            if let Some(definition) = equipment_definition(item) {
                total.armor_points += definition.armor_points;
                total.health_bonus += definition.health_bonus;
                total.stamina_bonus += definition.stamina_bonus;
            }
        }
        total.armor_points = total.armor_points.min(MAX_ARMOR_POINTS);
        total.health_bonus = total.health_bonus.min(MAX_EQUIPMENT_BONUS);
        total.stamina_bonus = total.stamina_bonus.min(MAX_EQUIPMENT_BONUS);
        total
    }
}

#[cfg(test)]
mod tests;
