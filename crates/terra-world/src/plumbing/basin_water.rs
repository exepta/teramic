//! The finite reservoir is also the single source for visible and physical water.
use crate::{edits::EditJournal, streaming::ChunkStore, water::WaterColumn};
use terra_core::plumbing::{BASIN, BasinShape};

/// Shell floor and rim leave the same usable height for every connected module.
pub fn basin_water_level(shape: BasinShape, amount: u16) -> f64 {
    shape.origin[1] as f64
        + 0.13
        + (f64::from(shape.size[1]) - 0.21) * f64::from(amount.min(shape.capacity()))
            / f64::from(shape.capacity().max(1))
}

/// Exact interior occupancy, shared by swimming, breath, currents and optics.
/// Reservoir units stay in plumbing; this never publishes terrain FluidCells.
pub fn basin_water_column(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [f64; 3],
) -> Option<WaterColumn> {
    if position
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
    {
        return None;
    }
    let cell = position.map(|v| v.floor() as i64);
    if chunks.block_at(cell) != Some(BASIN) {
        return None;
    }
    let shape = journal.plumbing_at(cell)?.basin?;
    if !shape.valid() || !shape.contains(cell) {
        return None;
    }
    for axis in [0, 2] {
        let minimum = shape.origin[axis] as f64 + 0.16;
        let maximum = shape.origin[axis] as f64 + f64::from(shape.size[axis]) - 0.16;
        if position[axis] < minimum || position[axis] >= maximum {
            return None;
        }
    }
    let amount: u16 = shape.cells().try_fold(0_u16, |total, p| {
        let state = journal.plumbing_at(p)?;
        (state.basin == Some(shape)).then(|| total + u16::from(state.water.amount))
    })?;
    if amount == 0 {
        return None;
    }
    let level = basin_water_level(shape, amount);
    let bed = shape.origin[1] as f64 + 0.13;
    (position[1] >= bed && position[1] < level).then_some(WaterColumn {
        level,
        bed,
        depth: level - bed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        streaming::LoadedSection,
        water::{FluidMotion, fluid_motion_at},
    };
    use terra_voxel::{BlockSection, global_to_section};

    #[test]
    fn finite_basin_water_samples_match_level_and_respect_the_shell_after_replication() {
        let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
            sea_level: -100.0,
            ..Default::default()
        })
        .unwrap();
        let shape = BasinShape {
            origin: [-1, 192, -1],
            size: [2, 3, 2],
        };
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for cell in shape.cells() {
            let (section, local) = global_to_section(cell);
            let section = chunks
                .loaded
                .entry(section)
                .or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(section).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                });
            section.data.set_block(local, BASIN).unwrap();
            journal.record(cell, BASIN).unwrap();
            journal
                .set_object_state(cell, Some(Default::default()))
                .unwrap();
        }
        let revisions: Vec<_> = chunks
            .loaded
            .keys()
            .map(|&section| (section, journal.revision(section)))
            .collect();
        let game_revision = journal.gameplay().revision;
        journal
            .replace_confirmed_basins([(shape, shape.capacity()), (shape, shape.capacity())])
            .unwrap();
        let p = [-0.5, 193.5, -0.5];
        let water = chunks.water_column_at_point(&terrain, &journal, p).unwrap();
        assert_eq!(water.level, basin_water_level(shape, shape.capacity()));
        assert!((water.bed - 192.13).abs() < 1e-8);
        assert_eq!(
            fluid_motion_at(&terrain, &chunks, &journal, p),
            FluidMotion {
                velocity: [0.0; 3],
                immersion: 1.0
            }
        );
        for dry in [
            [-0.95, 193.5, -0.5],
            [-0.5, 192.10, -0.5],
            [-0.5, 194.95, -0.5],
            [1.05, 193.5, -0.5],
        ] {
            assert!(
                chunks
                    .water_column_at_point(&terrain, &journal, dry)
                    .is_none()
            );
        }
        let mut conflict = shape;
        conflict.size[0] = 1;
        assert!(
            journal
                .replace_confirmed_basins([(shape, shape.capacity()), (conflict, 0)])
                .is_err()
        );
        assert_eq!(
            chunks.water_column_at_point(&terrain, &journal, p),
            Some(water)
        );
        journal.replace_confirmed_basins([(shape, 0)]).unwrap();
        assert!(
            chunks
                .water_column_at_point(&terrain, &journal, p)
                .is_none()
        );
        assert_eq!(journal.gameplay().revision, game_revision);
        for (section, revision) in revisions {
            assert_eq!(journal.revision(section), revision);
        }
        assert!(shape.cells().all(|cell| journal.fluid(cell).is_none()));
    }
}
