//! A connected network is not proof that every pair of touching realms agrees.
use super::*;
use terra_core::{
    PlayerUuid,
    kingdom::{ClaimActor, ClaimKind, DiplomaticRelation, DiplomaticTarget, KingdomId},
};

fn ally(
    journal: &mut EditJournal,
    a: PlayerUuid,
    first: KingdomId,
    b: PlayerUuid,
    second: KingdomId,
) {
    let registry = &mut journal.gameplay_mut().kingdoms;
    registry
        .set_relation(
            a,
            first,
            DiplomaticTarget::Player(second),
            DiplomaticRelation::Allied,
        )
        .unwrap();
    registry
        .set_relation(
            b,
            second,
            DiplomaticTarget::Player(first),
            DiplomaticRelation::Allied,
        )
        .unwrap();
}
fn three_realms(journal: &mut EditJournal) -> ([PlayerUuid; 3], [KingdomId; 3]) {
    let actors = [
        PlayerUuid::from_u128(911),
        PlayerUuid::from_u128(912),
        PlayerUuid::from_u128(913),
    ];
    let a = journal
        .gameplay_mut()
        .kingdoms
        .found(actors[0], [0, 20, 0], "North west")
        .unwrap();
    let b = journal
        .gameplay_mut()
        .kingdoms
        .found(actors[1], [0, 20, 16], "Southern bridge")
        .unwrap();
    let c = journal
        .gameplay_mut()
        .kingdoms
        .found(actors[2], [16, 20, 0], "North east")
        .unwrap();
    journal
        .gameplay_mut()
        .kingdoms
        .claim(
            b,
            [1, 1],
            ClaimKind::ProtectedBuilding,
            ClaimActor::Player(actors[1]),
        )
        .unwrap();
    ally(journal, actors[0], a, actors[1], b);
    ally(journal, actors[1], b, actors[2], c);
    (actors, [a, b, c])
}

#[test]
fn pressure_cannot_shortcut_a_forbidden_shared_face_but_a_legal_bridge_still_transports() {
    let (mut chunks, mut journal, _) = fixture();
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        water: false,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    three_realms(&mut journal);
    let a = [15, 4, 15];
    let c = [16, 4, 15];
    let sink = [16, 3, 15];
    put(&mut chunks, &mut journal, [15, 5, 15], BASIN, 0, 8);
    put(&mut chunks, &mut journal, a, WOOD_PIPE, 0, 0);
    put(&mut chunks, &mut journal, c, WOOD_PIPE, 0, 0);
    put(&mut chunks, &mut journal, sink, FLUID_INFUSER, 0, 0);
    for p in [
        [14, 4, 15],
        [14, 4, 16],
        [14, 4, 17],
        [14, 4, 18],
        [16, 4, 18],
        [16, 4, 17],
        [16, 4, 16],
    ] {
        put(&mut chunks, &mut journal, p, WOOD_PIPE, 0, 0);
    }
    let bridge = [15, 4, 18];
    // The stopped pump connects the structural component but cannot carry flow.
    // Only the forbidden A/C contact would bypass this genuine hydraulic stop.
    put(&mut chunks, &mut journal, bridge, PUMP, 1, 0);
    assert!(!crate::kingdom_access::can_transport(
        &chunks, &journal, a, c
    ));
    assert!(crate::kingdom_access::can_transport(
        &chunks,
        &journal,
        a,
        [14, 4, 16]
    ));
    let mut reached = std::collections::BTreeSet::from([a]);
    let mut pending = std::collections::VecDeque::from([a]);
    while let Some(cell) = pending.pop_front() {
        for direction in DIRECTIONS {
            let next = std::array::from_fn(|i| cell[i] + direction[i]);
            if next != bridge
                && chunks
                    .block_at(next)
                    .is_some_and(|m| matches!(m, WOOD_PIPE | BASIN | FLUID_INFUSER))
                && crate::kingdom_access::can_transport(&chunks, &journal, cell, next)
                && reached.insert(next)
            {
                pending.push_back(next);
            }
        }
    }
    assert!(
        !reached.contains(&c),
        "The stopped pump must be the only legal geometric bridge"
    );
    let mut runtime = Runtime::default();
    for _ in 0..3 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert_eq!(journal.plumbing_at(c).unwrap().water.amount, 0);
    assert_eq!(journal.plumbing_at(sink).unwrap().water.amount, 0);
    assert_eq!(total(&journal), 8);
    put(&mut chunks, &mut journal, bridge, WOOD_PIPE, 0, 0);
    for _ in 0..4 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert!(
        journal.plumbing_at(c).unwrap().water.amount > 0
            || journal.plumbing_at(sink).unwrap().water.amount > 0,
        "The allowed A→B→C route remains usable without direct A/C consent"
    );
    assert_eq!(total(&journal), 8);
}

#[test]
fn basin_rectangle_rechecks_every_shared_face_after_relation_withdrawal_even_from_the_common_ally()
{
    let (mut chunks, mut journal, terrain) = fixture();
    let (actors, [a, _b, c]) = three_realms(&mut journal);
    let cells = [[15, 4, 15], [16, 4, 15], [15, 4, 16], [16, 4, 16]];
    for p in cells {
        put(&mut chunks, &mut journal, p, BASIN, 0, 2);
    }
    for p in cells {
        assert_eq!(basin_shape(&chunks, &journal, p), None);
    }
    ally(&mut journal, actors[0], a, actors[2], c);
    let mut runtime = Runtime::default();
    step(&mut runtime, &mut chunks, &mut journal, &terrain);
    let bridge = cells[2];
    let cached = journal.plumbing_at(bridge).unwrap().basin.unwrap();
    assert_eq!(cached.size, [2, 1, 2]);
    journal
        .gameplay_mut()
        .kingdoms
        .set_relation(
            actors[0],
            a,
            DiplomaticTarget::Player(c),
            DiplomaticRelation::Neutral,
        )
        .unwrap();
    assert!(
        cells
            .into_iter()
            .all(|p| crate::kingdom_access::can_transport(&chunks, &journal, bridge, p)),
        "The caller's own pairwise checks alone must appear authorized in this regression"
    );
    let previous = cells.map(|p| journal.plumbing_at(p).unwrap().water.amount);
    assert!(!fill_basin(&chunks, &mut journal, bridge, 1).unwrap());
    assert_eq!(
        cells.map(|p| journal.plumbing_at(p).unwrap().water.amount),
        previous
    );
    for p in cells {
        assert_eq!(basin_shape(&chunks, &journal, p), None);
    }
}
