use super::*;
use terra_core::{ToolKind, farming::FarmingAction};
use terra_world::water::FluidCell;

fn apply(app: &mut App, peer: &mut Peer, command: EditCommand) -> EditOutcome {
    let mut state = SystemState::<ControlWorld>::new(app.world_mut());
    crate::server::farm_tools::apply(
        command,
        peer,
        &mut Default::default(),
        &mut state.get_mut(app.world_mut()).unwrap(),
    )
}

#[test]
fn replicated_source_fills_bucket_and_can_despite_coalesced_and_live_water_mesh_updates() {
    for tool in [ToolKind::Bucket, ToolKind::IronWateringCan] {
        let (mut app, mut peer, _) = fixture(ResourcePropKind::Wheat, false);
        let position = [6, 4, 4];
        let section = global_to_section(position).0;
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block([6, 4, 4], 27)
                .unwrap();
        }
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            journal.record(position, 27).unwrap();
            for i in 0..20 {
                journal
                    .set_fluid(
                        [7, 4, 4],
                        FluidCell {
                            amount: 1 + (i % 7),
                            flow: [1, 0],
                            source: false,
                        },
                    )
                    .unwrap();
            }
        }
        equip_farm_tool(&mut app, &peer, tool);
        let look = aim(&app, &peer, [6.5, 4.5, 4.5]);
        let action = if tool == ToolKind::Bucket {
            FarmingAction::FillBucket
        } else {
            FarmingAction::Refill
        };
        let mut command = farm_command(&app, &peer, position, action, look);
        let mut replica = EditJournal::default();
        let journal = app.world().resource::<EditJournal>();
        ChunkStore::default()
            .apply_confirmed_delta(&mut replica, journal.section_delta(section))
            .unwrap();
        assert_eq!(replica.revision(section), journal.revision(section));
        assert_ne!(
            replica.water_mesh_epoch(section),
            journal.water_mesh_epoch(section),
            "mesh epochs depend on local batching and cannot be transaction versions"
        );
        #[cfg(feature = "client")]
        {
            command.revision = crate::client::farming_revision(&replica, position, action);
        }
        let before =
            app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].clone();
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_fluid(
                [7, 4, 4],
                FluidCell {
                    amount: 2,
                    flow: [1, 0],
                    source: false,
                },
            )
            .unwrap();
        assert_eq!(apply(&mut app, &mut peer, command), EditOutcome::Applied);
        if tool == ToolKind::Bucket {
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(
                journal.gameplay().inventories[&peer.uuid]
                    .count(ItemKind::Tool(ToolKind::WaterBucket)),
                1
            );
            assert_eq!(journal.fluid(position).unwrap().amount, 0);
        } else {
            assert_eq!(peer.farming.water(), tool.watering_capacity());
            let after = &app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid];
            assert!(after.revision > before.revision);
            assert_eq!(
                after.slot(0).unwrap().item,
                ItemKind::Tool(tool)
                    .with_watering_units(tool.watering_capacity())
                    .unwrap()
            );
        }
    }
}

#[test]
fn source_fill_revalidates_actual_water_walls_inventory_and_permissions() {
    for blocked in [
        "source_removed",
        "wall",
        "slot",
        "permission",
        "block_changed",
    ] {
        let (mut app, mut peer, _) = fixture(ResourcePropKind::Wheat, false);
        let position = [6, 4, 4];
        let owner = global_to_section(position).0;
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([6, 4, 4], 27)
            .unwrap();
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record(position, 27)
            .unwrap();
        equip_farm_tool(&mut app, &peer, ToolKind::Bucket);
        let look = aim(&app, &peer, [6.5, 4.5, 4.5]);
        let command = farm_command(&app, &peer, position, FarmingAction::FillBucket, look);
        match blocked {
            "source_removed" => {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_fluid(position, FluidCell::default())
                    .unwrap();
            }
            "wall" => {
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .get_mut(&owner)
                    .unwrap()
                    .data
                    .set_block([5, 5, 6], 3)
                    .unwrap();
            }
            "slot" => peer.held_slot = None,
            "permission" => peer.permissions.build = false,
            "block_changed" => {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .record([8, 4, 4], 3)
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let before =
            app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].clone();
        assert_ne!(
            apply(&mut app, &mut peer, command),
            EditOutcome::Applied,
            "{blocked}"
        );
        assert_eq!(
            app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid],
            before
        );
    }
}

#[test]
fn authoritative_mining_reaches_ground_through_source_water() {
    let (mut app, mut peer, support) = fixture(ResourcePropKind::Wheat, true);
    peer.mode = Some(GameMode::Creative);
    // The fixture has a continuous flat floor and no plants. Aiming at the
    // block centre intersects the nearer floor cell first; aim just above the
    // intended top face so the real ray enters this bed through its source.
    assert!(
        app.world()
            .resource::<ChunkStore>()
            .loaded
            .values()
            .all(|section| section.props.is_empty())
    );
    let look = aim(&app, &peer, [4.5, 4.001, 4.5]);
    let eye =
        app.world().get::<Transform>(peer.body).unwrap().translation + Vec3::Y * PLAYER_EYE_OFFSET;
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let (picked, _) = terra_world::targeting::pick_world_for_mining(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        app.world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
        PropPose::Rest,
    );
    assert_eq!(picked.unwrap().position, support);
    let command = EditCommand {
        request: 1,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(support).0),
        look,
        source: None,
        action: EditAction::Block {
            position: support,
            expected: 3,
            material: 0,
        },
    };
    let mut state = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = state.get_mut(app.world_mut()).unwrap();
    assert_eq!(
        crate::server::validation::apply(command, &peer, &mut world, false),
        EditOutcome::Applied
    );
    assert_eq!(world.chunks.block_at(support), Some(0));
}

#[test]
fn authoritative_submerged_build_uses_the_real_bed_and_rejects_placing_in_the_eye_cell() {
    for material in [3, terra_core::plumbing::IRON_PIPE] {
        let (mut app, mut peer, support) = fixture(ResourcePropKind::Wheat, true);
        peer.mode = Some(GameMode::Creative);
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = Vec3::new(4.5, 6.5 - PLAYER_EYE_OFFSET, 7.5);
        for x in 3..=5 {
            for y in 4..=7 {
                for z in 4..=8 {
                    let p = [x, y, z];
                    let (owner, local) = global_to_section(p);
                    app.world_mut()
                        .resource_mut::<ChunkStore>()
                        .loaded
                        .get_mut(&owner)
                        .unwrap()
                        .data
                        .set_block(local, 27)
                        .unwrap();
                    app.world_mut()
                        .resource_mut::<EditJournal>()
                        .record(p, 27)
                        .unwrap();
                }
            }
        }
        let look = aim(&app, &peer, [4.5, 4.001, 4.5]);
        let eye = app.world().get::<Transform>(peer.body).unwrap().translation
            + Vec3::Y * PLAYER_EYE_OFFSET;
        let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
        let origin = eye.to_array().map(f64::from);
        let direction = direction.to_array().map(f64::from);
        let (picked, _) = terra_world::targeting::pick_world_for_placement(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            app.world()
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
            origin,
            direction,
            PropPose::Rest,
        );
        assert_eq!(picked.unwrap().position, support);
        let target = [4, 4, 4];
        let revision = app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(target).0);
        let command = EditCommand {
            request: 1,
            revision,
            look,
            source: None,
            action: EditAction::Block {
                position: target,
                expected: 27,
                material,
            },
        };
        let mut state = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = state.get_mut(app.world_mut()).unwrap();
        let eye_cell = origin.map(|v| v.floor() as i64);
        let forged = EditCommand {
            action: EditAction::Block {
                position: eye_cell,
                expected: 27,
                material,
            },
            ..command
        };
        assert_ne!(
            crate::server::validation::check(forged, &peer, &world),
            EditOutcome::Applied
        );
        assert_eq!(
            crate::server::validation::apply(command, &peer, &mut world, false),
            EditOutcome::Applied,
            "material={material}"
        );
        assert_eq!(world.chunks.block_at(target), Some(material));
        assert_eq!(world.chunks.block_at(eye_cell), Some(27));
    }
}
