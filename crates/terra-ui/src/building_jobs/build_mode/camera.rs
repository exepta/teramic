use super::*;
use bevy::input::mouse::AccumulatedMouseScroll;
use terra_network::client::ReplicatedCreatures;
use terra_world::streaming::ChunkStore;

pub(super) fn update(world: &mut World) {
    if !modal(world) {
        let keys = world.get_resource::<ButtonInput<KeyCode>>();
        let direction = keys.map_or(Vec3::ZERO, |keys| {
            Vec3::new(
                f32::from(u8::from(keys.pressed(KeyCode::KeyD)))
                    - f32::from(u8::from(keys.pressed(KeyCode::KeyA))),
                0.0,
                f32::from(u8::from(keys.pressed(KeyCode::KeyS)))
                    - f32::from(u8::from(keys.pressed(KeyCode::KeyW))),
            )
        });
        let rotate = keys.is_some_and(|keys| keys.just_pressed(KeyCode::KeyR));
        let dt = world
            .get_resource::<Time>()
            .map_or(0.0, |time| time.delta_secs().min(0.1));
        let camera = world.resource::<ConstructionCamera>();
        let center = camera.center + direction.normalize_or_zero() * dt * 20.0;
        let kingdom = world.resource::<State>().kingdom;
        let can_pan = super::super::stock::build_claim(
            world.resource::<ClientSession>(),
            center.to_array().map(|p| f64::from(p).floor() as i64),
        ) == kingdom;
        let pointer = world
            .query::<&Window>()
            .iter(world)
            .next()
            .and_then(Window::physical_cursor_position);
        let zoom = if pointer.is_some_and(|point| over_ui(world, point)) {
            0.0
        } else {
            world
                .get_resource::<AccumulatedMouseScroll>()
                .map_or(0.0, |scroll| scroll.delta.y)
        };
        let mut camera = world.resource_mut::<ConstructionCamera>();
        if can_pan {
            camera.center = center;
        }
        camera.height = (camera.height - zoom * 3.0).clamp(18.0, 72.0);
        if rotate {
            let mut state = world.resource_mut::<State>();
            state.facing = (state.facing + 1) % 4;
        }
    }
    let pose = world.resource::<ConstructionCamera>().pose();
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = pose;
    }
    let center = world
        .resource::<ConstructionCamera>()
        .center
        .to_array()
        .map(f64::from);
    if let Some(mut focus) = world.get_resource_mut::<terra_world::streaming::ChunkFocus>() {
        focus.0 = center;
    }
}

pub(super) fn over_ui(world: &mut World, point: Vec2) -> bool {
    world
        .query::<(
            &tilt_ui::ElementId,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&Visibility>,
            Option<&InheritedVisibility>,
        )>()
        .iter(world)
        .any(|(id, node, transform, visibility, inherited)| {
            matches!(
                id.0.as_str(),
                "build-mode-panel"
                    | "build-mode-help"
                    | "build-mode-toolbar"
                    | "build-mode-costs"
                    | "mini-map"
                    | "kingdom-resources"
                    | "work-area-overlay"
            ) && !node.is_empty()
                // Hidden UI keeps its layout. Its old rectangle must not
                // swallow world clicks or zoom, including hidden ancestors.
                && visibility != Some(&Visibility::Hidden)
                && inherited.is_none_or(|visibility| visibility.get())
                && node.contains_point(*transform, point)
        })
}

pub(super) fn ray(world: &mut World) -> Option<Ray3d> {
    let cursor = world
        .query::<&Window>()
        .iter(world)
        .next()?
        .cursor_position()?;
    let mut query =
        world.query_filtered::<(&Camera, &Transform), With<terra_player::TerraCamera>>();
    let (camera, pose) = query.iter(world).next()?;
    camera
        .viewport_to_world(&GlobalTransform::from(*pose), cursor)
        .ok()
}

fn ground(world: &World, ray: Ray3d) -> Option<terra_voxel::blocks::BlockHit> {
    let chunks = world.get_resource::<ChunkStore>()?;
    let journal = world.get_resource::<terra_world::edits::EditJournal>();
    let sky = f64::from(world.resource::<ConstructionCamera>().center.y) + 4.0;
    let mut entered = false;
    terra_voxel::blocks::raycast_blocks(
        ray.origin.to_array().map(f64::from),
        ray.direction.to_array().map(f64::from),
        128.0,
        |position| {
            let block = journal
                .and_then(|journal| journal.block(position))
                .or_else(|| chunks.block_at(position));
            if let Some(block) = block {
                entered = true;
                Some(block)
            } else if !entered && position[1] as f64 > sky {
                Some(0)
            } else {
                None
            }
        },
    )
}

fn npc(world: &World, ray: Ray3d, limit: f64) -> Option<u64> {
    let creatures = world.get_resource::<ReplicatedCreatures>()?;
    let session = world.get_resource::<ClientSession>()?;
    let kingdom = world.resource::<State>().kingdom;
    creatures
        .creatures
        .values()
        .filter_map(|track| {
            let creature = &track.current;
            if creature.kind != terra_core::creatures::CreatureKind::Villager
                || creature.health <= 0.0
                || !terra_core::creatures::villagers::is_civilian(creature.variant)
                || !session
                    .work
                    .workers
                    .iter()
                    .any(|worker| worker.npc == creature.id)
                || super::super::stock::build_claim(
                    session,
                    creature.position.map(|p| p.floor() as i64),
                ) != kingdom
            {
                return None;
            }
            terra_world::creatures::ray_distance(
                creature,
                ray.origin.to_array().map(f64::from),
                ray.direction.to_array().map(f64::from),
                limit,
            )
            .map(|distance| (distance, creature.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

pub(super) fn pick(world: &mut World) {
    let physical = world
        .query::<&Window>()
        .iter(world)
        .next()
        .and_then(Window::physical_cursor_position);
    let Some(physical) = physical else {
        world.resource_mut::<State>().origin = None;
        return;
    };
    if over_ui(world, physical) {
        return;
    }
    let Some(ray) = ray(world) else {
        return;
    };
    let hit = ground(world, ray);
    if let Some(mut target) = world.get_resource_mut::<terra_player::TargetBlock>() {
        target.0 = hit;
    }
    let left = world
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|mouse| mouse.just_pressed(MouseButton::Left));
    let right = world
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|mouse| mouse.just_pressed(MouseButton::Right));
    let selecting = world
        .resource::<crate::npc_work::WorkUiState>()
        .selection_active()
        || world
            .resource::<super::super::BuildingUiState>()
            .selection
            .is_some();
    if selecting {
        world.resource_mut::<State>().origin = None;
        return;
    }
    if right {
        world.resource_mut::<State>().placing = false;
    }
    if left
        && !world.resource::<State>().placing
        && let Some(npc) = npc(world, ray, hit.map_or(128.0, |hit| hit.distance))
    {
        crate::npc_work::open_managed(world, npc);
        return;
    }
    let origin = hit.and_then(|hit| {
        if hit.normal[1] <= 0 {
            return None;
        }
        let position = [
            hit.position[0],
            hit.position[1].checked_add(1)?,
            hit.position[2],
        ];
        let session = world.resource::<ClientSession>();
        (super::super::stock::build_claim(session, position) == world.resource::<State>().kingdom)
            .then_some(position)
    });
    world.resource_mut::<State>().origin = origin;
    if left {
        confirm(world);
    }
}

pub(super) fn can_place(state: &State) -> bool {
    state.placing
        && state.pending.is_none()
        && state.origin.is_some()
        && state.kingdom.is_some()
        && state
            .preview
            .as_ref()
            .is_some_and(|preview| preview.id == state.template)
}

pub(super) fn confirm(world: &mut World) {
    if !world.resource::<ConstructionCamera>().active || !playing(world) || modal(world) {
        return;
    }
    let state = world.resource::<State>();
    if !can_place(state) {
        return;
    }
    let (Some(origin), Some(kingdom)) = (state.origin, state.kingdom) else {
        return;
    };
    let action = BuildingAction::QueueHouse {
        kingdom,
        origin,
        facing: state.facing,
        template: state.template.clone(),
    };
    if let Some(request) = world
        .resource_mut::<ClientSession>()
        .make_building_request(action)
    {
        let id = request.0.request;
        world.write_message(request);
        let time = now(world);
        let mut state = world.resource_mut::<State>();
        state.pending = Some((id, time));
        state.reloading = false;
        state.error.clear();
        state.feedback = "Bauauftrag wird angelegt …".into();
    }
}
