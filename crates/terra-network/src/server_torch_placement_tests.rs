use super::rgb_filter_tests::{apply, fixture, look, place};
use super::*;
use terra_core::{Inventory, ItemKind};
use terra_world::edits::BlockObjectState;

fn held(app: &mut App, peer: &Peer, torch: u16) {
    let mut inventory = Inventory::default();
    inventory.insert(ItemKind::Block(torch), 2);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .insert(peer.uuid, inventory);
}
fn command(app: &App, peer: &Peer, position: [i64; 3], torch: u16) -> EditCommand {
    let journal = app.world().resource::<EditJournal>();
    EditCommand {
        request: 1,
        revision: journal.revision(terra_voxel::global_to_section(position).0),
        look: peer.look,
        source: Some(InventorySource {
            slot: 0,
            revision: journal.gameplay().inventories[&peer.uuid].revision,
        }),
        action: EditAction::Block {
            position,
            expected: 0,
            material: torch,
        },
    }
}

#[test]
fn survival_torches_mount_on_each_wall_and_floor_with_preview_matching_authority() {
    for (anchor, torch) in [
        ([7, 6, 7], 40),
        ([-1, 6, -1], terra_core::block_shapes::VIOLET_TORCH),
    ] {
        for (normal, expected_facing) in [
            ([1, 0, 0], 4),
            ([-1, 0, 0], 5),
            ([0, 0, 1], 6),
            ([0, 0, -1], 7),
            ([0, 1, 0], 0),
        ] {
            let (mut app, mut peer) = fixture();
            place(&mut app, anchor, 3, None);
            place(
                &mut app,
                [12, 6, 12],
                terra_core::kinetic::WOODEN_LARGE_GEAR,
                Some(BlockObjectState::default()),
            );
            assert!(
                app.world()
                    .resource::<EditJournal>()
                    .has_extended_kinetics()
            );
            held(&mut app, &peer, torch);
            let center = Vec3::from_array(anchor.map(|value| value as f32)) + Vec3::splat(0.5);
            let eye = center
                + Vec3::from_array(normal.map(|value| value as f32)) * 4.0
                + if normal[1] != 0 {
                    Vec3::X * 0.5
                } else {
                    Vec3::ZERO
                };
            look(&mut app, &mut peer, eye, center);
            assert!(peer.look[1].abs() <= 1.5);
            let direction =
                Quat::from_euler(EulerRot::YXZ, peer.look[0], peer.look[1], 0.0) * Vec3::NEG_Z;
            let hit = terra_world::targeting::pick_world_for_placement(
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
                app.world()
                    .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
                terra_world::props::PropPose::Rest,
            )
            .0
            .unwrap();
            assert_eq!(hit.position, anchor);
            assert_eq!(hit.normal, normal);
            let position = std::array::from_fn(|axis| anchor[axis] + i64::from(normal[axis]));
            let preview = terra_world::placement::resolve(
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
                hit,
                ItemKind::Block(torch),
                (
                    eye.to_array().map(f64::from),
                    direction.to_array().map(f64::from),
                ),
                peer.look,
                false,
            )
            .unwrap();
            assert_eq!(preview.states[0].1.unwrap().facing, expected_facing);
            let request = command(&app, &peer, position, torch);
            let inventory_revision = request.source.unwrap().revision;
            assert_eq!(apply(&mut app, &peer, request, false), EditOutcome::Applied);
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(position),
                Some(torch)
            );
            assert_eq!(
                journal.object_state(position).unwrap().facing,
                expected_facing
            );
            assert_eq!(
                terra_world::torches::support(position, expected_facing),
                Some(anchor)
            );
            assert!(terra_world::torches::supported(
                app.world().resource::<ChunkStore>(),
                app.world()
                    .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                position,
                expected_facing
            ));
            assert_eq!(
                journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(torch)),
                1
            );
            assert_eq!(
                journal.gameplay().inventories[&peer.uuid].revision,
                inventory_revision + 1
            );
        }
    }
}

#[test]
fn survival_torch_requests_reject_ceiling_and_partial_support_without_consuming_items() {
    for (normal, material) in [
        ([0, -1, 0], 3),
        ([1, 0, 0], 82),
        ([-1, 0, 0], 82),
        ([0, 0, 1], 82),
        ([0, 0, -1], 82),
    ] {
        let (mut app, mut peer) = fixture();
        let anchor = [7, 6, 7];
        place(
            &mut app,
            anchor,
            material,
            (material == 82).then_some(BlockObjectState::default()),
        );
        held(&mut app, &peer, 40);
        let center = Vec3::from_array(anchor.map(|value| value as f32))
            + Vec3::new(0.5, if material == 82 { 0.25 } else { 0.5 }, 0.5);
        let eye = center
            + Vec3::from_array(normal.map(|value| value as f32)) * 4.0
            + if normal[1] != 0 {
                Vec3::X * 0.5
            } else {
                Vec3::ZERO
            };
        look(&mut app, &mut peer, eye, center);
        let position = std::array::from_fn(|axis| anchor[axis] + i64::from(normal[axis]));
        let before =
            app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].clone();
        let request = command(&app, &peer, position, 40);
        assert_eq!(apply(&mut app, &peer, request, false), EditOutcome::Invalid);
        assert_eq!(
            app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid],
            before
        );
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(position),
            Some(0)
        );
    }
}
