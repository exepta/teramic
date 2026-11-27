use super::{BuildingStatus, BuildingWorld};
use crate::meshing::builder::RenderOptions;
use bevy::{ecs::system::SystemParam, light::NotShadowCaster, prelude::*};
use std::collections::{HashMap, HashSet, VecDeque};
use terra_core::ItemKind;

#[derive(Component)]
pub(super) struct ResourceDrop(u64, ItemKind, Option<u16>);

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
        let target = drop_transform(drop);
        let age_changed = self.age != drop.age_seconds;
        if age_changed || self.target != target {
            // Carried items pause lifetime, but their authoritative pose still advances.
            let interval = if age_changed {
                drop.age_seconds - self.age
            } else if self.elapsed > 0.0 {
                self.elapsed
            } else {
                self.duration
            };
            self.duration = interval.clamp(0.016, 0.2);
            self.age = drop.age_seconds;
            self.previous = current;
            self.target = target;
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
        self.elapsed += dt;
        let t = (self.elapsed / self.duration).min(1.0);
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
    dynamic_order: VecDeque<ItemKind>,
    mineral_shafts: HashMap<(u16, u16), Handle<Mesh>>,
}

const MAX_DYNAMIC_MODELS: usize = 128;

/// Wear changes inventory identity without allocating another identical GPU mesh.
/// Filled watering cans keep their real, stateful liquid geometry.
fn model_item(item: ItemKind) -> ItemKind {
    match item {
        ItemKind::WornTool { tool, .. } => ItemKind::Tool(tool),
        _ => item,
    }
}

impl DropModels {
    fn admit_dynamic(&mut self, item: ItemKind) {
        let dynamic = terra_core::tints::color(item).is_some();
        if !dynamic {
            return;
        }
        while self.dynamic_order.len() >= MAX_DYNAMIC_MODELS {
            if let Some(expired) = self.dynamic_order.pop_front() {
                // Live world/held entities retain their own strong mesh handles.
                self.meshes.remove(&expired);
                self.bounds.remove(&expired);
            }
        }
        self.dynamic_order.push_back(item);
    }
}

#[derive(SystemParam)]
pub(super) struct DropAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    models: ResMut<'w, DropModels>,
    water: Option<Res<'w, crate::particle_fluids::HydraulicMaterials>>,
    foliage: Option<Res<'w, crate::foliage::FoliageAssets>>,
    leaf_materials: Option<Res<'w, Assets<crate::foliage::LeafMaterial>>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

impl DropAssets<'_> {
    pub(super) fn prepare(&mut self, item: ItemKind, blocks: &crate::blocks::BlockAssets) -> bool {
        let item = model_item(item);
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
            tints: Default::default(),
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
            ItemKind::Armor(_)
            | ItemKind::Accessory(_)
            | ItemKind::Cape(_)
            | ItemKind::HeraldicCape { .. } => {
                crate::armor::item_batches(item, templates.item_materials.opaque)
            }
            ItemKind::Dust(kind) => crate::previews::dust_batches(kind, templates),
            ItemKind::Crop(kind) => crate::previews::crop_batches(kind, templates),
            ItemKind::BloodBerry
            | ItemKind::CrimsonMushroom
            | ItemKind::ThornTwig
            | ItemKind::BloodCrystal
            | ItemKind::DarkSap
            | ItemKind::BloodLeaf
            | ItemKind::BloodSpore
            | ItemKind::BloodSlimeGel
            | ItemKind::Bone
            | ItemKind::Bitumen
            | ItemKind::GloomEssence
            | ItemKind::DragonLoot(_)
            | ItemKind::RuneCore
            | ItemKind::ForestCrystal
            | ItemKind::LightSplitter
            | ItemKind::RoadAggregate => {
                crate::previews::forest_items::batches(item, templates.item_materials.opaque)
            }
            ItemKind::Apple | ItemKind::AppleCore => {
                crate::previews::apple_batches(item == ItemKind::AppleCore, templates)
            }
            ItemKind::Flower(color) => crate::previews::flower_batches(color, templates),
            ItemKind::Dye(color) => crate::previews::dye_batches(color, templates),
            ItemKind::GlassBottle | ItemKind::WaterBottle | ItemKind::Potion(_) => {
                crate::previews::alchemy::batches(item, templates)
            }
            ItemKind::DyedBed { material, color } => {
                crate::previews::dyed_bed_batches(material, color, templates, &options)
            }
            ItemKind::WoodenBlock { .. } => {
                crate::previews::wooden_block_batches(item, templates, &options)
            }
            ItemKind::TintedBlock { .. } => {
                crate::previews::tints::batches(item, templates, &options)
            }
            ItemKind::Tool(_)
            | ItemKind::WornTool { .. }
            | ItemKind::FilledWateringCan { .. }
            | ItemKind::Stick => crate::previews::tool_batches(item, templates),
        };
        let mut batches = match batches {
            Ok(batches) => batches,
            Err(error) => {
                tracing::warn!(%error, ?item, "Cannot compose item model");
                return false;
            }
        };
        if matches!(item, ItemKind::Prop(_) | ItemKind::Flower(_)) {
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
                *vertex = crate::previews::tools::cache_vertex(item, Vec3::from_array(*vertex))
                    .to_array();
                minimum = minimum.min(Vec3::from_array(*vertex));
                maximum = maximum.max(Vec3::from_array(*vertex));
            }
            parts.push((
                batch.material,
                self.meshes.add(crate::previews::batch_mesh(&batch)),
            ));
        }
        self.models.admit_dynamic(item);
        if minimum.is_finite() && maximum.is_finite() {
            self.models.bounds.insert(item, (minimum, maximum));
        }
        self.models.meshes.insert(item, parts);
        true
    }

    fn mineral_shaft(
        &mut self,
        drop: &terra_core::WorldDrop,
        blocks: &crate::blocks::BlockAssets,
    ) -> Option<(Handle<Mesh>, Handle<StandardMaterial>)> {
        let length = drop.detached_mineral_length()?;
        let ItemKind::Block(material) = drop.stack.item else {
            return None;
        };
        let mineral = if material == 460 { 51 } else { material };
        let slot = blocks
            .templates
            .as_ref()?
            .blocks
            .get(&mineral)?
            .face_materials[0];
        let surface = blocks.materials.get(slot)?.clone();
        let mesh = self
            .models
            .mineral_shafts
            .entry((mineral, length))
            .or_insert_with(|| {
                let mut batches = std::collections::BTreeMap::new();
                for y in 0..length {
                    crate::meshing::segments::append(
                        &mut batches,
                        slot,
                        mineral,
                        [0, i64::from(y), 0],
                        [0, u32::from(y), 0],
                        terra_world::segments::Profile {
                            below: y as u8,
                            above: (length - y - 1) as u8,
                            ..Default::default()
                        },
                        crate::meshing::DetailLevel::Near,
                    )
                    .expect("bounded detached mineral mesh");
                }
                let batch = batches.get_mut(&slot).expect("mineral shaft surface");
                for p in &mut batch.positions {
                    p[0] -= 0.5;
                    p[1] -= f32::from(length) * 0.5;
                    p[2] -= 0.5;
                }
                self.meshes.add(crate::previews::batch_mesh(batch))
            });
        Some((mesh.clone(), surface))
    }

    pub(super) fn bounds(&self, item: ItemKind) -> Option<(Vec3, Vec3)> {
        self.models.bounds.get(&model_item(item)).copied()
    }

    pub(super) fn spawn(
        &self,
        commands: &mut Commands,
        root: Entity,
        item: ItemKind,
        blocks: &crate::blocks::BlockAssets,
        world_object: bool,
    ) {
        self.spawn_on_layers(
            commands,
            root,
            item,
            blocks,
            world_object.then(|| {
                (
                    crate::miniature::layers(),
                    terra_core::miniature::MiniatureMesh::Drop,
                )
            }),
        );
    }

    pub(super) fn spawn_creature(
        &self,
        commands: &mut Commands,
        root: Entity,
        item: ItemKind,
        blocks: &crate::blocks::BlockAssets,
    ) {
        self.spawn_on_layers(
            commands,
            root,
            item,
            blocks,
            Some((
                crate::miniature::creature_layers(),
                terra_core::miniature::MiniatureMesh::Creature,
            )),
        );
    }

    fn spawn_on_layers(
        &self,
        commands: &mut Commands,
        root: Entity,
        item: ItemKind,
        blocks: &crate::blocks::BlockAssets,
        layers: Option<(
            bevy::camera::visibility::RenderLayers,
            terra_core::miniature::MiniatureMesh,
        )>,
    ) {
        let Some(parts) = self.models.meshes.get(&model_item(item)) else {
            return;
        };
        for (slot, mesh) in parts {
            let mut child = commands.spawn((
                Mesh3d(mesh.clone()),
                Transform::default(),
                ChildOf(root),
                NotShadowCaster,
            ));
            if let Some((layers, kind)) = &layers {
                child.insert((layers.clone(), *kind));
            }
            if let Some(water) = self.water.as_ref().and_then(|water| {
                blocks
                    .templates
                    .as_ref()
                    .and_then(|t| water.for_slot(t, *slot))
            }) {
                child.insert(MeshMaterial3d(water));
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
        .with_scale(Vec3::splat(if drop.detached_mineral_length().is_some() {
            1.
        } else if !drop.grounded && matches!(drop.stack.item, ItemKind::Block(51..=53)) {
            1.0 / 0.24
        } else if let Some(length) = crate::previews::tools::world_length(drop.stack.item) {
            // Pickups keep the existing 24 cm collision proxy. Equipped
            // tools reuse this mesh at its true 1.25 / 1.5 metre length.
            0.24 / length
        } else {
            1.0
        }))
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
            if data.stack.item != drop.1 || data.detached_mineral_length() != drop.2 {
                // An authoritative infuser keeps custody and the drop ID while
                // converting its contents. Refresh the model in this same pass.
                commands.entity(entity).despawn();
                continue;
            }
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
        let shaft = assets.mineral_shaft(data, &world.blocks);
        if shaft.is_none() && !assets.prepare(item, &world.blocks) {
            continue;
        }
        let root = commands
            .spawn((
                ResourceDrop(id, item, data.detached_mineral_length()),
                DropPresentation::new(data),
                drop_transform(data),
                Visibility::default(),
                Name::new("Dropped resource"),
            ))
            .id();
        if let Some((mesh, surface)) = shaft {
            commands.entity(root).insert((
                Mesh3d(mesh),
                MeshMaterial3d(surface),
                crate::miniature::layers(),
                terra_core::miniature::MiniatureMesh::Drop,
            ));
        } else {
            assets.spawn(&mut commands, root, item, &world.blocks, true);
        }
    }
    status.drops = visible.len();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_fill_replaces_the_model_of_an_existing_drop_id() {
        let mut app = super::super::tests::app();
        let material = app
            .world()
            .resource::<super::super::BuildingAssets>()
            .fallback
            .clone();
        {
            let mut blocks = app.world_mut().resource_mut::<crate::blocks::BlockAssets>();
            blocks.templates = Some(std::sync::Arc::new(crate::blocks::TemplateSet::default()));
            blocks.materials.push(material);
        }
        let empty = ItemKind::Tool(terra_core::ToolKind::IronWateringCan);
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .drops
            .push(terra_core::WorldDrop {
                id: 7,
                position: [1.5, 2.755, 3.5],
                stack: terra_core::ItemStack {
                    item: empty,
                    count: 1,
                },
                rotation: [0., 0., 0., 1.],
                angular_velocity: [0.; 3],
                velocity: [0.; 3],
                grounded: true,
                pickup_delay_seconds: 0.,
                age_seconds: 0.,
            });
        app.update();
        let old = app
            .world_mut()
            .query_filtered::<Entity, With<ResourceDrop>>()
            .single(app.world())
            .unwrap();
        let old_mesh = app.world().resource::<DropModels>().meshes[&empty][0]
            .1
            .id();
        let filled = empty.with_watering_units(12).unwrap();
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .drops[0]
            .stack
            .item = filled;
        app.update();
        assert!(app.world().get_entity(old).is_err());
        let (entity, drop) = app
            .world_mut()
            .query::<(Entity, &ResourceDrop)>()
            .single(app.world())
            .unwrap();
        assert_ne!(entity, old);
        assert_eq!((drop.0, drop.1), (7, filled));
        let cache = app.world().resource::<DropModels>();
        assert_ne!(cache.meshes[&filled][0].1.id(), old_mesh);
        assert_eq!(cache.bounds[&empty], cache.bounds[&filled]);
        let current = entity;
        app.update();
        assert!(
            app.world().get_entity(current).is_ok(),
            "identical snapshots keep their mesh"
        );
    }

    #[test]
    fn a_thousand_rgb_dye_and_cape_variants_keep_only_a_bounded_cache_without_deleting_live_meshes()
    {
        use bevy::{asset::AssetApp, mesh::PrimitiveTopology};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_resource::<DropModels>();
        let triangle = || {
            Mesh::new(
                PrimitiveTopology::TriangleList,
                bevy::asset::RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_POSITION,
                vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            )
        };
        let static_mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(triangle());
        app.world_mut()
            .resource_mut::<DropModels>()
            .meshes
            .insert(ItemKind::Stick, vec![(0, static_mesh.clone())]);
        let mut alive = None;
        let mut expired = None;
        for i in 0u16..1000 {
            let color = terra_core::beds::DyeColor::Rgb([i as u8, (i >> 8) as u8, 71]);
            let item = match i % 4 {
                0 => ItemKind::Dye(color),
                1 => ItemKind::Cape(terra_core::CapeItem {
                    color,
                    ..terra_core::CapeItem::ALL_WHITE[0]
                }),
                2 => ItemKind::TintedBlock {
                    material: 148,
                    wood: terra_core::WoodSpecies::Cherry,
                    color: [i as u8, (i >> 8) as u8, 71],
                },
                _ => ItemKind::DyedBed {
                    material: 71,
                    color,
                },
            };
            let mesh = app
                .world_mut()
                .resource_mut::<Assets<Mesh>>()
                .add(triangle());
            if i == 0 {
                alive = Some((
                    app.world_mut().spawn(Mesh3d(mesh.clone())).id(),
                    mesh.id(),
                    item,
                ));
            }
            if i == 1 {
                expired = Some(mesh.id());
            }
            let mut cache = app.world_mut().resource_mut::<DropModels>();
            cache.admit_dynamic(item);
            cache.meshes.insert(item, vec![(0, mesh)]);
            cache.bounds.insert(item, (Vec3::ZERO, Vec3::ONE));
            assert!(cache.dynamic_order.len() <= MAX_DYNAMIC_MODELS);
            assert!(cache.meshes.len() <= MAX_DYNAMIC_MODELS + 1);
            assert!(cache.bounds.len() <= MAX_DYNAMIC_MODELS);
        }
        app.update();
        let (entity, id, item) = alive.unwrap();
        assert!(
            !app.world()
                .resource::<DropModels>()
                .meshes
                .contains_key(&item)
        );
        assert!(
            !app.world()
                .resource::<DropModels>()
                .bounds
                .contains_key(&item)
        );
        assert_eq!(app.world().get::<Mesh3d>(entity).unwrap().0.id(), id);
        assert!(app.world().resource::<Assets<Mesh>>().contains(id));
        assert!(
            app.world()
                .resource::<Assets<Mesh>>()
                .contains(static_mesh.id())
        );
        assert!(
            !app.world()
                .resource::<Assets<Mesh>>()
                .contains(expired.unwrap())
        );
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            MAX_DYNAMIC_MODELS + 2
        );
        app.world_mut().despawn(entity);
        app.update();
        assert!(!app.world().resource::<Assets<Mesh>>().contains(id));
    }

    #[test]
    fn repeated_real_rgb_drop_preparation_reuses_its_cache_entry() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<DropModels>();
        let mut blocks = crate::blocks::BlockAssets::default();
        blocks.templates = Some(std::sync::Arc::new(crate::blocks::TemplateSet::default()));
        let mut state = bevy::ecs::system::SystemState::<DropAssets>::new(&mut world);
        let item = ItemKind::Dye(terra_core::beds::DyeColor::Rgb([71, 93, 184]));
        for _ in 0..3 {
            assert!(state.get_mut(&mut world).unwrap().prepare(item, &blocks));
        }
        let cache = world.resource::<DropModels>();
        assert_eq!(cache.dynamic_order, VecDeque::from([item]));
        assert_eq!(cache.meshes.len(), 1);
        assert_eq!(cache.bounds.len(), 1);
        assert_eq!(world.resource::<Assets<Mesh>>().len(), 1);
    }

    #[test]
    fn repeated_tool_cache_requests_reuse_one_mesh_per_item_and_no_new_materials() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<DropModels>();
        let mut blocks = crate::blocks::BlockAssets::default();
        blocks.templates = Some(std::sync::Arc::new(crate::blocks::TemplateSet::default()));
        let mut state = bevy::ecs::system::SystemState::<DropAssets>::new(&mut world);
        let initial_entities = world.entities().len();
        for _ in 0..128 {
            let mut assets = state.get_mut(&mut world).expect("drop asset resources");
            for tool in terra_core::ToolKind::ALL {
                assert!(assets.prepare(ItemKind::Tool(tool), &blocks));
                if tool.durability() > 0 {
                    for remaining in [1, tool.durability() - 1] {
                        let worn = ItemKind::WornTool { tool, remaining };
                        assert!(assets.prepare(worn, &blocks));
                        assert_eq!(assets.bounds(worn), assets.bounds(ItemKind::Tool(tool)));
                    }
                }
            }
        }
        assert_eq!(
            world.resource::<Assets<Mesh>>().len(),
            terra_core::ToolKind::ALL.len()
        );
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 0);
        let cache = world.resource::<DropModels>();
        assert_eq!(cache.meshes.len(), terra_core::ToolKind::ALL.len());
        assert_eq!(cache.bounds.len(), terra_core::ToolKind::ALL.len());
        assert!(cache.meshes.values().all(|parts| parts.len() == 1));
        assert!(cache.foliage.is_empty());
        assert_eq!(world.entities().len(), initial_entities);
    }

    #[test]
    fn full_size_equipped_mesh_keeps_the_existing_small_physical_pickup_size() {
        for tool in terra_core::ToolKind::ALL {
            let item = ItemKind::Tool(tool);
            let length = crate::previews::tools::world_length(item).unwrap();
            let drop = terra_core::WorldDrop {
                id: 1,
                position: [1.0, 2.12, 3.0],
                stack: terra_core::ItemStack { item, count: 1 },
                velocity: [0.0; 3],
                age_seconds: 10.0,
                rotation: Quat::IDENTITY.to_array(),
                angular_velocity: [0.0; 3],
                grounded: true,
                pickup_delay_seconds: 0.0,
            };
            let transform = drop_transform(&drop);
            assert!((transform.scale.y * length - 0.24).abs() < 1e-6);
            assert_eq!(transform.translation, Vec3::new(1.0, 2.12, 3.0));
        }
    }
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

    #[test]
    fn same_age_carried_snapshots_update_all_pose_components_without_resetting_identical_targets() {
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
        drop.position[0] = 0.1;
        drop.rotation = Quat::from_rotation_y(0.8).to_array();
        drop.stack.item = ItemKind::Tool(terra_core::ToolKind::Wrench);
        let first = presentation.update(&drop, initial, 0.01);
        assert!(first.translation.x > 0.0 && first.translation.x < 0.1);
        assert!(first.rotation.angle_between(Quat::IDENTITY) > 0.0);
        assert_eq!(first.scale, Vec3::splat(0.24));
        let previous = presentation.previous;
        let second = presentation.update(&drop, first, 0.01);
        assert_eq!(
            presentation.previous, previous,
            "an identical snapshot must not restart interpolation"
        );
        assert!((second.translation.x - 0.04).abs() < 1e-6);
        assert!(
            second.rotation.angle_between(Quat::IDENTITY)
                > first.rotation.angle_between(Quat::IDENTITY)
        );
        assert_eq!(second.scale, first.scale);
        assert_eq!(presentation.age, 10.0);

        drop.position[0] = 0.2;
        let next = presentation.update(&drop, second, 0.01);
        assert!((presentation.duration - 0.02).abs() < 1e-6);
        assert!(next.translation.x > second.translation.x && next.translation.x < 0.2);
        let reached = presentation.update(&drop, next, 0.2);
        assert!(
            reached
                .translation
                .abs_diff_eq(Vec3::new(0.2, 2.12, 3.0), 1e-6)
        );
        assert!(
            reached
                .rotation
                .abs_diff_eq(Quat::from_array(drop.rotation), 1e-6)
        );
        assert!(reached.scale.abs_diff_eq(Vec3::splat(0.24), 1e-6));

        drop.velocity = [0.0; 3];
        for dt in [0.0, 0.016, 0.2, 1.0] {
            assert_eq!(
                presentation.update(&drop, reached, dt),
                drop_transform(&drop)
            );
        }
    }

    #[test]
    fn same_age_snapshot_interval_keeps_counting_after_interpolation_finishes() {
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
        let mut current = drop_transform(&drop);
        drop.position[0] = 0.1;
        current = presentation.update(&drop, current, 0.0);
        assert_eq!(presentation.duration, 0.05);
        let previous = presentation.previous;
        for _ in 0..10 {
            current = presentation.update(&drop, current, 0.01);
            assert_eq!(presentation.previous, previous);
        }
        assert!((presentation.elapsed - 0.1).abs() < 1e-6);
        assert!((current.translation.x - 0.1).abs() < 1e-6);

        drop.position[0] = 0.2;
        current = presentation.update(&drop, current, 0.0);
        assert!((presentation.duration - 0.1).abs() < 1e-6);
        for _ in 0..5 {
            current = presentation.update(&drop, current, 0.01);
        }
        assert!((current.translation.x - 0.15).abs() < 1e-6);
        current = presentation.update(&drop, current, 0.01);
        assert!(current.translation.x > 0.15 && current.translation.x < 0.2);
        for _ in 0..4 {
            current = presentation.update(&drop, current, 0.01);
        }
        assert!((current.translation.x - 0.2).abs() < 1e-6);
        drop.velocity = [0.0; 3];
        for _ in 0..10 {
            current = presentation.update(&drop, current, 0.1);
            assert_eq!(current, drop_transform(&drop));
        }
    }

    #[test]
    fn rotation_and_scale_only_snapshots_are_not_lost_when_lifetime_is_paused() {
        let mut drop = terra_core::WorldDrop {
            id: 1,
            position: [1.0, 2.12, 3.0],
            stack: terra_core::ItemStack {
                item: ItemKind::Block(3),
                count: 1,
            },
            velocity: [0.0; 3],
            age_seconds: 10.0,
            rotation: Quat::IDENTITY.to_array(),
            angular_velocity: [0.0; 3],
            grounded: true,
            pickup_delay_seconds: 0.0,
        };
        let mut presentation = DropPresentation::new(&drop);
        let initial = drop_transform(&drop);
        drop.rotation = Quat::from_rotation_z(0.6).to_array();
        let turned = presentation.update(&drop, initial, 0.016);
        assert_eq!(turned, drop_transform(&drop));
        drop.stack.item = ItemKind::Tool(terra_core::ToolKind::Wrench);
        let resized = presentation.update(&drop, turned, 0.016);
        assert_eq!(resized, drop_transform(&drop));
        assert_eq!(resized.scale, Vec3::splat(0.24));
        assert_eq!(resized.translation, initial.translation);
        assert_eq!(presentation.update(&drop, resized, 0.2), resized);
    }
}
