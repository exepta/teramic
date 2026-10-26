//! Pot contents use the existing revisioned, ray-validated object interaction.
use super::{ControlWorld, Peer};
use crate::protocol::session::InventorySource;
use terra_core::{GameMode, Inventory, flower_pot};
use terra_world::streaming::commit_object_state;

fn transaction(
    contents: u8,
    inventory: &Inventory,
    source: Option<InventorySource>,
    held_slot: Option<u8>,
    mode: GameMode,
) -> Result<(u8, Inventory), String> {
    if contents >= flower_pot::STATES || mode == GameMode::Spectator {
        return Err("Invalid flower pot interaction".into());
    }
    if inventory.revision == u64::MAX
        || source.map(|source| source.slot) != held_slot
        || source.is_some_and(|source| source.revision != inventory.revision)
    {
        return Err("Held item changed".into());
    }
    let held = held_slot.and_then(|slot| inventory.slot(usize::from(slot)));
    let mut next = inventory.clone();
    if let Some(held) = held {
        if contents != 0 {
            return Err("Remove the plant with an empty hand first".into());
        }
        let state = flower_pot::state(held.item).ok_or("This item cannot be planted in a pot")?;
        if mode == GameMode::Survival && !next.take_from(usize::from(held_slot.unwrap()), held.item)
        {
            return Err("Missing flower pot plant".into());
        }
        Ok((state, next))
    } else if let Some(item) = flower_pot::contents(contents) {
        if next.revision == u64::MAX || next.insert(item, 1) != 0 {
            return Err("No room for the plant".into());
        }
        Ok((0, next))
    } else {
        Ok((0, next))
    }
}

pub(super) fn interact(
    position: [i64; 3],
    source: Option<InventorySource>,
    peer: &Peer,
    world: &mut ControlWorld,
) -> Result<bool, String> {
    if world.chunks.block_at(position) != Some(flower_pot::MATERIAL) {
        return Ok(false);
    }
    let state = world
        .journal
        .object_state(position)
        .ok_or("Missing flower pot state")?;
    if !state.valid_for(position, flower_pot::MATERIAL) {
        return Err("Invalid flower pot state".into());
    }
    let inventory = world
        .journal
        .gameplay()
        .inventories
        .get(&peer.uuid)
        .cloned()
        .unwrap_or_default();
    let (facing, next) = transaction(
        state.facing,
        &inventory,
        source,
        peer.held_slot,
        peer.game_mode(&world.config),
    )?;
    if facing == state.facing {
        // A valid empty-hand click on an empty pot needs no state mutation.
        return Ok(true);
    }
    if world.journal.gameplay().revision == u64::MAX {
        return Err("Inventory state exhausted".into());
    }
    let owner = terra_voxel::global_to_section(position).0;
    if world.journal.revision(owner) > u64::MAX - 4 {
        return Err("Flower pot revision exhausted".into());
    }
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let section = owner.offset([x, y, z]).map_err(|e| e.to_string())?;
                if world.journal.mesh_epoch(section) == u64::MAX
                    || world.journal.water_mesh_epoch(section) == u64::MAX
                {
                    return Err("Flower pot mesh revision exhausted".into());
                }
            }
        }
    }
    commit_object_state(
        &mut world.chunks,
        &mut world.journal,
        position,
        Some(terra_world::edits::BlockObjectState { facing, ..state }),
    )?;
    world
        .journal
        .gameplay_mut()
        .inventories
        .insert(peer.uuid, next);
    world.journal.gameplay_mut().revision += 1;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{ItemKind, ResourcePropKind, beds::DyeColor};
    #[test]
    fn all_existing_flowers_and_mushrooms_are_inserted_consumed_and_returned_exactly_once() {
        let items = [
            ItemKind::Prop(ResourcePropKind::Flower),
            ItemKind::Prop(ResourcePropKind::Mushroom),
        ]
        .into_iter()
        .chain(DyeColor::FLOWER_COLORS.map(ItemKind::Flower));
        for item in items {
            let mut bag = Inventory::default();
            assert_eq!(bag.insert(item, 2), 0);
            let source = Some(InventorySource {
                slot: 0,
                revision: bag.revision,
            });
            let (state, next) = transaction(0, &bag, source, Some(0), GameMode::Survival).unwrap();
            assert_eq!(next.count(item), 1);
            assert!(
                transaction(
                    state,
                    &next,
                    Some(InventorySource {
                        slot: 0,
                        revision: next.revision
                    }),
                    Some(0),
                    GameMode::Survival
                )
                .is_err()
            );
            let empty = Some(InventorySource {
                slot: 1,
                revision: next.revision,
            });
            let (state, returned) =
                transaction(state, &next, empty, Some(1), GameMode::Survival).unwrap();
            assert_eq!(state, 0);
            assert_eq!(returned.count(item), 2);
            let (_, same) = transaction(
                0,
                &returned,
                Some(InventorySource {
                    slot: 1,
                    revision: returned.revision,
                }),
                Some(1),
                GameMode::Survival,
            )
            .unwrap();
            assert_eq!(same, returned);
        }
    }
    #[test]
    fn stale_wrong_slot_and_full_inventory_cannot_lose_or_duplicate_pot_contents() {
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Prop(ResourcePropKind::Mushroom), 1);
        let source = Some(InventorySource {
            slot: 0,
            revision: bag.revision,
        });
        assert!(transaction(0, &bag, source, Some(1), GameMode::Survival).is_err());
        assert!(
            transaction(
                0,
                &bag,
                Some(InventorySource {
                    slot: 0,
                    revision: bag.revision + 1
                }),
                Some(0),
                GameMode::Survival
            )
            .is_err()
        );
        assert!(transaction(0, &bag, source, Some(0), GameMode::Spectator).is_err());
        let (_, creative) = transaction(0, &bag, source, Some(0), GameMode::Creative).unwrap();
        assert_eq!(creative, bag);
        for slot in 0..terra_core::INVENTORY_SLOTS {
            bag.creative_set(
                slot,
                terra_core::ItemStack {
                    item: ItemKind::Block(3),
                    count: terra_core::STACK_LIMIT,
                },
            );
        }
        assert!(transaction(9, &bag, None, None, GameMode::Survival).is_err());
        assert_eq!(bag.count(ItemKind::Prop(ResourcePropKind::Mushroom)), 0);
    }
}
