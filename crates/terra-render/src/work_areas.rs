//! Cached, drifting low mist for confirmed work areas and claims.
mod ambient;
mod fog_blocks;
mod geometry;
mod ground;
mod material;
pub use ambient::SpiritMistStatus;
pub use fog_blocks::FogBlockStatus;
#[cfg(test)]
mod tests;

use bevy::{
    camera::visibility::{NoFrustumCulling, VisibilitySystems},
    light::NotShadowCaster,
    prelude::*,
};
use geometry::{Contour, ContourKind, Geometry, append_area, append_claims};
use material::MistMaterial;
use std::collections::BTreeSet;
use terra_core::{
    kingdom::{ClaimKind, KingdomId},
    work_areas::{ConstructionAreaDisplay, WorkAreaDisplay, WorkAreaMark, WorkAreaTarget},
};
use terra_network::client::{
    ClientSession,
    work_areas::{visit_construction_areas, visit_saved_areas},
};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, SectionEditApplied, SectionPublished, SectionRemoved},
};

const REFRESH_SECONDS: f32 = 0.25;
const MAX_AREAS: usize = 256;
const MAX_SEGMENTS: usize = 16_384;

/// Observable real geometry state for diagnostics and native acceptance tests.
#[derive(Resource, Default, Debug)]
pub struct WorkAreaOverlayStatus {
    pub visible: bool,
    pub area_segments: usize,
    pub build_segments: usize,
    pub vertices: usize,
    pub generation: u64,
    pub pulse: f32,
    /// Actual X/Z advection uploaded to the area materials, for native diagnostics.
    pub mist_drift: [f32; 2],
}

#[derive(Component)]
struct ContourBatch(usize);

#[derive(Resource, Default)]
struct Cache {
    next_refresh: f32,
    dirty: bool,
    target: Option<WorkAreaTarget>,
    construction: Option<KingdomId>,
    claims_revision: u64,
    claims: BTreeSet<[i64; 2]>,
    draft: Vec<WorkAreaMark>,
    marks: Vec<WorkAreaMark>,
    contours: Vec<Contour>,
    focus: [i64; 2],
    meshes: [Handle<Mesh>; 5],
    materials: [Handle<MistMaterial>; 5],
    segments: [usize; 5],
    vertices: usize,
}

pub(crate) fn install(app: &mut App) {
    ambient::install(app);
    fog_blocks::install(app);
    crate::material_work::add::<MistMaterial>(app)
        .init_resource::<WorkAreaDisplay>()
        .init_resource::<ConstructionAreaDisplay>()
        .init_resource::<WorkAreaOverlayStatus>()
        .init_resource::<Cache>()
        .add_systems(Startup, setup)
        .add_systems(
            PostUpdate,
            (refresh, animate)
                .chain()
                .before(VisibilitySystems::VisibilityPropagate),
        );
}

fn setup(
    mut commands: Commands,
    mut cache: ResMut<Cache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MistMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let noise = images.add(material::density_texture());
    for kind in ContourKind::ALL {
        let index = kind as usize;
        // The shared mesh allocator requires a valid initial vertex buffer.
        cache.meshes[index] = meshes.add(Geometry::placeholder_mesh());
        cache.materials[index] = materials.add(MistMaterial {
            color: material::contour_color(kind.color()),
            flow: material::flow(0.0),
            noise: noise.clone(),
        });
        commands.spawn((
            Name::new(kind.name()),
            ContourBatch(index),
            Mesh3d(cache.meshes[index].clone()),
            MeshMaterial3d(cache.materials[index].clone()),
            Transform::default(),
            Visibility::Hidden,
            NoFrustumCulling,
            NotShadowCaster,
        ));
    }
    cache.dirty = true;
}

type TerrainEvents<'w, 's> = (
    MessageReader<'w, 's, SectionPublished>,
    MessageReader<'w, 's, SectionRemoved>,
    MessageReader<'w, 's, SectionEditApplied>,
);

#[allow(clippy::too_many_arguments)] // Shared input snapshots and independent terrain message cursors.
fn refresh(
    time: Res<Time<Real>>,
    session: Option<Res<ClientSession>>,
    display: Res<WorkAreaDisplay>,
    construction: Res<ConstructionAreaDisplay>,
    camera: Option<Res<terra_player::ConstructionCamera>>,
    config: Option<Res<terra_core::GameConfig>>,
    chunks: Option<Res<ChunkStore>>,
    journal: Option<Res<EditJournal>>,
    (mut published, mut removed, mut edited): TerrainEvents,
    mut cache: ResMut<Cache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut status: ResMut<WorkAreaOverlayStatus>,
) {
    cache.dirty |= published.read().next().is_some();
    cache.dirty |= removed.read().next().is_some();
    cache.dirty |= edited.read().next().is_some();
    published.clear();
    removed.clear();
    edited.clear();
    let now = time.elapsed_secs();
    let context_changed =
        cache.target != display.target || cache.construction != construction.kingdom;
    if !context_changed && now < cache.next_refresh {
        return;
    }
    cache.next_refresh = now + REFRESH_SECONDS;
    cache.target = display.target;
    cache.construction = construction.kingdom;
    let (Some(session), Some(chunks), Some(journal)) = (session, chunks, journal) else {
        return;
    };
    if !session.ready || session.error.is_some() {
        return;
    }
    let focus = camera.as_ref().filter(|camera| camera.active).map_or_else(
        || {
            session
                .confirmed_state()
                .map_or_else(|| session.spawn.to_array(), |body| body.position)
        },
        |camera| camera.center.to_array(),
    );
    let bucket = [
        (f64::from(focus[0]).floor() as i64).div_euclid(16),
        (f64::from(focus[2]).floor() as i64).div_euclid(16),
    ];
    let moved = cache.focus != bucket;
    cache.focus = bucket;
    let radius = config.map_or(256.0, |config| {
        f32::from(config.client.view_distance) * 16.0 + 24.0
    });
    cache.draft.clear();
    let mut emit = |mark: WorkAreaMark| {
        if cache.draft.len() < MAX_AREAS && geometry::near_area(mark.bounds, focus, radius) {
            cache.draft.push(mark);
        }
    };
    if let Some(kingdom) = construction.kingdom {
        visit_construction_areas(&session, kingdom, &mut emit);
    } else {
        visit_saved_areas(&session, display.target, &mut emit);
    }
    cache.draft.sort_unstable_by_key(|mark| {
        (
            ContourKind::from(mark.kind) as usize,
            mark.bounds.min,
            mark.bounds.max,
        )
    });
    cache.draft.dedup();
    let changed = cache.draft != cache.marks;
    if changed {
        let cache = &mut *cache;
        std::mem::swap(&mut cache.marks, &mut cache.draft);
    }
    let claims_changed = context_changed || cache.claims_revision != session.kingdoms.revision;
    if claims_changed {
        cache.claims_revision = session.kingdoms.revision;
        cache.claims.clear();
        if let Some(id) = construction.kingdom {
            let realm = session.kingdoms.realm_id(id);
            if session.uuid.is_some_and(|actor| {
                session
                    .kingdoms
                    .active_for_member(actor)
                    .is_some_and(|own| session.kingdoms.realm_id(own.id) == realm)
            }) {
                for claim in &session.kingdoms.claims {
                    if claim.kind == ClaimKind::ProtectedBuilding
                        && session.kingdoms.realm_id(claim.kingdom) == realm
                    {
                        cache.claims.insert(claim.chunk);
                    }
                }
            }
        }
    }
    if !(context_changed || changed || claims_changed || moved || cache.dirty) {
        return;
    }
    cache.contours.clear();
    for index in 0..cache.marks.len() {
        let mark = cache.marks[index];
        append_area(&mut cache.contours, mark);
    }
    {
        let cache = &mut *cache;
        append_claims(&mut cache.contours, &cache.claims, focus, radius);
    }
    let mut remaining = MAX_SEGMENTS;
    cache.segments = [0; 5];
    cache.vertices = 0;
    for kind in [
        ContourKind::BuildBorder,
        ContourKind::Work,
        ContourKind::Storage,
        ContourKind::Residential,
        ContourKind::Building,
    ] {
        let geometry =
            Geometry::from_contours(&cache.contours, kind, &chunks, &journal, &mut remaining);
        let index = kind as usize;
        cache.segments[index] = geometry.segments;
        cache.vertices += geometry.positions.len();
        if geometry.positions.is_empty() {
            continue;
        }
        if let Some(mut mesh) = meshes.get_mut(&cache.meshes[index]) {
            *mesh = geometry.mesh();
        }
    }
    cache.dirty = false;
    status.generation = status.generation.wrapping_add(1);
}

type DisplayContext<'w> = (
    Option<Res<'w, ClientSession>>,
    Res<'w, WorkAreaDisplay>,
    Res<'w, ConstructionAreaDisplay>,
    Option<Res<'w, terra_core::HudVisibility>>,
);

fn animate(
    time: Res<Time<Real>>,
    (session, display, construction, hud): DisplayContext,
    cache: Res<Cache>,
    mut materials: ResMut<Assets<MistMaterial>>,
    mut batches: Query<(&ContourBatch, &mut Visibility)>,
    mut status: ResMut<WorkAreaOverlayStatus>,
) {
    let enabled = session.is_some_and(|session| session.ready && session.error.is_none())
        && hud.is_none_or(|hud| hud.visible)
        && if construction.kingdom.is_some() {
            construction.visible
        } else {
            display.target.is_some()
        };
    let elapsed = time.elapsed_secs();
    status.pulse = geometry::pulse(elapsed, 0);
    status.visible = enabled && cache.segments.iter().any(|count| *count > 0);
    status.area_segments = cache.segments[..4].iter().sum();
    status.build_segments = cache.segments[4];
    status.vertices = cache.vertices;
    if enabled {
        let flow = material::flow(time.elapsed_secs_f64());
        for kind in ContourKind::ALL {
            if let Some(mut material) = materials.get_mut(&cache.materials[kind as usize]) {
                material.flow = flow;
                status.mist_drift = [flow.x, flow.y];
            }
        }
    }
    for (batch, mut visibility) in &mut batches {
        let next = if enabled && cache.segments[batch.0] > 0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
}
