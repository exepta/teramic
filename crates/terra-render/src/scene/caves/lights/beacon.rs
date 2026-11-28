//! The Fresnel block supplies both the original local lamp and a rotating coastal beam.
use super::*;
use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::MeshVertexBufferLayoutRef,
    render::render_resource::{
        AsBindGroup, Face, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};
use terra_player::TerraCamera;
use terra_voxel::MaterialId;

const REACH: f32 = 96.0;
const TURN_SECONDS: f64 = 12.0;
const SLOPE: f32 = 0.075;
const LENS_RADIUS: f32 = 0.24;
const MAX_BEAMS: usize = 2;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct BeamMaterial {
    #[uniform(0)]
    color: Vec4,
}

impl Material for BeamMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_beacon.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &bevy::pbr::MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![
            layout
                .0
                .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])?,
        ];
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

#[derive(Component)]
struct Beam {
    cell: [i64; 3],
    origin: Vec3,
}

#[derive(Default, Resource)]
struct Presentation {
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<BeamMaterial>>,
    next_selection: f64,
}

pub(super) fn install(app: &mut App) {
    crate::material_work::add::<BeamMaterial>(app)
        .init_resource::<Presentation>()
        .add_systems(Update, (select_beams.after(super::select), rotate).chain());
}

fn candidates(
    lamps: &Lamps,
    chunks: &ChunkStore,
    journal: &EditJournal,
    eye: Vec3,
    reach: f32,
) -> Vec<([i64; 3], BlockLight)> {
    let mut lenses: Vec<_> = lamps
        .emitters
        .iter()
        .copied()
        .filter(|(p, _)| {
            Vec3::from_array(p.map(|v| v as f32)).distance_squared(eye) <= reach * reach
                && chunks
                    .block_at(*p)
                    .map(|id| journal.block(*p).unwrap_or(id))
                    == Some(MaterialId::BeaconLens as u16)
        })
        .collect();
    lenses.sort_unstable_by(|(a, _), (b, _)| {
        let dist = |p: [i64; 3]| Vec3::from_array(p.map(|v| v as f32)).distance_squared(eye);
        dist(*a).total_cmp(&dist(*b)).then(a.cmp(b))
    });
    lenses.dedup_by_key(|(cell, _)| *cell);
    lenses.truncate(MAX_BEAMS);
    lenses
}

#[allow(clippy::too_many_arguments)]
fn select_beams(
    mut commands: Commands,
    time: Res<Time>,
    access: Res<WorldAccess>,
    graphics: Option<Res<GraphicsConfig>>,
    lamps: Res<Lamps>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    cameras: Query<&Transform, With<TerraCamera>>,
    beams: Query<(Entity, &Beam)>,
    mut state: ResMut<Presentation>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BeamMaterial>>,
) {
    let Some(camera) = cameras.iter().next().filter(|_| access.ready()) else {
        for (entity, _) in &beams {
            commands.entity(entity).despawn();
        }
        state.next_selection = 0.0;
        return;
    };
    if time.elapsed_secs_f64() < state.next_selection {
        return;
    }
    state.next_selection = time.elapsed_secs_f64() + 0.25;
    let reach = graphics
        .as_ref()
        .map_or(192.0, |g| f32::from(g.render_distance) * 16.0)
        + REACH;
    let selected = candidates(&lamps, &chunks, &journal, camera.translation, reach);
    for (entity, beam) in &beams {
        if !selected.iter().any(|(cell, _)| *cell == beam.cell) {
            commands.entity(entity).despawn();
        }
    }
    for (cell, light) in selected {
        if beams.iter().any(|(_, beam)| beam.cell == cell) {
            continue;
        }
        let mesh = state
            .mesh
            .get_or_insert_with(|| {
                meshes.add(Mesh::from(Cuboid::new(2.0, 2.0, 1.0)).translated_by(Vec3::NEG_Z * 0.5))
            })
            .clone();
        let material = state
            .material
            .get_or_insert_with(|| {
                materials.add(BeamMaterial {
                    color: Vec4::new(1.45, 1.04, 0.57, 1.0),
                })
            })
            .clone();
        let origin = emitter_location(
            &journal,
            cell,
            light,
            Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5),
        );
        commands.spawn((
            Name::new("Rotating Fresnel beacon"),
            Beam { cell, origin },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            SpotLight {
                color: Color::srgb(light.color[0], light.color[1], light.color[2]),
                intensity: 320_000.0,
                radius: LENS_RADIUS,
                inner_angle: 0.035,
                outer_angle: SLOPE.atan(),
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(origin),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

fn direction(elapsed: f64) -> Vec3 {
    let angle = (elapsed.rem_euclid(TURN_SECONDS) / TURN_SECONDS * std::f64::consts::TAU) as f32;
    Vec3::new(angle.cos(), 0.0, angle.sin())
}

fn clear_length(
    cell: [i64; 3],
    origin: Vec3,
    direction: Vec3,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> f32 {
    terra_voxel::blocks::raycast_blocks(
        origin.as_dvec3().to_array(),
        direction.as_dvec3().to_array(),
        REACH as f64,
        |p| {
            // Start in the glass lens itself. Unknown streamed voxels end the beam
            // at the same cell face as a wall; glazing and water let it through.
            Some(u16::from(
                p != cell
                    && block(p).is_none_or(|id| {
                        MaterialId::from_raw(id).is_none_or(MaterialId::is_opaque)
                    }),
            ))
        },
    )
    .map_or(REACH, |hit| hit.distance as f32)
}

fn rotate(
    time: Res<Time>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    mut beams: Query<(&Beam, &mut Transform, &mut SpotLight, &mut Visibility)>,
) {
    if beams.is_empty() {
        return;
    }
    let direction = direction(time.elapsed_secs_f64());
    for (beam, mut transform, mut light, mut visibility) in &mut beams {
        let mut block = chunks.block_sampler();
        let mut effective = |p| block(p).map(|id| journal.block(p).unwrap_or(id));
        if effective(beam.cell) != Some(MaterialId::BeaconLens as u16) {
            *visibility = Visibility::Hidden;
            continue;
        }
        let length = clear_length(beam.cell, beam.origin, direction, effective).max(0.001);
        let radius = LENS_RADIUS + length * SLOPE;
        *transform = Transform::from_translation(beam.origin)
            .looking_to(direction, Vec3::Y)
            .with_scale(Vec3::new(radius, radius, length));
        light.range = length;
        *visibility = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::BlockSection;
    use terra_world::streaming::LoadedSection;

    #[test]
    fn a_turn_clears_glazing_and_stops_at_the_first_wall_or_streaming_boundary() {
        let cell = [-3, 9, 1];
        let origin = Vec3::new(-2.5, 9.53, 1.5);
        let block = |[x, _, z]: [i64; 3]| {
            if z == -8 {
                return None;
            }
            Some(match (x, z) {
                (-3, 1) => MaterialId::BeaconLens,
                (0, 1) => MaterialId::Glass,
                (5, 1) => MaterialId::HarborTimber,
                _ => MaterialId::Air,
            } as u16)
        };
        for turn in [0., 120_000.] {
            let sight = |time| clear_length(cell, origin, direction(turn + time), block);
            assert!((sight(0.) - 7.5).abs() < 0.001);
            assert_eq!(sight(TURN_SECONDS * 0.25), REACH);
            assert_eq!(sight(TURN_SECONDS * 0.5), REACH);
            assert!((sight(TURN_SECONDS * 0.75) - 8.5).abs() < 0.001);
            assert_eq!(sight(TURN_SECONDS), sight(0.));
        }
    }

    #[test]
    fn persisted_and_edited_lenses_share_beam_geometry_and_disappear_on_removal() {
        let lens = MaterialId::BeaconLens as u16;
        let lamp = MaterialId::Lantern as u16;
        let defs: BTreeMap<_, _> = crate::blocks::definitions::BlockLibrary::load(
            &terra_core::asset_directory().join("terra/blocks"),
        )
        .unwrap()
        .blocks
        .into_iter()
        .filter_map(|b| b.light.map(|light| (b.id, light)))
        .collect();
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let mut lamps = Lamps::default();
        let (owner, _) = global_to_section([0, 0, 0]);
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block([2, 5, 2], lens).unwrap();
        data.set_block([3, 5, 2], lamp).unwrap();
        data.set_block([10, 5, 2], lamp).unwrap();
        journal.record([10, 5, 2], lens).unwrap();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        discover(&mut lamps, &chunks, &journal, &defs, Vec3::ZERO, 100.);
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<WorldAccess>()
            .init_resource::<Presentation>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<BeamMaterial>>()
            .insert_resource(chunks)
            .insert_resource(journal)
            .insert_resource(lamps)
            .add_systems(Update, (select_beams, rotate).chain());
        app.world_mut().spawn((TerraCamera, Transform::default()));
        app.update();
        let world = app.world_mut();
        let mut q = world.query::<(&Beam, &Mesh3d, &SpotLight, &Transform)>();
        let mut beams: Vec<_> = q.iter(world).collect();
        beams.sort_by_key(|(b, ..)| b.cell);
        assert_eq!(beams.len(), 2);
        assert_eq!(beams[0].0.cell, [2, 5, 2]);
        assert_eq!(beams[1].0.cell, [10, 5, 2]);
        assert_eq!(beams[0].1.0, beams[1].1.0);
        for (beam, _, spot, transform) in beams {
            assert!(spot.range > 1.0 && !spot.shadow_maps_enabled);
            assert!(transform.forward().abs_diff_eq(Vec3::X, 0.001));
            assert!((transform.translation.y - 5.53).abs() < 0.001);
            assert_eq!(beam.origin, transform.translation);
        }
        world
            .resource_mut::<EditJournal>()
            .record([2, 5, 2], 0)
            .unwrap();
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(3));
        app.update();
        let world = app.world_mut();
        let (_, _, _, remaining) = q.single(world).unwrap();
        assert!(remaining.forward().abs_diff_eq(Vec3::Z, 0.001));
        world.resource_mut::<ChunkStore>().loaded.remove(&owner);
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
        assert_eq!(q.iter(app.world()).count(), 0);
    }
}
