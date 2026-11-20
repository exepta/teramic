use super::*;

fn cell(material: u16, facing: u8) -> TemplateCell {
    TemplateCell {
        position: [0; 3],
        material,
        facing,
        rotate_facing: true,
        peer: None,
        secondary: false,
    }
}
fn rotated_boxes(material: u16, facing: u8, turns: u8) -> Vec<(u16, [i64; 6])> {
    let parts = if crate::construction::is_slab(material) {
        crate::construction::slab_parts(material, facing)
    } else {
        crate::construction::placed_boxes(material, facing, [None; 6])
            .into_iter()
            .map(|b| (material, b))
            .collect()
    };
    let mut boxes: Vec<_> = parts
        .into_iter()
        .map(|(m, b)| {
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for corner in 0..8 {
                let mut p: [f64; 3] = std::array::from_fn(|i| {
                    if corner & (1 << i) == 0 {
                        b.minimum[i]
                    } else {
                        b.maximum[i]
                    }
                });
                for _ in 0..turns {
                    p = [p[2], p[1], 1.0 - p[0]];
                }
                for i in 0..3 {
                    lo[i] = lo[i].min(p[i]);
                    hi[i] = hi[i].max(p[i]);
                }
            }
            (
                m,
                std::array::from_fn(|i| {
                    if i < 3 {
                        (lo[i] * 1e6).round() as i64
                    } else {
                        (hi[i - 3] * 1e6).round() as i64
                    }
                }),
            )
        })
        .collect();
    boxes.sort();
    boxes
}

#[test]
fn all_twenty_four_stair_states_rotate_the_real_saved_geometry_and_keep_one_item_cost() {
    for facing in 0..24 {
        let source = cell(74, facing);
        assert_eq!(
            cell_inputs(std::slice::from_ref(&source)).unwrap(),
            vec![ItemStack {
                item: ItemKind::Block(74),
                count: 1
            }]
        );
        for turns in 0..4 {
            let placed = rotated_facing(&source, turns).unwrap();
            assert_eq!(
                rotated_boxes(74, placed, 0),
                rotated_boxes(74, facing, turns)
            );
            if facing < 4 {
                assert_eq!(placed, (facing + turns) % 4);
            }
        }
    }
}
#[test]
fn mixed_slabs_keep_both_real_material_inputs_and_rotate_the_split_plane() {
    for first in [82, 84] {
        for second in [83, 85] {
            for half in [0, 4, 5, 6, 7, 8] {
                let facing = crate::construction::merged_slab(first, half, second).unwrap();
                let source = cell(first, facing);
                assert_eq!(
                    cell_inputs(std::slice::from_ref(&source))
                        .unwrap()
                        .iter()
                        .map(|s| s.count)
                        .sum::<u16>(),
                    2
                );
                for turns in 0..4 {
                    let placed = rotated_facing(&source, turns).unwrap();
                    assert_eq!(
                        rotated_boxes(first, placed, 0),
                        rotated_boxes(first, facing, turns)
                    );
                    assert_eq!(
                        cell_inputs(&[cell(first, placed)]).unwrap(),
                        cell_inputs(std::slice::from_ref(&source)).unwrap()
                    );
                }
            }
        }
    }
    assert_eq!(
        cell_inputs(&[cell(82, 9)]).unwrap(),
        vec![ItemStack {
            item: ItemKind::Block(82),
            count: 2
        }]
    );
}
#[test]
fn paired_chests_require_two_items_and_rotate_reciprocal_side_neighbors() {
    let mut primary = cell(29, 0);
    primary.peer = Some([1, 0, 0]);
    let mut secondary = cell(29, 0);
    secondary.position = [1, 0, 0];
    secondary.peer = Some([0; 3]);
    secondary.secondary = true;
    let cells = vec![primary, secondary];
    let costs = cell_inputs(&cells).unwrap();
    assert_eq!(
        costs,
        vec![ItemStack {
            item: ItemKind::Block(29),
            count: 2
        }]
    );
    let steps = vec![TemplateStep {
        phase: BuildPhase::Furnishing,
        cells,
        costs,
    }];
    let mut template = BuildingTemplate {
        schema_version: TEMPLATE_SCHEMA,
        id: "paired-chest-house".into(),
        display_name: "Chest house".into(),
        category: TemplateCategory::Residential,
        anchor: [0; 3],
        bounds: BuildArea::new([0; 3], [1, 0, 0]).unwrap(),
        materials: material_totals(&steps).unwrap(),
        steps,
    };
    template.validate().unwrap();
    for facing in 0..4 {
        let placed = template.placed([-1, 12, -32], facing).unwrap();
        assert_eq!(placed[0].costs[0].count, 2);
        assert_eq!(placed[0].cells[0].peer, Some(placed[0].cells[1].position));
    }
    template.steps[0].costs[0].count = 1;
    template.materials = material_totals(&template.steps).unwrap();
    assert!(template.validate().is_err());
}
