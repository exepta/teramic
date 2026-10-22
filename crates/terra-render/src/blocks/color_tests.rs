#[test]
fn cube_foliage_palette_retains_green_birch_and_orange_autumn() {
    use super::definitions::{BlockLibrary, BlockRenderKind, TexturePattern};

    let directory = terra_core::asset_directory().join("terra/blocks");
    let library = BlockLibrary::load(&directory).expect("block definitions");
    for (id, name, expected, pattern) in [
        (
            7,
            "Painted moss canopy",
            [0.42, 0.68, 0.12, 1.0],
            TexturePattern::Canopy,
        ),
        (
            8,
            "Autumn crown",
            [1.0, 0.35, 0.055, 1.0],
            TexturePattern::Canopy,
        ),
        (
            9,
            "Silver crown",
            [0.62, 0.72, 0.26, 1.0],
            TexturePattern::BirchCanopy,
        ),
    ] {
        let block = library
            .blocks
            .iter()
            .find(|block| block.id == id)
            .expect("foliage");
        assert_eq!(block.render_kind, BlockRenderKind::Cube);
        assert_eq!(block.materials[name].base_color, expected);
        assert!(block.textures.is_empty());
        assert_eq!(block.materials[name].texture_pattern, Some(pattern));
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
    assert_eq!(
        bark.materials["Silver bark"].texture_pattern,
        Some(super::definitions::TexturePattern::BirchBark)
    );
    assert!(bark.textures.is_empty());
    let oak = library.blocks.iter().find(|b| b.id == 6).unwrap();
    assert_eq!(oak.textures["Twisted tree bark"], "textures/wood.png");
    let sand = library.blocks.iter().find(|b| b.id == 4).unwrap();
    assert!(sand.textures.is_empty());
    assert_eq!(
        sand.materials["Fine sand"].texture_pattern,
        Some(super::definitions::TexturePattern::Sand)
    );
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
        let expected = if [6, 10].contains(&id) {
            "textures/wood.png"
        } else {
            "textures/canopy-stylized.png"
        };
        for texture in block.textures.values() {
            assert_eq!(texture, expected);
            assert!(directory.join(texture).is_file());
        }
        assert!(block.model.is_empty());
    }
}
