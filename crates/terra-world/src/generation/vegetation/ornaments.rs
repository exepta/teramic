//! Small authored lanterns hang from genuine generated branches, never from air.
use super::{Tree, mix, species::TreeSpecies};

impl Tree {
    pub(super) fn choose_blossom_lantern(&self) -> Option<[i64; 3]> {
        if !matches!(
            self.species,
            TreeSpecies::SpiritBlossom(_) | TreeSpecies::Bloodwood
        ) {
            return None;
        }
        let hash = mix(self.origin[0] as u64 ^ (self.origin[2] as u64).rotate_left(31));
        if !hash.is_multiple_of(3) {
            return None;
        }
        for turn in 0..4 {
            let direction = [[1, 0], [0, 1], [-1, 0], [0, -1]][((hash as usize) + turn) % 4];
            for reach in 3..=7 {
                for height in 4..self.bounds.max[1] - 1 {
                    let cell = [
                        self.origin[0] + direction[0] * reach,
                        self.origin[1] + i64::from(height),
                        self.origin[2] + direction[1] * reach,
                    ];
                    if self.wood_at(cell).is_none()
                        && self.leaves_at(cell).is_none()
                        && self.wood_at([cell[0], cell[1] + 1, cell[2]]).is_some()
                    {
                        return Some(cell);
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::MaterialId;

    #[test]
    fn all_blossom_crowns_have_real_logs_and_lanterns_attach_below_branches() {
        let mut lanterns = 0;
        for seed in 0..90 {
            let tree = Tree::with_species(
                [-17 * seed, 40, -32],
                seed as u64,
                [0.0; 4],
                TreeSpecies::SpiritBlossom((seed % 3) as u8),
            );
            assert_eq!(tree.wood_material, MaterialId::SpiritBlossomLog);
            assert!((310..=312).contains(&(tree.species.leaves() as u16)));
            if let Some(cell) = tree.lantern {
                lanterns += 1;
                assert_eq!(
                    tree.wood_at([cell[0], cell[1] + 1, cell[2]]),
                    Some(MaterialId::SpiritBlossomLog)
                );
                assert_eq!(
                    tree.leaves_at(cell),
                    Some(MaterialId::HangingBlossomLantern)
                );
                let start = cell.map(|v| v - 1);
                let mut blocks = vec![0; 27];
                tree.stamp_leaves(start, 3, &mut blocks);
                assert_eq!(blocks[13], MaterialId::HangingBlossomLantern as u16);
            }
        }
        assert!(
            lanterns > 4,
            "lanterns must actually be generated: {lanterns}"
        );
    }
}
