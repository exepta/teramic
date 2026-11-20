//! Enum item identities are encoded as counted entries, never JSON object keys.
use super::*;
use serde::{
    Deserializer, Serializer,
    de::{Error, SeqAccess, Visitor},
};

pub(super) fn serialize<S: Serializer>(
    items: &BTreeMap<ItemKind, u32>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(items.iter())
}
pub(super) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<ItemKind, u32>, D::Error> {
    struct Entries;
    impl<'de> Visitor<'de> for Entries {
        type Value = BTreeMap<ItemKind, u32>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "at most {MAX_TOWN_STOCK_ITEMS} unique counted town items"
            )
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            if seq.size_hint().is_some_and(|n| n > MAX_TOWN_STOCK_ITEMS) {
                return Err(A::Error::custom("Too many town stock items"));
            }
            let mut items = BTreeMap::new();
            while let Some((item, count)) = seq.next_element::<(ItemKind, u32)>()? {
                if items.len() >= MAX_TOWN_STOCK_ITEMS {
                    return Err(A::Error::custom("Too many town stock items"));
                }
                if items.insert(item, count).is_some() {
                    return Err(A::Error::custom("Duplicate town stock item"));
                }
            }
            Ok(items)
        }
    }
    deserializer.deserialize_seq(Entries)
}
