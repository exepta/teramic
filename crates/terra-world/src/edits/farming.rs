use super::EditJournal;
#[cfg(feature = "bevy")]
use crate::props::WorldProp;
use crate::{farming::ForageRegrowth, props::PropId};
use terra_core::farming::CropState;
use terra_voxel::{SectionPos, global_to_section};

pub const MAX_CROPS: usize = 32_768;
pub const MAX_FORAGE_REGROWTH: usize = 8_192;

impl EditJournal {
    pub fn crop_at(&self, support: [i64; 3]) -> Option<&CropState> {
        self.crops.get(&support)
    }

    pub fn crops(&self) -> impl Iterator<Item = ([i64; 3], &CropState)> + '_ {
        self.crops.iter().map(|(&support, state)| (support, state))
    }

    pub fn crops_in_section(&self, section: SectionPos) -> impl Iterator<Item = [i64; 3]> + '_ {
        self.crop_sections
            .get(&section)
            .into_iter()
            .flat_map(|positions| positions.iter().copied())
    }

    pub fn cultivated_sections(&self) -> impl Iterator<Item = SectionPos> + '_ {
        self.crop_sections.keys().copied()
    }

    pub fn can_plant_crop(&self, support: [i64; 3]) -> bool {
        self.crops.len() < MAX_CROPS && !self.crops.contains_key(&support)
    }

    pub fn can_place_crop_prop(&self, prop: &crate::props::WorldProp) -> bool {
        let support = crate::props::placement::support(prop);
        let Some(kind) = prop.kind.crop() else {
            return false;
        };
        if !prop.valid()
            || prop.removed
            || prop.id != crate::farming::crop_id(support, kind)
            || !self.can_plant_crop(support)
        {
            return false;
        }
        self.sections.get(&prop.section).is_none_or(|edits| {
            edits.prop_epoch < u64::MAX
                && edits.placed_props.len() < 256
                && !edits.placed_props.contains_key(&prop.id)
                && (edits.removed_props.contains(&prop.id)
                    || edits.placed_props.len() + edits.removed_props.len() < 3840)
        })
    }

    #[cfg(feature = "bevy")]
    pub(crate) fn place_crop_prop(&mut self, prop: WorldProp) -> Result<bool, String> {
        if !self.can_place_crop_prop(&prop)
            || self.block(crate::props::placement::support(&prop))
                != Some(terra_core::farming::FARMLAND)
        {
            return Ok(false);
        }
        let edits = self.sections.entry(prop.section).or_default();
        let epoch = edits
            .prop_epoch
            .checked_add(1)
            .ok_or("Prop revision overflow")?;
        edits.removed_props.remove(&prop.id);
        edits.placed_props.insert(prop.id, prop);
        edits.prop_epoch = epoch;
        Ok(true)
    }

    pub fn set_crop(&mut self, support: [i64; 3], state: CropState) -> Result<bool, String> {
        let section = global_to_section(support).0;
        if !state.valid()
            || self.block(support) != Some(terra_core::farming::FARMLAND)
            || self
                .sections
                .get(&section)
                .and_then(|edits| edits.placed_props.get(&PropId(state.prop_id)))
                .is_none_or(|prop| {
                    prop.removed
                        || prop.kind.crop() != Some(state.kind)
                        || crate::props::placement::support(prop) != support
                        || prop.variant != state.stage()
                })
            || self
                .crop_props
                .get(&PropId(state.prop_id))
                .is_some_and(|&previous| previous != support)
            || (!self.crops.contains_key(&support) && self.crops.len() >= MAX_CROPS)
        {
            return Err("Invalid or orphaned crop state".into());
        }
        if self.crops.get(&support) == Some(&state) {
            return Ok(false);
        }
        if let Some(previous) = self.crops.insert(support, state) {
            self.crop_props.remove(&PropId(previous.prop_id));
        }
        self.crop_sections
            .entry(section)
            .or_default()
            .insert(support);
        self.crop_props.insert(PropId(state.prop_id), support);
        Ok(true)
    }

    /// Progress-only saves do not change terrain or prop epochs; visible stages do.
    pub fn advance_crop(&mut self, support: [i64; 3], state: CropState) -> Result<bool, String> {
        let previous = self.crop_at(support).copied().ok_or("Missing crop state")?;
        if previous.kind != state.kind || previous.prop_id != state.prop_id || !state.valid() {
            return Err("Mismatched crop state".into());
        }
        let section = global_to_section(support).0;
        let mut prop = self
            .sections
            .get(&section)
            .and_then(|edits| edits.placed_props.get(&PropId(state.prop_id)))
            .copied()
            .ok_or("Missing crop prop")?;
        prop.variant = state.stage();
        self.update_placed_prop(prop)
            .map_err(|error| error.to_string())?;
        self.set_crop(support, state)
    }

    pub fn remove_crop(&mut self, support: [i64; 3]) -> bool {
        let Some(state) = self.crops.remove(&support) else {
            return false;
        };
        self.crop_props.remove(&PropId(state.prop_id));
        let section = global_to_section(support).0;
        if let Some(positions) = self.crop_sections.get_mut(&section) {
            positions.remove(&support);
            if positions.is_empty() {
                self.crop_sections.remove(&section);
            }
        }
        true
    }

    pub fn forage_regrowth(&self) -> impl Iterator<Item = &ForageRegrowth> + '_ {
        self.forage_regrowth.values()
    }

    pub fn set_forage_regrowth(&mut self, state: ForageRegrowth) -> Result<bool, String> {
        if !state.valid()
            || !self.prop_removed(state.prop.section, state.prop.id)
            || (!self.forage_regrowth.contains_key(&state.prop.id)
                && self.forage_regrowth.len() >= MAX_FORAGE_REGROWTH)
        {
            return Err("Invalid forage regrowth state".into());
        }
        if self.forage_regrowth.get(&state.prop.id) == Some(&state) {
            return Ok(false);
        }
        self.forage_regrowth.insert(state.prop.id, state);
        Ok(true)
    }

    /// A grazed generated plant reuses its identity rather than accumulating tombstones.
    #[cfg(feature = "bevy")]
    pub(crate) fn revive_forage(&mut self, mut prop: WorldProp) -> Result<bool, String> {
        prop.removed = false;
        if !prop.valid() || !self.prop_removed(prop.section, prop.id) {
            return Ok(false);
        }
        let Some(edits) = self.sections.get_mut(&prop.section) else {
            return Ok(false);
        };
        if !edits.placed_props.contains_key(&prop.id) && edits.placed_props.len() >= 256 {
            return Ok(false);
        }
        let epoch = edits
            .prop_epoch
            .checked_add(1)
            .ok_or("Prop revision overflow")?;
        edits.removed_props.remove(&prop.id);
        edits.placed_props.insert(prop.id, prop);
        edits.prop_epoch = epoch;
        self.forage_regrowth.remove(&prop.id);
        Ok(true)
    }
}
