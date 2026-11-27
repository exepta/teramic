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
        for (row, source) in section.blocks().chunks_exact(16).enumerate() {
            let start = 1 + 18 * (1 + row % 16 + 18 * (1 + row / 16));
            cells[start..start + 16].copy_from_slice(source);
        }
        // Owners are inside the section. A contact crosses exactly one face,
        // and only terrain materials can acquire tint from the adjacent cell.
        for axis in 0..3 {
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            for (side, step) in [(0, -1), (15, 1)] {
                let mut local = [0; 3];
                local[axis] = side;
                for j in 0..16 {
                    local[v] = j;
                    for i in 0..16 {
                        local[u] = i;
                        let [x, y, z] = local.map(|v| (v + 1) as usize);
                        if !(1..=5).contains(&cells[x + 18 * (y + 18 * z)]) {
                            continue;
                        }
                        let mut p = local;
                        p[axis] += step;
                        let [x, y, z] = p.map(|v| (v + 1) as usize);
                        cells[x + 18 * (y + 18 * z)] =
                            neighbor(std::array::from_fn(|a| origin[a] + i64::from(p[a])))
                                .unwrap_or(0);
                    }
                }
            }
        }
        Self(cells.into_boxed_slice())
    }
    #[cfg(test)]
    fn at(&self, p: [i32; 3]) -> u16 {
        if p.iter().any(|v| !(-1..=16).contains(v)) {
            return 0;
        }
        self.0[(p[0] + 1) as usize + 18 * ((p[1] + 1) as usize + 18 * (p[2] + 1) as usize)]
    }
    // Low three bits select the owner. Each following three-bit field is the
    // contact material on one of West, East, Down, Up, North and South.
    fn contacts(&self) -> (Vec<u32>, u8) {
        let mut cells = vec![0; 4096];
        let mut sources = 0;
        for z in 0..16 {
            for y in 0..16 {
                let row = 1 + 18 * (y + 1 + 18 * (z + 1));
                let output = 16 * (y + 16 * z);
                for (x, &owner) in self.0[row..row + 16].iter().enumerate() {
                    let mut packed = u32::from(owner.min(6));
                    let partners: u8 = match owner {
                        1 => 1 << 2,
                        2 => (1 << 1) | (1 << 3),
                        3 => (1 << 2) | (1 << 4) | (1 << 5),
                        4 | 5 => 1 << 3,
                        _ => 0,
                    };
                    if partners != 0 {
                        let i = row + x;
                        for (side, index) in [i - 1, i + 1, i - 18, i + 18, i - 324, i + 324]
                            .into_iter()
                            .enumerate()
                        {
                            let other = self.0[index];
                            if other <= 5 && partners & (1 << other) != 0 {
                                packed |= u32::from(other) << (3 + 3 * side);
                            }
                        }
                        if packed > 7 {
                            sources |= 1 << owner;
                        }
                    }
                    cells[output + x] = packed;
                }
            }
        }
        (cells, sources)
    }

    pub fn apply(&self, batches: &mut [MeshBatch], templates: &TemplateSet) {
        let (contacts, sources) = self.contacts();
        for batch in batches {
            let mut source_slots = 0u8;
            for id in 0..=5 {
                if templates
                    .blocks
                    .get(&id)
                    .is_some_and(|b| b.face_materials.contains(&batch.material))
                {
                    source_slots |= 1 << id;
                }
            }
            if source_slots & 0b111110 == 0 {
                continue;
            }
            batch.colors.resize(batch.positions.len(), [1.0; 4]);
            if sources & source_slots == 0 {
                continue;
            }
            for ((position, normal), color) in batch
                .positions
                .iter()
                .zip(&batch.normals)
                .zip(&mut batch.colors)
            {
                let initial =
                    std::array::from_fn(|i| (position[i] - normal[i] * 0.002).floor() as i32);
                let matching = |p: [i32; 3]| {
                    // A single unsigned bound rejects any negative or oversized coordinate.
                    if (p[0] | p[1] | p[2]) as u32 >= 16 {
                        return None;
                    }
                    let code = contacts[(p[0] + 16 * (p[1] + 16 * p[2])) as usize];
                    (source_slots & (1 << (code & 7)) != 0).then_some((p, code))
                };
                let owner = matching(initial).or_else(|| {
                    let mut tangent_edges = 0;
                    for axis in 0..3 {
                        if normal[axis].abs() <= 0.5
                            && (position[axis] - position[axis].round()).abs() <= 0.003
                        {
                            tangent_edges |= 1 << axis;
                        }
                    }
                    (1..=tangent_edges)
                        .filter(|bits| bits & !tangent_edges == 0)
                        .find_map(|bits| {
                            matching(std::array::from_fn(|i| initial[i] - ((bits >> i) & 1)))
                        })
                });
                let Some((owner, code)) = owner else { continue };
                if code <= 7 {
                    continue;
                }
                let base = tone((code & 7) as u16);
                for axis in 0..3 {
                    let sides = (code >> (3 + 6 * axis)) & 63;
                    if sides == 0 || normal[axis].abs() > 0.5 {
                        continue;
                    }
                    let local = position[axis] - owner[axis] as f32;
                    for (other, distance) in [(sides & 7, local), (sides >> 3, 1.0 - local)] {
                        if other == 0 || !(0.0..0.24).contains(&distance) {
                            continue;
                        }
                        let t = (1.0 - distance / 0.24).powi(2) * 0.18;
                        let target = tone(other as u16);
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
    #[test]
    fn shared_slot_keeps_the_first_owner_even_when_only_the_other_owner_has_contacts() {
        let mut section = BlockSection::empty(SectionPos::default()).unwrap();
        section.set_block([0, 0, 0], 1).unwrap();
        section.set_block([1, 0, 0], 5).unwrap();
        let halo = Neighborhood::capture(&section, |p| (p[0] == 0 && p[2] == -1).then_some(2));
        let mut templates = templates();
        let shared = templates.blocks[&3].clone();
        templates.blocks.insert(1, shared.clone());
        templates.blocks.insert(5, shared);
        let original = [0.6, 0.7, 0.8, 0.4];
        let mut batch = [MeshBatch {
            material: 3,
            positions: vec![[1.0, 1.0, 0.1], [0.99, 1.0, 0.1]],
            normals: vec![[0., 1., 0.]; 2],
            colors: vec![original; 2],
            ..Default::default()
        }];
        halo.apply(&mut batch, &templates);
        assert_eq!(batch[0].colors[0], original);
        assert_ne!(batch[0].colors[1], original);
        assert_eq!(batch[0].colors[1][3], original[3]);
    }
}
