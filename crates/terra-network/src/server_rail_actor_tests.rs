//! The builder may occupy a new rail, while other bodies retain protection.
use super::*;
use terra_core::WoodSpecies;

#[derive(Clone, Copy, Debug)]
enum Shape {
    Seed,
    Straight,
    Curve,
    Ramp,
}

fn fixture_with_actor_inside(shape: Shape) -> (App, Peer, Piece, EditCommand) {
    let (mut app, peer, old, source_command) = rail_fixture(rails::STRAIGHT, [8, 4, 3]);
    let connecting = !matches!(shape, Shape::Seed);
    if connecting {
        assert_eq!(
            run(&mut app, &peer, source_command, false),
            EditOutcome::Applied
        );
    }
    for x in 4..=18 {
        for z in -12..=8 {
            set_block(&mut app, [x, 3, z], 3);
            for y in 4..=7 {
                let cell = [x, y, z];
                if app.world().resource::<EditJournal>().block(cell).is_none() {
                    set_block(&mut app, cell, 0);
                }
            }
        }
    }
    let floor = match shape {
        Shape::Seed => [8, 3, 0],
        Shape::Curve => [14, 3, -8],
        Shape::Straight => [8, 3, -8],
        Shape::Ramp => [8, 4, -8],
    };
    set_block(&mut app, floor, 3);
    let point = Vec3::new(
        floor[0] as f32 + 0.5,
        (floor[1] + 1) as f32,
        floor[2] as f32 + if connecting { 0.5 } else { 0.9 },
    );
    let start = rails::Endpoint {
        anchor: old.anchor,
        port: 1,
    };
    let actor = if connecting {
        let candidate = terra_world::rails::proposal_connection(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            start,
            terra_voxel::blocks::BlockHit {
                position: floor,
                normal: [0, 1, 0],
                distance: 1.0,
            },
            [0.0; 2],
            WoodSpecies::Oak,
        )
        .unwrap();
        let endpoint = terra_world::rails::next_endpoint(&candidate).unwrap();
        let state = candidate
            .states
            .iter()
            .find(|(cell, _)| *cell == endpoint.anchor)
            .unwrap()
            .1
            .unwrap();
        let piece =
            terra_world::rails::piece_from_state(endpoint.anchor, rails::STRAIGHT, state).unwrap();
        let mid = piece.paths()[0].sample(0.5).position;
        // A path midpoint can lie between sleepers. Stand on the nearest
        // actual new wooden beam, then recompute the real target ray below.
        let distance = |beam: &rails::Beam| {
            (beam.center[0] - mid[0]).powi(2) + (beam.center[2] - mid[2]).powi(2)
        };
        let sleeper = piece
            .geometry()
            .into_iter()
            .filter(|beam| beam.material == rails::PartMaterial::Wood)
            .min_by(|first, second| distance(first).total_cmp(&distance(second)))
            .expect("a genuine new sleeper is required");
        Vec3::new(
            sleeper.center[0] as f32,
            4.0 + PLAYER_HALF_HEIGHT,
            sleeper.center[2] as f32,
        )
    } else {
        Vec3::new(8.5, 4.0 + PLAYER_HALF_HEIGHT, 0.5)
    };
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = actor;
    let eye = actor + Vec3::Y * PLAYER_EYE_OFFSET;
    let look = aim(&app, &peer, point.to_array().map(f64::from));
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let (_, target) = terra_world::targeting::pick_world_for_rails(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        app.world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
        PropPose::Rest,
    );
    let Some(WorldTarget::Block(hit)) = target else {
        panic!("{shape:?}: real floor ray required")
    };
    assert_eq!(hit.position, floor, "{shape:?}");
    let ray = (
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
    );
    let plan = if connecting {
        terra_world::rails::connection_aim(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            start,
            hit,
            ray,
            look,
            WoodSpecies::Oak,
        )
    } else {
        terra_world::rails::seed(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            hit,
            ray,
            look,
            WoodSpecies::Oak,
        )
    }
    .unwrap();
    let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
    assert!(
        terra_world::placement::boxes(
            &plan,
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            app.world()
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        )
        .iter()
        .any(|bounds| bounds.intersects(
            (actor - half).to_array().map(f64::from),
            (actor + half).to_array().map(f64::from)
        )),
        "{shape:?}: actor must really overlap authored rail geometry"
    );
    let endpoint = terra_world::rails::next_endpoint(&plan).unwrap();
    let state = plan
        .states
        .iter()
        .find(|(cell, _)| *cell == endpoint.anchor)
        .unwrap()
        .1
        .unwrap();
    let piece =
        terra_world::rails::piece_from_state(endpoint.anchor, rails::STRAIGHT, state).unwrap();
    let cost = terra_world::rails::cost(&plan).unwrap();
    let source = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let inventory = journal
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap();
        let wanted = ItemStack {
            item: ItemKind::Block(rails::STRAIGHT),
            count: cost + 1,
        };
        if inventory.slot(0) != Some(wanted) {
            assert!(inventory.creative_set(0, wanted));
        }
        assert_eq!(inventory.slot(0), Some(wanted));
        InventorySource {
            slot: 0,
            revision: inventory.revision,
        }
    };
    let action = if connecting {
        EditAction::StretchRail {
            start,
            position: hit.position,
            expected: 3,
            signature: terra_world::rails::connection_signature(&plan, start),
        }
    } else {
        EditAction::SeedRail {
            position: hit.position,
            expected: 3,
            signature: terra_world::rails::seed_signature(&plan),
        }
    };
    let command = EditCommand {
        request: 3,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(hit.position).0),
        look,
        source: Some(source),
        action,
    };
    (app, peer, piece, command)
}

#[test]
fn a_builder_inside_real_seed_straight_curve_and_ramp_can_build_but_another_identical_body_blocks()
{
    for shape in [Shape::Seed, Shape::Straight, Shape::Curve, Shape::Ramp] {
        let (mut app, peer, piece, command) = fixture_with_actor_inside(shape);
        let pose = *app.world().get::<Transform>(peer.body).unwrap();
        let other = app
            .world_mut()
            .spawn((
                pose,
                crate::server::ServerCharacter {
                    link: Entity::PLACEHOLDER,
                },
            ))
            .id();
        let before = snapshot(&app, piece);
        assert_eq!(
            run(&mut app, &peer, command, false),
            EditOutcome::Invalid,
            "{shape:?}: other player at exactly the same position remains protected"
        );
        assert_eq!(snapshot(&app, piece), before);
        app.world_mut().despawn(other);
        assert_eq!(
            run(&mut app, &peer, command, false),
            EditOutcome::Applied,
            "{shape:?}"
        );
        let journal = app.world().resource::<EditJournal>();
        assert!(terra_world::rails::complete(journal, piece));
        assert_eq!(
            journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(rails::STRAIGHT)),
            1
        );
        assert_eq!(
            app.world().get::<Transform>(peer.body).unwrap().translation,
            pose.translation,
            "rail placement does not teleport the actor"
        );
    }
}
