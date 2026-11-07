//! Shared rigid water patches follow real cups; no fluid authority is changed.
use super::*;
use terra_world::streaming::WorldTerrain;

const CUPS: usize = 12;

#[derive(Default)]
pub(super) struct Cache {
    assets: Option<(Handle<Mesh>, Handle<Mesh>, Handle<StandardMaterial>)>,
}

impl Cache {
    pub fn cached_meshes(&self) -> usize {
        usize::from(self.assets.is_some()) * 2
    }
}

#[derive(Component, Default)]
pub(super) struct WheelWater {
    fill: [f32; CUPS],
    drain: [f32; CUPS],
    wet: [bool; CUPS],
    next_sample: f32,
}

#[derive(Component)]
pub(super) struct Patch {
    root: Entity,
    cup: usize,
    drain: bool,
}

fn patch_mesh(drain: bool) -> Mesh {
    let positions = if drain {
        vec![
            [-0.5, 0.0, 0.0],
            [0.5, 0.0, 0.0],
            [0.5, -1.0, 0.0],
            [-0.5, -1.0, 0.0],
        ]
    } else {
        vec![
            [-0.5, 0.0, -0.5],
            [0.5, 0.0, -0.5],
            [0.5, 0.0, 0.5],
            [-0.5, 0.0, 0.5],
        ]
    };
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![
            if drain {
                [0.0, 0.0, 1.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            4
        ],
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    )
    .with_inserted_indices(Indices::U16(vec![0, 2, 1, 0, 3, 2]))
}

pub(super) fn spawn(
    commands: &mut Commands,
    cache: &mut Cache,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    root: Entity,
) {
    let (surface, drain, material) = cache.assets.get_or_insert_with(|| {
        (
            meshes.add(patch_mesh(false)),
            meshes.add(patch_mesh(true)),
            materials.add(StandardMaterial {
                base_color: Color::linear_rgba(0.035, 0.24, 0.27, 0.68),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.18,
                reflectance: 0.5,
                cull_mode: None,
                double_sided: true,
                ..default()
            }),
        )
    });
    commands.entity(root).insert(WheelWater::default());
    for cup in 0..CUPS {
        for (is_drain, mesh) in [(false, &*surface), (true, &*drain)] {
            commands.spawn((
                Name::new(if is_drain {
                    "Wheel scoop draining water"
                } else {
                    "Wheel scoop held water"
                }),
                Patch {
                    root,
                    cup,
                    drain: is_drain,
                },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::default(),
                Visibility::Hidden,
                NotShadowCaster,
                crate::miniature::layers(),
                ChildOf(root),
            ));
        }
    }
}

fn cup_frame(pose: &Transform, phase: f32, cup: usize) -> (Vec3, f32) {
    let angle = phase + cup as f32 * std::f32::consts::TAU / CUPS as f32;
    let outward = pose.rotation * (Quat::from_rotation_x(angle) * Vec3::Y);
    (outward, -outward.y)
}

fn fill_step(fill: f32, wet: bool, upright: f32, dt: f32) -> (f32, f32) {
    let dt = dt.clamp(0.0, 0.1);
    let next = if wet {
        (fill + dt * 5.0).min(1.0)
    } else {
        (fill - dt * (0.65 - upright).max(0.0) * 3.5).max(0.0)
    };
    (
        next,
        if dt > 0.0 {
            (fill - next).max(0.0) / dt
        } else {
            0.0
        },
    )
}

fn submerged(
    chunks: &ChunkStore,
    terrain: &terra_world::TerraGenerator,
    journal: &EditJournal,
    point: Vec3,
) -> bool {
    if !point.is_finite() {
        return false;
    }
    let cell = point.to_array().map(|v| v.floor() as i64);
    let Some(block) = chunks.block_at(cell) else {
        return false;
    };
    let fluid = chunks.fluid_cell_at(terrain, journal, cell, block);
    fluid.amount != 0 && f64::from(point.y) < cell[1] as f64 + fluid.height()
}

fn patch_pose(pose: &Transform, outward: Vec3, fill: f32, drain: f32, is_drain: bool) -> Transform {
    let mut point = pose.translation + outward * 1.40 + Vec3::Y * (0.015 + fill * 0.075);
    let mut axle = pose.rotation * Vec3::X;
    axle.y = 0.0;
    axle = axle.normalize_or_zero();
    if axle.length_squared() < 0.5 {
        axle = Vec3::X;
    }
    let world_rotation = Quat::from_mat3(&Mat3::from_cols(axle, Vec3::Y, axle.cross(Vec3::Y)));
    let scale = if is_drain {
        let spill = Vec3::new(outward.x, 0.0, outward.z).normalize_or_zero();
        point += spill * 0.13;
        Vec3::new(0.56, 0.18 + drain.min(2.0) * 0.24, 1.0)
    } else {
        Vec3::new(
            0.61,
            1.0,
            0.34 * (-outward.y).clamp(0.05, 1.0) * fill.sqrt(),
        )
    };
    Transform::from_translation(pose.compute_affine().inverse().transform_point3(point))
        .with_rotation(pose.rotation.inverse() * world_rotation)
        .with_scale(scale)
}

pub(super) fn animate(
    time: Res<Time>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    terrain: Option<Res<WorldTerrain>>,
    mut wheels: Query<(&Drive, &Transform, &mut WheelWater), Without<Patch>>,
    mut patches: Query<(&Patch, &mut Transform, &mut Visibility)>,
) {
    let dt = time.delta_secs();
    for (drive, pose, mut water) in &mut wheels {
        if time.elapsed_secs() >= water.next_sample {
            water.next_sample = time.elapsed_secs() + 0.1;
            for cup in 0..CUPS {
                let (outward, _) = cup_frame(pose, drive.phase, cup);
                water.wet[cup] = terrain.as_ref().is_some_and(|terrain| {
                    submerged(
                        &chunks,
                        &terrain.0,
                        &journal,
                        pose.translation + outward * 1.44,
                    )
                });
            }
        }
        for cup in 0..CUPS {
            let (_, upright) = cup_frame(pose, drive.phase, cup);
            let (fill, drain) = fill_step(water.fill[cup], water.wet[cup], upright, dt);
            water.fill[cup] = fill;
            water.drain[cup] = drain;
        }
    }
    for (patch, mut transform, mut visibility) in &mut patches {
        let Ok((drive, pose, water)) = wheels.get(patch.root) else {
            continue;
        };
        let (outward, upright) = cup_frame(pose, drive.phase, patch.cup);
        let visible = if patch.drain {
            water.drain[patch.cup] > 0.02
        } else {
            water.fill[patch.cup] > 0.02 && upright > 0.05
        };
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if visible {
            *transform = patch_pose(
                pose,
                outward,
                water.fill[patch.cup],
                water.drain[patch.cup],
                patch.drain,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_cups_fill_hold_and_drain_in_both_rotation_directions() {
        let (full, _) = fill_step(0.0, true, 1.0, 0.1);
        assert!(full > 0.0);
        assert_eq!(fill_step(full, false, 1.0, 0.1), (full, 0.0));
        for phase in [0.4, -0.4] {
            let (_, upright) = cup_frame(&Transform::default(), phase, 0);
            let (remaining, drain) = fill_step(full, false, upright, 0.1);
            assert!(remaining < full && drain > 0.0);
        }
        assert_eq!(fill_step(0.0, false, -1.0, 0.1), (0.0, 0.0));
        assert_eq!(fill_step(full, false, -1.0, 0.0), (full, 0.0));
    }

    #[test]
    fn held_water_stays_level_for_all_horizontal_axes_and_negative_positions() {
        for facing in 0..4 {
            let parent = assembly_pose(
                [-17, 4, -33],
                147,
                BlockObjectState {
                    facing,
                    ..default()
                },
            );
            for phase in [0.0, 1.0, 3.4, 5.0] {
                let (outward, _) = cup_frame(&parent, phase, 2);
                let patch = patch_pose(&parent, outward, 0.8, 0.4, false);
                let normal = parent.rotation * patch.rotation * Vec3::Y;
                assert!(normal.abs_diff_eq(Vec3::Y, 1e-5));
                assert!(patch.is_finite());
                assert!(patch.scale.min_element() > 0.0);
            }
        }
    }

    #[test]
    fn patches_share_two_meshes_and_one_material_and_despawn_with_the_wheel() {
        let mut world = World::new();
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let mut cache = Cache::default();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        let roots: Vec<_> = (0..3)
            .map(|_| {
                let root = commands.spawn_empty().id();
                spawn(&mut commands, &mut cache, &mut meshes, &mut materials, root);
                root
            })
            .collect();
        queue.apply(&mut world);
        assert_eq!(meshes.len(), 2);
        assert_eq!(materials.len(), 1);
        assert_eq!(cache.cached_meshes(), 2);
        assert_eq!(world.query::<&Patch>().iter(&world).count(), 3 * CUPS * 2);
        for root in roots {
            world.despawn(root);
        }
        assert_eq!(world.query::<&Patch>().iter(&world).count(), 0);
        assert_eq!(meshes.len(), 2);
    }

    #[test]
    fn only_loaded_real_fluid_fills_cups_at_negative_section_edges() {
        let terrain = terra_world::TerraGenerator::new(Default::default()).unwrap();
        let mut journal = EditJournal::default();
        let position = [-1, 0, 0];
        journal
            .set_fluid(
                position,
                terra_world::water::FluidCell {
                    amount: 4,
                    ..default()
                },
            )
            .unwrap();
        let mut chunks = ChunkStore::default();
        assert!(!submerged(
            &chunks,
            &terrain,
            &journal,
            Vec3::new(-0.5, 0.25, 0.5)
        ));
        let owner = terra_voxel::global_to_section(position).0;
        chunks.loaded.insert(
            owner,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        assert!(submerged(
            &chunks,
            &terrain,
            &journal,
            Vec3::new(-0.5, 0.25, 0.5)
        ));
        assert!(!submerged(
            &chunks,
            &terrain,
            &journal,
            Vec3::new(-0.5, 0.75, 0.5)
        ));
        journal.set_fluid(position, Default::default()).unwrap();
        assert!(!submerged(
            &chunks,
            &terrain,
            &journal,
            Vec3::new(-0.5, 0.25, 0.5)
        ));
    }
}
