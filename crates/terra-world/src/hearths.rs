//! Loaded hearth topology is shared by ignition, brewing and the temperature hook.
use crate::{edits::EditJournal, streaming::ChunkStore};
use terra_core::{block_properties::BlockPropertiesCatalog, brewing, campfire};
use terra_voxel::MaterialId;

pub const HEAT_RADIUS: f64 = 3.5;
type Cell = [i64; 3];

pub fn loaded_fire(chunks: &ChunkStore, position: Cell) -> bool {
    matches!(
        chunks.block_at(position),
        Some(campfire::MATERIAL | brewing::MATERIAL)
    ) && [
        [0, -1, 0],
        [0, 1, 0],
        [-1, 0, 0],
        [1, 0, 0],
        [0, 0, -1],
        [0, 0, 1],
    ]
    .into_iter()
    .all(|offset| {
        chunks
            .block_at(std::array::from_fn(|axis| position[axis] + offset[axis]))
            .is_some()
    })
}

pub fn can_light(chunks: &ChunkStore, journal: &EditJournal, position: Cell) -> bool {
    let attached = chunks.block_at(position) == Some(brewing::MATERIAL)
        && journal.has_attached_campfire(position);
    if !loaded_fire(chunks, position)
        || !(attached || journal.valid_campfire(position))
        || journal.fluid(position).is_some_and(|cell| cell.amount > 0)
    {
        return false;
    }
    let below = [position[0], position[1] - 1, position[2]];
    let above = [position[0], position[1] + 1, position[2]];
    if !chunks
        .block_at(below)
        .and_then(MaterialId::from_raw)
        .is_some_and(|material| {
            material.is_solid() && (!material.flammable() || material == MaterialId::Grass)
        })
        || !attached && !matches!(chunks.block_at(above), Some(0 | brewing::MATERIAL))
        || journal.fluid(above).is_some_and(|cell| cell.amount > 0)
    {
        return false;
    }
    // Require a known, non-flammable firebreak beside the exposed flame.
    [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]]
        .into_iter()
        .all(|offset| {
            let cell = std::array::from_fn(|axis| position[axis] + offset[axis]);
            chunks
                .block_at(cell)
                .and_then(MaterialId::from_raw)
                .is_some_and(|material| material != MaterialId::FluidWater && !material.flammable())
                && journal.fluid(cell).is_none_or(|cell| cell.amount == 0)
        })
}

pub fn heated_cauldron(chunks: &ChunkStore, journal: &EditJournal, position: Cell) -> bool {
    if chunks.block_at(position) == Some(brewing::MATERIAL)
        && journal.has_attached_campfire(position)
        && can_light(chunks, journal, position)
        && journal
            .attached_campfire_at(position)
            .is_some_and(|fire| fire.state.lit && fire.state.burn_remaining > 0.0)
    {
        return true;
    }
    let below = [position[0], position[1] - 1, position[2]];
    chunks.block_at(position) == Some(brewing::MATERIAL)
        && journal.valid_cauldron(position)
        && can_light(chunks, journal, below)
        && journal
            .campfire_at(below)
            .is_some_and(|state| state.lit && state.burn_remaining > 0.0)
}

/// Unknown roof cells are sheltered until loading resolves them, never assumed sky.
pub fn rain_exposed(chunks: &ChunkStore, journal: &EditJournal, position: Cell) -> bool {
    if journal.has_attached_campfire(position) {
        return false;
    }
    (1..=64).all(|dy| {
        let cell = [position[0], position[1] + dy, position[2]];
        journal
            .block(cell)
            .or_else(|| chunks.block_at(cell))
            .and_then(MaterialId::from_raw)
            .is_some_and(|material| !material.is_solid() && material != MaterialId::FluidWater)
    })
}

/// Zero behind solid geometry or unloaded cells; useful to F32 without a second temperature model.
pub fn heat_at(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    source: Cell,
    target: [f64; 3],
) -> f32 {
    if !target.iter().all(|value| value.is_finite())
        || !can_light(chunks, journal, source)
        || !journal
            .hearth_at(source)
            .is_some_and(|state| state.lit && state.burn_remaining > 0.0)
    {
        return 0.0;
    }
    let origin = [
        source[0] as f64 + 0.5,
        source[1] as f64
            + if journal.has_attached_campfire(source) {
                0.27
            } else {
                0.55
            },
        source[2] as f64 + 0.5,
    ];
    let delta: [f64; 3] = std::array::from_fn(|axis| target[axis] - origin[axis]);
    let distance = delta.iter().map(|value| value * value).sum::<f64>().sqrt();
    if distance >= HEAT_RADIUS {
        return 0.0;
    }
    if distance > 0.000001
        && crate::interaction_shapes::raycast_solid(
            chunks,
            journal,
            properties,
            origin,
            delta.map(|value| value / distance),
            distance,
        )
        .is_some()
    {
        return 0.0;
    }
    (1.0 - distance / HEAT_RADIUS) as f32
}

pub fn contacts_flame(source: Cell, center: [f64; 3], half: [f64; 3]) -> bool {
    if !center.iter().all(|value| value.is_finite())
        || !half.iter().all(|value| value.is_finite() && *value >= 0.0)
    {
        return false;
    }
    let minimum = [
        source[0] as f64 + 0.2,
        source[1] as f64,
        source[2] as f64 + 0.2,
    ];
    let maximum = [
        source[0] as f64 + 0.8,
        source[1] as f64 + 0.65,
        source[2] as f64 + 0.8,
    ];
    (0..3).all(|axis| {
        center[axis] + half[axis] > minimum[axis] && center[axis] - half[axis] < maximum[axis]
    })
}

pub fn contacts_hearth_flame(
    journal: &EditJournal,
    source: Cell,
    center: [f64; 3],
    half: [f64; 3],
) -> bool {
    if !journal.has_attached_campfire(source) {
        return contacts_flame(source, center, half);
    }
    if !center.iter().all(|value| value.is_finite())
        || !half.iter().all(|value| value.is_finite() && *value >= 0.0)
    {
        return false;
    }
    let low = crate::interaction_shapes::attached_campfire_point([0.2, 0.0, 0.2]);
    let high = crate::interaction_shapes::attached_campfire_point([0.8, 0.35, 0.8]);
    (0..3).all(|axis| {
        center[axis] + half[axis] > source[axis] as f64 + low[axis]
            && center[axis] - half[axis] < source[axis] as f64 + high[axis]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{edits::BlockObjectState, streaming::LoadedSection};
    use terra_voxel::{BlockSection, global_to_section};

    fn fixture(position: Cell) -> (ChunkStore, EditJournal) {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let owner = global_to_section(position).0;
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let section = owner.offset([x, y, z]).unwrap();
                    chunks.loaded.insert(
                        section,
                        LoadedSection {
                            data: BlockSection::empty(section).unwrap(),
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: None,
                            props: Vec::new(),
                        },
                    );
                }
            }
        }
        put(
            &mut chunks,
            &mut journal,
            [position[0], position[1] - 1, position[2]],
            1,
        );
        put(&mut chunks, &mut journal, position, campfire::MATERIAL);
        journal
            .set_campfire(
                position,
                terra_core::campfire::CampfireState {
                    lit: true,
                    burn_remaining: 30.0,
                    burn_total: 30.0,
                    ..Default::default()
                },
            )
            .unwrap();
        (chunks, journal)
    }

    fn put(chunks: &mut ChunkStore, journal: &mut EditJournal, position: Cell, material: u16) {
        let (owner, local) = global_to_section(position);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, material)
            .unwrap();
        journal.record(position, material).unwrap();
        if matches!(material, brewing::MATERIAL | campfire::MATERIAL) {
            journal
                .set_object_state(position, Some(BlockObjectState::default()))
                .unwrap();
        }
    }

    #[test]
    fn heating_crosses_sections_but_not_missing_support_water_or_flammable_enclosures() {
        for position in [[8, 15, 8], [-17, -1, -1]] {
            let (mut chunks, mut journal) = fixture(position);
            assert!(can_light(&chunks, &journal, position));
            let kettle = [position[0], position[1] + 1, position[2]];
            put(&mut chunks, &mut journal, kettle, brewing::MATERIAL);
            assert!(heated_cauldron(&chunks, &journal, kettle));
            chunks.loaded.remove(&global_to_section(kettle).0);
            assert!(!heated_cauldron(&chunks, &journal, kettle));
        }
        for obstruction in [27, 7] {
            let (mut chunks, mut journal) = fixture([8, 4, 8]);
            put(&mut chunks, &mut journal, [9, 4, 8], obstruction);
            assert!(!can_light(&chunks, &journal, [8, 4, 8]));
        }
        let (mut chunks, mut journal) = fixture([8, 4, 8]);
        put(&mut chunks, &mut journal, [8, 3, 8], 0);
        assert!(!can_light(&chunks, &journal, [8, 4, 8]));
    }

    #[test]
    fn heat_is_local_occluded_and_contact_is_not_the_entire_block() {
        let fire = [8, 4, 8];
        let (mut chunks, mut journal) = fixture(fire);
        let properties =
            BlockPropertiesCatalog::load(&terra_core::asset_directory().join("terra/blocks"))
                .unwrap();
        assert!(heat_at(&chunks, &journal, &properties, fire, [10.5, 4.55, 8.5]) > 0.0);
        assert_eq!(
            heat_at(&chunks, &journal, &properties, fire, [12.5, 4.55, 8.5]),
            0.0
        );
        put(&mut chunks, &mut journal, [9, 4, 8], 3);
        assert!(can_light(&chunks, &journal, fire));
        assert_eq!(
            heat_at(&chunks, &journal, &properties, fire, [10.5, 4.55, 8.5]),
            0.0
        );
        assert!(contacts_flame(fire, [8.5, 4.9, 8.5], [0.3, 0.9, 0.3]));
        assert!(!contacts_flame(fire, [9.3, 4.9, 8.5], [0.3, 0.9, 0.3]));
        assert!(!contacts_flame(fire, [8.5, 6.0, 8.5], [0.3, 0.9, 0.3]));
    }

    #[test]
    fn rain_requires_known_sky_and_saved_roofs_remain_shelter() {
        let fire = [8, 4, 8];
        let (mut chunks, mut journal) = fixture(fire);
        assert!(!rain_exposed(&chunks, &journal, fire));
        for y in 2..=4 {
            let section = terra_voxel::SectionPos { x: 0, y, z: 0 };
            chunks.loaded.insert(
                section,
                LoadedSection {
                    data: BlockSection::empty(section).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        assert!(rain_exposed(&chunks, &journal, fire));
        let roof = [8, 44, 8];
        journal.record(roof, 15).unwrap();
        assert!(!rain_exposed(&chunks, &journal, fire));
        journal.record(roof, 0).unwrap();
        assert!(rain_exposed(&chunks, &journal, fire));
        chunks
            .loaded
            .remove(&terra_voxel::SectionPos { x: 0, y: 3, z: 0 });
        assert!(!rain_exposed(&chunks, &journal, fire));
    }

    #[test]
    fn compact_bowl_shelters_its_fire_and_keeps_contact_heat_and_private_fuel_in_the_real_footprint()
     {
        let fire = [8, 4, 8];
        let (mut chunks, mut journal) = fixture(fire);
        for y in 2..=4 {
            let section = terra_voxel::SectionPos { x: 0, y, z: 0 };
            chunks.loaded.insert(
                section,
                LoadedSection {
                    data: BlockSection::empty(section).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        assert!(rain_exposed(&chunks, &journal, fire));
        let old_bowl = [8, 5, 8];
        put(&mut chunks, &mut journal, old_bowl, brewing::MATERIAL);
        assert!(
            !rain_exposed(&chunks, &journal, fire),
            "legacy stacked bowl shelters its fire"
        );
        put(&mut chunks, &mut journal, old_bowl, 0);
        assert!(rain_exposed(&chunks, &journal, fire));
        let state = journal.campfire_at(fire).unwrap().clone();
        let inventory = journal.container_at(fire).unwrap().clone();
        put(&mut chunks, &mut journal, fire, brewing::MATERIAL);
        let pose = BlockObjectState {
            facing: brewing::ATTACHED_FIRE,
            ..Default::default()
        };
        journal.set_object_state(fire, Some(pose)).unwrap();
        journal.set_cauldron(fire, Default::default()).unwrap();
        let mut attached = terra_core::campfire::AttachedCampfire { state, inventory };
        journal
            .commit_attached_campfire(fire, attached.clone())
            .unwrap();
        assert!(
            !rain_exposed(&chunks, &journal, fire),
            "the same-cell bowl is a real roof too"
        );
        assert!(heated_cauldron(&chunks, &journal, fire));
        let properties =
            BlockPropertiesCatalog::load(&terra_core::asset_directory().join("terra/blocks"))
                .unwrap();
        assert!(heat_at(&chunks, &journal, &properties, fire, [10.5, 4.27, 8.5]) > 0.0);
        assert!(contacts_hearth_flame(
            &journal,
            fire,
            [8.5, 4.25, 8.5],
            [0.01; 3]
        ));
        for point in [[8.5, 4.6, 8.5], [8.75, 4.25, 8.5]] {
            assert!(contacts_flame(fire, point, [0.01; 3]));
            assert!(!contacts_hearth_flame(&journal, fire, point, [0.01; 3]));
        }
        put(&mut chunks, &mut journal, [9, 4, 8], 3);
        assert_eq!(
            heat_at(&chunks, &journal, &properties, fire, [10.5, 4.27, 8.5]),
            0.0
        );
        let section = global_to_section(fire).0;
        let epoch = journal.mesh_epoch(section);
        let delta = journal.section_delta(section);
        attached.state.burn_remaining -= 0.25;
        journal
            .commit_attached_campfire(fire, attached.clone())
            .unwrap();
        assert_eq!(journal.mesh_epoch(section), epoch);
        assert_eq!(journal.section_delta(section), delta);
        assert!(crate::wrench::propose(&chunks, &journal, fire, false).is_err());
        assert_eq!(journal.object_state(fire), Some(pose));
        assert_eq!(journal.attached_campfire_at(fire), Some(&attached));
    }
}
