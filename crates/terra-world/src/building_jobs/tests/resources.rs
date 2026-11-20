use super::*;
use terra_core::{GameMode, MineralItem};

fn bag(stacks: &[(ItemKind, u16)]) -> Inventory {
    Inventory {
        slots: stacks
            .iter()
            .map(|(item, count)| ItemStack {
                item: *item,
                count: *count,
            })
            .collect(),
        ..Default::default()
    }
}
fn pair(journal: &mut EditJournal, a: [i64; 3], b: [i64; 3]) {
    for (position, peer, secondary) in [(a, b, false), (b, a, true)] {
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    facing: 0,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
}

#[test]
fn town_stock_counts_real_selected_inventories_and_double_chests_exactly_once() {
    let (mut chunks, mut journal, _, realm, owner) = fixture(0);
    let a = [0, 12, 16];
    let b = [1, 12, 16];
    pair(&mut journal, a, b);
    journal
        .set_container(
            a,
            bag(&[
                (ItemKind::Block(16), 120),
                (ItemKind::Block(3), 50),
                (ItemKind::Mineral(MineralItem::IronIngot), 7),
            ]),
        )
        .unwrap();
    journal
        .set_container(
            b,
            bag(&[
                (ItemKind::Block(16), 80),
                (ItemKind::Mineral(MineralItem::RawIron), 4),
                (ItemKind::Mineral(MineralItem::Coal), 9),
            ]),
        )
        .unwrap();
    let outside = [2, 12, 16];
    set(&mut chunks, outside, 29);
    journal.record(outside, 29).unwrap();
    journal
        .set_container(outside, bag(&[(ItemKind::Block(16), 128)]))
        .unwrap();
    let stock = town_stock(&chunks, &journal, realm, owner);
    assert!(stock.configured && stock.complete && stock.valid());
    assert_eq!(
        stock.chests, 3,
        "one paired chest and two empty single chests"
    );
    assert_eq!(stock.count(ItemKind::Block(16)), 200);
    assert_eq!(
        (stock.wood, stock.stone, stock.iron, stock.coal),
        (200, 50, 11, 9)
    );
    let foreign = town_stock(&chunks, &journal, realm, PlayerUuid::from_u128(999));
    assert!(foreign.items.is_empty() && !foreign.complete);
    journal
        .gameplay_mut()
        .buildings
        .settlements
        .get_mut(&realm)
        .unwrap()
        .storage = Some(BuildArea::new(a, a).unwrap());
    let partial = town_stock(&chunks, &journal, realm, owner);
    assert!(
        !partial.complete && partial.items.is_empty(),
        "both halves must be in the selected authorized storage"
    );
}

#[test]
fn unloaded_or_foreign_chest_halves_are_never_reported_as_available_town_inputs() {
    let (mut chunks, mut journal, _, realm, owner) = fixture(0);
    journal
        .set_container([0, 12, 16], bag(&[(ItemKind::Block(16), 27)]))
        .unwrap();
    let section = global_to_section([0, 12, 16]).0;
    let loaded = chunks.loaded.remove(&section).unwrap();
    let absent = town_stock(&chunks, &journal, realm, owner);
    assert!(!absent.complete && absent.items.is_empty());
    chunks.loaded.insert(section, loaded);
    assert_eq!(
        town_stock(&chunks, &journal, realm, owner).count(ItemKind::Block(16)),
        27
    );
    let a = [15, 12, 16];
    let b = [16, 12, 16];
    for cell in [a, b] {
        set(&mut chunks, cell, 29);
        journal.record(cell, 29).unwrap();
        journal
            .set_container(
                cell,
                bag(&[(ItemKind::Mineral(MineralItem::IronIngot), 17)]),
            )
            .unwrap();
    }
    pair(&mut journal, a, b);
    journal
        .gameplay_mut()
        .kingdoms
        .found(PlayerUuid::from_u128(555), [31, 12, 31], "Foreign")
        .unwrap();
    journal
        .gameplay_mut()
        .buildings
        .settlements
        .get_mut(&realm)
        .unwrap()
        .storage = Some(BuildArea::new(a, b).unwrap());
    let protected = town_stock(&chunks, &journal, realm, owner);
    assert!(!protected.complete && protected.items.is_empty());
}

#[test]
fn town_inventory_budget_marks_partial_results_instead_of_claiming_an_exact_truncated_total() {
    let (mut chunks, mut journal, _, realm, owner) = fixture(0);
    journal
        .gameplay_mut()
        .kingdoms
        .claim(
            realm,
            [1, 1],
            ClaimKind::ProtectedBuilding,
            terra_core::kingdom::ClaimActor::Player(owner),
        )
        .unwrap();
    // Keep the exact stress volume above the registered keep at Y=12.
    let area = BuildArea::new([0, 14, 16], [31, 29, 31]).unwrap();
    journal
        .gameplay_mut()
        .buildings
        .settlements
        .get_mut(&realm)
        .unwrap()
        .storage = Some(area);
    for index in 0..=MAX_TOWN_CHESTS {
        let cell = area.cell_at(index).unwrap();
        set(&mut chunks, cell, 29);
        journal.record(cell, 29).unwrap();
        journal
            .set_container(cell, bag(&[(ItemKind::Mineral(MineralItem::Coal), 1)]))
            .unwrap();
    }
    let stock = town_stock(&chunks, &journal, realm, owner);
    assert!(stock.valid() && !stock.complete);
    assert!(usize::from(stock.chests) <= MAX_TOWN_CHESTS);
    assert!(stock.coal <= MAX_TOWN_CHESTS as u32);
}

#[test]
fn creative_owner_builds_real_voxels_without_storage_costs_or_generated_refund_items_and_policy_survives_reload()
 {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    journal
        .gameplay_mut()
        .buildings
        .settlements
        .get_mut(&realm)
        .unwrap()
        .storage = None;
    journal
        .gameplay_mut()
        .player_stats
        .entry(owner)
        .or_default()
        .game_mode = Some(GameMode::Creative);
    let mut job = simple_job(realm, owner, vec![], [8, 12, 24]);
    job.steps = vec![
        BuildStep {
            phase: BuildPhase::Walls,
            cells: vec![BuildCell {
                position: [8, 12, 24],
                material: 16,
                facing: 0,
                peer: None,
                secondary: false,
            }],
            costs: vec![ItemStack {
                item: ItemKind::Block(16),
                count: 1,
            }],
        },
        BuildStep {
            phase: BuildPhase::Cleanup,
            cells: vec![BuildCell {
                position: [8, 12, 24],
                material: 0,
                facing: 0,
                peer: None,
                secondary: false,
            }],
            costs: vec![],
        },
        BuildStep {
            phase: BuildPhase::Walls,
            cells: vec![BuildCell {
                position: [8, 12, 24],
                material: 18,
                facing: 0,
                peer: None,
                secondary: false,
            }],
            costs: vec![ItemStack {
                item: ItemKind::Block(18),
                count: 1,
            }],
        },
    ];
    let id = journal.gameplay_mut().buildings.insert(job).unwrap();
    let mut runtime = BuildRuntime::default();
    runtime.set_creative_owners([owner]);
    let mut restored = false;
    let mut placed = false;
    for _ in 0..50_000 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &BlockPropertiesCatalog::default(),
            0.1,
            &mut runtime,
        );
        placed |= chunks.block_at([8, 12, 24]) == Some(16);
        if !restored && journal.gameplay().buildings.jobs[&id].next_step > 0 {
            let text = serde_json::to_string(&journal.gameplay().buildings).unwrap();
            journal.gameplay_mut().buildings = serde_json::from_str(&text).unwrap();
            runtime = BuildRuntime::default();
            runtime.set_creative_owners(journal.gameplay().player_stats.iter().filter_map(
                |(id, stats)| (stats.game_mode == Some(GameMode::Creative)).then_some(*id),
            ));
            restored = true;
        }
        if journal.gameplay().buildings.jobs[&id].status == BuildStatus::Complete {
            break;
        }
    }
    let built = &journal.gameplay().buildings.jobs[&id];
    assert!(built.free_resources && built.free_from_step == 0 && restored && placed);
    assert_eq!(built.status, BuildStatus::Complete);
    assert_eq!(chunks.block_at([8, 12, 24]), Some(18));
    assert!(
        totals(&journal).is_empty(),
        "Creative funding must not mint items into real bags or chests"
    );
    assert!(journal.gameplay().drops.is_empty());
}

#[test]
fn creative_funding_loss_pauses_progress_and_does_not_spend_or_refund_stock() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let mut job = simple_job(realm, owner, vec![], [8, 12, 24]);
    job.free_resources = true;
    let id = journal.gameplay_mut().buildings.insert(job).unwrap();
    let before = totals(&journal);
    let mut runtime = BuildRuntime::default();
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &BlockPropertiesCatalog::default(),
            0.1,
            &mut runtime,
        );
    }
    let paused = &journal.gameplay().buildings.jobs[&id];
    assert_eq!(paused.status, BuildStatus::Waiting(BuildWait::Protected));
    assert_eq!((paused.next_step, paused.work_seconds), (0, 0.0));
    assert_eq!(totals(&journal), before);
    runtime.set_creative_owners([owner]);
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &BlockPropertiesCatalog::default(),
            0.1,
            &mut runtime,
        );
    }
    assert_ne!(
        journal.gameplay().buildings.jobs[&id].status,
        BuildStatus::Waiting(BuildWait::Protected)
    );
}

#[test]
fn explicit_double_chest_steps_cost_two_items_initialize_both_inventories_and_do_not_link_neighbors()
 {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let a = [8, 12, 24];
    let b = [9, 12, 24];
    let neighbor = [10, 12, 24];
    set(&mut chunks, neighbor, 29);
    journal.record(neighbor, 29).unwrap();
    journal
        .set_container(neighbor, bag(&[(ItemKind::Mineral(MineralItem::Coal), 7)]))
        .unwrap();
    let mut job = simple_job(realm, owner, vec![], a);
    job.steps = vec![BuildStep {
        phase: BuildPhase::Furnishing,
        cells: vec![
            BuildCell {
                position: a,
                material: 29,
                facing: 0,
                peer: Some(b),
                secondary: false,
            },
            BuildCell {
                position: b,
                material: 29,
                facing: 0,
                peer: Some(a),
                secondary: true,
            },
        ],
        costs: vec![ItemStack {
            item: ItemKind::Block(29),
            count: 2,
        }],
    }];
    job.work_seconds = 1799.9;
    stock(&mut journal, job.required_materials());
    // stock() touches only the four actual town chests, never the neighboring chest.
    let id = journal.gameplay_mut().buildings.insert(job).unwrap();
    let mut runtime = BuildRuntime::default();
    for _ in 0..2000 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &BlockPropertiesCatalog::default(),
            0.1,
            &mut runtime,
        );
        if journal.gameplay().buildings.jobs[&id].status == BuildStatus::Complete {
            break;
        }
    }
    assert_eq!(
        journal.gameplay().buildings.jobs[&id].status,
        BuildStatus::Complete
    );
    for (position, peer) in [(a, b), (b, a)] {
        assert_eq!(chunks.block_at(position), Some(29));
        assert_eq!(journal.object_state(position).unwrap().peer, Some(peer));
        assert!(journal.container_at(position).unwrap().slots.is_empty());
    }
    assert!(journal.object_state(neighbor).is_none());
    assert_eq!(
        journal
            .container_at(neighbor)
            .unwrap()
            .count(ItemKind::Mineral(MineralItem::Coal)),
        7
    );
    assert_eq!(
        totals(&journal)
            .get(&ItemKind::Block(29))
            .copied()
            .unwrap_or(0),
        0
    );
}

#[test]
fn entering_creative_preserves_paid_scaffold_refunds_and_never_refunds_later_free_materials() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let position = [8, 12, 24];
    let mut job = simple_job(realm, owner, vec![], position);
    let placed = |material| BuildCell {
        position,
        material,
        facing: 0,
        peer: None,
        secondary: false,
    };
    job.steps = vec![
        BuildStep {
            phase: BuildPhase::Walls,
            cells: vec![placed(16)],
            costs: vec![ItemStack {
                item: ItemKind::Block(16),
                count: 1,
            }],
        },
        BuildStep {
            phase: BuildPhase::Cleanup,
            cells: vec![placed(0)],
            costs: vec![],
        },
        BuildStep {
            phase: BuildPhase::Walls,
            cells: vec![placed(18)],
            costs: vec![ItemStack {
                item: ItemKind::Block(18),
                count: 1,
            }],
        },
    ];
    job.work_seconds = 599.9;
    stock(
        &mut journal,
        vec![ItemStack {
            item: ItemKind::Block(16),
            count: 1,
        }],
    );
    let id = journal.gameplay_mut().buildings.insert(job).unwrap();
    let mut runtime = BuildRuntime::default();
    for _ in 0..2000 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &BlockPropertiesCatalog::default(),
            0.1,
            &mut runtime,
        );
        if journal.gameplay().buildings.jobs[&id].next_step == 1 {
            break;
        }
    }
    assert_eq!(chunks.block_at(position), Some(16));
    assert_eq!(
        totals(&journal)
            .get(&ItemKind::Block(16))
            .copied()
            .unwrap_or(0),
        0
    );
    runtime.set_creative_owners([owner]);
    for _ in 0..30_000 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &BlockPropertiesCatalog::default(),
            0.1,
            &mut runtime,
        );
        if journal.gameplay().buildings.jobs[&id].status == BuildStatus::Complete {
            break;
        }
    }
    let done = &journal.gameplay().buildings.jobs[&id];
    assert_eq!(done.status, BuildStatus::Complete);
    assert_eq!(done.free_from_step, 1);
    assert_eq!(totals(&journal).get(&ItemKind::Block(16)), Some(&1));
    assert_eq!(chunks.block_at(position), Some(18));
    assert_eq!(totals(&journal).get(&ItemKind::Block(18)), None);
}

#[test]
fn paid_native_spruce_basin_keeps_its_wood_across_replication_and_drop_cost_roundtrip() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let position = [8, 12, 24];
    let input = ItemKind::Block(169);
    let mut job = simple_job(realm, owner, vec![], position);
    job.steps = vec![BuildStep {
        phase: BuildPhase::Furnishing,
        cells: vec![BuildCell {
            position,
            material: 169,
            facing: 0,
            peer: None,
            secondary: false,
        }],
        costs: vec![ItemStack {
            item: input,
            count: 1,
        }],
    }];
    job.work_seconds = 1799.9;
    stock(&mut journal, job.required_materials());
    let id = journal.gameplay_mut().buildings.insert(job).unwrap();
    let mut runtime = BuildRuntime::default();
    for _ in 0..2000 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &BlockPropertiesCatalog::default(),
            0.1,
            &mut runtime,
        );
        if journal.gameplay().buildings.jobs[&id].status == BuildStatus::Complete {
            break;
        }
    }
    assert_eq!(
        journal.gameplay().buildings.jobs[&id].status,
        BuildStatus::Complete
    );
    assert_eq!(
        journal.object_state(position).unwrap().wood,
        terra_core::WoodSpecies::Spruce
    );
    assert_eq!(totals(&journal).get(&input), None);
    let mut replica = EditJournal::default();
    replica
        .import_delta(journal.section_delta(global_to_section(position).0))
        .unwrap();
    assert_eq!(
        replica.object_state(position),
        journal.object_state(position)
    );
    let recovered = crate::placement::picked_item(&chunks, &replica, position).unwrap();
    assert_eq!(recovered, input);
    assert!(
        crate::streaming::commit_block_edit(
            &mut chunks,
            &mut journal,
            &crate::streaming::BlockEditRequest {
                position,
                expected_material: 169,
                material: 0
            },
            &BlockPropertiesCatalog::default()
        )
        .unwrap()
    );
    assert!(journal.gameplay_mut().drop_stack(
        position.map(|v| v as f64 + 0.5),
        ItemStack {
            item: recovered,
            count: 1
        },
        [0.0; 3],
        0.0
    ));
    assert_eq!(journal.gameplay().drops.len(), 1);
    assert_eq!(journal.gameplay().drops[0].stack.item, input);
}

#[test]
fn ornate_town_chest_counts_and_supplies_real_building_material_without_pairing() {
    let (mut chunks, mut journal, _, realm, owner) = fixture(0);
    let chest = [0, 12, 16];
    let material = terra_core::grave_blocks::ORNATE_CHEST;
    let item = ItemKind::Block(16);
    set(&mut chunks, chest, material);
    journal.record(chest, material).unwrap();
    journal
        .set_object_state(
            chest,
            Some(BlockObjectState {
                facing: 3,
                ..Default::default()
            }),
        )
        .unwrap();
    journal.set_container(chest, bag(&[(item, 8)])).unwrap();
    let report = town_stock(&chunks, &journal, realm, owner);
    assert!(report.complete && report.valid());
    assert_eq!(report.count(item), 8);
    assert_eq!(report.chests, 4);
    let mut job = simple_job(realm, owner, vec![], [8, 12, 24]);
    job.steps[0].costs = vec![ItemStack { item, count: 1 }];
    let area = journal.gameplay().buildings.settlements[&realm]
        .storage
        .unwrap();
    assert!(supplies::chests(&chunks, &journal, &job, area).contains(&chest));
    let mut worker = Inventory::default();
    assert!(supplies::load(&mut journal, &job, &mut worker, chest));
    assert!(worker.count(item) > 0);
    assert_eq!(
        worker.count(item) + journal.container_at(chest).unwrap().count(item),
        8
    );
    assert_eq!(journal.object_state(chest).unwrap().peer, None);
}
