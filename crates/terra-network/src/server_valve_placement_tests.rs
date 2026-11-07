use super::*;

#[test]
fn survival_valve_placement_uses_the_authoritative_clicked_face_in_all_six_directions() {
    for facing in 0..6 {
        let (mut app, mut peer) = fixture();
        let support = [7, 6, 7];
        let normal = terra_core::kinetic::axis(facing);
        let position = std::array::from_fn(|axis| support[axis] + normal[axis]);
        place(&mut app, support, 3, None);
        held(&mut app, &mut peer, ItemKind::Block(VALVE), 2);
        let center = Vec3::from_array(support.map(|v| v as f32)) + Vec3::splat(0.5);
        let outward = Vec3::from_array(normal.map(|v| v as f32));
        // The real camera clamps pitch to ±1.5 radians. A slightly oblique
        // vertical ray still targets the top/bottom face without sending an
        // impossible ±π/2 look that authority correctly rejects.
        let eye = center
            + outward * 4.0
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
        .expect("the legal player look must hit the real support");
        assert_eq!(hit.position, support);
        assert_eq!(hit.normal, normal.map(|value| value as i32));
        let before = bag(&app, &peer);
        let command = EditCommand {
            request: 1,
            revision: app
                .world()
                .resource::<EditJournal>()
                .revision(terra_voxel::global_to_section(position).0),
            look: peer.look,
            source: Some(InventorySource {
                slot: 0,
                revision: before.revision,
            }),
            action: EditAction::Block {
                position,
                expected: 0,
                material: VALVE,
            },
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let result = validation::apply(
            command,
            &peer,
            &mut system.get_mut(app.world_mut()).unwrap(),
            false,
        );
        assert_eq!(result, EditOutcome::Applied, "facing {facing}");
        let chunks = app.world().resource::<ChunkStore>();
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(chunks.block_at(support), Some(3));
        assert_eq!(chunks.block_at(position), Some(VALVE));
        let state = journal.object_state(position).unwrap();
        assert_eq!(
            state.facing, facing,
            "server must not replace the clicked face with yaw"
        );
        assert!(state.valid_for(position, VALVE));
        assert_eq!(
            ports(VALVE, state.facing),
            3 << (normal.iter().position(|v| *v != 0).unwrap() * 2)
        );
        assert_eq!(bag(&app, &peer).count(ItemKind::Block(VALVE)), 1);
        assert_eq!(bag(&app, &peer).revision, before.revision + 1);
        assert_eq!(journal.plumbing_at(position).unwrap().water.amount, 0);
    }
}
