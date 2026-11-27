use super::*;
use crate::streaming::{BlockEditRequest, LoadedSection, commit_block_edit};
use terra_voxel::{BlockSection, SectionPos};

mod leaf_drops;

#[test]
fn idle_growth_does_not_invalidate_chunk_or_journal_resources_between_checks() {
    #[derive(Resource, Default)]
    struct Mutations(Vec<(bool, bool)>);

    fn observe(
        chunks: Res<ChunkStore>,
        journal: Res<EditJournal>,
        mut observed: ResMut<Mutations>,
    ) {
        observed.0.push((chunks.is_changed(), journal.is_changed()));
    }

    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<WorldAccess>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<Mutations>()
        .insert_resource(WorldTerrain(
            crate::TerraGenerator::new(terra_core::WorldConfig::default()).unwrap(),
        ))
        .add_systems(Update, (tick, observe).chain());
    // Resource insertion is a real change; the following frames must be idle.
    app.update();
    for _ in 0..12 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(500));
        app.update();
    }
    let observed = &app.world().resource::<Mutations>().0;
    assert_eq!(observed[0], (true, true));
    assert!(
        observed[1..]
            .iter()
            .all(|changes| *changes == (false, false)),
        "Idle growth spuriously restarted renderer selection: {observed:?}"
    );
}

fn fixture(species: TreeSpecies, id: u64) -> (ChunkStore, EditJournal, WorldProp) {
    let support = [15, 15, 15];
    let mut chunks = ChunkStore::default();
    for x in -1..=2 {
        for z in -1..=2 {
            for y in 0..=4 {
                let s = SectionPos::new(x, y, z);
                chunks.loaded.insert(
                    s,
                    LoadedSection {
                        data: BlockSection::empty(s).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: vec![],
                    },
                );
            }
        }
    }
    let (s, l) = global_to_section(support);
    chunks
        .loaded
        .get_mut(&s)
        .unwrap()
        .data
        .set_block(l, 2)
        .unwrap();
    let prop = WorldProp {
        id: PropId(id),
        kind: species.sapling().into(),
        position: [15.5, 16., 15.5],
        rotation: 0.,
        scale: 1.,
        variant: 0,
        section: s,
        removed: false,
    };
    (chunks, EditJournal::default(), prop)
}
fn ready(journal: &mut EditJournal, id: u64) {
    let t = journal.gameplay_mut().trees.get_mut(id).unwrap();
    t.growth_seconds = t.species.growth_seconds();
}
#[test]
fn all_species_plant_and_mature_into_exact_persistent_trees_across_sections() {
    for species in TreeSpecies::ALL {
        let (mut chunks, mut journal, prop) = fixture(species, 101);
        assert!(plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap());
        assert!(!plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap());
        assert!(!grow(&mut chunks, &mut journal, 101).unwrap());
        ready(&mut journal, 101);
        assert!(grow(&mut chunks, &mut journal, 101).unwrap());
        let tree = journal.gameplay().trees.get(101).unwrap();
        assert_eq!(tree.stage, TreeStage::Generated);
        assert_eq!(tree.species, species);
        assert!(tree.valid());
        assert!(tree.wood.len() > 10 && tree.leaves.len() > 40);
        assert!(tree.wood.len() + tree.leaves.len() < terra_core::trees::MAX_TREE_CELLS);
        for (&p, m) in tree
            .wood
            .iter()
            .map(|p| (p, species.wood()))
            .chain(tree.leaves.iter().map(|p| (p, species.leaves())))
        {
            assert_eq!(chunks.block_at(tree.global(p)), Some(m));
            assert_eq!(journal.block(tree.global(p)), Some(m));
        }
        let plan = crate::tree_felling::Plan::prepare(&chunks, &journal, tree.root).unwrap();
        assert!(
            plan.collapse([1., 0.]).valid(),
            "{species:?} collapse keeps its leaf material, including Apple195"
        );
        assert!(journal.prop_removed(prop.section, prop.id));
        assert!(journal.gameplay().valid());
    }
}
#[test]
fn soil_and_local_space_are_checked_while_growth_keeps_buildings_intact() {
    let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Oak, 201);
    let support = placement::support(&prop);
    let (s, l) = global_to_section(support);
    for m in [0, 3, 4, 27] {
        chunks
            .loaded
            .get_mut(&s)
            .unwrap()
            .data
            .set_block(l, m)
            .unwrap();
        assert!(!planting_environment_valid(
            &chunks,
            &journal,
            support,
            TreeSpecies::Oak
        ));
    }
    chunks
        .loaded
        .get_mut(&s)
        .unwrap()
        .data
        .set_block(l, 1)
        .unwrap();
    assert!(planting_environment_valid(
        &chunks,
        &journal,
        support,
        TreeSpecies::Oak
    ));
    assert!(!light(&chunks, [15, 16, 15], false));
    assert!(plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap());
    let adjacent = [18, 15, 15];
    let (s, l) = global_to_section(adjacent);
    chunks
        .loaded
        .get_mut(&s)
        .unwrap()
        .data
        .set_block(l, 2)
        .unwrap();
    assert!(
        planting_environment_valid(&chunks, &journal, adjacent, TreeSpecies::Oak),
        "nearby trees do not impose a planting distance"
    );
    ready(&mut journal, 201);
    let root = [15, 16, 15];
    let obstacle = shape(TreeSpecies::Oak, 201)
        .0
        .into_iter()
        .find(|p| p[1] >= 3)
        .map(|p| std::array::from_fn(|a| root[a] + i64::from(p[a])))
        .unwrap();
    let (s, l) = global_to_section(obstacle);
    chunks
        .loaded
        .get_mut(&s)
        .unwrap()
        .data
        .set_block(l, 29)
        .unwrap();
    journal.record(obstacle, 29).unwrap();
    let before = journal.edited_blocks();
    assert!(!grow(&mut chunks, &mut journal, 201).unwrap());
    assert_eq!(journal.edited_blocks(), before);
    assert_eq!(chunks.block_at(obstacle), Some(29));
    assert_eq!(chunks.block_at(root), Some(0));
    chunks
        .loaded
        .get_mut(&s)
        .unwrap()
        .data
        .set_block(l, 0)
        .unwrap();
    journal.record(obstacle, 0).unwrap();
    assert!(grow(&mut chunks, &mut journal, 201).unwrap());
}
#[test]
fn incomplete_streaming_defers_growth_then_resumes_without_partial_canopy() {
    let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Apple, 301);
    assert!(plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap());
    ready(&mut journal, 301);
    let missing = SectionPos::new(1, 1, 1);
    let saved = chunks.loaded.remove(&missing).unwrap();
    assert!(!grow(&mut chunks, &mut journal, 301).unwrap());
    assert_eq!(journal.edited_blocks(), 0);
    assert_eq!(
        journal.gameplay().trees.get(301).unwrap().stage,
        TreeStage::Sapling
    );
    chunks.loaded.insert(missing, saved);
    assert!(grow(&mut chunks, &mut journal, 301).unwrap());
    assert!(journal.gameplay().trees.get(301).unwrap().fruit.is_some());
}
#[test]
fn removal_retires_sapling_and_replaced_player_wood_survives_planted_tree_felling() {
    let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Birch, 401);
    let actor = PlayerUuid::from_u128(1);
    assert!(plant(&mut chunks, &mut journal, prop, actor).unwrap());
    ready(&mut journal, 401);
    assert!(grow(&mut chunks, &mut journal, 401).unwrap());
    let tree = journal.gameplay().trees.get(401).unwrap();
    let root = tree.root;
    let expected_saplings = tree
        .leaves
        .iter()
        .filter(|cell| leaf_drop_roll(tree.global(**cell)))
        .count() as u16;
    assert!(crate::tree_felling::eligible(&chunks, &journal, root));
    let plan = crate::tree_felling::Plan::prepare(&chunks, &journal, root).unwrap();
    let protected = plan.wood_positions().find(|p| *p != root).unwrap();
    for (from, to) in [(10, 0), (0, 10)] {
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position: protected,
                    expected_material: from,
                    material: to
                },
                &Default::default()
            )
            .unwrap()
        );
    }
    assert!(
        !journal
            .gameplay()
            .trees
            .get(401)
            .unwrap()
            .owns(protected, 10)
    );
    let mut queue = crate::tree_felling::Queue::default();
    assert!(queue.enqueue(plan, actor));
    queue
        .finish(&mut chunks, &mut journal, &Default::default())
        .unwrap();
    assert_eq!(chunks.block_at(protected), Some(10));
    assert_eq!(journal.block(root), Some(0));
    assert_eq!(
        journal.gameplay().trees.get(401).unwrap().stage,
        TreeStage::GeneratedFelled
    );
    assert_eq!(
        journal
            .gameplay()
            .drops
            .iter()
            .filter(|d| d.stack.item == terra_core::ItemKind::Prop(TreeSpecies::Birch.sapling()))
            .map(|d| d.stack.count)
            .sum::<u16>(),
        expected_saplings
    );
    let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Cherry, 402);
    assert!(plant(&mut chunks, &mut journal, prop, actor).unwrap());
    assert!(
        crate::streaming::commit_prop_removal(
            &mut chunks,
            &mut journal,
            &crate::streaming::PropRemoveRequest {
                section: prop.section,
                id: prop.id
            }
        )
        .unwrap()
    );
    assert_eq!(
        journal.gameplay().trees.get(402).unwrap().stage,
        TreeStage::Felled
    );
    assert!(!grow(&mut chunks, &mut journal, 402).unwrap());
}
#[test]
fn sapling_meshes_are_distinct_finite_minature_trees_without_dirt_plinth() {
    use crate::props::visual::{PropDetail, PropPalette, template};
    let mut fingerprints = BTreeSet::new();
    for species in TreeSpecies::ALL {
        for detail in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
            let m = template(species.sapling().into(), 0, detail, &PropPalette::default()).unwrap();
            assert!(
                m.iter()
                    .flat_map(|m| &m.positions)
                    .all(|p| p.iter().all(|v| v.is_finite()) && p[1] >= -0.001 && p[1] < 1.2)
            );
            let hash = m
                .iter()
                .flat_map(|m| {
                    m.positions
                        .iter()
                        .flatten()
                        .chain(m.colors.iter().flatten())
                })
                .fold(0_u64, |h, v| h.wrapping_mul(31) ^ u64::from(v.to_bits()));
            if detail == PropDetail::Near {
                assert!(fingerprints.insert(hash));
            }
        }
    }
}

#[test]
fn growth_rechecks_kicked_creator_and_every_canopy_claim_before_writing_any_cell() {
    use terra_core::kingdom::KingdomRole;
    let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Oak, 701);
    let creator = PlayerUuid::from_u128(1);
    let founder = PlayerUuid::from_u128(2);
    let neighbour = PlayerUuid::from_u128(3);
    let realm = journal
        .gameplay_mut()
        .kingdoms
        .found(founder, [0, 20, 0], "Orchard")
        .unwrap();
    journal
        .gameplay_mut()
        .kingdoms
        .join(creator, realm)
        .unwrap();
    journal
        .gameplay_mut()
        .kingdoms
        .set_role(founder, realm, creator, KingdomRole::Member)
        .unwrap();
    assert!(plant(&mut chunks, &mut journal, prop, creator).unwrap());
    ready(&mut journal, 701);
    journal
        .gameplay_mut()
        .kingdoms
        .kick(founder, realm, creator)
        .unwrap();
    assert!(!grow(&mut chunks, &mut journal, 701).unwrap());
    assert_eq!(journal.edited_blocks(), 0);
    journal
        .gameplay_mut()
        .kingdoms
        .join(creator, realm)
        .unwrap();
    assert!(
        !grow(&mut chunks, &mut journal, 701).unwrap(),
        "fresh Visitor remains denied"
    );
    journal
        .gameplay_mut()
        .kingdoms
        .set_role(founder, realm, creator, KingdomRole::Member)
        .unwrap();
    let other = journal
        .gameplay_mut()
        .kingdoms
        .found(neighbour, [16, 20, 16], "Neighbour")
        .unwrap();
    assert!(
        !grow(&mut chunks, &mut journal, 701).unwrap(),
        "far side of the canopy crosses another protected chunk"
    );
    assert_eq!(journal.edited_blocks(), 0);
    assert_eq!(
        journal.gameplay().trees.get(701).unwrap().stage,
        TreeStage::Sapling
    );
    journal
        .gameplay_mut()
        .kingdoms
        .dissolve(neighbour, other)
        .unwrap();
    assert!(grow(&mut chunks, &mut journal, 701).unwrap());
}

#[test]
fn growth_reserves_both_registry_and_tree_revisions_before_mutating_blocks_or_prop() {
    for (registry_revision, tree_revision) in [(u64::MAX - 1, 0), (1, u64::MAX - 1)] {
        let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Oak, 801);
        assert!(plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap());
        ready(&mut journal, 801);
        journal.gameplay_mut().trees.revision = registry_revision;
        journal.gameplay_mut().trees.get_mut(801).unwrap().revision = tree_revision;
        assert!(!grow(&mut chunks, &mut journal, 801).unwrap());
        assert_eq!(journal.edited_blocks(), 0);
        assert!(!journal.prop_removed(prop.section, prop.id));
        assert_eq!(journal.gameplay().trees.revision, registry_revision);
        assert_eq!(
            journal.gameplay().trees.get(801).unwrap().stage,
            TreeStage::Sapling
        );
    }
}

#[test]
fn planted_apple_root_remove_and_equal_wood_replacement_cannot_keep_fruit_on_surviving_branches() {
    let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Apple, 901);
    assert!(plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap());
    ready(&mut journal, 901);
    assert!(grow(&mut chunks, &mut journal, 901).unwrap());
    {
        let tree = journal.gameplay_mut().trees.get_mut(901).unwrap();
        tree.fruit = Some(terra_core::FruitState {
            phase: terra_core::FruitPhase::Ripe,
            remaining: 8,
            picked: 0,
            cycle_day: 0,
            last_harvest_day: None,
            revision: 0,
        });
    }
    let root = journal.gameplay().trees.get(901).unwrap().root;
    assert!(
        crate::fruit::harvest_state(journal.gameplay().trees.get(901).unwrap(), 0, 0, 1).is_ok()
    );
    for (from, to) in [(6, 0), (0, 6)] {
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position: root,
                    expected_material: from,
                    material: to
                },
                &Default::default()
            )
            .unwrap()
        );
    }
    let tree = journal.gameplay().trees.get(901).unwrap();
    assert_eq!(tree.stage, TreeStage::GeneratedFelled);
    assert!(tree.fruit.is_none());
    assert!(!tree.owns(root, 6));
    assert!(
        !tree.wood.is_empty(),
        "real surviving branches retain exact attribution"
    );
    assert!(tree.owns(tree.global(tree.wood[0]), 6));
    assert!(crate::fruit::harvest_state(tree, 0, 0, 1).is_err());
    assert!(tree.valid());
}

#[test]
fn every_grown_species_has_one_face_connected_root_trunk_and_branch_skeleton() {
    for species in TreeSpecies::ALL {
        for seed in [0, 1, 17, 901] {
            let (wood, _) = shape(species, seed);
            let cells: std::collections::HashSet<_> = wood.iter().copied().collect();
            let mut reached = std::collections::HashSet::from([[0_i8; 3]]);
            let mut todo = vec![[0_i8; 3]];
            while let Some(p) = todo.pop() {
                for delta in [
                    [-1, 0, 0],
                    [1, 0, 0],
                    [0, -1, 0],
                    [0, 1, 0],
                    [0, 0, -1],
                    [0, 0, 1],
                ] {
                    let next = std::array::from_fn(|a| p[a] + delta[a]);
                    if cells.contains(&next) && reached.insert(next) {
                        todo.push(next);
                    }
                }
            }
            assert_eq!(
                reached.len(),
                wood.len(),
                "{species:?} seed {seed}: every branch must join the rooted skeleton"
            );
        }
    }
}

#[test]
fn canonical_growth_stages_only_one_cell_budget_and_never_publishes_partial_work() {
    let (mut c, mut j, p) = fixture(TreeSpecies::Spruce, 1201);
    assert!(plant(&mut c, &mut j, p, PlayerUuid::from_u128(1)).unwrap());
    ready(&mut j, 1201);
    let (w, l) = shape(TreeSpecies::Spruce, 1201);
    let mut batch =
        crate::edits::TreeGrowthBatch::begin(&j, &c, 1201, w, l, BTreeSet::new()).unwrap();
    assert!(batch.cells.len() > CELL_BUDGET);
    assert!(!batch.advance(&j, &c, CELL_BUDGET).unwrap());
    assert!(batch.cursor > 0 && batch.cursor <= CELL_BUDGET);
    assert_eq!(j.edited_blocks(), 0);
    assert_eq!(c.block_at([15, 16, 15]), Some(0));
    assert!(!j.prop_removed(p.section, p.id));
    while !batch.advance(&j, &c, CELL_BUDGET).unwrap() {}
    assert_eq!(j.edited_blocks(), 0);
    assert!(batch.publish(&mut j, &mut c).unwrap());
    assert_eq!(
        j.gameplay().trees.get(1201).unwrap().stage,
        TreeStage::Generated
    );
    assert!(j.prop_removed(p.section, p.id));
}
#[test]
fn a_real_player_block_edit_mid_staging_cancels_the_entire_unpublished_tree() {
    let (mut c, mut j, p) = fixture(TreeSpecies::Oak, 1202);
    assert!(plant(&mut c, &mut j, p, PlayerUuid::from_u128(1)).unwrap());
    ready(&mut j, 1202);
    let (w, l) = shape(TreeSpecies::Oak, 1202);
    let mut b = crate::edits::TreeGrowthBatch::begin(&j, &c, 1202, w, l, BTreeSet::new()).unwrap();
    b.advance(&j, &c, 1).unwrap();
    let target = b
        .cells
        .iter()
        .map(|(p, _)| *p)
        .find(|p| *p != [15, 16, 15])
        .unwrap();
    assert!(
        commit_block_edit(
            &mut c,
            &mut j,
            &BlockEditRequest {
                position: target,
                expected_material: 0,
                material: 3
            },
            &Default::default()
        )
        .unwrap()
    );
    assert!(b.advance(&j, &c, CELL_BUDGET).is_err());
    assert!(!b.publish(&mut j, &mut c).unwrap());
    assert_eq!(c.block_at(target), Some(3));
    assert_eq!(j.edited_blocks(), 1);
    assert_eq!(
        j.gameplay().trees.get(1202).unwrap().stage,
        TreeStage::Sapling
    );
}
#[test]
fn only_unsaved_natural_soft_cover_yields_to_growth_and_saved_player_props_are_preserved() {
    for saved in [false, true] {
        let (mut c, mut j, p) = fixture(TreeSpecies::Oak, 1203);
        assert!(plant(&mut c, &mut j, p, PlayerUuid::from_u128(1)).unwrap());
        ready(&mut j, 1203);
        let offset = shape(TreeSpecies::Oak, 1203)
            .0
            .into_iter()
            .find(|p| p[1] == 0 && (p[0] != 0 || p[2] != 0))
            .unwrap();
        let cell: [i64; 3] = std::array::from_fn(|a| [15, 16, 15][a] + i64::from(offset[a]));
        let support = [cell[0], cell[1] - 1, cell[2]];
        let owner = global_to_section(support).0;
        let grass = WorldProp {
            id: PropId(90003),
            kind: crate::props::PropKind::Grass,
            position: [cell[0] as f64 + 0.5, cell[1] as f64, cell[2] as f64 + 0.5],
            rotation: 0.,
            scale: 1.,
            variant: 0,
            section: owner,
            removed: false,
        };
        if saved {
            assert!(j.place_prop(grass).unwrap());
        }
        c.loaded.get_mut(&owner).unwrap().props.push(grass);
        assert_eq!(grow(&mut c, &mut j, 1203).unwrap(), !saved);
        assert_eq!(j.prop_removed(owner, grass.id), !saved);
        if saved {
            assert_eq!(j.edited_blocks(), 0);
            assert_eq!(
                j.gameplay().trees.get(1203).unwrap().stage,
                TreeStage::Sapling
            );
        }
    }
}
#[test]
fn fruit_only_metadata_keeps_cached_geometry_and_causes_no_mesh_invalidation() {
    let (mut c, mut j, p) = fixture(TreeSpecies::Apple, 1204);
    assert!(plant(&mut c, &mut j, p, PlayerUuid::from_u128(1)).unwrap());
    ready(&mut j, 1204);
    assert!(grow(&mut c, &mut j, 1204).unwrap());
    let section = global_to_section([15, 16, 15]).0;
    let first = j.planted_appearance_records(section);
    assert_eq!(first.len(), 1);
    let key = first[0].geometry_key();
    let epoch = j.mesh_epoch(section);
    j.gameplay_mut()
        .trees
        .get_mut(1204)
        .unwrap()
        .fruit
        .as_mut()
        .unwrap()
        .cycle_day += 1;
    let second = j.planted_appearance_records(section);
    assert!(std::sync::Arc::ptr_eq(&first[0], &second[0]));
    invalidate_tree_visuals(&mut c, &mut j, Some(key), Some(second[0].geometry_key())).unwrap();
    assert_eq!(j.mesh_epoch(section), epoch);
    let record = j.gameplay_mut().trees.get_mut(1204).unwrap();
    record.stage = TreeStage::GeneratedFelled;
    record.fruit = None;
    let partial = j.planted_appearance_records(section);
    assert_eq!(partial[0].stage, TreeStage::GeneratedFelled);
    invalidate_tree_visuals(&mut c, &mut j, Some(key), Some(partial[0].geometry_key())).unwrap();
    assert!(j.mesh_epoch(section) > epoch);
}

#[test]
fn natural_soft_prop_spanning_from_a_foreign_claim_cannot_be_removed_by_allowed_growth_cells() {
    let (mut c, mut j, p) = fixture(TreeSpecies::Oak, 1205);
    let actor = PlayerUuid::from_u128(1);
    assert!(plant(&mut c, &mut j, p, actor).unwrap());
    let grass = WorldProp {
        id: PropId(90005),
        kind: crate::props::PropKind::Grass,
        position: [16.01, 20., 1.5],
        rotation: 0.,
        scale: 1.,
        variant: 0,
        section: global_to_section([16, 19, 1]).0,
        removed: false,
    };
    c.loaded.get_mut(&grass.section).unwrap().props.push(grass);
    let cells = vec![([15, 20, 1], 6)];
    assert!(
        prop_space(&c, &j, 1205, &cells)
            .unwrap()
            .contains(&(grass.section, grass.id))
    );
    j.gameplay_mut()
        .kingdoms
        .found(PlayerUuid::from_u128(2), [16, 20, 1], "Neighbour")
        .unwrap();
    assert!(crate::kingdom_access::can_use(
        &j.gameplay().kingdoms,
        &c,
        actor,
        terra_core::kingdom::KingdomPermission::Build,
        [15, 20, 1]
    ));
    assert!(prop_space(&c, &j, 1205, &cells).is_none());
    assert!(!j.prop_removed(grass.section, grass.id));
}

#[test]
fn reloaded_different_section_payload_with_the_same_revision_aborts_staged_growth() {
    let (mut c, mut j, p) = fixture(TreeSpecies::Spruce, 1206);
    assert!(plant(&mut c, &mut j, p, PlayerUuid::from_u128(1)).unwrap());
    ready(&mut j, 1206);
    let (w, l) = shape(TreeSpecies::Spruce, 1206);
    let mut batch =
        crate::edits::TreeGrowthBatch::begin(&j, &c, 1206, w, l, BTreeSet::new()).unwrap();
    batch.advance(&j, &c, CELL_BUDGET).unwrap();
    let target = batch.cells[0].0;
    let (section, local) = global_to_section(target);
    let original = &c.loaded[&section].data;
    let revision = original.revision();
    let mut blocks = original.blocks().to_vec();
    blocks[local[0] + 16 * (local[1] + 16 * local[2])] = 3;
    c.loaded.get_mut(&section).unwrap().data =
        BlockSection::from_blocks(section, revision, blocks).unwrap();
    assert!(batch.advance(&j, &c, CELL_BUDGET).is_err());
    assert!(!batch.publish(&mut j, &mut c).unwrap());
    assert_eq!(c.block_at(target), Some(3));
    assert_eq!(j.edited_blocks(), 0);
    assert!(!j.prop_removed(p.section, p.id));
}

#[test]
fn wide_generated_trunk_fells_from_exposed_lower_cells_but_not_branches_or_player_replacements() {
    let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Oak, 1207);
    assert!(plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap());
    ready(&mut journal, 1207);
    assert!(grow(&mut chunks, &mut journal, 1207).unwrap());
    let tree = journal.gameplay().trees.get(1207).unwrap();
    let root = tree.root;
    let wood = tree.species.wood();
    let count = tree.wood.len();
    let foot = tree
        .wood
        .iter()
        .filter(|p| (0..=2).contains(&p[1]) && (p[0] != 0 || p[2] != 0))
        .map(|p| tree.global(*p))
        .find(|p| {
            [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]]
                .into_iter()
                .any(|d| chunks.block_at(std::array::from_fn(|a| p[a] + d[a])) == Some(0))
        })
        .expect("a real broad oak exposes a lower side of its trunk");
    assert_ne!(foot, root);
    assert!(crate::tree_felling::eligible(&chunks, &journal, foot));
    crate::tree_felling::request_missing(&mut chunks, &journal, foot);
    let plan = crate::tree_felling::Plan::prepare(&chunks, &journal, foot).unwrap();
    assert_eq!(plan.foot(), foot);
    assert_eq!(plan.wood_count(), count);
    assert!(plan.wood_positions().any(|p| p == root));
    let upper = journal
        .gameplay()
        .trees
        .get(1207)
        .unwrap()
        .wood
        .iter()
        .find(|p| p[1] >= 6)
        .map(|p| std::array::from_fn(|a| root[a] + i64::from(p[a])))
        .unwrap();
    assert!(!crate::tree_felling::eligible(&chunks, &journal, upper));
    assert!(crate::tree_felling::Plan::prepare(&chunks, &journal, upper).is_none());
    for (from, to) in [(wood, 0), (0, wood)] {
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position: foot,
                    expected_material: from,
                    material: to,
                },
                &Default::default(),
            )
            .unwrap()
        );
    }
    assert!(!crate::tree_felling::eligible(&chunks, &journal, foot));
    assert!(crate::tree_felling::Plan::prepare(&chunks, &journal, foot).is_none());
    assert_eq!(chunks.block_at(foot), Some(wood));
    assert_eq!(
        crate::tree_felling::Plan::prepare(&chunks, &journal, root)
            .unwrap()
            .wood_count(),
        count - 1
    );
    // Existing saved compact trees still trigger only from their exact root.
    journal.gameplay_mut().trees.get_mut(1207).unwrap().stage = TreeStage::Mature;
    assert!(crate::tree_felling::eligible(&chunks, &journal, root));
    let other = journal
        .gameplay()
        .trees
        .get(1207)
        .unwrap()
        .wood
        .iter()
        .find(|p| (0..=2).contains(&p[1]) && (p[0] != 0 || p[2] != 0))
        .map(|p| std::array::from_fn(|a| root[a] + i64::from(p[a])))
        .unwrap();
    assert!(!crate::tree_felling::eligible(&chunks, &journal, other));
}

#[test]
fn planting_needs_only_the_miniature_footprint_not_daylight_full_crowns_or_distant_streaming() {
    let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Oak, 1301);
    let roof = [15, 19, 15];
    let trunk = [18, 16, 15];
    for (p, material) in [(roof, 3), (trunk, 6), ([17, 15, 15], 2)] {
        let (section, local) = global_to_section(p);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, material)
            .unwrap();
    }
    chunks
        .loaded
        .retain(|p, _| p.y <= 1 && (0..=1).contains(&p.x) && p.z == 0);
    assert!(!light(&chunks, [15, 16, 15], true));
    assert!(plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap());
    let nearby = WorldProp {
        id: PropId(1302),
        position: [17.5, 16., 15.5],
        section: global_to_section([17, 15, 15]).0,
        ..prop
    };
    assert!(plant(&mut chunks, &mut journal, nearby, PlayerUuid::from_u128(1)).unwrap());
    ready(&mut journal, prop.id.0);
    assert!(!grow(&mut chunks, &mut journal, prop.id.0).unwrap());
    assert_eq!(chunks.block_at(roof), Some(3));
    assert_eq!(chunks.block_at(trunk), Some(6));
    assert_eq!(journal.edited_blocks(), 0);
}

#[test]
fn planting_replaces_only_authorized_natural_ground_cover_and_rolls_back_at_capacity() {
    use terra_core::trees::SaplingPlacementError as Error;
    for saved in [false, true] {
        let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Birch, 1303);
        let grass = WorldProp {
            id: PropId(93003),
            kind: crate::props::PropKind::Grass,
            ..prop
        };
        chunks
            .loaded
            .get_mut(&grass.section)
            .unwrap()
            .props
            .push(grass);
        if saved {
            assert!(journal.place_prop(grass).unwrap());
        }
        let status = placement_check(&chunks, &journal, prop, Some(PlayerUuid::from_u128(1)));
        if saved {
            assert_eq!(status, Err(Error::Occupied));
        } else {
            assert_eq!(status, Ok(()));
        }
        assert_eq!(
            plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap(),
            !saved
        );
        assert_eq!(journal.prop_removed(grass.section, grass.id), !saved);
        assert_eq!(
            chunks.loaded[&grass.section]
                .props
                .iter()
                .find(|p| p.id == grass.id)
                .unwrap()
                .removed,
            !saved
        );
    }
    let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Birch, 1304);
    let grass = WorldProp {
        id: PropId(93004),
        kind: crate::props::PropKind::Grass,
        ..prop
    };
    chunks
        .loaded
        .get_mut(&grass.section)
        .unwrap()
        .props
        .push(grass);
    journal
        .import_delta(crate::edits::SectionDelta {
            position: prop.section,
            prop_epoch: u64::MAX,
            ..Default::default()
        })
        .unwrap();
    let before = journal.section_delta(prop.section);
    assert_eq!(
        placement_check(&chunks, &journal, prop, Some(PlayerUuid::from_u128(1))),
        Err(Error::Limit)
    );
    assert!(!plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap());
    assert_eq!(journal.section_delta(prop.section), before);
    assert!(!chunks.loaded[&grass.section].props[0].removed);
    assert!(journal.gameplay().trees.trees.is_empty());
}

#[test]
fn planting_reports_local_soil_water_space_and_foreign_ground_cover_without_removing_it() {
    use terra_core::trees::SaplingPlacementError as Error;
    let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Apple, 1305);
    let actor = PlayerUuid::from_u128(1);
    let support = placement::support(&prop);
    for (material, expected) in [(3, Error::Soil), (0, Error::Soil)] {
        let (section, local) = global_to_section(support);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, material)
            .unwrap();
        assert_eq!(
            placement_check(&chunks, &journal, prop, Some(actor)),
            Err(expected)
        );
    }
    let (section, local) = global_to_section(support);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 2)
        .unwrap();
    let root = [15, 16, 15];
    let (section, local) = global_to_section(root);
    for (material, expected) in [(27, Error::Water), (3, Error::Occupied)] {
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, material)
            .unwrap();
        assert_eq!(
            placement_check(&chunks, &journal, prop, Some(actor)),
            Err(expected)
        );
    }
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 0)
        .unwrap();
    let unloaded = chunks.loaded.remove(&section).unwrap();
    assert_eq!(
        placement_check(&chunks, &journal, prop, Some(actor)),
        Err(Error::Unloaded)
    );
    chunks.loaded.insert(section, unloaded);
    let grass = WorldProp {
        id: PropId(93005),
        kind: crate::props::PropKind::Grass,
        position: [16.01, 16., 15.5],
        section: global_to_section([16, 15, 15]).0,
        ..prop
    };
    chunks
        .loaded
        .get_mut(&grass.section)
        .unwrap()
        .props
        .push(grass);
    journal
        .gameplay_mut()
        .kingdoms
        .found(PlayerUuid::from_u128(2), [16, 20, 15], "Neighbour")
        .unwrap();
    assert_eq!(
        placement_check(&chunks, &journal, prop, Some(actor)),
        Err(Error::Protected)
    );
    assert!(!plant(&mut chunks, &mut journal, prop, actor).unwrap());
    assert!(!journal.prop_removed(grass.section, grass.id));
}

#[test]
fn planted_birches_follow_saved_ecology_and_mature_cells_survive_reload() {
    let mut historical = None;
    for revision in 0..=7 {
        let (mut chunks, mut journal, prop) = fixture(TreeSpecies::Birch, 42042);
        assert!(plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap());
        ready(&mut journal, 42042);
        assert!(grow_with_ecology(&mut chunks, &mut journal, 42042, revision).unwrap());
        let record = journal.gameplay().trees.get(42042).unwrap().clone();
        assert!(record.valid());
        let cells = (record.wood.clone(), record.leaves.clone());
        assert_eq!(
            cells,
            shape_with_ecology(TreeSpecies::Birch, 42042, revision)
        );
        if revision == 0 {
            historical = Some(cells.clone());
        } else if revision < 7 {
            assert_eq!(Some(&cells), historical.as_ref());
        } else {
            assert_ne!(Some(&cells), historical.as_ref());
            let width = |min: i8, max: i8| {
                let xs: Vec<_> = record
                    .wood
                    .iter()
                    .chain(&record.leaves)
                    .filter(|p| (min..=max).contains(&p[1]))
                    .map(|p| p[0])
                    .collect();
                i16::from(*xs.iter().max().unwrap()) - i16::from(*xs.iter().min().unwrap()) + 1
            };
            assert!(width(0, 2) > width(4, 6));
            assert!(width(9, 30) >= width(4, 6) * 3);
            assert!(record.leaves.iter().all(|p| !(4..=6).contains(&p[1])));
        }
        let encoded = serde_json::to_string(&record).unwrap();
        let restored: TreeRecord = serde_json::from_str(&encoded).unwrap();
        assert_eq!(record, restored);
        // An already generated tree never re-runs geometry, even if a caller
        // requests a different revision after reload.
        assert!(!grow_with_ecology(&mut chunks, &mut journal, 42042, 7).unwrap());
        assert_eq!(journal.gameplay().trees.get(42042), Some(&restored));
        for (positions, material) in [(&restored.wood, 10), (&restored.leaves, 9)] {
            for p in positions {
                assert_eq!(journal.block(restored.global(*p)), Some(material));
                assert_eq!(chunks.block_at(restored.global(*p)), Some(material));
            }
        }
    }
    for species in [
        TreeSpecies::Oak,
        TreeSpecies::Spruce,
        TreeSpecies::Cherry,
        TreeSpecies::Apple,
    ] {
        assert_eq!(
            shape_with_ecology(species, 731, 6),
            shape_with_ecology(species, 731, 7)
        );
    }
}
