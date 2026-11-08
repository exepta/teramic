//! Bounded immutable placement metadata for background collision workers.
use super::EditJournal;
use terra_voxel::{SectionPos, blocks::BlockError, global_to_section};

fn copy_cell(source: &EditJournal, snapshot: &mut EditJournal, cell: [i64; 3]) {
    let (owner, local) = global_to_section(cell);
    let Some(source) = source.sections.get(&owner) else {
        return;
    };
    let target = snapshot.sections.entry(owner).or_default();
    if let Some(material) = source.blocks.get(&local) {
        target.blocks.insert(local, *material);
    }
    if let Some(state) = source.objects.get(&local) {
        target.objects.insert(local, *state);
    }
    if let Some(belt) = source.belts.get(&local) {
        target.belts.insert(local, *belt);
    }
}

impl EditJournal {
    /// Copy the section halo used by collision geometry, excluding all gameplay,
    /// inventories, simulation queues, props, fluids and appearance payloads.
    /// The returned journal is a read-only worker view, not a persistence source.
    pub fn geometry_snapshot(&self, center: SectionPos) -> Result<Self, BlockError> {
        let mut snapshot = Self::default();
        for x in -1..=1 {
            for y in -2..=2 {
                for z in -1..=1 {
                    let section = center.offset([x, y, z])?;
                    if let Some(source) = self.sections.get(&section) {
                        snapshot.sections.insert(section, source.geometry_only());
                    }
                    if let Some(owners) = self.extended_kinetic_sections.get(&section) {
                        snapshot
                            .extended_kinetic_sections
                            .insert(section, owners.clone());
                    }
                }
            }
        }
        // Window tiling and double-door parity consult a connected row of at
        // most 64 cells. Keep only opening poses in that larger finite radius;
        // their block IDs suffice for the shape lookup, so distant terrain and
        // unrelated machine sections are never copied.
        for x in -4..=4 {
            for y in -4..=4 {
                for z in -4..=4 {
                    if (-1..=1).contains(&x) && (-2..=2).contains(&y) && (-1..=1).contains(&z) {
                        continue;
                    }
                    let owner = center.offset([x, y, z])?;
                    let Some(source) = self.sections.get(&owner) else {
                        continue;
                    };
                    for (&local, &state) in &source.objects {
                        if let Some(&material) = source.blocks.get(&local)
                            && crate::objects::is_opening(material)
                        {
                            let target = snapshot.sections.entry(owner).or_default();
                            target.blocks.insert(local, material);
                            target.objects.insert(local, state);
                        }
                    }
                }
            }
        }
        // A stretched belt's controller can be outside the local halo. Its
        // bounded ownership chain needs at most three metadata hops, never a
        // recursive world copy or a complete distant section.
        for _ in 0..3 {
            let peers: Vec<_> = snapshot
                .sections
                .values()
                .flat_map(|section| section.objects.values().filter_map(|state| state.peer))
                .filter(|peer| snapshot.object_state(*peer).is_none())
                .collect();
            if peers.is_empty() {
                break;
            }
            for peer in peers {
                copy_cell(self, &mut snapshot, peer);
            }
        }
        // Owned belt validation checks every segment, including those outside
        // this section's physical halo. Each owner contributes at most 34
        // metadata cells and never causes a terrain or inventory copy.
        let belts: Vec<_> = snapshot
            .sections
            .iter()
            .flat_map(|(section, edits)| {
                edits.belts.iter().filter_map(|(local, belt)| {
                    section
                        .global_block(*local)
                        .ok()
                        .map(|controller| (controller, *belt))
                })
            })
            .collect();
        for (controller, belt) in belts {
            if !belt.valid_at(controller) {
                continue;
            }
            let delta: [i64; 3] = std::array::from_fn(|axis| belt.end[axis] - belt.start[axis]);
            let length = delta
                .iter()
                .map(|value| value.unsigned_abs())
                .max()
                .unwrap_or(0);
            for index in 0..=length {
                let cell = std::array::from_fn(|axis| {
                    belt.start[axis] + delta[axis].signum() * index as i64
                });
                copy_cell(self, &mut snapshot, cell);
            }
        }
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edits::BlockObjectState;
    use terra_core::{ItemKind, ItemStack, PlayerUuid};

    #[test]
    fn geometry_view_keeps_local_poses_and_extended_indexes_without_private_world_state() {
        let mut journal = EditJournal::default();
        let center = SectionPos::default();
        let near = [15, 5, 8];
        let extended = [16, 5, 8];
        let far = [1600, 5, 8];
        for (cell, material) in [(near, 29), (extended, 151), (far, 29)] {
            journal.record(cell, material).unwrap();
            journal
                .set_object_state(cell, Some(BlockObjectState::default()))
                .unwrap();
        }
        let inventory = terra_core::Inventory {
            slots: vec![ItemStack {
                item: ItemKind::Block(3),
                count: 9,
            }],
            ..Default::default()
        };
        journal.set_container(near, inventory.clone()).unwrap();
        journal
            .gameplay_mut()
            .inventories
            .insert(PlayerUuid::from_u128(1), inventory);
        let snapshot = journal.geometry_snapshot(center).unwrap();
        assert_eq!(snapshot.block(near), Some(29));
        assert_eq!(
            snapshot.object_state(extended),
            journal.object_state(extended)
        );
        assert_eq!(
            snapshot
                .extended_kinetics_in_section(SectionPos::new(1, 0, 0))
                .collect::<Vec<_>>(),
            vec![extended]
        );
        assert!(snapshot.block(far).is_none());
        assert!(snapshot.container_at(near).is_none());
        assert!(snapshot.gameplay().inventories.is_empty());
        journal.record(near, 0).unwrap();
        assert_eq!(
            snapshot.block(near),
            Some(29),
            "worker inputs are immutable"
        );
    }

    #[test]
    fn remote_belt_controller_retains_every_validated_segment_without_distant_section_copies() {
        let mut journal = EditJournal::default();
        let start = [-32, 4, 8];
        let controller = [-31, 4, 8];
        let end = [1, 4, 8];
        for x in start[0]..=end[0] {
            let cell = [x, 4, 8];
            let material = if cell == start || cell == end {
                144
            } else {
                148
            };
            journal.record(cell, material).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(BlockObjectState {
                        facing: 1,
                        peer: Some(if cell == end {
                            start
                        } else if cell == controller {
                            end
                        } else {
                            controller
                        }),
                        wood: terra_core::WoodSpecies::Birch,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        journal
            .set_belt_owner(
                controller,
                Some(super::super::BeltOwner {
                    start,
                    end,
                    item: ItemKind::for_wood(148, terra_core::WoodSpecies::Birch),
                }),
            )
            .unwrap();
        journal.record([-31, 4, 15], 3).unwrap();
        let snapshot = journal.geometry_snapshot(SectionPos::default()).unwrap();
        let selected = [0, 4, 8];
        let live = crate::conveyors::span_at(&journal, selected).unwrap();
        assert_eq!(live.length, 32);
        assert_eq!(crate::conveyors::span_at(&snapshot, selected), Some(live));
        assert!(
            snapshot.block([-31, 4, 15]).is_none(),
            "peer sections contribute only required cells"
        );
    }
}
