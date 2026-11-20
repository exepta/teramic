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
    let _scope = terra_core::profiling::ScopeTimer::new("rails.player_preview");
    use terra_core::{kingdom::KingdomPermission, rails};
    if keys.pressed(Action::Break) {
        return;
    }
    let placing = keys.just_pressed(Action::Place) && !keys.just_pressed(Action::Use);
    let remember_start = keys.pressed(Action::FlyDown) && placing;
    if remember_start {
        presentation.preview.cancel_rail_chain();
        return;
    }
    if presentation.preview.rail_waiting {
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
    let origin = target.origin(camera);
    let rotation = look.rotation();
    let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
    let ray = (
        origin.to_array().map(f64::from),
        (rotation * Vec3::NEG_Z).to_array().map(f64::from),
    );
    let wood = item.wood_species().unwrap_or_default();
    let held_material = item.block_material().unwrap_or(rails::STRAIGHT);
    if held_material == rails::CROSSING {
        presentation.preview.rail_error = Some("Kreuzungen können nicht neu gebaut werden".into());
        return;
    }
    let fixed_piece = matches!(
        held_material,
        rails::SWITCH_LEFT | rails::SWITCH_RIGHT | rails::BUFFER | rails::TURNTABLE
    );
    let start = presentation.preview.rail_start;
    if start.is_none()
        && !fixed_piece
        && presentation.preview.rail_mode < 2
        && chunks.block_at(hit.position).is_some_and(rails::is_rail)
    {
        if placing && hit.distance <= rails::RAIL_BUILD_REACH {
            presentation.preview.rail_start =
                terra_world::rails::select_endpoint(journal, hit, ray);
            presentation.preview.rail_origin =
                presentation.preview.rail_start.and_then(|endpoint| {
                    Some((
                        terra_world::rails::owner_at(journal, endpoint.anchor)?,
                        journal.object_state(endpoint.anchor)?.wood,
                    ))
                });
            if presentation.preview.rail_start.is_none() {
                presentation.preview.rail_error =
                    Some("Kein freies Gleisende in Reichweite".into());
            }
        }
        return;
    }
    let starter =
        start.is_none() && held_material == rails::STRAIGHT && presentation.preview.rail_mode < 2;
    let material = if fixed_piece {
        held_material
    } else {
        match presentation.preview.rail_mode {
            2 => rails::BUFFER,
            _ => rails::STRAIGHT,
        }
    };
    let valid_plan = if let Some(start) = start {
        terra_world::rails::connection_aim(chunks, journal, start, hit, ray, [yaw, pitch], wood)
    } else if starter {
        terra_world::rails::seed(chunks, journal, hit, ray, [yaw, pitch], wood)
    } else {
        terra_world::rails::placement(chunks, journal, hit, material, ray, [yaw, pitch])
    };
    let environment_valid = valid_plan.is_ok();
    let environment_error = valid_plan.as_ref().err().cloned();
    let plan = valid_plan.or_else(|_| {
        if let Some(start) = start {
            terra_world::rails::proposal_connection_aim(
                chunks,
                journal,
                start,
                hit,
                ray,
                [yaw, pitch],
                wood,
            )
        } else if starter {
            terra_world::rails::proposal_seed(chunks, journal, hit, ray, [yaw, pitch], wood)
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
    if start.is_some() {
        let curved = plan
            .states
            .iter()
            .filter_map(|(_, state)| state.as_ref()?.rail_curve)
            .any(|curve| {
                let first = curve.points[0];
                let last = curve.points[3];
                curve.points[1..3].iter().any(|point| {
                    i64::from(point[0] - first[0]) * i64::from(last[2] - first[2])
                        != i64::from(point[2] - first[2]) * i64::from(last[0] - first[0])
                })
            });
        presentation.preview.rail_choice = Some(if curved {
            rails::BuildChoice::Curve
        } else {
            rails::BuildChoice::Straight
        });
    }
    presentation.preview.rail_ramp = plan
        .states
        .iter()
        .filter_map(|(_, state)| state.as_ref()?.rail_curve)
        .any(|curve| curve.points[0][1] != curve.points[3][1]);
    let cost = terra_world::rails::cost(&plan).unwrap_or(u16::MAX);
    let existing_sites = if let Some(start) = start {
        terra_world::rails::connection_sites_for_plan(journal, start, hit, &plan)
    } else {
        terra_world::rails::placement_sites(journal, hit)
    };
    let existing_sites_loaded = existing_sites.as_ref().is_some_and(|sites| {
        sites.iter().all(|cell| {
            chunks.block_at(*cell).is_some()
                && journal
                    .block(*cell)
                    .is_none_or(|material| chunks.block_at(*cell) == Some(material))
        })
    });
    let endpoint_reachable = start.is_none()
        || terra_world::rails::connection_target(&plan).is_some_and(|port| {
            let endpoint = port.position.position();
            (0..3)
                .map(|axis| (endpoint[axis] - ray.0[axis]).powi(2))
                .sum::<f64>()
                <= rails::RAIL_BUILD_REACH * rails::RAIL_BUILD_REACH
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
    let vehicle_blocked = target.session.as_ref().is_some_and(|session| {
        plan.edits.iter().any(|edit| {
            edit.material != 0
                && terra_world::trains::reserved_in(&session.trains.vehicles, edit.position)
        })
    });
    let local_player = target.session.as_ref().and_then(|session| session.player);
    let blocked = target.remote.as_ref().is_some_and(|players| {
        players
            .players
            .iter()
            .any(|(player, track)| Some(*player) != local_player && overlaps(track.position()))
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
        && hit.distance <= rails::RAIL_BUILD_REACH
        && has_item
        && permitted
        && !blocked
        && !vehicle_blocked;
    if !valid {
        presentation.preview.rail_error = Some(if let Some(error) = environment_error {
            error
        } else if !endpoint_reachable || hit.distance > rails::RAIL_BUILD_REACH {
            "Das Ziel-Gleisende ist zu weit entfernt".into()
        } else if !has_item {
            format!("Zu wenig Material: {cost} benötigt")
        } else if !permitted {
            "Keine Baurechte am Gleis oder im Baubereich".into()
        } else if !existing_sites_loaded {
            "Der vorhandene Gleisanschluss ist noch nicht vollständig geladen".into()
        } else if vehicle_blocked {
            "Ein Fahrzeug belegt den Gleisbereich".into()
        } else if blocked {
            "Ein Spieler steht im Gleisbereich".into()
        } else if !within_height {
            "Das Gleis liegt außerhalb der Bauhöhe".into()
        } else if !foreground_clear {
            "Das anvisierte Objekt versperrt den Gleisbau".into()
        } else {
            "Der Gleisbereich ist blockiert".into()
        });
    }
    presentation.preview.position = Some(anchor);
    presentation.preview.material = material;
    presentation.preview.valid = valid;
    presentation.preview.rail_cost = cost;
    if valid && placing {
        presentation.motion.placing = true;
        presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
        if starter || start.is_some() {
            let action = if let Some(start) = start {
                terra_network::client::RailPlacementAction::Stretch {
                    start,
                    position: hit.position,
                    expected: chunks.block_at(hit.position).unwrap_or(0),
                    signature: terra_world::rails::connection_signature(&plan, start),
                }
            } else {
                terra_network::client::RailPlacementAction::Seed {
                    position: hit.position,
                    expected: chunks.block_at(hit.position).unwrap_or(0),
                    signature: terra_world::rails::seed_signature(&plan),
                }
            };
            let token = presentation.preview.rail_sequence.checked_add(1);
            let pending = token.and_then(|token| {
                super::super::rail_sequence::Pending::new(
                    token,
                    action,
                    &plan,
                    wood,
                    target.session.as_deref(),
                )
            });
            if let Some(pending) = pending {
                presentation.preview.rail_sequence = pending.token;
                requests
                    .rail_placements
                    .write(terra_network::client::RailPlacementRequest {
                        token: pending.token,
                        action,
                    });
                presentation.preview.rail_pending = Some(pending);
                presentation.preview.rail_waiting = true;
            } else {
                presentation.preview.valid = false;
                presentation.preview.rail_error =
                    Some("Die Gleisanfrage konnte nicht vorgemerkt werden".into());
            }
        } else {
            requests.blocks.write(BlockEditRequest {
                position: anchor,
                expected_material: chunks.block_at(anchor).unwrap_or(0),
                material,
            });
        }
    }
    presentation.preview.plan = Some(plan);
}
