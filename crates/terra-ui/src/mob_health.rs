//! Bounded, camera-projected health bars over the existing replicated creatures.
use bevy::prelude::*;
use serde::Serialize;
use std::collections::BTreeMap;
use terra_core::{GameplayConfig, MobHealthBars, creatures::Creature};
use terra_network::client::{ClientSession, ReplicatedCreatures};
use tilt_ui::html_shared;

const RANGE: f32 = 64.0;
const MAX_BARS: usize = 64;
const SIGHT_BUDGET: usize = 8;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MobHealthBar {
    pub id: String,
    pub title: String,
    pub label: String,
    pub position: String,
    pub fill: String,
    pub font: String,
}

#[html_shared]
#[derive(Resource, Default, PartialEq, Serialize)]
pub struct MobHealthHud {
    pub bars: Vec<MobHealthBar>,
}

struct Sight {
    origin: Vec3,
    target: Vec3,
    checked: f64,
    visible: bool,
}

#[derive(Default)]
struct SightCache(BTreeMap<u64, Sight>);

struct Candidate {
    animal: Creature,
    point: Vec3,
    screen: Vec2,
    distance: f32,
}

#[derive(bevy::ecs::system::SystemParam)]
struct Scene<'w, 's> {
    config: Res<'w, GameplayConfig>,
    session: Option<Res<'w, ClientSession>>,
    creatures: Option<Res<'w, ReplicatedCreatures>>,
    menu: Option<Res<'w, crate::menu::MenuState>>,
    cameras:
        Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<terra_player::TerraCamera>>,
    scale: Option<Res<'w, UiScale>>,
    time: Res<'w, Time<Real>>,
    chunks: Res<'w, terra_world::streaming::ChunkStore>,
    journal: Res<'w, terra_world::edits::EditJournal>,
    properties: Res<'w, terra_core::block_properties::BlockPropertiesCatalog>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<MobHealthHud>().add_systems(
        PostUpdate,
        update
            .run_if(crate::cadence::ready)
            .after(bevy::transform::TransformSystems::Propagate),
    );
}

fn visible(scene: &Scene, origin: Vec3, point: Vec3) -> bool {
    let direction = point - origin;
    let distance = direction.length();
    distance > 0.01
        && terra_world::interaction_shapes::raycast_solid(
            &scene.chunks,
            &scene.journal,
            &scene.properties,
            origin.to_array().map(f64::from),
            (direction / distance).to_array().map(f64::from),
            f64::from((distance - 0.05).max(0.0)),
        )
        .is_none()
}

fn selected(mode: MobHealthBars, aimed: bool, sight: bool) -> bool {
    match mode {
        MobHealthBars::None => false,
        MobHealthBars::LookingAt => aimed && sight,
        MobHealthBars::InView => sight,
        MobHealthBars::Always => true,
    }
}

fn rgb([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn fill(health: f32, max: f32, color: [u8; 3]) -> String {
    let percent = if health.is_finite() && max.is_finite() && max > 0.0 {
        (health / max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    format!("width: {percent:.1}%; background-color: {};", rgb(color))
}

fn update(scene: Scene, mut hud: ResMut<MobHealthHud>, mut cache: Local<SightCache>) {
    let mut bars = Vec::new();
    let active = scene.config.mob_health_bars != MobHealthBars::None
        && scene.session.as_ref().is_some_and(|s| s.ready && !s.dead())
        && scene.menu.as_ref().is_none_or(|m| m.world_hud_active());
    let Some((camera, transform)) = scene.cameras.iter().next().filter(|_| active) else {
        cache.0.clear();
        if !hud.bars.is_empty() {
            hud.bars.clear();
        }
        return;
    };
    let Some(creatures) = scene.creatures.as_deref() else {
        cache.0.clear();
        if !hud.bars.is_empty() {
            hud.bars.clear();
        }
        return;
    };
    let origin = transform.translation();
    let forward = Vec3::from(transform.forward());
    let scale = scene.scale.as_ref().map_or(1.0, |s| s.0).max(0.1);
    let viewport = camera.logical_viewport_size().unwrap_or(Vec2::ZERO);
    let mut candidates: Vec<Candidate> = Vec::with_capacity(MAX_BARS + 1);
    // The rider's horse has a separate predicted/rendered seat pose. Do not
    // project its delayed creature snapshot through the first-person rider.
    let ridden = scene
        .session
        .as_ref()
        .and_then(|session| session.mounted)
        .map(|horse| horse.id);
    for track in creatures.creatures.values() {
        if track.current.health <= 0.0
            || track.current.behavior == terra_core::creatures::CreatureBehavior::InNest
            || ridden == Some(track.current.id)
        {
            continue;
        }
        let feet = track.position();
        let distance = feet.distance_squared(origin);
        if distance > RANGE * RANGE {
            continue;
        }
        let height = if track.current.kind == terra_core::creatures::CreatureKind::SandWorm {
            terra_core::creatures::sandworm::visible_height(&track.current)
        } else {
            track.current.size()[1] as f32
        };
        let point = feet + Vec3::Y * (height * 0.65);
        let label = feet + Vec3::Y * (height + 0.24);
        let Ok(screen) = camera.world_to_viewport(transform, label) else {
            continue;
        };
        if !screen.cmpge(Vec2::ZERO).all() || !screen.cmple(viewport).all() {
            continue;
        }
        if candidates.len() == MAX_BARS
            && candidates
                .last()
                .is_some_and(|last| distance >= last.distance)
        {
            continue;
        }
        let mut animal = track.current.clone();
        animal.position = feet.to_array().map(f64::from);
        let index = candidates.partition_point(|c| c.distance < distance);
        candidates.insert(
            index,
            Candidate {
                animal,
                point,
                screen,
                distance,
            },
        );
        if candidates.len() > MAX_BARS {
            candidates.pop();
        }
    }
    let aimed = if scene.config.mob_health_bars == MobHealthBars::LookingAt {
        candidates
            .iter()
            .filter_map(|c| {
                terra_world::creatures::ray_distance(
                    &c.animal,
                    origin.to_array().map(f64::from),
                    forward.to_array().map(f64::from),
                    f64::from(RANGE),
                )
                .map(|distance| (c.animal.id, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
    } else {
        None
    };
    let now = scene.time.elapsed_secs_f64();
    let mut budget = SIGHT_BUDGET;
    cache
        .0
        .retain(|id, _| candidates.iter().any(|c| c.animal.id == *id));
    // Oldest checks win even while the camera moves, so near bars cannot starve far ones.
    candidates.sort_unstable_by(|a, b| {
        let checked = |c: &Candidate| {
            cache
                .0
                .get(&c.animal.id)
                .map_or(f64::NEG_INFINITY, |s| s.checked)
        };
        checked(a)
            .total_cmp(&checked(b))
            .then(a.animal.id.cmp(&b.animal.id))
    });
    for c in candidates {
        let id = c.animal.id;
        if scene.config.mob_health_bars == MobHealthBars::LookingAt
            && aimed.map(|a| a.0) != Some(id)
        {
            continue;
        }
        let sight_point = aimed
            .filter(|a| a.0 == id)
            .map_or(c.point, |(_, distance)| origin + forward * distance as f32);
        let sight = if scene.config.mob_health_bars == MobHealthBars::Always {
            true
        } else {
            let stale = cache.0.get(&id).is_none_or(|entry| {
                now - entry.checked >= 0.2
                    || entry.origin.distance_squared(origin) > 0.25
                    || entry.target.distance_squared(sight_point) > 0.25
            });
            if stale && budget > 0 {
                budget -= 1;
                cache.0.insert(
                    id,
                    Sight {
                        origin,
                        target: sight_point,
                        checked: now,
                        visible: visible(&scene, origin, sight_point),
                    },
                );
            }
            cache
                .0
                .get(&id)
                .is_some_and(|entry| entry.visible && now - entry.checked < 0.5)
        };
        if !selected(
            scene.config.mob_health_bars,
            aimed.map(|a| a.0) == Some(id),
            sight,
        ) {
            continue;
        }
        let friendly_pet = creatures.bonds.get(&id).is_some_and(|b| b.owner.is_some());
        let hostile = c.animal.kind.hostile_to_players() && !friendly_pet;
        let max = c.animal.kind.health();
        let right = (viewport.x / scale - 80.0).max(80.0);
        let screen = Vec2::new(
            (c.screen.x / scale).clamp(80.0, right),
            (c.screen.y / scale).max(38.0),
        );
        bars.push(MobHealthBar {
            id: id.to_string(),
            title: c.animal.title(),
            label: format!("{} / {}", c.animal.health.ceil() as u16, max.ceil() as u16),
            position: format!("left: {:.1}px; top: {:.1}px;", screen.x, screen.y),
            fill: fill(
                c.animal.health,
                max,
                if hostile {
                    scene.config.mob_health_hostile_color
                } else {
                    scene.config.mob_health_friendly_color
                },
            ),
            font: format!("color: {};", rgb(scene.config.mob_health_font_color)),
        });
    }
    bars.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    if hud.bars != bars {
        hud.bars = bars;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, SectionPos};

    fn scene() -> App {
        use bevy::camera::{ComputedCameraValues, RenderTargetInfo};
        use terra_core::creatures::{CreatureBehavior, CreatureKind, CreatureNeeds};
        let mut app = App::new();
        app.init_resource::<GameplayConfig>()
            .init_resource::<Time<Real>>()
            .init_resource::<terra_world::streaming::ChunkStore>()
            .init_resource::<terra_world::edits::EditJournal>()
            .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            .init_resource::<ClientSession>()
            .init_resource::<ReplicatedCreatures>();
        app.world_mut().resource_mut::<ClientSession>().ready = true;
        for z in [-1, 0] {
            let p = SectionPos::new(0, 0, z);
            app.world_mut()
                .resource_mut::<terra_world::streaming::ChunkStore>()
                .loaded
                .insert(
                    p,
                    terra_world::streaming::LoadedSection {
                        data: BlockSection::empty(p).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
        }
        app.world_mut().spawn((
            terra_player::TerraCamera,
            Camera {
                computed: ComputedCameraValues {
                    clip_from_view: bevy::math::proj::perspective_infinite_reverse(
                        std::f32::consts::FRAC_PI_2,
                        4.0 / 3.0,
                        0.1,
                    ),
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(800, 600),
                        scale_factor: 1.0,
                    }),
                    ..default()
                },
                ..default()
            },
            GlobalTransform::from_translation(Vec3::new(0.5, 0.8, 0.5)),
        ));
        for (id, x, kind) in [(1, 0.5, CreatureKind::Cow), (2, 4.5, CreatureKind::Stalker)] {
            let a = Creature {
                id,
                kind,
                home: [0, -1],
                position: [x, 0.0, -5.5],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: kind.health() * 0.5,
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(kind),
            };
            app.world_mut()
                .resource_mut::<ReplicatedCreatures>()
                .creatures
                .insert(
                    id,
                    terra_network::client::CreatureTrack {
                        previous: a.clone(),
                        current: a,
                        received: std::time::Instant::now(),
                        playback: Default::default(),
                        interval: 0.1,
                    },
                );
        }
        install(&mut app);
        app
    }

    #[test]
    fn real_projection_aim_walls_colors_and_mode_changes_use_one_bounded_hud() {
        let mut app = scene();
        app.update();
        let bars = &app.world().resource::<MobHealthHud>().bars;
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].id, "1");
        assert!(bars[0].fill.contains("#69cb86"));
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .mob_health_bars = MobHealthBars::InView;
        app.update();
        assert_eq!(app.world().resource::<MobHealthHud>().bars.len(), 2);
        assert!(
            app.world()
                .resource::<MobHealthHud>()
                .bars
                .iter()
                .any(|b| b.id == "2" && b.fill.contains("#df625d"))
        );
        let section = SectionPos::new(0, 0, -1);
        for x in 0..=4 {
            for y in 0..=3 {
                app.world_mut()
                    .resource_mut::<terra_world::streaming::ChunkStore>()
                    .loaded
                    .get_mut(&section)
                    .unwrap()
                    .data
                    .set_block([x, y, 13], 3)
                    .unwrap();
            }
        }
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_millis(300));
        app.update();
        assert!(app.world().resource::<MobHealthHud>().bars.is_empty());
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .mob_health_bars = MobHealthBars::Always;
        app.update();
        assert_eq!(app.world().resource::<MobHealthHud>().bars.len(), 2);
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .mob_health_bars = MobHealthBars::None;
        app.update();
        assert!(app.world().resource::<MobHealthHud>().bars.is_empty());
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .mob_health_bars = MobHealthBars::Always;
        app.world_mut().resource_mut::<ClientSession>().ready = false;
        app.update();
        assert!(app.world().resource::<MobHealthHud>().bars.is_empty());
    }

    #[test]
    fn configured_colors_update_existing_bars_without_recreating_mobs() {
        let mut app = scene();
        {
            let mut config = app.world_mut().resource_mut::<GameplayConfig>();
            config.mob_health_bars = MobHealthBars::Always;
            config.mob_health_friendly_color = [10, 20, 30];
            config.mob_health_hostile_color = [250, 80, 40];
            config.mob_health_font_color = [140, 190, 240];
        }
        app.update();
        let bars = &app.world().resource::<MobHealthHud>().bars;
        assert_eq!(bars.len(), 2);
        assert!(
            bars.iter()
                .find(|b| b.id == "1")
                .unwrap()
                .fill
                .contains("#0a141e")
        );
        assert!(
            bars.iter()
                .find(|b| b.id == "2")
                .unwrap()
                .fill
                .contains("#fa5028")
        );
        assert!(bars.iter().all(|b| b.font == "color: #8cbef0;"));
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .mob_health_font_color = [255, 255, 255];
        app.update();
        assert!(
            app.world()
                .resource::<MobHealthHud>()
                .bars
                .iter()
                .all(|b| b.font == "color: #ffffff;")
        );
        assert_eq!(
            app.world()
                .resource::<ReplicatedCreatures>()
                .creatures
                .len(),
            2
        );
    }

    #[test]
    fn continuous_camera_motion_cannot_starve_the_last_health_bars() {
        let mut app = scene();
        let template = app.world().resource::<ReplicatedCreatures>().creatures[&1].clone();
        app.world_mut()
            .resource_mut::<ReplicatedCreatures>()
            .creatures
            .clear();
        for index in 0..MAX_BARS {
            let mut track = template.clone();
            track.current.id = index as u64 + 1;
            track.current.position[0] = 0.5 + index as f64 * 0.06;
            track.previous = track.current.clone();
            app.world_mut()
                .resource_mut::<ReplicatedCreatures>()
                .creatures
                .insert(track.current.id, track);
        }
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .mob_health_bars = MobHealthBars::InView;
        let camera = app
            .world_mut()
            .query_filtered::<Entity, With<terra_player::TerraCamera>>()
            .single(app.world())
            .unwrap();
        for frame in 0..16 {
            app.world_mut()
                .get_mut::<GlobalTransform>(camera)
                .unwrap()
                .set_if_neq(GlobalTransform::from_translation(Vec3::new(
                    if frame % 2 == 0 { 0.5 } else { 1.1 },
                    0.8,
                    0.5,
                )));
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(std::time::Duration::from_millis(17));
            app.update();
            let count = app.world().resource::<MobHealthHud>().bars.len();
            assert!(count <= MAX_BARS);
            if frame >= 7 {
                assert_eq!(count, MAX_BARS, "frame={frame}");
            }
        }
    }

    #[test]
    fn invisible_nesting_bees_never_receive_in_view_or_always_health_bars() {
        use terra_core::creatures::{CreatureBehavior, CreatureKind, CreatureNeeds};
        let mut app = scene();
        {
            let mut animals = app.world_mut().resource_mut::<ReplicatedCreatures>();
            let track = animals.creatures.get_mut(&1).unwrap();
            track.current.kind = CreatureKind::Bee;
            track.current.health = CreatureKind::Bee.health();
            track.current.needs = CreatureNeeds::full(CreatureKind::Bee);
            track.current.behavior = CreatureBehavior::InNest;
            track.previous = track.current.clone();
        }
        for mode in [MobHealthBars::InView, MobHealthBars::Always] {
            app.world_mut()
                .resource_mut::<GameplayConfig>()
                .mob_health_bars = mode;
            app.update();
            let bars = &app.world().resource::<MobHealthHud>().bars;
            assert_eq!(bars.len(), 1);
            assert_eq!(bars[0].id, "2");
        }
        {
            let mut animals = app.world_mut().resource_mut::<ReplicatedCreatures>();
            let track = animals.creatures.get_mut(&1).unwrap();
            track.current.behavior = CreatureBehavior::Fly;
            track.previous = track.current.clone();
        }
        app.update();
        assert_eq!(app.world().resource::<MobHealthHud>().bars.len(), 2);
    }

    #[test]
    fn the_local_mount_does_not_project_a_stale_snapshot_but_returns_after_dismount() {
        use terra_core::creatures::{CreatureKind, CreatureNeeds};
        let mut app = scene();
        let horse = {
            let mut animals = app.world_mut().resource_mut::<ReplicatedCreatures>();
            let track = animals.creatures.get_mut(&1).unwrap();
            track.current.kind = CreatureKind::Horse;
            track.current.health = CreatureKind::Horse.health();
            track.current.needs = CreatureNeeds::full(CreatureKind::Horse);
            track.previous = track.current.clone();
            track.current.clone()
        };
        app.world_mut().resource_mut::<ClientSession>().mounted = Some(
            terra_network::protocol::session::MountedState::new(&horse, 30.0, true),
        );
        for mode in [
            MobHealthBars::LookingAt,
            MobHealthBars::InView,
            MobHealthBars::Always,
        ] {
            app.world_mut()
                .resource_mut::<GameplayConfig>()
                .mob_health_bars = mode;
            app.update();
            assert!(
                app.world()
                    .resource::<MobHealthHud>()
                    .bars
                    .iter()
                    .all(|bar| bar.id != "1")
            );
        }
        app.world_mut().resource_mut::<ClientSession>().mounted = None;
        app.update();
        assert_eq!(app.world().resource::<MobHealthHud>().bars.len(), 2);
    }

    #[test]
    fn modes_preserve_aim_and_occlusion_except_explicit_all_time() {
        for aimed in [false, true] {
            for sight in [false, true] {
                assert!(!selected(MobHealthBars::None, aimed, sight));
                assert!(selected(MobHealthBars::Always, aimed, sight));
                assert_eq!(selected(MobHealthBars::InView, aimed, sight), sight);
                assert_eq!(
                    selected(MobHealthBars::LookingAt, aimed, sight),
                    aimed && sight
                );
            }
        }
        assert!(fill(5.0, 10.0, [105, 203, 134]).contains("50.0%; background-color: #69cb86"));
        assert!(fill(100.0, 10.0, [223, 98, 93]).contains("100.0%; background-color: #df625d"));
        assert!(fill(f32::NAN, 10.0, [0, 0, 0]).contains("0.0%"));
    }

    #[test]
    fn sandworm_health_label_rises_from_the_moving_sand_to_the_exposed_body() {
        use terra_core::creatures::{CreatureBehavior, CreatureKind, CreatureNeeds};
        let mut app = scene();
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .mob_health_bars = MobHealthBars::InView;
        {
            let mut creatures = app.world_mut().resource_mut::<ReplicatedCreatures>();
            creatures.creatures.remove(&2);
            let track = creatures.creatures.get_mut(&1).unwrap();
            track.current.kind = CreatureKind::SandWorm;
            // Keep the full height of the reared worm in the camera frustum.
            track.current.position[2] = -12.5;
            track.current.velocity = [0.0, 0.0, 1.5];
            track.current.health = 60.0;
            track.current.needs = CreatureNeeds::full(CreatureKind::SandWorm);
            track.current.behavior = CreatureBehavior::Burrowed;
            track.previous = track.current.clone();
        }
        app.update();
        let buried = app.world().resource::<MobHealthHud>().bars.clone();
        assert_eq!(buried.len(), 1);
        assert_eq!(buried[0].label, "60 / 120");
        {
            let mut creatures = app.world_mut().resource_mut::<ReplicatedCreatures>();
            let track = creatures.creatures.get_mut(&1).unwrap();
            track.current.behavior = CreatureBehavior::Attack;
            track.current.velocity = [0.0; 3];
            track.previous = track.current.clone();
        }
        app.update();
        let exposed = &app.world().resource::<MobHealthHud>().bars;
        assert_eq!(exposed.len(), 1);
        assert_eq!(exposed[0].label, "60 / 120");
        let top = |bar: &MobHealthBar| -> f32 {
            bar.position
                .split_once("top: ")
                .unwrap()
                .1
                .strip_suffix("px;")
                .unwrap()
                .parse()
                .unwrap()
        };
        assert!(
            top(&buried[0]) - top(&exposed[0]) > 100.0,
            "the bar must rise with the actual emerging creature"
        );
    }
}
