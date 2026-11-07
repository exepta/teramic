use super::{
    rgb_filter_tests::{apply, fixture, look, place},
    *,
};
use terra_core::{ItemKind, ItemStack};
use terra_world::edits::BlockObjectState;

fn held(app: &mut App, peer: &Peer, material: u16) {
    assert!(
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap()
            .creative_set(
                0,
                ItemStack {
                    item: ItemKind::Block(material),
                    count: 3
                },
            )
    );
}

#[test]
fn player_snow_plates_stack_eight_times_with_exact_consumption_and_saved_player_height() {
    let (mut app, mut peer) = fixture();
    let position = [-1, 4, -1];
    place(&mut app, [-1, 3, -1], 3, None);
    held(&mut app, &peer, 98);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .get_mut(&peer.uuid)
        .unwrap()
        .creative_set(
            0,
            ItemStack {
                item: ItemKind::Block(98),
                count: 8,
            },
        );
    for layers in 1..=8 {
        look(
            &mut app,
            &mut peer,
            Vec3::new(-0.5, 8.0, 0.3),
            Vec3::new(-0.5, 4.01, -0.5),
        );
        let request = command(&app, &peer, position, if layers == 1 { 0 } else { 98 }, 98);
        assert_eq!(
            apply(&mut app, &peer, request, false),
            EditOutcome::Applied,
            "layer {layers}"
        );
        let material = if layers == 8 { 5 } else { 98 };
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(position),
            Some(material)
        );
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(
            journal.object_state(position).map(|state| state.facing),
            terra_core::construction::manual_snow_facing(layers)
        );
        assert_eq!(
            journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(98)),
            8 - u16::from(layers)
        );
        let mut restored = EditJournal::default();
        restored
            .import_delta(journal.section_delta(terra_voxel::global_to_section(position).0))
            .unwrap();
        assert_eq!(
            restored.object_state(position),
            journal.object_state(position)
        );
        assert!(journal.gameplay().valid());
    }
}

#[test]
fn mining_a_player_snow_stack_returns_its_real_plate_count_and_clears_pose_metadata() {
    for layers in 1..=7 {
        let (mut app, mut peer) = fixture();
        let position = [-1, 4, -1];
        place(&mut app, [-1, 3, -1], 3, None);
        place(
            &mut app,
            position,
            98,
            Some(BlockObjectState {
                facing: terra_core::construction::manual_snow_facing(layers).unwrap(),
                ..default()
            }),
        );
        look(
            &mut app,
            &mut peer,
            Vec3::new(-0.5, 8.0, 0.3),
            Vec3::new(-0.5, 4.05, -0.5),
        );
        let request = command(&app, &peer, position, 98, 0);
        assert_eq!(apply(&mut app, &peer, request, true), EditOutcome::Applied);
        let journal = app.world().resource::<EditJournal>();
        assert!(journal.object_state(position).is_none());
        assert_eq!(journal.gameplay().drops.len(), 1);
        assert_eq!(
            journal.gameplay().drops[0].stack,
            ItemStack {
                item: ItemKind::Block(98),
                count: u16::from(layers)
            }
        );
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(position),
            Some(0)
        );
    }
}

#[test]
fn plate_coating_uses_the_actual_shape_and_survival_item_without_overwriting_its_pose() {
    for material in [
        74, 78, 82, 86, 90, 94, 101, 102, 104, 105, 106, 136, 137, 138,
    ] {
        let (mut app, mut peer) = fixture();
        let position = [-1, 4, -1];
        let state =
            terra_core::construction::is_shape(material).then_some(BlockObjectState::default());
        place(&mut app, position, material, state);
        held(&mut app, &peer, 98);
        look(
            &mut app,
            &mut peer,
            Vec3::new(-0.5, 8.0, 0.3),
            Vec3::new(-0.5, 4.05, -0.5),
        );
        let request = command(&app, &peer, position, material, 98);
        assert_eq!(
            apply(&mut app, &peer, request, false),
            EditOutcome::Applied,
            "material {material}"
        );
        let chunks = app.world().resource::<ChunkStore>();
        assert_eq!(chunks.block_at(position), Some(material));
        assert_eq!(chunks.block_at([-1, 6, -1]), Some(0));
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.object_state(position), state);
        assert_eq!(journal.surface_condition(position).snow, 1.0);
        assert_eq!(
            journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(98)),
            2
        );
        assert!(journal.gameplay().drops.is_empty());
        let inventory = journal.gameplay().inventories[&peer.uuid].clone();
        let request = command(&app, &peer, position, material, 98);
        assert_ne!(apply(&mut app, &peer, request, false), EditOutcome::Applied);
        assert_eq!(
            app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid],
            inventory
        );
    }
}

#[test]
fn plate_coating_rejects_stale_inventory_wrong_source_and_occlusion_atomically() {
    for fault in [
        "revision",
        "inventory",
        "source",
        "wall",
        "permission",
        "body",
    ] {
        let (mut app, mut peer) = fixture();
        let position = [-1, 4, -1];
        let material = if fault == "body" { 0 } else { 78 };
        place(&mut app, [-1, 3, -1], 3, None);
        if material != 0 {
            place(&mut app, position, material, Some(Default::default()));
        }
        held(&mut app, &peer, if fault == "source" { 3 } else { 98 });
        look(
            &mut app,
            &mut peer,
            Vec3::new(-0.5, 8.0, 0.3),
            Vec3::new(-0.5, 4.05, -0.5),
        );
        if fault == "wall" {
            place(&mut app, [-1, 5, -1], 3, None);
        }
        if fault == "permission" {
            peer.permissions.build = false;
        }
        if fault == "body" {
            app.world_mut().spawn((
                Transform::from_xyz(-0.5, 4.5, -0.5),
                ServerCharacter {
                    link: Entity::PLACEHOLDER,
                },
            ));
        }
        let mut request = command(&app, &peer, position, material, 98);
        if fault == "revision" {
            request.revision += 1;
        }
        if fault == "inventory" {
            request.source.as_mut().unwrap().revision += 1;
        }
        let journal = app.world().resource::<EditJournal>();
        let delta = journal.section_delta(terra_voxel::global_to_section(position).0);
        let inventory = journal.gameplay().inventories[&peer.uuid].clone();
        assert_ne!(
            apply(&mut app, &peer, request, false),
            EditOutcome::Applied,
            "{fault}"
        );
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.section_delta(delta.position), delta);
        assert_eq!(journal.gameplay().inventories[&peer.uuid], inventory);
    }
}

fn command(
    app: &App,
    peer: &Peer,
    position: [i64; 3],
    expected: u16,
    material: u16,
) -> EditCommand {
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
            expected,
            material,
        },
    }
}

#[test]
fn survival_replaces_every_incomplete_snow_layer_and_consumes_only_the_new_building_item() {
    for facing in 0..=7 {
        let (mut app, mut peer) = fixture();
        let position = [-1, 4, -1];
        place(&mut app, [-1, 3, -1], 3, None);
        place(
            &mut app,
            position,
            98,
            (facing > 0).then_some(BlockObjectState {
                facing,
                ..default()
            }),
        );
        held(&mut app, &peer, 3);
        look(
            &mut app,
            &mut peer,
            Vec3::new(-0.5, 8.0, 0.3),
            Vec3::new(-0.5, 4.05, -0.5),
        );
        let request = command(&app, &peer, position, 98, 3);
        assert_eq!(
            apply(&mut app, &peer, request, false),
            EditOutcome::Applied,
            "layer {facing}"
        );
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(position),
            Some(3)
        );
        let journal = app.world().resource::<EditJournal>();
        assert!(journal.object_state(position).is_none());
        assert_eq!(
            journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(3)),
            2
        );
        assert!(journal.gameplay().drops.is_empty());
        assert!(journal.gameplay().valid());
    }
}

#[test]
fn replacing_snow_never_bypasses_full_blocks_occlusion_or_revision_checks() {
    for fault in ["full", "occluded", "stale"] {
        let (mut app, mut peer) = fixture();
        let position = [-1, 4, -1];
        let material = if fault == "full" { 5 } else { 98 };
        place(&mut app, position, material, None);
        held(&mut app, &peer, 3);
        look(
            &mut app,
            &mut peer,
            Vec3::new(-0.5, 8.0, 0.3),
            Vec3::new(-0.5, 4.05, -0.5),
        );
        if fault == "occluded" {
            place(&mut app, [-1, 5, -1], 3, None);
        }
        let mut request = command(&app, &peer, position, material, 3);
        if fault == "stale" {
            request.revision += 1;
        }
        let journal = app.world().resource::<EditJournal>();
        let before = journal.section_delta(terra_voxel::global_to_section(position).0);
        let inventory = journal.gameplay().inventories[&peer.uuid].clone();
        assert_ne!(
            apply(&mut app, &peer, request, false),
            EditOutcome::Applied,
            "{fault}"
        );
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(
            journal.section_delta(terra_voxel::global_to_section(position).0),
            before
        );
        assert_eq!(journal.gameplay().inventories[&peer.uuid], inventory);
    }
}

#[test]
fn player_snow_blocks_stack_immediately_without_weather_time_or_natural_accumulation() {
    let (mut app, mut peer) = fixture();
    place(&mut app, [-1, 4, -1], 5, None);
    held(&mut app, &peer, 5);
    for y in [5, 6] {
        look(
            &mut app,
            &mut peer,
            Vec3::new(-0.5, 9.0, 0.3),
            Vec3::new(-0.5, y as f32 - 0.05, -0.5),
        );
        let request = command(&app, &peer, [-1, y, -1], 0, 5);
        assert_eq!(apply(&mut app, &peer, request, false), EditOutcome::Applied);
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at([-1, y, -1]),
            Some(5)
        );
    }
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(
        journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(5)),
        1
    );
    assert_eq!(journal.surface_condition_count(), 0);
}

#[test]
fn colored_bed_replaces_both_thin_snow_caps_without_losing_its_color_or_peer() {
    let (mut app, mut peer) = fixture();
    let anchor = [-1, 4, -1];
    let second = [-1, 4, -2];
    for position in [anchor, second] {
        place(&mut app, [position[0], 3, position[2]], 3, None);
        place(&mut app, position, 98, None);
    }
    let color = terra_core::beds::DyeColor::Red;
    let item = ItemKind::DyedBed {
        material: 71,
        color,
    };
    assert!(
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap()
            .creative_set(0, ItemStack { item, count: 1 })
    );
    look(
        &mut app,
        &mut peer,
        Vec3::new(-0.5, 8.0, 2.5),
        Vec3::new(-0.5, 4.05, -0.5),
    );
    let request = command(&app, &peer, anchor, 98, 71);
    assert_eq!(apply(&mut app, &peer, request, false), EditOutcome::Applied);
    let journal = app.world().resource::<EditJournal>();
    for (position, other) in [(anchor, second), (second, anchor)] {
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(position),
            Some(71)
        );
        let state = journal.object_state(position).unwrap();
        assert_eq!(state.peer, Some(other));
        assert_eq!(terra_core::beds::bed_color(71, state.facing), color);
    }
    assert_eq!(journal.gameplay().inventories[&peer.uuid].count(item), 0);
    assert!(journal.gameplay().valid());
}
