//! Shared meshes for framed doors/windows; only their hinged leaf animates.
use crate::blocks::BlockAssets;
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::{HashMap, HashSet};
use terra_world::{edits::EditJournal, streaming::ChunkStore};

const HINGE: Vec3 = Vec3::new(0.15, 0.0, 0.5);
const CENTER: Vec3 = Vec3::new(0.5, 0.0, 0.5);
const OPEN_ANGLE: f32 = -std::f32::consts::FRAC_PI_2;
const DURATION: f32 = 0.45;

#[derive(Clone)]
struct Part {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    moving: bool,
    glass: bool,
}

#[derive(Resource, Default)]
struct Openings {
    templates: HashMap<u16, Vec<Part>>,
    placed: HashMap<[i64; 3], (Entity, Entity, u16, u8)>,
    next_scan: f32,
}

#[derive(Component)]
struct Leaf {
    fraction: f32,
    start: f32,
    target: f32,
    elapsed: f32,
}
impl Leaf {
    fn new(open: bool) -> Self {
        let fraction = f32::from(u8::from(open));
        Self {
            fraction,
            start: fraction,
            target: fraction,
            elapsed: DURATION,
        }
    }

    fn target(&mut self, open: bool) {
        let target = f32::from(u8::from(open));
        if self.target != target {
            self.start = self.fraction;
            self.target = target;
            self.elapsed = 0.0;
        }
    }

    fn advance(&mut self, dt: f32) {
        self.elapsed = (self.elapsed + dt).min(DURATION);
        let t = self.elapsed / DURATION;
        let smooth = t * t * (3.0 - 2.0 * t);
        self.fraction = self.start + (self.target - self.start) * smooth;
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Openings>()
        .add_systems(Update, (synchronize, animate).chain());
}

fn has_loaded_part(
    chunks: &ChunkStore,
    position: [i64; 3],
    material: u16,
    object: terra_world::edits::BlockObjectState,
) -> bool {
    chunks.block_at(position) == Some(material)
        || object
            .peer
            .is_some_and(|peer| chunks.block_at(peer) == Some(material))
}

fn parts(
    id: u16,
    assets: &BlockAssets,
    meshes: &mut Assets<Mesh>,
    materials: &Assets<StandardMaterial>,
) -> Option<Vec<Part>> {
    let templates = assets.templates.as_ref()?;
    let block = templates.blocks.get(&id)?;
    let mut result = Vec::new();
    for primitive in &block.model {
        let moving = templates.hinge_materials.contains(&primitive.material);
        let origin = if moving { HINGE } else { CENTER };
        let material = assets.materials.get(primitive.material)?.clone();
        let glass = materials
            .get(&material)
            .is_some_and(|material| material.alpha_mode == AlphaMode::Blend);
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            primitive
                .positions
                .iter()
                .map(|p| (Vec3::from_array(*p) - origin).to_array())
                .collect::<Vec<_>>(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, primitive.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, primitive.uvs.clone())
        .with_inserted_indices(Indices::U32(primitive.indices.clone()));
        result.push(Part {
            mesh: meshes.add(mesh),
            material,
            moving,
            glass,
        });
    }
    Some(result)
}

#[allow(clippy::too_many_arguments)]
fn synchronize(
    mut commands: Commands,
    time: Res<Time>,
    journal: Res<EditJournal>,
    chunks: Res<ChunkStore>,
    assets: Res<BlockAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    mut state: ResMut<Openings>,
    mut leaves: Query<&mut Leaf>,
) {
    let now = time.elapsed_secs();
    if now < state.next_scan || assets.templates.is_none() {
        return;
    }
    state.next_scan = now + 0.1;
    let mut live = HashSet::new();
    for (position, material, object) in journal.object_states() {
        if !(65..=70).contains(&material) || object.secondary {
            continue;
        }
        // A two-cell door can straddle a vertical section boundary. Its shared
        // model remains visible while either occupied half is still loaded.
        if !has_loaded_part(&chunks, position, material, object) {
            continue;
        }
        live.insert(position);
        if let Some(&(root, leaf, old_material, old_facing)) = state.placed.get(&position) {
            if old_material == material && old_facing % 4 == object.facing % 4 {
                if let Ok(mut leaf) = leaves.get_mut(leaf) {
                    leaf.target(object.facing >= 4);
                }
                continue;
            }
            commands.entity(root).despawn();
            state.placed.remove(&position);
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = state.templates.entry(material) {
            let Some(parts) = parts(material, &assets, &mut meshes, &materials) else {
                continue;
            };
            entry.insert(parts);
        }
        let Some(parts) = state.templates.get(&material) else {
            continue;
        };
        let open = object.facing >= 4;
        let root = commands
            .spawn((
                Name::new("Framed opening"),
                Transform::from_translation(Vec3::from_array(position.map(|v| v as f32)) + CENTER)
                    .with_rotation(Quat::from_rotation_y(
                        f32::from(object.facing % 4) * std::f32::consts::FRAC_PI_2,
                    )),
                Visibility::Inherited,
            ))
            .id();
        let leaf = commands
            .spawn((
                Name::new("Hinged leaf"),
                Leaf::new(open),
                Transform::from_translation(HINGE - CENTER)
                    .with_rotation(Quat::from_rotation_y(if open { OPEN_ANGLE } else { 0.0 })),
                Visibility::Inherited,
                ChildOf(root),
            ))
            .id();
        for part in parts {
            let mut entity = commands.spawn((
                Mesh3d(part.mesh.clone()),
                MeshMaterial3d(part.material.clone()),
                Transform::default(),
                Visibility::Inherited,
                ChildOf(if part.moving { leaf } else { root }),
            ));
            if part.glass {
                entity.insert(NotShadowCaster);
            }
        }
        state
            .placed
            .insert(position, (root, leaf, material, object.facing));
    }
    state.placed.retain(|position, entry| {
        if live.contains(position) {
            true
        } else {
            commands.entity(entry.0).despawn();
            false
        }
    });
}

fn animate(time: Res<Time>, mut leaves: Query<(&mut Leaf, &mut Transform)>) {
    for (mut leaf, mut transform) in &mut leaves {
        if leaf.elapsed >= DURATION {
            continue;
        }
        leaf.advance(time.delta_secs());
        transform.rotation = Quat::from_rotation_y(OPEN_ANGLE * leaf.fraction);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{
        BlockTemplate, PrimitiveTemplate, TemplateSet, definitions::BlockRenderKind,
    };
    use std::sync::Arc;
    use terra_voxel::{BlockSection, SectionPos};
    use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

    #[test]
    fn hinge_moves_smoothly_and_reverses_from_its_current_angle() {
        let mut leaf = Leaf::new(false);
        leaf.target(true);
        leaf.advance(DURATION / 4.0);
        assert!((leaf.fraction - 0.15625).abs() < 1.0e-6);
        let midway = leaf.fraction;
        leaf.target(false);
        assert_eq!(leaf.fraction, midway);
        leaf.advance(DURATION / 4.0);
        assert!(leaf.fraction > 0.0 && leaf.fraction < midway);
        leaf.advance(DURATION);
        assert_eq!(leaf.fraction, 0.0);
    }

    #[test]
    fn opening_has_no_initial_animation_flash_after_chunk_reload() {
        let mut leaf = Leaf::new(true);
        leaf.advance(1.0 / 60.0);
        assert_eq!(leaf.fraction, 1.0);
        assert_eq!(leaf.elapsed, DURATION);
        let point = HINGE + Quat::from_rotation_y(OPEN_ANGLE) * (Vec3::new(0.85, 1.0, 0.5) - HINGE);
        assert!((point - Vec3::new(0.15, 1.0, 1.2)).length() < 1.0e-6);
    }

    #[test]
    fn upper_door_half_keeps_shared_model_visible_after_owner_section_unloads() {
        let owner = [-1, 15, -16];
        let peer = [-1, 16, -16];
        let object = BlockObjectState {
            peer: Some(peer),
            ..default()
        };
        let (section, local) = terra_voxel::global_to_section(peer);
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block(local, 65).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        assert_eq!(chunks.block_at(owner), None);
        assert!(has_loaded_part(&chunks, owner, 65, object));
        assert!(!has_loaded_part(&chunks, owner, 66, object));
        assert!(!has_loaded_part(
            &chunks,
            owner,
            65,
            BlockObjectState::default()
        ));
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 0)
            .unwrap();
        assert!(!has_loaded_part(&chunks, owner, 65, object));
    }

    #[test]
    fn placed_opening_reuses_meshes_animates_and_cleans_up_on_unload() {
        let position = [1, 2, 3];
        let mut journal = EditJournal::default();
        journal.record(position, 68).unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
        let primitive = |material| PrimitiveTemplate {
            positions: vec![[0.2, 0.2, 0.5], [0.8, 0.2, 0.5], [0.2, 0.8, 0.5]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
            material,
        };
        let mut templates = TemplateSet::default();
        templates.hinge_materials.insert(1);
        templates.blocks.insert(
            68,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: vec![primitive(0), primitive(1)],
            },
        );
        let mut materials = Assets::<StandardMaterial>::default();
        let mut assets = BlockAssets::default();
        assets.materials = vec![
            materials.add(StandardMaterial::default()),
            materials.add(StandardMaterial::default()),
        ];
        assets.templates = Some(Arc::new(templates));
        let section = SectionPos::new(0, 0, 0);
        let mut chunks = ChunkStore::default();
        let loaded = || {
            let mut data = BlockSection::empty(section).unwrap();
            data.set_block([1, 2, 3], 68).unwrap();
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            }
        };
        chunks.loaded.insert(section, loaded());
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(assets)
            .insert_resource(materials)
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(journal)
            .insert_resource(chunks);
        install(&mut app);
        app.update();
        assert_eq!(app.world().resource::<Openings>().placed.len(), 1);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing: 4,
                    ..default()
                }),
            )
            .unwrap();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(110));
        app.update();
        let fraction = app
            .world_mut()
            .query::<&Leaf>()
            .single(app.world())
            .unwrap()
            .fraction;
        assert!(fraction > 0.0 && fraction < 0.5);
        app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(110));
        app.update();
        assert!(app.world().resource::<Openings>().placed.is_empty());
        assert_eq!(
            app.world_mut().query::<&Leaf>().iter(app.world()).count(),
            0
        );
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(section, loaded());
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(110));
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        assert_eq!(
            app.world_mut()
                .query::<&Leaf>()
                .single(app.world())
                .unwrap()
                .fraction,
            1.0
        );
    }
}
