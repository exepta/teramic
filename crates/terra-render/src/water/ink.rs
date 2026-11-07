//! Short, body-hit squid ink exposure, composited by the existing environment pass.
use bevy::prelude::*;
use std::collections::BTreeSet;
use terra_core::{
    GameConfig, PlayerUuid, WorldId,
    creatures::{CreatureBehavior, CreatureKind},
};
use terra_network::client::{ClientSession, ReplicatedCreatures};
use terra_player::{PlayerBody, TerraCamera};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess, WorldTerrain},
};

const HIT_RADIUS: f32 = 2.5;
const PROBE_SECONDS: f32 = 0.05;
const COOLDOWN_SECONDS: f32 = 2.0;
const CLEAR_SECONDS: f32 = 4.2;

#[derive(Resource, Default, Debug)]
pub struct InkExposure {
    pub intensity: f32,
    pub age: f32,
    pub hits: u64,
    cooldown: f32,
    next_probe: f32,
    identity: Option<(WorldId, Option<PlayerUuid>)>,
    seed: f32,
    seen: BTreeSet<u64>,
    current: Vec<u64>,
}
impl InkExposure {
    fn context(&mut self, active: bool, identity: (WorldId, Option<PlayerUuid>)) -> bool {
        if !active {
            *self = Self::default();
            false
        } else {
            if self.identity != Some(identity) {
                *self = Self {
                    identity: Some(identity),
                    ..default()
                };
            }
            true
        }
    }
    fn advance(&mut self, dt: f32) {
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.age = (self.age + dt).min(CLEAR_SECONDS);
        let t = ((self.age - 0.9) / (CLEAR_SECONDS - 0.9)).clamp(0.0, 1.0);
        if self.intensity > 0.0 {
            self.intensity = 1.0 - t * t * (3.0 - 2.0 * t);
        }
        if self.intensity < 0.001 || self.age >= CLEAR_SECONDS {
            self.intensity = 0.0;
        }
    }

    fn hit(&mut self, id: u64, center: Vec3, head: Vec3) -> bool {
        if self.cooldown > 0.0 || center.distance_squared(head) > HIT_RADIUS * HIT_RADIUS {
            return false;
        }
        self.intensity = 1.0;
        self.age = 0.0;
        self.cooldown = COOLDOWN_SECONDS;
        self.hits = self.hits.saturating_add(1);
        // Seed changes pattern per squid, without random runtime state.
        self.seed = (id & 255) as f32;
        true
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct Scene<'w, 's> {
    time: Res<'w, Time>,
    config: Res<'w, GameConfig>,
    access: Res<'w, WorldAccess>,
    session: Option<Res<'w, ClientSession>>,
    animals: Option<Res<'w, ReplicatedCreatures>>,
    terrain: Res<'w, WorldTerrain>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    bodies: Query<'w, 's, &'static Transform, With<PlayerBody>>,
}

pub(super) fn update(
    scene: Scene,
    mut exposure: ResMut<InkExposure>,
    mut cameras: Query<&mut crate::scene::caves::shader::CaveSettings, With<TerraCamera>>,
) {
    let head = scene
        .bodies
        .iter()
        .next()
        .map(|body| body.translation + Vec3::Y * terra_player::PLAYER_EYE_OFFSET);
    let active = scene.access.ready()
        && scene.session.as_ref().is_none_or(|session| session.ready)
        && head.is_some_and(|head| submerged(head, &scene.terrain, &scene.chunks, &scene.journal));
    let identity = (
        scene.config.world.id,
        scene.session.as_ref().and_then(|session| session.uuid),
    );
    if exposure.context(active, identity) {
        exposure.advance(scene.time.delta_secs());
        let now = scene.time.elapsed_secs();
        if now >= exposure.next_probe {
            exposure.next_probe = now + PROBE_SECONDS;
            exposure.current.clear();
            if let (Some(head), Some(animals)) = (head, scene.animals.as_ref()) {
                for track in animals
                    .creatures
                    .values()
                    .take(terra_core::creatures::MAX_VISIBLE_CREATURES)
                {
                    let animal = &track.current;
                    if animal.kind != CreatureKind::Squid
                        || animal.behavior != CreatureBehavior::Ink
                    {
                        continue;
                    }
                    let new_release = exposure.seen.insert(animal.id);
                    exposure.current.push(animal.id);
                    if new_release {
                        let center = track.position() + Vec3::Y * animal.size()[1] as f32 * 0.5;
                        exposure.hit(animal.id, center, head);
                    }
                }
            }
            let current = std::mem::take(&mut exposure.current);
            exposure.seen.retain(|id| current.contains(id));
            exposure.current = current;
        }
    }
    let ink = Vec4::new(exposure.intensity, exposure.age, exposure.seed, 0.0);
    for mut settings in &mut cameras {
        if settings.ink != ink {
            settings.ink = ink;
        }
    }
}

fn submerged(
    head: Vec3,
    terrain: &WorldTerrain,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> bool {
    let cell = head.floor().as_i64vec3().to_array();
    if chunks
        .prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])
        .is_none()
    {
        return false;
    }
    chunks.block_at(cell).is_some_and(|raw| {
        if terra_core::construction::is_slab(raw) || terra_core::construction::is_stair(raw) {
            return chunks
                .water_column_at_point(&terrain.0, journal, head.as_dvec3().to_array())
                .is_some_and(|water| f64::from(head.y) < water.level);
        }
        let fluid = chunks.fluid_cell_at(&terrain.0, journal, cell, raw);
        fluid.amount > 0
            && head.y
                < cell[1] as f32
                    + f32::from(fluid.amount) / f32::from(terra_world::water::FLUID_CAPACITY)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_is_radius_and_cooldown_bounded_and_clears() {
        let mut exposure = InkExposure::default();
        assert!(!exposure.hit(1, Vec3::X * 2.51, Vec3::ZERO));
        assert!(exposure.hit(1, Vec3::X * 2.49, Vec3::ZERO));
        assert_eq!(exposure.hits, 1);
        assert!(!exposure.hit(2, Vec3::ZERO, Vec3::ZERO));
        for _ in 0..50 {
            exposure.advance(0.1);
        }
        assert_eq!(exposure.intensity, 0.0);
        assert!(exposure.hit(2, Vec3::ZERO, Vec3::ZERO));
        assert_eq!(exposure.hits, 2);
    }

    #[test]
    fn ink_decay_is_frame_rate_independent_and_leaving_water_or_session_clears_it() {
        let mut slow = InkExposure::default();
        let mut fast = InkExposure::default();
        let identity = (WorldId(1), None);
        for exposure in [&mut slow, &mut fast] {
            assert!(exposure.context(true, identity));
            assert!(exposure.hit(7, Vec3::ZERO, Vec3::ZERO));
        }
        for _ in 0..20 {
            slow.advance(0.1);
        }
        for _ in 0..200 {
            fast.advance(0.01);
        }
        assert!((slow.intensity - fast.intensity).abs() < 0.0001);
        assert!(!slow.context(false, identity));
        assert_eq!(slow.intensity, 0.0);
        assert_eq!(slow.hits, 0);
        assert!(fast.context(true, (WorldId(2), None)));
        assert_eq!(fast.intensity, 0.0);
        assert!(fast.seen.is_empty());
    }

    #[test]
    fn ink_shader_is_the_same_gameplay_effect_at_every_water_quality() {
        let shader = include_str!("../../../../assets/shaders/terra_caves.wesl");
        assert!(shader.contains("cave.ink.x"));
        assert!(shader.contains("fn ink_mask"));
        let pipeline = include_str!("../scene/caves/shader.rs");
        assert!(pipeline.contains("settings.ink.x <= 0.001"));
    }
}
