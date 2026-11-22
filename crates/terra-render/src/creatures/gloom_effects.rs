//! One bounded shared glow batch for creature trails, cast particles and short reactions.
//! No per-creature meshes/materials or accumulating particle entities.
use super::{CreatureAvatar, Kind, Motion};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use std::collections::{BTreeMap, VecDeque};
use terra_player::TerraCamera;

const MAX_SPIRITS: usize = 24;
const MAX_QUADS: usize = 384;
const TRAIL_POINTS: usize = 8;
const STEP: f32 = 1.0 / 20.0;
#[derive(Clone)]
struct Trail {
    points: VecDeque<(Vec3, f32)>,
    next: f32,
    health: f32,
    burst: f32,
    variant: u8,
}
impl Trail {
    fn new(position: Vec3, now: f32, health: f32, variant: u8) -> Self {
        Self {
            points: VecDeque::from([(position, now)]),
            next: now + 0.18,
            health,
            burst: 0.0,
            variant,
        }
    }
    fn sample(&mut self, position: Vec3, now: f32, health: f32, variant: u8) {
        self.variant = variant;
        if health < self.health {
            self.burst = now + 0.8;
        }
        self.health = health;
        if self
            .points
            .back()
            .is_some_and(|(p, _)| p.distance_squared(position) > 9.0)
        {
            self.points.clear();
        }
        while self.points.front().is_some_and(|(_, t)| now - *t > 1.5) {
            self.points.pop_front();
        }
        if now >= self.next {
            self.next = now + 0.18;
            if self
                .points
                .back()
                .is_none_or(|(p, _)| p.distance_squared(position) > 0.025_f32.powi(2))
            {
                self.points.push_back((position, now));
                while self.points.len() > TRAIL_POINTS {
                    self.points.pop_front();
                }
            }
        }
    }
}
#[derive(Component)]
struct GloomGlowBatch;
type GlowBatchFilter = (
    With<GloomGlowBatch>,
    Without<CreatureAvatar>,
    Without<TerraCamera>,
);
type DragonCasts<'w, 's> = Query<
    'w,
    's,
    (
        &'static GlobalTransform,
        &'static Motion,
        &'static super::dragon_animation::Presentation,
    ),
    With<CreatureAvatar>,
>;
#[derive(Resource, Default)]
struct State {
    trails: BTreeMap<u64, Trail>,
    next: f32,
    mesh: Handle<Mesh>,
}
#[derive(Clone, Copy)]
struct Mote {
    position: Vec3,
    size: f32,
    color: [f32; 4],
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, update.after(TransformSystems::Propagate));
}
fn setup(
    mut commands: Commands,
    mut state: ResMut<State>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut pixels = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16 {
        for x in 0..16 {
            let dx = (x as f32 + 0.5) / 8.0 - 1.0;
            let dy = (y as f32 + 0.5) / 8.0 - 1.0;
            let radial = (1.0 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            // A bright compact core inside a soft glow, without square card edges.
            pixels.extend([255, 255, 255, (radial * radial * 255.0) as u8]);
        }
    }
    let image = images.add(Image::new(
        Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    state.mesh = meshes.add(mesh(&[], Vec3::ZERO, Vec3::X, Vec3::Y));
    let material = materials.add(StandardMaterial {
        base_color: Color::linear_rgba(1.8, 1.8, 1.8, 1.0),
        base_color_texture: Some(image),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Name::new("Gloom trails and drifting light petals"),
        GloomGlowBatch,
        Mesh3d(state.mesh.clone()),
        MeshMaterial3d(material),
        Transform::default(),
        Visibility::Hidden,
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}
#[allow(clippy::too_many_arguments)]
fn update(
    time: Res<Time>,
    replicas: Option<Res<terra_network::client::ReplicatedCreatures>>,
    session: Option<Res<terra_network::client::ClientSession>>,
    camera: Query<&GlobalTransform, With<TerraCamera>>,
    spirits: Query<(&CreatureAvatar, &GlobalTransform, &Motion)>,
    dragons: DragonCasts,
    mut state: ResMut<State>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut batch: Query<(&mut Transform, &mut GlobalTransform, &mut Visibility), GlowBatchFilter>,
) {
    let now = time.elapsed_secs();
    if now < state.next {
        return;
    }
    state.next = now + STEP;
    let Some(camera) = camera.iter().next() else {
        return;
    };
    let center = camera.translation();
    let mut selected: Vec<_> = spirits
        .iter()
        .filter(|(_, _, m)| m.kind == Kind::SpiritGloom && m.behavior != super::Behavior::Dead)
        .map(|(id, t, m)| (id.0, t.translation(), m.speed))
        .filter(|(_, p, _)| p.distance_squared(center) < 48.0 * 48.0)
        .collect();
    selected.sort_by(|a, b| {
        a.1.distance_squared(center)
            .total_cmp(&b.1.distance_squared(center))
            .then(a.0.cmp(&b.0))
    });
    selected.truncate(MAX_SPIRITS);
    state
        .trails
        .retain(|id, _| selected.iter().any(|s| s.0 == *id));
    let mut motes = Vec::with_capacity(MAX_QUADS);
    for (root, motion, cast) in &dragons {
        if root.translation().distance_squared(center) < 96. * 96. && motes.len() < MAX_QUADS / 2 {
            cast.motes(motion.kind, root, |position, size, color| {
                motes.push(Mote {
                    position,
                    size,
                    color,
                });
            });
        }
    }
    for (id, position, speed) in selected {
        let animal = replicas
            .as_ref()
            .and_then(|r| r.creatures.get(&id))
            .map(|t| &t.current);
        let variant = animal.map_or(0, |a| a.variant);
        let health = animal.map_or(8.0, |a| a.health);
        let trail = state
            .trails
            .entry(id)
            .or_insert_with(|| Trail::new(position, now, health, variant));
        trail.sample(position, now, health, variant);
        let palette = terra_core::creatures::gloom::palette(trail.variant);
        let color = Color::srgb(palette.glow[0], palette.glow[1], palette.glow[2]).to_linear();
        let rgba = |a| [color.red, color.green, color.blue, a];
        if speed > 0.025 {
            let points: Vec<_> = trail.points.iter().copied().collect();
            for pair in points.windows(2) {
                for t in [0.0, 0.5] {
                    let age = now - (pair[0].1 + (pair[1].1 - pair[0].1) * t);
                    let fade = (1.0 - age / 1.5).clamp(0.0, 1.0);
                    motes.push(Mote {
                        position: pair[0].0.lerp(pair[1].0, t) + Vec3::Y * 0.15,
                        size: 0.07 * fade,
                        color: rgba(fade * 0.55),
                    });
                }
            }
        }
        for i in 0..4 {
            let angle = now * 0.6 + i as f32 * 2.39996 + (id % 31) as f32;
            motes.push(Mote {
                position: position
                    + Vec3::new(
                        angle.sin() * 0.38,
                        0.20 + (angle * 1.2).sin() * 0.17,
                        angle.cos() * 0.32,
                    ),
                size: 0.033 + 0.012 * (angle * 2.0).sin().abs(),
                color: rgba(0.7),
            });
        }
        if trail.burst > now {
            let t = 1.0 - (trail.burst - now) / 0.8;
            for i in 0..12 {
                let a = i as f32 * 2.39996;
                motes.push(Mote {
                    position: position
                        + Vec3::new(a.sin(), (a * 1.3).sin() * 0.4 + 0.4, a.cos())
                            * (0.12 + t * 0.6),
                    size: 0.07 * (1.0 - t),
                    color: rgba(1.0 - t),
                });
            }
        }
        if motes.len() >= MAX_QUADS {
            break;
        }
    }
    // The same cached batch also gives the actual collectable resources a
    // small matching aura. At most 32 nearby drops contribute four motes each.
    for drop in session
        .iter()
        .flat_map(|session| session.drops.iter())
        .filter(|drop| {
            matches!(
                drop.stack.item,
                terra_core::ItemKind::GloomEssence | terra_core::ItemKind::LightSplitter
            )
        })
        .filter(|drop| {
            Vec3::from_array(drop.position.map(|v| v as f32)).distance_squared(center) < 32.0 * 32.0
        })
        .take(32)
    {
        let position = Vec3::from_array(drop.position.map(|v| v as f32));
        let gold = drop.stack.item == terra_core::ItemKind::LightSplitter;
        for i in 0..4 {
            let phase = now * 1.3 + i as f32 * std::f32::consts::FRAC_PI_2;
            motes.push(Mote {
                position: position
                    + Vec3::new(
                        phase.cos() * 0.15,
                        0.18 + phase.sin() * 0.08,
                        phase.sin() * 0.15,
                    ),
                size: 0.035,
                color: if gold {
                    [1.0, 0.74, 0.22, 0.7]
                } else {
                    [0.28, 0.81, 1.0, 0.7]
                },
            });
        }
    }
    motes.truncate(MAX_QUADS);
    let right = camera.rotation() * Vec3::X;
    let up = camera.rotation() * Vec3::Y;
    // Hidden idle batches keep their existing finite geometry. Publishing a
    // zero-vertex Mesh every tick makes Bevy retry it and log at particle rate.
    if !motes.is_empty()
        && let Some(mut target) = meshes.get_mut(&state.mesh)
    {
        *target = mesh(&motes, center, right, up);
    }
    for (mut transform, mut global, mut visible) in &mut batch {
        *transform = Transform::from_translation(center);
        // This batch updates after propagation to sample the animated spirits.
        // Keep its world transform current for this frame's transparent draw.
        *global = GlobalTransform::from(*transform);
        *visible = if motes.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}
fn mesh(motes: &[Mote], origin: Vec3, right: Vec3, up: Vec3) -> Mesh {
    // Startup also needs a valid GPU asset before any Gloom/drop is nearby.
    // The entity is hidden; this transparent card is only its finite placeholder.
    let hidden = Mote {
        position: origin,
        size: 0.001,
        color: [0.0; 4],
    };
    let motes = if motes.is_empty() {
        std::slice::from_ref(&hidden)
    } else {
        motes
    };
    let mut p = Vec::with_capacity(motes.len() * 4);
    let mut uv = Vec::with_capacity(motes.len() * 4);
    let mut colors = Vec::with_capacity(motes.len() * 4);
    let mut indices = Vec::with_capacity(motes.len() * 6);
    for mote in motes.iter().take(MAX_QUADS) {
        let base = p.len() as u32;
        let center = mote.position - origin;
        for ([x, y], tex) in [
            ([-1.0, -1.0], [0.0, 1.0]),
            ([1.0, -1.0], [1.0, 1.0]),
            ([1.0, 1.0], [1.0, 0.0]),
            ([-1.0, 1.0], [0.0, 0.0]),
        ] {
            p.push((center + (right * x + up * y) * mote.size).to_array());
            uv.push(tex);
            colors.push(mote.color);
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let count = p.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; count])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trails_expire_teleports_clear_and_damage_reacts_without_accumulation() {
        let mut trail = Trail::new(Vec3::ZERO, 0.0, 8.0, 0);
        for n in 1..2000 {
            let now = n as f32 * 0.05;
            trail.sample(Vec3::X * now * 0.65, now, 8.0, 0);
            assert!(trail.points.len() <= TRAIL_POINTS);
        }
        trail.sample(Vec3::new(1000.0, 0.0, 0.0), 101.0, 7.0, 0);
        assert!(trail.burst > 101.0);
        assert!(trail.points.len() <= 1);
        trail.sample(Vec3::new(1000.0, 0.0, 0.0), 110.0, 7.0, 0);
        assert!(trail.points.len() <= 1);
    }
    #[test]
    fn idle_gloom_effect_asset_has_finite_invisible_geometry_instead_of_empty_gpu_uploads() {
        let batch = mesh(&[], Vec3::new(13.0, 25.0, -42.0), Vec3::X, Vec3::Y);
        assert_eq!(batch.count_vertices(), 4);
        assert_eq!(batch.indices().unwrap().len(), 6);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(points)) =
            batch.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("finite idle positions");
        };
        assert!(points.iter().flatten().all(|v| v.is_finite()));
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
            batch.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("transparent idle colors");
        };
        assert!(colors.iter().all(|color| color[3] == 0.0));
    }

    #[test]
    fn glow_geometry_has_fixed_budget_and_soft_cards_keep_vertex_alpha() {
        let motes = vec![
            Mote {
                position: Vec3::ZERO,
                size: 0.05,
                color: [0.4, 0.8, 1.0, 0.5]
            };
            MAX_QUADS + 10
        ];
        let batch = mesh(&motes, Vec3::ZERO, Vec3::X, Vec3::Y);
        assert_eq!(batch.count_vertices(), MAX_QUADS * 4);
        assert_eq!(batch.indices().unwrap().len(), MAX_QUADS * 6);
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
            batch.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("soft glow vertex colors");
        };
        assert!(colors.iter().all(|color| color[3] == 0.5));
    }
}
