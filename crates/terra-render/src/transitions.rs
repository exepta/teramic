use crate::{
    foliage::LeafMaterial, props::wind::PropMaterial, water::material::WaterMaterial,
    weather::surfaces::WetMaterial,
};
use bevy::{
    camera::visibility::{RenderLayers, VisibilityRange},
    prelude::*,
};
use terra_core::{ChunkAnimation, GameConfig};
#[path = "transitions/feedback.rs"]
mod feedback;
#[path = "transitions/fixture.rs"]
mod fixture;
#[path = "transitions/wet_probe.rs"]
mod wet_probe;
pub(crate) use feedback::TransitionDraws;

#[derive(Component)]
pub(crate) struct EnteringChunk;

#[derive(Component)]
pub(crate) struct QueuedChunk {
    mode: ChunkAnimation,
    outgoing: bool,
    meshes: Vec<Handle<Mesh>>,
    visibility: Visibility,
}

#[derive(Resource, Default)]
struct QueueCandidates {
    refreshed: Option<f64>,
    entities: Vec<Entity>,
}

#[derive(Resource, Default, Debug)]
pub struct ChunkAnimationStatus {
    pub queued: usize,
    pub queued_in_view: usize,
    pub active: usize,
    pub waiting_for_draw: usize,
    /// Material counts on waiting roots: standard, leaf, prop, water.
    pub waiting_materials: [usize; 4],
    pub started_after_draw: usize,
    pub completed: usize,
    pub peak_active: usize,
}

const MAX_FADING_ROOTS: usize = 96;
const MAX_BACKGROUND_ROOTS: usize = 64;
pub(crate) const QUEUE_SHADOW_LAYER: usize = 62;

#[derive(Component)]
struct QueuedShadow {
    layers: Option<RenderLayers>,
    visibility: Visibility,
}

fn mesh_entities(world: &World, root: Entity) -> Vec<Entity> {
    let mut entities = vec![root];
    let mut i = 0;
    while i < entities.len() {
        if let Some(children) = world.get::<Children>(entities[i]) {
            entities.extend(children.iter());
        }
        i += 1;
    }
    entities.retain(|entity| world.get::<Mesh3d>(*entity).is_some());
    entities
}

fn queue_shadows(world: &mut World, root: Entity, root_visibility: Visibility) {
    let _scope = terra_core::profiling::ScopeTimer::new("transitions.queue_shadows");
    if root_visibility == Visibility::Hidden {
        return;
    }
    for entity in mesh_entities(world, root) {
        queue_shadow(world, root, entity, root_visibility);
    }
}

fn queue_shadow(world: &mut World, root: Entity, entity: Entity, root_visibility: Visibility) {
    if world.get::<bevy::light::NotShadowCaster>(entity).is_some()
        || world.get::<QueuedShadow>(entity).is_some()
    {
        return;
    }
    let layers = world.get::<RenderLayers>(entity).cloned();
    let visibility = if entity == root {
        root_visibility
    } else {
        world.get::<Visibility>(entity).copied().unwrap_or_default()
    };
    world.entity_mut(entity).insert((
        QueuedShadow { layers, visibility },
        RenderLayers::layer(QUEUE_SHADOW_LAYER),
        // Visible overrides the queued parent's hidden state, on the sun-only layer.
        Visibility::Visible,
    ));
}

pub(crate) fn restore_proxy_caster(world: &mut World, entity: Entity) {
    let root = world.get::<ChildOf>(entity).map(ChildOf::parent);
    if root.is_some_and(|root| world.get::<ChunkFade>(root).is_some()) {
        world.entity_mut(entity).insert(RestoreShadow);
        return;
    }
    world
        .entity_mut(entity)
        .remove::<bevy::light::NotShadowCaster>();
    let queued = root.and_then(|root| {
        world
            .get::<QueuedChunk>(root)
            .filter(|queued| !queued.outgoing)
            .map(|queued| (root, queued.visibility))
    });
    if let Some((root, visibility)) = queued
        && visibility != Visibility::Hidden
    {
        queue_shadow(world, root, entity, visibility);
    }
}

fn restore_queued_shadow(world: &mut World, entity: Entity) {
    if let Some(shadow) = world.entity_mut(entity).take::<QueuedShadow>() {
        let mut entity = world.entity_mut(entity);
        entity.insert(shadow.visibility);
        if let Some(layers) = shadow.layers {
            entity.insert(layers);
        } else {
            entity.remove::<RenderLayers>();
        }
    }
}

trait FadeMaterial: Material + Clone {
    fn base(&mut self) -> &mut StandardMaterial;
    fn alpha(&self) -> f32;
    fn set_alpha(&mut self, alpha: f32) {
        self.base().base_color.set_alpha(alpha);
    }
    fn opacity(&mut self, opacity: f32) {
        let base = self.base();
        base.base_color.set_alpha(base.base_color.alpha() * opacity);
        base.alpha_mode = AlphaMode::Blend;
    }
}
impl FadeMaterial for StandardMaterial {
    fn alpha(&self) -> f32 {
        self.base_color.alpha()
    }
    fn base(&mut self) -> &mut StandardMaterial {
        self
    }
}
impl FadeMaterial for LeafMaterial {
    fn alpha(&self) -> f32 {
        self.base.base_color.alpha()
    }
    fn base(&mut self) -> &mut StandardMaterial {
        &mut self.base
    }
}
impl FadeMaterial for WetMaterial {
    fn alpha(&self) -> f32 {
        self.base.base_color.alpha()
    }
    fn base(&mut self) -> &mut StandardMaterial {
        &mut self.base
    }
}
impl FadeMaterial for PropMaterial {
    fn alpha(&self) -> f32 {
        self.base.base_color.alpha()
    }
    fn base(&mut self) -> &mut StandardMaterial {
        &mut self.base
    }
}
impl FadeMaterial for WaterMaterial {
    fn alpha(&self) -> f32 {
        self.extension.tint.w
    }
    fn set_alpha(&mut self, alpha: f32) {
        self.extension.tint.w = alpha;
    }
    fn base(&mut self) -> &mut StandardMaterial {
        &mut self.base
    }
    fn opacity(&mut self, opacity: f32) {
        self.base.alpha_mode = AlphaMode::Blend;
        self.extension.tint.w *= opacity;
    }
}

enum FadedHandle {
    Standard(Entity, Handle<StandardMaterial>, Handle<StandardMaterial>),
    Leaf(Entity, Handle<LeafMaterial>, Handle<LeafMaterial>),
    Prop(Entity, Handle<PropMaterial>, Handle<PropMaterial>),
    Water(Entity, Handle<WaterMaterial>, Handle<WaterMaterial>),
    Wet(Entity, Handle<WetMaterial>, Handle<WetMaterial>),
}
impl FadedHandle {
    fn entity(&self) -> Entity {
        match self {
            Self::Standard(e, _, _)
            | Self::Leaf(e, _, _)
            | Self::Prop(e, _, _)
            | Self::Water(e, _, _)
            | Self::Wet(e, _, _) => *e,
        }
    }
}

struct FadingMaterial {
    handle: FadedHandle,
    drawn_at: Option<f32>,
    exit_progress: f32,
}

impl FadingMaterial {
    fn entity(&self) -> Entity {
        self.handle.entity()
    }

    fn progress(&self, elapsed: f32, seconds: f32) -> f32 {
        self.drawn_at
            .map_or(0.0, |start| ((elapsed - start) / seconds).clamp(0.0, 1.0))
    }
}

#[derive(Component)]
pub(crate) struct ChunkFade {
    elapsed: f32,
    seconds: f32,
    outgoing: bool,
    materials: Vec<FadingMaterial>,
    retired_meshes: Vec<Handle<Mesh>>,
    mode: ChunkAnimation,
    original: Transform,
    visibility: Visibility,
    started: bool,
    age: f32,
    start_progress: f32,
}

fn duration(mode: ChunkAnimation) -> f32 {
    match mode {
        ChunkAnimation::None => 0.0,
        ChunkAnimation::Fade => 0.3,
        ChunkAnimation::FadeSlow => 0.75,
        ChunkAnimation::Bounce => 1.0,
        ChunkAnimation::Rise | ChunkAnimation::Slide => 0.9,
    }
}

fn clone_material<M: FadeMaterial>(
    world: &mut World,
    entity: Entity,
    outgoing: bool,
) -> Option<(Handle<M>, Handle<M>)> {
    let original = world.get::<MeshMaterial3d<M>>(entity)?.0.clone();
    let mut material = world.get_resource::<Assets<M>>()?.get(&original)?.clone();
    material.opacity(if outgoing { 1.0 } else { 0.0 });
    let temporary = world.resource_mut::<Assets<M>>().add(material);
    world.entity_mut(entity).insert((
        MeshMaterial3d(temporary.clone()),
        bevy::light::NotShadowCaster,
    ));
    Some((original, temporary))
}

#[derive(Component)]
struct RestoreShadow;

#[derive(Component)]
struct RestoreBatching;

fn start(
    world: &mut World,
    root: Entity,
    mode: ChunkAnimation,
    outgoing: bool,
    retired_meshes: Vec<Handle<Mesh>>,
) -> bool {
    if mode == ChunkAnimation::None
        || world.get_entity(root).is_err()
        || world.query::<&ChunkFade>().iter(world).count() >= MAX_FADING_ROOTS
    {
        return false;
    }
    if let Some(old) = world.entity_mut(root).take::<ChunkFade>() {
        finish(world, root, old);
    }
    if world.get_entity(root).is_err() {
        return false;
    }
    let mut materials = Vec::new();
    for entity in mesh_entities(world, root) {
        restore_queued_shadow(world, entity);
        if world
            .get::<crate::meshing::shadows::ShadowProxy>(entity)
            .is_some()
        {
            continue;
        }
        let restore_shadow = world.get::<bevy::light::NotShadowCaster>(entity).is_none();
        let material =
            if let Some((a, b)) = clone_material::<StandardMaterial>(world, entity, outgoing) {
                Some(FadedHandle::Standard(entity, a, b))
            } else if let Some((a, b)) = clone_material::<LeafMaterial>(world, entity, outgoing) {
                Some(FadedHandle::Leaf(entity, a, b))
            } else if let Some((a, b)) = clone_material::<PropMaterial>(world, entity, outgoing) {
                Some(FadedHandle::Prop(entity, a, b))
            } else if let Some((a, b)) = clone_material::<WetMaterial>(world, entity, outgoing) {
                Some(FadedHandle::Wet(entity, a, b))
            } else {
                clone_material::<WaterMaterial>(world, entity, outgoing)
                    .map(|(a, b)| FadedHandle::Water(entity, a, b))
            };
        if let Some(material) = material {
            if world
                .get::<bevy::render::batching::NoAutomaticBatching>(entity)
                .is_none()
            {
                // Readiness must refer to this mesh, not a representative from
                // another root in the same indirect multi-draw batch.
                world
                    .entity_mut(entity)
                    .insert((bevy::render::batching::NoAutomaticBatching, RestoreBatching));
            }
            if restore_shadow {
                world.entity_mut(entity).insert(RestoreShadow);
            }
            materials.push(FadingMaterial {
                handle: material,
                drawn_at: outgoing.then_some(0.0),
                exit_progress: 1.0,
            });
        }
    }
    let original = world.get::<Transform>(root).copied().unwrap_or_default();
    let visibility = world.get::<Visibility>(root).copied().unwrap_or_default();
    world.entity_mut(root).insert(ChunkFade {
        elapsed: 0.0,
        seconds: duration(mode),
        outgoing,
        materials,
        retired_meshes,
        mode,
        original,
        visibility,
        started: outgoing,
        age: 0.0,
        start_progress: 1.0,
    });
    if !outgoing {
        world.init_resource::<TransitionDraws>();
        let entities = world
            .get::<ChunkFade>(root)
            .map(|fade| {
                fade.materials
                    .iter()
                    .map(FadingMaterial::entity)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        world
            .resource_mut::<TransitionDraws>()
            .pending
            .extend(entities);
        world.entity_mut(root).insert(EnteringChunk);
    }
    true
}

pub(crate) fn enter(commands: &mut Commands, root: Entity, mode: ChunkAnimation) {
    if mode != ChunkAnimation::None {
        commands.queue(move |world: &mut World| {
            let _scope = terra_core::profiling::ScopeTimer::new("transitions.enter_commit");
            let Some(visibility) = world.get::<Visibility>(root).copied() else {
                return;
            };
            world.entity_mut(root).insert((
                Visibility::Hidden,
                EnteringChunk,
                QueuedChunk {
                    mode,
                    outgoing: false,
                    meshes: Vec::new(),
                    visibility,
                },
            ));
            queue_shadows(world, root, visibility);
        });
    }
}

pub(crate) fn retire(
    commands: &mut Commands,
    root: Entity,
    meshes: Vec<Handle<Mesh>>,
    mode: ChunkAnimation,
) {
    commands.queue(move |world: &mut World| {
        let _scope = terra_core::profiling::ScopeTimer::new("transitions.retire_commit");
        if world.get_entity(root).is_err() {
            if let Some(mut assets) = world.get_resource_mut::<Assets<Mesh>>() {
                for mesh in meshes {
                    assets.remove(mesh.id());
                }
            }
            return;
        }
        if mode == ChunkAnimation::None || world.get::<QueuedChunk>(root).is_some() {
            if let Some(fade) = world.entity_mut(root).take::<ChunkFade>() {
                finish(world, root, fade);
            }
            world.despawn(root);
            if let Some(mut assets) = world.get_resource_mut::<Assets<Mesh>>() {
                for mesh in meshes {
                    assets.remove(mesh.id());
                }
            }
            return;
        }
        if let Some(mut fade) = world.get_mut::<ChunkFade>(root) {
            fade.start_progress = (fade.elapsed / fade.seconds).clamp(0.0, 1.0);
            let elapsed = fade.elapsed;
            let seconds = fade.seconds;
            for material in &mut fade.materials {
                material.exit_progress = material.progress(elapsed, seconds);
            }
            fade.elapsed = 0.0;
            fade.started = true;
            fade.outgoing = true;
            fade.retired_meshes = meshes;
        } else if let Some(visibility) = world.get::<Visibility>(root).copied() {
            world.entity_mut(root).insert(QueuedChunk {
                mode,
                outgoing: true,
                meshes,
                visibility,
            });
        }
    });
}

fn update_material<M: FadeMaterial>(
    world: &mut World,
    original: &Handle<M>,
    temporary: &Handle<M>,
    opacity: f32,
) {
    let Some(mut assets) = world.get_resource_mut::<Assets<M>>() else {
        return;
    };
    let Some(alpha) = assets.get(original).map(FadeMaterial::alpha) else {
        return;
    };
    let alpha = alpha * opacity;
    if assets
        .get(temporary)
        .is_none_or(|target| target.alpha() == alpha)
    {
        return;
    }
    if let Some(mut target) = assets.get_mut(temporary) {
        // Only update the opacity uniform, not cloned textures/material state every frame.
        target.set_alpha(alpha);
    }
}

fn restore<M: Material>(
    world: &mut World,
    entity: Entity,
    original: Handle<M>,
    temporary: Handle<M>,
) {
    if let Ok(mut entity) = world.get_entity_mut(entity) {
        entity.insert(MeshMaterial3d(original));
        if entity.take::<RestoreBatching>().is_some() {
            entity.remove::<bevy::render::batching::NoAutomaticBatching>();
        }
        if entity.take::<RestoreShadow>().is_some() {
            entity.remove::<bevy::light::NotShadowCaster>();
        }
    }
    if let Some(mut assets) = world.get_resource_mut::<Assets<M>>() {
        assets.remove(temporary.id());
    }
}

fn finish(world: &mut World, root: Entity, fade: ChunkFade) {
    cleanup(world, root, fade, true);
}

fn cleanup(world: &mut World, root: Entity, fade: ChunkFade, completed: bool) {
    let _scope = terra_core::profiling::ScopeTimer::new("transitions.cleanup");
    if let Some(mut pending) = world.get_resource_mut::<TransitionDraws>() {
        for material in &fade.materials {
            pending.pending.remove(&material.entity());
        }
    }
    for material in fade.materials {
        match material.handle {
            FadedHandle::Standard(e, a, b) => restore(world, e, a, b),
            FadedHandle::Leaf(e, a, b) => restore(world, e, a, b),
            FadedHandle::Prop(e, a, b) => restore(world, e, a, b),
            FadedHandle::Water(e, a, b) => restore(world, e, a, b),
            FadedHandle::Wet(e, a, b) => restore(world, e, a, b),
        }
    }
    if let Ok(mut entity) = world.get_entity_mut(root) {
        entity
            .insert((fade.original, fade.visibility))
            .remove::<EnteringChunk>();
    }
    if fade.outgoing {
        world.despawn(root);
        if let Some(mut meshes) = world.get_resource_mut::<Assets<Mesh>>() {
            for mesh in fade.retired_meshes {
                meshes.remove(mesh.id());
            }
        }
    }
    if completed && let Some(mut status) = world.get_resource_mut::<ChunkAnimationStatus>() {
        status.completed += 1;
    }
}

fn animated_transform(original: Transform, mode: ChunkAnimation, progress: f32) -> Transform {
    let t = progress.clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    let mut result = original;
    match mode {
        ChunkAnimation::Bounce => {
            let x = t - 1.0;
            let back = 1.0 + 2.70158 * x * x * x + 1.70158 * x * x;
            result.translation.y -= 8.0 * (1.0 - back);
        }
        ChunkAnimation::Rise => result.translation.y -= 12.0 * (1.0 - smooth),
        ChunkAnimation::Slide => result.translation += Vec3::new(-8.0, -3.0, 5.0) * (1.0 - smooth),
        _ => {}
    }
    result
}

fn animate(world: &mut World) {
    let _scope = terra_core::profiling::ScopeTimer::new("transitions.animate");
    let dt = world.resource::<Time>().delta_secs().min(1.0 / 30.0);
    let disabled = world.resource::<GameConfig>().client.chunk_animation == ChunkAnimation::None;
    let ready = world
        .get_resource::<TransitionDraws>()
        .map(TransitionDraws::take_ready)
        .unwrap_or_default();
    if world.contains_resource::<TransitionDraws>() {
        world.resource_scope(|world, mut feedback: Mut<TransitionDraws>| {
            feedback
                .pending
                .retain(|entity| world.entities().contains(*entity));
        });
    }
    let roots = world
        .query_filtered::<Entity, With<ChunkFade>>()
        .iter(world)
        .collect::<Vec<_>>();
    if roots.is_empty() && world.query::<&QueuedChunk>().iter(world).next().is_none() {
        if let Some(mut cache) = world.get_resource_mut::<QueueCandidates>() {
            cache.entities.clear();
            cache.refreshed = None;
        }
        update_status(world);
        return;
    }
    let mut completion_views = None;
    for root in roots {
        let Some(mut fade) = world.entity_mut(root).take::<ChunkFade>() else {
            continue;
        };
        fade.age += dt;
        if fade.outgoing
            && fade.age > dt * 2.0
            && fade.materials.iter().all(|material| {
                world
                    .get::<ViewVisibility>(material.entity())
                    .is_some_and(|visibility| !visibility.get())
            })
        {
            finish(world, root, fade);
            continue;
        }
        if !disabled && !fade.outgoing {
            // Each uploaded material batch needs its own first color draw.
            // A ready sibling must not consume a still-uploading batch's fade.
            for material in &mut fade.materials {
                if material.drawn_at.is_none() && ready.contains(&material.entity()) {
                    material.drawn_at = Some(fade.elapsed);
                    if let Some(mut pending) = world.get_resource_mut::<TransitionDraws>() {
                        pending.pending.remove(&material.entity());
                    }
                }
            }
        }
        if !disabled && !fade.started {
            if fade
                .materials
                .iter()
                .any(|material| material.drawn_at.is_some())
            {
                fade.started = true;
                if let Some(mut transform) = world.get_mut::<Transform>(root) {
                    let next = animated_transform(fade.original, fade.mode, 0.0);
                    if *transform != next {
                        *transform = next;
                    }
                }
                if let Some(mut status) = world.get_resource_mut::<ChunkAnimationStatus>() {
                    status.started_after_draw += 1;
                }
            }
            if !fade.started && fade.age > 1.0 {
                let mode = fade.mode;
                let visibility = fade.visibility;
                cleanup(world, root, fade, false);
                world.entity_mut(root).insert((
                    Visibility::Hidden,
                    EnteringChunk,
                    QueuedChunk {
                        mode,
                        outgoing: false,
                        meshes: Vec::new(),
                        visibility,
                    },
                ));
                queue_shadows(world, root, visibility);
                continue;
            }
            world.entity_mut(root).insert(fade);
            continue;
        }
        fade.elapsed += dt;
        let completed = fade.elapsed >= fade.seconds
            && (fade.outgoing
                || fade.materials.iter().all(|material| {
                    if material.progress(fade.elapsed, fade.seconds) >= 1.0 {
                        return true;
                    }
                    // Separate material bounds may be outside the camera. Do
                    // not hold an animation slot for geometry that cannot draw.
                    let views = completion_views.get_or_insert_with(|| camera_views(world));
                    mesh_in_view(world, material.entity(), views) == Some(false)
                }));
        if disabled || completed {
            finish(world, root, fade);
            continue;
        }
        let t = (fade.elapsed / fade.seconds).clamp(0.0, 1.0);
        let progress = if fade.outgoing {
            fade.start_progress * (1.0 - t)
        } else {
            t
        };
        if let Some(mut transform) = world.get_mut::<Transform>(root) {
            let next = animated_transform(fade.original, fade.mode, progress);
            if *transform != next {
                *transform = next;
            }
        }
        for material in &fade.materials {
            let progress = if fade.outgoing {
                material.exit_progress * (1.0 - t)
            } else {
                material.progress(fade.elapsed, fade.seconds)
            };
            let opacity = progress * progress * (3.0 - 2.0 * progress);
            match &material.handle {
                FadedHandle::Standard(_, a, b) => update_material(world, a, b, opacity),
                FadedHandle::Leaf(_, a, b) => update_material(world, a, b, opacity),
                FadedHandle::Prop(_, a, b) => update_material(world, a, b, opacity),
                FadedHandle::Water(_, a, b) => update_material(world, a, b, opacity),
                FadedHandle::Wet(_, a, b) => update_material(world, a, b, opacity),
            }
        }
        world.entity_mut(root).insert(fade);
    }
    let active = world.query::<&ChunkFade>().iter(world).count();
    let budget = MAX_FADING_ROOTS.saturating_sub(active).min(8);
    let now = world.resource::<Time>().elapsed_secs_f64();
    let mut candidates = world
        .remove_resource::<QueueCandidates>()
        .unwrap_or_default();
    if disabled || candidates.refreshed.is_none_or(|last| now - last >= 0.1) {
        let focus = world
            .get_resource::<terra_world::streaming::ChunkFocus>()
            .map_or(Vec3::ZERO, |f| Vec3::from_array(f.0.map(|v| v as f32)));
        let frusta = camera_views(world);
        let mut expired = Vec::new();
        let mut background = Vec::new();
        let mut queued = world
            .query::<(Entity, &QueuedChunk, Option<&Transform>)>()
            .iter(world)
            .map(|(entity, queued, t)| {
                (
                    entity,
                    queued.outgoing,
                    t.map_or(0.0, |t| t.translation.distance_squared(focus)),
                )
            })
            .filter(|(entity, outgoing, distance)| {
                let visible = frusta.is_empty() || in_view(world, *entity, &frusta);
                if !disabled && *outgoing && !visible {
                    expired.push(*entity);
                } else if !disabled && !visible && known_outside_view(world, *entity, &frusta) {
                    background.push((*entity, *distance));
                }
                disabled || visible
            })
            .collect::<Vec<_>>();
        // Roots already outside the real view cannot show a fade-out. Do not
        // spend material/animation slots on them while incoming chunks wait.
        for root in expired {
            if let Some(queued) = world.entity_mut(root).take::<QueuedChunk>() {
                retire_meshes(world, root, queued.meshes);
            }
        }
        // Prepare the full surroundings without reserving fades for meshes
        // that cannot draw. Missing bounds still wait for render preparation.
        background.sort_unstable_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        for (root, _) in background.into_iter().take(MAX_BACKGROUND_ROOTS) {
            if let Some(queued) = world.entity_mut(root).take::<QueuedChunk>() {
                reveal(world, root, queued);
            }
        }
        queued.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.2.total_cmp(&b.2))
                .then_with(|| a.0.cmp(&b.0))
        });
        candidates.entities = queued
            .into_iter()
            .rev()
            .map(|(entity, _, _)| entity)
            .collect();
        candidates.refreshed = Some(now);
    }
    let mut admitted = 0;
    while (disabled || admitted < budget) && !candidates.entities.is_empty() {
        let Some(root) = candidates.entities.pop() else {
            break;
        };
        if world.get_entity(root).is_err() {
            continue;
        }
        let Some(queued) = world.entity_mut(root).take::<QueuedChunk>() else {
            continue;
        };
        admitted += 1;
        if disabled {
            reveal(world, root, queued);
        } else {
            world.entity_mut(root).insert(queued.visibility);
            start(world, root, queued.mode, queued.outgoing, queued.meshes);
        }
    }
    if let Some(mut status) = world.get_resource_mut::<ChunkAnimationStatus>() {
        status.queued_in_view = candidates.entities.len();
    }
    world.insert_resource(candidates);
    update_status(world);
}

fn reveal(world: &mut World, root: Entity, queued: QueuedChunk) {
    world.entity_mut(root).insert(queued.visibility);
    for entity in mesh_entities(world, root) {
        restore_queued_shadow(world, entity);
    }
    world.entity_mut(root).remove::<EnteringChunk>();
    if queued.outgoing {
        retire_meshes(world, root, queued.meshes);
    }
}

fn known_outside_view(
    world: &World,
    root: Entity,
    frusta: &[(bevy::camera::primitives::Frustum, Vec3)],
) -> bool {
    // Shadow proxies never produce the color-draw feedback required by a fade.
    let meshes: Vec<_> = mesh_entities(world, root)
        .into_iter()
        .filter(|entity| {
            world
                .get::<crate::meshing::shadows::ShadowProxy>(*entity)
                .is_none()
        })
        .collect();
    !meshes.is_empty()
        && meshes.iter().all(|entity| {
            transforms_current(world, *entity)
                && mesh_in_view(world, *entity, frusta) == Some(false)
        })
}

fn transforms_current(world: &World, entity: Entity) -> bool {
    // Admission precedes transform propagation. Fresh child bounds must not be
    // classified at identity before their section's translation reaches them.
    let mut current = Some(entity);
    while let Some(entity) = current {
        let (Some(local), Some(global)) = (
            world.get::<Transform>(entity),
            world.get::<GlobalTransform>(entity),
        ) else {
            return false;
        };
        current = world.get::<ChildOf>(entity).map(ChildOf::parent);
        let expected = if let Some(parent) = current {
            let Some(parent) = world.get::<GlobalTransform>(parent) else {
                return false;
            };
            parent.mul_transform(*local)
        } else {
            GlobalTransform::from(*local)
        };
        if !global.affine().abs_diff_eq(expected.affine(), 0.0001) {
            return false;
        }
    }
    true
}

fn camera_views(world: &mut World) -> Vec<(bevy::camera::primitives::Frustum, Vec3)> {
    world
        .query_filtered::<
            (&bevy::camera::primitives::Frustum, &GlobalTransform),
            With<terra_player::TerraCamera>,
        >()
        .iter(world)
        .map(|(frustum, transform)| (*frustum, transform.translation()))
        .collect()
}

fn mesh_in_view(
    world: &World,
    entity: Entity,
    frusta: &[(bevy::camera::primitives::Frustum, Vec3)],
) -> Option<bool> {
    if frusta.is_empty() {
        return Some(true);
    }
    world
        .get::<bevy::camera::primitives::Aabb>(entity)
        .zip(world.get::<GlobalTransform>(entity))
        .map(|(bounds, transform)| {
            frusta.iter().any(|(frustum, eye)| {
                let in_range = world.get::<VisibilityRange>(entity).is_none_or(|range| {
                    let center = if range.use_aabb {
                        transform.affine().transform_point3a(bounds.center).into()
                    } else {
                        transform.translation()
                    };
                    range.is_visible_at_all(center.distance(*eye))
                });
                in_range && frustum.intersects_obb(bounds, &transform.affine(), true, true)
            })
        })
}

fn in_view(
    world: &World,
    root: Entity,
    frusta: &[(bevy::camera::primitives::Frustum, Vec3)],
) -> bool {
    let visible = |entity| {
        world
            .get::<crate::meshing::shadows::ShadowProxy>(entity)
            .is_none()
            && mesh_in_view(world, entity, frusta).unwrap_or(false)
    };
    visible(root)
        || world
            .get::<Children>(root)
            .is_some_and(|children| children.iter().any(visible))
}

fn update_status(world: &mut World) {
    let queued = world.query::<&QueuedChunk>().iter(world).count();
    let active = world.query::<&ChunkFade>().iter(world).count();
    let mut waiting = 0;
    let mut waiting_materials = [0; 4];
    for fade in world
        .query::<&ChunkFade>()
        .iter(world)
        .filter(|fade| !fade.outgoing)
    {
        let mut has_waiting = false;
        for material in fade
            .materials
            .iter()
            .filter(|material| material.drawn_at.is_none())
        {
            has_waiting = true;
            let kind = match &material.handle {
                FadedHandle::Standard(..) | FadedHandle::Wet(..) => 0,
                FadedHandle::Leaf(..) => 1,
                FadedHandle::Prop(..) => 2,
                FadedHandle::Water(..) => 3,
            };
            waiting_materials[kind] += 1;
        }
        waiting += usize::from(has_waiting);
    }
    if let Some(mut status) = world.get_resource_mut::<ChunkAnimationStatus>() {
        status.queued = queued;
        if queued == 0 {
            status.queued_in_view = 0;
        }
        status.active = active;
        status.waiting_for_draw = waiting;
        status.waiting_materials = waiting_materials;
        status.peak_active = status.peak_active.max(active);
    }
}

fn retire_meshes(world: &mut World, root: Entity, meshes: Vec<Handle<Mesh>>) {
    let _scope = terra_core::profiling::ScopeTimer::new("transitions.retire_meshes");
    world.despawn(root);
    if let Some(mut assets) = world.get_resource_mut::<Assets<Mesh>>() {
        for mesh in meshes {
            assets.remove(mesh.id());
        }
    }
}

#[derive(Resource, Default)]
pub struct LodSwapStatus {
    pub completed: usize,
}

pub(crate) fn install(app: &mut App) {
    fixture::install(app);
    wet_probe::install(app);
    app.init_resource::<LodSwapStatus>()
        .init_resource::<ChunkAnimationStatus>()
        .init_resource::<QueueCandidates>()
        .init_resource::<TransitionDraws>()
        .add_plugins(bevy::render::extract_resource::ExtractResourcePlugin::<
            TransitionDraws,
        >::default())
        .add_systems(PostUpdate, animate.before(TransformSystems::Propagate));
}

pub(crate) fn record_swap(commands: &mut Commands) {
    commands.queue(|world: &mut World| {
        world.resource_mut::<LodSwapStatus>().completed += 1;
    });
}

pub(crate) fn abrupt_prop_range(end: f32) -> VisibilityRange {
    // This limit lies behind the fog. Fade margins would enable color dithering
    // without matching depth/shadow masking in Bevy's depth-only rendering path.
    VisibilityRange::abrupt(0.0, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_release_keeps_outgoing_queued_color_visible_until_fade_admission() {
        let mut world = World::new();
        let root = world
            .spawn((
                Visibility::Inherited,
                QueuedChunk {
                    mode: ChunkAnimation::Fade,
                    outgoing: true,
                    meshes: Vec::new(),
                    visibility: Visibility::Inherited,
                },
            ))
            .id();
        let child = world
            .spawn((
                Mesh3d::default(),
                bevy::light::NotShadowCaster,
                RenderLayers::default(),
                Visibility::Inherited,
                ChildOf(root),
            ))
            .id();
        restore_proxy_caster(&mut world, child);
        assert!(world.get::<bevy::light::NotShadowCaster>(child).is_none());
        assert!(world.get::<QueuedShadow>(child).is_none());
        assert_eq!(
            world.get::<RenderLayers>(child),
            Some(&RenderLayers::default())
        );
        assert_eq!(world.get::<Visibility>(child), Some(&Visibility::Inherited));
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Inherited));
        assert!(world.get::<QueuedChunk>(root).unwrap().outgoing);
    }

    #[test]
    fn proxy_release_restores_queued_and_fading_casters_without_exposing_color() {
        let mut world = World::new();
        world.init_resource::<Assets<StandardMaterial>>();
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let root = world
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        let child = world
            .spawn((
                Mesh3d::default(),
                MeshMaterial3d(material.clone()),
                bevy::light::NotShadowCaster,
                RenderLayers::default(),
                Visibility::Inherited,
                ChildOf(root),
            ))
            .id();
        world.entity_mut(root).insert((
            Visibility::Hidden,
            QueuedChunk {
                mode: ChunkAnimation::Fade,
                outgoing: false,
                meshes: Vec::new(),
                visibility: Visibility::Inherited,
            },
        ));
        restore_proxy_caster(&mut world, child);
        assert!(world.get::<bevy::light::NotShadowCaster>(child).is_none());
        assert!(world.get::<QueuedShadow>(child).is_some());
        assert_eq!(
            world.get::<RenderLayers>(child),
            Some(&RenderLayers::layer(QUEUE_SHADOW_LAYER))
        );
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Hidden));
        let fading = world
            .spawn((
                Mesh3d::default(),
                MeshMaterial3d(material),
                bevy::light::NotShadowCaster,
                ChildOf(root),
            ))
            .id();
        assert!(start(
            &mut world,
            root,
            ChunkAnimation::Fade,
            false,
            Vec::new()
        ));
        world.entity_mut(root).remove::<QueuedChunk>();
        assert!(world.get::<RestoreShadow>(fading).is_none());
        restore_proxy_caster(&mut world, fading);
        assert!(world.get::<bevy::light::NotShadowCaster>(fading).is_some());
        assert!(world.get::<RestoreShadow>(fading).is_some());
        restore_proxy_caster(&mut world, child);
        assert!(world.get::<bevy::light::NotShadowCaster>(child).is_some());
        assert!(world.get::<RestoreShadow>(child).is_some());
        assert_eq!(
            world.get::<RenderLayers>(child),
            Some(&RenderLayers::default())
        );
        let fade = world.entity_mut(root).take::<ChunkFade>().unwrap();
        finish(&mut world, root, fade);
        assert!(world.get::<bevy::light::NotShadowCaster>(child).is_none());
        assert!(world.get::<RestoreShadow>(child).is_none());
        assert!(world.get::<bevy::light::NotShadowCaster>(fading).is_none());
        assert!(world.get::<RestoreShadow>(fading).is_none());
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
    }

    fn repeated_material_fade_lifecycle<M: FadeMaterial>(material: M) {
        let mut world = World::new();
        world.init_resource::<Assets<M>>();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Time>();
        world.init_resource::<ChunkAnimationStatus>();
        let mut config = GameConfig::default();
        config.client.chunk_animation = ChunkAnimation::Fade;
        world.insert_resource(config);
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(1.0 / 30.0));
        let shared = world.resource_mut::<Assets<M>>().add(material);
        let mut settled_entity_count = None;
        for cycle in 0..100 {
            let mesh = world
                .resource_mut::<Assets<Mesh>>()
                .add(Mesh::from(Cuboid::default()));
            let root = world
                .spawn((Transform::default(), Visibility::Inherited))
                .id();
            let child = world
                .spawn((
                    ChildOf(root),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(shared.clone()),
                ))
                .id();
            assert!(start(
                &mut world,
                root,
                ChunkAnimation::Fade,
                false,
                Vec::new()
            ));
            assert_eq!(world.resource::<Assets<M>>().len(), 2, "cycle {cycle}");
            world.resource::<TransitionDraws>().drawn(child);
            for _ in 0..20 {
                animate(&mut world);
            }
            assert!(world.get::<ChunkFade>(root).is_none());
            assert_eq!(world.get::<MeshMaterial3d<M>>(child).unwrap().0, shared);
            assert_eq!(world.resource::<Assets<M>>().len(), 1);
            assert!(world.resource::<TransitionDraws>().pending.is_empty());
            retire(
                &mut world.commands(),
                root,
                vec![mesh.clone()],
                ChunkAnimation::Fade,
            );
            world.flush();
            for _ in 0..22 {
                animate(&mut world);
            }
            assert!(world.get_entity(root).is_err(), "cycle {cycle}");
            assert!(world.get_entity(child).is_err());
            // Lazy Bevy component metadata keeps its own entities alive after
            // the first cycle; only scene entities must disappear on retire.
            let entity_count = world.query::<Entity>().iter(&world).count();
            let baseline = *settled_entity_count.get_or_insert(entity_count);
            assert_eq!(entity_count, baseline, "cycle {cycle}");
            assert!(world.resource::<Assets<Mesh>>().is_empty());
            assert_eq!(world.resource::<Assets<M>>().len(), 1);
            assert!(world.resource::<Assets<M>>().contains(shared.id()));
            assert!(world.resource::<TransitionDraws>().pending.is_empty());
            assert_eq!(world.resource::<ChunkAnimationStatus>().active, 0);
            assert_eq!(world.resource::<ChunkAnimationStatus>().queued, 0);
        }
    }

    #[test]
    fn repeated_chunk_fades_release_temporary_materials_and_retired_meshes() {
        repeated_material_fade_lifecycle(StandardMaterial::default());
        repeated_material_fade_lifecycle(PropMaterial::default());
        repeated_material_fade_lifecycle(WetMaterial::default());
        repeated_material_fade_lifecycle(WaterMaterial {
            base: StandardMaterial::default(),
            extension: crate::water::material::WaterExtension {
                native_depth_filter: false,
                tint: Vec4::ONE,
                settings: Vec4::ZERO,
                effects: Vec4::ZERO,
                reflections: Vec4::ZERO,
                weather: Vec4::ZERO,
            },
        });
    }

    #[test]
    fn shadow_only_proxies_never_wait_for_color_draw_feedback_or_clone_fade_materials() {
        let mut world = World::new();
        world.init_resource::<Assets<StandardMaterial>>();
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let root = world
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        let color = world
            .spawn((
                ChildOf(root),
                Mesh3d::default(),
                MeshMaterial3d(material.clone()),
            ))
            .id();
        let proxy = world
            .spawn((
                ChildOf(root),
                Mesh3d::default(),
                MeshMaterial3d(material.clone()),
                crate::meshing::shadows::ShadowProxy,
                RenderLayers::layer(crate::meshing::shadows::LAYER),
            ))
            .id();
        assert!(start(&mut world, root, ChunkAnimation::Fade, false, vec![]));
        let pending = &world.resource::<TransitionDraws>().pending;
        assert!(pending.contains(&color));
        assert!(!pending.contains(&proxy));
        assert_eq!(world.get::<ChunkFade>(root).unwrap().materials.len(), 1);
        assert_eq!(
            world
                .get::<MeshMaterial3d<StandardMaterial>>(proxy)
                .unwrap()
                .0,
            material
        );
        assert!(world.get::<bevy::light::NotShadowCaster>(proxy).is_none());
    }

    fn two_batch_fade() -> (World, Entity, [Entity; 2], Handle<StandardMaterial>) {
        let mut world = World::new();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Time>();
        world.init_resource::<ChunkAnimationStatus>();
        let mut config = GameConfig::default();
        config.client.chunk_animation = ChunkAnimation::Fade;
        world.insert_resource(config);
        let original = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let root = world
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        let children = std::array::from_fn(|_| {
            world
                .spawn((
                    ChildOf(root),
                    Mesh3d::default(),
                    MeshMaterial3d(original.clone()),
                ))
                .id()
        });
        assert!(start(
            &mut world,
            root,
            ChunkAnimation::Fade,
            false,
            Vec::new()
        ));
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(1.0 / 30.0));
        (world, root, children, original)
    }

    fn alpha(world: &World, entity: Entity) -> f32 {
        let handle = &world
            .get::<MeshMaterial3d<StandardMaterial>>(entity)
            .unwrap()
            .0;
        world
            .resource::<Assets<StandardMaterial>>()
            .get(handle)
            .unwrap()
            .base_color
            .alpha()
    }

    #[test]
    fn fresh_batches_fade_from_their_own_draw_without_delaying_ready_siblings() {
        let (mut world, root, [first, late], original) = two_batch_fade();
        world.resource::<TransitionDraws>().drawn(first);
        animate(&mut world);
        for _ in 0..5 {
            animate(&mut world);
        }
        assert!(alpha(&world, first) > 0.4);
        assert_eq!(alpha(&world, late), 0.0);
        for _ in 0..10 {
            animate(&mut world);
        }
        assert_eq!(alpha(&world, first), 1.0);
        assert_eq!(alpha(&world, late), 0.0);
        assert!(world.get::<ChunkFade>(root).is_some());
        let pending = &world.resource::<TransitionDraws>().pending;
        assert!(!pending.contains(&first));
        assert!(pending.contains(&late));
        assert_eq!(
            world.resource::<ChunkAnimationStatus>().waiting_materials,
            [1, 0, 0, 0]
        );
        world.resource::<TransitionDraws>().drawn(late);
        for _ in 0..4 {
            animate(&mut world);
        }
        assert!(alpha(&world, late) > 0.3 && alpha(&world, late) < 0.6);
        assert_eq!(alpha(&world, first), 1.0);
        for _ in 0..10 {
            animate(&mut world);
        }
        assert!(world.get::<ChunkFade>(root).is_none());
        for entity in [first, late] {
            assert_eq!(
                world
                    .get::<MeshMaterial3d<StandardMaterial>>(entity)
                    .unwrap()
                    .0,
                original
            );
        }
        assert!(world.resource::<TransitionDraws>().pending.is_empty());
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
    }

    #[test]
    fn offscreen_material_batches_do_not_hold_completed_animation_slots() {
        use bevy::camera::{CameraProjection, primitives::Aabb};
        let (mut world, root, [first, offscreen], _) = two_batch_fade();
        for (entity, z) in [(first, -10.0), (offscreen, 10.0)] {
            world.entity_mut(entity).insert((
                Aabb {
                    center: Vec3::ZERO.into(),
                    half_extents: Vec3::ONE.into(),
                },
                GlobalTransform::from_xyz(0.0, 0.0, z),
            ));
        }
        world.spawn((
            terra_player::TerraCamera,
            GlobalTransform::IDENTITY,
            PerspectiveProjection::default().compute_frustum(&GlobalTransform::IDENTITY),
        ));
        world.resource::<TransitionDraws>().drawn(first);
        for _ in 0..15 {
            animate(&mut world);
        }
        assert!(world.get::<ChunkFade>(root).is_none());
        assert!(world.resource::<TransitionDraws>().pending.is_empty());
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
    }

    #[test]
    fn retiring_a_partial_fresh_chunk_does_not_flash_its_undrawn_materials() {
        let (mut world, root, [first, late], _) = two_batch_fade();
        world.resource::<TransitionDraws>().drawn(first);
        for _ in 0..5 {
            animate(&mut world);
        }
        let before = alpha(&world, first);
        retire(
            &mut world.commands(),
            root,
            Vec::new(),
            ChunkAnimation::Fade,
        );
        world.flush();
        animate(&mut world);
        assert!(alpha(&world, first) <= before);
        assert_eq!(alpha(&world, late), 0.0);
        for _ in 0..15 {
            animate(&mut world);
        }
        assert!(world.get_entity(root).is_err());
        assert!(world.resource::<TransitionDraws>().pending.is_empty());
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
    }

    #[test]
    fn fades_restore_shared_materials_and_retire_meshes_only_after_completion() {
        let mut world = World::new();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Time>();
        world.init_resource::<ChunkAnimationStatus>();
        let mut config = GameConfig::default();
        config.client.chunk_animation = ChunkAnimation::Fade;
        world.insert_resource(config);
        let original = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let mesh = world
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Cuboid::default()));
        let root = world
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        let child = world
            .spawn((
                ChildOf(root),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(original.clone()),
            ))
            .id();
        assert!(!start(
            &mut world,
            root,
            ChunkAnimation::None,
            false,
            Vec::new()
        ));
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
        assert!(start(
            &mut world,
            root,
            ChunkAnimation::Fade,
            false,
            Vec::new()
        ));
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(1.0 / 30.0));
        for _ in 0..8 {
            animate(&mut world);
        }
        let current = world
            .get::<MeshMaterial3d<StandardMaterial>>(child)
            .unwrap()
            .0
            .clone();
        assert_ne!(current, original);
        let materials = world.resource::<Assets<StandardMaterial>>();
        // CPU time and upload alone must never consume an incoming animation.
        assert_eq!(materials.get(&current).unwrap().base_color.alpha(), 0.0);
        assert_eq!(materials.get(&original).unwrap().base_color.alpha(), 1.0);
        world.resource::<TransitionDraws>().drawn(child);
        animate(&mut world);
        assert_eq!(
            world.resource::<ChunkAnimationStatus>().started_after_draw,
            1
        );
        for _ in 0..4 {
            animate(&mut world);
        }
        let alpha = world
            .resource::<Assets<StandardMaterial>>()
            .get(&current)
            .unwrap()
            .base_color
            .alpha();
        assert!(alpha > 0.4 && alpha < 0.6, "{alpha}");
        for _ in 0..10 {
            animate(&mut world);
        }
        assert_eq!(
            world
                .get::<MeshMaterial3d<StandardMaterial>>(child)
                .unwrap()
                .0,
            original
        );
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
        assert!(world.get::<bevy::light::NotShadowCaster>(child).is_none());
        assert!(start(
            &mut world,
            root,
            ChunkAnimation::Fade,
            true,
            vec![mesh.clone()]
        ));
        animate(&mut world);
        assert!(world.get_entity(root).is_ok());
        assert!(world.resource::<Assets<Mesh>>().get(&mesh).is_some());
        for _ in 0..20 {
            animate(&mut world);
        }
        assert!(world.get_entity(root).is_err());
        assert!(world.resource::<Assets<Mesh>>().get(&mesh).is_none());
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
    }

    #[test]
    fn queued_roots_stay_hidden_until_admitted_and_have_no_instant_budget_fallback() {
        let mut world = World::new();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Time>();
        let mut config = GameConfig::default();
        config.client.chunk_animation = ChunkAnimation::Bounce;
        world.insert_resource(config);
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        for _ in 0..MAX_FADING_ROOTS + 10 {
            let root = world
                .spawn((Transform::default(), Visibility::Inherited))
                .id();
            world.spawn((
                ChildOf(root),
                Mesh3d::default(),
                MeshMaterial3d(material.clone()),
            ));
            enter(&mut world.commands(), root, ChunkAnimation::Bounce);
        }
        world.flush();
        assert_eq!(
            world.query::<&QueuedChunk>().iter(&world).count(),
            MAX_FADING_ROOTS + 10
        );
        assert!(
            world
                .query::<(&QueuedChunk, &Visibility)>()
                .iter(&world)
                .all(|(_, v)| *v == Visibility::Hidden)
        );
        for _ in 0..20 {
            animate(&mut world);
        }
        assert_eq!(
            world.query::<&ChunkFade>().iter(&world).count(),
            MAX_FADING_ROOTS
        );
        assert_eq!(world.query::<&QueuedChunk>().iter(&world).count(), 10);
        assert!(
            world
                .query::<(&QueuedChunk, &Visibility)>()
                .iter(&world)
                .all(|(_, v)| *v == Visibility::Hidden)
        );
        assert_eq!(
            world.resource::<Assets<StandardMaterial>>().len(),
            MAX_FADING_ROOTS + 1
        );
        world.resource_mut::<GameConfig>().client.chunk_animation = ChunkAnimation::None;
        animate(&mut world);
        assert_eq!(world.query::<&QueuedChunk>().iter(&world).count(), 0);
        assert_eq!(world.query::<&ChunkFade>().iter(&world).count(), 0);
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
    }

    #[test]
    fn offscreen_chunks_finish_without_fades_but_visible_or_unprepared_chunks_wait() {
        use bevy::camera::{CameraProjection, primitives::Aabb};
        let mut world = World::new();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Time>();
        let mut config = GameConfig::default();
        config.client.chunk_animation = ChunkAnimation::Rise;
        world.insert_resource(config);
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let camera = world
            .spawn((
                terra_player::TerraCamera,
                GlobalTransform::IDENTITY,
                PerspectiveProjection::default().compute_frustum(&GlobalTransform::IDENTITY),
            ))
            .id();
        let mut roots = Vec::new();
        for (position, prepared) in [
            (Vec3::new(-8.0, 0.0, -32.0), true),
            (Vec3::new(-8.0, 0.0, 32.0), true),
            (Vec3::new(-8.0, 0.0, 64.0), false),
        ] {
            let root = world
                .spawn((
                    Transform::from_translation(position),
                    GlobalTransform::from_translation(position),
                    Visibility::Inherited,
                ))
                .id();
            let child = world
                .spawn((
                    ChildOf(root),
                    Mesh3d::default(),
                    MeshMaterial3d(material.clone()),
                    GlobalTransform::from_translation(position),
                ))
                .id();
            if prepared {
                world.entity_mut(child).insert(Aabb {
                    center: Vec3::ZERO.into(),
                    half_extents: Vec3::splat(3.0).into(),
                });
            }
            enter(&mut world.commands(), root, ChunkAnimation::Rise);
            roots.push((root, child));
        }
        world.flush();
        animate(&mut world);
        let [(front, _), (back, back_mesh), (unprepared, _)] = roots.as_slice() else {
            panic!("missing fixture roots")
        };
        assert!(world.get::<ChunkFade>(*front).is_some());
        assert!(world.get::<QueuedChunk>(*back).is_none());
        assert!(world.get::<ChunkFade>(*back).is_none());
        assert!(world.get::<EnteringChunk>(*back).is_none());
        assert_eq!(world.get::<Visibility>(*back), Some(&Visibility::Inherited));
        assert!(world.get::<QueuedShadow>(*back_mesh).is_none());
        assert!(world.get::<RenderLayers>(*back_mesh).is_none());
        assert_eq!(
            world
                .get::<MeshMaterial3d<StandardMaterial>>(*back_mesh)
                .unwrap()
                .0,
            material
        );
        assert!(world.get::<QueuedChunk>(*unprepared).is_some());
        assert_eq!(
            world.get::<Visibility>(*unprepared),
            Some(&Visibility::Hidden)
        );
        let views = camera_views(&mut world);
        world.entity_mut(*back).insert(GlobalTransform::IDENTITY);
        assert!(
            !known_outside_view(&world, *back, &views),
            "unpropagated transforms cannot bypass the first-draw fade"
        );
        world
            .entity_mut(*back)
            .insert(GlobalTransform::from_xyz(-8.0, 0.0, 32.0));
        assert!(known_outside_view(&world, *back, &views));
        // Looking back at prepared geometry must not restart its admission/fade.
        let pose = GlobalTransform::from(Transform::from_rotation(Quat::from_rotation_y(
            std::f32::consts::PI,
        )));
        world.entity_mut(camera).insert((
            pose,
            PerspectiveProjection::default().compute_frustum(&pose),
        ));
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(110));
        animate(&mut world);
        assert!(world.get::<ChunkFade>(*back).is_none());
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 2);
    }

    #[test]
    fn visible_shadow_proxy_cannot_hold_an_offscreen_color_root_in_draw_readiness() {
        use bevy::camera::{CameraProjection, primitives::Aabb};
        let mut world = World::new();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Time>();
        world.init_resource::<ChunkAnimationStatus>();
        let mut config = GameConfig::default();
        config.client.chunk_animation = ChunkAnimation::Rise;
        world.insert_resource(config);
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        world.spawn((
            terra_player::TerraCamera,
            GlobalTransform::IDENTITY,
            PerspectiveProjection::default().compute_frustum(&GlobalTransform::IDENTITY),
        ));
        let root = world
            .spawn((
                Transform::IDENTITY,
                GlobalTransform::IDENTITY,
                Visibility::Inherited,
            ))
            .id();
        for (z, shadow_only) in [(20.0, false), (-20.0, true)] {
            let mut child = world.spawn((
                ChildOf(root),
                Mesh3d::default(),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(0.0, 0.0, z),
                GlobalTransform::from_xyz(0.0, 0.0, z),
                Aabb {
                    center: Vec3::ZERO.into(),
                    half_extents: Vec3::ONE.into(),
                },
            ));
            if shadow_only {
                child.insert((
                    crate::meshing::shadows::ShadowProxy,
                    RenderLayers::layer(crate::meshing::shadows::LAYER),
                ));
            }
        }
        let views = camera_views(&mut world);
        assert!(!in_view(&world, root, &views));
        assert!(known_outside_view(&world, root, &views));
        enter(&mut world.commands(), root, ChunkAnimation::Rise);
        world.flush();
        animate(&mut world);
        assert!(world.get::<QueuedChunk>(root).is_none());
        assert!(world.get::<ChunkFade>(root).is_none());
        assert!(world.get::<EnteringChunk>(root).is_none());
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Inherited));
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
        let status = world.resource::<ChunkAnimationStatus>();
        assert_eq!(status.active, 0);
        assert_eq!(status.waiting_for_draw, 0);
    }

    #[test]
    fn full_animation_budget_does_not_block_background_preparation() {
        use bevy::camera::{CameraProjection, primitives::Aabb};
        let (mut world, _, _, material) = two_batch_fade();
        for _ in 1..MAX_FADING_ROOTS {
            let root = world
                .spawn((Transform::default(), Visibility::Inherited))
                .id();
            world.spawn((
                ChildOf(root),
                Mesh3d::default(),
                MeshMaterial3d(material.clone()),
            ));
            assert!(start(
                &mut world,
                root,
                ChunkAnimation::Fade,
                false,
                Vec::new()
            ));
        }
        world.spawn((
            terra_player::TerraCamera,
            GlobalTransform::IDENTITY,
            PerspectiveProjection::default().compute_frustum(&GlobalTransform::IDENTITY),
        ));
        let root = world
            .spawn((
                Transform::from_xyz(0.0, 0.0, 32.0),
                GlobalTransform::from_xyz(0.0, 0.0, 32.0),
                Visibility::Inherited,
            ))
            .id();
        world.spawn((
            ChildOf(root),
            Mesh3d::default(),
            MeshMaterial3d(material.clone()),
            GlobalTransform::from_xyz(0.0, 0.0, 32.0),
            Aabb {
                center: Vec3::ZERO.into(),
                half_extents: Vec3::ONE.into(),
            },
        ));
        enter(&mut world.commands(), root, ChunkAnimation::Fade);
        world.flush();
        animate(&mut world);
        assert!(world.get::<QueuedChunk>(root).is_none());
        assert!(world.get::<ChunkFade>(root).is_none());
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Inherited));
        assert_eq!(
            world.query::<&ChunkFade>().iter(&world).count(),
            MAX_FADING_ROOTS
        );
    }

    #[test]
    fn queued_shadow_casters_are_color_hidden_and_restore_their_original_layers() {
        let mut world = World::new();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Time>();
        let mut config = GameConfig::default();
        config.client.chunk_animation = ChunkAnimation::Fade;
        world.insert_resource(config);
        let original = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let root = world
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        let child = world
            .spawn((
                ChildOf(root),
                Mesh3d::default(),
                MeshMaterial3d(original.clone()),
            ))
            .id();
        enter(&mut world.commands(), root, ChunkAnimation::Fade);
        world.flush();
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Hidden));
        assert_eq!(world.get::<Visibility>(child), Some(&Visibility::Visible));
        let layers = world.get::<RenderLayers>(child).unwrap();
        assert!(!layers.intersects(&RenderLayers::default()));
        assert!(layers.intersects(&RenderLayers::default().with(QUEUE_SHADOW_LAYER)));
        assert!(world.get::<bevy::light::NotShadowCaster>(child).is_none());
        animate(&mut world);
        assert!(world.get::<QueuedShadow>(child).is_none());
        assert!(world.get::<RenderLayers>(child).is_none());
        assert_eq!(world.get::<Visibility>(child), Some(&Visibility::Inherited));
        assert!(world.get::<bevy::light::NotShadowCaster>(child).is_some());
        world.resource_mut::<GameConfig>().client.chunk_animation = ChunkAnimation::None;
        animate(&mut world);
        assert!(world.get::<bevy::light::NotShadowCaster>(child).is_none());
        assert_eq!(
            world
                .get::<MeshMaterial3d<StandardMaterial>>(child)
                .unwrap()
                .0,
            original
        );
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
    }

    #[test]
    fn queue_visibility_uses_actual_mesh_bounds_at_negative_coordinates() {
        use bevy::camera::{CameraProjection, primitives::Aabb};
        let mut world = World::new();
        let root = world.spawn(Transform::from_xyz(-16.0, 0.0, -16.0)).id();
        let child = world
            .spawn((
                ChildOf(root),
                Aabb {
                    center: Vec3::splat(8.0).into(),
                    half_extents: Vec3::splat(3.0).into(),
                },
                GlobalTransform::from_xyz(-16.0, 0.0, -16.0),
            ))
            .id();
        let frustum = PerspectiveProjection::default()
            .compute_frustum(&GlobalTransform::from_xyz(-8.0, 8.0, 32.0));
        let views = [(frustum, Vec3::new(-8.0, 8.0, 32.0))];
        assert!(in_view(&world, root, &views));
        world
            .entity_mut(child)
            .insert(VisibilityRange::abrupt(0.0, 32.0));
        assert!(!in_view(&world, root, &views));
        world.entity_mut(child).remove::<VisibilityRange>();
        world
            .entity_mut(child)
            .insert(GlobalTransform::from_xyz(-16.0, -1000.0, -16.0));
        assert!(!in_view(&world, root, &views));
    }

    #[test]
    fn motion_modes_preserve_final_cube_alignment_and_bounce_has_an_overshoot() {
        let original = Transform::from_xyz(-32.0, 48.0, -160.0);
        for mode in [
            ChunkAnimation::Bounce,
            ChunkAnimation::Rise,
            ChunkAnimation::Slide,
        ] {
            assert_eq!(animated_transform(original, mode, 1.0), original);
            assert_ne!(animated_transform(original, mode, 0.0), original);
        }
        assert!(
            animated_transform(original, ChunkAnimation::Bounce, 0.7)
                .translation
                .y
                > original.translation.y
        );
        assert_eq!(
            animated_transform(original, ChunkAnimation::Fade, 0.0),
            original
        );
    }

    #[test]
    fn prop_distance_culling_never_enables_the_dither_pipeline() {
        for distance in [32.0, 96.0, 192.0, 512.0] {
            let range = abrupt_prop_range(distance);
            assert!(range.is_abrupt());
            assert!(range.is_visible_at_all(distance - 0.01));
            assert!(!range.is_visible_at_all(distance));
            assert_eq!(range.start_margin, 0.0..0.0);
            assert_eq!(range.end_margin, distance..distance);
        }
    }
}
