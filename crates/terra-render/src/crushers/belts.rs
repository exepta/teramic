use super::*;
use terra_world::conveyors::{PROFILE_RADIUS, Span};

const THICKNESS: f32 = 0.02;
const HALF_WIDTH: f32 = 0.32;
const ARC_STEPS: usize = 16;
pub(super) const LEGACY_RUN: f32 = 0.75;

#[derive(Default)]
pub(super) struct Cache {
    meshes: HashMap<(u8, bool, u8), (Handle<Mesh>, Aabb)>,
}

#[derive(Component)]
pub(super) struct Surface {
    pub root: Entity,
    pub material: SurfaceMaterial,
    pub flow_sign: f32,
    pub offset: f32,
    pub contacts: Contacts,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Contacts {
    mask: u8,
    inventories: [Option<InventoryContact>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ContactPart {
    position: [i64; 3],
    material: u16,
    state: terra_world::edits::BlockObjectState,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct InventoryContact {
    parts: [Option<ContactPart>; 4],
}

pub(super) enum SurfaceMaterial {
    Standard(Handle<StandardMaterial>),
    Rgb(Handle<ribbon_tint::Material>),
}

pub(super) fn step_length(span: Span) -> f32 {
    span.step
        .iter()
        .map(|v| (*v as f32).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
fn contact_mask(chunks: &ChunkStore, journal: &EditJournal, span: Span) -> u8 {
    contacts(chunks, journal, span, Contacts::default()).mask
}

pub(super) fn contacts(
    chunks: &ChunkStore,
    journal: &EditJournal,
    span: Span,
    previous: Contacts,
) -> Contacts {
    if !span.linked || span.step[1] != 0 || step_length(span) != 1.0 || span.normal()[1] < 0.999 {
        return Contacts::default();
    }
    let mut result = Contacts::default();
    for (end, cell, sign) in [(0, span.start, -1), (1, span.end, 1)] {
        if let Some(contact) = inventory_contact(
            chunks,
            journal,
            cell,
            span.step.map(|value| value * sign),
            previous.inventories[end],
        ) {
            result.mask |= 1 << (end * 2);
            result.inventories[end] = Some(contact);
            continue;
        }
        let target = std::array::from_fn(|axis| cell[axis] + span.step[axis] * sign);
        let port = [target[0], target[1] + 1, target[2]];
        let Some(port_material) = journal
            .block(port)
            .filter(|material| matches!(material, 158 | 160))
        else {
            continue;
        };
        if chunks.block_at(port) != Some(port_material)
            || !journal
                .object_state(port)
                .is_some_and(|state| state.valid_for(port, port_material))
        {
            continue;
        }
        let Some(target) = terra_world::conveyors::segment_at(journal, target).filter(|target| {
            target.span.controller != span.controller
                && target.span.step[1] == 0
                && target.span.normal()[1] > 0.999
                && step_length(target.span) == 1.0
                && target.span.loaded(chunks, journal)
        }) else {
            continue;
        };
        let parallel = span
            .step
            .iter()
            .zip(target.span.step)
            .map(|(a, b)| a * b)
            .sum::<i64>()
            .abs()
            == 1;
        let contact = if parallel {
            if target.span.linked { 1 } else { 3 }
        } else {
            2
        };
        result.mask |= contact << (end * 2);
    }
    result
}

fn inventory_contact(
    chunks: &ChunkStore,
    journal: &EditJournal,
    endpoint: [i64; 3],
    outward: [i64; 3],
    previous: Option<InventoryContact>,
) -> Option<InventoryContact> {
    use terra_core::logistics;
    let port = [endpoint[0], endpoint[1] + 1, endpoint[2]];
    let mut contact = InventoryContact::default();
    let mut push = |position| {
        let material = journal.block(position)?;
        let state = journal
            .object_state(position)
            .filter(|state| state.valid_for(position, material))?;
        let part = ContactPart {
            position,
            material,
            state,
        };
        if !contact.parts.contains(&Some(part)) {
            *contact.parts.iter_mut().find(|part| part.is_none())? = Some(part);
        }
        Some(part)
    };
    let port_part = push(port)?;
    if !matches!(
        port_part.material,
        logistics::TUNNEL | logistics::FUNNEL | logistics::FILTERED_FUNNEL | 159
    ) {
        return None;
    }
    let peer = std::array::from_fn(|axis| port[axis] + outward[axis]);
    if port_part.state.peer != Some(peer) {
        return None;
    }
    let peer_part = push(peer)?;
    let inventory = if peer_part.material == terra_core::crusher::HAND_CRANK_MATERIAL {
        if port
            != terra_core::kinetic::offset(peer, terra_core::kinetic::axis(peer_part.state.facing))
        {
            return None;
        }
        let anchor = journal.crank_anchor(peer)?;
        let anchor_part = push(anchor)?;
        if !matches!(
            anchor_part.material,
            terra_core::crusher::MATERIAL | terra_core::mixer::MATERIAL
        ) {
            return None;
        }
        anchor_part
    } else {
        peer_part
    };
    if !logistics::inventory(inventory.material) {
        return None;
    }
    if let Some(peer) = inventory.state.peer {
        let paired = push(peer)?;
        if paired.material != inventory.material
            || paired.state.peer != Some(inventory.position)
            || paired.state.facing != inventory.state.facing
            || paired.state.secondary == inventory.state.secondary
        {
            return None;
        }
    }
    // Only an already observed, unchanged attachment may survive an unloaded
    // neighbor. Loaded contradictions or journal edits always revoke it.
    let mut unloaded = false;
    for part in contact.parts.iter().flatten() {
        match chunks.block_at(part.position) {
            Some(material) if material == part.material => {}
            Some(_) => return None,
            None => unloaded = true,
        }
    }
    (!unloaded || previous == Some(contact)).then_some(contact)
}

pub(super) fn pose(span: Span) -> Transform {
    let direction = Vec3::from_array(span.direction());
    let normal = Vec3::from_array(span.normal());
    let width = normal.cross(direction).normalize();
    Transform::from_translation(Vec3::from_array(span.start.map(|v| v as f32 + 0.5)))
        .with_rotation(Quat::from_mat3(&Mat3::from_cols(width, normal, direction)))
}

fn append_quad(batch: &mut MeshBatch, points: [Vec3; 4], uv: [[f32; 2]; 4], shade: f32) {
    let index = batch.positions.len() as u32;
    let normal = (points[1] - points[0])
        .cross(points[2] - points[0])
        .normalize();
    batch.positions.extend(points.map(|point| point.to_array()));
    batch.normals.extend([normal.to_array(); 4]);
    let tangent = crate::blocks::tangent_frame(
        points[1] - points[0],
        normal,
        normal
            .cross(points[1] - points[0])
            .dot(points[3] - points[0]),
    );
    batch.tangents.extend([tangent; 4]);
    batch.uvs.extend(uv);
    batch.colors.extend([[shade, shade, shade, 1.0]; 4]);
    batch
        .indices
        .extend([index, index + 1, index + 2, index, index + 2, index + 3]);
}

pub(super) fn geometry(length: u8, step: f32) -> MeshBatch {
    let radius = PROFILE_RADIUS - THICKNESS * 0.5;
    let distance = (f32::from(length) + 1.0) * step;
    let mut frames = vec![
        (Vec3::Y * radius, Vec3::Y, 0.0),
        (Vec3::new(0.0, radius, distance), Vec3::Y, distance),
    ];
    for i in 1..=ARC_STEPS {
        let angle = std::f32::consts::PI * i as f32 / ARC_STEPS as f32;
        let normal = Vec3::new(0.0, angle.cos(), angle.sin());
        frames.push((
            Vec3::Z * distance + normal * radius,
            normal,
            distance + radius * angle,
        ));
    }
    frames.push((
        Vec3::NEG_Y * radius,
        Vec3::NEG_Y,
        distance * 2.0 + std::f32::consts::PI * radius,
    ));
    for i in 1..=ARC_STEPS {
        let angle = std::f32::consts::PI * (1.0 + i as f32 / ARC_STEPS as f32);
        let normal = Vec3::new(0.0, angle.cos(), angle.sin());
        frames.push((normal * radius, normal, distance * 2.0 + radius * angle));
    }
    let ring = |(center, normal, _): (Vec3, Vec3, f32)| {
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .map(|(x, y)| center + Vec3::X * x * HALF_WIDTH + normal * y * THICKNESS * 0.5)
    };
    let mut batch = MeshBatch::default();
    for pair in frames.windows(2) {
        let a = ring(pair[0]);
        let b = ring(pair[1]);
        for side in 0..4 {
            let next = (side + 1) % 4;
            append_quad(
                &mut batch,
                [a[side], a[next], b[next], b[side]],
                [
                    [0.0, pair[0].2],
                    [0.64, pair[0].2],
                    [0.64, pair[1].2],
                    [0.0, pair[1].2],
                ],
                if side == 2 { 1.0 } else { 0.58 },
            );
        }
    }
    batch
}

fn geometry_with_contacts(length: u8, step: f32, contacts: u8) -> MeshBatch {
    let mut batch = geometry(length, step);
    let distance = (f32::from(length) + 1.0) * step;
    for end in 0..2 {
        let reach = match (contacts >> (end * 2)) & 3 {
            1 => 0.5,
            2 => 1.0 - HALF_WIDTH,
            3 => 1.0 - LEGACY_RUN * 0.5,
            _ => continue,
        };
        let (near, far) = if end == 0 {
            (-reach, 0.0)
        } else {
            (distance, distance + reach)
        };
        let point = |x, y, z| Vec3::new(x, y, z);
        let w = HALF_WIDTH;
        let top = PROFILE_RADIUS;
        let bottom = top - THICKNESS;
        let faces = [
            [
                point(w, top, near),
                point(-w, top, near),
                point(-w, top, far),
                point(w, top, far),
            ],
            [
                point(-w, bottom, near),
                point(w, bottom, near),
                point(w, bottom, far),
                point(-w, bottom, far),
            ],
            [
                point(-w, bottom, near),
                point(-w, bottom, far),
                point(-w, top, far),
                point(-w, top, near),
            ],
            [
                point(w, bottom, far),
                point(w, bottom, near),
                point(w, top, near),
                point(w, top, far),
            ],
            [
                point(w, bottom, near),
                point(-w, bottom, near),
                point(-w, top, near),
                point(w, top, near),
            ],
            [
                point(-w, bottom, far),
                point(w, bottom, far),
                point(w, top, far),
                point(-w, top, far),
            ],
        ];
        for (index, points) in faces.into_iter().enumerate() {
            let u = (points[1] - points[0]).normalize();
            let v = (points[3] - points[0]).normalize();
            let uv = points.map(|point| {
                if index == 0 {
                    [w - point.x, point.z]
                } else {
                    [point.dot(u), point.dot(v)]
                }
            });
            append_quad(&mut batch, points, uv, if index == 0 { 1.0 } else { 0.58 });
        }
    }
    batch
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn(
    commands: &mut Commands,
    cache: &mut Cache,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    tint_materials: &mut Assets<ribbon_tint::Material>,
    span: Span,
    root: Entity,
    source: &Handle<StandardMaterial>,
    tint: Option<[u8; 3]>,
    offset: f32,
    contacts: Contacts,
    drive_facing: u8,
) -> Option<()> {
    let key = (span.length, step_length(span) > 1.1, contacts.mask);
    let (mesh, bounds) = cache.meshes.entry(key).or_insert_with(|| {
        let batch = geometry_with_contacts(span.length, step_length(span), contacts.mask);
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, batch.tangents)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, batch.colors)
        .with_inserted_indices(Indices::U32(batch.indices));
        let bounds = mesh.get_aabb().expect("A bounded conveyor profile");
        (meshes.add(mesh), bounds)
    });
    let mut material = materials.get(source)?.clone();
    material.uv_transform = bevy::math::Affine2::from_translation(Vec2::new(0.0, offset));
    let material = if tint.is_some() {
        SurfaceMaterial::Rgb(tint_materials.add(ribbon_tint::material(material)))
    } else {
        SurfaceMaterial::Standard(materials.add(material))
    };
    let mut entity = commands.spawn((
        Name::new("Stretched conveyor ribbon"),
        Mesh3d(mesh.clone()),
        *bounds,
        NoAutoAabb,
        Transform::default(),
        crate::miniature::layers(),
        terra_core::miniature::MiniatureMesh::Terrain,
        ChildOf(root),
    ));
    match &material {
        SurfaceMaterial::Standard(handle) => {
            entity.insert(MeshMaterial3d(handle.clone()));
        }
        SurfaceMaterial::Rgb(handle) => {
            entity.insert((
                MeshMaterial3d(handle.clone()),
                ribbon_tint::tag(tint.unwrap()),
            ));
        }
    }
    commands.entity(root).insert(Surface {
        root,
        material,
        flow_sign: span.flow_sign()
            * terra_core::kinetic::axis(span.facing)
                .into_iter()
                .zip(terra_core::kinetic::axis(drive_facing))
                .map(|(a, b)| (a * b) as f32)
                .sum::<f32>(),
        offset,
        contacts,
    });
    Some(())
}

pub(super) fn animate(
    time: Res<Time>,
    mut surfaces: Query<&mut Surface>,
    drives: Query<&Drive>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tint_materials: ResMut<Assets<ribbon_tint::Material>>,
) {
    for mut surface in &mut surfaces {
        let Ok(drive) = drives.get(surface.root) else {
            continue;
        };
        if drive.speed == 0.0 {
            continue;
        }
        let rpm = drive.speed * 60.0 / std::f32::consts::TAU;
        surface.offset = (surface.offset
            - terra_world::conveyors::belt_speed(rpm)
                * time.delta_secs().clamp(0.0, 0.1)
                * surface.flow_sign)
            .rem_euclid(1.0);
        let transform = bevy::math::Affine2::from_translation(Vec2::new(0.0, surface.offset));
        match &surface.material {
            SurfaceMaterial::Standard(handle) => {
                if let Some(mut material) = materials.get_mut(handle) {
                    material.uv_transform = transform;
                }
            }
            SurfaceMaterial::Rgb(handle) => {
                if let Some(mut material) = tint_materials.get_mut(handle) {
                    material.base.uv_transform = transform;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "belt_contact_tests.rs"]
mod contact_tests;

#[cfg(test)]
#[path = "belt_inventory_contact_tests.rs"]
mod inventory_contact_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_belts_are_continuous_bounded_shared_solids_instead_of_repeated_frames() {
        for length in [1, 2, 8, 32] {
            for step in [1.0, std::f32::consts::SQRT_2] {
                let mesh = geometry(length, step);
                assert_eq!(mesh.positions.len(), 544);
                let distance = (f32::from(length) + 1.0) * step;
                let mut min = Vec3::splat(f32::INFINITY);
                let mut max = Vec3::splat(f32::NEG_INFINITY);
                for point in &mesh.positions {
                    let point = Vec3::from_array(*point);
                    min = min.min(point);
                    max = max.max(point);
                }
                assert!((min.x + HALF_WIDTH).abs() < 1e-6 && (max.x - HALF_WIDTH).abs() < 1e-6);
                assert!(
                    (min.y + PROFILE_RADIUS).abs() < 1e-6 && (max.y - PROFILE_RADIUS).abs() < 1e-6
                );
                assert!((min.z + PROFILE_RADIUS).abs() < 1e-6);
                assert!((max.z - distance - PROFILE_RADIUS).abs() < 1e-5);
                for triangle in mesh.indices.chunks_exact(3) {
                    let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                        .map(|i| Vec3::from_array(mesh.positions[i as usize]));
                    let normal = Vec3::from_array(mesh.normals[triangle[0] as usize]);
                    assert!((b - a).cross(c - a).dot(normal) > 1e-8);
                }
            }
        }
    }

    #[test]
    fn span_pose_puts_both_pulleys_on_the_authoritative_end_shafts_for_all_profiles() {
        for facing in 0..4 {
            let forward = terra_core::kinetic::forward(facing);
            for horizontal in [-1, 0, 1] {
                for vertical in [-1, 0, 1] {
                    if horizontal == 0 && vertical == 0 {
                        continue;
                    }
                    let step = [forward[0] * horizontal, vertical, forward[2] * horizontal];
                    let start = [-16, 4, -16];
                    let span = Span {
                        controller: std::array::from_fn(|i| start[i] + step[i]),
                        first: std::array::from_fn(|i| start[i] + step[i]),
                        start,
                        end: std::array::from_fn(|i| start[i] + step[i] * 4),
                        step,
                        length: 3,
                        facing,
                        wood: Default::default(),
                        linked: true,
                    };
                    let pose = pose(span);
                    assert!(
                        pose.transform_point(Vec3::ZERO)
                            .abs_diff_eq(Vec3::from_array(start.map(|v| v as f32 + 0.5)), 1e-5)
                    );
                    assert!(
                        pose.transform_point(Vec3::Z * step_length(span) * 4.0)
                            .abs_diff_eq(Vec3::from_array(span.end.map(|v| v as f32 + 0.5)), 1e-5)
                    );
                    assert!(
                        (pose.rotation * Vec3::Y)
                            .abs_diff_eq(Vec3::from_array(span.normal()), 1e-6)
                    );
                }
            }
        }
    }

    #[test]
    fn diagonal_xz_transmission_belts_stand_between_vertical_shafts_without_degenerate_frames() {
        for facing in [4, 5] {
            for x in [-1, 1] {
                for z in [-1, 1] {
                    let start = [-17, -1, -17];
                    let step = [x, 0, z];
                    let span = Span {
                        controller: std::array::from_fn(|i| start[i] + step[i] * 2),
                        first: std::array::from_fn(|i| start[i] + step[i]),
                        start,
                        end: std::array::from_fn(|i| start[i] + step[i] * 3),
                        step,
                        length: 2,
                        facing,
                        wood: Default::default(),
                        linked: true,
                    };
                    let root = pose(span);
                    assert!(root.rotation.is_finite());
                    assert!((root.rotation * Vec3::X).abs().abs_diff_eq(Vec3::Y, 1e-6));
                    assert!(span.normal()[1].abs() < 1e-6);
                    assert!(
                        root.transform_point(Vec3::Z * step_length(span) * 3.0)
                            .abs_diff_eq(Vec3::from_array(span.end.map(|v| v as f32 + 0.5)), 1e-5)
                    );
                    assert!(
                        geometry(span.length, step_length(span))
                            .positions
                            .iter()
                            .all(|p| root.transform_point(Vec3::from_array(*p)).is_finite())
                    );
                }
            }
        }
    }
}
