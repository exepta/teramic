use super::EditJournal;
use terra_core::{ItemKind, kinetic::Cell};
use terra_voxel::{SectionPos, global_to_section};

/// A belt's own item metadata must not replace the wood or pose of its shaft.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BeltOwner {
    pub start: Cell,
    pub end: Cell,
    pub item: ItemKind,
}

impl BeltOwner {
    pub fn valid_at(self, controller: Cell) -> bool {
        if !self.item.valid()
            || self.item.block_material() != Some(terra_core::kinetic::CONVEYOR)
            || self
                .start
                .iter()
                .chain(&self.end)
                .chain(&controller)
                .any(|value| value.unsigned_abs() > 990_000)
        {
            return false;
        }
        let delta: Cell = std::array::from_fn(|axis| self.end[axis] - self.start[axis]);
        let distance = delta
            .iter()
            .map(|value| value.unsigned_abs())
            .max()
            .unwrap_or(0);
        if !(2..=u64::from(terra_core::kinetic::MAX_BELT_SEGMENTS) + 1).contains(&distance)
            || delta.iter().filter(|&&value| value != 0).count() > 2
            || delta
                .iter()
                .any(|value| *value != 0 && value.unsigned_abs() != distance)
        {
            return false;
        }
        (1..distance).any(|index| {
            (0..3).all(|axis| {
                self.start[axis] + delta[axis].signum() * index as i64 == controller[axis]
            })
        })
    }
}

impl EditJournal {
    pub fn belt_owner(&self, position: Cell) -> Option<BeltOwner> {
        let (section, local) = global_to_section(position);
        self.sections.get(&section)?.belts.get(&local).copied()
    }

    pub fn set_belt_owner(
        &mut self,
        position: Cell,
        owner: Option<BeltOwner>,
    ) -> Result<Vec<SectionPos>, String> {
        if self.belt_owner(position) == owner {
            return Ok(Vec::new());
        }
        let material = self
            .block(position)
            .ok_or("Belt owner requires a recorded block")?;
        if owner.is_some_and(|owner| {
            !owner.valid_at(position)
                || !matches!(material, 144..=146 | 148)
                || self.object_state(position).is_none()
        }) {
            return Err("Invalid belt owner metadata".into());
        }
        let (section, local) = global_to_section(position);
        let appearance_only = self
            .belt_owner(position)
            .zip(owner)
            .is_some_and(|(old, new)| old.start == new.start && old.end == new.end);
        let revision = self
            .revision(section)
            .checked_add(1)
            .ok_or("Belt revision overflow")?;
        let appearance = self
            .appearance_epoch(section)
            .checked_add(1)
            .ok_or("Belt appearance overflow")?;
        let affected = if appearance_only {
            Vec::new()
        } else {
            self.record(position, material)
                .map_err(|error| error.to_string())?
        };
        let edits = self.sections.get_mut(&section).unwrap();
        edits.commit_belt_owner(
            local,
            owner,
            appearance_only.then_some(revision),
            appearance,
        );
        Ok(affected)
    }
}
