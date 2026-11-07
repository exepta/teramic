use super::rgb_filter_tests::{apply, fixture, look, place};
use super::*;
use terra_core::{
    Inventory, ItemKind,
    lantern::{CHAIN, LANTERN},
};

#[test]
fn all_six_lantern_mounts_match_prediction_and_authority_in_survival() {
    for (normal, facing) in [
        ([1, 0, 0], 4),
        ([-1, 0, 0], 5),
        ([0, 0, 1], 6),
        ([0, 0, -1], 7),
        ([0, 1, 0], 0),
        ([0, -1, 0], 8),
    ] {
        let (mut app, mut peer) = fixture();
        let anchor = [7, 6, 7];
        place(&mut app, anchor, 3, None);
        let mut inventory = Inventory::default();
        inventory.insert(ItemKind::Block(LANTERN), 2);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .insert(peer.uuid, inventory);
        let center = Vec3::from_array(anchor.map(|v| v as f32)) + Vec3::splat(0.5);
        let eye = center
            + Vec3::from_array(normal.map(|v| v as f32)) * 4.
            + if normal[1] != 0 {
                Vec3::X * 0.5
            } else {
                Vec3::ZERO
            };
        look(&mut app, &mut peer, eye, center);
        let direction =
            Quat::from_euler(EulerRot::YXZ, peer.look[0], peer.look[1], 0.) * Vec3::NEG_Z;
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
        assert_eq!(hit.normal, normal);
        let position = std::array::from_fn(|i| anchor[i] + i64::from(normal[i]));
        let preview = terra_world::placement::resolve(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            hit,
            ItemKind::Block(LANTERN),
            (
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
            ),
            peer.look,
            false,
        )
        .unwrap();
        assert_eq!(preview.states[0].1.unwrap().facing, facing);
        let journal = app.world().resource::<EditJournal>();
        let request = EditCommand {
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
                material: LANTERN,
            },
        };
        assert_eq!(apply(&mut app, &peer, request, false), EditOutcome::Applied);
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(position),
            Some(LANTERN)
        );
        assert_eq!(journal.object_state(position).unwrap().facing, facing);
        assert_eq!(
            journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(LANTERN)),
            1
        );
    }
}

#[test]
fn chain_placement_cannot_forge_a_floor_mount_or_consume_an_item_on_rejection() {
    let (mut app, mut peer) = fixture();
    let anchor = [7, 6, 7];
    place(&mut app, anchor, 3, None);
    let mut inventory = Inventory::default();
    inventory.insert(ItemKind::Block(CHAIN), 2);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .insert(peer.uuid, inventory);
    let center = Vec3::new(7.5, 6.5, 7.5);
    look(&mut app, &mut peer, center + Vec3::new(0.5, 4., 0.), center);
    let p = [7, 7, 7];
    let journal = app.world().resource::<EditJournal>();
    let command = EditCommand {
        request: 1,
        revision: journal.revision(terra_voxel::global_to_section(p).0),
        look: peer.look,
        source: Some(InventorySource {
            slot: 0,
            revision: journal.gameplay().inventories[&peer.uuid].revision,
        }),
        action: EditAction::Block {
            position: p,
            expected: 0,
            material: CHAIN,
        },
    };
    assert_eq!(apply(&mut app, &peer, command, false), EditOutcome::Invalid);
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
            .count(ItemKind::Block(CHAIN)),
        2
    );
    assert_eq!(app.world().resource::<ChunkStore>().block_at(p), Some(0));
}
