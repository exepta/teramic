use super::*;
use terra_core::{Inventory, ItemKind, ItemStack, WorldDrop};

fn drop_at(id: u64, x: f64) -> WorldDrop {
    WorldDrop {
        id,
        position: [x, 1., 0.],
        stack: ItemStack {
            item: ItemKind::Block(3),
            count: 1,
        },
        velocity: [0.; 3],
        age_seconds: 0.,
        rotation: [0., 0., 0., 1.],
        angular_velocity: [0.; 3],
        grounded: true,
        pickup_delay_seconds: 0.,
    }
}

#[test]
fn private_plumbing_revisions_do_not_change_the_cached_gameplay_payload() {
    let mut journal = EditJournal::default();
    let position = [-1, 16, -1];
    journal
        .record(position, terra_core::plumbing::IRON_PIPE)
        .unwrap();
    journal
        .set_object_state(position, Some(Default::default()))
        .unwrap();
    let inventory = Inventory {
        slots: vec![ItemStack {
            item: ItemKind::GlassBottle,
            count: 2,
        }],
        ..Default::default()
    };
    let drop = drop_at(1, 3.);
    let drops = [(9., &drop)];
    let cached = CachedGameplay::capture(3, &inventory, &drops);
    let before = journal.gameplay().revision;
    let inventory_buffer = cached.inventory.slots.as_ptr();
    let drop_buffer = cached.drops.as_ptr();
    for amount in 0..=8 {
        journal
            .set_plumbing(
                position,
                terra_world::plumbing::PlumbingState {
                    water: terra_world::water::FluidCell {
                        amount,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(cached.matches(3, &inventory, &drops));
        assert_eq!(cached.inventory.slots.as_ptr(), inventory_buffer);
        assert_eq!(cached.drops.as_ptr(), drop_buffer);
    }
    assert!(journal.gameplay().revision > before);
}

#[test]
fn initial_empty_payload_and_new_generation_require_a_send_but_new_center_alone_does_not() {
    let inventory = Inventory::default();
    let mut cached: Option<CachedGameplay> = None;
    let unchanged = |cached: &Option<CachedGameplay>, generation| {
        cached
            .as_ref()
            .is_some_and(|previous| previous.matches(generation, &inventory, &[]))
    };
    assert!(
        !unchanged(&cached, 0),
        "a new empty peer still receives its initial state"
    );
    cached = Some(CachedGameplay::capture(0, &inventory, &[]));
    assert!(unchanged(&cached, 0));
    assert!(
        !unchanged(&cached, 1),
        "a new generation must receive state again"
    );
    let drop = drop_at(1, 3.);
    let cached = CachedGameplay::capture(1, &inventory, &[(9., &drop)]);
    assert!(
        cached.matches(1, &inventory, &[(121., &drop)]),
        "movement changes distance/center but not an identical visible payload"
    );
}

#[test]
fn inventory_identity_revision_and_every_visible_drop_change_invalidate_the_payload() {
    let inventory = Inventory {
        slots: vec![ItemStack {
            item: ItemKind::TintedBlock {
                material: 103,
                wood: terra_core::WoodSpecies::Oak,
                color: [10, 20, 30],
            },
            count: 2,
        }],
        ..Default::default()
    };
    let first = drop_at(1, 3.);
    let second = drop_at(2, 4.);
    let cached = CachedGameplay::capture(0, &inventory, &[(9., &first), (16., &second)]);
    for mutation in 0..4 {
        let mut changed = inventory.clone();
        match mutation {
            0 => changed.revision += 1,
            1 => changed.slots[0].count -= 1,
            2 => {
                changed.slots[0].item = ItemKind::TintedBlock {
                    material: 103,
                    wood: terra_core::WoodSpecies::Oak,
                    color: [30, 20, 10],
                }
            }
            _ => changed.cursor = Some(changed.slots[0]),
        }
        assert!(!cached.matches(0, &changed, &[(9., &first), (16., &second)]));
    }
    for mutation in 0..6 {
        let mut changed = first.clone();
        match mutation {
            0 => changed.position[0] += 0.1,
            1 => changed.stack.count += 1,
            2 => changed.age_seconds += 1.,
            3 => changed.velocity[0] = 1.,
            4 => changed.grounded = false,
            _ => changed.pickup_delay_seconds = 0.2,
        }
        assert!(!cached.matches(0, &inventory, &[(9., &changed), (16., &second)]));
    }
    assert!(
        !cached.matches(0, &inventory, &[(9., &first)]),
        "leaving visibility must remove a drop"
    );
    assert!(!cached.matches(0, &inventory, &[]));
    assert!(!cached.matches(0, &inventory, &[(16., &second), (9., &first)]));
    assert!(cached.matches(0, &inventory, &[(9., &first), (16., &second)]));
}
