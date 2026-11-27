use super::EditJournal;
use serde::{Deserialize, Serialize};
use terra_voxel::global_to_section;

#[cfg(test)]
mod filling_pose_tests {
    use super::*;

    #[test]
    fn repeated_block_refresh_and_section_reload_preserve_filling_station_direction() {
        for material in [
            terra_core::infusion::HOLD_BLOCK,
            terra_core::infusion::FLUID_INFUSER,
        ] {
            let p = [-17, 20, -1];
            let mut journal = EditJournal::default();
            journal.record(p, material).unwrap();
            let state = BlockObjectState {
                facing: 3,
                ..Default::default()
            };
            journal.set_object_state(p, Some(state)).unwrap();
            journal.record(p, material).unwrap();
            assert_eq!(journal.object_state(p), Some(state));
            let mut replica = EditJournal::default();
            replica
                .import_delta(journal.section_delta(global_to_section(p).0))
                .unwrap();
            assert_eq!(replica.object_state(p), Some(state));
            assert!(!crate::weather::retains_snow(material));
        }
    }
}

#[cfg(test)]
mod train_attachment_pose_tests {
    use super::*;

    #[test]
    fn rail_builder_states_roundtrip_all_yaws_and_reject_unrelated_metadata() {
        let material = terra_core::train_parts::RAIL_BUILDER;
        let position = [-17, 20, -1];
        for facing in 0..4 {
            let state = BlockObjectState {
                facing,
                ..Default::default()
            };
            let mut journal = EditJournal::default();
            journal.record(position, material).unwrap();
            journal.set_object_state(position, Some(state)).unwrap();
            journal.record(position, material).unwrap();
            assert_eq!(journal.object_state(position), Some(state));
            let mut replica = EditJournal::default();
            replica
                .import_delta(journal.section_delta(global_to_section(position).0))
                .unwrap();
            assert_eq!(replica.object_state(position), Some(state));
            assert!(!BlockObjectState { facing: 4, ..state }.valid_for(position, material));
            assert!(
                !BlockObjectState {
                    peer: Some([-16, 20, -1]),
                    ..state
                }
                .valid_for(position, material)
            );
            assert!(
                !BlockObjectState {
                    secondary: true,
                    ..state
                }
                .valid_for(position, material)
            );
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidState {
    pub origin: [i64; 3],
    pub volume: u8,
    pub age: u8,
    pub source: bool,
}

impl LiquidState {
    /// A new downhill step can feed another run of this many blocks.
    pub(crate) const LAVA_RUN: u8 = 14;

    pub fn valid(self, position: [i64; 3], material: u16) -> bool {
        let horizontal = position[0]
            .abs_diff(self.origin[0])
            .saturating_add(position[2].abs_diff(self.origin[2]));
        position
            .iter()
            .chain(self.origin.iter())
            .all(|p| p.unsigned_abs() <= 1_000_000)
            && if material == terra_voxel::MaterialId::Lava as u16 {
                let descent = self.origin[1].abs_diff(position[1]);
                (1..=8).contains(&self.volume)
                    && self.age <= Self::LAVA_RUN
                    && horizontal <= u64::from(Self::LAVA_RUN) * (descent + 1)
                    && (!self.source || self.origin == position)
                    && self.origin[1] >= position[1]
                    && self.origin[1].abs_diff(position[1]) <= 512
            } else if material == terra_core::bile::FLUID {
                horizontal <= 4
                    && (if self.age & terra_core::bile::FINE != 0 {
                        (1..=64).contains(&self.volume)
                            && self.age & terra_core::bile::LIFETIME_MASK
                                <= if self.age & terra_core::bile::TIMER_8HZ != 0 {
                                    terra_core::bile::SPIT_STEPS
                                } else {
                                    25
                                }
                            && (self.source || self.age & terra_core::bile::LIFETIME_MASK == 0)
                    } else {
                        self.age <= 31 && (1..=8).contains(&self.volume)
                    })
                    && position[1].abs_diff(self.origin[1]) <= 512
                    && (!self.source || position == self.origin)
            } else {
                horizontal <= 4
                    && self.age <= 31
                    && terra_core::roads::is_tar(material)
                    && position[1] == self.origin[1]
                    && (1..=64).contains(&self.volume)
                    && (material == terra_core::roads::WET_TAR || self.age == 0)
            }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockObjectState {
    pub facing: u8,
    pub peer: Option<[i64; 3]>,
    pub secondary: bool,
    #[serde(default)]
    pub wood: terra_core::WoodSpecies,
    #[serde(default)]
    pub rail_curve: Option<terra_core::rails::CurveSpec>,
    #[serde(default)]
    pub liquid: Option<LiquidState>,
}

impl BlockObjectState {
    /// Naturally frozen surface-source height. Player-built ice has no marker.
    /// The source and zero flow are intentional: only still natural reservoirs freeze.
    pub fn frozen_water(self) -> Option<crate::water::FluidCell> {
        ((1..=8).contains(&self.facing) && self.peer.is_none() && !self.secondary).then_some(
            crate::water::FluidCell {
                amount: self.facing,
                source: true,
                flow: [0, 0],
            },
        )
    }
    pub fn from_yaw(yaw: f32) -> Self {
        Self {
            facing: ((yaw / std::f32::consts::FRAC_PI_2).round() as i32).rem_euclid(4) as u8,
            ..Self::default()
        }
    }
    pub fn right(self) -> [i64; 3] {
        [[1, 0, 0], [0, 0, -1], [-1, 0, 0], [0, 0, 1]][usize::from(self.facing.min(3))]
    }
    pub fn valid_at(self, position: [i64; 3]) -> bool {
        self.facing <= terra_core::construction::LAST_MIXED_SLAB
            && (!self.secondary || self.peer.is_some())
            && self.peer.is_none_or(|peer| {
                (0..3)
                    .map(|i| (i128::from(peer[i]) - i128::from(position[i])).abs())
                    .sum::<i128>()
                    == 1
                    && peer.iter().all(|p| p.unsigned_abs() <= 1_000_000)
            })
    }
    pub fn valid_for(self, position: [i64; 3], material: u16) -> bool {
        if let Some(liquid) = self.liquid {
            return self.peer.is_none()
                && !self.secondary
                && self.rail_curve.is_none()
                && self.wood == Default::default()
                && liquid.valid(position, material)
                && self.facing
                    == if material == terra_voxel::MaterialId::Lava as u16 {
                        liquid.volume - 1
                    } else if material == terra_core::bile::FLUID {
                        terra_core::bile::facing(liquid.volume, liquid.age)
                    } else {
                        terra_core::roads::tar_state(liquid.volume.div_ceil(8), liquid.age)
                    };
        }
        if terra_core::quartz_prisms::is_quartz(material) {
            return (self.facing < 4
                || terra_core::quartz_prisms::slice(material, self.facing).is_some())
                && self.peer.is_none()
                && !self.secondary
                && self.wood == Default::default()
                && self.rail_curve.is_none()
                && self.liquid.is_none();
        }
        if terra_core::rails::is_rail(material) {
            return crate::rails::valid_state(position, material, self);
        }
        if self.rail_curve.is_some() {
            return false;
        }
        // Tar packs its finite coating amount and drying stage into this byte;
        // it has no directional or attachment metadata. Validate before the
        // ordinary object-facing bound, which intentionally excludes byte 255.
        if terra_core::roads::is_tar(material) {
            return (material == terra_core::roads::WET_TAR || self.facing < 8)
                && self.peer.is_none()
                && !self.secondary
                && self.wood == Default::default()
                && position.iter().all(|p| p.unsigned_abs() <= 1_000_000);
        }
        if material == terra_core::kinetic::CONVEYOR {
            return self.facing < terra_core::kinetic::BELT_FACING_COUNT
                && (self.facing < 56 || self.peer.is_some())
                && !self.secondary
                && position.iter().all(|p| p.unsigned_abs() <= 1_000_000)
                && self.peer.is_none_or(|peer| {
                    let delta: [i128; 3] =
                        std::array::from_fn(|i| i128::from(peer[i]) - i128::from(position[i]));
                    peer.iter().all(|p| p.unsigned_abs() <= 1_000_000)
                        && delta.iter().any(|v| *v != 0)
                        && delta
                            .iter()
                            .all(|v| v.abs() <= i128::from(terra_core::kinetic::MAX_BELT_SEGMENTS))
                });
        }
        if matches!(material, 144..=146) {
            return self.facing < 6
                && !self.secondary
                && position.iter().all(|p| p.unsigned_abs() <= 1_000_000)
                && self.peer.is_none_or(|peer| {
                    peer.iter().all(|p| p.unsigned_abs() <= 1_000_000)
                        && (0..3).any(|i| peer[i] != position[i])
                        && (0..3).all(|i| {
                            peer[i].abs_diff(position[i])
                                <= u64::from(terra_core::kinetic::MAX_BELT_SEGMENTS) + 1
                        })
                });
        }
        if material == terra_core::crusher::CRANK_MATERIAL {
            return self.facing < 6
                && !self.secondary
                && position
                    .iter()
                    .all(|value| value.unsigned_abs() <= 1_000_000)
                && self.peer.is_some_and(|anchor| {
                    anchor.iter().all(|value| value.unsigned_abs() <= 1_000_000)
                        && [1, 2].into_iter().any(|distance| {
                            position
                                == std::array::from_fn(|axis| {
                                    anchor[axis]
                                        + terra_core::kinetic::axis(self.facing)[axis] * distance
                                })
                        })
                });
        }
        self.valid_at(position)
            && match material {
                id if id == terra_voxel::MaterialId::Fumarole as u16 // Legacy oriented vent props keep their saved state.
                    || terra_core::profession_stations::is_station(id)
                    || (terra_core::grave_blocks::is_prop(id)
                        || terra_core::blood_blocks::is_prop(id)
                        || terra_core::biome_flora::is_prop(id)) =>
                {
                    self.facing < 4 && self.peer.is_none() && !self.secondary
                }
                id if terra_core::castle::is_castle(id) => {
                    self.facing < 24 && self.peer.is_none() && !self.secondary
                }
                152..=153 | 157 | 159 => self.facing < 8 && !self.secondary,
                158 | 160 => self.facing < 4 && self.peer.is_none() && !self.secondary,
                154..=156 => self.facing < 4 && self.peer.is_none() && !self.secondary,
                161 => self.facing < 8 && self.peer.is_none() && !self.secondary,
                id if terra_core::construction::is_shape(id) => {
                    terra_core::construction::valid_facing(id, self.facing)
                        && self.peer.is_none()
                        && !self.secondary
                }
                247 => self.facing < 8 && self.peer.is_none() && !self.secondary,
                id if terra_core::locomotive_parts::is_part(id) && id != 248 => {
                    terra_core::locomotive_parts::valid_facing(id, self.facing)
                        && self.peer.is_none()
                        && !self.secondary
                }
                id if id == terra_core::rails::SWITCH_STAND
                    || terra_core::train_parts::is_train(id) && id != 248 =>
                {
                    self.facing < 4 && self.peer.is_none() && !self.secondary
                }
                162 | 170 | 171 => self.facing < 4 && self.peer.is_none() && !self.secondary,
                163..=169 | 172 => {
                    terra_core::plumbing::valid_facing(material, self.facing)
                        && self.peer.is_none()
                        && !self.secondary
                }
                141..=151 => {
                    self.facing < if material == 148 { 4 } else { 6 }
                        && self.peer.is_none()
                        && !self.secondary
                }
                131 => {
                    self.facing < terra_core::flower_pot::STATES
                        && self.peer.is_none()
                        && !self.secondary
                }
                98 => {
                    (self.facing <= 7 || (17..=23).contains(&self.facing))
                        && self.peer.is_none()
                        && !self.secondary
                }
                5 => self.facing == 8 && self.peer.is_none() && !self.secondary,
                49 => self.frozen_water().is_some(),
                id if terra_core::construction::is_shape(id) => {
                    terra_core::construction::valid_facing(material, self.facing)
                        && self.peer.is_none()
                        && !self.secondary
                }
                71..=73 => {
                    self.facing < terra_core::beds::BED_FACING_COUNT
                        && self.peer.is_some_and(|peer| {
                            let forward = [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]]
                                [usize::from(self.facing % 4)];
                            peer == std::array::from_fn(|axis| {
                                position[axis] + forward[axis] * if self.secondary { -1 } else { 1 }
                            })
                        })
                }
                28 | 29 => self.facing < 4 && self.peer.is_none_or(|peer| peer[1] == position[1]),
                99 => {
                    self.facing < 4
                        && (!self.secondary || self.peer.is_some())
                        && self.peer.is_none_or(|peer| {
                            peer == std::array::from_fn(|axis| {
                                position[axis]
                                    + self.right()[axis] * if self.secondary { -1 } else { 1 }
                            })
                        })
                }
                59 | 65..=67 | 139 | 248 => {
                    self.facing < if material == 59 { 4 } else { 8 }
                        && self.peer.is_some_and(|peer| {
                            peer == [
                                position[0],
                                position[1] + if self.secondary { -1 } else { 1 },
                                position[2],
                            ]
                        })
                }
                60 => {
                    (self.facing < 4
                        || self.facing >= 8
                            && crate::objects::chimney_ports(60, self.facing).is_some())
                        && self.peer.is_none()
                        && !self.secondary
                }
                61..=64 => self.facing < 4 && self.peer.is_none() && !self.secondary,
                68..=70 | 140 => self.facing < 8 && self.peer.is_none() && !self.secondary,
                40 | terra_core::block_shapes::VIOLET_TORCH => {
                    (self.facing == 0 || (4..8).contains(&self.facing))
                        && self.peer.is_none()
                        && !self.secondary
                }
                terra_core::lantern::LANTERN | terra_core::lantern::CHAIN => {
                    terra_core::lantern::valid_facing(material, self.facing)
                        && self.peer.is_none()
                        && !self.secondary
                }
                _ => false,
            }
    }
}
impl EditJournal {
    /// Rail-bearing sparse sections include unloaded, persisted modules.
    pub fn rail_sections(&self) -> impl Iterator<Item = terra_voxel::SectionPos> + '_ {
        self.rail_sections.iter().copied()
    }
    pub fn rail_module_count(&self) -> usize {
        self.rail_anchor_counts.values().sum()
    }
    /// Runtime-only geometry stamp; ordinary terrain edits do not change it.
    pub fn rail_epoch(&self) -> u64 {
        self.rail_epoch
    }
    pub(crate) fn rail_cache_identity(&self) -> std::sync::Arc<()> {
        self.rail_identity.clone()
    }
    /// Sparse runtime index, including generated metadata and unloaded objects.
    /// Ordinary terrain edits do not make a section an object-render candidate.
    pub fn object_sections(&self) -> impl ExactSizeIterator<Item = terra_voxel::SectionPos> + '_ {
        self.object_sections.iter().copied()
    }

    pub(super) fn refresh_object_section(&mut self, section: terra_voxel::SectionPos) {
        let has_rail = self
            .object_states_in_section(section)
            .any(|(_, material, _)| terra_core::rails::is_rail(material));
        if has_rail {
            self.rail_sections.insert(section);
        } else {
            self.rail_sections.remove(&section);
        }
        let anchors = self
            .object_states_in_section(section)
            .filter(|(_, material, state)| {
                terra_core::rails::is_rail(*material) && !state.secondary
            })
            .count();
        if anchors == 0 {
            self.rail_anchor_counts.remove(&section);
        } else {
            self.rail_anchor_counts.insert(section, anchors);
        }
        let holds: std::collections::BTreeSet<_> = self
            .object_states_in_section(section)
            .filter_map(|(p, material, state)| {
                (material == terra_core::infusion::HOLD_BLOCK && state.valid_for(p, material))
                    .then_some(p)
            })
            .collect();
        if holds.is_empty() {
            self.hold_sections.remove(&section);
        } else {
            self.hold_sections.insert(section, holds.clone());
        }
        self.infusion_views
            .retain(|p, _| terra_voxel::global_to_section(*p).0 != section || holds.contains(p));
        if self
            .sections
            .get(&section)
            .is_some_and(|edits| !edits.objects.is_empty())
        {
            self.object_sections.insert(section);
        } else {
            self.object_sections.remove(&section);
        }
    }

    /// Public placement metadata without cloning a complete section delta or
    /// inspecting private machine inventories. Suitable for loaded-area renderers.
    pub fn object_states_in_section(
        &self,
        section: terra_voxel::SectionPos,
    ) -> impl Iterator<Item = ([i64; 3], u16, BlockObjectState)> + '_ {
        let origin = section.origin().ok();
        self.sections
            .get(&section)
            .into_iter()
            .flat_map(move |edits| {
                edits.objects.iter().filter_map(move |(local, state)| {
                    let origin = origin?;
                    let position = std::array::from_fn(|axis| origin[axis] + local[axis] as i64);
                    Some((position, *edits.blocks.get(local)?, *state))
                })
            })
    }

    pub fn object_states(&self) -> impl Iterator<Item = ([i64; 3], u16, BlockObjectState)> + '_ {
        self.sections.iter().flat_map(|(section, edits)| {
            edits.objects.iter().filter_map(move |(local, state)| {
                let origin = section.origin().ok()?;
                let position = std::array::from_fn(|axis| origin[axis] + local[axis] as i64);
                Some((position, *edits.blocks.get(local)?, *state))
            })
        })
    }
    pub fn object_state(&self, position: [i64; 3]) -> Option<BlockObjectState> {
        let (section, local) = global_to_section(position);
        self.sections.get(&section)?.objects.get(&local).copied()
    }
    pub fn set_object_state(
        &mut self,
        position: [i64; 3],
        state: Option<BlockObjectState>,
    ) -> Result<Vec<terra_voxel::SectionPos>, String> {
        if self.object_state(position) == state {
            return Ok(Vec::new());
        }
        let material = self
            .block(position)
            .ok_or("Object state requires a recorded block")?;
        if state.is_some_and(|s| !s.valid_for(position, material)) {
            return Err("Object state on ordinary block".into());
        }
        let (owner, local) = global_to_section(position);
        let clear_tint = self.tint_at(position).is_some()
            && !terra_core::tints::cell_dyeable(material, state.map_or(0, |state| state.facing));
        let tint_epoch = if clear_tint {
            Some(
                self.appearance_epoch(owner)
                    .checked_add(1)
                    .ok_or("Object tint appearance overflow")?,
            )
        } else {
            None
        };
        let affected = self.record(position, material).map_err(|e| e.to_string())?;
        let edits = self.sections.entry(owner).or_default();
        if let Some(state) = state {
            edits.objects.insert(local, state);
        } else {
            edits.objects.remove(&local);
            edits.belts.remove(&local);
            self.distributors.remove(&position);
            self.logistics_filters.remove(&position);
            self.random_tunnels.remove(&position);
            self.cauldrons.remove(&position);
            self.campfires.remove(&position);
            if matches!(material, 161..=162) {
                self.containers.remove(&position);
            }
        }
        // record() sees the old pose. A mixed stone/wool slab can become pure
        // stone without changing its primary material, so validate the final pose.
        if let Some(epoch) = tint_epoch {
            edits.clear_pose_tint(local, epoch);
        }
        if state.is_none() {
            self.remove_plumbing(position);
        }
        self.refresh_object_section(owner);
        if material == terra_core::brewing::MATERIAL
            && !state.is_some_and(|state| terra_core::brewing::has_fire(state.facing))
        {
            self.attached_campfires.remove(&position);
        }
        if matches!(material, 144..=146) {
            if state.is_some_and(|state| state.peer.is_some()) {
                self.extended_kinetic_sections
                    .entry(owner)
                    .or_default()
                    .insert(position);
            } else if let Some(positions) = self.extended_kinetic_sections.get_mut(&owner) {
                positions.remove(&position);
                if positions.is_empty() {
                    self.extended_kinetic_sections.remove(&owner);
                }
            }
        }
        Ok(affected)
    }
}

#[cfg(test)]
#[path = "objects_pot_tests.rs"]
mod flower_pot_tests;

#[cfg(test)]
mod section_index_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ordinary_edit_history_does_not_expand_the_object_section_index() {
        let mut journal = EditJournal::default();
        for x in 0..512 {
            journal.record([x * 16, 2, 0], 3).unwrap();
        }
        assert_eq!(journal.object_sections().len(), 0);
        let first = [-1, 15, -17];
        let second = [-2, 15, -17];
        let section = global_to_section(first).0;
        for position in [first, second] {
            journal.record(position, 29).unwrap();
            journal
                .set_object_state(position, Some(BlockObjectState::default()))
                .unwrap();
        }
        assert_eq!(journal.object_sections().collect::<Vec<_>>(), vec![section]);
        journal.set_object_state(first, None).unwrap();
        assert_eq!(journal.object_sections().len(), 1);
        journal.record(second, 3).unwrap();
        assert_eq!(journal.object_sections().len(), 0);
        assert_eq!(journal.object_states_in_section(section).count(), 0);
    }

    #[test]
    fn generated_metadata_and_mechanical_sections_are_indexed_without_terrain_invalidation() {
        let mut journal = EditJournal::default();
        let owner = [-1, 15, -16];
        let peer = [-1, 16, -16];
        let shaft = [-17, 4, -17];
        for (position, material, state) in [
            (
                owner,
                65,
                BlockObjectState {
                    peer: Some(peer),
                    ..Default::default()
                },
            ),
            (
                peer,
                65,
                BlockObjectState {
                    peer: Some(owner),
                    secondary: true,
                    ..Default::default()
                },
            ),
            (shaft, 144, BlockObjectState::default()),
        ] {
            assert!(
                journal
                    .register_generated_block(position, material, Some(state))
                    .unwrap()
            );
            assert!(
                !journal
                    .register_generated_block(position, material, Some(state))
                    .unwrap()
            );
        }
        let expected: HashSet<_> = [owner, peer, shaft].map(|p| global_to_section(p).0).into();
        assert_eq!(journal.object_sections().collect::<HashSet<_>>(), expected);
        assert!(
            journal
                .object_sections()
                .all(|p| journal.mesh_epoch(p) == 0)
        );
        assert_eq!(
            journal.kinetic_sections().collect::<Vec<_>>(),
            vec![global_to_section(shaft).0]
        );
        assert_eq!(
            journal
                .kinetics_in_section(global_to_section(shaft).0)
                .collect::<Vec<_>>(),
            vec![shaft]
        );
    }

    #[test]
    fn snapshot_replacement_clone_and_stale_import_keep_sparse_object_sections_consistent() {
        let mut source = EditJournal::default();
        let position = [-17, -1, -33];
        let section = global_to_section(position).0;
        source.record(position, 29).unwrap();
        source
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
        let original = source.section_delta(section);
        let mut replica = EditJournal::default();
        replica.import_delta(original.clone()).unwrap();
        assert_eq!(replica.object_sections().collect::<Vec<_>>(), vec![section]);
        assert_eq!(
            replica.clone().object_sections().collect::<Vec<_>>(),
            vec![section]
        );
        let mut cleared = original.clone();
        cleared.revision += 1;
        cleared.objects.clear();
        replica.import_delta(cleared.clone()).unwrap();
        assert_eq!(replica.block(position), Some(29));
        assert_eq!(replica.object_sections().len(), 0);
        replica.import_delta(original.clone()).unwrap();
        assert_eq!(
            replica.object_sections().len(),
            0,
            "a stale snapshot cannot resurrect metadata"
        );
        let mut restored = original;
        restored.revision = cleared.revision + 1;
        replica.import_delta(restored).unwrap();
        assert_eq!(replica.object_sections().collect::<Vec<_>>(), vec![section]);
    }
}
