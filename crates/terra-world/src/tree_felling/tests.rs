use super::*;
use crate::{SectionGenerator, TerraGenerator, streaming::LoadedSection};
use terra_core::{WorldConfig, WorldSeed};
use terra_voxel::BlockSection;

fn fixture() -> (ChunkStore, EditJournal, Shape) {
    let generator = SectionGenerator::new(
        TerraGenerator::new(WorldConfig {
            seed: WorldSeed(42042),
            landscape_revision: 3,
            sea_level: 8.0,
            base_height: 12.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            cave_structure_revision: 0,
            rivers: false,
            water: false,
            biomes: false,
            trees: true,
            tree_density: 1.0,
            ..Default::default()
        })
        .unwrap(),
    );
    let mut shape = None;
    'found: for z in -2..=0 {
        for x in -2..=0 {
            let column = generator.prepare_column([x, z], false).unwrap();
            for lz in 0..16 {
                for lx in 0..16 {
                    let p = [x * 16 + lx, 12, z * 16 + lz];
                    if let Some(tree) = column.felling_shape(p)
                        && column.felling_owner(tree.origin, tree.wood_material)
                            == Some(tree.origin)
                    {
                        shape = Some(tree);
                        break 'found;
                    }
                }
            }
        }
    }
    let shape = shape.expect("the dense deterministic fixture contains a natural tree");
    let mut chunks = ChunkStore::default();
    for (&cell, material) in shape
        .wood
        .iter()
        .map(|p| (p, shape.wood_material))
        .chain(shape.leaves.iter().map(|p| (p, shape.leaves_material)))
    {
        let (owner, local) = global_to_section(cell);
        chunks
            .loaded
            .entry(owner)
            .or_insert_with(|| LoadedSection {
                data: BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            })
            .data
            .set_block(local, material)
            .unwrap();
        let coordinate = [owner.x, owner.z];
        if chunks.prepared_column(coordinate).is_none() {
            chunks.set_prepared_column_for_test(
                coordinate,
                Some(Arc::new(
                    generator.prepare_column(coordinate, false).unwrap(),
                )),
            );
        }
    }
    (chunks, EditJournal::default(), shape)
}

#[test]
fn natural_tree_plan_is_deterministic_across_negative_section_seams_and_protects_edited_logs() {
    let (chunks, mut journal, shape) = fixture();
    assert!(shape.origin[0] < 0 && shape.origin[2] < 0);
    let plan = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
    let again = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
    assert_eq!(plan.wood, again.wood);
    assert_eq!(plan.leaves, again.leaves);
    assert_eq!(WOOD_SECONDS, 0.1);
    assert_eq!(plan.duration(), plan.wood_count() as f32 * WOOD_SECONDS);
    let expected: std::collections::HashSet<_> = shape
        .wood
        .iter()
        .copied()
        .filter(|p| {
            chunks.block_at(*p) == Some(shape.wood_material)
                && chunks
                    .prepared_column([p[0].div_euclid(16), p[2].div_euclid(16)])
                    .is_some_and(|column| {
                        column.felling_owner(*p, shape.wood_material) == Some(shape.origin)
                    })
        })
        .collect();
    assert_eq!(
        plan.wood_positions()
            .collect::<std::collections::HashSet<_>>(),
        expected
    );
    assert!(
        expected.iter().any(|p| p[1] > shape.origin[1] + 4
            && p[0]
                .abs_diff(shape.origin[0])
                .max(p[2].abs_diff(shape.origin[2]))
                >= 3),
        "upper branches, not only the trunk, must be timed and removed"
    );
    assert!(plan.wood_count() > 20 && plan.wood_count() <= MAX_WOOD);
    assert!(
        plan.revisions.len() > 1,
        "tree must cross real section seams"
    );
    let protected = plan.wood[0];
    assert_ne!(protected, shape.origin);
    journal.record(protected, shape.wood_material).unwrap();
    let edited = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
    assert_eq!(edited.wood_count(), plan.wood_count() - 1);
    assert!(!edited.wood.contains(&protected));
    assert_eq!(
        edited.duration(),
        (plan.wood_count() - 1) as f32 * WOOD_SECONDS
    );
    assert!(!plan.unchanged(&chunks, &journal));
    assert!(eligible(
        &chunks,
        &journal,
        [shape.origin[0], shape.origin[1] + 2, shape.origin[2]]
    ));
    journal.record(shape.origin, shape.wood_material).unwrap();
    assert!(!eligible(&chunks, &journal, shape.origin));
    assert!(Plan::prepare(&chunks, &journal, shape.origin).is_none());
}

#[test]
fn every_owned_branch_can_start_the_same_complete_tree_cut_and_uses_its_base_pivot() {
    let (chunks, journal, shape) = fixture();
    let base = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
    let branch = base
        .wood_positions()
        .find(|cell| {
            cell[1] > shape.origin[1] + 4
                && cell[0]
                    .abs_diff(shape.origin[0])
                    .max(cell[2].abs_diff(shape.origin[2]))
                    >= 3
        })
        .unwrap();
    let upper = Plan::prepare(&chunks, &journal, branch).unwrap();
    assert!(eligible(&chunks, &journal, branch));
    assert_eq!(upper.wood, base.wood);
    assert_eq!(upper.leaves, base.leaves);
    assert_eq!(upper.foot(), branch);
    assert_eq!(
        upper.collapse([1.0, 0.0]).visual_origin(),
        base.collapse([1.0, 0.0]).visual_origin()
    );
    assert!(upper.collapse([1.0, 0.0]).valid());
    for &cell in &base.wood {
        let targeted = Plan::prepare(&chunks, &journal, cell).unwrap();
        assert_eq!(targeted.wood, base.wood, "owned cell {cell:?}");
    }
    let mut queue = Queue::default();
    assert!(queue.enqueue(base, PlayerUuid::new_v4()));
    assert!(
        !queue.enqueue(upper, PlayerUuid::new_v4()),
        "two players clicking different branches must not enqueue the same tree twice"
    );
}

#[test]
fn gold_and_ruby_timer_discounts_never_remove_or_drop_fewer_logs() {
    for axe in [terra_core::ToolKind::GoldAxe, terra_core::ToolKind::RubyAxe] {
        let (mut chunks, mut journal, shape) = fixture();
        let plan = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
        let count = plan.wood_count();
        let cells: Vec<_> = plan.wood_positions().collect();
        assert!(plan.duration_with_tool(Some(ItemKind::Tool(axe))) < plan.duration());
        let mut queue = Queue::default();
        assert!(queue.enqueue(plan, PlayerUuid::new_v4()));
        queue
            .finish(&mut chunks, &mut journal, &Default::default())
            .unwrap();
        assert!(cells.iter().all(|&cell| journal.block(cell) == Some(0)));
        assert_eq!(
            journal
                .gameplay()
                .drops
                .iter()
                .filter(|drop| drop.stack.item == ItemKind::Block(shape.wood_material))
                .map(|drop| usize::from(drop.stack.count))
                .sum::<usize>(),
            count
        );
    }
}

#[test]
fn axe_tiers_have_exact_per_log_timing_and_only_gold_and_ruby_skip_every_third() {
    use terra_core::ToolKind;
    for (axe, seconds, skip) in [
        (ToolKind::WoodAxe, 0.1, false),
        (ToolKind::StoneAxe, 0.0925, false),
        (ToolKind::IronAxe, 0.08, false),
        (ToolKind::GoldAxe, 0.09, true),
        (ToolKind::DiamondAxe, 0.05, false),
        (ToolKind::RubyAxe, 0.05, true),
    ] {
        assert!(is_axe(Some(ItemKind::Tool(axe))));
        for logs in [1, 2, 3, 4, 5, 6, 37, MAX_WOOD] {
            assert_eq!(
                duration(logs, Some(ItemKind::Tool(axe))),
                (logs - if skip { logs / 3 } else { 0 }) as f32 * seconds
            );
        }
    }
    assert!(!is_axe(None));
    assert!(!is_axe(Some(ItemKind::Tool(ToolKind::DiamondPickaxe))));
    let (chunks, journal, shape) = fixture();
    let plan = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
    let cells: Vec<_> = plan.wood_positions().collect();
    for axe in [ToolKind::GoldAxe, ToolKind::RubyAxe] {
        assert!(plan.duration_with_tool(Some(ItemKind::Tool(axe))) < plan.duration());
        assert_eq!(plan.wood_positions().collect::<Vec<_>>(), cells);
    }
}

#[test]
fn creative_felling_removes_all_owned_wood_without_spawning_free_log_stacks() {
    let (mut chunks, mut journal, shape) = fixture();
    let plan = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
    let cells: Vec<_> = plan.wood_positions().collect();
    let mut queue = Queue::default();
    assert!(queue.enqueue_with_drops(plan, PlayerUuid::new_v4(), false));
    queue
        .finish(&mut chunks, &mut journal, &Default::default())
        .unwrap();
    assert!(cells.iter().all(|&cell| journal.block(cell) == Some(0)));
    assert!(journal.gameplay().drops.is_empty());
}

#[test]
fn whole_tree_batches_preserve_late_builds_count_exact_drops_and_delay_separate_leaf_decay() {
    let (mut chunks, mut journal, shape) = fixture();
    let plan = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
    let expected = plan.wood_count();
    let log = plan.wood[0];
    let leaf = *plan.leaves.first().expect("natural crown");
    let owner = PlayerUuid::new_v4();
    let mut queue = Queue::default();
    assert!(queue.enqueue(plan, owner));
    journal.record(log, shape.wood_material).unwrap();
    journal.record(leaf, shape.leaves_material).unwrap();
    while queue.0.iter().any(|job| !job.wood.is_empty()) {
        let batch = queue
            .advance(&mut chunks, &mut journal, &Default::default(), 0.0)
            .unwrap();
        assert!(batch.wood <= WOOD_BUDGET && batch.leaves == 0 && batch.drops <= 4);
    }
    assert_eq!(chunks.block_at(log), Some(shape.wood_material));
    assert_eq!(chunks.block_at(leaf), Some(shape.leaves_material));
    let dropped: usize = journal
        .gameplay()
        .drops
        .iter()
        .filter(|drop| drop.stack.item == ItemKind::Block(shape.wood_material))
        .map(|drop| usize::from(drop.stack.count))
        .sum();
    assert_eq!(dropped, expected - 1);
    assert_eq!(
        journal.gameplay().player_stats[&owner].blocks_broken,
        (expected - 1) as u64
    );
    assert!(journal.gameplay().valid());
    assert_eq!(
        queue
            .advance(
                &mut chunks,
                &mut journal,
                &Default::default(),
                LEAF_DELAY - 0.01
            )
            .unwrap()
            .leaves,
        0
    );
    let decay = queue
        .advance(&mut chunks, &mut journal, &Default::default(), LEAF_DELAY)
        .unwrap();
    assert!(decay.leaves > 0 && decay.leaves <= LEAF_BUDGET);
    queue
        .finish(&mut chunks, &mut journal, &Default::default())
        .unwrap();
    assert!(queue.is_empty());
    assert_eq!(journal.block(log), Some(shape.wood_material));
    assert_eq!(journal.block(leaf), Some(shape.leaves_material));
    let mut reloaded = EditJournal::default();
    for position in journal.changed_sections() {
        reloaded
            .import_delta(journal.section_delta(position))
            .unwrap();
    }
    assert_eq!(reloaded.block(shape.origin), Some(0));
    assert_eq!(reloaded.block(log), Some(shape.wood_material));
    assert_eq!(reloaded.block(leaf), Some(shape.leaves_material));
}

#[test]
fn an_unloaded_upper_trunk_cannot_silently_reduce_a_whole_tree_timer() {
    let (mut chunks, journal, shape) = fixture();
    let plan = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
    let foot = global_to_section(shape.origin).0;
    let upper = plan
        .revisions
        .iter()
        .map(|entry| entry.0)
        .find(|p| *p != foot)
        .unwrap();
    let missing = chunks.loaded.remove(&upper).unwrap();
    assert!(!plan.unchanged(&chunks, &journal));
    assert!(Plan::prepare(&chunks, &journal, shape.origin).is_none());
    request_missing(&mut chunks, &journal, shape.origin);
    request_missing(&mut chunks, &journal, shape.origin);
    assert!(chunks.is_wanted(upper));
    assert!(chunks.is_wanted(foot));
    chunks.loaded.insert(upper, missing);
    assert!(Plan::prepare(&chunks, &journal, shape.origin).is_some());
}

#[test]
fn final_save_drains_confirmed_felling_after_streaming_eviction_without_regrowing_wood() {
    let (mut chunks, mut journal, shape) = fixture();
    let plan = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
    let expected = plan.wood_count();
    let cells = plan.wood.clone();
    let mut queue = Queue::default();
    assert!(queue.enqueue(plan.clone(), PlayerUuid::new_v4()));
    assert!(
        !queue.enqueue(plan, PlayerUuid::new_v4()),
        "one job per natural tree"
    );
    chunks.loaded.clear();
    queue
        .finish(&mut chunks, &mut journal, &Default::default())
        .unwrap();
    assert!(cells.into_iter().all(|p| journal.block(p) == Some(0)));
    assert_eq!(
        journal
            .gameplay()
            .drops
            .iter()
            .filter(|d| d.stack.item == ItemKind::Block(shape.wood_material))
            .map(|d| usize::from(d.stack.count))
            .sum::<usize>(),
        expected
    );
    assert!(journal.gameplay().valid());
}

#[test]
fn full_drop_budget_keeps_pending_tree_blocks_and_items_intact() {
    let (mut chunks, mut journal, shape) = fixture();
    let plan = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
    let mut queue = Queue::default();
    assert!(queue.enqueue(plan, PlayerUuid::new_v4()));
    for _ in 0..terra_core::MAX_WORLD_DROPS {
        assert!(
            journal
                .gameplay_mut()
                .drop_one([0.5; 3], ItemKind::Block(2))
        );
    }
    let batch = queue
        .advance(&mut chunks, &mut journal, &Default::default(), 0.0)
        .unwrap();
    assert_eq!(batch.wood, 0);
    assert_eq!(journal.block(shape.origin), None);
    assert!(
        queue
            .finish(&mut chunks, &mut journal, &Default::default())
            .is_err()
    );
    journal.gameplay_mut().drops.clear();
    queue
        .finish(&mut chunks, &mut journal, &Default::default())
        .unwrap();
    assert_eq!(journal.block(shape.origin), Some(0));
}

#[test]
fn deferred_canopy_retains_settlement_domain_after_column_eviction_and_rechecks_kicks_and_capture()
{
    use crate::generation::structures::{
        SettlementLayout, VillageSettlement, VillageSize, VillageStyle,
    };
    use terra_core::kingdom::{KingdomPermission, KingdomRole};
    for change in ["authorized", "kick", "capture"] {
        let (mut chunks, mut journal, shape) = fixture();
        let actor = PlayerUuid::from_u128(1);
        let founder = PlayerUuid::from_u128(2);
        let all: Vec<_> = shape.wood.iter().chain(&shape.leaves).copied().collect();
        let low: [i64; 3] =
            std::array::from_fn(|axis| all.iter().map(|p| p[axis]).min().unwrap() - 1);
        let mut high: [i64; 3] =
            std::array::from_fn(|axis| all.iter().map(|p| p[axis]).max().unwrap() + 1);
        // The only explicit claim is the distant settlement center; every tree
        // cell is protected by the captured immutable footprint, not its chunk.
        let center = [high[0].div_euclid(16) * 16 + 32, shape.origin[1], low[2]];
        high[0] = center[0] + 1;
        let settlement = Arc::new(VillageSettlement {
            id: 0x199,
            size: VillageSize::Small,
            layout: SettlementLayout::Village,
            style: VillageStyle::Temperate,
            center,
            bounds: [low, high],
            homes: Vec::new(),
            services: Vec::new(),
            residents: Vec::new(),
        });
        let coordinate = [
            shape.origin[0].div_euclid(16),
            shape.origin[2].div_euclid(16),
        ];
        let column = chunks.prepared_column(coordinate).unwrap();
        chunks.set_prepared_column_for_test(coordinate, None);
        let column = Arc::try_unwrap(column)
            .ok()
            .expect("fixture owns the prepared column");
        chunks.set_prepared_column_for_test(
            coordinate,
            Some(Arc::new(
                column.with_settlement_for_test(settlement.clone()),
            )),
        );
        let realm = journal
            .gameplay_mut()
            .kingdoms
            .found(founder, center, "Captured woodland")
            .unwrap();
        journal.gameplay_mut().kingdoms.join(actor, realm).unwrap();
        journal
            .gameplay_mut()
            .kingdoms
            .set_role(founder, realm, actor, KingdomRole::Member)
            .unwrap();
        let plan = Plan::prepare(&chunks, &journal, shape.origin).unwrap();
        assert_eq!(plan.domains, vec![settlement.domain()]);
        assert!(plan.allowed_for(actor, &chunks, &journal));
        let leaves = plan.leaves.clone();
        assert!(!leaves.is_empty());
        assert!(leaves.iter().all(|p| {
            journal
                .gameplay()
                .kingdoms
                .claim_at(terra_core::kingdom::claim_chunk(*p))
                .is_none()
        }));
        let mut queue = Queue::default();
        assert!(queue.enqueue(plan, actor));
        let mut now = 0.0;
        while !queue.0.front().unwrap().wood.is_empty() {
            queue
                .advance(&mut chunks, &mut journal, &Default::default(), now)
                .unwrap();
            now += 0.1;
        }
        assert!(leaves.iter().all(|p| journal.block(*p).is_none()));
        // Evict the real prepared columns and every canopy section before the
        // delayed leaf commit, as a moving player's streaming window does.
        let columns: std::collections::HashSet<_> = all
            .iter()
            .map(|p| [p[0].div_euclid(16), p[2].div_euclid(16)])
            .collect();
        for column in columns {
            chunks.set_prepared_column_for_test(column, None);
        }
        chunks.loaded.clear();
        assert_eq!(chunks.village_settlements().count(), 0);
        match change {
            "kick" => journal
                .gameplay_mut()
                .kingdoms
                .kick(founder, realm, actor)
                .unwrap(),
            "capture" => {
                journal
                    .gameplay_mut()
                    .kingdoms
                    .dissolve(founder, realm)
                    .unwrap();
                journal
                    .gameplay_mut()
                    .kingdoms
                    .found(PlayerUuid::from_u128(3), center, "New captor")
                    .unwrap();
            }
            _ => {}
        }
        assert!(
            crate::kingdom_access::can_use(
                &journal.gameplay().kingdoms,
                &chunks,
                actor,
                KingdomPermission::Break,
                leaves[0]
            ),
            "evicted live metadata alone looks like wilderness"
        );
        queue
            .finish(&mut chunks, &mut journal, &Default::default())
            .unwrap();
        if change == "authorized" {
            assert!(
                leaves.iter().all(|p| journal.block(*p) == Some(0)),
                "authorized unloaded canopy must still finish completely"
            );
        } else {
            assert!(
                leaves.iter().all(|p| journal.block(*p).is_none()),
                "revoked or captured domain must deny every deferred write"
            );
        }
    }
}

#[test]
fn shared_natural_green_leaves_return_the_trunks_species_sapling_in_real_felling_drops() {
    for (wood_material, expected) in [
        (6, terra_core::TreeSpecies::Oak),
        (36, terra_core::TreeSpecies::Spruce),
    ] {
        let root = [0, 16, 0];
        let crown = [1, 18, 0];
        let section = global_to_section(root).0;
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block(global_to_section(root).1, wood_material)
            .unwrap();
        data.set_block(global_to_section(crown).1, 7).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut journal = EditJournal::default();
        let plan = Plan {
            planted: None,
            species: species(wood_material, 7),
            origin: root,
            foot: root,
            material: wood_material,
            leaves_material: 7,
            wood: vec![root],
            leaves: vec![crown],
            revisions: vec![(section, 0)],
            domains: vec![],
        };
        let mut queue = Queue::default();
        assert!(queue.enqueue(plan, PlayerUuid::from_u128(1)));
        queue
            .finish(&mut chunks, &mut journal, &Default::default())
            .unwrap();
        assert_eq!(
            journal
                .gameplay()
                .drops
                .iter()
                .filter(|d| d.stack.item == ItemKind::Prop(expected.sapling()))
                .map(|d| d.stack.count)
                .sum::<u16>(),
            1
        );
        let other = if expected == terra_core::TreeSpecies::Oak {
            terra_core::TreeSpecies::Spruce
        } else {
            terra_core::TreeSpecies::Oak
        };
        assert!(
            !journal
                .gameplay()
                .drops
                .iter()
                .any(|d| d.stack.item == ItemKind::Prop(other.sapling()))
        );
    }
}
