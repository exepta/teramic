//! Player save coordinates share the normal character collision geometry.
use super::{Peer, ServerCharacter, spawn::LoadedSpawn};
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use std::collections::HashMap;
use terra_core::{GameConfig, PlayerLocation, block_properties::BlockPropertiesCatalog};
use terra_physics::character::{CharacterGrounded, PLAYER_HALF_HEIGHT, PLAYER_RADIUS};
use terra_world::{
    TerraGenerator, edits::EditJournal, generation::sections::SectionGenerator,
    streaming::ChunkStore,
};

fn bounds(position: Vec3) -> (Vec3, Vec3) {
    let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
    (position - half, position + half)
}

fn cells(position: Vec3) -> impl Iterator<Item = [i64; 3]> {
    let (low, high) = bounds(position);
    cells_in(low, high)
}

fn cells_in(low: Vec3, high: Vec3) -> impl Iterator<Item = [i64; 3]> {
    let low = low.floor().as_i64vec3();
    let high = (high - Vec3::splat(0.001)).floor().as_i64vec3();
    (low.z..=high.z).flat_map(move |z| {
        (low.y..=high.y).flat_map(move |y| (low.x..=high.x).map(move |x| [x, y, z]))
    })
}

fn recovery_clear(
    from: Vec3,
    to: Vec3,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
) -> bool {
    let (from_low, from_high) = bounds(from);
    let (to_low, to_high) = bounds(to);
    let mut obstacles = Vec::new();
    for p in cells_in(from_low.min(to_low), from_high.max(to_high)) {
        if !terra_world::interaction_shapes::visit_boxes_with(
            journal,
            properties,
            p,
            &mut block_at,
            |shape| obstacles.push(shape),
        ) {
            let Some(material) = block_at(p).and_then(terra_voxel::MaterialId::from_raw) else {
                return false;
            };
            if material.is_solid() {
                obstacles.push(terra_core::block_shapes::ShapeBox {
                    minimum: p.map(|v| v as f64),
                    maximum: p.map(|v| v as f64 + 1.0),
                    hinged: false,
                });
            }
        }
    }
    let overlap = |shape: &terra_core::block_shapes::ShapeBox, position| {
        let (low, high) = bounds(position);
        (0..3)
            .map(|i| {
                (shape.maximum[i].min(f64::from(high[i])) - shape.minimum[i].max(f64::from(low[i])))
                    .max(0.0)
            })
            .product::<f64>()
    };
    let steps = (from.distance(to) / 0.1).ceil().max(1.0) as u32;
    for shape in obstacles {
        let mut previous = overlap(&shape, from);
        for step in 1..=steps {
            let next = overlap(&shape, from.lerp(to, step as f32 / steps as f32));
            // Recovery may leave an existing penetration, never deepen one or
            // enter a new obstacle on its way to an otherwise clear endpoint.
            if next > previous + 1e-8 {
                return false;
            }
            previous = next;
        }
        if previous > 1e-8 {
            return false;
        }
    }
    true
}

pub(super) fn resolve(
    mut location: PlayerLocation,
    terrain: &TerraGenerator,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    spectator: bool,
    can_fly: bool,
) -> Option<PlayerLocation> {
    if !location.valid() {
        return None;
    }
    location.flying &= can_fly;
    if spectator {
        location.flying = true;
        return Some(location);
    }
    let generator = SectionGenerator::new(terrain.clone());
    let mut sampled = HashMap::new();
    let mut block_at = |p| {
        *sampled
            .entry(p)
            .or_insert_with(|| journal.block(p).or_else(|| generator.block_at(p).ok()))
    };
    let original = Vec3::from_array(location.position);
    let (low, high) = bounds(original);
    let containing: Vec<_> = journal
        .gameplay()
        .trains
        .vehicles
        .values()
        .filter(|car| {
            car.parts.iter().enumerate().any(|(index, _)| {
                car.transformed_shapes(index).iter().any(|shape| {
                    shape.intersects(
                        low.to_array().map(f64::from),
                        high.to_array().map(f64::from),
                    )
                })
            })
        })
        .map(|car| car.id)
        .collect();
    // Only nearby clearance recovery; never relocate to a different cave or surface.
    let mut offsets = vec![
        Vec3::ZERO,
        Vec3::Y * 0.5,
        Vec3::Y,
        Vec3::Y * 2.0,
        Vec3::X,
        -Vec3::X,
        Vec3::Z,
        -Vec3::Z,
    ];
    // Old saves may contain a mounted position without a persistent seat binding.
    // Recover outside that vehicle, using the same collision volumes as spawning.
    for id in &containing {
        let extent = journal.gameplay().trains.vehicles[id].model_bounds();
        let margin = PLAYER_RADIUS + 0.15;
        offsets.extend([
            Vec3::new(extent[0][0] as f32 - margin - original.x, 0.0, 0.0),
            Vec3::new(extent[1][0] as f32 + margin - original.x, 0.0, 0.0),
            Vec3::new(0.0, 0.0, extent[0][2] as f32 - margin - original.z),
            Vec3::new(0.0, 0.0, extent[1][2] as f32 + margin - original.z),
            Vec3::Y * (extent[1][1] as f32 + PLAYER_HALF_HEIGHT + 0.15 - original.y),
        ]);
    }
    for offset in offsets {
        let candidate = original + offset;
        let steps = (offset.length() / 0.1).ceil().max(1.0) as u32;
        let train_clear = (0..=steps).all(|step| {
            let position = original.lerp(candidate, step as f32 / steps as f32);
            let (low, high) = bounds(position);
            !journal.gameplay().trains.vehicles.values().any(|car| {
                (step == steps || !containing.contains(&car.id))
                    && car.parts.iter().enumerate().any(|(index, _)| {
                        car.transformed_shapes(index).iter().any(|shape| {
                            shape.intersects(
                                low.to_array().map(f64::from),
                                high.to_array().map(f64::from),
                            )
                        })
                    })
            })
        });
        let clear =
            train_clear && recovery_clear(original, candidate, journal, properties, &mut block_at);
        if clear {
            location.position = candidate.to_array();
            return location.valid().then_some(location);
        }
    }
    None
}

pub(super) fn loaded_state(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    position: Vec3,
    spectator: bool,
) -> LoadedSpawn {
    if cells(position).any(|p| chunks.block_at(p).is_none()) {
        return LoadedSpawn::Waiting;
    }
    if !spectator
        && !terra_physics::character::space_clear_with_shapes(chunks, journal, properties, position)
    {
        return LoadedSpawn::Invalid;
    }
    LoadedSpawn::Ready
}

pub(super) fn restore(
    mut peers: Query<&mut Peer>,
    mut bodies: Query<
        (&mut Transform, &mut LinearVelocity, &mut CharacterGrounded),
        With<ServerCharacter>,
    >,
) {
    for mut peer in &mut peers {
        let Some(location) = peer.restore_location else {
            continue;
        };
        let Ok((mut transform, mut velocity, mut grounded)) = bodies.get_mut(peer.body) else {
            continue;
        };
        transform.translation = Vec3::from_array(location.position);
        velocity.0 = Vec3::ZERO;
        grounded.0 = false;
        peer.restore_location = None;
    }
}

pub(super) fn capture(
    peers: Query<&Peer>,
    bodies: Query<&Transform, With<ServerCharacter>>,
    config: Res<GameConfig>,
    seated: Option<Res<super::trains::SeatedPlayers>>,
    mut journal: ResMut<EditJournal>,
) {
    for peer in &peers {
        if !peer.accepted
            || !peer.synced
            || !peer.spawned
            || peer.vitals.dead()
            || peer.chat.teleport.is_some()
            || peer.restore_location.is_some()
            || seated.as_ref().is_some_and(|seats| {
                seats.binding(peer.uuid).is_some() || seats.has_exit(peer.uuid)
            })
        {
            continue;
        }
        let Ok(body) = bodies.get(peer.body) else {
            continue;
        };
        let location = PlayerLocation {
            position: body.translation.to_array(),
            look: peer.look,
            flying: peer.can_fly(&config)
                && (peer.last_movement.flight_speed.is_some()
                    || peer.game_mode(&config) == terra_core::GameMode::Spectator),
        };
        if let Err(error) = journal
            .bypass_change_detection()
            .update_player_location(peer.uuid, location)
        {
            tracing::warn!(%error, player = %peer.uuid, "Cannot capture player location");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_cannot_cross_a_thin_roof_even_if_its_endpoint_is_clear() {
        let mut journal = EditJournal::default();
        let roof = [-1, -38, -1];
        journal.record(roof, 98).unwrap();
        let properties = BlockPropertiesCatalog::default();
        let from = Vec3::new(-0.5, -38.8, -0.5);
        let lookup = |p| Some(if p == roof { 98 } else { 0 });
        assert!(recovery_clear(
            from + Vec3::Y * 2.0,
            from + Vec3::Y * 2.0,
            &journal,
            &properties,
            lookup
        ));
        assert!(!recovery_clear(
            from,
            from + Vec3::Y * 2.0,
            &journal,
            &properties,
            lookup
        ));
        assert!(recovery_clear(
            from,
            from - Vec3::Y,
            &journal,
            &properties,
            lookup
        ));
    }

    #[test]
    fn old_seated_save_recovers_outside_the_train_using_spawn_collision_geometry() {
        use terra_core::{
            train_blueprint,
            trains::{TrainPart, Vehicle, VehicleKind},
        };
        let terrain = TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let properties = BlockPropertiesCatalog::default();
        let mut journal = EditJournal::default();
        for x in -6..=6 {
            for z in -6..=12 {
                for y in -41..=-28 {
                    journal
                        .record([x, y, z], if y == -41 { 3 } else { 0 })
                        .unwrap();
                }
            }
        }
        let guide = train_blueprint::building_template_with_width(
            [0, -39, 0],
            0,
            VehicleKind::Locomotive,
            4,
            3,
        );
        let mut car = Vehicle {
            name: String::new(),
            id: 1,
            owner: terra_core::PlayerUuid::from_u128(222),
            kind: VehicleKind::Locomotive,
            anchor: [0, -39, 0],
            parts: guide
                .iter()
                .map(|p| TrainPart {
                    offset: [p.cell[0] as i16, (p.cell[1] + 39) as i16, p.cell[2] as i16],
                    material: p.material,
                    facing: p.facing,
                    wood: Default::default(),
                })
                .collect(),
            cargo: Default::default(),
            blocks: Default::default(),
            payloads: Default::default(),
            motion: Default::default(),
            burn_seconds: 0.0,
            steam: Default::default(),
            couplings: [None; 2],
            revision: 1,
        };
        for index in car.cargo_indices().collect::<Vec<_>>() {
            car.cargo.insert(index, terra_core::Inventory::default());
        }
        let index = car
            .parts
            .iter()
            .position(|p| p.material == terra_core::train_parts::CAB)
            .unwrap();
        let (feet, _) = car.seat_pose(index).unwrap();
        let location = PlayerLocation {
            position: [
                feet[0] as f32,
                feet[1] as f32 + PLAYER_HALF_HEIGHT,
                feet[2] as f32,
            ],
            look: [0.0; 2],
            flying: false,
        };
        journal.gameplay_mut().trains.insert(car).unwrap();
        let restored = resolve(location, &terrain, &journal, &properties, false, false)
            .expect("legacy mounted save is recoverable");
        assert_ne!(restored.position, location.position);
        let (low, high) = bounds(Vec3::from_array(restored.position));
        assert!(
            terra_world::trains::boxes_in(
                &journal,
                low.to_array().map(f64::from),
                high.to_array().map(f64::from)
            )
            .is_empty()
        );
        assert!(recovery_clear(
            Vec3::from_array(location.position),
            Vec3::from_array(restored.position),
            &journal,
            &properties,
            |p| journal.block(p)
        ));
    }

    #[test]
    fn saved_cave_clearance_uses_authored_geometry_and_authorized_flight() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let properties = BlockPropertiesCatalog::default();
        let mut journal = EditJournal::default();
        for x in -3..=3 {
            for z in -3..=3 {
                for y in -40..=-34 {
                    journal.record([x, y, z], 0).unwrap();
                }
            }
        }
        let location = PlayerLocation {
            position: [-0.5, -38.5, -0.5],
            look: [-2.0, 0.4],
            flying: true,
        };
        let restored = resolve(location, &terrain, &journal, &properties, false, false).unwrap();
        assert_eq!(restored.position, location.position);
        assert_eq!(restored.look, location.look);
        assert!(!restored.flying);
        journal.record([-1, -39, -1], 3).unwrap();
        let recovered = resolve(location, &terrain, &journal, &properties, false, true).unwrap();
        assert_ne!(recovered.position, location.position);
        assert!(recovered.flying);
        for x in -3..=3 {
            for z in -3..=3 {
                journal.record([x, -38, z], 3).unwrap();
            }
        }
        assert!(
            resolve(location, &terrain, &journal, &properties, false, false).is_none(),
            "a clear endpoint above a new ceiling is not a safe recovery path"
        );
        for x in -3..=3 {
            for z in -3..=3 {
                for y in -40..=-34 {
                    journal.record([x, y, z], 3).unwrap();
                }
            }
        }
        assert!(resolve(location, &terrain, &journal, &properties, false, false).is_none());
        assert_eq!(
            resolve(location, &terrain, &journal, &properties, true, true),
            Some(location)
        );
    }
}
