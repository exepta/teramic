use super::*;
use std::time::{Duration, Instant};
use terra_core::creatures::{
    Creature, CreatureBehavior, CreatureKind, CreatureNeeds, taming::CompanionView,
};

fn app(owned: bool) -> App {
    let mut app = App::new();
    let owner = terra_core::PlayerUuid::from_u128(120);
    let mut session = ClientSession::default();
    session.ready = true;
    session.uuid = Some(owner);
    session.spawn = Vec3::new(-17.5, 3.0, -17.5);
    let mut input = terra_player::CameraInputState::default();
    input.captured = true;
    let animal = Creature {
        id: (1 << 62) + 120,
        kind: CreatureKind::Cow,
        home: [-1, -1],
        position: [-17.5, 2.0, -20.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: CreatureKind::Cow.health() * 0.6,
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Cow),
    };
    let id = animal.id;
    let mut creatures = ReplicatedCreatures::default();
    creatures.creatures.insert(
        id,
        terra_network::client::CreatureTrack {
            previous: animal.clone(),
            current: animal,
            received: Instant::now(),
            playback: Default::default(),
            interval: 0.1,
        },
    );
    creatures.bonds.insert(
        id,
        CompanionView {
            id,
            owner: owned.then_some(owner),
            player: owner,
            trust: if owned { 100 } else { 75 },
            mode: CompanionMode::Wait,
        },
    );
    app.insert_resource(session)
        .insert_resource(creatures)
        .insert_resource(input)
        .insert_resource(terra_player::TargetCreature(Some(id)))
        .init_resource::<terra_player::PlayerInputBlock>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<Time<Real>>()
        .init_resource::<PetMenuState>()
        .init_resource::<PetMenuView>()
        .init_resource::<PetCommands>()
        .add_message::<PetMenuAction>()
        .add_message::<terra_player::CaptureRequest>();
    app
}

fn open(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    opening_frame(app.world_mut());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    update(app.world_mut());
}

#[test]
fn sneak_place_opens_only_owned_pets_and_blocks_the_same_opening_frame() {
    let mut owner = app(true);
    open(&mut owner);
    assert!(owner.world().resource::<PetMenuView>().visible);
    assert!(owner.world().resource::<PetMenuView>().rest);
    assert_eq!(owner.world().resource::<PetMenuView>().hearts.len(), 10);
    assert!(owner.world().resource::<terra_player::PlayerInputBlock>().0);
    assert!(
        owner
            .world_mut()
            .resource_mut::<Messages<terra_player::CaptureRequest>>()
            .drain()
            .any(|m| !m.0)
    );
    let mut stranger = app(false);
    open(&mut stranger);
    assert!(!stranger.world().resource::<PetMenuView>().visible);
    let mut normal = app(true);
    normal
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    opening_frame(normal.world_mut());
    assert!(
        normal.world().resource::<PetMenuState>().id.is_none(),
        "ordinary feeding/mounting is not intercepted"
    );
}

#[test]
fn selecting_the_active_mode_is_an_explicit_ui_no_op_without_a_network_request() {
    for mode in [
        CompanionMode::Follow,
        CompanionMode::Wait,
        CompanionMode::None,
    ] {
        let mut app = app(true);
        let id = app
            .world()
            .resource::<terra_player::TargetCreature>()
            .0
            .unwrap();
        app.world_mut()
            .resource_mut::<ReplicatedCreatures>()
            .bonds
            .get_mut(&id)
            .unwrap()
            .mode = mode;
        open(&mut app);
        app.world_mut().write_message(PetMenuAction::Select(mode));
        update(app.world_mut());
        assert!(!app.world().resource::<PetMenuView>().visible);
        assert!(app.world().resource::<PetCommands>().0.is_empty());
        assert!(app.world().resource::<PetMenuState>().pending.is_none());
        assert!(app.world().resource::<PetMenuState>().resume_pending);
        assert_eq!(
            app.world().resource::<ReplicatedCreatures>().bonds[&id].mode,
            mode
        );
    }
}

#[test]
fn pet_command_waits_for_replication_and_escape_never_opens_pause_behind_the_overlay() {
    let mut app = app(true);
    open(&mut app);
    let id = app.world().resource::<PetMenuView>().id;
    app.world_mut()
        .write_message(PetMenuAction::Select(CompanionMode::Follow));
    update(app.world_mut());
    assert_eq!(
        app.world().resource::<PetCommands>().0.front(),
        Some(&(id, CompanionMode::Follow))
    );
    assert!(app.world().resource::<PetMenuView>().pending);
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_millis(200));
    update(app.world_mut());
    assert!(
        app.world().resource::<PetMenuView>().visible,
        "the request is not a fabricated authority acknowledgement"
    );
    app.world_mut()
        .resource_mut::<ReplicatedCreatures>()
        .bonds
        .get_mut(&id)
        .unwrap()
        .mode = CompanionMode::Follow;
    update(app.world_mut());
    assert!(!app.world().resource::<PetMenuView>().visible);
    assert!(app.world().resource::<PetMenuState>().resume_pending);
    opening_frame(app.world_mut());
    open(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    update(app.world_mut());
    assert!(app.world().resource::<PetMenuState>().consumed_escape);
    assert!(!app.world().resource::<PetMenuView>().visible);
}

#[test]
fn stale_owner_and_timeout_never_claim_a_successful_pet_command() {
    let mut app = app(true);
    open(&mut app);
    app.world_mut()
        .write_message(PetMenuAction::Select(CompanionMode::Follow));
    update(app.world_mut());
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_secs(4));
    update(app.world_mut());
    assert!(app.world().resource::<PetMenuView>().visible);
    assert!(!app.world().resource::<PetMenuView>().pending);
    assert!(!app.world().resource::<PetMenuView>().error.is_empty());
    let id = app.world().resource::<PetMenuView>().id;
    app.world_mut()
        .resource_mut::<ReplicatedCreatures>()
        .bonds
        .get_mut(&id)
        .unwrap()
        .owner = None;
    update(app.world_mut());
    assert!(!app.world().resource::<PetMenuView>().visible);
    tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
        "../../../../src-ui/components/pet-menu/pet-menu.component.css"
    ))
    .unwrap();
}

#[test]
fn radial_modifier_uses_the_existing_rebound_gameplay_binding() {
    let mut app = app(true);
    let mut controls = terra_core::ControlsConfig::default();
    controls
        .rebind(
            GameAction::FlyDown,
            terra_core::InputBinding::Key(KeyCode::KeyV),
        )
        .unwrap();
    app.insert_resource(controls);
    open(&mut app);
    assert!(!app.world().resource::<PetMenuView>().visible);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyV);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    opening_frame(app.world_mut());
    update(app.world_mut());
    assert!(app.world().resource::<PetMenuView>().visible);
}

#[test]
fn closing_releases_the_input_block_before_requesting_capture_and_waits_for_key_release() {
    let mut app = app(true);
    open(&mut app);
    app.world_mut()
        .resource_mut::<terra_player::CameraInputState>()
        .captured = false;
    app.world_mut()
        .resource_mut::<Messages<terra_player::CaptureRequest>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    update(app.world_mut());
    opening_frame(app.world_mut());
    assert!(app.world().resource::<PetMenuState>().resume_pending);
    assert!(app.world().resource::<terra_player::PlayerInputBlock>().0);
    assert!(
        !app.world_mut()
            .resource_mut::<Messages<terra_player::CaptureRequest>>()
            .drain()
            .any(|m| m.0)
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    opening_frame(app.world_mut());
    assert!(!app.world().resource::<PetMenuState>().resume_pending);
    assert!(!app.world().resource::<terra_player::PlayerInputBlock>().0);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<terra_player::CaptureRequest>>()
            .drain()
            .any(|m| m.0)
    );
}

#[test]
fn reference_sector_hover_is_exclusive_and_does_not_change_authority_or_create_assets() {
    let mut app = app(true);
    open(&mut app);
    let id = app.world().resource::<PetMenuView>().id;
    let frame = app.world().resource::<PetMenuView>().art_style.clone();
    for (mode, expected) in [
        (CompanionMode::Follow, [true, false, false]),
        (CompanionMode::Wait, [false, true, false]),
        (CompanionMode::None, [false, false, true]),
    ] {
        app.world_mut().write_message(PetMenuAction::Hover(mode));
        update(app.world_mut());
        let view = app.world().resource::<PetMenuView>();
        assert_eq!([view.follow, view.rest, view.free], expected);
        assert_eq!(view.art_style, frame);
        assert_eq!(view.hearts.len(), 10);
        assert_eq!(
            app.world().resource::<ReplicatedCreatures>().bonds[&id].mode,
            CompanionMode::Wait
        );
        assert!(app.world().resource::<PetCommands>().0.is_empty());
    }
}

#[test]
fn compact_nameplate_keeps_the_species_readable_and_the_full_variant_in_its_tooltip() {
    let mut app = app(true);
    let id = app
        .world()
        .resource::<terra_player::TargetCreature>()
        .0
        .unwrap();
    let mut window = Window::default();
    window.resolution.set(640.0, 480.0);
    app.world_mut().spawn((window, bevy::window::PrimaryWindow));
    app.insert_resource(UiScale(0.64));
    {
        let mut pets = app.world_mut().resource_mut::<ReplicatedCreatures>();
        let animal = &mut pets.creatures.get_mut(&id).unwrap().current;
        animal.kind = CreatureKind::Horse;
        animal.variant = 9;
    }
    open(&mut app);
    let view = app.world().resource::<PetMenuView>();
    assert!(view.compact);
    assert_eq!(view.title, "Horse");
    assert!(view.health.starts_with("Chestnut Horse with Blaze:"));
    assert!(view.follow_info_style.contains("height: 64.0px"));
    assert!(view.rest_info_style.contains("height: 88.0px"));
}
