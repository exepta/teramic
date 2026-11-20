use super::EditJournal;
use crate::{generation::structures::StructureObject, props::WorldProp};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::farming::CropState;
use terra_voxel::{SectionPos, global_to_section};

#[cfg(test)]
mod tests;

fn extend_halo(
    result: &mut BTreeSet<SectionPos>,
    position: [i64; 3],
    radius: usize,
) -> Result<(), String> {
    let (owner, local) = global_to_section(position);
    let offsets = local.map(|v| {
        if v < radius {
            -1
        } else if v >= 16 - radius {
            1
        } else {
            0
        }
    });
    for x in [0, offsets[0]]
        .into_iter()
        .take(if offsets[0] == 0 { 1 } else { 2 })
    {
        for y in [0, offsets[1]]
            .into_iter()
            .take(if offsets[1] == 0 { 1 } else { 2 })
        {
            for z in [0, offsets[2]]
                .into_iter()
                .take(if offsets[2] == 0 { 1 } else { 2 })
            {
                result.insert(owner.offset([x, y, z]).map_err(|e| e.to_string())?);
            }
        }
    }
    Ok(())
}

impl EditJournal {
    /// First publication only. The recorded block is also the persistent
    /// initialization marker, including when a chest or field was emptied.
    pub fn initialize_structures(
        &mut self,
        objects: &[StructureObject],
    ) -> Result<BTreeSet<SectionPos>, String> {
        let mut affected = BTreeSet::new();
        let mut water = BTreeSet::new();
        let mut staged = Vec::new();
        let mut seen = BTreeSet::new();
        let mut revisions = BTreeMap::<SectionPos, u64>::new();
        let mut sources = BTreeMap::<SectionPos, u64>::new();
        let mut crops = BTreeMap::<SectionPos, usize>::new();
        let mut crop_ids = BTreeSet::new();
        let mut crop_count = 0_usize;
        let mut furniture = 0_u64;
        let mut private_revisions = 0_u64;
        for object in objects {
            let p = object.position;
            let source_repair = object.material == terra_voxel::MaterialId::FluidWater as u16
                && self.block(p) == Some(object.material)
                && self.fluid(p).is_none();
            if self.block(p).is_some() && !source_repair || !seen.insert(p) {
                continue;
            }
            if object
                .state
                .is_some_and(|state| !state.valid_for(p, object.material))
                || object.loot.as_ref().is_some_and(|loot| {
                    !terra_core::containers::is_chest(object.material)
                        || !terra_core::containers::valid_chest(loot)
                        || p.iter().any(|v| v.unsigned_abs() > 1_000_000)
                })
                || object.crop.is_some() && object.material != terra_core::farming::FARMLAND
            {
                return Err("Invalid generated structure object".into());
            }
            let crop = object.crop.map(|kind| {
                let id = crate::farming::crop_id(p, kind);
                let mut state = CropState::new(kind, id.0);
                state.progress_seconds = state.growth_duration_seconds;
                state.moisture = 1.0;
                let prop = WorldProp {
                    id,
                    kind: match kind {
                        terra_core::CropKind::Wheat => crate::props::PropKind::Wheat,
                        terra_core::CropKind::Corn => crate::props::PropKind::Corn,
                        terra_core::CropKind::Lettuce => crate::props::PropKind::Lettuce,
                    },
                    position: [p[0] as f64 + 0.5, p[1] as f64 + 1.0, p[2] as f64 + 0.5],
                    rotation: 0.0,
                    scale: 1.0,
                    variant: state.stage(),
                    section: global_to_section(p).0,
                    removed: false,
                };
                (prop, state)
            });
            // A removed or manually occupied crop identity is not resurrected.
            if let Some((prop, _)) = &crop {
                let extra = crops.get(&prop.section).copied().unwrap_or(0);
                let existing = self.sections.get(&prop.section);
                if !self.can_place_crop_prop(prop)
                    || self.prop_removed(prop.section, prop.id)
                    || self
                        .crop_props
                        .get(&prop.id)
                        .is_some_and(|&previous| previous != p)
                    || self.crops.len() + crop_count >= super::farming::MAX_CROPS
                    || existing.map_or(0, |s| s.placed_props.len()) + extra >= 256
                    || existing.map_or(0, |s| s.placed_props.len() + s.removed_props.len()) + extra
                        >= 3840
                    || !crop_ids.insert(prop.id)
                {
                    continue;
                }
                *crops.entry(prop.section).or_default() += 1;
                crop_count += 1;
            }
            if object
                .state
                .is_some_and(|s| object.material == 59 && !s.secondary)
            {
                let furnace = terra_core::furnace::FurnaceState {
                    outlet: [p[0], p[1] + 2, p[2]],
                    ..Default::default()
                };
                if !furnace.valid() {
                    return Err("Invalid generated furnace state".into());
                }
                private_revisions += u64::from(self.furnace_at(p) != Some(&furnace));
            }
            private_revisions += u64::from(
                object
                    .loot
                    .as_ref()
                    .is_some_and(|loot| self.container_at(p) != Some(loot)),
            );
            furniture += u64::from(!source_repair && matches!(object.material, 28 | 29));
            if object.material == terra_voxel::MaterialId::FluidWater as u16
                && self.fluid(p)
                    != Some(crate::water::FluidCell {
                        amount: crate::water::FLUID_CAPACITY,
                        flow: [0; 2],
                        source: true,
                    })
            {
                *sources.entry(global_to_section(p).0).or_default() += 1;
            }
            if !source_repair {
                *revisions.entry(global_to_section(p).0).or_default() += 1;
                extend_halo(&mut affected, p, 1)?;
            }
            extend_halo(&mut water, p, 2)?;
            staged.push((object, crop, source_repair));
        }
        // Validate the whole first-publication batch before inserting its persistent markers.
        for (&section, &count) in &revisions {
            self.revision(section)
                .checked_add(count)
                .ok_or("Structure revision overflow")?;
        }
        for (&section, &count) in &crops {
            self.prop_epoch(section)
                .checked_add(count as u64)
                .ok_or("Structure prop revision overflow")?;
        }
        for (&section, &count) in &sources {
            self.fluid_epoch(section)
                .checked_add(count)
                .ok_or("Structure fluid revision overflow")?;
        }
        self.next_furniture_order
            .checked_add(furniture)
            .ok_or("Structure furniture order overflow")?;
        self.gameplay()
            .revision
            .checked_add(private_revisions)
            .ok_or("Structure inventory revision overflow")?;
        self.invalidate_generated_water_sections(&water)
            .map_err(|e| e.to_string())?;
        for (object, crop, source_repair) in staged {
            let p = object.position;
            if !source_repair {
                self.register_generated_block(p, object.material, object.state)
                    .map_err(|e| e.to_string())?;
            }
            if object.material == terra_voxel::MaterialId::FluidWater as u16 {
                self.register_generated_source(p)
                    .map_err(|e| e.to_string())?;
            }
            if object
                .state
                .is_some_and(|s| object.material == 59 && !s.secondary)
            {
                self.set_furnace(
                    p,
                    terra_core::furnace::FurnaceState {
                        outlet: [p[0], p[1] + 2, p[2]],
                        ..Default::default()
                    },
                )?;
            }
            if let Some(loot) = &object.loot {
                self.set_container(p, loot.clone())?;
            }
            if let Some((prop, state)) = crop {
                if !self.place_prop(prop).map_err(|e| e.to_string())? {
                    return Err("Generated crop prop could not be initialized".into());
                }
                self.set_crop(p, state)?;
            }
        }
        Ok(affected)
    }
}
