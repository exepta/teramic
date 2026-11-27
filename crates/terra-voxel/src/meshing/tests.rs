use super::*;
fn index(p: [usize; 3]) -> usize {
    p[0] + 18 * (p[1] + 18 * p[2])
}
#[test]
fn isolated_blocks_have_six_faces_with_correct_winding() {
    let mut voxels = vec![0; PADDED_VOLUME];
    voxels[index([1; 3])] = 3;
    let mesh = mesh_section(&voxels).unwrap();
    assert_eq!(mesh.indices.len(), 36);
    assert!(
        mesh.positions
            .iter()
            .all(|p| p.iter().all(|v| (0.0..=1.0).contains(v)))
    );
    for triangle in mesh.indices.chunks_exact(3) {
        let a = mesh.positions[triangle[0] as usize];
        let b = mesh.positions[triangle[1] as usize];
        let c = mesh.positions[triangle[2] as usize];
        let u = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
        let v = std::array::from_fn::<_, 3, _>(|i| c[i] - a[i]);
        let cross = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let n = mesh.normals[triangle[0] as usize];
        assert!(cross.iter().zip(n).map(|(a, b)| a * b).sum::<f32>() > 0.0);
    }
}
#[test]
fn greedy_meshing_culls_internal_faces_and_keeps_material_boundaries() {
    let mut voxels = vec![0; PADDED_VOLUME];
    voxels[index([1, 1, 1])] = 3;
    voxels[index([2, 1, 1])] = 3;
    assert_eq!(mesh_section(&voxels).unwrap().indices.len(), 36);
    voxels[index([2, 1, 1])] = 2;
    assert_eq!(mesh_section(&voxels).unwrap().indices.len(), 60);
    voxels[index([0, 1, 1])] = 3;
    let mesh = mesh_section(&voxels).unwrap();
    assert_eq!(mesh.indices.len(), 54);
    assert!(
        !mesh
            .normals
            .iter()
            .zip(&mesh.positions)
            .any(|(n, p)| n == &[-1.0, 0.0, 0.0] && p[0] == 0.0)
    );
}
#[test]
fn fully_surrounded_sections_have_no_geometry() {
    assert!(mesh_section(&vec![3; PADDED_VOLUME]).unwrap().is_empty());
    assert!(mesh_section(&vec![0; PADDED_VOLUME]).unwrap().is_empty());
    assert!(mesh_section(&[]).is_err());
}

#[test]
fn occlusion_lookup_matches_the_catalog_and_keeps_unknown_ids_out_of_ambient_occlusion() {
    use crate::MaterialId;
    for id in 0..=u16::MAX {
        let expected = !matches!(id, 0 | 27)
            && id != MaterialId::Lava as u16
            && MaterialId::from_raw(id).is_none_or(MaterialId::is_opaque);
        assert_eq!(occludes_face(id), expected, "material {id}");
    }
    let mut blocks = vec![0; PADDED_VOLUME];
    let stone = index([1, 1, 1]);
    let unknown = index([2, 1, 1]);
    blocks[stone] = MaterialId::Stone as u16;
    blocks[unknown] = u16::MAX;
    let mesh = mesh_section_faces(&blocks).unwrap();
    assert_eq!(mesh.faces.len(), 10);
    let occupied = |i: usize| mesh.occluders[i / 64] & (1 << (i % 64)) != 0;
    assert!(occupied(stone));
    assert!(!occupied(unknown));
}

#[test]
fn fluid_source_has_no_cube_faces_and_does_not_hide_adjacent_terrain() {
    let mut voxels = vec![0; PADDED_VOLUME];
    voxels[index([1, 1, 1])] = crate::MaterialId::FluidWater as u16;
    assert!(mesh_section(&voxels).unwrap().is_empty());
    voxels[index([2, 1, 1])] = crate::MaterialId::Stone as u16;
    let mesh = mesh_section(&voxels).unwrap();
    assert_eq!(mesh.faces.len(), 6);
    assert!(
        mesh.faces
            .iter()
            .all(|face| face.material == crate::MaterialId::Stone as u16)
    );
}

#[test]
fn foliage_keeps_trunks_visible_and_merges_touching_crowns_across_sections() {
    for leaf in crate::MaterialId::FOLIAGE {
        let mut voxels = vec![0; PADDED_VOLUME];
        voxels[index([1, 1, 1])] = leaf;
        for neighbor in [6, leaf, 9] {
            voxels[index([2, 1, 1])] = neighbor;
            let mesh = mesh_section_faces(&voxels).unwrap();
            assert!(
                !mesh
                    .faces
                    .iter()
                    .any(|f| f.material == leaf && f.side == 1 && f.minimum[0] == 0)
            );
            assert_eq!(
                mesh.faces.iter().any(|f| f.material == neighbor
                    && f.side == 0
                    && f.minimum[0] == 1
                    && f.occupied_neighbor),
                neighbor == 6,
            );
        }
        voxels[index([2, 1, 1])] = 0;
        voxels[index([0, 1, 1])] = leaf;
        for (id, visible) in [(6, true), (leaf, false)] {
            voxels[index([1, 1, 1])] = id;
            let mesh = mesh_section_faces(&voxels).unwrap();
            assert_eq!(
                mesh.faces
                    .iter()
                    .any(|f| f.side == 0 && f.occupied_neighbor),
                visible
            );
        }
    }
    let mut voxels = vec![0; PADDED_VOLUME];
    voxels[index([1, 1, 1])] = 7;
    for p in [[2, 1, 1], [0, 1, 1]] {
        voxels[index(p)] = crate::MaterialId::Glass as u16;
        let mesh = mesh_section_faces(&voxels).unwrap();
        let leaves: Vec<_> = mesh.faces.iter().filter(|f| f.material == 7).collect();
        assert_eq!(leaves.len(), 6);
        assert_eq!(leaves.iter().filter(|f| f.occupied_neighbor).count(), 1);
        assert!(
            leaves
                .iter()
                .any(|f| f.side == usize::from(p[0] == 2) && f.occupied_neighbor)
        );
        voxels[index(p)] = 0;
    }
    assert!(mesh_section(&vec![7; PADDED_VOLUME]).unwrap().is_empty());
}

#[test]
fn only_mixed_air_glass_contacts_split_greedy_faces_and_do_not_grow_the_face_cache_record() {
    assert_eq!(
        std::mem::size_of::<BlockFace>(),
        if usize::BITS == 64 { 48 } else { 32 }
    );
    let mut voxels = vec![0; PADDED_VOLUME];
    voxels[index([1, 1, 1])] = 3;
    voxels[index([2, 1, 1])] = 3;
    let plain = mesh_section(&voxels).unwrap();
    assert_eq!(plain.faces.len(), 6);
    assert!(plain.faces.iter().all(|f| !f.occupied_neighbor));
    voxels[index([1, 2, 1])] = crate::MaterialId::Glass as u16;
    let mixed = mesh_section(&voxels).unwrap();
    assert_eq!(mixed.faces.iter().filter(|f| f.material == 3).count(), 7);
    let top: Vec<_> = mixed
        .faces
        .iter()
        .filter(|f| f.material == 3 && f.side == 3)
        .collect();
    assert_eq!(top.len(), 2);
    assert!(top.iter().all(|f| f.width * f.height == 1));
    assert!(
        top.iter()
            .all(|f| f.occupied_neighbor == (f.minimum[0] == 0))
    );
    voxels[index([2, 2, 1])] = crate::MaterialId::Glass as u16;
    let covered = mesh_section(&voxels).unwrap();
    assert_eq!(covered.faces.iter().filter(|f| f.material == 3).count(), 6);
    let top: Vec<_> = covered
        .faces
        .iter()
        .filter(|f| f.material == 3 && f.side == 3)
        .collect();
    assert_eq!(top.len(), 1);
    assert!(top[0].occupied_neighbor);
    assert_eq!(top[0].width * top[0].height, 2);
}

#[test]
fn glass_and_solid_water_show_the_neighboring_stone_face_while_frosted_ice_occludes_it() {
    for (material, shows_stone) in [
        (crate::MaterialId::Glass, true),
        (crate::MaterialId::SolidWater, true),
        (crate::MaterialId::Ice, false),
        (crate::MaterialId::ClearCaveIce, false),
    ] {
        let mut voxels = vec![0; PADDED_VOLUME];
        voxels[index([1, 1, 1])] = 3;
        voxels[index([2, 1, 1])] = material as u16;
        let mesh = mesh_section(&voxels).unwrap();
        let stone_face = mesh.faces.iter().find(|f| f.material == 3 && f.side == 1);
        assert_eq!(
            stone_face.map(|f| f.occupied_neighbor),
            shows_stone.then_some(true),
            "{material:?}"
        );
        assert!(
            !mesh
                .faces
                .iter()
                .any(|f| f.material == material as u16 && f.side == 0)
        );
        voxels[index([1, 1, 1])] = material as u16;
        let mesh = mesh_section(&voxels).unwrap();
        assert!(!mesh.faces.iter().any(|f| f.side == 1 && f.minimum[0] == 0));
        assert!(!mesh.faces.iter().any(|f| f.side == 0 && f.minimum[0] == 1));
        assert!(
            mesh.indices
                .iter()
                .all(|&i| (i as usize) < mesh.positions.len())
        );
    }
}

#[test]
fn connected_glass_frames_follow_padding_at_positive_and_negative_section_edges() {
    for x in [1, 16] {
        let mut voxels = vec![0; PADDED_VOLUME];
        voxels[index([x, 1, 1])] = crate::MaterialId::Glass as u16;
        let neighbor = if x == 1 { 0 } else { 17 };
        voxels[index([neighbor, 1, 1])] = crate::MaterialId::Glass as u16;
        let mesh = mesh_section(&voxels).unwrap();
        assert_eq!(mesh.faces.len(), 5);
        let face = mesh.faces.iter().find(|f| f.side == 3).unwrap();
        let bit = match (face.u_axis == 0, x == 1) {
            (true, true) => 1,
            (true, false) => 2,
            (false, true) => 4,
            (false, false) => 8,
        };
        assert_eq!(face.edge_mask, 15 & !bit);
        assert_eq!([face.width, face.height], [1, 1]);
        voxels[index([neighbor, 1, 1])] = 0;
        assert!(
            mesh_section(&voxels)
                .unwrap()
                .faces
                .iter()
                .all(|f| f.edge_mask == 15)
        );
    }
}

#[test]
fn connected_glass_panel_has_no_interior_frame_edges() {
    let mut voxels = vec![0; PADDED_VOLUME];
    for z in 1..=3 {
        for x in 1..=3 {
            voxels[index([x, 1, z])] = crate::MaterialId::Glass as u16;
        }
    }
    let mesh = mesh_section(&voxels).unwrap();
    let top: Vec<_> = mesh.faces.iter().filter(|f| f.side == 3).collect();
    assert_eq!(top.len(), 9);
    assert_eq!(
        top.iter()
            .find(|f| f.minimum == [1, 0, 1])
            .unwrap()
            .edge_mask,
        0
    );
    assert_eq!(
        top.iter().map(|f| f.edge_mask.count_ones()).sum::<u32>(),
        12
    );
}

#[test]
fn partial_lava_keeps_the_bank_visible_and_culls_the_join_between_liquid_cells() {
    let lava = crate::MaterialId::Lava as u16;
    let mut voxels = vec![0; PADDED_VOLUME];
    voxels[index([2, 1, 1])] = 3;
    voxels[index([1, 1, 1])] = lava;
    voxels[index([1, 1, 2])] = lava;
    let mesh = mesh_section(&voxels).unwrap();
    assert!(
        mesh.faces
            .iter()
            .any(|f| f.material == 3 && f.side == 0 && f.occupied_neighbor)
    );
    assert!(
        !mesh
            .faces
            .iter()
            .any(|f| f.material == lava && f.minimum == [0, 0, 0] && f.side == 5)
    );
}

#[test]
fn streamed_faces_match_full_geometry_without_allocating_vertex_arrays() {
    let mut voxels = vec![0; PADDED_VOLUME];
    for (x, raw) in [3, 3, 7, 15, 27, 613].into_iter().enumerate() {
        voxels[index([x + 1, 1, 1])] = raw;
    }
    let full = mesh_section(&voxels).unwrap();
    let streamed = mesh_section_faces(&voxels).unwrap();
    assert_eq!(streamed.faces, full.faces);
    assert_eq!(streamed.occluders, full.occluders);
    assert!(!streamed.is_empty());
    assert!(streamed.positions.capacity() == 0 && streamed.normals.capacity() == 0);
    assert!(streamed.materials.capacity() == 0 && streamed.uvs.capacity() == 0);
    assert_eq!(streamed.indices.capacity(), 0);
}
