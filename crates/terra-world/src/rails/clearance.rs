use super::{Cell, cell_boxes};
use crate::{
    edits::EditJournal,
    placement::Layout,
    props::{PropId, PropKind, WorldProp},
    streaming::ChunkStore,
};
use std::collections::{BTreeMap, BTreeSet};
use terra_voxel::{SectionPos, global_to_section};

pub const MAX_CLEARED_PROPS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropSource {
    Generated,
    Saved,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropClearance {
    pub prop: WorldProp,
    pub source: PropSource,
    pub rights_cells: Vec<Cell>,
}

/// Destructible props touched by the authored rail solids, in stable owner/ID order.
/// This never edits a prop or grants permission to remove it.
pub fn clearance(
    plan: &Layout,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Result<Vec<PropClearance>, String> {
    if plan.edits.len() > 512 {
        return Err("Rail clearance exceeds its cell budget".into());
    }
    let mut sections = BTreeSet::new();
    let mut bounds = Vec::new();
    for edit in &plan.edits {
        if !terra_core::rails::is_rail(edit.material) {
            continue;
        }
        let state = plan
            .states
            .iter()
            .find(|(cell, _)| *cell == edit.position)
            .and_then(|(_, state)| *state)
            .ok_or("Missing rail clearance pose")?;
        for shape in cell_boxes(edit.position, edit.material, state).iter() {
            let mut shape = *shape;
            for axis in 0..3 {
                shape.minimum[axis] += edit.position[axis] as f64;
                shape.maximum[axis] += edit.position[axis] as f64;
            }
            bounds.push(shape);
        }
        let owner = global_to_section(edit.position).0;
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    sections.insert(owner.offset([x, y, z]).map_err(|error| error.to_string())?);
                }
            }
        }
    }
    let mut props = BTreeMap::<(SectionPos, PropId), WorldProp>::new();
    for section in sections {
        for prop in chunks
            .loaded
            .get(&section)
            .into_iter()
            .flat_map(|section| section.props.iter())
            .chain(journal.placed_props(section))
        {
            if prop.removed || journal.prop_removed(prop.section, prop.id) {
                continue;
            }
            let prop_bounds = prop.placement_bounds();
            if !bounds.iter().any(|shape| {
                (0..3).all(|axis| {
                    shape.minimum[axis] < prop_bounds[1][axis] - 1e-7
                        && shape.maximum[axis] > prop_bounds[0][axis] + 1e-7
                })
            }) {
                continue;
            }
            if matches!(prop.kind, PropKind::AppleFruit | PropKind::BeeNest) {
                return Err("Rail geometry intersects a protected fruit or hive".into());
            }
            props.entry((prop.section, prop.id)).or_insert(*prop);
            if props.len() > MAX_CLEARED_PROPS {
                return Err("Rail clearance exceeds its prop budget".into());
            }
        }
    }
    props
        .into_values()
        .map(|prop| {
            let bounds = prop.placement_bounds();
            let low = bounds[0].map(|value| value.floor() as i64);
            let high = bounds[1].map(|value| (value - 1e-7).floor() as i64);
            let mut rights = BTreeSet::from([crate::props::placement::support(&prop)]);
            for x in low[0]..=high[0] {
                for y in low[1]..=high[1] {
                    for z in low[2]..=high[2] {
                        rights.insert([x, y, z]);
                        if rights.len() > 4096 {
                            return Err(
                                "Rail prop clearance rights exceed their footprint budget".into()
                            );
                        }
                    }
                }
            }
            let source = if journal
                .placed_props(prop.section)
                .any(|saved| saved.id == prop.id)
            {
                PropSource::Saved
            } else {
                PropSource::Generated
            };
            Ok(PropClearance {
                prop,
                source,
                rights_cells: rights.into_iter().collect(),
            })
        })
        .collect()
}
