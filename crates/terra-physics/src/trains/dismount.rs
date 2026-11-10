//! Shared terrain-checked dismount search for authority and saved-world diagnostics.
use bevy::prelude::*;
use terra_core::trains::Vehicle;
use terra_world::{edits::EditJournal, streaming::ChunkStore};
#[derive(Clone, Copy)]
struct ExitIntent {
    vehicle: u64,
    feet: [f64; 3],
    yaw: f32,
}
fn overlaps_vehicle(car: &Vehicle, low: [f64; 3], high: [f64; 3]) -> bool {
    let bounds = car.model_bounds();
    if !(0..3).all(|axis| bounds[0][axis] < high[axis] && bounds[1][axis] > low[axis]) {
        return false;
    }
    car.parts.iter().enumerate().any(|(index, _part)| {
        car.transformed_shapes(index)
            .into_iter()
            .any(|shape| shape.intersects(low, high))
    })
}
fn exit_clear(
    intent: ExitIntent,
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    center: Vec3,
    ignore_mount: bool,
) -> bool {
    let half = Vec3::new(
        crate::character::PLAYER_RADIUS,
        crate::character::PLAYER_HALF_HEIGHT,
        crate::character::PLAYER_RADIUS,
    );
    let low = (center - half).to_array().map(f64::from);
    let high = (center + half).to_array().map(f64::from);
    for x in low[0].floor() as i64..=(high[0] - 1e-6).floor() as i64 {
        for y in low[1].floor() as i64..=(high[1] - 1e-6).floor() as i64 {
            for z in low[2].floor() as i64..=(high[2] - 1e-6).floor() as i64 {
                if terra_world::interaction_shapes::space_intersects(
                    chunks,
                    journal,
                    properties,
                    [x, y, z],
                    low,
                    high,
                ) {
                    return false;
                }
            }
        }
    }
    !journal
        .gameplay()
        .trains
        .vehicles
        .values()
        .any(|car| (!ignore_mount || car.id != intent.vehicle) && overlaps_vehicle(car, low, high))
}
/// Find a loaded, collision-free dismount beside a vehicle, including elevated cabs.
pub fn safe_exit(
    vehicle: u64,
    feet: [f64; 3],
    yaw: f32,
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &terra_world::TerraGenerator,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
) -> Option<Vec3> {
    let intent = ExitIntent { vehicle, feet, yaw };
    let habitat = terra_world::creatures::Habitat {
        terrain,
        chunks,
        journal,
    };
    let (sin, cos) = f64::from(intent.yaw).sin_cos();
    for distance in [1.2, 1.8, 2.4, 3.2, 4.0, 5.0, 6.0, 8.0] {
        for [x, z] in [
            [1.0, 0.0],
            [-1.0, 0.0],
            [0.0, 1.0],
            [0.0, -1.0],
            [1.0, 1.0],
            [-1.0, 1.0],
            [1.0, -1.0],
            [-1.0, -1.0],
        ] {
            for vertical in [
                0.5, -0.5, -1.5, -2.5, 1.5, -3.5, -4.5, -5.5, -6.5, -7.5, -8.5,
            ] {
                let probe = [
                    intent.feet[0] + distance * (x * cos + z * sin),
                    intent.feet[1] + vertical,
                    intent.feet[2] + distance * (-x * sin + z * cos),
                ];
                let Some(feet) = habitat.ground_sized(probe, [0.64, 1.8, 0.64], 0.0) else {
                    continue;
                };
                let target = Vec3::from_array(feet.map(|v| v as f32))
                    + Vec3::Y * crate::character::PLAYER_HALF_HEIGHT;
                if !exit_clear(intent, chunks, journal, properties, target, false) {
                    continue;
                }
                let start = Vec3::from_array(intent.feet.map(|v| v as f32))
                    + Vec3::Y * crate::character::PLAYER_HALF_HEIGHT;
                // Like rider dismount, the controlled transition ignores its
                // own mount. Terrain and every other vehicle still block it.
                let segment_clear = |from: Vec3, to: Vec3| {
                    let samples = (from.distance(to) / 0.125).ceil().max(1.0) as usize;
                    (0..=samples).all(|step| {
                        exit_clear(
                            intent,
                            chunks,
                            journal,
                            properties,
                            from.lerp(to, step as f32 / samples as f32),
                            true,
                        )
                    })
                };
                // Step out at cab height, then descend beside the body. A direct
                // diagonal from a tall cab can cut into a real embankment even
                // though the side and landing are both free.
                let outside = Vec3::new(target.x, start.y.max(target.y), target.z);
                if segment_clear(start, target)
                    || segment_clear(start, outside) && segment_clear(outside, target)
                {
                    return Some(target);
                }
            }
        }
    }
    // A carriage platform is a valid place to stand even above a ravine or
    // without a dry terrain landing. The old animal-ground search ignores the
    // vehicle itself and could therefore trap passengers on elevated track.
    let car = journal.gameplay().trains.vehicles.get(&vehicle)?;
    let floors = (0..car.parts.len())
        .flat_map(|index| car.transformed_shapes(index))
        .collect::<Vec<_>>();
    let start =
        Vec3::from_array(feet.map(|v| v as f32)) + Vec3::Y * crate::character::PLAYER_HALF_HEIGHT;
    let margin = f64::from(crate::character::PLAYER_RADIUS) * 0.75;
    for distance in [0.65, 0.9, 1.2, 1.8] {
        for [x, z] in [
            [1., 0.],
            [-1., 0.],
            [0., 1.],
            [0., -1.],
            [1., 1.],
            [-1., 1.],
            [1., -1.],
            [-1., -1.],
        ] {
            let [x, z] = [
                feet[0] + distance * (x * cos + z * sin),
                feet[2] + distance * (-x * sin + z * cos),
            ];
            for floor in &floors {
                if floor.maximum[1] < feet[1] - 1.5
                    || floor.maximum[1] > feet[1] + 0.6
                    || x - margin < floor.minimum[0]
                    || x + margin > floor.maximum[0]
                    || z - margin < floor.minimum[2]
                    || z + margin > floor.maximum[2]
                {
                    continue;
                }
                let target = Vec3::new(
                    x as f32,
                    (floor.maximum[1] + 0.003) as f32 + crate::character::PLAYER_HALF_HEIGHT,
                    z as f32,
                );
                if !exit_clear(intent, chunks, journal, properties, target, false) {
                    continue;
                }
                let samples = (start.distance(target) / 0.125).ceil().max(1.) as usize;
                if (0..=samples).all(|step| {
                    exit_clear(
                        intent,
                        chunks,
                        journal,
                        properties,
                        start.lerp(target, step as f32 / samples as f32),
                        true,
                    )
                }) {
                    return Some(target);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{
        Inventory, PlayerUuid, train_blueprint,
        trains::{TrainPart, VehicleKind},
    };
    use terra_voxel::{BlockSection, SectionPos};
    use terra_world::streaming::LoadedSection;

    #[test]
    fn passengers_can_leave_onto_their_platform_above_a_void_in_every_orientation() {
        for facing in 0..4 {
            let anchor = [7, 10, 7];
            let guide = train_blueprint::building_template_with_width(
                anchor,
                facing,
                VehicleKind::Passenger,
                4,
                3,
            );
            let mut car = Vehicle {
                name: String::new(),
                id: 1,
                owner: PlayerUuid::from_u128(224),
                kind: VehicleKind::Passenger,
                anchor,
                parts: guide
                    .into_iter()
                    .map(|part| TrainPart {
                        offset: std::array::from_fn(|axis| (part.cell[axis] - anchor[axis]) as i16),
                        material: part.material,
                        facing: part.facing,
                        wood: Default::default(),
                    })
                    .collect(),
                cargo: Default::default(),
                blocks: Default::default(),
                payloads: Default::default(),
                couplings: [None; 2],
                revision: 1,
                motion: Default::default(),
                burn_seconds: 0.,
                steam: Default::default(),
            };
            for index in car.cargo_indices().collect::<Vec<_>>() {
                car.cargo.insert(index, Inventory::default());
            }
            let index = car
                .parts
                .iter()
                .position(|part| part.material == 215)
                .unwrap();
            let (feet, yaw) = car.seat_pose(index).unwrap();
            let mut journal = EditJournal::default();
            journal.gameplay_mut().trains.insert(car).unwrap();
            let owner = SectionPos::default();
            let mut chunks = ChunkStore::default();
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
                water: false,
                ..Default::default()
            })
            .unwrap();
            let properties = Default::default();
            let target = safe_exit(1, feet, yaw, &chunks, &journal, &terrain, &properties)
                .expect("a real carriage floor is valid support without terrain beneath it");
            assert!(crate::character::space_clear_with_shapes(
                &chunks,
                &journal,
                &properties,
                target
            ));
            assert!((target.y - 11.1655).abs() < 0.01);
        }
    }
}
