//! Read actual, loaded town chests with the viewer's live realm permissions.
use super::*;
use std::collections::BTreeSet;

pub fn town_stock(
    chunks: &ChunkStore,
    journal: &EditJournal,
    kingdom: KingdomId,
    viewer: PlayerUuid,
) -> TownStock {
    let Some(area) = journal
        .gameplay()
        .buildings
        .settlements
        .get(&kingdom)
        .and_then(|s| s.storage)
    else {
        return TownStock::default();
    };
    let mut result = TownStock {
        configured: true,
        complete: true,
        ..Default::default()
    };
    if !area.valid() {
        result.complete = false;
        return result;
    }
    let registry = &journal.gameplay().kingdoms;
    let realm = registry.realm_id(kingdom);
    if viewer.is_nil()
        || !registry.get(realm).is_some_and(|r| {
            r.state == terra_core::kingdom::KingdomState::Active && r.members.contains(&viewer)
        })
    {
        result.complete = false;
        return result;
    }
    let minimum = terra_voxel::global_to_section(area.min).0;
    let maximum = terra_voxel::global_to_section(area.max).0;
    for x in minimum.x..=maximum.x {
        for y in minimum.y..=maximum.y {
            for z in minimum.z..=maximum.z {
                result.complete &= chunks
                    .loaded
                    .contains_key(&terra_voxel::SectionPos::new(x, y, z));
            }
        }
    }
    let readable = |p| {
        area.contains(p)
            && chunks
                .block_at(p)
                .is_some_and(terra_core::containers::is_chest)
            && registry.claim_at(claim_chunk(p)).is_some_and(|c| {
                c.kind == ClaimKind::ProtectedBuilding && registry.realm_id(c.kingdom) == realm
            })
            && crate::kingdom_access::can_use(
                registry,
                chunks,
                viewer,
                KingdomPermission::Inventories,
                p,
            )
            && crate::kingdom_access::can_use(
                registry,
                chunks,
                viewer,
                KingdomPermission::NpcWork,
                p,
            )
    };
    let mut seen = BTreeSet::new();
    for (index, (position, _)) in journal
        .containers_in_area(area.min, area.max)
        .take(MAX_TOWN_CHESTS + 1)
        .enumerate()
    {
        if index == MAX_TOWN_CHESTS {
            result.complete = false;
            break;
        }
        if seen.contains(&position) {
            continue;
        }
        if chunks
            .block_at(position)
            .is_some_and(|material| !terra_core::containers::is_chest(material))
        {
            continue;
        }
        if !readable(position) {
            result.complete = false;
            continue;
        }
        let mut cells = vec![position];
        if let Some(state) = journal.object_state(position) {
            if let Some(peer) = state.peer {
                let valid_pair = readable(peer)
                    && journal.object_state(peer).is_some_and(|other| {
                        other.peer == Some(position)
                            && other.secondary != state.secondary
                            && other.facing == state.facing
                    })
                    && position[1] == peer[1]
                    && position
                        .into_iter()
                        .zip(peer)
                        .map(|(a, b)| (a - b).abs())
                        .sum::<i64>()
                        == 1;
                if !valid_pair {
                    result.complete = false;
                    continue;
                }
                cells.push(peer);
            } else if state.secondary {
                result.complete = false;
                continue;
            }
        }
        if cells.iter().any(|p| {
            journal
                .container_at(*p)
                .is_none_or(|bag| !terra_core::containers::valid_chest(bag))
        }) {
            result.complete = false;
            continue;
        }
        result.chests += 1;
        for cell in cells {
            if !seen.insert(cell) {
                continue;
            }
            if let Some(bag) = journal.container_at(cell) {
                for stack in bag.slots.iter().filter(|s| s.valid()) {
                    if !result.items.contains_key(&stack.item)
                        && result.items.len() >= MAX_TOWN_STOCK_ITEMS
                    {
                        result.complete = false;
                        continue;
                    }
                    *result.items.entry(stack.item).or_default() += u32::from(stack.count);
                }
            }
        }
    }
    result.summarize();
    result
}
