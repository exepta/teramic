use super::{BuildingStatus, BuildingWorld};
use crate::meshing::builder::RenderOptions;
use bevy::{ecs::system::SystemParam, light::NotShadowCaster, prelude::*};
use std::collections::{HashMap, HashSet};
use terra_core::ItemKind;

#[derive(Component)]
pub(super) struct ResourceDrop(u64);

#[derive(Component)]
pub(super) struct DropPresentation {
    age: f32,
    elapsed: f32,
    duration: f32,
    previous: Transform,
    target: Transform,
}
impl DropPresentation {
    fn new(drop: &terra_core::WorldDrop) -> Self {
        let target = drop_transform(drop);
        Self {
            age: drop.age_seconds,
            elapsed: 0.0,
            duration: 0.05,
            previous: target,
            target,
        }
    }
    fn update(&mut self, drop: &terra_core::WorldDrop, current: Transform, dt: f32) -> Transform {
        if self.age != drop.age_seconds {
            self.duration = (drop.age_seconds - self.age).clamp(0.016, 0.2);
            self.age = drop.age_seconds;
            self.previous = current;
            self.target = drop_transform(drop);
            self.elapsed = 0.0;
        }
        let velocity = Vec3::from_array(drop.velocity).length();
        let angular_speed = Vec3::from_array(drop.angular_velocity).length_squared();
        // Settled poses and teleports snap exactly. Flying snapshots interpolate,
        // never extrapolate through a collision surface ahead of the authority.
        if (drop.grounded && velocity < 0.05 && angular_speed < 0.01)
            || current.translation.distance(self.target.translation)
                > velocity * self.duration + 0.5
        {
            return self.target;
        }
        self.elapsed = (self.elapsed + dt).min(self.duration);
        let t = self.elapsed / self.duration;
        Transform::from_translation(self.previous.translation.lerp(self.target.translation, t))
            .with_rotation(self.previous.rotation.slerp(self.target.rotation, t))
            .with_scale(self.target.scale)
    }
}

#[derive(Resource, Default)]
pub(super) struct DropModels {
    meshes: HashMap<ItemKind, Vec<(usize, Handle<Mesh>)>>,
    bounds: HashMap<ItemKind, (Vec3, Vec3)>,
    foliage: HashMap<usize, Handle<StandardMaterial>>,
}

#[derive(SystemParam)]
pub(super) struct DropAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    models: ResMut<'w, DropModels>,
    water: Option<Res<'w, crate::water::material::WaterAssets>>,
    foliage: Option<Res<'w, crate::foliage::FoliageAssets>>,
    leaf_materials: Option<Res<'w, Assets<crate::foliage::LeafMaterial>>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

impl DropAssets<'_> {
    pub(super) fn prepare(&mut self, item: ItemKind, blocks: &crate::blocks::BlockAssets) -> bool {
        if self.models.meshes.contains_key(&item) {
            return true;
        }
        let Some(templates) = &blocks.templates else {
            return false;
        };
        let options = RenderOptions {
            water_level: None,
            detail_level: crate::meshing::DetailLevel::Near,
            terrain: None,
            exposure: None,
            conditions: Default::default(),
        };
        let batches = match item {
            ItemKind::Block(material) => {
                crate::previews::block_batches(material, templates, &options)
            }
            ItemKind::Prop(kind) => crate::previews::prop_batches(kind, templates),
            ItemKind::Mineral(kind) => crate::previews::mineral_batches(kind, templates),
            ItemKind::Animal(kind) => crate::previews::animal_batches(kind, templates),
            ItemKind::Bellows(kind) => crate::previews::bellows_batches(kind, templates),
            ItemKind::Dye(color) => crate::previews::dye_batches(color, templates),
            ItemKind::DyedBed { material, color } => {
                crate::previews::dyed_bed_batches(material, color, templates, &options)
            }
            ItemKind::WoodenBlock { .. } => {
                crate::previews::wooden_block_batches(item, templates, &options)
            }
            ItemKind::Tool(_) | ItemKind::Stick => crate::previews::tool_batches(item, templates),
        };
        let mut batches = match batches {
            Ok(batches) => batches,
            Err(error) => {
                tracing::warn!(%error, ?item, "Cannot compose item model");
                return false;
            }
        };
        if matches!(item, ItemKind::Prop(_)) {
            let bottom = batches
                .iter()
                .flat_map(|b| &b.positions)
                .map(|p| p[1])
                .fold(f32::INFINITY, f32::min);
            for p in batches.iter_mut().flat_map(|b| &mut b.positions) {
                p[1] -= bottom;
            }
        }
        let mut parts = Vec::with_capacity(batches.len());
        let mut minimum = Vec3::splat(f32::INFINITY);
        let mut maximum = Vec3::splat(f32::NEG_INFINITY);
        for mut batch in batches {
            if !self.models.foliage.contains_key(&batch.material) {
                let base = self
                    .foliage
                    .as_ref()
                    .and_then(|a| a.materials.get(&batch.material))
                    .and_then(|h| self.leaf_materials.as_ref().and_then(|a| a.get(h)))
                    .map(|m| m.base.clone());
                if let Some(base) = base {
                    self.models
                        .foliage
                        .insert(batch.material, self.materials.add(base));
                }
            }
            for vertex in &mut batch.positions {
                *vertex = ((Vec3::from_array(*vertex) - Vec3::splat(0.5)) * 0.24).to_array();
                minimum = minimum.min(Vec3::from_array(*vertex));
                maximum = maximum.max(Vec3::from_array(*vertex));
            }
            parts.push((
                batch.material,
                self.meshes.add(crate::previews::batch_mesh(&batch)),
            ));
        }
        if minimum.is_finite() && maximum.is_finite() {
            self.models.bounds.insert(item, (minimum, maximum));
        }
        self.models.meshes.insert(item, parts);
        true
    }

    pub(super) fn bounds(&self, item: ItemKind) -> Option<(Vec3, Vec3)> {
        self.models.bounds.get(&item).copied()
    }

    pub(super) fn spawn(
        &self,
        commands: &mut Commands,
        root: Entity,
        item: ItemKind,
        blocks: &crate::blocks::BlockAssets,
        world_object: bool,
    ) {
        let Some(parts) = self.models.meshes.get(&item) else {
            return;
        };
        for (slot, mesh) in parts {
            let mut child = commands.spawn((
                Mesh3d(mesh.clone()),
                Transform::default(),
                ChildOf(root),
                NotShadowCaster,
            ));
            if world_object {
                child.insert((
                    crate::miniature::layers(),
                    terra_core::miniature::MiniatureMesh::Drop,
                ));
            }
            if let Some(water) = self.water.as_ref().filter(|_| {
                blocks
                    .templates
                    .as_ref()
                    .is_some_and(|t| t.water_material(*slot))
            }) {
                child.insert(MeshMaterial3d(water.0.clone()));
            } else if let Some(material) = self
                .models
                .foliage
                .get(slot)
                .or_else(|| blocks.materials.get(*slot))
            {
                child.insert(MeshMaterial3d(material.clone()));
            }
        }
    }
}

fn drop_transform(drop: &terra_core::WorldDrop) -> Transform {
    let rotation = Quat::from_array(drop.rotation);
    Transform::from_translation(Vec3::from_array(drop.position.map(|v| v as f32)))
        .with_scale(Vec3::splat(
            if !drop.grounded && matches!(drop.stack.item, ItemKind::Block(51..=53)) {
                1.0 / 0.24
            } else {
                1.0
            },
        ))
        .with_rotation(
            if rotation.is_finite() && rotation.length_squared() > 0.001 {
                rotation.normalize()
            } else {
                Quat::IDENTITY
            },
        )
}

pub(super) fn animate(
    mut commands: Commands,
    world: BuildingWorld,
    mut assets: DropAssets,
    mut drops: Query<(Entity, &ResourceDrop, &mut Transform, &mut DropPresentation)>,
    mut status: ResMut<BuildingStatus>,
) {
    let visible: HashMap<_, _> = world
        .session
        .as_ref()
        .filter(|s| s.ready && s.error.is_none())
        .into_iter()
        .flat_map(|s| s.drops.iter().take(256))
        .map(|d| (d.id, d))
        .collect();
    let mut present = HashSet::new();
    for (entity, drop, mut transform, mut presentation) in &mut drops {
        if let Some(data) = visible.get(&drop.0) {
            // No decorative bob or perpetual yaw: the server owns flight and rest.
            *transform =
                presentation.update(data, *transform, world.time.delta_secs().clamp(0.0, 0.1));
            present.insert(drop.0);
        } else {
            commands.entity(entity).despawn();
        }
    }
    for (&id, data) in &visible {
        if present.contains(&id) {
            continue;
        }
        let item = data.stack.item;
        if !assets.prepare(item, &world.blocks) {
            continue;
        }
        let root = commands
            .spawn((
                ResourceDrop(id),
                DropPresentation::new(data),
                drop_transform(data),
                Visibility::default(),
                Name::new("Dropped resource"),
            ))
            .id();
        assets.spawn(&mut commands, root, item, &world.blocks, true);
    }
    status.drops = visible.len();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grounded_drops_drifting_in_current_interpolate_without_hover_or_extrapolation() {
        let mut drop = terra_core::WorldDrop {
            id: 1,
            position: [0.0, 2.12, 3.0],
            stack: terra_core::ItemStack {
                item: ItemKind::Block(3),
                count: 1,
            },
            velocity: [1.0, 0.0, 0.0],
            age_seconds: 10.0,
            rotation: Quat::IDENTITY.to_array(),
            angular_velocity: [0.0; 3],
            grounded: true,
            pickup_delay_seconds: 0.0,
        };
        let mut presentation = DropPresentation::new(&drop);
        let initial = drop_transform(&drop);
        drop.age_seconds += 0.1;
        drop.position[0] = 0.1;
        let first = presentation.update(&drop, initial, 0.02);
        assert!(first.translation.x > 0.0 && first.translation.x < 0.1);
        assert_eq!(first.translation.y, initial.translation.y);
        drop.age_seconds += 0.1;
        drop.position[0] = 0.2;
        let second = presentation.update(&drop, first, 0.02);
        assert!(second.translation.x > first.translation.x && second.translation.x < 0.2);
        assert_eq!(second.translation.y, initial.translation.y);
        let reached = presentation.update(&drop, second, 1.0);
        assert_eq!(reached.translation, drop_transform(&drop).translation);
        assert_eq!(presentation.update(&drop, reached, 1.0), reached);
    }

    #[test]
    fn resting_drop_pose_has_no_cosmetic_hover_or_rotation() {
        let drop = terra_core::WorldDrop {
            id: 1,
            position: [1.0, 2.12, 3.0],
            stack: terra_core::ItemStack {
                item: ItemKind::Block(3),
                count: 1,
            },
            velocity: [0.0; 3],
            age_seconds: 10.0,
            rotation: Quat::from_rotation_y(0.7).to_array(),
            angular_velocity: [0.0; 3],
            grounded: true,
            pickup_delay_seconds: 0.0,
        };
        let transform = drop_transform(&drop);
        assert_eq!(transform.translation, Vec3::new(1.0, 2.12, 3.0));
        assert!(
            transform
                .rotation
                .abs_diff_eq(Quat::from_rotation_y(0.7), 0.0001)
        );
        let mut later = drop.clone();
        later.age_seconds += 20.0;
        assert_eq!(drop_transform(&later), transform);
        let mut presentation = DropPresentation::new(&drop);
        assert_eq!(
            presentation.update(&later, Transform::from_xyz(1.0, 3.0, 3.0), 0.016),
            transform
        );
    }
}
