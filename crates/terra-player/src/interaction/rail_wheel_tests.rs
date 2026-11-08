use super::*;
use terra_core::{
    WoodSpecies,
    rails::{self, Piece},
};
use terra_voxel::{BlockSection, global_to_section};
use terra_world::streaming::LoadedSection;

fn block(chunks: &mut ChunkStore, cell: [i64; 3], material: u16) {
    let (owner, local) = global_to_section(cell);
    chunks
        .loaded
        .entry(owner)
        .or_insert_with(|| LoadedSection {
            data: BlockSection::empty(owner).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        })
        .data
        .set_block(local, material)
        .unwrap();
}
fn fixture() -> (App, BlockHit) {
    let mut chunks = ChunkStore::default();
    for x in -4..=12 {
        for z in -4..=14 {
            block(&mut chunks, [x, 0, z], 3);
        }
    }
    let mut journal = EditJournal::default();
    for piece in [
        Piece::new([0, 1, 0], STRAIGHT, 0).unwrap(),
        Piece::new([4, 1, 8], STRAIGHT, 1).unwrap(),
    ] {
        let plan = terra_world::rails::plan(
            &chunks,
            &journal,
            piece.anchor,
            piece.material,
            piece.encoded_facing(),
        )
        .unwrap();
        assert!(terra_world::rails::commit(&mut chunks, &mut journal, &plan).unwrap());
    }
    let eye = Vec3::new(2.25, 2.6, 6.25);
    let point = Vec3::new(1.5, 1.28, 3.875);
    let camera = Transform::from_translation(eye).looking_at(point, Vec3::Y);
    let mut look = crate::CameraLook::default();
    look.sync_external(camera.rotation);
    let hit = BlockHit {
        position: [1, 1, 3],
        normal: [0, 1, 0],
        distance: f64::from(eye.distance(point)),
    };
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<crate::PlayerInputBlock>()
        .insert_resource(crate::CameraInputState {
            captured: true,
            ..Default::default()
        })
        .init_resource::<PlacementPreview>()
        .init_resource::<RailBuildContext>()
        .init_resource::<BlockPalette>()
        .insert_resource(ActiveItem(Some(ItemKind::for_wood(
            STRAIGHT,
            WoodSpecies::Birch,
        ))))
        .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))))
        .insert_resource(chunks)
        .insert_resource(journal)
        .add_message::<MouseWheel>()
        .add_systems(
            Update,
            (prepare_intent, update)
                .chain()
                .in_set(crate::PlayerSet::RailWheel),
        );
    app.world_mut().spawn((crate::TerraCamera, camera, look));
    (app, hit)
}
fn wheel(app: &mut App) {
    app.world_mut().write_message(MouseWheel {
        unit: bevy::input::mouse::MouseScrollUnit::Line,
        x: 0.0,
        y: -1.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
}

#[test]
fn actual_fresh_dock_scrolls_only_two_distinct_real_geometries_and_resets_on_new_aim() {
    let (mut app, _) = fixture();
    app.update();
    let context = app.world().resource::<RailBuildContext>();
    assert!(context.active);
    assert!(
        context.captures_wheel,
        "fixture needs actual straight and curve layouts: {:?}",
        context.choices
    );
    assert_eq!(context.selected, BuildChoice::Automatic);
    let alternate = context.choices[1].choice;
    wheel(&mut app);
    app.update();
    assert_eq!(
        app.world().resource::<RailBuildContext>().selected,
        alternate
    );
    app.update();
    assert_eq!(
        app.world().resource::<RailBuildContext>().selected,
        alternate,
        "drained wheel cannot repeat"
    );
    app.world_mut().resource_mut::<TargetWorld>().0 = None;
    wheel(&mut app);
    app.update();
    let context = app.world().resource::<RailBuildContext>();
    assert!(!context.active && !context.captures_wheel);
    assert!(context.choices.is_empty() && context.hit.is_none());
    assert_eq!(context.selected, BuildChoice::Automatic);
}

#[test]
fn missing_alternative_duplicate_geometry_floor_and_blocked_input_leave_wheel_unowned() {
    let (mut app, hit) = fixture();
    app.update();
    let item = app.world().resource::<ActiveItem>().0.unwrap();
    let choices = app.world().resource::<RailBuildContext>().choices.clone();
    assert_eq!(choices.len(), 2);
    let mut context = RailBuildContext::default();
    context.set_choices(hit, item, vec![choices[0].clone()], -1.0, true);
    assert!(context.active && !context.captures_wheel);
    let mut duplicate = choices[0].clone();
    duplicate.choice = choices[1].choice;
    context.set_choices(hit, item, vec![choices[0].clone(), duplicate], -1.0, true);
    assert!(!context.captures_wheel);
    context.set_choices(hit, item, choices.clone(), -1.0, false);
    assert!(context.active && !context.captures_wheel);
    assert_eq!(
        context.selected,
        BuildChoice::Automatic,
        "floor ramps keep automatic intent"
    );
    let mut missing = choices[1].clone();
    missing.layout = None;
    context.set_choices(hit, item, vec![choices[0].clone(), missing], -1.0, true);
    assert!(
        !context.captures_wheel,
        "a label/error is not a second geometric choice"
    );
    app.world_mut().resource_mut::<crate::PlayerInputBlock>().0 = true;
    wheel(&mut app);
    app.update();
    assert!(!app.world().resource::<RailBuildContext>().active);
    app.world_mut().resource_mut::<crate::PlayerInputBlock>().0 = false;
    app.world_mut().resource_mut::<ActiveItem>().0 = Some(ItemKind::Block(rails::SWITCH_LEFT));
    app.update();
    assert!(!app.world().resource::<RailBuildContext>().captures_wheel);
}

#[test]
fn fresh_wood_item_ignores_previous_items_mode_or_explicit_start_in_the_same_frame() {
    for explicit_start in [false, true] {
        let (mut app, _) = fixture();
        let fresh = app.world().resource::<ActiveItem>().0.unwrap();
        {
            let mut preview = app.world_mut().resource_mut::<PlacementPreview>();
            preview.rail_item = Some(ItemKind::for_wood(STRAIGHT, WoodSpecies::Oak));
            if explicit_start {
                preview.rail_start = Some(rails::Endpoint {
                    anchor: [0, 1, 0],
                    port: 1,
                });
            } else {
                preview.rail_mode = 2;
            }
        }
        app.update();
        let context = app.world().resource::<RailBuildContext>();
        assert!(
            context.active && context.captures_wheel,
            "old item intent cannot suppress fresh wood Track context"
        );
        assert_eq!(context.item, Some(fresh));
        assert_eq!(context.selected, BuildChoice::Automatic);
        {
            let mut preview = app.world_mut().resource_mut::<PlacementPreview>();
            assert_eq!(preview.rail_item, Some(fresh));
            assert_eq!(preview.rail_mode, 0);
            assert!(preview.rail_start.is_none());
            if explicit_start {
                preview.rail_start = Some(rails::Endpoint {
                    anchor: [0, 1, 0],
                    port: 1,
                });
            } else {
                preview.rail_mode = 2;
            }
        }
        app.update();
        assert!(
            !app.world().resource::<RailBuildContext>().active,
            "the same item's explicit intent must still retain priority"
        );
    }
}

#[test]
fn a_fresh_same_frame_target_runs_before_context_routing_and_navigation_retargets_before_interaction()
 {
    use crate::PlayerSet;
    #[derive(Resource, Default)]
    struct Observed(bool);
    fn aim(mut target: ResMut<TargetWorld>) {
        target.0 = None;
    }
    fn navigate(mut context: ResMut<RailBuildContext>) {
        context.mark_hotbar_changed();
    }
    fn interact(target: Res<TargetWorld>, chunks: Res<ChunkStore>, mut observed: ResMut<Observed>) {
        observed.0 = matches!(target.0, Some(WorldTarget::Block(hit)) if chunks.block_at(hit.position).is_some_and(rails::is_rail));
    }
    let (mut app, _) = fixture();
    app.configure_sets(
        Update,
        (
            PlayerSet::Targeting,
            PlayerSet::RailWheel,
            PlayerSet::WheelSelection,
            PlayerSet::Retargeting,
            PlayerSet::Interaction,
        )
            .chain(),
    );
    // Fresh targeting invalidates the previous frame's rail aim. Navigation
    // requests a real same-frame target refresh before interaction consumes it.
    app.add_systems(Update, aim.in_set(PlayerSet::Targeting))
        .add_systems(Update, navigate.in_set(PlayerSet::WheelSelection))
        .add_systems(
            Update,
            (super::super::target_block, super::super::target_creature)
                .chain()
                .run_if(retarget_needed)
                .in_set(PlayerSet::Retargeting),
        )
        .add_systems(Update, interact.in_set(PlayerSet::Interaction))
        .init_resource::<super::super::TargetBlock>()
        .init_resource::<super::super::TargetCreature>()
        .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .init_resource::<Observed>();
    app.update();
    assert!(app.world().resource::<Observed>().0);
    assert!(!app.world().resource::<RailBuildContext>().active);
}

#[test]
fn r_selects_a_real_curve_before_wheel_routing_and_preserves_it_until_intent_changes() {
    let (mut app, _) = fixture();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    wheel(&mut app);
    app.update();
    let preview = app.world().resource::<PlacementPreview>();
    assert_eq!(preview.rail_mode, 1);
    let context = app.world().resource::<RailBuildContext>();
    assert!(context.active && context.captures_wheel);
    assert_eq!(
        context.selected,
        BuildChoice::Curve,
        "R wins simultaneous scrolling"
    );
    assert!(
        context
            .choices
            .iter()
            .any(|choice| choice.choice == BuildChoice::Curve && choice.layout.is_some())
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.update();
    assert_eq!(
        app.world().resource::<RailBuildContext>().selected,
        BuildChoice::Curve
    );
    // Wheel can still return to the actual Straight proposal; the visible form
    // and subsequent R cycle follow that choice rather than stale curve intent.
    wheel(&mut app);
    app.update();
    assert_eq!(
        app.world().resource::<RailBuildContext>().selected,
        BuildChoice::Straight
    );
    assert_eq!(app.world().resource::<PlacementPreview>().rail_mode, 0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    assert_eq!(
        app.world().resource::<RailBuildContext>().selected,
        BuildChoice::Curve
    );
}

#[test]
fn r_cycles_without_a_target_but_blocked_input_and_new_items_do_not_retain_a_dock() {
    let (mut app, _) = fixture();
    app.world_mut().resource_mut::<TargetWorld>().0 = None;
    for mode in [1, 2, 0] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        assert_eq!(app.world().resource::<PlacementPreview>().rail_mode, mode);
        assert!(!app.world().resource::<RailBuildContext>().active);
    }
    app.world_mut().resource_mut::<crate::PlayerInputBlock>().0 = true;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    assert_eq!(app.world().resource::<PlacementPreview>().rail_mode, 0);
    assert!(!app.world().resource::<RailBuildContext>().active);
    app.world_mut().resource_mut::<crate::PlayerInputBlock>().0 = false;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut().resource_mut::<ActiveItem>().0 = Some(ItemKind::Block(3));
    app.update();
    assert!(
        app.world()
            .resource::<PlacementPreview>()
            .rail_item
            .is_none()
    );
    assert!(
        app.world()
            .resource::<RailBuildContext>()
            .choices
            .is_empty()
    );
}
