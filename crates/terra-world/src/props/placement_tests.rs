use super::{PropId, PropKind, WorldProp, placement::*};

fn plant(id: u64, kind: PropKind, cell: [i64; 3], slot: usize) -> WorldProp {
    let mut prop = WorldProp {
        id: PropId(id),
        kind,
        position: cell.map(|v| v as f64),
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: terra_voxel::global_to_section(cell).0,
        removed: false,
    };
    align(&mut prop, cell, slot);
    prop
}

fn unchanged_identity(before: WorldProp, after: WorldProp) {
    assert_eq!(support(&before), support(&after));
    assert_eq!(
        WorldProp {
            position: after.position,
            ..before
        },
        after
    );
}

fn assert_fits_without_overlap(props: &[WorldProp], cell: [i64; 3]) {
    for (index, prop) in props.iter().enumerate() {
        let [low, high] = prop.bounds();
        for axis in [0, 2] {
            assert!(low[axis] >= cell[axis] as f64 - 0.00001);
            assert!(high[axis] <= cell[axis] as f64 + 1.00001);
            assert!(high[axis] - low[axis] <= 0.50001);
            assert!((prop.position[axis] - cell[axis] as f64 - 0.5).abs() <= 0.25001);
        }
        for other in &props[index + 1..] {
            let [other_low, other_high] = other.bounds();
            assert!(
                [0, 2].into_iter().any(|axis| {
                    high[axis] <= other_low[axis] + 0.00001
                        || low[axis] >= other_high[axis] - 0.00001
                }),
                "overlapping plants: {prop:?} and {other:?}"
            );
        }
    }
}

#[test]
fn one_to_four_plants_form_compact_centered_groups_independent_of_order() {
    let cell = [-17, 15, -1];
    for count in 1..=4 {
        let mut props: Vec<_> = [17, 1, 29, 4]
            .into_iter()
            .take(count)
            .map(|id| plant(id, PropKind::Flower, cell, 0))
            .collect();
        let before = props.clone();
        reflow(&mut props);
        assert_fits_without_overlap(&props, cell);
        for original in &before {
            unchanged_identity(
                *original,
                *props.iter().find(|p| p.id == original.id).unwrap(),
            );
        }
        let center = [cell[0] as f64 + 0.5, cell[2] as f64 + 0.5];
        match count {
            1 => assert_eq!(props[0].position, [center[0], 16.0, center[1]]),
            2 => {
                assert_eq!(props[0].position[2], center[1]);
                assert_eq!(props[1].position[2], center[1]);
                assert!(
                    (props[0].position[0] + props[1].position[0] - 2.0 * center[0]).abs() < 1e-8
                );
            }
            3 => {
                assert_eq!(props[0].position[2], props[1].position[2]);
                assert_eq!(props[2].position[0], center[0]);
                assert!(props[0].position[2] < center[1]);
                assert!(props[2].position[2] > center[1]);
            }
            _ => {
                for axis in [0, 2] {
                    assert!(
                        (props.iter().map(|p| p.position[axis]).sum::<f64>() / 4.0
                            - cell[axis] as f64
                            - 0.5)
                            .abs()
                            < 1e-8
                    );
                }
            }
        }
        let expected = props.clone();
        reflow(&mut props);
        assert_eq!(props, expected);
        let mut shuffled = before;
        shuffled.reverse();
        reflow(&mut shuffled);
        assert_eq!(shuffled, expected);
    }
}

#[test]
fn four_plants_use_capacity_and_removal_reflows_without_changing_identity() {
    let cell = [-17, 15, -1];
    let mut props = Vec::new();
    for (index, kind) in [
        PropKind::Flower,
        PropKind::Bush,
        PropKind::Fern,
        PropKind::Mushroom,
    ]
    .into_iter()
    .enumerate()
    {
        let slot = next_slot(&props, cell, kind).unwrap();
        props.push(plant(index as u64 + 1, kind, cell, slot));
        reflow(&mut props);
        assert_fits_without_overlap(&props, cell);
    }
    assert_eq!(next_slot(&props, cell, PropKind::SmallPlant), None);
    assert_eq!(next_slot(&props, cell, PropKind::Grass), None);
    let before = props.clone();
    props[1].removed = true;
    reflow(&mut props);
    let slot = next_slot(&props, cell, PropKind::Flower).unwrap();
    props.push(plant(10, PropKind::Flower, cell, slot));
    reflow(&mut props);
    for original in before.iter().filter(|p| p.id != PropId(2)) {
        unchanged_identity(
            *original,
            *props.iter().find(|p| p.id == original.id).unwrap(),
        );
    }
    let live: Vec<_> = props.into_iter().filter(|p| !p.removed).collect();
    assert_fits_without_overlap(&live, cell);
    assert_eq!(next_slot(&live, cell, PropKind::Flower), None);
}

#[test]
fn grass_uses_two_units_and_moves_companions_outward_only_while_present() {
    let cell = [-1, -17, -16];
    let mut props = vec![
        plant(1, PropKind::Flower, cell, 0),
        plant(2, PropKind::Mushroom, cell, 3),
    ];
    reflow(&mut props);
    let inner = props.clone();
    let slot = next_slot(&props, cell, PropKind::Grass).unwrap();
    props.push(plant(3, PropKind::Grass, cell, slot));
    reflow(&mut props);
    assert_eq!(props[2].position, [-0.5, -16.0, -15.5]);
    assert_eq!(next_slot(&props, cell, PropKind::Flower), None);
    assert_eq!(next_slot(&props, cell, PropKind::Grass), None);
    assert_eq!(props.iter().map(|p| cost(p.kind)).sum::<usize>(), CAPACITY);
    for p in &props[..2] {
        for axis in [0, 2] {
            assert_eq!((p.position[axis] - cell[axis] as f64 - 0.5).abs(), 0.25);
        }
    }
    let outer = props[..2].to_vec();
    props[2].removed = true;
    reflow(&mut props);
    assert_eq!(&props[..2], &inner);
    let slot = next_slot(&props, cell, PropKind::Grass).unwrap();
    props.push(plant(4, PropKind::Grass, cell, slot));
    reflow(&mut props);
    assert_eq!(&props[..2], &outer);
    assert!(props.iter().find(|p| p.id == PropId(3)).unwrap().removed);
    assert_eq!(
        props.iter().find(|p| p.id == PropId(4)).unwrap().position,
        [-0.5, -16.0, -15.5]
    );
}

#[test]
fn adjacent_supports_do_not_consume_capacity_or_move_when_a_group_changes() {
    let cell = [-1, 15, -1];
    let next = [0, 15, -1];
    let mut neighbor = [plant(100, PropKind::Bush, next, 0)];
    reflow(&mut neighbor);
    let mut props: Vec<_> = (0..4)
        .map(|slot| plant(slot as u64 + 1, PropKind::Bush, cell, slot))
        .collect();
    assert_eq!(next_slot(&props, next, PropKind::Grass), Some(0));
    reflow(&mut props);
    assert_fits_without_overlap(&props, cell);
    props.push(neighbor[0]);
    props[0].removed = true;
    reflow(&mut props);
    assert_eq!(
        props.iter().find(|p| p.id == neighbor[0].id),
        Some(&neighbor[0])
    );
}

#[test]
fn flowers_and_mushrooms_keep_requested_height_ranges_and_quarter_block_footprints() {
    use super::visual::{PropDetail, PropPalette, template};
    let cell = [-1, 15, -1];
    for (kind, minimum, maximum) in [
        (PropKind::Flower, 0.65, 0.95),
        (PropKind::Mushroom, 0.35, 0.55),
    ] {
        for variant in 0..8 {
            for id in [0, 32768, 65535, u64::MAX] {
                let mut prop = plant(id, kind, cell, 0);
                prop.variant = variant;
                reflow(std::slice::from_mut(&mut prop));
                assert!(prop.valid());
                let meshes =
                    template(kind, variant, PropDetail::Near, &PropPalette::default()).unwrap();
                let high = meshes
                    .iter()
                    .flat_map(|mesh| &mesh.positions)
                    .map(|p| p[1] * prop.visual_scale())
                    .fold(0.0_f32, f32::max);
                assert!(
                    high >= minimum - 0.00001 && high <= maximum + 0.00001,
                    "{prop:?}: height={high}"
                );
                assert_fits_without_overlap(&[prop], cell);
            }
        }
    }
}

#[test]
fn legacy_oversized_plants_and_unaligned_grass_are_not_silently_moved() {
    let cell = [0, 15, 0];
    let mut old = plant(1, PropKind::Bush, cell, 0);
    old.position = [0.5, 16.0, 0.5];
    old.scale = 2.0;
    assert_eq!(next_slot(&[old], cell, PropKind::Grass), None);
    assert_eq!(next_slot(&[old], cell, PropKind::Flower), None);
    let mut legacy = [old];
    reflow(&mut legacy);
    assert_eq!(legacy, [old]);
    let mut grass = plant(2, PropKind::Grass, cell, 0);
    for (position, rotation, scale) in [
        ([0.55, 16.0, 0.5], 0.0, 1.0),
        ([0.5, 16.0, 0.5], 0.6, 1.0),
        ([0.5, 16.0, 0.5], 0.0, 1.5),
    ] {
        grass.position = position;
        grass.rotation = rotation;
        grass.scale = scale;
        assert_eq!(next_slot(&[grass], cell, PropKind::Flower), None);
        let mut group = [grass];
        reflow(&mut group);
        assert_eq!(group, [grass]);
    }
}
