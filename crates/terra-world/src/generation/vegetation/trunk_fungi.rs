use super::{Tree, mix, species::TreeSpecies};
use crate::props::{PropId, PropKind, WorldProp};
use terra_voxel::global_to_section;

fn ticket(origin: [i64; 3], seed: u64) -> u64 {
    mix(seed
        ^ (origin[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (origin[1] as u64).rotate_left(21)
        ^ (origin[2] as u64).wrapping_mul(0xbf58476d1ce4e5b9)
        ^ 0x5452_554e_4b46_554e)
}

pub(crate) fn attached_mushroom(tree: &Tree, seed: u64) -> Option<WorldProp> {
    if matches!(tree.species, TreeSpecies::Palm | TreeSpecies::Mushroom(_)) {
        return None;
    }
    let seed = ticket(tree.origin, seed);
    if seed % 100 >= 24 {
        return None;
    }
    let face = ((seed >> 8) & 3) as usize;
    let offset = [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]][face];
    let y = tree.origin[1] + 1 + ((seed >> 16) % 4) as i64;
    // Scan only the trunk's small local neighborhood, not every branch voxel.
    let mut candidates = Vec::new();
    for z in tree.origin[2] - 3..=tree.origin[2] + 3 {
        for x in tree.origin[0] - 3..=tree.origin[0] + 3 {
            let support = [x, y, z];
            let outside = std::array::from_fn(|axis| support[axis] + offset[axis]);
            if tree.wood_at(support).is_some()
                && tree.wood_at(outside).is_none()
                && tree.leaves_at(outside).is_none()
            {
                candidates.push(support);
            }
        }
    }
    let support = *candidates.get(((seed >> 24) as usize) % candidates.len().max(1))?;
    Some(WorldProp {
        id: PropId(mix(seed ^ 0x4641_4345_4655_4e47)),
        kind: PropKind::Mushroom,
        position: [
            support[0] as f64 + 0.5 + offset[0] as f64 * 0.64,
            support[1] as f64 + 0.20,
            support[2] as f64 + 0.5 + offset[2] as f64 * 0.64,
        ],
        rotation: face as f32 * std::f32::consts::FRAC_PI_2,
        scale: 0.30 + ((seed >> 32) & 255) as f32 / 255.0 * 0.12,
        variant: 128 + ((seed >> 40) % 3) as u8,
        section: global_to_section(support).0,
        removed: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tree_fungi_are_stable_wall_owned_and_about_twenty_four_percent() {
        let mut count = 0;
        for index in 0..1000 {
            let tree = Tree::with_species(
                [-index * 17, 30, index * 17],
                index as u64,
                [0.0; 4],
                TreeSpecies::Green,
            );
            let first = attached_mushroom(&tree, 77);
            assert_eq!(first, attached_mushroom(&tree, 77));
            if let Some(prop) = first {
                count += 1;
                assert!(prop.valid());
                assert!(prop.wall_attached());
                assert!(
                    tree.wood_at(crate::props::placement::support(&prop))
                        .is_some()
                );
                let saved = prop;
                let mut group = [prop];
                crate::props::placement::reflow(&mut group);
                assert_eq!(group[0], saved);
            }
        }
        assert!((200..=280).contains(&count), "observed {count}/1000");
    }

    #[test]
    fn fungi_match_headless_mesh_generation_and_stay_removed_after_reload() {
        use crate::{SectionGenerator, TerraGenerator, edits::EditJournal};
        use terra_core::{WorldConfig, WorldSeed};
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(42042),
            landscape_revision: 3,
            sea_level: 8.0,
            base_height: 12.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            biomes: false,
            tree_density: 1.0,
            ..Default::default()
        })
        .unwrap();
        let candidate = super::super::trees_in_region(&terrain, [-128, 0, -128], 256)
            .iter()
            .find_map(|tree| attached_mushroom(tree, terrain.config().seed.0))
            .unwrap();
        let generator = SectionGenerator::new(terrain);
        let a = generator
            .build(candidate.section, 0, &Default::default(), false)
            .unwrap();
        let b = generator
            .build(candidate.section, 0, &Default::default(), true)
            .unwrap();
        let fungi = |props: &[WorldProp]| {
            let mut result: Vec<_> = props
                .iter()
                .filter(|p| p.wall_attached())
                .copied()
                .collect();
            result.sort_by_key(|p| p.id);
            result
        };
        assert_eq!(fungi(&a.props), fungi(&b.props));
        assert!(a.props.iter().any(|p| p.id == candidate.id));
        let mut journal = EditJournal::default();
        journal
            .remove_prop(candidate.section, candidate.id)
            .unwrap();
        let mut replica = EditJournal::default();
        replica
            .import_delta(journal.section_delta(candidate.section))
            .unwrap();
        let mut reloaded = generator
            .build(candidate.section, 0, &Default::default(), false)
            .unwrap();
        replica.apply_prop_removals(candidate.section, &mut reloaded.props);
        assert!(
            reloaded
                .props
                .iter()
                .find(|p| p.id == candidate.id)
                .unwrap()
                .removed
        );
        let support = crate::props::placement::support(&candidate);
        journal.record(support, 0).unwrap();
        let rebuilt = generator
            .build(
                candidate.section,
                journal.revision(candidate.section),
                &journal.snapshot(candidate.section).unwrap(),
                false,
            )
            .unwrap();
        assert!(rebuilt.props.iter().all(|p| p.id != candidate.id));
    }
}
