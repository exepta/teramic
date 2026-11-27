use super::MeshBatch;
use bevy::platform::collections::{HashMap, hash_map::Entry};
use std::collections::HashSet;

pub(crate) const PROFILE_ERROR_METRES: f32 = 0.027;

pub(crate) fn append_profiles(
    batches: &mut Vec<MeshBatch>,
    slots: &HashSet<usize>,
    section: terra_voxel::SectionPos,
    folded: crate::meshing::lod::FoldedLeaves,
) {
    let Ok(origin) = section.origin() else { return };
    let mut proxies = std::collections::BTreeMap::new();
    for batch in batches.iter_mut() {
        if !slots.contains(&batch.material)
            || batch.leaf_shadow_sides.is_empty()
            || batch.leaf_shadow_sides.len() != batch.foliage_owners.len()
            || !batch.rooted_wind.is_empty()
            || !batch.surface_water.is_empty()
            || batch.positions.len() != batch.packed_colors.len()
        {
            continue;
        }
        let mut end = 0;
        let complete = batch.foliage_owners.iter().all(|owner| {
            let range = &owner.vertices;
            let start = end;
            end = range.end;
            owner.material == 7
                && range.start == start
                && batch
                    .packed_colors
                    .get(range.clone())
                    .is_some_and(|colors| {
                        !colors.is_empty() && colors.iter().all(|c| c[3] == colors[0][3])
                    })
        }) && end == batch.positions.len();
        if !complete {
            continue;
        }
        for (owner, &side) in batch.foliage_owners.iter().zip(&batch.leaf_shadow_sides) {
            let cell = &owner.cell;
            let global = std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis]));
            let variant = crate::blocks::sculpt::variant(global, 7);
            let alpha = batch.packed_colors[owner.vertices.start][3];
            let depth = proxies
                .entry((batch.material, cell.map(|v| v / 4)))
                .or_insert_with(|| MeshBatch {
                    material: batch.material,
                    paired_leaf_shadow: true,
                    profile_leaf_shadow: true,
                    shadow_only: true,
                    ..Default::default()
                });
            for card in
                crate::blocks::sculpt::shadow_cards(usize::from(side), variant, !folded.at(*cell))
            {
                let first = depth.positions.len() as u32;
                let bytes = card.stations.to_le_bytes();
                depth.positions.extend(
                    card.positions
                        .map(|p| std::array::from_fn(|i| p[i] + cell[i] as f32)),
                );
                depth
                    .packed_colors
                    .extend([[bytes[0], bytes[1], 0, alpha]; 6]);
                depth
                    .uvs
                    .extend([[0., 1.], [0., 0.], [0., 1.], [1., 1.], [1., 0.], [1., 1.]]);
                depth
                    .indices
                    .extend([0, 3, 4, 0, 4, 1, 1, 4, 5, 1, 5, 2].map(|i| first + i));
            }
        }
        batch.paired_leaf_shadow = true;
        batch.profile_leaf_shadow = true;
    }
    batches.extend(proxies.into_values());
}

/// Run after seasonal annotation, vertex packing and spatial/upload splitting.
/// Each eligible source keeps its own independently culled shadow twin.
pub(crate) fn append_leaf_shadows(
    batches: &mut Vec<MeshBatch>,
    opaque_slots: &HashSet<usize>,
    weld_profile_fallbacks: bool,
) -> Result<(), String> {
    if opaque_slots.is_empty() {
        return Ok(());
    }
    let mut twins = Vec::new();
    let mut lookup = HashMap::<([u32; 3], u8), u16>::default();
    let mut remap = Vec::new();
    for color in &mut *batches {
        if !opaque_slots.contains(&color.material)
            || color.shadow_only
            || (color.paired_leaf_shadow && !(weld_profile_fallbacks && color.profile_leaf_shadow))
            || color.staged.is_some()
            || !color.rooted_wind.is_empty()
            || !color.surface_water.is_empty()
            || color.packed_colors.len() != color.positions.len()
            || color.positions.len() < 128
            || color.compact_indices.is_empty()
        {
            continue;
        }
        let mut depth = MeshBatch {
            material: color.material,
            shadow_only: true,
            paired_leaf_shadow: true,
            fine_leaf_shadow: color.profile_leaf_shadow,
            ..Default::default()
        };
        // The opaque blade shader uses only position for wind and alpha for
        // season. Surface-water/rooted records can select different movement,
        // so these keep their original shadow draw above.
        lookup.clear();
        remap.clear();
        remap.reserve(color.positions.len());
        for (position, tint) in color.positions.iter().zip(&color.packed_colors) {
            let key = (position.map(f32::to_bits), tint[3]);
            let index = match lookup.entry(key) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => {
                    let index = u16::try_from(depth.positions.len())
                        .map_err(|_| "Leaf shadow piece exceeds its compact index range")?;
                    depth.positions.push(*position);
                    depth.packed_colors.push([0, 0, 0, tint[3]]);
                    *entry.insert(index)
                }
            };
            remap.push(index);
        }
        // Tiny or already welded pieces are cheaper as normal casters.
        if depth.positions.len() * 4 > color.positions.len() * 3 {
            continue;
        }
        depth
            .compact_indices
            .reserve_exact(color.compact_indices.len());
        for &index in &color.compact_indices {
            depth.compact_indices.push(
                *remap
                    .get(usize::from(index))
                    .ok_or("Leaf shadow index outside color piece")?,
            );
        }
        color.paired_leaf_shadow = true;
        // The weld becomes the exact caster in fine cascades and local lights.
        // The color mesh now only draws in cameras, including on LOD transitions.
        color.profile_leaf_shadow = false;
        twins.push(depth);
    }
    batches.extend(twins);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{
        BlockTemplate, TemplateSet,
        definitions::{BlockDefinition, BlockLibrary, BlockRenderKind},
        sculpt,
    };

    fn oak_templates() -> TemplateSet {
        let def: BlockDefinition =
            serde_json::from_str(include_str!("../../../../assets/terra/blocks/leaves.json"))
                .unwrap();
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            7,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::Cube,
                bevel_width: 0.,
                faces: Default::default(),
                face_materials: [2; 6],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
        sculpt::register(
            &BlockLibrary {
                blocks: vec![def].into(),
            },
            &mut templates,
            &sculpt::ReferencePalette {
                leaves: Some(3),
                ..Default::default()
            },
        );
        templates
    }

    #[test]
    fn nearby_crown_assigns_one_exact_and_one_coarse_caster_and_keeps_its_cutout_core() {
        use crate::meshing::{builder::append_sculpture, lod::FoldedLeaves};
        use terra_voxel::{SectionPos, meshing::BlockFace};
        let templates = oak_templates();
        let shape = &templates.sculpted_blocks[&7];
        let section = SectionPos::new(-1, 2, 3);
        let origin = section.origin().unwrap();
        let slots = HashSet::from([2]);
        let mut materials = std::collections::BTreeMap::new();
        let mut blades = 0;
        for cell in [[2, 3, 1], [10, 3, 1]] {
            let global = std::array::from_fn(|i| origin[i] + i64::from(cell[i]));
            let variant = sculpt::variant(global, 7);
            let face = BlockFace {
                minimum: cell,
                width: 1,
                height: 1,
                u_axis: 0,
                v_axis: 2,
                side: 3,
                material: 7,
                occupied_neighbor: false,
                edge_mask: 0,
            };
            append_sculpture(
                &mut materials,
                shape,
                face,
                cell,
                shape.relief(3, variant, 63, true),
                &templates,
            )
            .unwrap();
            blades += sculpt::shadow_cards(3, variant, false).len();
        }
        let mut batches: Vec<_> = materials.into_values().collect();
        for b in &mut batches {
            b.packed_colors = vec![[100, 150, 22, 78]; b.positions.len()];
        }
        append_profiles(&mut batches, &slots, section, FoldedLeaves::FULL);
        let mut batches = crate::upload_budget::split_terrain_batches(batches, true).unwrap();
        for b in &mut batches {
            b.compact_indices();
        }
        append_leaf_shadows(&mut batches, &slots, true).unwrap();
        let core = batches.iter().find(|b| b.material == 3).unwrap();
        assert!(!core.paired_leaf_shadow && !core.shadow_only && core.profile_caster().is_none());
        let source = batches
            .iter()
            .find(|b| b.material == 2 && !b.shadow_only)
            .unwrap();
        assert!(source.paired_leaf_shadow && source.profile_caster().is_none());
        let exact: Vec<_> = batches
            .iter()
            .filter(|b| b.profile_caster() == Some(false))
            .collect();
        assert_eq!(exact.len(), 1);
        assert!(exact[0].shadow_only && exact[0].uvs.is_empty());
        assert_eq!(exact[0].compact_indices.len(), source.compact_indices.len());
        let coarse: Vec<_> = batches
            .iter()
            .filter(|b| b.profile_caster() == Some(true))
            .collect();
        assert_eq!(coarse.len(), 2);
        assert!(
            coarse
                .iter()
                .all(|b| b.shadow_only && b.packed_colors.iter().all(|c| c[3] == 78))
        );
        assert_eq!(
            coarse
                .iter()
                .map(|b| b.compact_indices.len())
                .sum::<usize>(),
            blades * 12
        );
    }

    #[test]
    fn welded_oak_shadows_replay_all_authored_triangles_and_seasonal_boundaries() {
        let templates = oak_templates();
        let geometry = &templates.sculpted_blocks[&7];
        let slots = HashSet::from([2]);
        for near in [true, false] {
            let (mut before, mut after, mut triangles) = (0, 0, 0);
            for side in 0..6 {
                for variant in 0..4 {
                    let piece = &geometry.relief(side, variant, 63, near).unwrap()[0];
                    let mut batch = MeshBatch {
                        material: 2,
                        positions: piece.positions.clone(),
                        packed_colors: vec![[100, 150, 22, 78]; piece.positions.len()],
                        indices: piece.indices.clone(),
                        ..Default::default()
                    };
                    // Distinct provenance on coincident seams must survive a weld.
                    batch.packed_colors[1][3] = 255;
                    batch.compact_indices();
                    let mut batches = vec![batch];
                    append_leaf_shadows(&mut batches, &slots, false).unwrap();
                    assert_eq!(batches.len(), 2);
                    let (color, depth) = (&batches[0], &batches[1]);
                    assert!(
                        depth.paired_leaf_shadow && depth.shadow_only && color.paired_leaf_shadow
                    );
                    assert!(
                        depth.shadow_sources.is_empty()
                            && depth.normals.is_empty()
                            && depth.uvs.is_empty()
                    );
                    before += color.positions.len();
                    after += depth.positions.len();
                    triangles += depth.compact_indices.len() / 3;
                    assert_eq!(color.compact_indices.len(), depth.compact_indices.len());
                    for (&from, &to) in color.compact_indices.iter().zip(&depth.compact_indices) {
                        let (from, to) = (usize::from(from), usize::from(to));
                        assert_eq!(
                            color.positions[from].map(f32::to_bits),
                            depth.positions[to].map(f32::to_bits)
                        );
                        assert_eq!(color.packed_colors[from][3], depth.packed_colors[to][3]);
                    }
                }
            }
            assert!(after * 2 < before);
            eprintln!(
                "oak shadow near={near}: {before} -> {after} vertices; {triangles} identical triangles"
            );
        }
    }

    #[test]
    fn selection_is_per_piece_and_requires_certified_dry_packed_paint() {
        fn piece() -> MeshBatch {
            MeshBatch {
                material: 9,
                positions: vec![[1., 0., 0.]; 144],
                packed_colors: vec![[255; 4]; 144],
                compact_indices: (0..144).collect(),
                ..Default::default()
            }
        }
        let mut batches: Vec<_> = (0..5).map(|_| piece()).collect();
        batches[1].surface_water = vec![[0., -0.25]; 144];
        batches[2].rooted_wind = vec![[0., 1.]; 144];
        batches[3].material = 10;
        batches[4].packed_colors.clear();
        append_leaf_shadows(&mut batches, &HashSet::from([9]), false).unwrap();
        assert_eq!(batches.len(), 6);
        assert!(batches[0].paired_leaf_shadow && batches[5].shadow_only);
        assert!(batches[1..5].iter().all(|b| !b.paired_leaf_shadow));
    }
}
