use super::*;
use crate::{
    TerraGenerator,
    creatures::{Kind, step_budgeted},
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{
    WorldConfig,
    creatures::{CreatureNeeds, creature_id},
};
use terra_voxel::{BlockSection, SectionPos, global_to_section};

std::thread_local! {
    static WATER_RESOLUTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
pub(super) fn record_water_resolution() {
    WATER_RESOLUTIONS.set(WATER_RESOLUTIONS.get() + 1);
}

fn fixture(vertical_sections: i64, trees: bool) -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        water: false,
        trees: false,
        caves: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for x in -1..=2 {
        for z in -1..=2 {
            for y in 0..vertical_sections {
                let position = SectionPos::new(x, y, z);
                let mut data = BlockSection::empty(position).unwrap();
                if y == 0 {
                    for x in 0..16 {
                        for z in 0..16 {
                            data.set_block([x, 0, z], 1).unwrap();
                        }
                    }
                }
                chunks.loaded.insert(
                    position,
                    LoadedSection {
                        data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
        }
    }
    if trees {
        // Actual collidable voxel crowns, with navigable gaps between trees.
        // A bird starting at grass height must climb around them, not teleport.
        for x in [-8, 8, 24, 40] {
            for z in [-8, 8, 24, 40] {
                for y in 1..=12 {
                    put(&mut chunks, [x, y, z], 405);
                }
                for dy in 12..=15 {
                    for dx in -3_i64..=3 {
                        for dz in -3_i64..=3 {
                            if dx * dx + dz * dz <= 10 {
                                put(&mut chunks, [x + dx, dy, z + dz], 406);
                            }
                        }
                    }
                }
            }
        }
    }
    (terrain, chunks, EditJournal::default())
}

fn put(chunks: &mut ChunkStore, cell: [i64; 3], material: u16) {
    let (section, local) = global_to_section(cell);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
}

fn bird() -> Creature {
    Creature {
        id: creature_id([0, 0], 1).unwrap(),
        kind: Kind::Raven,
        home: [0, 0],
        position: [16.5, 2.0, 16.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: Kind::Raven.health(),
        behavior: Behavior::Fly,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(Kind::Raven),
    }
}

fn advance(a: &mut Creature, h: &Habitat, players: &[[f64; 3]]) {
    let previous = a.position;
    step_budgeted(a, h, players, 0.1, false);
    assert!(a.valid(), "{a:?}");
    assert!(
        h.clear_rotated(a.position, a.size(), false, f64::from(a.yaw)),
        "{a:?}"
    );
    assert!(
        (a.position[1] - previous[1]).abs() <= CLIMB_SPEED * 0.1 + 1e-6,
        "a raven must fly to its new altitude rather than teleport"
    );
}

#[test]
fn authoritative_raven_climbs_from_ground_and_cruises_above_actual_tree_crowns() {
    let mut final_heights = Vec::new();
    for trees in [false, true] {
        let (terrain, chunks, journal) = fixture(4, trees);
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut raven = bird();
        let mut high_frames = 0;
        let mut travelled = 0.0;
        for frame in 0..600 {
            let before = raven.position;
            advance(&mut raven, &h, &[]);
            travelled += (raven.position[0] - before[0]).hypot(raven.position[2] - before[2]);
            if frame >= 250 && raven.position[1] >= if trees { 25.0 } else { 12.0 } {
                high_frames += 1;
            }
        }
        assert!(
            high_frames >= 320,
            "raven stayed low ({trees} trees): {high_frames}/350 frames, {raven:?}"
        );
        assert!(
            travelled > 40.0,
            "high flight must include real horizontal travel"
        );
        final_heights.push(raven.position[1]);
    }
    assert!(
        final_heights[1] >= final_heights[0] + 10.0,
        "edited tree crowns must raise the cruising band: {final_heights:?}"
    );
}

#[test]
fn unknown_sky_and_solid_roofs_limit_the_climb_without_chunk_requests() {
    for roof in [false, true] {
        let (terrain, mut chunks, journal) = fixture(1, false);
        if roof {
            for x in -16..48 {
                for z in -16..48 {
                    put(&mut chunks, [x, 8, z], 3);
                }
            }
        }
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let count = chunks.loaded.len();
        let mut raven = bird();
        let mut highest = raven.position[1];
        for _ in 0..450 {
            advance(&mut raven, &h, &[]);
            highest = highest.max(raven.position[1]);
            assert!(raven.position[1] + raven.size()[1] < if roof { 8.02 } else { 16.0 });
        }
        assert!(highest > if roof { 6.0 } else { 12.0 });
        assert_eq!(chunks.loaded.len(), count);
        assert_eq!(chunks.wanted_sections().count(), 0);
    }
}

#[test]
fn ravens_land_on_support_briefly_rest_and_take_off_again() {
    let (terrain, chunks, journal) = fixture(4, false);
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut raven = bird();
    raven.position[1] = 20.0;
    raven.behavior = Behavior::Land;
    raven.timer = 26.0;
    let mut perched = false;
    let mut took_off = false;
    for _ in 0..320 {
        advance(&mut raven, &h, &[]);
        if raven.behavior == Behavior::Nectar {
            perched = true;
            assert!((raven.position[1] - 1.0).abs() < 0.001);
        }
        took_off |= perched && raven.behavior == Behavior::Fly && raven.position[1] > 12.0;
    }
    assert!(perched && took_off);
    raven.position = [16.5, 1.0, 16.5];
    raven.behavior = Behavior::Nectar;
    raven.timer = 5.0;
    raven.health -= 1.0;
    advance(&mut raven, &h, &[[16.5, 1.0, 17.5]]);
    assert_eq!(raven.behavior, Behavior::Flee);
    assert!(
        raven.position[1] > 1.0,
        "an injured perched raven immediately departs"
    );
}

#[test]
fn altitude_and_steering_continue_deterministically_after_save_reload() {
    let (terrain, chunks, journal) = fixture(4, true);
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut raven = bird();
    for _ in 0..300 {
        advance(&mut raven, &h, &[]);
    }
    assert!(raven.position[1] > 25.0);
    let bytes = serde_json::to_vec(&raven).unwrap();
    let mut restored: Creature = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(raven, restored);
    for _ in 0..200 {
        advance(&mut raven, &h, &[]);
        advance(&mut restored, &h, &[]);
        assert_eq!(raven, restored);
    }
}

#[test]
fn crossing_open_water_keeps_the_cruising_band_above_the_surface() {
    let (_, chunks, journal) = fixture(4, false);
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 3.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        trees: false,
        biomes: false,
        rivers: false,
        water: true,
        sea_level: 13.0,
        ..Default::default()
    })
    .unwrap();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(chunks.prepared_column([1, 1]).is_none());
    let mut raven = bird();
    raven.position[1] = 14.0;
    assert!(h.wet([16.5, 12.5, 16.5]));
    WATER_RESOLUTIONS.set(0);
    let high_column = column(&h, [16.5, 55.0, 16.5]).unwrap();
    assert_eq!(high_column.0, Some(13.0));
    assert_eq!(
        WATER_RESOLUTIONS.get(),
        1,
        "42 empty cells above water must share one natural-height resolution"
    );
    WATER_RESOLUTIONS.set(0);
    assert!(cruise_height(&raven, &h) >= 24.5);
    assert!(
        WATER_RESOLUTIONS.get() <= 10,
        "a complete height decision resolves each of its ten columns only once"
    );
    for _ in 0..250 {
        advance(&mut raven, &h, &[]);
    }
    assert!(
        raven.position[1] >= 24.5,
        "the bird must not descend toward the submerged lake bed"
    );
}

#[test]
fn cruising_waypoint_keeps_turning_across_ticks_and_simulation_step_sizes() {
    let (terrain, chunks, journal) = fixture(4, false);
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut initial = bird();
    initial.age = 1.0;
    initial.position[1] = 20.0;
    initial.timer = 5.0; // The old timer-gated code discarded this turn entirely.
    initial.yaw = (cruise_heading(&initial) + 1.8).rem_euclid(std::f32::consts::TAU);
    let mut raven = initial.clone();
    let start = raven.yaw;
    for _ in 0..4 {
        let before = raven.yaw;
        advance(&mut raven, &h, &[]);
        assert!(
            yaw_delta(before, raven.yaw).abs() > 0.15,
            "a persistent target must continue steering after the first tick"
        );
    }
    assert!(yaw_delta(start, raven.yaw).abs() > 0.7);

    let mut results = Vec::new();
    for (dt, steps) in [(0.05_f32, 120), (0.1, 60), (0.2, 30)] {
        let mut raven = initial.clone();
        for _ in 0..steps {
            step_budgeted(&mut raven, &h, &[], dt, false);
            assert!(h.clear_rotated(raven.position, raven.size(), false, f64::from(raven.yaw)));
        }
        assert!((raven.age - 7.0).abs() < 1e-5);
        results.push(raven);
    }
    for other in &results[1..] {
        assert!(
            distance(results[0].position, other.position).sqrt() < 0.6,
            "equal simulated time should give comparable courses, not one turn per frame"
        );
        assert!(yaw_delta(results[0].yaw, other.yaw).abs() < 0.2);
    }
}
