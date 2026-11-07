//! Two-click rail connections share the authority's geometry and clearance plan.
use super::*;

pub(super) fn update(
    keys: &GameplayInput,
    target: &EditTarget,
    chunks: &ChunkStore,
    requests: &mut EditRequests,
    presentation: &mut EditPresentation,
    item: ItemKind,
    creative: bool,
) {
    use terra_core::{kingdom::KingdomPermission, rails};
    if keys.pressed(Action::Break) {
        return;
    }
    if keys.pressed(Action::FlyDown) && keys.just_pressed(Action::Place) {
        presentation.preview.rail_start = None;
        return;
    }
    let hit = match target.world.0 {
        Some(WorldTarget::Block(hit)) => Some(hit),
        Some(WorldTarget::Prop(_)) => target.block.0,
        None => None,
    };
    let Some(hit) = hit else { return };
    let Some(journal) = target.journal.as_deref() else {
        return;
    };
    let Some((camera, look)) = target.camera.iter().next() else {
        return;
    };
    let origin = if target
        .perspective
        .as_deref()
        .is_some_and(|mode| mode.is_third_person())
    {
        target
            .bodies
            .iter()
            .next()
            .map_or(camera.translation, |body| {
                body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET
            })
    } else {
        camera.translation
    };
    let rotation = look.rotation();
    let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
    let ray = (
        origin.to_array().map(f64::from),
        (rotation * Vec3::NEG_Z).to_array().map(f64::from),
    );
    let wood = item.wood_species().unwrap_or_default();
    let held_material = item.block_material().unwrap_or(rails::STRAIGHT);
    let fixed_piece = matches!(
        held_material,
        rails::SWITCH_LEFT | rails::SWITCH_RIGHT | rails::CROSSING | rails::BUFFER
    );
    let start = presentation.preview.rail_start;
    if start.is_none()
        && !fixed_piece
        && presentation.preview.rail_mode == 0
        && chunks.block_at(hit.position).is_some_and(rails::is_rail)
    {
        if keys.just_pressed(Action::Place) && hit.distance <= 4.5 {
            presentation.preview.rail_start =
                terra_world::rails::select_endpoint(journal, hit, ray);
            if presentation.preview.rail_start.is_none() {
                presentation.preview.rail_error =
                    Some("Kein freies Gleisende in Reichweite".into());
            }
        }
        return;
    }
    let material = if fixed_piece {
        held_material
    } else {
        match presentation.preview.rail_mode {
            1 => rails::CROSSING,
            2 => rails::BUFFER,
            _ => rails::STRAIGHT,
        }
    };
    let valid_plan = if let Some(start) = start {
        terra_world::rails::connection(chunks, journal, start, hit, [yaw, pitch], wood)
    } else {
        terra_world::rails::placement(chunks, journal, hit, material, ray, [yaw, pitch])
    };
    let environment_valid = valid_plan.is_ok();
    let environment_error = valid_plan.as_ref().err().cloned();
    let plan = valid_plan.or_else(|_| {
        if let Some(start) = start {
            terra_world::rails::proposal_connection(chunks, journal, start, hit, [yaw, pitch], wood)
        } else {
            terra_world::rails::proposal(chunks, journal, hit, material, ray, [yaw, pitch])
        }
    });
    let mut plan = match plan {
        Ok(plan) => plan,
        Err(error) => {
            presentation.preview.rail_error = Some(error);
            return;
        }
    };
    terra_world::rails::set_wood(&mut plan, wood);
    let Some(anchor) = plan.states.iter().find_map(|(cell, state)| {
        state
            .is_some_and(|state| !state.secondary && state.peer == Some(*cell))
            .then_some(*cell)
    }) else {
        return;
    };
    let cost = terra_world::rails::cost(&plan).unwrap_or(u16::MAX);
    let existing_sites = if let Some(start) = start {
        terra_world::rails::connection_sites(journal, start, hit)
    } else {
        terra_world::rails::placement_sites(journal, hit)
    };
    let existing_sites_loaded = existing_sites.as_ref().is_some_and(|sites| {
        sites.iter().all(|cell| {
            chunks.block_at(*cell).is_some() && chunks.block_at(*cell) == journal.block(*cell)
        })
    });
    let endpoint_reachable = start.is_none()
        || terra_world::rails::connection_target(&plan).is_some_and(|port| {
            let endpoint = port.position.position();
            (0..3)
                .map(|axis| (endpoint[axis] - ray.0[axis]).powi(2))
                .sum::<f64>()
                <= 4.5 * 4.5
        });
    let has_item = creative
        || target
            .session
            .as_ref()
            .is_some_and(|session| session.inventory.count(item) >= cost);
    let clearance = terra_world::rails::clearance(&plan, chunks, journal);
    let foreground_clear = match target.world.0 {
        Some(WorldTarget::Prop(hit)) => clearance.as_ref().is_ok_and(|props| {
            props.iter().any(|cleared| {
                cleared.prop.id == hit.prop.id && cleared.prop.section == hit.prop.section
            })
        }),
        _ => true,
    };
    let permitted = target.session.as_ref().is_none_or(|session| {
        session.uuid.is_some_and(|actor| {
            let allowed = |permission, cell| {
                terra_world::kingdom_access::can_use(
                    &session.kingdoms,
                    chunks,
                    actor,
                    permission,
                    cell,
                )
            };
            plan.edits
                .iter()
                .all(|edit| allowed(KingdomPermission::Build, edit.position))
                && existing_sites.as_ref().is_some_and(|sites| {
                    sites
                        .iter()
                        .all(|cell| allowed(KingdomPermission::Build, *cell))
                })
                && clearance.as_ref().is_ok_and(|props| {
                    props.iter().all(|cleared| {
                        cleared.rights_cells.iter().all(|cell| {
                            allowed(KingdomPermission::Break, *cell)
                                && allowed(KingdomPermission::Build, *cell)
                        })
                    })
                })
        })
    });
    let boxes = target
        .properties
        .as_deref()
        .map(|properties| terra_world::placement::boxes(&plan, chunks, journal, properties));
    let half = Vec3::new(
        terra_physics::character::PLAYER_RADIUS,
        terra_physics::character::PLAYER_HALF_HEIGHT,
        terra_physics::character::PLAYER_RADIUS,
    );
    let overlaps = |center: Vec3| {
        boxes.as_ref().is_some_and(|boxes| {
            boxes.iter().any(|shape| {
                shape.intersects(
                    (center - half).to_array().map(f64::from),
                    (center + half).to_array().map(f64::from),
                )
            })
        })
    };
    let blocked = target.bodies.iter().any(|body| overlaps(body.translation))
        || target.remote.as_ref().is_some_and(|players| {
            players
                .players
                .values()
                .any(|track| overlaps(track.position()))
        });
    let within_height = target.config.as_ref().is_none_or(|config| {
        !config.world.bounded_world
            || plan
                .edits
                .iter()
                .all(|edit| (-128..=356).contains(&edit.position[1]))
    });
    let valid = environment_valid
        && existing_sites_loaded
        && endpoint_reachable
        && foreground_clear
        && within_height
        && hit.distance <= 4.5
        && has_item
        && permitted
        && !blocked;
    if !valid {
        presentation.preview.rail_error = Some(if !endpoint_reachable || hit.distance > 4.5 {
            "Das Ziel-Gleisende ist zu weit entfernt".into()
        } else if !has_item {
            format!("Zu wenig Material: {cost} benötigt")
        } else if !permitted {
            "Keine Baurechte am Gleis oder im Baubereich".into()
        } else if !existing_sites_loaded {
            "Der vorhandene Gleisanschluss ist noch nicht vollständig geladen".into()
        } else if blocked {
            "Ein Spieler steht im Gleisbereich".into()
        } else if !within_height {
            "Das Gleis liegt außerhalb der Bauhöhe".into()
        } else if !foreground_clear {
            "Das anvisierte Objekt versperrt den Gleisbau".into()
        } else {
            environment_error.unwrap_or_else(|| "Der Gleisbereich ist blockiert".into())
        });
    }
    presentation.preview.position = Some(anchor);
    presentation.preview.material = material;
    presentation.preview.valid = valid;
    presentation.preview.rail_cost = cost;
    presentation.preview.plan = Some(plan);
    if valid && keys.just_pressed(Action::Place) {
        presentation.motion.placing = true;
        presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
        if let Some(start) = start {
            requests
                .rails
                .write(terra_network::client::RailConnectRequest {
                    start,
                    position: hit.position,
                    expected: chunks.block_at(hit.position).unwrap_or(0),
                });
            presentation.preview.rail_start = None;
        } else {
            requests.blocks.write(BlockEditRequest {
                position: anchor,
                expected_material: chunks.block_at(anchor).unwrap_or(0),
                material,
            });
        }
    }
}
