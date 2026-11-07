use super::definitions::BlockLibrary;
use super::*;

#[test]
fn alchemy_blocks_have_open_bounded_hulls_and_never_permanent_fire_lights() {
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    for material in [161, 162] {
        let block = library
            .blocks
            .iter()
            .find(|block| block.id == material)
            .unwrap();
        block.validate().unwrap();
        assert_eq!(block.render_kind, definitions::BlockRenderKind::CustomMesh);
        assert!(
            block.light.is_none(),
            "fuel state, not a static block definition, controls fire light"
        );
        assert!(block.collision_boxes.len() <= 16);
        for bounds in &block.collision_boxes {
            assert!((0..3).all(|axis| bounds.minimum[axis] >= 0.0
                && bounds.maximum[axis] <= 1.0
                && bounds.minimum[axis] < bounds.maximum[axis]));
        }
        if material == 161 {
            let contains = |point: [f64; 3]| {
                block.collision_boxes.iter().any(|bounds| {
                    (0..3).all(|axis| {
                        point[axis] >= bounds.minimum[axis] && point[axis] <= bounds.maximum[axis]
                    })
                })
            };
            assert!(
                contains([0.5, 0.365, 0.5]),
                "the actual bowl bottom retains items"
            );
            assert!(!contains([0.5, 0.90, 0.5]), "the vessel mouth is open");
            assert!(
                !contains([0.5, 0.15, 0.5]),
                "the burner stand has no solid central fill"
            );
        }
    }
}

#[test]
fn raised_hearth_piers_contact_the_authored_cauldron_feet_and_real_floor() {
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    let cauldron = library.blocks.iter().find(|block| block.id == 161).unwrap();
    let supports = terra_world::interaction_shapes::campfire_support_boxes();
    let feet: Vec<_> = cauldron
        .collision_boxes
        .iter()
        .filter(|bounds| bounds.minimum[1] == 0.0)
        .collect();
    assert_eq!(feet.len(), 4);
    for foot in feet {
        let column = supports
            .iter()
            .find(|bounds| {
                bounds.maximum[1] == 1.0
                    && [0, 2].into_iter().all(|axis| {
                        (bounds.minimum[axis] - foot.minimum[axis]).abs() < 1e-9
                            && (bounds.maximum[axis] - foot.maximum[axis]).abs() < 1e-9
                    })
            })
            .expect("each real authored foot sits on its own full-width pier");
        assert_eq!(column.maximum[1], 1.0 + foot.minimum[1]);
        assert!(supports.iter().any(|base| {
            base.minimum[1] == 0.0
                && base.maximum[1] >= column.minimum[1]
                && [0, 2].into_iter().all(|axis| {
                    base.minimum[axis] <= column.minimum[axis]
                        && base.maximum[axis] >= column.maximum[axis]
                })
        }));
    }
}
