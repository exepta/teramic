//! Visual-only waterline attachment for lily pads and lotus models. Logical cells,
//! inventories and saved block coordinates remain untouched.
use super::*;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

#[derive(Clone, Copy)]
pub(super) struct Attachment {
    natural_level: Option<f64>,
    pub offset: f32,
}

impl Attachment {
    pub(super) fn offset(&self, cell: [i64; 3], journal: &EditJournal) -> f32 {
        let below = [cell[0], cell[1] - 1, cell[2]];
        let level = journal.fluid(below).map_or(self.natural_level, |fluid| {
            (fluid.amount > 0).then_some(below[1] as f64 + fluid.height())
        });
        waterline_offset(cell[1], level)
    }
}

fn waterline_offset(y: i64, level: Option<f64>) -> f32 {
    level
        .filter(|level| level.is_finite() && level.ceil() == y as f64)
        .map_or(0.0, |level| (level - y as f64) as f32)
}

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
            if !matches!(model.material, 319 | 320) {
                continue;
            }
            let cell = std::array::from_fn(|i| origin[i] + i64::from(model.cell[i]));
            let below = [cell[0], cell[1] - 1, cell[2]];
            // A player-authored model on stone, earth or furniture stays there.
            // Unknown neighboring sections are not guessed to contain water.
            if !matches!(
                chunks
                    .block_at(below)
                    .map(|id| journal.block(below).unwrap_or(id)),
                Some(0 | 14 | 27)
            ) {
                continue;
            }
            let natural_level = chunks
                .surface_column_at(cell[0], cell[2])
                .and_then(|column| column.water_columns().at(below[1]))
                .filter(|column| below[1] as f64 >= column.bed && (below[1] as f64) < column.level)
                .map(|column| {
                    // The shared fluid mesh quantizes natural reservoirs to eighths.
                    let capacity = f64::from(terra_world::water::FLUID_CAPACITY);
                    below[1] as f64
                        + ((column.level - below[1] as f64).min(1.0) * capacity).ceil() / capacity
                });
            let mut attachment = Attachment {
                natural_level,
                offset: 0.0,
            };
            attachment.offset = attachment.offset(cell, journal);
            self.water_plants.insert(model.cell, attachment);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
    use terra_world::{streaming::LoadedSection, water::FluidCell};

    #[test]
    fn waterline_offsets_handle_negative_levels_without_moving_dry_or_submerged_models() {
        assert_eq!(waterline_offset(16, Some(15.375)), -0.625);
        assert_eq!(waterline_offset(-16, Some(-16.625)), -0.625);
        assert_eq!(waterline_offset(16, Some(16.0)), 0.0);
        for level in [None, Some(14.9), Some(16.1), Some(f64::NAN)] {
            assert_eq!(waterline_offset(16, level), 0.0);
        }
    }

    #[test]
    fn actual_lotus_and_lily_glbs_follow_water_across_negative_section_seams_but_keep_their_cells()
    {
        let owner = [-16, 0, -16];
        let below = [-16, -1, -16];
        let section = terra_voxel::global_to_section(owner).0;
        for (material, path) in [
            (319, "terra/blocks/models/lotus_flower.glb"),
            (320, "terra/blocks/models/lily_pad.glb"),
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
            for ground in [0, 3] {
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
                let expected_offset = if ground == 0 { -0.625 } else { 0.0 };
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
                assert_eq!(data.block([0; 3]).unwrap(), material);
                journal.set_fluid(below, FluidCell::default()).unwrap();
                if ground == 0 {
                    assert!(snapshot.water_contacts_changed(section, &journal));
                    snapshot.refresh_water_contacts(section, &journal);
                    assert_eq!(snapshot.water_plants[&[0; 3]].offset, 0.0);
                }
            }
        }
    }
}
