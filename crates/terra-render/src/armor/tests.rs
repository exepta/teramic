use super::*;
use terra_core::{ArmorPiece, EquipmentSlot};

fn complete(material: ArmorMaterial) -> EquipmentSlots {
    let mut equipment = EquipmentSlots::default();
    for piece in ArmorPiece::ALL {
        assert!(equipment.set(
            piece.slot(),
            Some(ItemKind::Armor(ArmorItem { material, piece }))
        ));
    }
    equipment
}

#[test]
fn all_equipment_uses_valid_bounded_three_dimensional_geometry() {
    for item in ArmorItem::ALL
        .map(ItemKind::Armor)
        .into_iter()
        .chain(AccessoryKind::ALL.map(ItemKind::Accessory))
        .chain(terra_core::CapeItem::ALL.map(ItemKind::Cape))
    {
        for gender in [PlayerGender::Male, PlayerGender::Female] {
            let parts = model::parts(item, gender).unwrap();
            assert!(!parts.is_empty());
            assert!(parts.len() <= 3);
            let mut vertices = 0;
            for (_, part) in parts {
                vertices += part.positions.len();
                assert!(!part.positions.is_empty());
                assert_eq!(part.positions.len(), part.normals.len());
                assert_eq!(part.positions.len(), part.colors.len());
                assert_eq!(part.positions.len(), part.uvs.len());
                assert!(part.positions.iter().flatten().all(|v| v.is_finite()));
                assert!(
                    part.normals
                        .iter()
                        .all(|n| Vec3::from_array(*n).length_squared() > 0.99)
                );
                assert!(
                    part.indices
                        .iter()
                        .all(|index| (*index as usize) < part.positions.len())
                );
                assert_eq!(part.indices.len() % 3, 0);
                for triangle in part.indices.chunks_exact(3) {
                    let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                        .map(|i| Vec3::from_array(part.positions[i as usize]));
                    let normal = Vec3::from_array(part.normals[triangle[0] as usize]);
                    assert!((b - a).cross(c - a).dot(normal) > 0.0, "{item:?} winding");
                }
            }
            assert!(vertices < 24_000, "{item:?}: {vertices}");
        }
        let batches = item_batches(item, 17).unwrap();
        assert_eq!(batches.len(), 1);
        let batch = &batches[0];
        assert_eq!(batch.material, 17);
        assert!(
            batch
                .positions
                .iter()
                .flatten()
                .all(|coordinate| (0.07..=0.93).contains(coordinate))
        );
        for axis in 0..3 {
            let min = batch
                .positions
                .iter()
                .map(|p| p[axis])
                .fold(f32::INFINITY, f32::min);
            let max = batch
                .positions
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(max - min > 0.01, "{item:?} must not be a flat sprite");
        }
    }
}

#[test]
fn tiers_have_distinct_palette_and_crystal_armor_has_additional_facets() {
    let item = |material| {
        ItemKind::Armor(ArmorItem {
            material,
            piece: ArmorPiece::Helmet,
        })
    };
    let iron = item_batches(item(ArmorMaterial::Iron), 0)
        .unwrap()
        .remove(0);
    let gold = item_batches(item(ArmorMaterial::Gold), 0)
        .unwrap()
        .remove(0);
    let diamond = item_batches(item(ArmorMaterial::Diamond), 0)
        .unwrap()
        .remove(0);
    let ruby = item_batches(item(ArmorMaterial::Ruby), 0)
        .unwrap()
        .remove(0);
    assert_ne!(iron.colors, gold.colors);
    assert_ne!(gold.colors, diamond.colors);
    assert_ne!(diamond.colors, ruby.colors);
    assert!(diamond.indices.len() > gold.indices.len());
    assert_eq!(diamond.positions, ruby.positions);
}

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<ArmorRenderStatus>()
        .add_systems(Startup, prepare)
        .add_systems(Update, (sync, count).chain());
    app
}

fn avatar(app: &mut App, equipment: EquipmentSlots) -> (Entity, Vec<Entity>) {
    let root = app
        .world_mut()
        .spawn((
            AvatarEquipment::from(equipment),
            crate::players::AvatarGender(PlayerGender::Male),
        ))
        .id();
    let mut joints = Vec::new();
    for part in [
        AvatarPart::Body,
        AvatarPart::Head,
        AvatarPart::Arm(-1),
        AvatarPart::Arm(1),
        AvatarPart::Forearm(-1),
        AvatarPart::Forearm(1),
        AvatarPart::Leg(-1),
        AvatarPart::Leg(1),
        AvatarPart::Calf(-1),
        AvatarPart::Calf(1),
    ] {
        let joint = app
            .world_mut()
            .spawn((
                AvatarJoint {
                    root,
                    rest: Vec3::ZERO,
                },
                part,
                RenderLayers::default(),
                Transform::default(),
                ChildOf(root),
            ))
            .id();
        joints.push(joint);
    }
    (root, joints)
}

#[test]
fn equipment_swaps_reuse_shared_assets_and_despawning_cleans_all_attachments() {
    let mut app = app();
    let (root, joints) = avatar(&mut app, complete(ArmorMaterial::Iron));
    app.update();
    let meshes = app.world().resource::<Assets<Mesh>>().len();
    let materials = app.world().resource::<Assets<StandardMaterial>>().len();
    assert_eq!(meshes, 120);
    assert_eq!(materials, 7);
    assert_eq!(
        app.world().resource::<ArmorRenderStatus>().equipped_parts,
        11
    );
    for _ in 0..4 {
        for material in ArmorMaterial::ALL {
            *app.world_mut().get_mut::<AvatarEquipment>(root).unwrap() =
                AvatarEquipment::from(complete(material));
            app.update();
            assert_eq!(
                app.world().resource::<ArmorRenderStatus>().equipped_parts,
                11
            );
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), meshes);
            assert_eq!(
                app.world().resource::<Assets<StandardMaterial>>().len(),
                materials
            );
            for (_, parent, transform) in app
                .world_mut()
                .query::<(&EquipmentAttachment, &ChildOf, &Transform)>()
                .iter(app.world())
            {
                assert!(joints.contains(&parent.parent()));
                assert_eq!(*transform, Transform::default());
            }
        }
    }
    app.world_mut().despawn(root);
    app.update();
    assert_eq!(
        app.world().resource::<ArmorRenderStatus>().equipped_parts,
        0
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), meshes);
}

#[test]
fn armor_clear_and_mixed_pieces_do_not_leave_old_equipment_entities() {
    let mut app = app();
    let mut equipment = complete(ArmorMaterial::Leather);
    let (root, _) = avatar(&mut app, equipment);
    app.update();
    equipment.set(
        EquipmentSlot::Helmet,
        Some(ItemKind::Armor(ArmorItem {
            material: ArmorMaterial::Ruby,
            piece: ArmorPiece::Helmet,
        })),
    );
    equipment.set(
        EquipmentSlot::Necklace,
        Some(ItemKind::Accessory(AccessoryKind::Necklace)),
    );
    *app.world_mut().get_mut::<AvatarEquipment>(root).unwrap() = AvatarEquipment::from(equipment);
    app.update();
    assert_eq!(
        app.world().resource::<ArmorRenderStatus>().equipped_parts,
        12
    );
    *app.world_mut().get_mut::<AvatarEquipment>(root).unwrap() = AvatarEquipment::default();
    app.update();
    assert_eq!(
        app.world().resource::<ArmorRenderStatus>().equipped_parts,
        0
    );
    let rebuilds = app.world().resource::<ArmorRenderStatus>().rebuilds;
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<ArmorRenderStatus>().rebuilds,
        rebuilds
    );
}

#[test]
fn cape_cache_is_lazy_bounded_and_repeated_swaps_reuse_the_same_mesh_handles() {
    use terra_core::{CapeItem, beds::DyeColor};
    let mut app = app();
    let (root, _) = avatar(&mut app, EquipmentSlots::default());
    app.update();
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 120);
    let capes = [
        CapeItem {
            material: ArmorMaterial::Leather,
            color: DyeColor::White,
        },
        CapeItem {
            material: ArmorMaterial::Ruby,
            color: DyeColor::Blue,
        },
    ];
    for round in 0..4 {
        for (index, cape) in capes.into_iter().enumerate() {
            let mut slots = EquipmentSlots::default();
            slots.set(EquipmentSlot::Cape, Some(ItemKind::Cape(cape)));
            *app.world_mut().get_mut::<AvatarEquipment>(root).unwrap() =
                AvatarEquipment::from(slots);
            app.update();
            assert_eq!(
                app.world().resource::<ArmorRenderStatus>().equipped_parts,
                3
            );
            assert_eq!(
                app.world().resource::<Assets<Mesh>>().len(),
                120 + if round == 0 { 3 * (index + 1) } else { 6 }
            );
            assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 7);
        }
    }
    app.world_mut().despawn(root);
    app.update();
    assert_eq!(
        app.world().resource::<ArmorRenderStatus>().equipped_parts,
        0
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 126);
}

#[test]
fn capes_only_cover_the_back_and_keep_the_chest_and_face_clear() {
    for cape in terra_core::CapeItem::ALL {
        for gender in [PlayerGender::Male, PlayerGender::Female] {
            let parts = cape::parts(cape, gender).unwrap();
            assert_eq!(parts.len(), 3);
            for (_, batch) in &parts {
                assert!(batch.positions.iter().all(|p| p[2] > 0.19));
                assert!(batch.positions.iter().all(|p| p[1] < 0.37));
            }
            assert!(parts[1].1.positions.iter().all(|p| p[1] > -0.31));
            assert!(parts[2].1.positions.iter().all(|p| p[1] < -0.28));
            assert!(parts[2].1.positions.iter().any(|p| p[1] < -0.8));
        }
    }
}

#[test]
fn all_cape_colors_and_genders_keep_the_shared_cache_bounded() {
    let mut app = app();
    let (root, _) = avatar(&mut app, EquipmentSlots::default());
    app.update();
    let limit = 120 + terra_core::CapeItem::ALL.len() * 2 * 3;
    for _ in 0..2 {
        for gender in [PlayerGender::Male, PlayerGender::Female] {
            app.world_mut()
                .get_mut::<crate::players::AvatarGender>(root)
                .unwrap()
                .0 = gender;
            for cape in terra_core::CapeItem::ALL {
                let mut slots = EquipmentSlots::default();
                slots.set(EquipmentSlot::Cape, Some(ItemKind::Cape(cape)));
                *app.world_mut().get_mut::<AvatarEquipment>(root).unwrap() =
                    AvatarEquipment::from(slots);
                app.update();
                assert_eq!(
                    app.world().resource::<ArmorRenderStatus>().equipped_parts,
                    3
                );
                assert!(app.world().resource::<Assets<Mesh>>().len() <= limit);
                assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 7);
            }
        }
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), limit);
    }
}

fn moving_cape() -> (App, Entity) {
    let mut app = app();
    app.init_resource::<Time>()
        .init_resource::<crate::weather::WeatherVisual>()
        .add_systems(Update, animate_capes.after(sync));
    let mut slots = EquipmentSlots::default();
    slots.set(
        EquipmentSlot::Cape,
        Some(ItemKind::Cape(terra_core::CapeItem {
            material: ArmorMaterial::Iron,
            color: terra_core::beds::DyeColor::White,
        })),
    );
    let (root, _) = avatar(&mut app, slots);
    app.world_mut()
        .entity_mut(root)
        .insert(GlobalTransform::default());
    cape_frame(&mut app);
    (app, root)
}

fn cape_frame(app: &mut App) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(16));
    app.update();
}

fn cape_panels(app: &mut App) -> [Transform; 2] {
    let mut panels = [Transform::default(); 2];
    for (motion, transform) in app
        .world_mut()
        .query::<(&CapeMotion, &Transform)>()
        .iter(app.world())
    {
        panels[usize::from(motion.panel - 1)] = *transform;
    }
    panels
}

#[test]
fn cape_motion_follows_locomotion_without_rebuilding_geometry() {
    let (mut app, root) = moving_cape();
    let mesh_count = app.world().resource::<Assets<Mesh>>().len();
    for step in 1..=60 {
        *app.world_mut().get_mut::<GlobalTransform>(root).unwrap() =
            GlobalTransform::from_translation(-Vec3::Z * step as f32 * 0.096);
        cape_frame(&mut app);
    }
    let [upper, lower] = cape_panels(&mut app);
    let hem = Vec3::new(0.0, -0.82, 0.32);
    let animated = upper.transform_point(lower.transform_point(hem));
    assert!(
        animated.z - hem.z > 0.5,
        "cape must visibly trail behind a runner"
    );
    assert!(upper.rotation.angle_between(Quat::IDENTITY) > 0.35);
    assert!(lower.rotation.angle_between(Quat::IDENTITY) > 0.15);
    assert!((upper.transform_point(cape::SHOULDER_PIVOT) - cape::SHOULDER_PIVOT).length() < 0.0001);
    assert!((lower.transform_point(cape::HEM_PIVOT) - cape::HEM_PIVOT).length() < 0.0001);
    let panels: Vec<_> = app
        .world_mut()
        .query::<(Entity, &CapeMotion, &ChildOf)>()
        .iter(app.world())
        .map(|(entity, motion, parent)| (entity, motion.panel, parent.parent()))
        .collect();
    assert_eq!(panels.len(), 2);
    let upper_entity = panels.iter().find(|(_, panel, _)| *panel == 1).unwrap().0;
    assert_eq!(
        panels.iter().find(|(_, panel, _)| *panel == 2).unwrap().2,
        upper_entity
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
}

#[test]
fn stationary_capes_follow_existing_weather_wind_and_settle_in_calm_air() {
    let (mut app, root) = moving_cape();
    let meshes = app.world().resource::<Assets<Mesh>>().len();
    assert_eq!(cape_panels(&mut app), [Transform::default(); 2]);
    app.world_mut()
        .resource_mut::<crate::weather::WeatherVisual>()
        .wind = Vec2::new(8.0, 0.0);
    *app.world_mut().get_mut::<GlobalTransform>(root).unwrap() = GlobalTransform::from(
        Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
    );
    for _ in 0..60 {
        cape_frame(&mut app);
    }
    let [upper, lower] = cape_panels(&mut app);
    assert!(upper.rotation.angle_between(Quat::IDENTITY) > 0.6);
    assert!(lower.rotation.angle_between(Quat::IDENTITY) > 0.25);
    app.world_mut()
        .resource_mut::<crate::weather::WeatherVisual>()
        .wind = Vec2::ZERO;
    for _ in 0..90 {
        cape_frame(&mut app);
    }
    assert!(
        cape_panels(&mut app)
            .iter()
            .all(|panel| panel.rotation.angle_between(Quat::IDENTITY) < 0.01)
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), meshes);
}
