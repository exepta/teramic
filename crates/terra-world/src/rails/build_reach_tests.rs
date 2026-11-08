use super::*;
use crate::targeting::{WorldTarget, pick_world_for_placement, pick_world_for_rails};
use terra_core::{WoodSpecies, block_properties::BlockPropertiesCatalog};

fn rotate([x, y, z]: Cell, facing: u8) -> Cell {
    match facing {
        0 => [x, y, z],
        1 => [z, y, -x],
        2 => [-x, y, -z],
        _ => [-z, y, x],
    }
}
fn aim(
    chunks: &ChunkStore,
    journal: &EditJournal,
    eye: [f64; 3],
    point: [f64; 3],
) -> (
    terra_voxel::blocks::BlockHit,
    ([f64; 3], [f64; 3]),
    [f32; 2],
) {
    let delta: [f64; 3] = std::array::from_fn(|axis| point[axis] - eye[axis]);
    let length = delta.iter().map(|value| value * value).sum::<f64>().sqrt();
    let direction = delta.map(|value| value / length);
    let (_, target) = pick_world_for_rails(
        chunks,
        journal,
        &Default::default(),
        eye,
        direction,
        crate::props::PropPose::Rest,
    );
    let Some(WorldTarget::Block(hit)) = target else {
        panic!("real rail construction ray must hit the loaded floor")
    };
    (
        hit,
        (eye, direction),
        [
            (-direction[0]).atan2(-direction[2]) as f32,
            direction[1].asin() as f32,
        ],
    )
}

#[test]
fn broad_turns_follow_the_real_corner_aim_and_one_support_holds_over_void() {
    for facing in 0..4 {
        let original = Piece::new([-17, 16, -17], rails::STRAIGHT, facing).unwrap();
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for x in -24..=24 {
            for z in -24..=28 {
                let shift = rotate([x, -1, z], facing);
                block(
                    &mut chunks,
                    std::array::from_fn(|axis| original.anchor[axis] + shift[axis]),
                    3,
                );
                let air = rotate([x, 0, z], facing);
                block(
                    &mut chunks,
                    std::array::from_fn(|axis| original.anchor[axis] + air[axis]),
                    0,
                );
            }
        }
        prepare(&mut chunks, original);
        install(&mut chunks, &mut journal, original);
        let start = Endpoint {
            anchor: original.anchor,
            port: 1,
        };
        let port = original.ports()[1];
        let position = port.position.position();
        let forward = port.tangent.map(f64::from);
        let right = [forward[2], 0.0, -forward[0]];
        let eye = std::array::from_fn(|axis| {
            position[axis] - forward[axis] * 5.0 + if axis == 1 { 3.0 } else { 0.0 }
        });
        let mut previous = None;
        for epsilon in [-0.0001, 0.0001] {
            let point = std::array::from_fn(|axis| {
                position[axis] + right[axis] * 18.4 + forward[axis] * (18.0 + epsilon)
                    - if axis == 1 { 0.125 } else { 0.0 }
            });
            let (hit, ray, look) = aim(&chunks, &journal, eye, point);
            assert!(hit.distance > 8.0 && hit.distance <= rails::RAIL_BUILD_REACH);
            assert!(
                pick_world_for_placement(
                    &chunks,
                    &journal,
                    &Default::default(),
                    ray.0,
                    ray.1,
                    crate::props::PropPose::Rest
                )
                .1
                .is_none(),
                "ordinary block targeting retains its original reach"
            );
            let plan =
                connection_aim(&chunks, &journal, start, hit, ray, look, WoodSpecies::Oak).unwrap();
            let piece = placement::proposed_piece(&plan).unwrap();
            assert!(original.ports()[1].connects(piece.ports()[0]));
            let end = piece.ports()[1];
            assert_eq!(end.tangent, right.map(|value| value as i16));
            let signature = connection_signature(&plan, start);
            if let Some(first) = previous {
                assert_eq!(
                    signature, first,
                    "near-corner aim stays on the same half-block end line"
                );
            } else {
                previous = Some(signature);
            }
        }
        let point = std::array::from_fn(|axis| {
            position[axis] + right[axis] * 18.4 + forward[axis] * 17.9999
                - if axis == 1 { 0.125 } else { 0.0 }
        });
        let (hit, ray, look) = aim(&chunks, &journal, eye, point);
        let proposal =
            proposal_connection_aim(&chunks, &journal, start, hit, ray, look, WoodSpecies::Oak)
                .unwrap();
        let piece = placement::proposed_piece(&proposal).unwrap();
        let supports: Vec<Cell> = piece
            .supports()
            .iter()
            .map(|point| {
                std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis]))
            })
            .collect();
        assert!(
            supports.contains(&hit.position),
            "the actually targeted block can carry the real turn"
        );
        for &support in &supports {
            block(&mut chunks, support, 0);
        }
        assert!(environment(&proposal, &chunks, &journal).is_err());
        block(&mut chunks, hit.position, 3);
        let (fresh, ray, look) = aim(&chunks, &journal, eye, point);
        assert_eq!(fresh.position, hit.position);
        let plan =
            connection_aim(&chunks, &journal, start, fresh, ray, look, WoodSpecies::Oak).unwrap();
        assert_eq!(
            supports
                .iter()
                .filter(|cell| chunks.block_at(**cell) == Some(3))
                .count(),
            1
        );
        assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
        assert!(complete(&journal, piece));
        assert!(complete(&journal, original));
        assert_eq!(
            network(&journal)
                .unwrap()
                .route(
                    Endpoint {
                        anchor: original.anchor,
                        port: 0
                    },
                    next_endpoint(&plan).unwrap(),
                    16
                )
                .unwrap()
                .legs
                .len(),
            2
        );
    }
}

#[test]
fn construction_rays_reach_thirty_two_but_stop_at_real_obstructions_and_unloaded_cells() {
    let mut chunks = ChunkStore::default();
    let journal = EditJournal::default();
    let properties = BlockPropertiesCatalog::default();
    for z in 0..=40 {
        block(&mut chunks, [0, 0, z], 0);
    }
    let origin = [0.5, 0.5, 0.0];
    let direction = [0.0, 0.0, 1.0];
    block(&mut chunks, [0, 0, 20], 3);
    assert!(
        matches!(pick_world_for_rails(&chunks,&journal,&properties,origin,direction,crate::props::PropPose::Rest).1,Some(WorldTarget::Block(hit)) if hit.position==[0,0,20] && hit.distance==20.0)
    );
    assert!(
        pick_world_for_placement(
            &chunks,
            &journal,
            &properties,
            origin,
            direction,
            crate::props::PropPose::Rest
        )
        .1
        .is_none()
    );
    block(&mut chunks, [0, 0, 5], 3);
    assert!(
        matches!(pick_world_for_rails(&chunks,&journal,&properties,origin,direction,crate::props::PropPose::Rest).1,Some(WorldTarget::Block(hit)) if hit.position==[0,0,5])
    );
    block(&mut chunks, [0, 0, 5], 0);
    block(&mut chunks, [0, 0, 20], 0);
    block(&mut chunks, [0, 0, 32], 3);
    assert!(
        matches!(pick_world_for_rails(&chunks,&journal,&properties,origin,direction,crate::props::PropPose::Rest).1,Some(WorldTarget::Block(hit)) if hit.distance==32.0)
    );
    block(&mut chunks, [0, 0, 32], 0);
    block(&mut chunks, [0, 0, 33], 3);
    assert!(
        pick_world_for_rails(
            &chunks,
            &journal,
            &properties,
            origin,
            direction,
            crate::props::PropPose::Rest
        )
        .1
        .is_none()
    );
    block(&mut chunks, [0, 0, 20], 3);
    chunks.loaded.remove(&global_to_section([0, 0, 5]).0);
    assert!(
        pick_world_for_rails(
            &chunks,
            &journal,
            &properties,
            origin,
            direction,
            crate::props::PropPose::Rest
        )
        .1
        .is_none()
    );
}
