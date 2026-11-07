//! Small shared fruit instances update independently from chunk/foliage meshes.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::{HashMap, HashSet};
use terra_world::{
    fruit::FruitProps,
    props::{
        PropId, WorldProp,
        visual::{PropDetail, PropPalette},
    },
    streaming::ChunkFocus,
};

const MAX_VISIBLE: usize = 1024;
#[derive(Default, Resource)]
struct FruitMeshes {
    meshes: HashMap<u8, Handle<Mesh>>,
    placed: HashMap<PropId, (Entity, WorldProp)>,
    next: f32,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<FruitMeshes>().add_systems(
        Update,
        synchronize.after(terra_world::streaming::StreamingSet::Edits),
    );
}

#[allow(clippy::too_many_arguments)]
fn synchronize(
    mut commands: Commands,
    time: Res<Time>,
    props: Option<Res<FruitProps>>,
    focus: Res<ChunkFocus>,
    assets: Res<crate::blocks::BlockAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: ResMut<FruitMeshes>,
) {
    if time.elapsed_secs() < state.next {
        return;
    }
    state.next = time.elapsed_secs() + 0.1;
    let Some(templates) = assets.templates.as_ref() else {
        return;
    };
    let material = templates.prop_materials.crop;
    let Some(material_handle) = assets.materials.get(material) else {
        return;
    };
    let center = Vec3::from_array(focus.0.map(|v| v as f32));
    let mut visible = props
        .as_deref()
        .into_iter()
        .flat_map(|p| p.props.iter())
        .filter(|p| {
            (Vec3::from_array(p.position.map(|v| v as f32)) - center).length_squared()
                < 128.0 * 128.0
        })
        .collect::<Vec<_>>();
    visible.sort_unstable_by(|a, b| {
        let d = |p: &&WorldProp| {
            (Vec3::from_array(p.position.map(|v| v as f32)) - center).length_squared()
        };
        d(a).total_cmp(&d(b))
    });
    let mut live = HashSet::new();
    for prop in visible.into_iter().take(MAX_VISIBLE) {
        live.insert(prop.id);
        if let Some(&(entity, old)) = state.placed.get(&prop.id) {
            if old.variant == prop.variant {
                if old != *prop {
                    commands.entity(entity).insert(
                        Transform::from_translation(Vec3::from_array(
                            prop.position.map(|v| v as f32),
                        ))
                        .with_rotation(Quat::from_rotation_y(prop.rotation))
                        .with_scale(Vec3::splat(prop.scale)),
                    );
                    state.placed.insert(prop.id, (entity, *prop));
                }
                continue;
            }
            commands.entity(entity).despawn();
            state.placed.remove(&prop.id);
        }
        let mesh = state
            .meshes
            .entry(prop.variant)
            .or_insert_with(|| {
                let mut part = terra_world::props::visual::apple(
                    prop.variant,
                    PropDetail::Near,
                    &PropPalette {
                        crop: material,
                        ..default()
                    },
                )
                .remove(0);
                let mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                )
                .with_inserted_attribute(
                    Mesh::ATTRIBUTE_POSITION,
                    std::mem::take(&mut part.positions),
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, part.normals)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, part.uvs)
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, part.colors)
                .with_inserted_indices(Indices::U32(part.indices));
                meshes.add(mesh)
            })
            .clone();
        let entity = commands
            .spawn((
                Name::new("Orchard fruit"),
                Mesh3d(mesh),
                MeshMaterial3d(material_handle.clone()),
                Transform::from_translation(Vec3::from_array(prop.position.map(|v| v as f32)))
                    .with_rotation(Quat::from_rotation_y(prop.rotation))
                    .with_scale(Vec3::splat(prop.scale)),
                Visibility::Inherited,
            ))
            .id();
        state.placed.insert(prop.id, (entity, *prop));
    }
    state.placed.retain(|id, (entity, _)| {
        if live.contains(id) {
            true
        } else {
            commands.entity(*entity).despawn();
            false
        }
    });
}
