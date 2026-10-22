use super::EditJournal;
use serde::{Deserialize, Serialize};
use terra_voxel::global_to_section;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockObjectState {
    pub facing: u8,
    pub peer: Option<[i64; 3]>,
    pub secondary: bool,
    #[serde(default)]
    pub wood: terra_core::WoodSpecies,
}
impl BlockObjectState {
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
        self.valid_at(position)
            && match material {
                98 => self.facing <= 7 && self.peer.is_none() && !self.secondary,
                5 => self.facing == 8 && self.peer.is_none() && !self.secondary,
                74..=97 => {
                    terra_core::construction::valid_facing(material, self.facing)
                        && self.peer.is_none()
                        && !self.secondary
                }
                71..=73 => {
                    self.facing < 28
                        && self.peer.is_some_and(|peer| {
                            let forward = [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]]
                                [usize::from(self.facing % 4)];
                            peer == std::array::from_fn(|axis| {
                                position[axis] + forward[axis] * if self.secondary { -1 } else { 1 }
                            })
                        })
                }
                28 | 29 => self.facing < 4 && self.peer.is_none_or(|peer| peer[1] == position[1]),
                59 | 65..=67 => {
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
                68..=70 => self.facing < 8 && self.peer.is_none() && !self.secondary,
                40 => {
                    (self.facing == 0 || (4..8).contains(&self.facing))
                        && self.peer.is_none()
                        && !self.secondary
                }
                _ => false,
            }
    }
}
impl EditJournal {
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
        if state.is_some_and(|state| !state.valid_at(position)) {
            return Err("Invalid block object state".into());
        }
        if self.object_state(position) == state {
            return Ok(Vec::new());
        }
        let material = self
            .block(position)
            .ok_or("Object state requires a recorded block")?;
        if state.is_some_and(|s| !s.valid_for(position, material)) {
            return Err("Object state on ordinary block".into());
        }
        let affected = self.record(position, material).map_err(|e| e.to_string())?;
        let (owner, local) = global_to_section(position);
        let edits = self.sections.entry(owner).or_default();
        if let Some(state) = state {
            edits.objects.insert(local, state);
        } else {
            edits.objects.remove(&local);
        }
        Ok(affected)
    }
}
