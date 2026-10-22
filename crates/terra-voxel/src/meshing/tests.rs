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
fn masked_leaves_preserve_solid_faces_without_internal_leaf_faces() {
    for leaf in 7..=9 {
        let mut voxels = vec![0; PADDED_VOLUME];
        voxels[index([1, 1, 1])] = 6;
        voxels[index([2, 1, 1])] = leaf;
        let mesh = mesh_section(&voxels).unwrap();
        assert!(
            mesh.faces
                .iter()
                .any(|f| f.material == 6 && f.side == 1 && f.occupied_neighbor)
        );
        assert!(!mesh.faces.iter().any(|f| f.material == leaf && f.side == 0));
        assert_eq!(mesh.faces.len(), 11);

        voxels[index([1, 1, 1])] = leaf;
        let joined = mesh_section(&voxels).unwrap();
        assert_eq!(joined.faces.len(), 6);
        assert!(
            !joined
                .faces
                .iter()
                .any(|f| f.side == 1 && f.minimum[0] == 0)
        );
    }
}

#[test]
fn padded_foliage_does_not_hide_a_section_boundary_trunk() {
    let mut voxels = vec![0; PADDED_VOLUME];
    voxels[index([1, 1, 1])] = 10;
    voxels[index([0, 1, 1])] = 9;
    let mesh = mesh_section(&voxels).unwrap();
    assert!(mesh.faces.iter().any(|f| f.material == 10 && f.side == 0));
    assert!(mesh.faces.iter().all(|f| f.material == 10));
    assert!(
        mesh.faces
            .iter()
            .any(|f| f.side == 0 && f.occupied_neighbor)
    );
    assert!(mesh_section(&vec![7; PADDED_VOLUME]).unwrap().is_empty());
}

#[test]
fn only_mixed_air_leaf_contacts_split_greedy_faces_and_do_not_grow_the_face_cache_record() {
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
    voxels[index([1, 2, 1])] = 7;
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
    voxels[index([2, 2, 1])] = 7;
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
fn glass_and_solid_water_keep_opaque_neighbors_visible_and_cull_their_shared_faces() {
    for material in [
        crate::MaterialId::Glass,
        crate::MaterialId::SolidWater,
        crate::MaterialId::Ice,
    ] {
        let mut voxels = vec![0; PADDED_VOLUME];
        voxels[index([1, 1, 1])] = 3;
        voxels[index([2, 1, 1])] = material as u16;
        let mesh = mesh_section(&voxels).unwrap();
        assert!(
            mesh.faces
                .iter()
                .any(|f| f.material == 3 && f.side == 1 && f.occupied_neighbor)
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
