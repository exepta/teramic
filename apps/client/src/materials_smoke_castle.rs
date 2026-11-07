//! Opt-in native review of the complete authored castle family.
use super::*;
use terra_core::castle;

pub(super) fn camera(y: f32, stage: u8) -> Transform {
    let (eye, target) = match stage {
        1 => ([22.5, y + 11.0, -2.0], [12.5, y + 0.45, 11.0]),
        2 => ([2.5, y + 11.0, 24.0], [12.5, y + 0.45, 11.0]),
        3 => ([18.5, y + 5.4, 13.5], [12.2, y + 1.4, 20.4]),
        4 => ([11.5, y + 8.0, 34.0], [11.5, y + 0.5, 24.5]),
        8 => ([10.5, y + 2.15, 22.0], [10.5, y + 1.45, 28.5]),
        9 => ([16.0, y + 3.6, 23.0], [10.5, y + 1.45, 28.5]),
        10 => ([23.5, y + 8.0, 25.5], [12.5, y + 1.3, 36.5]),
        11 => ([1.0, y + 7.0, 45.0], [12.5, y + 1.3, 36.5]),
        _ => ([14.5, y + 3.0, -9.0], [14.5, y + 0.5, 5.5]),
    };
    Transform::from_translation(Vec3::from_array(eye)).looking_at(Vec3::from_array(target), Vec3::Y)
}

fn placement_samples(y: i64) -> Vec<([i64; 3], [i32; 3], u8)> {
    [
        [0, 1, 0],
        [0, -1, 0],
        [1, 0, 0],
        [-1, 0, 0],
        [0, 0, 1],
        [0, 0, -1],
    ]
    .into_iter()
    .enumerate()
    .map(|(index, normal)| {
        (
            [
                6 + (index % 3) as i64 * 6,
                y + 1,
                34 + (index / 3) as i64 * 4,
            ],
            normal,
            castle::placement_facing(normal.map(|v| v as i8), [0.0, -0.25, 1.0]),
        )
    })
    .collect()
}

fn samples(y: i64) -> Vec<([i64; 3], u16, u8)> {
    let mut result: Vec<_> = castle::BLOCKS
        .into_iter()
        .enumerate()
        .map(|(i, material)| {
            let x = if material == castle::ARCH_RIGHT {
                9
            } else {
                8 + (i % 5) as i64 * 2
            };
            ([x, y, 8 + (i / 5) as i64 * 2], material, 0)
        })
        .collect();
    result.extend([
        ([8, y, 5], castle::BERGFRIED, 0),
        ([10, y, 5], castle::ROYAL_WALL, 0),
        ([18, y, 5], castle::BERGFRIED, 0),
        ([20, y, 5], castle::ROYAL_WALL, 0),
        ([8, y + 1, 20], castle::ARCH_LEFT, 0),
        ([9, y + 1, 20], castle::ARCH_RIGHT, 0),
        ([12, y + 1, 20], castle::ARCH_RIGHT, 4),
        ([13, y + 1, 20], castle::ARCH_LEFT, 4),
        ([16, y + 1, 20], castle::ARROW_SLIT, 9),
        ([18, y + 1, 20], castle::WINDOW, 16),
        ([7, y + 1, 28], castle::WINDOW, 0),
        ([10, y + 1, 28], castle::WINDOW, 2),
        ([13, y + 1, 28], MaterialId::OakWindow as u16, 0),
    ]);
    result.extend(
        placement_samples(y)
            .into_iter()
            .map(|(position, _, facing)| (position, castle::WINDOW, facing)),
    );
    for (index, material) in [
        castle::STONE_WALL,
        castle::CRACKED_WALL,
        castle::CRACKED_WALL,
        castle::OVERGROWN_WALL,
        castle::OVERGROWN_WALL,
        castle::MOSS_WALL,
    ]
    .into_iter()
    .enumerate()
    {
        result.push(([6 + index as i64 * 2, y, 24], material, 0));
    }
    result
}

fn heraldry_registry(y: i64) -> Result<terra_core::kingdom::KingdomRegistry, String> {
    use terra_core::kingdom::{KingdomHeraldry, KingdomIcon, KingdomRegistry, SettlementKind};
    let mut registry = KingdomRegistry::default();
    for (uuid, x, name, style) in [
        (
            61001,
            8,
            "West",
            KingdomHeraldry {
                color: [30, 75, 200],
                icon: KingdomIcon::Star,
            },
        ),
        (
            61002,
            18,
            "East",
            KingdomHeraldry {
                color: [215, 160, 35],
                icon: KingdomIcon::Hammer,
            },
        ),
    ] {
        registry
            .found_settlement(
                terra_core::PlayerUuid::from_u128(uuid),
                name,
                [x, y, 5],
                name,
                SettlementKind::Kingdom,
                style,
            )
            .map_err(|error| format!("Castle realm fixture: {error:?}"))?;
    }
    Ok(registry)
}

fn change_heraldry(world: &mut FixtureWorld, stage: u8) -> Result<(), String> {
    use terra_core::kingdom::{KingdomHeraldry, KingdomIcon};
    let registry = &mut world.journal.gameplay_mut().kingdoms;
    if stage == 6 {
        for (index, style) in [
            KingdomHeraldry {
                color: [190, 35, 55],
                icon: KingdomIcon::Crown,
            },
            KingdomHeraldry {
                color: [30, 145, 70],
                icon: KingdomIcon::Tree,
            },
        ]
        .into_iter()
        .enumerate()
        {
            let record = &registry.kingdoms[index];
            registry
                .change_appearance(record.founder, record.id, style)
                .map_err(|error| format!("Castle live heraldry fixture: {error:?}"))?;
        }
    } else if stage == 7 {
        let record = &registry.kingdoms[0];
        registry
            .dissolve(record.founder, record.id)
            .map_err(|error| format!("Castle live dissolve fixture: {error:?}"))?;
    }
    Ok(())
}

fn cells(y: i64) -> Vec<([i64; 3], u16)> {
    let mut cells = Vec::new();
    for z in 4..=40 {
        for x in 4..=22 {
            cells.push(([x, y - 1, z], MaterialId::SmoothStone as u16));
            for h in 0..=4 {
                cells.push(([x, y + h, z], 0));
            }
        }
    }
    for (position, material, _) in samples(y) {
        cells.iter_mut().find(|(p, _)| *p == position).unwrap().1 = material;
    }
    // A colored backing makes the actual opening visibly readable and gives
    // the independent native picking check a definite target behind each arch.
    for x in [8, 9, 12, 13] {
        cells
            .iter_mut()
            .find(|(p, _)| *p == [x, y + 1, 22])
            .unwrap()
            .1 = castle::DESERT_WALL;
    }
    // The same visible backing behind castle and ordinary glass reveals opacity
    // mistakes without an artificial overlay or a second rendering path.
    for x in [7, 10, 13] {
        for position in [
            [x, y, 28],
            [x - 1, y + 1, 28],
            [x + 1, y + 1, 28],
            [x, y + 2, 28],
        ] {
            cells.iter_mut().find(|(p, _)| *p == position).unwrap().1 = castle::STONE_WALL;
        }
        cells
            .iter_mut()
            .find(|(p, _)| *p == [x, y + 1, 30])
            .unwrap()
            .1 = castle::DESERT_WALL;
    }
    for (position, normal, _) in placement_samples(y) {
        let support = std::array::from_fn(|axis| position[axis] - i64::from(normal[axis]));
        cells.iter_mut().find(|(p, _)| *p == support).unwrap().1 = castle::STONE_WALL;
    }
    cells
}

fn validate(world: &FixtureWorld, y: i64) -> Result<(), String> {
    let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
    for (x, height, expected) in [
        (8.9, 1.45, [8, y + 1, 22]),
        (8.1, 1.45, [8, y + 1, 20]),
        (12.9, 1.55, [12, y + 1, 22]),
        (12.1, 1.55, [12, y + 1, 20]),
    ] {
        let hit = terra_world::interaction_shapes::raycast(
            &world.chunks,
            &world.journal,
            &properties,
            [x, y as f64 + height, 18.0],
            [0.0, 0.0, 1.0],
            6.0,
        )
        .ok_or("Castle gallery ray found no physical stone")?;
        if hit.position != expected {
            return Err(format!(
                "Castle arch opening/pier disagrees with picking: {x}/{height}: {:?}, expected {expected:?}",
                hit.position
            ));
        }
    }
    for (left, right, low, high) in [
        ([8, y + 1, 20], [9, y + 1, 20], 1.20, 1.65),
        ([12, y + 1, 20], [13, y + 1, 20], 1.35, 1.80),
    ] {
        let center = left[0] as f64 + 1.0;
        let mut blocked = false;
        for position in [left, right] {
            terra_world::interaction_shapes::visit_collision_boxes(
                &world.chunks,
                &world.journal,
                &properties,
                position,
                |bounds| {
                    blocked |= bounds.intersects(
                        [center - 0.2, y as f64 + low, 20.0],
                        [center + 0.2, y as f64 + high, 21.0],
                    );
                },
            );
        }
        if blocked {
            return Err("Castle arch has an invisible collider across its opening".into());
        }
    }
    for (position, normal, facing) in placement_samples(y) {
        let support = std::array::from_fn(|axis| position[axis] - i64::from(normal[axis]));
        let actual = terra_world::targeting::placement_facing(
            &world.chunks,
            &world.journal,
            terra_voxel::blocks::BlockHit {
                position: support,
                normal,
                distance: 1.0,
            },
            castle::WINDOW,
            ([0.0; 3], [0.0, -0.25, 1.0]),
        );
        if actual != facing
            || world.journal.object_state(position).map(|s| s.facing) != Some(facing)
        {
            return Err(format!(
                "Castle gallery shared support orientation disagrees: {normal:?}, expected {facing}, got {actual}"
            ));
        }
    }
    for (x, facing) in [(7, 0), (10, 2)] {
        let position = [x, y + 1, 28];
        let origin = castle::point(facing, [0.35, 0.35, -0.5]);
        let end = castle::point(facing, [0.35, 0.35, 0.5]);
        let direction = std::array::from_fn(|axis| end[axis] - origin[axis]);
        let origin = std::array::from_fn(|axis| origin[axis] + position[axis] as f64);
        let hit = terra_world::interaction_shapes::raycast(
            &world.chunks,
            &world.journal,
            &properties,
            origin,
            direction,
            4.0,
        )
        .ok_or("Castle gallery missed the thin window pane")?;
        if hit.position != position || (hit.distance - 0.995).abs() > 0.00001 {
            return Err(format!(
                "Castle gallery window is a full-cell blocker instead of a thin pane: {hit:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn run(
    commands: &mut Commands,
    state: &mut MaterialsSmoke,
    world: &mut FixtureWorld,
    edits: &mut MessageWriter<BlockEditRequest>,
    exit: &mut MessageWriter<AppExit>,
    cameras: &mut Query<&mut Transform, With<TerraCamera>>,
) {
    if state.stage == COMPLETE_STAGE {
        return;
    }
    if timed_out(
        state.started.elapsed(),
        state.capture_started.map(|start| start.elapsed()),
    ) || world.render.error.is_some()
        || world.previews.error.is_some()
    {
        error!(stage=state.stage,render_error=?world.render.error,preview_error=?world.previews.error,"Castle materials gallery failed or timed out");
        state.stage = COMPLETE_STAGE;
        exit.write(AppExit::error());
        return;
    }
    if world.generation.status != GenerationStatus::Ready
        || !world.render.ready
        || world.previews.pending != 0
        || castle::BLOCKS.iter().any(|id| {
            world
                .previews
                .images
                .get(id)
                .is_none_or(|handle| !world.images.contains(handle.id()))
        })
    {
        return;
    }
    let y = state.base_y;
    let scene = cells(y);
    if scene
        .iter()
        .any(|(position, _)| world.chunks.block_at(*position).is_none())
    {
        return;
    }
    if state.stage == 0 {
        match heraldry_registry(y) {
            Ok(registry) => world.journal.gameplay_mut().kingdoms = registry,
            Err(error) => {
                error!(%error, "Castle heraldry fixture failed");
                state.stage = COMPLETE_STAGE;
                exit.write(AppExit::error());
                return;
            }
        }
        for (position, material) in scene {
            let expected_material = world.chunks.block_at(position).unwrap();
            if expected_material != material {
                edits.write(BlockEditRequest {
                    position,
                    expected_material,
                    material,
                });
            }
        }
        state.stage = 1;
        state.since = Instant::now();
        return;
    }
    if state.since.elapsed() < Duration::from_secs(3) {
        return;
    }
    if samples(y).iter().any(|(position, material, _)| {
        let section = global_to_section(*position).0;
        world.chunks.block_at(*position) != Some(*material)
            || world
                .rendered
                .sections
                .get(&section)
                .is_none_or(|rendered| rendered.mesh_epoch != world.journal.mesh_epoch(section))
    }) {
        return;
    }
    if !state.oriented {
        for (position, _, facing) in samples(y) {
            if let Err(error) = terra_world::streaming::commit_object_state(
                &mut world.chunks,
                &mut world.journal,
                position,
                Some(BlockObjectState {
                    facing,
                    ..default()
                }),
            ) {
                error!(%error,"Castle gallery pose failed");
                state.stage = COMPLETE_STAGE;
                exit.write(AppExit::error());
                return;
            }
        }
        state.oriented = true;
        state.capture_started = Some(Instant::now());
        state.since = Instant::now();
        return;
    }
    if !state.captured {
        if let Err(error) = validate(world, y) {
            error!(%error,"Castle native picking/collision check failed");
            state.stage = COMPLETE_STAGE;
            exit.write(AppExit::error());
            return;
        }
        let label = match state.stage {
            1 => "castle-front",
            2 => "castle-back",
            3 => "castle-openings",
            4 => "castle-natural-walls",
            5 => "castle-heraldry-initial",
            6 => "castle-heraldry-updated",
            7 => "castle-heraldry-dissolved",
            8 => "castle-window-comparison",
            9 => "castle-window-oblique",
            10 => "castle-placement-six-faces-front",
            _ => "castle-placement-six-faces-back",
        };
        let folder = std::env::var_os("TERRA_CAPTURE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp/terra-castle-gallery"));
        if let Err(error) = std::fs::create_dir_all(&folder) {
            error!(%error,"Cannot create castle capture directory");
            state.stage = COMPLETE_STAGE;
            exit.write(AppExit::error());
            return;
        }
        let path = folder.join(format!("{label}.png"));
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path.clone()));
        info!(label,path=%path.display(),blocks=castle::BLOCKS.len(),previews=world.previews.images.len(),
            "Castle gallery captured; real front/back models, paired normal/inverted arches and rotated openings; native collision and picking passed");
        state.captured = true;
    }
    if state.since.elapsed() < Duration::from_secs(CAPTURE_INTERVAL_SECONDS) {
        return;
    }
    state.stage += 1;
    state.captured = false;
    state.since = Instant::now();
    if state.stage == 12 {
        state.stage = COMPLETE_STAGE;
        info!(
            "Castle materials gallery passed: twenty models and GPU previews, full-depth continuous arches, natural walls, two actual realms and live heraldry changes/removal, transparent panes beside ordinary glass, six upright placement supports"
        );
        exit.write(AppExit::Success);
        return;
    }
    if let Err(error) = change_heraldry(world, state.stage) {
        error!(%error, "Castle live heraldry fixture failed");
        state.stage = COMPLETE_STAGE;
        exit.write(AppExit::error());
        return;
    }
    for mut transform in cameras.iter_mut() {
        *transform = camera(y as f32, state.stage);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_review_has_real_glass_comparison_and_six_clear_placement_examples() {
        let y = 63;
        let scene = cells(y);
        let samples = samples(y);
        assert!(samples.contains(&([13, y + 1, 28], MaterialId::OakWindow as u16, 0)));
        assert_eq!(placement_samples(y).len(), 6);
        for (position, material, _) in samples {
            assert_eq!(
                scene.iter().find(|(p, _)| *p == position).unwrap().1,
                material
            );
        }
        for (position, normal, facing) in placement_samples(y) {
            let support = std::array::from_fn(|axis| position[axis] - i64::from(normal[axis]));
            assert_eq!(
                scene.iter().find(|(p, _)| *p == support).unwrap().1,
                castle::STONE_WALL
            );
            let up = terra_core::construction::stair_basis(facing)[1];
            assert_eq!(up, if normal[1] < 0 { [0, -1, 0] } else { [0, 1, 0] });
        }
    }

    #[test]
    fn gallery_has_all_castle_identities_and_connected_full_depth_arch_pairs() {
        let samples = samples(63);
        for material in castle::BLOCKS {
            assert!(samples.iter().any(|(_, id, _)| *id == material));
        }
        let unique: std::collections::HashSet<_> = samples.iter().map(|(p, _, _)| p).collect();
        assert_eq!(unique.len(), samples.len());
        for (position, material, facing) in &samples {
            assert!(
                BlockObjectState {
                    facing: *facing,
                    ..default()
                }
                .valid_for(*position, *material)
            );
        }
        assert!(samples.contains(&([8, 64, 20], castle::ARCH_LEFT, 0)));
        assert!(samples.contains(&([9, 64, 20], castle::ARCH_RIGHT, 0)));
        assert!(samples.contains(&([12, 64, 20], castle::ARCH_RIGHT, 4)));
        assert!(samples.contains(&([13, 64, 20], castle::ARCH_LEFT, 4)));
        let registry = heraldry_registry(63).unwrap();
        assert!(registry.valid());
        assert_ne!(
            registry.heraldry_at([10, 63, 5]),
            registry.heraldry_at([20, 63, 5])
        );
        assert!(
            cells(63)
                .iter()
                .all(|(p, _)| p[0] >= 4 && p[0] <= 22 && p[2] >= 4 && p[2] <= 40)
        );
    }
}
