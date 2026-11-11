//! Cargo-funded ballast columns have a finite depth and must reach actual support.
use super::*;
use terra_core::block_shapes::ShapeBox;

/// Maximum number of new ballast blocks vertically beneath a sleeper.
pub const MAX_FOUNDATION_DEPTH: i64 = 32;
const MAX_FOUNDATION_BLOCKS: usize = 2048;

pub(super) struct FoundationPlan {
    pub edits: Vec<BlockEditRequest>,
    pub drops: Vec<([f64; 3], ItemStack)>,
    pub read_cells: BTreeSet<Cell>,
}

pub(super) fn plan(
    chunks: &ChunkStore,
    journal: &EditJournal,
    columns: &BTreeSet<[i64; 2]>,
    floor_y: i64,
) -> Result<FoundationPlan, String> {
    let mut plan = FoundationPlan {
        edits: Vec::new(),
        drops: Vec::new(),
        read_cells: BTreeSet::new(),
    };
    for &[x, z] in columns {
        for depth in 0..=MAX_FOUNDATION_DEPTH {
            let y = floor_y
                .checked_sub(depth)
                .ok_or("Ungültige Fundamenthöhe")?;
            let cell = [x, y, z];
            let old = chunks
                .block_at(cell)
                .ok_or("Fundamentbereich noch nicht geladen")?;
            plan.read_cells.insert(cell);
            if journal.object_state(cell).is_some()
                || journal.fluid(cell).is_some_and(|fluid| fluid.amount > 0)
                || journal.container_at(cell).is_some()
                || journal.furnace_at(cell).is_some()
                || journal.plumbing_at(cell).is_some()
                || journal.has_attached_campfire(cell)
            {
                return Err("Gebautes Objekt oder Wasser unter dem Gleisbett".into());
            }
            if old != 0 && !earth(old) {
                return Err("Geschützter Boden unter dem Gleisbett".into());
            }
            if old != 0 && (depth > 0 || old == 206) {
                break;
            }
            if depth == MAX_FOUNDATION_DEPTH {
                return Err(format!(
                    "Kein tragfähiger Boden innerhalb von {MAX_FOUNDATION_DEPTH} Blöcken unter dem Gleisbett"
                ));
            }
            if plan.edits.len() == MAX_FOUNDATION_BLOCKS {
                return Err("Gleisfundament überschreitet sein Blockbudget".into());
            }
            if old != 0 {
                for stack in clearing::block_drops(journal, cell, old) {
                    plan.drops
                        .push(([x as f64 + 0.5, y as f64 + 1.5, z as f64 + 0.5], stack));
                }
            }
            plan.edits.push(BlockEditRequest {
                position: cell,
                expected_material: old,
                material: 206,
            });
            if old != 0 {
                break;
            }
        }
    }
    reject_occupied_space(chunks, journal, &plan.edits)?;
    Ok(plan)
}

fn reject_occupied_space(
    chunks: &ChunkStore,
    journal: &EditJournal,
    edits: &[BlockEditRequest],
) -> Result<(), String> {
    let shapes: Vec<_> = edits
        .iter()
        .filter(|edit| edit.expected_material == 0)
        .map(|edit| ShapeBox {
            minimum: edit.position.map(|value| value as f64 + 1e-6),
            maximum: edit.position.map(|value| value as f64 + 1.0 - 1e-6),
            hinged: false,
        })
        .collect();
    if shapes.is_empty() {
        return Ok(());
    }
    let intersects = |bounds: [[f64; 3]; 2]| {
        shapes
            .iter()
            .any(|shape| shape.intersects(bounds[0], bounds[1]))
    };
    for car in journal.gameplay().trains.vehicles.values() {
        if intersects(car.model_bounds())
            && (0..car.parts.len()).any(|index| {
                car.transformed_shapes(index)
                    .iter()
                    .any(|shape| intersects([shape.minimum, shape.maximum]))
            })
        {
            return Err("Ein Fahrzeug steht im geplanten Gleisfundament".into());
        }
    }
    let mut owners = BTreeSet::new();
    for edit in edits.iter().filter(|edit| edit.expected_material == 0) {
        let owner = global_to_section(edit.position).0;
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    owners.insert(owner.offset([x, y, z]).map_err(|error| error.to_string())?);
                }
            }
        }
    }
    for owner in owners {
        for prop in chunks
            .loaded
            .get(&owner)
            .into_iter()
            .flat_map(|section| section.props.iter())
            .chain(journal.placed_props(owner))
        {
            if !prop.removed
                && !journal.prop_removed(prop.section, prop.id)
                && intersects(prop.placement_bounds())
            {
                return Err("Vegetation oder ein Objekt belegt das Gleisfundament".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::BlockSection;

    fn set(chunks: &mut ChunkStore, cell: Cell, material: u16) {
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
    }

    #[test]
    fn deep_foundations_count_every_block_across_negative_horizontal_and_vertical_sections() {
        let mut chunks = ChunkStore::default();
        let journal = EditJournal::default();
        let columns = BTreeSet::from([[-17, -1], [-16, 0]]);
        for &[x, z] in &columns {
            for y in -16..=16 {
                set(&mut chunks, [x, y, z], if y == -2 { 3 } else { 0 });
            }
        }
        let prepared = plan(&chunks, &journal, &columns, 16).unwrap();
        assert_eq!(prepared.edits.len(), 36);
        assert!(prepared.drops.is_empty());
        for &[x, z] in &columns {
            for y in -1..=16 {
                assert!(prepared.edits.iter().any(|edit| {
                    edit.position == [x, y, z]
                        && edit.expected_material == 0
                        && edit.material == 206
                }));
            }
            assert!(prepared.read_cells.contains(&[x, -2, z]));
            assert_eq!(chunks.block_at([x, 16, z]), Some(0));
        }
    }

    #[test]
    fn maximum_depth_is_inclusive_of_the_support_and_never_publishes_a_floating_column() {
        let mut chunks = ChunkStore::default();
        let journal = EditJournal::default();
        let columns = BTreeSet::from([[0, 0]]);
        for y in -MAX_FOUNDATION_DEPTH..=0 {
            set(&mut chunks, [0, y, 0], 0);
        }
        assert!(plan(&chunks, &journal, &columns, 0).is_err());
        set(&mut chunks, [0, -MAX_FOUNDATION_DEPTH, 0], 3);
        assert_eq!(
            plan(&chunks, &journal, &columns, 0).unwrap().edits.len(),
            MAX_FOUNDATION_DEPTH as usize
        );
        assert_eq!(chunks.block_at([0, 0, 0]), Some(0));
    }

    #[test]
    fn foundations_reject_unloaded_protected_and_metadata_bearing_support_without_edits() {
        let columns = BTreeSet::from([[0, 0]]);
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        set(&mut chunks, [0, 0, 0], 0);
        assert!(plan(&chunks, &journal, &columns, 0).is_err());
        for material in [14, 29, 41] {
            set(&mut chunks, [0, -3, 0], material);
            assert!(plan(&chunks, &journal, &columns, 0).is_err());
        }
        set(&mut chunks, [0, -3, 0], 5);
        journal.record([0, -3, 0], 5).unwrap();
        journal
            .set_object_state(
                [0, -3, 0],
                Some(BlockObjectState {
                    facing: 8,
                    ..Default::default()
                }),
            )
            .unwrap();
        assert!(plan(&chunks, &journal, &columns, 0).is_err());
        assert_eq!(chunks.block_at([0, 0, 0]), Some(0));
    }

    #[test]
    fn water_in_the_fill_volume_blocks_foundation_planning() {
        let columns = BTreeSet::from([[0, 0]]);
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for y in -3..=0 {
            set(&mut chunks, [0, y, 0], if y == -3 { 3 } else { 0 });
        }
        journal
            .set_fluid(
                [0, -1, 0],
                crate::water::FluidCell {
                    amount: 8,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(plan(&chunks, &journal, &columns, 0).is_err());
        assert_eq!(chunks.block_at([0, 0, 0]), Some(0));
        assert_eq!(journal.fluid([0, -1, 0]).unwrap().amount, 8);
    }

    #[test]
    fn foundation_block_budget_is_enforced_before_any_world_change() {
        let columns: BTreeSet<_> = (0..=64).map(|x| [x, 0]).collect();
        let mut chunks = ChunkStore::default();
        let journal = EditJournal::default();
        for &[x, z] in &columns {
            for y in -MAX_FOUNDATION_DEPTH..=0 {
                set(
                    &mut chunks,
                    [x, y, z],
                    if y == -MAX_FOUNDATION_DEPTH { 3 } else { 0 },
                );
            }
        }
        let error = plan(&chunks, &journal, &columns, 0)
            .err()
            .expect("65 full columns exceed the bounded plan budget");
        assert!(error.contains("Blockbudget"));
        assert_eq!(chunks.block_at([0, 0, 0]), Some(0));
    }
}
