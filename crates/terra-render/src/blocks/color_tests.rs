#[test]
fn cube_foliage_uses_distinct_cutout_authored_top_and_side_art_without_double_tint() {
    use super::definitions::{BlockLibrary, BlockRenderKind};
    let directory = terra_core::asset_directory().join("terra/blocks");
    let library = BlockLibrary::load(&directory).expect("block definitions");
    for (id, style) in [
        (7, "oak"),
        (8, "autumn"),
        (9, "birch"),
        (132, "cherry_pink"),
        (133, "cherry_white"),
        (195, "apple"),
        (310, "spirit_pink"),
        (311, "spirit_pearl"),
        (312, "spirit_lavender"),
        (406, "blood"),
    ] {
        let block = library.blocks.iter().find(|block| block.id == id).unwrap();
        assert_eq!(block.render_kind, BlockRenderKind::Cube);
        assert!(block.model.is_empty() && block.light.is_none());
        let cluster = block.foliage_cluster.as_ref().unwrap();
        assert_eq!(
            block.materials.len(),
            3 + usize::from(cluster.needles.is_some())
        );
        assert_eq!(cluster.needles.is_some(), id == 7);
        for (name, sprite) in std::iter::once((&cluster.material, style))
            .chain(cluster.needles.iter().map(|name| (name, "spruce")))
        {
            let material = &block.materials[name];
            assert_eq!(material.base_color, [1.0; 4]);
            assert!(material.texture_pattern.is_none());
            assert_eq!(material.emission, [0.0; 3]);
            assert_eq!(
                block.textures[name],
                format!("textures/foliage/{sprite}_cluster.png")
            );
            assert!(directory.join(&block.textures[name]).is_file());
        }
        let faces = block.face_materials.as_ref().unwrap();
        assert_ne!(faces[0], faces[3]);
        for (side, name) in faces.iter().enumerate() {
            let material = &block.materials[name];
            assert_eq!(material.base_color, [1.0; 4]);
            assert!(material.texture_pattern.is_none());
            assert_eq!(material.emission, [0.0; 3]);
            let suffix = if side == 3 { "top" } else { "side" };
            assert_eq!(
                block.textures[name],
                format!("textures/foliage/{style}_{suffix}.png")
            );
            assert!(directory.join(&block.textures[name]).is_file());
        }
    }
}

#[test]
fn cube_definitions_preserve_bark_and_use_actual_granular_sand() {
    let directory = terra_core::asset_directory().join("terra/blocks");
    let library = super::definitions::BlockLibrary::load(&directory).expect("block definitions");
    let bark = library
        .blocks
        .iter()
        .find(|block| block.id == 10)
        .expect("bark");
    assert_eq!(bark.materials["Silver bark"].base_color, [1.0; 4]);
    for (id, expected) in [(10, "birch_bark"), (6, "oak_bark"), (4, "sand")] {
        let block = library.blocks.iter().find(|b| b.id == id).unwrap();
        let face = &block.face_materials.as_ref().unwrap()[0];
        assert!(block.materials[face].texture_pattern.is_none());
        assert_eq!(
            block.textures[face],
            format!("textures/block_palette/{expected}.png")
        );
        assert_eq!(
            block.materials[face].normal_texture.as_deref(),
            Some(format!("textures/block_palette/{expected}_normal.png").as_str())
        );
        assert!(directory.join(&block.textures[face]).is_file());
    }
}

#[test]
fn tree_assets_use_botanical_and_bark_textures_not_stone() {
    let directory = terra_core::asset_directory().join("terra/blocks");
    let library = super::definitions::BlockLibrary::load(&directory).expect("block definitions");
    for id in [6, 7, 8, 9, 10] {
        let block = library
            .blocks
            .iter()
            .find(|b| b.id == id)
            .expect("tree block");
        for texture in block.textures.values() {
            if [6, 10].contains(&id) {
                assert!(texture.starts_with("textures/block_palette/"));
                assert!(texture.contains("oak_") || texture.contains("birch_"));
            } else {
                assert!(texture.starts_with("textures/foliage/"));
            }
            assert!(directory.join(texture).is_file());
        }
        assert!(block.model.is_empty());
    }
}

#[test]
fn ballast_block_stairs_and_slab_share_the_actual_angular_material_and_physical_profiles() {
    use super::definitions::{BlockLibrary, BlockRenderKind, TexturePattern};
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    for (id, kind, boxes) in [
        (206, BlockRenderKind::Cube, 0),
        (207, BlockRenderKind::Boxes, 4),
        (208, BlockRenderKind::Boxes, 1),
    ] {
        let definition = library.blocks.iter().find(|block| block.id == id).unwrap();
        definition.validate().unwrap();
        assert_eq!(definition.render_kind, kind);
        assert_eq!(definition.creative_category.as_deref(), Some("trains"));
        assert_eq!(definition.collision_boxes.len(), boxes);
        assert_eq!(definition.bevel_width, 0.0);
        assert!(definition.model.is_empty());
        for face in definition.face_materials.as_ref().unwrap() {
            assert_eq!(
                definition.materials[face].texture_pattern,
                Some(TexturePattern::Ballast)
            );
        }
        if id != 206 {
            assert_eq!(
                definition.collision_boxes,
                terra_core::construction::boxes(id)
            );
        }
    }
    let image = super::textures::generate(TexturePattern::Ballast);
    assert_eq!(image.texture_descriptor.size.width, 512);
    assert_eq!(image.texture_descriptor.size.height, 512);
    assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 1);
    assert_eq!(image.texture_descriptor.mip_level_count, 10);
    assert_eq!(
        image.texture_descriptor.format,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb
    );
    let expected_bytes = (0..image.texture_descriptor.mip_level_count)
        .map(|level| (512_usize >> level).max(1).pow(2) * 4)
        .sum::<usize>();
    let data = image.data.unwrap();
    assert_eq!(
        data.len(),
        expected_bytes,
        "base level plus every mip through 1x1"
    );
    assert!(data.chunks_exact(4).all(|pixel| pixel[3] == 255));
    let gravel = super::textures::generate(TexturePattern::Gravel)
        .data
        .unwrap();
    assert_ne!(
        &data[..512 * 512 * 4],
        &gravel[..512 * 512 * 4],
        "ballast angular shards must have their own native base texture"
    );
}
