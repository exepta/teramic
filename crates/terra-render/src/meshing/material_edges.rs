//! Visual-only contact tinting. A small halo is owned by one near-detail job,
//! then discarded; it never changes block IDs, collision or saved terrain.
use super::builder::MeshBatch;
use crate::blocks::TemplateSet;
use terra_voxel::BlockSection;

pub(super) struct Neighborhood(Box<[u16]>);
impl Neighborhood {
    pub fn capture(
        section: &BlockSection,
        mut neighbor: impl FnMut([i64; 3]) -> Option<u16>,
    ) -> Self {
        let mut cells = vec![0; 18 * 18 * 18];
        let Ok(origin) = section.position().origin() else {
            return Self(cells.into_boxed_slice());
        };
        for z in -1..=16 {
            for y in -1..=16 {
                for x in -1..=16 {
                    let p = [x, y, z];
                    let id = if p.iter().all(|v| (0..16).contains(v)) {
                        section.blocks()[x as usize + 16 * (y as usize + 16 * z as usize)]
                    } else {
                        neighbor(std::array::from_fn(|i| origin[i] + i64::from(p[i]))).unwrap_or(0)
                    };
                    cells[(x + 1) as usize + 18 * ((y + 1) as usize + 18 * (z + 1) as usize)] = id;
                }
            }
        }
        Self(cells.into_boxed_slice())
    }
    fn at(&self, p: [i32; 3]) -> u16 {
        if p.iter().any(|v| !(-1..=16).contains(v)) {
            return 0;
        }
        self.0[(p[0] + 1) as usize + 18 * ((p[1] + 1) as usize + 18 * (p[2] + 1) as usize)]
    }
    pub fn apply(&self, batches: &mut [MeshBatch], templates: &TemplateSet) {
        for batch in batches {
            if ![1, 2, 3, 4, 5].into_iter().any(|id| {
                templates
                    .blocks
                    .get(&id)
                    .is_some_and(|b| b.face_materials.contains(&batch.material))
            }) {
                continue;
            }
            let source_slots: [bool; 6] = std::array::from_fn(|id| {
                templates
                    .blocks
                    .get(&(id as u16))
                    .is_some_and(|b| b.face_materials.contains(&batch.material))
            });
            batch.colors.resize(batch.positions.len(), [1.0; 4]);
            for ((position, normal), color) in batch
                .positions
                .iter()
                .zip(&batch.normals)
                .zip(&mut batch.colors)
            {
                let initial: [i32; 3] =
                    std::array::from_fn(|i| (position[i] - normal[i] * 0.002).floor() as i32);
                let Some(owner) = (0..8).find_map(|mask| {
                    let mut p = initial;
                    for axis in 0..3 {
                        if mask & (1 << axis) != 0 {
                            if normal[axis].abs() > 0.5
                                || (position[axis] - position[axis].round()).abs() > 0.003
                            {
                                return None;
                            }
                            p[axis] -= 1;
                        }
                    }
                    let id = self.at(p) as usize;
                    (p.iter().all(|v| (0..16).contains(v)) && id < 6 && source_slots[id])
                        .then_some(p)
                }) else {
                    continue;
                };
                let source = self.at(owner);
                if !matches!(source, 1..=5) {
                    continue;
                }
                let base = tone(source);
                for axis in 0..3 {
                    if normal[axis].abs() > 0.5 {
                        continue;
                    }
                    let local = position[axis] - owner[axis] as f32;
                    for (direction, distance) in [(-1, local), (1, 1.0 - local)] {
                        if !(0.0..0.24).contains(&distance) {
                            continue;
                        }
                        let mut adjacent = owner;
                        adjacent[axis] += direction;
                        let other = self.at(adjacent);
                        if !matches!(
                            (source.min(other), source.max(other)),
                            (1, 2) | (2, 3) | (3, 4) | (3, 5)
                        ) {
                            continue;
                        }
                        let t = (1.0 - distance / 0.24).powi(2) * 0.18;
                        let target = tone(other);
                        for i in 0..3 {
                            color[i] *= 1.0 + (target[i] / base[i] - 1.0).clamp(-0.6, 0.6) * t;
                        }
                    }
                }
            }
        }
    }
}
fn tone(material: u16) -> [f32; 3] {
    match material {
        1 => [0.30, 0.52, 0.19],
        2 => [0.38, 0.25, 0.14],
        3 => [0.46, 0.47, 0.48],
        4 => [0.78, 0.65, 0.39],
        5 => [0.91, 0.95, 0.98],
        _ => [1.0; 3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
    use terra_voxel::SectionPos;

    fn templates() -> TemplateSet {
        TemplateSet {
            blocks: [(
                3,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::Cube,
                    bevel_width: 0.0,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [3; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model: Vec::new(),
                },
            )]
            .into(),
            ..Default::default()
        }
    }

    #[test]
    fn contact_tints_read_the_real_negative_section_halo_and_fade_inward() {
        let mut section = BlockSection::empty(SectionPos::new(-1, -1, -1)).unwrap();
        section.set_block([15, 15, 15], 3).unwrap();
        let neighborhood =
            Neighborhood::capture(&section, |global| Some(if global[0] == 0 { 4 } else { 0 }));
        assert_eq!(neighborhood.at([15, 15, 15]), 3);
        assert_eq!(neighborhood.at([16, 15, 15]), 4);
        let mut batches = [MeshBatch {
            material: 3,
            positions: vec![[15.99, 16.0, 15.5], [15.5, 16.0, 15.5]],
            normals: vec![[0.0, 1.0, 0.0]; 2],
            ..Default::default()
        }];
        neighborhood.apply(&mut batches, &templates());
        assert_ne!(batches[0].colors[0], [1.0; 4]);
        assert_eq!(batches[0].colors[1], [1.0; 4]);
        assert_eq!(section.block([15, 15, 15]).unwrap(), 3);
        assert_eq!(batches[0].positions.len(), 2);
    }

    #[test]
    fn air_and_transparent_neighbors_do_not_create_contact_stains() {
        let mut section = BlockSection::empty(SectionPos::new(0, 0, 0)).unwrap();
        section.set_block([0, 0, 0], 3).unwrap();
        for other in [0, 7, 14, 15, 27] {
            let neighborhood = Neighborhood::capture(&section, |_| Some(other));
            let mut batches = [MeshBatch {
                material: 3,
                positions: vec![[0.01, 1.0, 0.5]],
                normals: vec![[0.0, 1.0, 0.0]],
                ..Default::default()
            }];
            neighborhood.apply(&mut batches, &templates());
            assert_eq!(batches[0].colors, vec![[1.0; 4]]);
        }
    }
}
