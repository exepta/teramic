//! Snapshot floating plant poses for rendering with the world’s picking geometry.
use super::*;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

pub(super) use terra_world::water::floating_plants::Attachment;
use terra_world::water::floating_plants::is_floating_plant;

impl ModelSnapshot {
    pub(crate) fn refresh_water_plants(
        &mut self,
        section: SectionPos,
        chunks: &ChunkStore,
        journal: &EditJournal,
    ) {
        self.water_plants.clear();
        if self.standalone {
            return;
        }
        let Ok(origin) = section.origin() else {
            return;
        };
        for model in &self.models {
            if !is_floating_plant(model.material) {
                continue;
            }
            let cell = std::array::from_fn(|i| origin[i] + i64::from(model.cell[i]));
            if let Some(attachment) = Attachment::at(chunks, journal, cell) {
                self.water_plants.insert(model.cell, attachment);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
    use terra_world::{streaming::LoadedSection, water::FluidCell};

    #[test]
    fn actual_lotus_and_lily_glbs_follow_water_across_negative_section_seams_but_keep_their_cells()
    {
        let owner = [-16, 0, -16];
        let below = [-16, -1, -16];
        let section = terra_voxel::global_to_section(owner).0;
        for (material, path) in [
            (319, "terra/blocks/models/lotus_flower.glb"),
            (320, "terra/blocks/models/lily_pad.glb"),
            (490, "terra/blocks/models/pond_blossoms.glb"),
        ] {
            let mut templates = TemplateSet::default();
            templates.blocks.insert(
                material,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.0,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [0; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model: crate::blocks::test_static_scene(path, material),
                },
            );
            for ground in [0, 3, 50, 553] {
                let mut chunks = ChunkStore::default();
                let mut data = BlockSection::empty(section).unwrap();
                data.set_block([0; 3], material).unwrap();
                let below_section = terra_voxel::global_to_section(below).0;
                let mut below_data = BlockSection::empty(below_section).unwrap();
                below_data.set_block([0, 15, 0], ground).unwrap();
                for data in [data.clone(), below_data] {
                    chunks.loaded.insert(
                        data.position(),
                        LoadedSection {
                            data,
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: None,
                            props: Vec::new(),
                        },
                    );
                }
                let mut journal = EditJournal::default();
                journal
                    .set_fluid(
                        below,
                        FluidCell {
                            amount: 3,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                let mut snapshot =
                    capture(&data, &templates, &journal, |p| chunks.block_at(p)).unwrap();
                snapshot.refresh_water_plants(section, &chunks, &journal);
                let options = RenderOptions {
                    water_level: None,
                    detail_level: super::super::super::DetailLevel::Near,
                    terrain: None,
                    exposure: None,
                    conditions: Default::default(),
                    tints: Default::default(),
                };
                let dry = compose(
                    &BlockMesh::default(),
                    &snapshot,
                    &templates,
                    section,
                    &options,
                )
                .unwrap();
                let expected_offset = if ground == 3 { 0.0 } else { -0.625 };
                let source_min = templates.blocks[&material]
                    .model
                    .iter()
                    .flat_map(|p| &p.positions)
                    .map(|p| p[1])
                    .fold(f32::INFINITY, f32::min);
                let rendered_min = dry
                    .iter()
                    .flat_map(|b| &b.positions)
                    .map(|p| p[1])
                    .fold(f32::INFINITY, f32::min);
                assert!((rendered_min - source_min - expected_offset).abs() < 0.00001);
                let bounds = terra_world::water::floating_plants::PlantSurface::at(
                    &chunks, &journal, owner, material,
                )
                .unwrap()
                .bounds();
                for axis in 0..3 {
                    let values = || {
                        dry.iter()
                            .flat_map(|batch| &batch.positions)
                            .map(|v| f64::from(v[axis]) + owner[axis] as f64)
                    };
                    let low = values().fold(f64::INFINITY, f64::min);
                    let high = values().fold(f64::NEG_INFINITY, f64::max);
                    assert!((low - bounds.minimum[axis]).abs() < 1e-5);
                    assert!((high - bounds.maximum[axis]).abs() < 1e-5);
                }
                assert_eq!(data.block([0; 3]).unwrap(), material);
                journal.set_fluid(below, FluidCell::default()).unwrap();
                if ground != 3 {
                    assert!(snapshot.water_contacts_changed(section, &journal));
                    snapshot.refresh_water_contacts(section, &journal);
                    assert_eq!(snapshot.water_plants[&[0; 3]].offset, 0.0);
                }
            }
        }
    }
}
