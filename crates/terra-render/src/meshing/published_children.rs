//! Transfer fully matching, settled draw entities to a replacement section.
use super::{builder::MeshBatch, shadows};
use crate::{foliage, water, weather::surfaces};
use bevy::{
    camera::{
        primitives::Aabb,
        visibility::{NoAutoAabb, RenderLayers, VisibilityClass},
    },
    ecs::query::QueryData,
    light::NotShadowCaster,
    prelude::*,
};
use terra_core::miniature::MiniatureMesh;

pub(super) enum Surface<'a> {
    Standard(&'a Handle<StandardMaterial>),
    Water(&'a Handle<water::material::WaterMaterial>),
    Flame(&'a Handle<foliage::LeafMaterial>),
    Leaf(&'a Handle<foliage::LeafMaterial>),
    Fibre(&'a Handle<foliage::LeafMaterial>),
    Wet(&'a Handle<surfaces::WetMaterial>),
    Depth(&'a Handle<surfaces::TerrainDepthMaterial>),
}

impl Surface<'_> {
    pub(super) fn insert(&self, entity: &mut EntityCommands) {
        match *self {
            Self::Standard(h) => {
                entity.insert(MeshMaterial3d(h.clone()));
            }
            Self::Water(h) => {
                entity.insert(MeshMaterial3d(h.clone()));
            }
            Self::Flame(h) | Self::Fibre(h) | Self::Leaf(h) => {
                entity.insert(MeshMaterial3d(h.clone()));
            }
            Self::Wet(h) => {
                entity.insert(MeshMaterial3d(h.clone()));
            }
            Self::Depth(h) => {
                entity.insert(MeshMaterial3d(h.clone()));
            }
        }
    }
}

#[derive(QueryData)]
pub(super) struct DrawChild {
    pub mesh: &'static Mesh3d,
    parent: &'static ChildOf,
    transform: &'static Transform,
    visibility: Option<&'static Visibility>,
    layers: &'static RenderLayers,
    classes: Option<&'static VisibilityClass>,
    terrain: Option<&'static MiniatureMesh>,
    shadow: Has<shadows::ShadowProxy>,
    blade: Has<shadows::LeafShadowProxy>,
    profile: Option<&'static shadows::LeafProfileCaster>,
    no_shadow: Has<NotShadowCaster>,
    replaced: Has<shadows::ReplacedShadowCaster>,
    moving_leaf: Has<foliage::WindFoliage>,
    moving_fibre: Has<foliage::WindFibre>,
    bounds: Option<&'static Aabb>,
    fixed_bounds: Has<NoAutoAabb>,
    standard: Option<&'static MeshMaterial3d<StandardMaterial>>,
    leaf: Option<&'static MeshMaterial3d<foliage::LeafMaterial>>,
    wet: Option<&'static MeshMaterial3d<surfaces::WetMaterial>>,
    depth: Option<&'static MeshMaterial3d<surfaces::TerrainDepthMaterial>>,
}

pub(super) struct DrawRole {
    pub replaced: bool,
    pub no_shadow: bool,
    pub moving_leaf: bool,
    pub moving_fibre: bool,
}

impl DrawRole {
    pub(super) fn matches(
        &self,
        entity: DrawChildItem<'_, '_>,
        old_root: Entity,
        batch: &MeshBatch,
        surface: &Surface,
        bounds: Option<Aabb>,
    ) -> bool {
        let shadow = batch.shadow_only;
        let blade = shadow && batch.paired_leaf_shadow;
        let layers = if shadow && !blade {
            RenderLayers::layer(shadows::LAYER)
        } else {
            crate::miniature::layers()
        };
        entity.parent.parent() == old_root
            && entity.transform == &Transform::IDENTITY
            && entity
                .visibility
                .is_none_or(|v| *v == Visibility::Inherited)
            && *entity.layers == layers
            && (!blade || entity.classes.is_some_and(|c| c.0.is_empty()))
            && matches!(entity.terrain, Some(MiniatureMesh::Terrain)) == !shadow
            && entity.shadow == shadow
            && entity.blade == blade
            && entity.profile.map(|caster| caster.0) == batch.profile_caster()
            && entity.replaced == self.replaced
            && entity.no_shadow == self.no_shadow
            && entity.moving_leaf == self.moving_leaf
            && entity.moving_fibre == self.moving_fibre
            && entity.fixed_bounds == bounds.is_some()
            && entity.bounds.map(|b| (b.center, b.half_extents))
                == bounds.map(|b| (b.center, b.half_extents))
            && match surface {
                Surface::Standard(h) => entity.standard.is_some_and(|m| m.0.id() == h.id()),
                Surface::Leaf(h) | Surface::Fibre(h) => {
                    entity.leaf.is_some_and(|m| m.0.id() == h.id())
                }
                Surface::Wet(h) => entity.wet.is_some_and(|m| m.0.id() == h.id()),
                Surface::Depth(h) => entity.depth.is_some_and(|m| m.0.id() == h.id()),
                // Time-dependent liquid/flame geometry never participates in resident reuse.
                Surface::Flame(_) | Surface::Water(_) => false,
            }
    }
}
