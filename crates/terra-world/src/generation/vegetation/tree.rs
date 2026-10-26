use super::{
    DIRECTIONS,
    geometry::{Bounds, Crown, WoodSegment, visit_voxels},
    random,
    species::TreeSpecies,
};
use terra_voxel::MaterialId;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tree {
    pub origin: [i64; 3],
    pub species: TreeSpecies,
    pub wood_material: MaterialId,
    pub form: TreeForm,
    pub bounds: Bounds,
    pub wood: Vec<WoodSegment>,
    pub crowns: Vec<Crown>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TreeForm {
    Spreading,
    Upright,
    Tiered,
}

impl TreeForm {
    fn dimensions(self) -> (f32, f32) {
        match self {
            Self::Spreading => (1.12, 0.82),
            Self::Upright => (0.72, 1.16),
            Self::Tiered => (0.95, 1.0),
        }
    }
}

impl Tree {
    #[cfg(feature = "bevy")]
    pub(crate) fn felling_shape(&self) -> Option<crate::tree_felling::Shape> {
        use std::collections::HashSet;
        let start = std::array::from_fn(|i| self.origin[i] + i64::from(self.bounds.min[i]));
        let size = (0..3)
            .map(|i| (self.bounds.max[i] - self.bounds.min[i]) as usize)
            .max()?;
        let global =
            |point: [f32; 3]| std::array::from_fn(|i| self.origin[i] + point[i].floor() as i64);
        let mut wood = HashSet::new();
        for segment in &self.wood {
            visit_voxels(segment.bounds, self.origin, start, size, |point, _| {
                if segment.contains(point) {
                    wood.insert(global(point));
                }
            });
            if wood.len() > crate::tree_felling::MAX_WOOD {
                return None;
            }
        }
        let mut leaves = HashSet::new();
        for crown in &self.crowns {
            visit_voxels(crown.bounds, self.origin, start, size, |point, _| {
                let cell = global(point);
                if !wood.contains(&cell) && crown.contains(point) {
                    leaves.insert(cell);
                }
            });
            if leaves.len() > crate::tree_felling::MAX_LEAVES {
                return None;
            }
        }
        let mut wood: Vec<_> = wood.into_iter().collect();
        wood.sort_unstable_by_key(|p| (std::cmp::Reverse(p[1]), p[2], p[0]));
        let mut leaves: Vec<_> = leaves.into_iter().collect();
        leaves.sort_unstable_by_key(|p| {
            (
                super::mix(
                    p[0] as u64 ^ (p[1] as u64).rotate_left(21) ^ (p[2] as u64).rotate_left(42),
                ),
                *p,
            )
        });
        Some(crate::tree_felling::Shape {
            origin: self.origin,
            wood_material: self.wood_material as u16,
            leaves_material: self.species.leaves() as u16,
            wood,
            leaves,
        })
    }

    #[cfg(test)]
    pub fn new(origin: [i64; 3], seed: u64, roots: [f32; 4]) -> Self {
        Self::with_species(origin, seed, roots, TreeSpecies::from_seed(seed))
    }
    pub fn with_species(
        origin: [i64; 3],
        seed: u64,
        roots: [f32; 4],
        species: TreeSpecies,
    ) -> Self {
        match species {
            TreeSpecies::Palm => return Self::palm(origin, seed),
            TreeSpecies::Spruce => return Self::spruce(origin, seed),
            TreeSpecies::Mushroom(variant) => return Self::mushroom(origin, seed, variant),
            _ => {}
        }
        let form = match (seed >> 24) % 3 {
            _ if matches!(species, TreeSpecies::Cherry(_)) => TreeForm::Spreading,
            0 => TreeForm::Spreading,
            1 => TreeForm::Upright,
            _ => TreeForm::Tiered,
        };
        let (spread, crown_height) = form.dimensions();
        let (low, high) = species.height_range();
        let height = low + (high - low) * random(seed, 1);
        let lean = [(random(seed, 2) - 0.5) * 2.6, (random(seed, 3) - 0.5) * 2.6];
        let bend = [(random(seed, 4) - 0.5) * 1.8, (random(seed, 5) - 0.5) * 1.8];
        let base_radius = 1.35 + random(seed, 6) * 0.5;
        let trunk = |level: f32| {
            [
                0.5 + lean[0] * level + bend[0] * level * (1.0 - level) * 3.0,
                height * level,
                0.5 + lean[1] * level + bend[1] * level * (1.0 - level) * 3.0,
            ]
        };
        // Radii >= 0.9 also connect diagonal segments through voxel face neighbors.
        let mut wood = Vec::with_capacity(32);
        let mut crowns = Vec::with_capacity(16);
        for i in 0..4 {
            let a = i as f32 * 0.25;
            let b = (i + 1) as f32 * 0.25;
            wood.push(WoodSegment::new(
                trunk(a),
                trunk(b),
                [
                    base_radius + (0.95 - base_radius) * a,
                    base_radius + (0.95 - base_radius) * b,
                ],
            ));
        }
        for (i, &ground) in roots.iter().enumerate() {
            let direction = DIRECTIONS[i * 2];
            wood.push(WoodSegment::new(
                [0.5, 1.3, 0.5],
                [
                    0.5 + direction[0] * 4.0,
                    ground + 0.25,
                    0.5 + direction[1] * 4.0,
                ],
                [1.1, 0.9],
            ));
        }
        let count = 6 + (seed % 3) as usize;
        for i in 0..count {
            let channel = 10 + i as u64 * 8;
            let direction = DIRECTIONS[(((seed >> 8) % 8) as usize + i * 3) % 8];
            let angle = (random(seed, channel + 6) - 0.5) * 0.7;
            let (sin, cos) = angle.sin_cos();
            let direction = [
                direction[0] * cos - direction[1] * sin,
                direction[0] * sin + direction[1] * cos,
            ];
            let level = match form {
                TreeForm::Spreading => 0.48 + random(seed, channel) * 0.24,
                TreeForm::Upright => 0.38 + i as f32 / count as f32 * 0.48,
                TreeForm::Tiered => 0.40 + i as f32 / count as f32 * 0.42,
            };
            let start = trunk(level);
            let length = (4.0 + random(seed, channel + 1) * 3.2) * spread;
            let rise = 1.8 + random(seed, channel + 2) * 2.0;
            let middle = [
                start[0] + direction[0] * length * 0.55,
                start[1] + rise * 0.20,
                start[2] + direction[1] * length * 0.55,
            ];
            let tip = [
                start[0] + direction[0] * length,
                start[1] + rise,
                start[2] + direction[1] * length,
            ];
            wood.push(WoodSegment::new(start, middle, [1.15, 1.0]));
            wood.push(WoodSegment::new(middle, tip, [1.0, 0.9]));
            crowns.push(Crown::new(
                [tip[0], tip[1] + 0.7, tip[2]],
                [
                    (2.5 + random(seed, channel + 3) * 1.2) * spread,
                    (1.9 + random(seed, channel + 4) * 0.7) * crown_height,
                    (2.5 + random(seed, channel + 5) * 1.2) * spread,
                ],
            ));
            if i.is_multiple_of(2) {
                let fork = [
                    middle[0] - direction[1] * 2.4,
                    middle[1] + 1.6,
                    middle[2] + direction[0] * 2.4,
                ];
                wood.push(WoodSegment::new(middle, fork, [1.0, 0.9]));
                crowns.push(Crown::new(
                    [fork[0], fork[1] + 0.6, fork[2]],
                    [2.3 * spread, 1.9 * crown_height, 2.5 * spread],
                ));
            }
        }
        let top = trunk(1.0);
        crowns.push(Crown::new(
            [top[0], top[1] + 0.7, top[2]],
            [3.2 * spread, 2.6 * crown_height, 3.0 * spread],
        ));
        Self::from_shapes(origin, species, form, wood, crowns)
    }

    pub(super) fn from_shapes(
        origin: [i64; 3],
        species: TreeSpecies,
        form: TreeForm,
        wood: Vec<WoodSegment>,
        crowns: Vec<Crown>,
    ) -> Self {
        let mut bounds = Bounds {
            min: [0; 3],
            max: [1; 3],
        };
        for segment in &wood {
            bounds = bounds.union(segment.bounds);
        }
        for crown in &crowns {
            bounds = bounds.union(crown.bounds);
        }
        Self {
            origin,
            species,
            wood_material: species.wood(),
            form,
            bounds,
            wood,
            crowns,
        }
    }

    fn local(&self, global: [i64; 3]) -> [f32; 3] {
        std::array::from_fn(|i| (global[i] - self.origin[i]) as f32 + 0.5)
    }
    pub fn wood_at(&self, global: [i64; 3]) -> Option<MaterialId> {
        let point = self.local(global);
        (self.bounds.contains(point) && self.wood.iter().any(|s| s.contains(point)))
            .then_some(self.wood_material)
    }
    pub fn leaves_at(&self, global: [i64; 3]) -> Option<MaterialId> {
        let point = self.local(global);
        (self.bounds.contains(point) && self.crowns.iter().any(|c| c.contains(point)))
            .then_some(self.species.leaves())
    }
    pub fn stamp_wood(&self, start: [i64; 3], size: usize, blocks: &mut [u16]) {
        for segment in &self.wood {
            visit_voxels(segment.bounds, self.origin, start, size, |point, index| {
                if blocks[index] == 0 && segment.contains(point) {
                    blocks[index] = self.wood_material as u16;
                }
            });
        }
    }
    pub fn stamp_leaves(&self, start: [i64; 3], size: usize, blocks: &mut [u16]) {
        for crown in &self.crowns {
            visit_voxels(crown.bounds, self.origin, start, size, |point, index| {
                if blocks[index] == 0 && crown.contains(point) {
                    blocks[index] = self.species.leaves() as u16;
                }
            });
        }
    }
}
