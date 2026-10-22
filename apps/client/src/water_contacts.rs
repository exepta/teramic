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
                (0.2 + (-velocity.y).max(0.0) * 0.12).clamp(0.2, 2.0),
                velocity,
            ));
        }
        self.wake -= dt;
        self.bubble -= dt;
        if wet && !submerged && velocity.xz().length() > 0.25 && self.wake <= 0.0 {
            self.wake = 0.18;
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
    for body in &scene.bodies {
        actor(
            (0, 0),
            body.translation - Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT,
            1.6,
            true,
        );
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
            use terra_core::creatures::{CreatureBehavior, CreatureKind};
            if t.current.kind == CreatureKind::Bee || t.current.behavior == CreatureBehavior::InNest
            {
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
