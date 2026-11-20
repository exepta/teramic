//! Body-driven presentation contacts; neither free cameras nor world generation emit them.
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use terra_core::footsteps::{WaterMotion, WaterMotionKind as Kind};
use terra_network::client::{ClientSession, ReplicatedCreatures, ReplicatedPlayers};
use terra_world::{
    creatures::Habitat,
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess, WorldTerrain},
};

#[derive(Default)]
struct Contact {
    previous: Option<Vec3>,
    wet: bool,
    wake: f32,
    bubble: f32,
}
impl Contact {
    fn advance(
        &mut self,
        p: Vec3,
        wet: bool,
        submerged: bool,
        mammal: bool,
        dt: f32,
    ) -> Vec<(Kind, f32, Vec3)> {
        let mut result = Vec::new();
        let Some(old) = self.previous.replace(p) else {
            self.wet = wet;
            return result;
        };
        let change = p - old;
        if !p.is_finite() || change.length_squared() > 16.0 {
            self.wet = wet;
            return result;
        }
        let velocity = change / dt.max(0.01);
        if wet && !self.wet {
            result.push((
                Kind::Splash,
                (0.16 + (-velocity.y).max(0.0) * 0.07).clamp(0.16, 0.7),
                velocity,
            ));
        }
        self.wake -= dt;
        self.bubble -= dt;
        if wet && !submerged && velocity.xz().length() > 0.25 && self.wake <= 0.0 {
            self.wake = 0.25;
            result.push((
                Kind::Wake,
                (velocity.xz().length() * 0.12).clamp(0.15, 0.8),
                velocity,
            ));
        }
        if submerged && mammal && self.bubble <= 0.0 {
            self.bubble = 2.2;
            result.push((Kind::Bubble, 0.25, velocity));
        }
        self.wet = wet;
        result
    }
}
#[derive(Resource, Default)]
struct Contacts {
    actors: HashMap<(u8, u64), Contact>,
    next: f32,
    previous: f32,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<Contacts>().add_systems(
        Update,
        update
            .in_set(terra_core::footsteps::WaterMotionSet)
            .after(terra_player::PlayerSet::Movement),
    );
}
#[derive(bevy::ecs::system::SystemParam)]
struct Scene<'w, 's> {
    time: Res<'w, Time>,
    access: Res<'w, WorldAccess>,
    terrain: Res<'w, WorldTerrain>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    animals: Option<Res<'w, ReplicatedCreatures>>,
    players: Option<Res<'w, ReplicatedPlayers>>,
    session: Option<Res<'w, ClientSession>>,
    mode: Option<Res<'w, terra_player::ControllerMode>>,
    bodies: Query<'w, 's, &'static Transform, With<terra_player::PlayerBody>>,
}
fn update(scene: Scene, mut state: ResMut<Contacts>, mut events: MessageWriter<WaterMotion>) {
    if !scene.access.ready() {
        state.actors.clear();
        state.previous = 0.0;
        return;
    }
    let now = scene.time.elapsed_secs();
    if now < state.next {
        return;
    }
    let dt = (now - state.previous).clamp(0.01, 0.25);
    state.previous = now;
    state.next = now + 0.05;
    let h = Habitat {
        terrain: &scene.terrain.0,
        chunks: &scene.chunks,
        journal: &scene.journal,
    };
    let listener = scene.bodies.iter().next().map(|b| b.translation);
    let mut seen = HashSet::new();
    let mut actor = |key, p: Vec3, height: f32, mammal: bool| {
        if listener.is_none_or(|l| l.distance_squared(p) > 48.0 * 48.0) {
            return;
        }
        if h.block(p.floor().as_i64vec3().to_array()).is_none() {
            return;
        }
        seen.insert(key);
        let wet = h.wet((p + Vec3::Y * 0.05).to_array().map(f64::from));
        let head = p + Vec3::Y * height;
        let submerged = h.wet(head.to_array().map(f64::from));
        for (kind, strength, velocity) in state
            .actors
            .entry(key)
            .or_default()
            .advance(p, wet, submerged, mammal, dt)
        {
            let mut position = if kind == Kind::Bubble { head } else { p };
            if kind != Kind::Bubble {
                for _ in 0..40 {
                    if !h.wet(position.to_array().map(f64::from)) {
                        break;
                    }
                    position.y += 0.05;
                }
            }
            events.write(WaterMotion {
                position,
                kind,
                strength,
                velocity,
            });
        }
    };
    if scene.mode.as_deref() != Some(&terra_player::ControllerMode::Fly) {
        for body in &scene.bodies {
            actor(
                (0, 0),
                body.translation - Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT,
                1.6,
                true,
            );
        }
    }
    if let Some(players) = &scene.players {
        for (id, t) in &players.players {
            if scene.session.as_ref().and_then(|s| s.player) != Some(*id) {
                actor(
                    (1, id.0),
                    t.position() - Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT,
                    1.6,
                    true,
                );
            }
        }
    }
    if let Some(animals) = &scene.animals {
        for (id, t) in &animals.creatures {
            use terra_core::creatures::CreatureBehavior;
            if t.current.kind.is_flying() || t.current.behavior == CreatureBehavior::InNest {
                continue;
            }
            actor(
                (2, *id),
                t.position(),
                t.current.size()[1] as f32,
                !t.current.kind.is_aquatic(),
            );
        }
    }
    state.actors.retain(|id, _| seen.contains(id));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_flight_through_water_emits_nothing_but_walking_entry_remains_bounded() {
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::{TerraGenerator, streaming::LoadedSection};
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            base_height: -16.,
            sea_level: -24.,
            hill_amplitude: 0.,
            mountain_amplitude: 0.,
            biomes: false,
            rivers: false,
            caves: false,
            trees: false,
            ..Default::default()
        })
        .unwrap();
        let mut data = BlockSection::empty(SectionPos::default()).unwrap();
        data.set_block([3, 2, 3], 27).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            SectionPos::default(),
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        // A raw FluidWater block only identifies a fluid container. This cell
        // is above the generated reservoir, so give it actual saved occupancy.
        let mut journal = EditJournal::default();
        journal
            .set_fluid(
                [3, 2, 3],
                terra_world::water::FluidCell {
                    amount: 8,
                    source: true,
                    flow: [0; 2],
                },
            )
            .unwrap();
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        assert!(habitat.wet([3.5, 2.05, 3.5]));
        assert!(!habitat.wet([3.5, 3.15, 3.5]));
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<WorldAccess>()
            .insert_resource(journal)
            .insert_resource(WorldTerrain(terrain))
            .insert_resource(chunks)
            .insert_resource(terra_player::ControllerMode::Fly)
            .add_message::<WaterMotion>();
        install(&mut app);
        let body = app
            .world_mut()
            .spawn((terra_player::PlayerBody, Transform::from_xyz(3.5, 4., 3.5)))
            .id();
        let frame = |app: &mut App| {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(50));
            app.update();
        };
        frame(&mut app);
        app.world_mut()
            .get_mut::<Transform>(body)
            .unwrap()
            .translation
            .y = 2.0 + terra_physics::character::PLAYER_HALF_HEIGHT;
        frame(&mut app);
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<WaterMotion>>()
                .drain()
                .count(),
            0
        );
        assert!(app.world().resource::<Contacts>().actors.is_empty());
        app.insert_resource(terra_player::ControllerMode::Walk);
        app.world_mut()
            .get_mut::<Transform>(body)
            .unwrap()
            .translation
            .y = 4.;
        frame(&mut app);
        app.world_mut()
            .get_mut::<Transform>(body)
            .unwrap()
            .translation
            .y = 2.0 + terra_physics::character::PLAYER_HALF_HEIGHT;
        frame(&mut app);
        let events: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<WaterMotion>>()
            .drain()
            .collect();
        assert_eq!(events.iter().filter(|e| e.kind == Kind::Splash).count(), 1);
        assert!(events.iter().all(|e| e.strength <= 0.7));
    }

    #[test]
    fn body_entry_scales_with_impact_and_fish_never_bubble() {
        let entry = |speed: f32| {
            let mut c = Contact::default();
            c.advance(Vec3::Y, false, false, false, 0.05);
            c.advance(Vec3::Y * (1.0 - speed * 0.05), true, false, false, 0.05)[0].1
        };
        assert!(entry(10.0) > entry(1.0));
        let mut c = Contact::default();
        assert!(c.advance(Vec3::ZERO, true, true, false, 0.05).is_empty());
        assert!(c.advance(Vec3::ZERO, true, true, false, 0.05).is_empty());
        assert!(
            c.advance(Vec3::ZERO, true, true, true, 0.05)
                .iter()
                .any(|e| e.0 == Kind::Bubble)
        );
        assert!(
            c.advance(Vec3::splat(100.0), true, false, true, 0.05)
                .is_empty()
        );
    }
}
