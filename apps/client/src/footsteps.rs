//! Distance-driven contacts for local/remote players and replicated land animals.
use bevy::prelude::*;
use std::collections::HashMap;
use terra_core::footsteps::{Footstep, FootstepSet, Walker};
use terra_network::client::{ClientSession, ReplicatedCreatures, ReplicatedPlayers};
use terra_world::{
    creatures::Habitat,
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess, WorldTerrain},
};

#[derive(Default)]
struct Cadence {
    previous: Option<Vec3>,
    distance: f32,
    alternate: bool,
    grounded: Option<bool>,
    peak: f32,
}
impl Cadence {
    fn landing(&mut self, p: Vec3, grounded: bool) -> Option<f32> {
        let old = self.grounded.replace(grounded);
        if old.is_none()
            || self
                .previous
                .is_some_and(|old| old.distance_squared(p) > 9.0)
        {
            self.peak = p.y;
            return None;
        }
        self.peak = self.peak.max(p.y);
        let fall = self.peak - p.y;
        if grounded {
            self.peak = p.y;
        }
        (grounded && old == Some(false) && fall > 0.15).then_some(fall)
    }
    fn advance(&mut self, p: Vec3, grounded: bool, stride: f32) -> Option<(Vec2, bool)> {
        if !p.is_finite() {
            self.previous = None;
            self.distance = 0.0;
            return None;
        }
        let old = self.previous.replace(p)?;
        let delta = p - old;
        let horizontal = Vec2::new(delta.x, delta.z);
        if !grounded || delta.length_squared() > 9.0 || !p.is_finite() {
            self.distance = 0.0;
            return None;
        }
        self.distance += horizontal.length();
        if self.distance < stride || horizontal.length_squared() < 0.000001 {
            return None;
        }
        self.distance %= stride;
        self.alternate = !self.alternate;
        Some((horizontal.normalize(), self.alternate))
    }
}

#[derive(Resource, Default)]
struct Steps(HashMap<(u8, u64), Cadence>);

#[derive(Message)]
pub(crate) struct Landing {
    pub position: Vec3,
    pub material: u16,
    pub fall: f32,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Steps>()
        .add_message::<Landing>()
        .add_systems(
            Update,
            update
                .in_set(FootstepSet)
                .after(terra_player::PlayerSet::Movement),
        );
}

#[derive(bevy::ecs::system::SystemParam)]
struct StepWorld<'w, 's> {
    access: Res<'w, WorldAccess>,
    terrain: Res<'w, WorldTerrain>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    animals: Option<Res<'w, ReplicatedCreatures>>,
    players: Option<Res<'w, ReplicatedPlayers>>,
    session: Option<Res<'w, ClientSession>>,
    bodies: Query<
        'w,
        's,
        (&'static Transform, &'static terra_player::PlayerGrounded),
        With<terra_player::PlayerBody>,
    >,
}
fn update(
    world: StepWorld,
    mut state: ResMut<Steps>,
    mut events: MessageWriter<Footstep>,
    mut landings: MessageWriter<Landing>,
) {
    if !world.access.ready() {
        state.0.clear();
        return;
    }
    let habitat = Habitat {
        terrain: &world.terrain.0,
        chunks: &world.chunks,
        journal: &world.journal,
    };
    let listener = world.bodies.iter().next().map(|(t, _)| t.translation);
    let mut seen = std::collections::HashSet::new();
    let mut contact = |key, feet: Vec3, grounded, walker, stride| {
        seen.insert(key);
        let cadence = state.0.entry(key).or_default();
        let dry = !habitat.wet((feet + Vec3::Y * 0.1).to_array().map(f64::from));
        if let Some(fall) = cadence.landing(feet, grounded && dry)
            && walker == Walker::Player
            && dry
        {
            let support = (feet - Vec3::Y * 0.08).floor().as_i64vec3().to_array();
            if let Some(material) = habitat.block(support).filter(|m| {
                terra_voxel::MaterialId::from_raw(*m).is_some_and(terra_voxel::MaterialId::is_solid)
            }) {
                landings.write(Landing {
                    position: feet,
                    material,
                    fall,
                });
            }
        }
        let Some((forward, alternate)) = cadence.advance(feet, grounded && dry, stride) else {
            return;
        };
        // Grounded contacts also include furniture tops; only sand/snow leave prints.
        let support = (feet - Vec3::Y * 0.08).floor().as_i64vec3().to_array();
        let Some(material) = habitat.block(support).filter(|m| {
            terra_voxel::MaterialId::from_raw(*m).is_some_and(|material| {
                material.is_solid() && material != terra_voxel::MaterialId::SolidWater
            })
        }) else {
            return;
        };
        events.write(Footstep {
            position: if matches!(material, 4 | 5) {
                Vec3::new(feet.x, (support[1] + 1) as f32, feet.z)
            } else if material == 98 {
                let layers = world.journal.object_state(support).map_or(1, |state| {
                    terra_core::construction::snow_layers(state.facing)
                });
                Vec3::new(feet.x, support[1] as f32 + f32::from(layers) / 8.0, feet.z)
            } else {
                feet
            },
            forward,
            walker,
            material,
            alternate,
        });
    };
    for (body, grounded) in &world.bodies {
        contact(
            (0, 0),
            body.translation - Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT,
            grounded.0,
            Walker::Player,
            1.25,
        );
    }
    if let Some(players) = &world.players {
        for (id, track) in &players.players {
            if world.session.as_ref().and_then(|s| s.player) == Some(*id) {
                continue;
            }
            let p = track.position();
            if listener.is_some_and(|l| l.distance_squared(p) <= 48.0 * 48.0) {
                contact(
                    (1, id.0),
                    p - Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT,
                    track.current.grounded,
                    Walker::Player,
                    1.25,
                );
            }
        }
    }
    if let Some(animals) = &world.animals {
        for (id, track) in &animals.creatures {
            use terra_core::creatures::CreatureKind;
            let walker = match track.current.kind {
                CreatureKind::Cow | CreatureKind::Horse => Walker::Cow,
                CreatureKind::Sheep | CreatureKind::SpiritFox => Walker::Sheep,
                CreatureKind::Pig => Walker::Pig,
                CreatureKind::Bear => Walker::Bear,
                CreatureKind::Stalker => Walker::Stalker,
                CreatureKind::Villager | CreatureKind::Skeleton => Walker::Player,
                CreatureKind::ThornBrute => Walker::Bear,
                CreatureKind::Fish
                | CreatureKind::SandWorm
                | CreatureKind::Bee
                | CreatureKind::Butterfly
                | CreatureKind::SpiritGloom
                | CreatureKind::Raven
                | CreatureKind::BloodSlime
                | CreatureKind::BloodStalker
                | CreatureKind::BloodWisp
                | CreatureKind::Squid
                | CreatureKind::MoonJelly
                | CreatureKind::FireJelly
                | CreatureKind::VoidJelly
                | CreatureKind::SunJelly
                | CreatureKind::CrystalJelly
                | CreatureKind::ReefShark
                | CreatureKind::HammerheadShark
                | CreatureKind::WhaleShark => continue,
                CreatureKind::SmallSpider
                | CreatureKind::MediumSpider
                | CreatureKind::LargeSpider => continue,
            };
            let p = track.position();
            if listener.is_some_and(|l| l.distance_squared(p) <= 48.0 * 48.0) {
                contact(
                    (2, *id),
                    p,
                    track.current.velocity[1].abs() < 0.1,
                    walker,
                    if walker == Walker::Stalker {
                        1.05
                    } else {
                        0.65
                    },
                );
            }
        }
    }
    state.0.retain(|id, _| seen.contains(id));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn landing_uses_highest_airborne_point_and_does_not_fire_at_spawn_or_teleport() {
        let mut cadence = Cadence::default();
        assert!(cadence.landing(Vec3::Y, true).is_none());
        cadence.previous = Some(Vec3::Y);
        assert!(cadence.landing(Vec3::Y * 2.0, false).is_none());
        cadence.previous = Some(Vec3::Y * 2.0);
        assert!(cadence.landing(Vec3::Y * 4.0, false).is_none());
        cadence.previous = Some(Vec3::Y * 2.0);
        assert_eq!(cadence.landing(Vec3::Y, true), Some(3.0));
        cadence.previous = Some(Vec3::Y);
        assert!(cadence.landing(Vec3::Y * 100.0, true).is_none());
    }
    #[test]
    fn real_loaded_support_drives_local_contacts_and_air_does_not() {
        use terra_voxel::{SectionPos, blocks::BlockSection};
        use terra_world::streaming::LoadedSection;
        let mut app = App::new();
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            trees: false,
            water: false,
            biomes: false,
            ..default()
        })
        .unwrap();
        let owner = SectionPos::default();
        let mut data = BlockSection::empty(owner).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 0, z], 4).unwrap();
            }
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.insert_resource(WorldTerrain(terrain))
            .insert_resource(chunks)
            .init_resource::<EditJournal>()
            .init_resource::<WorldAccess>()
            .add_message::<Footstep>();
        install(&mut app);
        let body = app
            .world_mut()
            .spawn((
                terra_player::PlayerBody,
                terra_player::PlayerGrounded(true),
                Transform::from_xyz(2.5, 1.9, 3.5),
            ))
            .id();
        let mut contacts = Vec::new();
        for i in 0..=30 {
            app.world_mut()
                .get_mut::<Transform>(body)
                .unwrap()
                .translation
                .x = 2.5 + i as f32 * 0.1;
            app.update();
            contacts.extend(app.world_mut().resource_mut::<Messages<Footstep>>().drain());
        }
        assert_eq!(contacts.len(), 2);
        assert!(
            contacts
                .iter()
                .all(|s| s.material == 4 && s.position.y == 1.0)
        );
        assert_ne!(contacts[0].alternate, contacts[1].alternate);
        app.world_mut()
            .get_mut::<terra_player::PlayerGrounded>(body)
            .unwrap()
            .0 = false;
        for i in 0..30 {
            app.world_mut()
                .get_mut::<Transform>(body)
                .unwrap()
                .translation
                .x = 5.5 + i as f32 * 0.1;
            app.update();
            assert_eq!(
                app.world_mut()
                    .resource_mut::<Messages<Footstep>>()
                    .drain()
                    .count(),
                0
            );
        }
        // A lower furniture top is still a wooden contact, despite not being a full cube.
        for x in 0..16 {
            for z in 0..16 {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .record([x, 0, z], terra_voxel::MaterialId::Workbench as u16)
                    .unwrap();
            }
        }
        app.world_mut()
            .get_mut::<terra_player::PlayerGrounded>(body)
            .unwrap()
            .0 = true;
        contacts.clear();
        for i in 0..=30 {
            app.world_mut()
                .get_mut::<Transform>(body)
                .unwrap()
                .translation = Vec3::new(2.5 + i as f32 * 0.1, 1.65, 3.5);
            app.update();
            contacts.extend(app.world_mut().resource_mut::<Messages<Footstep>>().drain());
        }
        assert_eq!(contacts.len(), 2);
        assert!(contacts.iter().all(|s| {
            s.material == terra_voxel::MaterialId::Workbench as u16
                && (s.position.y - 0.75).abs() < 0.001
        }));
    }
    #[test]
    fn cadence_uses_distance_not_fps_and_never_emits_on_spawn_flight_or_teleport() {
        for hz in [30, 60, 144] {
            let mut c = Cadence::default();
            let mut count = 0;
            for frame in 0..=hz * 10 {
                count += usize::from(
                    c.advance(Vec3::X * (frame as f32 / hz as f32 * 5.0), true, 1.25)
                        .is_some(),
                );
            }
            assert!((39..=40).contains(&count), "{hz}: {count}");
            assert!(c.advance(Vec3::splat(100.0), true, 1.25).is_none());
            assert!(c.advance(Vec3::splat(101.0), false, 1.25).is_none());
        }
    }
}
