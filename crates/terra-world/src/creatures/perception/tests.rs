use super::*;
use crate::{
    TerraGenerator,
    edits::{BlockObjectState, EditJournal},
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{
    WorldConfig,
    creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds, creature_id},
};
use terra_voxel::{BlockSection, MaterialId, SectionPos};

pub(in crate::creatures) fn fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        water: false,
        biomes: false,
        caves: false,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for x in -1..=1 {
        for z in -1..=1 {
            let owner = SectionPos::new(x, 0, z);
            let mut data = BlockSection::empty(owner).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    data.set_block([x, 0, z], 3).unwrap();
                }
            }
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
    }
    (terrain, chunks, EditJournal::default())
}

#[test]
fn sight_respects_real_slabs_open_doorways_closed_leaves_and_unknown_cells() {
    let (terrain, chunks, mut journal) = fixture();
    let check = |journal: &EditJournal, a, b| {
        visible(
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal,
            },
            a,
            b,
        )
    };
    journal
        .record([8, 1, 8], MaterialId::OakSlab as u16)
        .unwrap();
    assert!(check(&journal, [6.5, 1.75, 8.5], [10.5, 1.75, 8.5]));
    assert!(!check(&journal, [6.5, 1.25, 8.5], [10.5, 1.25, 8.5]));
    for (cell, peer, secondary) in [([8, 1, 8], [8, 2, 8], false), ([8, 2, 8], [8, 1, 8], true)] {
        journal.record(cell, MaterialId::OakDoor as u16).unwrap();
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    assert!(!check(&journal, [8.5, 1.8, 6.5], [8.5, 1.8, 10.5]));
    for cell in [[8, 1, 8], [8, 2, 8]] {
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    facing: 4,
                    ..journal.object_state(cell).unwrap()
                }),
            )
            .unwrap();
    }
    assert!(check(&journal, [8.5, 1.8, 6.5], [8.5, 1.8, 10.5]));
    assert!(!check(&journal, [8.5, 1.8, 6.5], [40.5, 1.8, 6.5]));
    assert!(!check(&journal, [f64::NAN, 1.8, 6.5], [8.5, 1.8, 10.5]));
    assert!(!check(&journal, [8.5, 1.8, 6.5], [200.5, 1.8, 6.5]));
}

#[test]
fn sight_reaches_exact_floor_and_voxel_edge_endpoints_without_overshooting() {
    let (terrain, chunks, journal) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for (from, to) in [
        ([8.0, 1.65, 8.0], [8.0, 1.0, 10.0]),
        ([8.0, 1.65, 8.0], [10.0, 1.0, 8.0]),
        ([8.0, 1.65, 8.0], [6.0, 1.0, 8.0]),
        ([8.0, 1.65, 8.0], [8.0, 1.0, 6.0]),
    ] {
        assert!(visible(&habitat, from, to), "{from:?} -> {to:?}");
        assert!(visible(&habitat, to, from), "{to:?} -> {from:?}");
    }
    assert!(!visible(&habitat, [8.0, 1.65, 8.0], [8.0, 0.9, 10.0]));
}

#[test]
fn a_hidden_nearer_player_does_not_mask_a_visible_enemy() {
    let (terrain, chunks, mut journal) = fixture();
    journal.record([8, 1, 7], 3).unwrap();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let players = [[8.5, 1.7, 6.5], [11.5, 1.7, 8.5], [f64::NAN, 1.7, 8.5]];
    assert_eq!(
        nearest_visible(&h, [8.5, 1.7, 8.5], [8.5, 1.0, 8.5], &players, 16.0, |_| {
            true
        }),
        Some((1, players[1]))
    );
}

#[test]
fn an_open_torso_below_a_blocked_strike_endpoint_does_not_mask_an_attackable_enemy() {
    let (terrain, chunks, mut journal) = fixture();
    journal.record([8, 2, 7], MaterialId::Stone as u16).unwrap();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let players = [[8.5, 1.9, 6.5], [11.0, 1.9, 8.5]];
    let eye = [8.5, 1.65, 8.5];
    let origin = [8.5, 1.0, 8.5];
    assert_eq!(
        nearest_visible(&habitat, eye, origin, &players, 16.0, |_| true),
        Some((0, players[0]))
    );
    assert_eq!(
        nearest_visible_at(
            &habitat,
            eye,
            origin,
            &players,
            16.0,
            |_| true,
            |p| [p[0], p[1] + 0.65, p[2]]
        ),
        Some((1, players[1]))
    );
}

#[test]
fn forest_enemies_investigate_last_seen_position_then_stop_tracking_through_walls() {
    let (terrain, chunks, mut journal) = fixture();
    let mut a = Creature {
        id: creature_id([0, 0], 0).unwrap(),
        kind: CreatureKind::BloodStalker,
        home: [0, 0],
        position: [8.5, 1.0, 8.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: 30.0,
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::BloodStalker),
    };
    let mut nav = super::super::navigation::Navigation::default();
    super::super::forest::step(
        &mut a,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        &[[8.5, 1.9, 4.5]],
        0.1,
        true,
        Some(&mut nav),
    );
    assert_eq!(a.behavior, CreatureBehavior::Hunt);
    for x in 0..16 {
        for y in 1..8 {
            journal.record([x, y, 7], 3).unwrap();
        }
    }
    for _ in 0..10 {
        assert_eq!(
            super::super::forest::step(
                &mut a,
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal
                },
                &[[14.5, 1.9, 4.5]],
                0.1,
                false,
                Some(&mut nav)
            ),
            super::super::bears::Effect::None
        );
        let destination = nav.inspection(&a).destination.expect("last seen location");
        assert!(distance(destination, [8.5, 1.0, 4.5]) < 1e-12);
    }
    for _ in 0..45 {
        super::super::forest::step(
            &mut a,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            &[[14.5, 1.9, 4.5]],
            0.1,
            false,
            Some(&mut nav),
        );
    }
    assert!(!matches!(
        a.behavior,
        CreatureBehavior::Hunt | CreatureBehavior::Attack
    ));
    assert!(a.position[2] >= 8.0);
    nav.see_enemy(&a, [8.5, 1.0, 4.5]);
    assert_eq!(nav.remembered_enemy(&a), Some([8.5, 1.0, 4.5]));
    nav.forget(a.id);
    assert_eq!(nav.remembered_enemy(&a), None);
}
