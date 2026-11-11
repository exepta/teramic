//! Bounded warm spotlights and retained, non-coplanar number-plate lettering.
use super::*;
use bevy::{
    asset::RenderAssetUsages,
    light::VolumetricLight,
    mesh::{Indices, PrimitiveTopology},
};
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Mounted(u64, usize),
    Placed([i64; 3]),
}
#[derive(Clone, PartialEq)]
enum Kind {
    Headlight,
    Ceiling,
    Plate(String),
    DoorPlate(String, bool),
}
struct Entry {
    entity: Entity,
    parent: Option<Entity>,
    kind: Kind,
    mesh: Option<Handle<Mesh>>,
}
#[derive(Resource, Default)]
struct Details {
    entries: BTreeMap<Key, Entry>,
    next: f64,
    gold: Option<Handle<StandardMaterial>>,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<Details>()
        .add_systems(Update, synchronize.after(super::synchronize));
}
fn kind(material: u16, name: &str, facing: u8) -> Option<Kind> {
    match material {
        264 | 265 => Some(Kind::Headlight),
        294 => Some(Kind::Ceiling),
        223 => Some(Kind::DoorPlate(
            if name.is_empty() {
                "52 4867".into()
            } else {
                name.to_string()
            },
            terra_core::train_boiler::is_wide(material, facing),
        )),
        266 => Some(Kind::Plate(if name.is_empty() {
            "52 4867".into()
        } else {
            name.to_string()
        })),
        _ => None,
    }
}
fn pose(cell: [f32; 3], facing: u8, kind: &Kind) -> Transform {
    let yaw = Quat::from_rotation_y(f32::from(facing % 4) * std::f32::consts::FRAC_PI_2);
    let local = match kind {
        Kind::Headlight => Vec3::new(0.0, 0.55, -0.41),
        Kind::Ceiling => Vec3::new(0.0, 0.76, 0.0),
        Kind::Plate(_) | Kind::DoorPlate(_, _) => Vec3::ZERO,
    };
    Transform::from_translation(Vec3::from_array(cell) + Vec3::new(0.5, 0.0, 0.5) + yaw * local)
        .with_scale(if matches!(kind, Kind::DoorPlate(_, true)) {
            Vec3::new(3.0, 3.0, 1.0)
        } else {
            Vec3::ONE
        })
        .with_rotation(
            yaw * if *kind == Kind::Ceiling {
                Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
            } else {
                Quat::IDENTITY
            },
        )
}
fn spotlight(ceiling: bool) -> SpotLight {
    SpotLight {
        color: Color::srgb(1.0, 0.83, 0.57),
        intensity: if ceiling { 4200.0 } else { 270_000.0 },
        range: if ceiling { 7.0 } else { 48.0 },
        radius: 0.12,
        inner_angle: if ceiling { 0.65 } else { 0.22 },
        outer_angle: if ceiling { 1.25 } else { 0.48 },
        shadow_maps_enabled: !ceiling,
        ..default()
    }
}
#[derive(bevy::ecs::system::SystemParam)]
struct Scene<'w, 's> {
    session: Option<Res<'w, terra_network::client::ClientSession>>,
    access: Res<'w, WorldAccess>,
    journal: Res<'w, EditJournal>,
    chunks: Res<'w, ChunkStore>,
    trains: Res<'w, Trains>,
    cameras: Query<'w, 's, &'static Transform, With<terra_player::TerraCamera>>,
}
fn synchronize(
    mut commands: Commands,
    time: Res<Time>,
    scene: Scene,
    mut state: ResMut<Details>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if time.elapsed_secs_f64() < state.next {
        return;
    }
    state.next = time.elapsed_secs_f64() + 0.15;
    let listener = scene
        .cameras
        .iter()
        .next()
        .map_or(Vec3::ZERO, |camera| camera.translation);
    let mut desired = Vec::new();
    if let Some(session) = scene.session.as_ref().filter(|session| session.ready) {
        for car in &session.trains.vehicles {
            let Some(placed) = scene.trains.placed.get(&car.id) else {
                continue;
            };
            for (index, part) in car.parts.iter().enumerate() {
                if let Some(kind) = kind(part.material, &car.name, part.facing) {
                    let position = Vec3::from_array(
                        car.world_point(part.offset.map(|v| f64::from(v) + 0.5))
                            .map(|v| v as f32),
                    );
                    let distance = listener.distance_squared(position);
                    if distance < 64.0 * 64.0 {
                        desired.push((
                            distance,
                            Key::Mounted(car.id, index),
                            Some(placed.root),
                            kind.clone(),
                            pose(part.offset.map(f32::from), part.facing, &kind),
                        ));
                    }
                }
            }
        }
    } else if scene.access.authoritative() {
        for car in scene.journal.gameplay().trains.vehicles.values() {
            let Some(placed) = scene.trains.placed.get(&car.id) else {
                continue;
            };
            for (index, part) in car.parts.iter().enumerate() {
                if let Some(kind) = kind(part.material, &car.name, part.facing) {
                    let position = Vec3::from_array(
                        car.world_point(part.offset.map(|v| f64::from(v) + 0.5))
                            .map(|v| v as f32),
                    );
                    let distance = listener.distance_squared(position);
                    if distance < 64.0 * 64.0 {
                        desired.push((
                            distance,
                            Key::Mounted(car.id, index),
                            Some(placed.root),
                            kind.clone(),
                            pose(part.offset.map(f32::from), part.facing, &kind),
                        ));
                    }
                }
            }
        }
    }
    for section in scene.chunks.loaded.keys() {
        let Ok(origin) = section.origin() else {
            continue;
        };
        if listener.distance_squared(Vec3::from_array(origin.map(|v| v as f32)) + Vec3::splat(8.0))
            > 80.0 * 80.0
        {
            continue;
        }
        for (cell, material, selected) in scene.journal.object_states_in_section(*section) {
            let Some(kind) = kind(material, "", selected.facing) else {
                continue;
            };
            if scene.chunks.block_at(cell) != Some(material) {
                continue;
            }
            let distance = listener.distance_squared(Vec3::from_array(cell.map(|v| v as f32)));
            if distance < 64.0 * 64.0 {
                desired.push((
                    distance,
                    Key::Placed(cell),
                    None,
                    kind.clone(),
                    pose(cell.map(|v| v as f32), selected.facing, &kind),
                ));
            }
        }
    }
    desired.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut lamps = 0;
    let mut plates = 0;
    desired.retain(|(_, _, _, kind, _)| match kind {
        Kind::Plate(_) | Kind::DoorPlate(_, _) => {
            plates += 1;
            plates <= 32
        }
        _ => {
            lamps += 1;
            lamps <= 12
        }
    });
    let keys: BTreeSet<_> = desired.iter().map(|(_, key, ..)| key.clone()).collect();
    let removed: Vec<_> = state
        .entries
        .keys()
        .filter(|key| !keys.contains(*key))
        .cloned()
        .collect();
    for key in removed {
        if let Some(old) = state.entries.remove(&key) {
            commands.entity(old.entity).try_despawn();
            if let Some(mesh) = old.mesh {
                meshes.remove(mesh.id());
            }
        }
    }
    for (_, key, parent, kind, transform) in desired {
        if let Some(old) = state.entries.get(&key)
            && old.parent == parent
            && old.kind == kind
        {
            commands.entity(old.entity).insert(transform);
            continue;
        }
        if let Some(old) = state.entries.remove(&key) {
            commands.entity(old.entity).try_despawn();
            if let Some(mesh) = old.mesh {
                meshes.remove(mesh.id());
            }
        }
        let (entity, mesh) = if let Kind::Plate(name) | Kind::DoorPlate(name, _) = &kind {
            let mesh = meshes.add(lettering(name, matches!(kind, Kind::DoorPlate(_, _))));
            let gold = state
                .gold
                .get_or_insert_with(|| {
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.94, 0.74, 0.36),
                        metallic: 0.65,
                        perceptual_roughness: 0.5,
                        ..default()
                    })
                })
                .clone();
            (
                commands
                    .spawn((
                        Name::new("Train number plate lettering"),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(gold),
                        transform,
                    ))
                    .id(),
                Some(mesh),
            )
        } else {
            (
                commands
                    .spawn((
                        Name::new("Train cone light"),
                        spotlight(kind == Kind::Ceiling),
                        VolumetricLight,
                        transform,
                    ))
                    .id(),
                None,
            )
        };
        if let Some(parent) = parent {
            commands.entity(parent).add_child(entity);
        }
        state.entries.insert(
            key,
            Entry {
                entity,
                parent,
                kind,
                mesh,
            },
        );
    }
}
/// Five-by-seven metal lettering; unknown glyphs have a visible question-mark fallback.
fn glyph(character: char) -> [u8; 7] {
    match character.to_ascii_uppercase() {
        'A' | 'Ä' | 'ä' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 14],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' | 'Ö' | 'ö' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' | 'ß' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' | 'Ü' | 'ü' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        ' ' => [0; 7],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        '_' => [0, 0, 0, 0, 0, 0, 31],
        _ => [14, 17, 1, 2, 4, 0, 4],
    }
}
fn lettering(name: &str, door: bool) -> Mesh {
    let letters: Vec<_> = name.chars().take(24).collect();
    let pixel = (0.82 / (letters.len().max(1) * 6) as f32).min(0.033);
    let z = if door { -0.506 } else { -0.426 };
    let left = -(letters.len() as f32 * 6.0 - 1.0) * pixel * 0.5;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for (letter, character) in letters.into_iter().enumerate() {
        for (row, bits) in glyph(character).into_iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                let x = left + (letter * 6 + column) as f32 * pixel;
                let y = 0.5 + (3 - row as i32) as f32 * pixel;
                let base = positions.len() as u32;
                positions.extend(
                    [
                        [x, y, z],
                        [x, y + pixel * 0.88, z],
                        [x + pixel * 0.88, y + pixel * 0.88, z],
                        [x + pixel * 0.88, y, z],
                    ]
                    .map(|mut point| {
                        if door {
                            point[0] = -point[0];
                        }
                        point
                    }),
                );
                normals.extend([[0.0, 0.0, if door { -1.0 } else { 1.0 }]; 4]);
                uvs.extend([[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
                indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
            }
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn train_lamps_are_fifty_percent_brighter_without_changing_their_beams() {
        for (ceiling, previous_intensity, range, inner_angle, outer_angle) in [
            (false, 180_000.0, 48.0, 0.22, 0.48),
            (true, 2800.0, 7.0, 0.65, 1.25),
        ] {
            let light = spotlight(ceiling);
            assert_eq!(light.intensity, previous_intensity * 1.5);
            assert_eq!(light.range, range);
            assert_eq!(light.inner_angle, inner_angle);
            assert_eq!(light.outer_angle, outer_angle);
            assert_eq!(light.color, Color::srgb(1.0, 0.83, 0.57));
            assert_eq!(light.shadow_maps_enabled, !ceiling);
        }
    }
    #[test]
    fn lamps_point_forward_or_down_with_a_real_cone() {
        for facing in 0..4 {
            for kind in [Kind::Headlight, Kind::Ceiling] {
                let transform = pose([0.0; 3], facing, &kind);
                let direction = transform.rotation * Vec3::NEG_Z;
                if kind == Kind::Ceiling {
                    assert!(direction.distance(Vec3::NEG_Y) < 1e-5);
                } else {
                    assert!(
                        direction.distance(
                            Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2)
                                * Vec3::NEG_Z
                        ) < 1e-5
                    );
                }
                let light = spotlight(kind == Kind::Ceiling);
                assert!(light.intensity > 1000.0 && light.outer_angle > light.inner_angle);
            }
        }
    }
    #[test]
    fn lettering_is_finite_and_in_front_of_the_plate_without_coplanar_faces() {
        for name in ["52 4867", "TERAMIC EXPRESS", "Köln 12"] {
            for door in [false, true] {
                let mesh = lettering(name, door);
                let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("positions");
                };
                assert!(!positions.is_empty());
                assert!(
                    positions
                        .iter()
                        .all(|point| point.iter().all(|value| value.is_finite())
                            && point[0].abs() < 0.5
                            && if door {
                                point[2] < -0.5
                            } else {
                                point[2] > -0.435 && point[2] < -0.4
                            })
                );
                let Some(Indices::U32(indices)) = mesh.indices() else {
                    panic!("indices");
                };
                let normal = if door { Vec3::NEG_Z } else { Vec3::Z };
                for triangle in indices.chunks_exact(3) {
                    let [a, b, c] = std::array::from_fn(|index| {
                        Vec3::from_array(positions[triangle[index] as usize])
                    });
                    assert!(
                        (b - a).cross(c - a).dot(normal) > 0.0,
                        "front lettering must face the observer"
                    );
                }
            }
        }
    }
}
