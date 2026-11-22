//! Run-over damage uses only the physical sweeps of committed train movement.
use super::*;
use terra_core::{
    DamageCause, ItemKind, ItemStack, PlayerUuid,
    block_shapes::ShapeBox,
    creatures::{AnimalProduct, Creature, CreatureBehavior, CreatureKind},
    trains::Vehicle,
};

const HIT_INTERVAL: f64 = 0.75;
const MIN_SPEED: f64 = 0.25;

pub(super) struct Impact {
    bounds: ShapeBox,
    shapes: Vec<ShapeBox>,
    damage: f32,
}
impl Impact {
    /// Wheel distance excludes teleports, turntable rotation and a rejected rail step.
    pub(super) fn moving(old: &Vehicle, next: &Vehicle, shapes: Vec<ShapeBox>) -> Option<Self> {
        let speed = old.motion.speed_mps.abs().max(next.motion.speed_mps.abs());
        if speed < MIN_SPEED
            || (next.motion.wheel_distance_m - old.motion.wheel_distance_m).abs() < 1e-6
            || shapes.is_empty()
        {
            return None;
        }
        let bounds = ShapeBox {
            minimum: std::array::from_fn(|axis| {
                shapes
                    .iter()
                    .map(|s| s.minimum[axis])
                    .fold(f64::INFINITY, f64::min)
            }),
            maximum: std::array::from_fn(|axis| {
                shapes
                    .iter()
                    .map(|s| s.maximum[axis])
                    .fold(f64::NEG_INFINITY, f64::max)
            }),
            hinged: false,
        };
        Some(Self {
            bounds,
            shapes,
            damage: (speed * 10.0).clamp(5.0, 100.0) as f32,
        })
    }
    fn hits(&self, low: [f64; 3], high: [f64; 3]) -> bool {
        self.bounds.intersects(low, high)
            && self.shapes.iter().any(|shape| shape.intersects(low, high))
    }
    fn hits_creature(&self, animal: &Creature) -> bool {
        // Anatomy accounts for juvenile size, swimming bodies and sleeping villagers.
        terra_core::creatures::hitboxes::for_creature(animal)
            .iter()
            .any(|part| {
                let corners = part.corners();
                let low = std::array::from_fn(|axis| {
                    corners
                        .iter()
                        .map(|p| p[axis])
                        .fold(f64::INFINITY, f64::min)
                });
                let high = std::array::from_fn(|axis| {
                    corners
                        .iter()
                        .map(|p| p[axis])
                        .fold(f64::NEG_INFINITY, f64::max)
                });
                self.hits(low, high)
            })
    }
}

#[derive(Resource, Default)]
pub(super) struct Impacts(pub Vec<Impact>);

#[derive(Default)]
pub(super) struct Cooldowns {
    clock: f64,
    players: HashMap<PlayerUuid, f64>,
    creatures: HashMap<u64, f64>,
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct DamageWorld<'w, 's> {
    journal: ResMut<'w, EditJournal>,
    config: Res<'w, GameConfig>,
    seats: Res<'w, trains::SeatedPlayers>,
    mounts: ResMut<'w, riding::Mounts>,
    peers: Query<'w, 's, &'static mut Peer, With<Connected>>,
    bodies: Query<'w, 's, &'static Transform, With<ServerCharacter>>,
}

pub(super) fn tick(
    time: Res<Time<Fixed>>,
    mut impacts: ResMut<Impacts>,
    mut cooldowns: Local<Cooldowns>,
    mut world: DamageWorld,
) {
    cooldowns.clock += time.delta_secs_f64().min(0.1);
    let now = cooldowns.clock;
    cooldowns.players.retain(|_, until| *until > now);
    cooldowns.creatures.retain(|_, until| *until > now);
    let impacts = std::mem::take(&mut impacts.0);
    if impacts.is_empty() {
        return;
    }
    for mut peer in &mut world.peers {
        if !peer.synced
            || peer.vitals.dead()
            || peer.chat.teleport.is_some()
            || world.seats.binding(peer.uuid).is_some()
            || cooldowns.players.contains_key(&peer.uuid)
        {
            continue;
        }
        let Ok(body) = world.bodies.get(peer.body) else {
            continue;
        };
        let center = body.translation.to_array().map(f64::from);
        let half = [
            f64::from(terra_physics::character::PLAYER_RADIUS),
            f64::from(terra_physics::character::PLAYER_HALF_HEIGHT),
            f64::from(terra_physics::character::PLAYER_RADIUS),
        ];
        let low = std::array::from_fn(|i| center[i] - half[i]);
        let high = std::array::from_fn(|i| center[i] + half[i]);
        let damage = impacts
            .iter()
            .filter(|impact| impact.hits(low, high))
            .map(|impact| impact.damage)
            .fold(0.0, f32::max);
        if damage == 0.0 {
            continue;
        }
        let health = peer.vitals.health;
        let mode = peer.game_mode(&world.config);
        survival::environmental_damage(&mut peer, damage, mode, DamageCause::Train);
        if peer.vitals.health < health {
            cooldowns.players.insert(peer.uuid, now + HIT_INTERVAL);
        }
    }
    // Broad-phase rejects distant populations before constructing anatomical hitboxes.
    let hits: Vec<_> = world
        .journal
        .gameplay()
        .creatures
        .animals
        .values()
        .filter(|animal| animal.health > 0.0 && !cooldowns.creatures.contains_key(&animal.id))
        .filter_map(|animal| {
            let size = animal.size();
            let margin = size.into_iter().fold(0.0, f64::max) * 1.5 + 1.0;
            let low = animal.position.map(|p| p - margin);
            let high = animal.position.map(|p| p + margin);
            let damage = impacts
                .iter()
                .filter(|impact| {
                    impact.bounds.intersects(low, high) && impact.hits_creature(animal)
                })
                .map(|impact| impact.damage)
                .fold(0.0, f32::max);
            (damage > 0.0).then_some((animal.id, damage))
        })
        .collect();
    for (id, damage) in hits {
        if hurt_creature(world.journal.gameplay_mut(), id, damage) {
            cooldowns.creatures.insert(id, now + HIT_INTERVAL);
            if let Some(animal) = world.journal.gameplay().creatures.animals.get(&id) {
                // A mounted horse also exists in the session runtime. Never resurrect
                // a struck horse from that copy when the next movement step publishes.
                for mount in world
                    .mounts
                    .active
                    .values_mut()
                    .filter(|m| m.horse.id == id)
                {
                    mount.horse = animal.clone();
                    mount.leave |= animal.health == 0.0;
                }
            }
        }
    }
}

pub(super) fn hurt_creature(game: &mut terra_core::GameplayState, id: u64, damage: f32) -> bool {
    let Some(animal) = game.creatures.animals.get(&id).filter(|a| a.health > 0.0) else {
        return false;
    };
    let damage = terra_core::creatures::dragons::incoming_damage(animal, damage);
    let dies = damage >= animal.health;
    let position = animal.position;
    let mut drops = Vec::new();
    if dies && let Some(product) = animal.product() {
        drops.push(ItemStack {
            item: ItemKind::Animal(product),
            count: if animal.kind == CreatureKind::Fish {
                1
            } else {
                2
            },
        });
        if let Some(product) = match animal.kind {
            CreatureKind::Sheep => Some(AnimalProduct::Wool),
            CreatureKind::Cow => Some(AnimalProduct::Leather),
            _ => None,
        } {
            drops.push(ItemStack {
                item: ItemKind::Animal(product),
                count: 1,
            });
        }
    }
    if dies {
        drops.extend(
            terra_core::creatures::drops::forest_loot(animal.kind, animal.variant, animal.id)
                .into_iter()
                .flatten(),
        );
    }
    if game.drops.len() + drops.len() > terra_core::MAX_WORLD_DROPS
        || game.next_drop.checked_add(drops.len() as u64).is_none()
    {
        return false;
    }
    let animal = game.creatures.animals.get_mut(&id).unwrap();
    animal.health = (animal.health - damage).max(0.0);
    animal.behavior = if dies {
        CreatureBehavior::Dead
    } else {
        CreatureBehavior::Flee
    };
    animal.timer = 5.0;
    animal.velocity = [0.0; 3];
    for stack in drops {
        let accepted = game.drop_stack(position, stack, [0.0, 1.8, 0.0], 0.35);
        debug_assert!(accepted);
    }
    game.revision = game.revision.saturating_add(1);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{
        GameMode, WoodSpecies, train_parts,
        trains::{TrainPart, VehicleKind},
    };

    fn car() -> Vehicle {
        Vehicle {
            name: String::new(),
            id: 1,
            owner: PlayerUuid::from_u128(1),
            kind: VehicleKind::Locomotive,
            anchor: [0, 3, 0],
            parts: vec![TrainPart {
                offset: [0, 0, 0],
                material: train_parts::FRAME,
                facing: 0,
                wood: WoodSpecies::Oak,
            }],
            cargo: Default::default(),
            blocks: Default::default(),
            payloads: Default::default(),
            motion: Default::default(),
            burn_seconds: 0.0,
            steam: Default::default(),
            couplings: [None; 2],
            revision: 0,
        }
    }
    fn sweep() -> Impact {
        let mut old = car();
        old.motion.speed_mps = 3.0;
        let mut next = old.clone();
        next.motion.translation[2] = 4.0;
        next.motion.wheel_distance_m = 4.0;
        Impact::moving(
            &old,
            &next,
            vec![ShapeBox {
                minimum: [-0.6, 0.0, -2.0],
                maximum: [0.6, 3.0, 2.0],
                hinged: false,
            }],
        )
        .unwrap()
    }
    fn fixture() -> App {
        let mut app = App::new();
        let mut time = Time::<Fixed>::from_hz(10.0);
        time.advance_by(Duration::from_millis(100));
        app.insert_resource(time)
            .init_resource::<GameConfig>()
            .init_resource::<EditJournal>()
            .init_resource::<trains::SeatedPlayers>()
            .init_resource::<riding::Mounts>()
            .init_resource::<Impacts>()
            .add_systems(Update, tick);
        app
    }
    fn player(app: &mut App, mode: GameMode, at: Vec3, health: f32) -> Entity {
        let mut peer = super::super::survival::tests::resting_peer(health);
        peer.mode = Some(mode);
        peer.body = app
            .world_mut()
            .spawn((
                ServerCharacter {
                    link: Entity::PLACEHOLDER,
                },
                Transform::from_translation(at),
            ))
            .id();
        let transport_id = peer.body.to_bits();
        let connection = app
            .world_mut()
            .spawn((RemoteId(PeerId::Netcode(transport_id)), peer))
            .id();
        app.world_mut().entity_mut(connection).insert(Connected);
        connection
    }
    fn animal(id: u64, kind: CreatureKind, position: [f64; 3]) -> Creature {
        Creature {
            id,
            kind,
            home: [0, 0],
            position,
            velocity: [0.0; 3],
            yaw: 0.0,
            health: kind.health(),
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: Default::default(),
        }
    }
    fn frame(app: &mut App) {
        // Overlapping cars/parts count as one impact per creature and interval.
        app.world_mut().resource_mut::<Impacts>().0 = vec![sweep(), sweep()];
        app.update();
    }
    #[test]
    fn committed_train_hits_use_survival_immunity_death_and_bounded_repeat_damage() {
        let mut app = fixture();
        let on_track = player(&mut app, GameMode::Survival, Vec3::Y, 100.0);
        let beside = player(
            &mut app,
            GameMode::Survival,
            Vec3::new(2.0, 1.0, 0.0),
            100.0,
        );
        let creative = player(&mut app, GameMode::Creative, Vec3::Y, 100.0);
        let spectator = player(&mut app, GameMode::Spectator, Vec3::Y, 100.0);
        let protected = player(&mut app, GameMode::Survival, Vec3::Y, 100.0);
        app.world_mut()
            .get_mut::<Peer>(protected)
            .unwrap()
            .survival
            .protection = 2.0;
        let rider = player(&mut app, GameMode::Survival, Vec3::Y, 100.0);
        let rider_uuid = app.world().get::<Peer>(rider).unwrap().uuid;
        app.world_mut()
            .resource_mut::<trains::SeatedPlayers>()
            .bind_for_test(
                rider_uuid,
                terra_core::trains::SeatBinding {
                    player: rider_uuid,
                    vehicle: 1,
                    part: 0,
                },
            );
        let doomed = player(&mut app, GameMode::Survival, Vec3::Y, 20.0);
        frame(&mut app);
        assert_eq!(
            app.world().get::<Peer>(on_track).unwrap().vitals.health,
            70.0
        );
        for id in [beside, creative, spectator, protected, rider] {
            assert_eq!(app.world().get::<Peer>(id).unwrap().vitals.health, 100.0);
        }
        let dead = app.world().get::<Peer>(doomed).unwrap();
        assert!(dead.vitals.dead());
        assert_eq!(dead.vitals.last_damage, Some(DamageCause::Train));
        assert_eq!(dead.survival.damage_epoch, 1);
        for _ in 0..6 {
            frame(&mut app);
        }
        assert_eq!(
            app.world().get::<Peer>(on_track).unwrap().vitals.health,
            70.0
        );
        frame(&mut app);
        frame(&mut app);
        assert_eq!(
            app.world().get::<Peer>(on_track).unwrap().vitals.health,
            40.0
        );
        assert_eq!(
            app.world()
                .get::<Peer>(doomed)
                .unwrap()
                .survival
                .damage_epoch,
            1
        );
        assert!(app.world().resource::<Impacts>().0.is_empty());
    }
    #[test]
    fn animals_hostile_mobs_and_mounted_horses_take_hits_with_single_normal_death_drops() {
        let mut app = fixture();
        let sheep = animal(1, CreatureKind::Sheep, [0.0; 3]);
        let mut spider = animal(2, CreatureKind::LargeSpider, [0.0; 3]);
        spider.health = 100.0;
        let outside = animal(3, CreatureKind::Cow, [5.0, 0.0, 0.0]);
        let mut horse = animal(4, CreatureKind::Horse, [0.0; 3]);
        horse.health = 1.0;
        app.world_mut()
            .resource_mut::<riding::Mounts>()
            .active
            .insert(
                PlayerUuid::from_u128(44),
                riding::Mount {
                    horse: horse.clone(),
                    stamina: 100.0,
                    grounded: true,
                    leave: false,
                },
            );
        for animal in [sheep, spider, outside, horse] {
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .creatures
                .animals
                .insert(animal.id, animal);
        }
        frame(&mut app);
        let game = app.world().resource::<EditJournal>().gameplay();
        assert_eq!(game.creatures.animals[&1].behavior, CreatureBehavior::Dead);
        assert_eq!(game.creatures.animals[&2].health, 70.0);
        assert_eq!(game.creatures.animals[&2].behavior, CreatureBehavior::Flee);
        assert_eq!(
            game.creatures.animals[&3].health,
            CreatureKind::Cow.health()
        );
        assert_eq!(game.creatures.animals[&4].behavior, CreatureBehavior::Dead);
        let drops = game.drops.len();
        assert!(drops >= 2, "A sheep must retain meat and wool drops");
        let mount = &app.world().resource::<riding::Mounts>().active[&PlayerUuid::from_u128(44)];
        assert_eq!(mount.horse.health, 0.0);
        assert!(mount.leave);
        frame(&mut app);
        assert_eq!(
            app.world().resource::<EditJournal>().gameplay().drops.len(),
            drops
        );
    }
    #[test]
    fn new_forest_death_loot_is_authoritative_and_not_repeated() {
        for kind in [
            CreatureKind::Skeleton,
            CreatureKind::Raven,
            CreatureKind::BloodSlime,
            CreatureKind::BloodStalker,
            CreatureKind::BloodWisp,
            CreatureKind::ThornBrute,
        ] {
            let mut game = terra_core::GameplayState::default();
            let mut creature = animal(1, kind, [0.0; 3]);
            if kind == CreatureKind::Skeleton {
                creature.variant = 2;
            }
            let expected: Vec<_> =
                terra_core::creatures::drops::forest_loot(kind, creature.variant, creature.id)
                    .into_iter()
                    .flatten()
                    .collect();
            game.creatures.animals.insert(1, creature);
            assert!(hurt_creature(&mut game, 1, 100.0));
            assert_eq!(game.creatures.animals[&1].behavior, CreatureBehavior::Dead);
            let drops: Vec<_> = game.drops.iter().map(|drop| drop.stack).collect();
            assert_eq!(drops, expected, "{kind:?}");
            assert!(!hurt_creature(&mut game, 1, 100.0));
            assert_eq!(game.drops.len(), expected.len());
        }
    }

    #[test]
    fn actual_fast_forward_and_reverse_sweeps_hit_between_poses_but_stationary_trains_do_not() {
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::streaming::LoadedSection;
        let mut chunks = ChunkStore::default();
        let section = SectionPos::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data: BlockSection::empty(section).unwrap(),
                mesh: None,
                water: None,
                mesh_epoch: 0,
                water_epoch: 0,
                props: vec![],
            },
        );
        let journal = EditJournal::default();
        let properties = Default::default();
        let mut old = car();
        old.anchor = [4, 4, 4];
        old.motion.speed_mps = 50.0;
        let mut next = old.clone();
        next.motion.translation[2] = 5.0;
        next.motion.wheel_distance_m = 5.0;
        let shapes =
            terra_world::train_motion::clear_swept(&chunks, &journal, &properties, &old, &next)
                .unwrap();
        let shape = old.transformed_shapes(0)[0];
        let center: [f64; 3] = std::array::from_fn(|axis| {
            (shape.minimum[axis] + shape.maximum[axis]) * 0.5 + if axis == 2 { 2.5 } else { 0.0 }
        });
        let low = center.map(|v| v - 0.02);
        let high = center.map(|v| v + 0.02);
        assert!(
            !old.transformed_shapes(0)
                .iter()
                .any(|s| s.intersects(low, high))
        );
        assert!(
            !next
                .transformed_shapes(0)
                .iter()
                .any(|s| s.intersects(low, high))
        );
        assert!(
            Impact::moving(&old, &next, shapes.clone())
                .unwrap()
                .hits(low, high)
        );
        assert!(
            Impact::moving(&next, &old, shapes.clone())
                .unwrap()
                .hits(low, high)
        );
        assert!(Impact::moving(&old, &old, shapes).is_none());
    }
}
