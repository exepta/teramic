//! Two shared meshes per client, not a mesh or entity per water droplet.
use super::*;
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
};
use terra_player::interaction::VesselMotion;

#[derive(Resource)]
struct Assets {
    liquid: Handle<Mesh>,
    stream: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    tar: Handle<StandardMaterial>,
    bile: Handle<StandardMaterial>,
}

#[derive(Component)]
struct Part {
    root: Entity,
    stream: bool,
}

pub(super) fn install(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, update.after(super::render));
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<bevy::asset::Assets<Mesh>>,
    mut materials: ResMut<bevy::asset::Assets<StandardMaterial>>,
) {
    commands.insert_resource(Assets {
        liquid: meshes.add(liquid_mesh()),
        stream: meshes.add(stream_mesh()),
        material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.16, 0.57, 0.78, 0.82),
            perceptual_roughness: 0.18,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        }),
        bile: materials.add(StandardMaterial {
            base_color: Color::srgba(0.56, 0.62, 0.08, 0.90),
            perceptual_roughness: 0.18,
            reflectance: 0.6,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        }),
        tar: materials.add(StandardMaterial {
            base_color: Color::srgb(0.035, 0.028, 0.022),
            perceptual_roughness: 0.16,
            reflectance: 0.55,
            cull_mode: None,
            ..default()
        }),
    });
}

fn liquid_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for side in 0..12 {
        let a = side as f32 * std::f32::consts::TAU / 12.;
        let b = (side + 1) as f32 * std::f32::consts::TAU / 12.;
        let p = |angle: f32, y: f32| [angle.cos(), y, angle.sin()];
        let base = positions.len() as u32;
        positions.extend([p(a, 0.), p(a, 1.), p(b, 1.), p(b, 0.)]);
        normals.extend([
            [a.cos(), 0., a.sin()],
            [a.cos(), 0., a.sin()],
            [b.cos(), 0., b.sin()],
            [b.cos(), 0., b.sin()],
        ]);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        let base = positions.len() as u32;
        positions.extend([[0., 1., 0.], p(b, 1.), p(a, 1.)]);
        normals.extend([[0., 1., 0.]; 3]);
        indices.extend([base, base + 1, base + 2]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn stream_mesh() -> Mesh {
    let mut builder = crate::players::model::Builder::default();
    for i in 0..8 {
        let y = i as f32 / 8.;
        let x = ((i * 7 % 5) as f32 - 2.) * 0.12;
        builder
            .voxel([x, y, 0.], [0.22, 0.065, 0.22], [0.55, 0.85, 1.])
            .expect("bounded vessel droplets");
    }
    let batch = builder.finish_batch().expect("bounded vessel stream");
    crate::previews::batch_mesh(&batch)
}

pub(super) fn is_vessel(item: ItemKind) -> bool {
    matches!(
        item,
        ItemKind::GlassBottle | ItemKind::WaterBottle | ItemKind::Potion(_)
    ) || matches!(
        item,
        ItemKind::Tool(
            terra_core::ToolKind::Bucket
                | terra_core::ToolKind::WaterBucket
                | terra_core::ToolKind::TarBucket
                | terra_core::ToolKind::BileBucket
        )
    ) || farm_kind(item) == Some(FarmToolKind::WateringCan)
}

pub(super) fn base(item: ItemKind) -> ItemKind {
    match item {
        ItemKind::Tool(
            terra_core::ToolKind::WaterBucket
            | terra_core::ToolKind::TarBucket
            | terra_core::ToolKind::BileBucket,
        ) => ItemKind::Tool(terra_core::ToolKind::Bucket),
        ItemKind::WaterBottle => ItemKind::GlassBottle,
        _ => item.preview_item(),
    }
}

fn default_fill(item: ItemKind) -> f32 {
    if let ItemKind::FilledWateringCan { tool, units } = item {
        f32::from(units) / f32::from(tool.watering_capacity())
    } else if matches!(
        item,
        ItemKind::WaterBottle
            | ItemKind::Tool(
                terra_core::ToolKind::WaterBucket
                    | terra_core::ToolKind::TarBucket
                    | terra_core::ToolKind::BileBucket
            )
    ) {
        1.
    } else {
        0.
    }
}

fn profile(item: ItemKind) -> Option<(Vec3, f32, f32)> {
    if farm_kind(item) == Some(FarmToolKind::WateringCan) {
        Some((Vec3::new(0.35, 0.105, 0.5), 0.145, 0.47))
    } else if matches!(
        item,
        ItemKind::Tool(
            terra_core::ToolKind::Bucket
                | terra_core::ToolKind::WaterBucket
                | terra_core::ToolKind::TarBucket
                | terra_core::ToolKind::BileBucket
        )
    ) {
        Some((Vec3::new(0.5, 0.18, 0.5), 0.22, 0.4075))
    } else if matches!(item, ItemKind::GlassBottle | ItemKind::WaterBottle) {
        Some((Vec3::new(0.5, 0.17, 0.5), 0.135, 0.38))
    } else {
        None
    }
}

fn liquid_pose(item: ItemKind, fill: f32) -> Option<Transform> {
    let (bottom, radius, height) = profile(item)?;
    let scale = crate::previews::tools::world_length(item).unwrap_or(0.24);
    Some(
        Transform::from_translation(crate::previews::tools::cache_vertex(item, bottom)).with_scale(
            Vec3::new(
                radius * scale,
                height * fill.clamp(0., 1.) * scale,
                radius * scale,
            ),
        ),
    )
}

pub(super) fn action_pose(mut pose: Transform, item: ItemKind, amount: f32) -> Transform {
    if amount <= 0.0 || !is_vessel(item) {
        return pose;
    }
    let angle = amount.clamp(0.0, 1.0) * 0.55;
    let grip = crate::previews::tools::grip(item).unwrap_or(Vec3::ZERO);
    let pivot = pose.transform_point(grip);
    pose.rotation *= Quat::from_rotation_z(-angle);
    pose.translation = pivot - pose.rotation * (grip * pose.scale);
    pose
}

pub(super) fn camera_safe(
    mut pose: Transform,
    camera: &Transform,
    bounds: (Vec3, Vec3),
) -> Transform {
    let mut nearest = f32::NEG_INFINITY;
    for x in [bounds.0.x, bounds.1.x] {
        for y in [bounds.0.y, bounds.1.y] {
            for z in [bounds.0.z, bounds.1.z] {
                nearest = nearest.max(
                    (camera.rotation.inverse()
                        * (pose.transform_point(Vec3::new(x, y, z)) - camera.translation))
                        .z,
                );
            }
        }
    }
    pose.translation -= camera.rotation * Vec3::Z * (nearest + 0.12).max(0.0);
    pose
}

fn update(
    mut commands: Commands,
    assets: Option<Res<Assets>>,
    motion: Option<Res<VesselMotion>>,
    time: Res<Time>,
    roots: Query<(Entity, &HeldItem, &Transform, &GlobalTransform)>,
    mut parts: Query<
        (
            &Part,
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<HeldItem>,
    >,
) {
    let Some(assets) = assets else {
        return;
    };
    for (root, held, pose, global) in &roots {
        if profile(held.item).is_none() {
            continue;
        }
        let motion = motion
            .as_deref()
            .filter(|motion| held.owner.is_none() && motion.item == Some(held.item));
        let fill = motion.map_or(default_fill(held.item), |motion| motion.fill);
        let fill = if fill.is_finite() {
            fill.clamp(0., 1.)
        } else {
            0.
        };
        let water = liquid_pose(held.item, fill).unwrap();
        let active = motion.is_some_and(|motion| motion.pouring);
        let material = if held.item == ItemKind::Tool(terra_core::ToolKind::TarBucket) {
            &assets.tar
        } else if held.item == ItemKind::Tool(terra_core::ToolKind::BileBucket) {
            &assets.bile
        } else {
            &assets.material
        };
        let spout = if farm_kind(held.item) == Some(FarmToolKind::WateringCan) {
            Vec3::new(0.87, 0.75, 0.5)
        } else {
            Vec3::new(0.5, 0.84, 0.5)
        };
        let rotation = if held.parent.is_none() {
            pose.rotation
        } else {
            global.rotation()
        };
        let mut stream =
            Transform::from_translation(crate::previews::tools::cache_vertex(held.item, spout));
        stream.rotation = Quat::from_rotation_arc(Vec3::Y, rotation.inverse() * Vec3::NEG_Y);
        let scale = if held.parent.is_none() {
            pose.scale
        } else {
            global.scale()
        };
        stream.scale = Vec3::new(0.045, 0.55, 0.045) / scale.abs().max(Vec3::splat(0.01));
        if active {
            stream.translation +=
                stream.rotation * Vec3::Y * ((time.elapsed_secs() * 2.).fract() * 0.025);
        }
        for (is_stream, next, visible) in [
            (false, water, fill.is_finite() && fill > 0.001),
            (true, stream, active),
        ] {
            if let Some((_, mut transform, mut visibility, mut surface)) = parts
                .iter_mut()
                .find(|(part, _, _, _)| part.root == root && part.stream == is_stream)
            {
                if surface.0 != *material {
                    surface.0 = material.clone();
                }
                if *transform != next {
                    *transform = next;
                }
                let next = if visible {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *visibility != next {
                    *visibility = next;
                }
            } else {
                commands.spawn((
                    Part {
                        root,
                        stream: is_stream,
                    },
                    Mesh3d(if is_stream {
                        assets.stream.clone()
                    } else {
                        assets.liquid.clone()
                    }),
                    MeshMaterial3d(material.clone()),
                    next,
                    if visible {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    },
                    ChildOf(root),
                    NotShadowCaster,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filled_held_vessels_reuse_empty_shells_and_actual_bounded_levels() {
        for item in [
            ItemKind::Tool(terra_core::ToolKind::WaterBucket),
            ItemKind::Tool(terra_core::ToolKind::TarBucket),
            ItemKind::Tool(terra_core::ToolKind::WoodWateringCan),
            ItemKind::WaterBottle,
        ] {
            let a = liquid_pose(item, 0.25).unwrap();
            let b = liquid_pose(item, 1.).unwrap();
            assert_eq!(a.translation, b.translation);
            assert!((a.scale.y * 4. - b.scale.y).abs() < 1e-6);
            assert_eq!(a.scale.x, b.scale.x);
            assert!(b.scale.min_element() > 0.);
        }
        assert_eq!(base(ItemKind::WaterBottle), ItemKind::GlassBottle);
        assert_eq!(
            default_fill(ItemKind::Tool(terra_core::ToolKind::WaterBucket)),
            1.
        );
    }
    #[test]
    fn shared_water_geometry_is_finite_and_top_faces_point_up() {
        let mesh = liquid_mesh();
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("water positions")
        };
        assert_eq!(positions.len(), 84);
        assert!(positions.iter().flatten().all(|v| v.is_finite()));
        for triangle in mesh
            .indices()
            .unwrap()
            .iter()
            .collect::<Vec<_>>()
            .chunks_exact(3)
        {
            let [a, b, c] = std::array::from_fn(|i| Vec3::from_array(positions[triangle[i]]));
            if a.y == 1. && b.y == 1. && c.y == 1. {
                assert!((b - a).cross(c - a).y > 0.);
            }
        }
    }

    #[test]
    fn idle_filled_held_items_share_handles_and_never_modify_mesh_assets() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<bevy::asset::Assets<Mesh>>()
            .init_resource::<bevy::asset::Assets<StandardMaterial>>()
            .add_systems(Startup, setup)
            .add_systems(Update, update);
        app.update();
        let roots: Vec<_> = [
            ItemKind::WaterBottle,
            ItemKind::Tool(terra_core::ToolKind::WaterBucket),
        ]
        .into_iter()
        .map(|item| {
            app.world_mut()
                .spawn((
                    HeldItem {
                        owner: None,
                        item,
                        parent: None,
                    },
                    Transform::default(),
                    GlobalTransform::default(),
                ))
                .id()
        })
        .collect();
        app.update();
        let handles: Vec<_> = app
            .world_mut()
            .query::<(&Part, &Mesh3d, &Visibility)>()
            .iter(app.world())
            .filter(|(part, _, _)| !part.stream)
            .map(|(_, mesh, visibility)| {
                assert_eq!(*visibility, Visibility::Inherited);
                mesh.0.id()
            })
            .collect();
        assert_eq!(handles.len(), 2);
        assert_eq!(handles[0], handles[1]);
        let count = app.world().resource::<bevy::asset::Assets<Mesh>>().len();
        app.world_mut().clear_trackers();
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<bevy::asset::Assets<Mesh>>().len(),
            count
        );
        assert!(
            !app.world()
                .get_resource_ref::<bevy::asset::Assets<Mesh>>()
                .unwrap()
                .is_changed()
        );
        for root in roots {
            app.world_mut().despawn(root);
        }
        assert_eq!(
            app.world_mut().query::<&Part>().iter(app.world()).count(),
            0
        );
    }

    #[test]
    fn idle_bucket_pose_faces_the_real_open_surface_toward_the_camera() {
        let item = ItemKind::Tool(terra_core::ToolKind::WaterBucket);
        let camera = Transform::default();
        let pose = first_person_pose(
            &camera,
            None,
            0.,
            item,
            (Vec3::new(-0.21, -0.28, -0.21), Vec3::new(0.21, 0.31, 0.21)),
        );
        let surface = liquid_pose(item, 1.).unwrap();
        let point = pose.transform_point(surface.translation + Vec3::Y * surface.scale.y);
        assert!((pose.rotation * Vec3::Y).dot(-point) > 0.);
    }

    #[test]
    fn actual_bottle_body_and_liquid_are_visible_above_the_lower_screen_edge() {
        let camera = Transform::default();
        for item in [ItemKind::GlassBottle, ItemKind::WaterBottle] {
            let batches = crate::previews::alchemy::batches(
                base(item),
                &crate::blocks::TemplateSet::default(),
            )
            .unwrap();
            let mut minimum = Vec3::splat(f32::INFINITY);
            let mut maximum = Vec3::splat(f32::NEG_INFINITY);
            for point in batches.iter().flat_map(|batch| &batch.positions) {
                let point = crate::previews::tools::cache_vertex(item, Vec3::from_array(*point));
                minimum = minimum.min(point);
                maximum = maximum.max(point);
            }
            let pose = camera_safe(
                first_person_pose(&camera, None, 0., item, (minimum, maximum)),
                &camera,
                (minimum, maximum),
            );
            let tangent = (65.0_f32.to_radians() * 0.5).tan();
            let points = [
                Vec3::new(0.5, 0.49, 0.5),
                Vec3::new(0.5, 0.55, 0.5),
                Vec3::new(0.5, 0.61, 0.5),
            ];
            for point in points {
                let point = pose.transform_point(crate::previews::tools::cache_vertex(item, point));
                let projected_y = point.y / (-point.z * tangent);
                assert!(
                    (-0.92..=0.92).contains(&projected_y),
                    "bottle body/liquid is outside the usable screen: {projected_y}"
                );
            }
            assert!(pose.scale.x * (maximum.y - minimum.y) > 0.40);
        }
    }

    #[test]
    fn actual_can_pour_tips_the_spout_at_a_fixed_grip_without_crossing_the_camera() {
        let item = ItemKind::Tool(terra_core::ToolKind::IronWateringCan);
        let idle =
            Transform::from_rotation(Quat::from_rotation_y(0.3)).with_scale(Vec3::splat(0.8));
        let poured = action_pose(idle, item, 1.0);
        let grip = crate::previews::tools::grip(item).unwrap();
        assert!(
            idle.transform_point(grip)
                .abs_diff_eq(poured.transform_point(grip), 1e-6)
        );
        assert!((idle.rotation.inverse() * poured.rotation * Vec3::X).y < -0.4);
        let bounds = (Vec3::splat(-0.4), Vec3::splat(0.4));
        let safe = camera_safe(poured, &Transform::default(), bounds);
        for x in [-0.4, 0.4] {
            for y in [-0.4, 0.4] {
                for z in [-0.4, 0.4] {
                    assert!(safe.transform_point(Vec3::new(x, y, z)).z <= -0.11999);
                }
            }
        }
    }
}
