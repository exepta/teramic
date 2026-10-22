//! A bounded world-space scattering approximation, not another fullscreen pass.
use super::*;
pub(super) const VERTICES: usize = 9 * 2 * 4;

#[derive(Default)]
pub(super) struct Sunshafts {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<CloudMaterial>>,
}

impl Sunshafts {
    pub(super) fn clear(&mut self, commands: &mut Commands, meshes: &mut Assets<Mesh>) {
        if let Some(entity) = self.entity.take() {
            commands.entity(entity).despawn();
        }
        if let Some(mesh) = self.mesh.take() {
            meshes.remove(mesh.id());
        }
    }

    pub(super) fn rebuild(
        &mut self,
        anchor: [i64; 2],
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<CloudMaterial>,
    ) {
        let mesh = mesh(anchor);
        if let Some(handle) = &self.mesh {
            if let Err(error) = meshes.insert(handle.id(), mesh) {
                tracing::error!(%error, "Unable to replace cloud sunshafts mesh");
                self.clear(commands, meshes);
            }
            return;
        }
        let material = self.material.get_or_insert_with(|| {
            materials.add(ExtendedMaterial {
                base: StandardMaterial {
                    unlit: true,
                    alpha_mode: AlphaMode::Add,
                    cull_mode: None,
                    ..default()
                },
                extension: CloudExtension {
                    weather: Vec4::ZERO,
                    observer: Vec4::ZERO,
                    haze: Vec4::ZERO,
                    lightning: Vec4::ZERO,
                    lightning_tint: Vec4::ONE,
                    sun: Vec4::ZERO,
                },
            })
        });
        let handle = meshes.add(mesh);
        self.entity = Some(
            commands
                .spawn((
                    Name::new("Bounded clearing-cloud sunshafts"),
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    NotShadowCaster,
                    NotShadowReceiver,
                    NoFrustumCulling,
                    Visibility::Hidden,
                ))
                .id(),
        );
        self.mesh = Some(handle);
    }

    pub(super) fn update(
        &self,
        extension: CloudExtension,
        drift: Vec2,
        commands: &mut Commands,
        materials: &mut Assets<CloudMaterial>,
    ) -> bool {
        let active = self.entity.is_some() && extension.sun.w > 0.001 && extension.sun.y > 0.10;
        if let Some(entity) = self.entity {
            commands.entity(entity).insert((
                Transform::from_translation(Vec3::new(drift.x, 0.0, drift.y)),
                if active {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ));
        }
        if let Some(material) = self.material.as_ref().and_then(|h| materials.get_mut(h)) {
            material.into_inner().extension = extension;
        }
        active
    }
}

pub(super) fn clearing_strength(sky: &mut Sky, cover: f32, dt: f32) -> f32 {
    if cover + 0.000_01 < sky.previous_cover {
        sky.clearing = 1.0;
    } else {
        sky.clearing = (sky.clearing - dt * 0.08).max(0.0);
    }
    sky.previous_cover = cover;
    let ramp = |a: f32, b: f32| {
        let t = ((cover - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    sky.clearing * ramp(0.20, 0.45) * (1.0 - ramp(0.78, 0.98))
}

fn mesh(anchor: [i64; 2]) -> Mesh {
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut origins = Vec::new();
    let mut markers = Vec::new();
    let mut indices = Vec::new();
    for gx in anchor[0] - 1..=anchor[0] + 1 {
        for gz in anchor[1] - 1..=anchor[1] + 1 {
            let seed = hash(gx, gz, 0x434c4f5544 ^ 1);
            let origin = Vec2::new(
                gx as f32 * SPACING + random(seed, 0) * 24.0,
                gz as f32 * SPACING + random(seed, 32) * 24.0,
            );
            for axis in [Vec2::X, Vec2::Y] {
                let first = positions.len() as u32;
                for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                    let width = 3.0 + v * 21.0;
                    let offset = axis * ((u * 2.0 - 1.0) * width);
                    positions.push([
                        origin.x + offset.x,
                        MIN_HEIGHT - v * 160.0,
                        origin.y + offset.y,
                    ]);
                    colors.push([u, random(seed, 40), v, 0.0]);
                    origins.push(origin.to_array());
                    markers.push([MIN_HEIGHT, 1.0]);
                }
                indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
            }
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, origins)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, markers)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shafts_only_show_while_clouds_clear_and_fade_afterwards() {
        let mut sky = Sky::default();
        assert_eq!(clearing_strength(&mut sky, 1.0, 0.1), 0.0);
        assert_eq!(clearing_strength(&mut sky, 1.0, 0.1), 0.0);
        assert!(clearing_strength(&mut sky, 0.65, 0.1) > 0.9);
        assert_eq!(clearing_strength(&mut sky, 0.65, 20.0), 0.0);
        assert_eq!(clearing_strength(&mut sky, 0.0, 0.1), 0.0);
    }

    #[test]
    fn shafts_use_one_small_batch_and_valid_indices_at_negative_coordinates() {
        let mesh = mesh([-12, -7]);
        assert_eq!(mesh.count_vertices(), 9 * 2 * 4);
        assert!(
            mesh.indices()
                .unwrap()
                .iter()
                .all(|i| i < mesh.count_vertices())
        );
    }

    #[test]
    fn additive_shafts_premultiply_rgb_without_replacing_background_alpha() {
        let shader = include_str!("../../../../../assets/shaders/terra_clouds.wgsl");
        let branch = shader
            .split("if input.shade.a < 0.5 {")
            .nth(1)
            .and_then(|shaft| shaft.split("return shaft;").next())
            .expect("The clearing-cloud shaft fragment branch must exist");
        let compact: String = branch.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("letalpha=edge*ends*near_fade*far_fade*gap*sun.w*weather.y*0.065;")
        );
        assert!(compact.contains("shaft.color=vec4<f32>(vec3<f32>(1.0,0.90,0.69)*alpha,0.0);"));

        let tint = Vec3::new(1.0, 0.90, 0.69);
        let background = Vec4::new(0.21, 0.30, 0.40, 1.0);
        for visibility in [0.0, 0.001, 0.1, 0.5, 1.0] {
            let alpha = visibility * 0.065;
            let source = (tint * alpha).extend(0.0);
            // Bevy's Add path uses premultiplied blending: One, OneMinusSrcAlpha.
            let blended = source + background * (1.0 - source.w);
            assert_eq!(source.w, 0.0);
            assert_eq!(blended.w, background.w);
            assert!(source.truncate().min_element() >= 0.0);
            assert!(source.truncate().max_element() <= 0.065);
            assert!((blended.truncate() - background.truncate() - tint * alpha).length() < 1e-6);
        }
    }
}
