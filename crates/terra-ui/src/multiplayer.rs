use bevy::{prelude::*, ui::UiGlobalTransform, window::PrimaryWindow};
use serde::Serialize;
use terra_core::MultiplayerConfig;
use terra_network::client::{ClientSession, CursorShare, ReplicatedPlayers, SocialState};
use tilt_ui::{ElementId, html_shared};
mod cursor;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct NameTag {
    pub id: String,
    pub name: String,
    pub style: String,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ChestPointer {
    pub id: String,
    pub name: String,
    pub style: String,
    pub color: String,
    pub item: bool,
    pub preview: String,
    pub count: String,
}

#[derive(Clone)]
struct CursorMotion {
    anchor: [i64; 3],
    from: Vec2,
    target: Vec2,
    received: std::time::Instant,
}
impl CursorMotion {
    fn position(&self) -> Vec2 {
        self.from.lerp(
            self.target,
            (self.received.elapsed().as_secs_f32() / 0.1).clamp(0.0, 1.0),
        )
    }
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct MultiplayerHud {
    pub names: Vec<NameTag>,
    pub cursors: Vec<ChestPointer>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<MultiplayerConfig>()
        .init_resource::<CursorShare>()
        .init_resource::<MultiplayerHud>()
        .add_systems(
            PostUpdate,
            (unclip_palettes, cursor::images).before(bevy::ui::UiSystems::Prepare),
        )
        .add_systems(
            PostUpdate,
            update
                .after(bevy::ui::UiSystems::PostLayout)
                .after(bevy::transform::TransformSystems::Propagate),
        );
}

fn unclip_palettes(
    mut commands: Commands,
    parts: Query<(Entity, &tilt_ui::ControlPart), Added<tilt_ui::ControlPart>>,
    pickers: Query<(), With<tilt_ui::ColorPickerState>>,
) {
    for (entity, part) in &parts {
        if part.kind == tilt_ui::ControlPartKind::Popup && pickers.contains(part.owner) {
            commands.entity(entity).insert(bevy::ui::OverrideClip);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update(
    config: Res<MultiplayerConfig>,
    profile: Option<Res<terra_core::PlayerProfile>>,
    session: Option<Res<ClientSession>>,
    social: Option<Res<SocialState>>,
    remotes: Option<Res<ReplicatedPlayers>>,
    inventory: Res<crate::InventoryView>,
    menu: Option<Res<crate::menu::MenuState>>,
    scale: Option<Res<UiScale>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<terra_player::TerraCamera>>,
    bodies: Query<&Transform, With<terra_player::PlayerBody>>,
    nodes: Query<(Entity, &ElementId, &ComputedNode, &UiGlobalTransform)>,
    parents: Query<&ChildOf>,
    mut share: ResMut<CursorShare>,
    mut hud: ResMut<MultiplayerHud>,
    mut motion: Local<std::collections::HashMap<terra_core::PlayerId, CursorMotion>>,
) {
    let mut next = MultiplayerHud::default();
    share.0 = None;
    let playing = menu.as_ref().is_none_or(|m| m.world_hud_active());
    let scale = scale.map_or(1.0, |s| s.0).max(0.1);
    if playing && session.as_ref().is_some_and(|s| s.ready && !s.dead()) {
        if config.player_names
            && let (Some(social), Some(remotes), Some((camera, transform))) =
                (social.as_ref(), remotes.as_ref(), cameras.iter().next())
        {
            for (&id, remote) in remotes.players.iter().take(128) {
                let position = remote.position();
                let origin = bodies
                    .iter()
                    .next()
                    .map_or(transform.translation(), |b| b.translation);
                if position.distance_squared(origin) > config.player_name_distance.powi(2) {
                    continue;
                }
                let Some(name) = social.names.get(&id) else {
                    continue;
                };
                let Ok(p) = camera.world_to_viewport(transform, position + Vec3::Y * 1.35) else {
                    continue;
                };
                next.names.push(NameTag {
                    id: id.0.to_string(),
                    name: name.clone(),
                    style: format!("left: {:.1}px; top: {:.1}px;", p.x / scale, p.y / scale),
                });
            }
        }
        if config.chest_cursors
            && inventory.open
            && let Some(container) = session.as_ref().and_then(|s| s.container.as_ref())
        {
            let id = if inventory.linked_container {
                "workbench-container-grid"
            } else {
                "container-grid"
            };
            if let Some((grid, _, node, transform)) = nodes
                .iter()
                .find(|(_, element, node, _)| element.0 == id && !node.is_empty())
            {
                let mut slots: Vec<_> = nodes
                    .iter()
                    .filter_map(|(entity, id, node, transform)| {
                        if id.0 != "inventory-cell" || node.is_empty() {
                            return None;
                        }
                        let mut current = entity;
                        for _ in 0..32 {
                            if current == grid {
                                return Some(Rect::from_center_size(
                                    transform.translation,
                                    node.size(),
                                ));
                            }
                            current = parents.get(current).ok()?.parent();
                        }
                        None
                    })
                    .collect();
                slots.sort_by(|a, b| {
                    a.min
                        .y
                        .total_cmp(&b.min.y)
                        .then(a.min.x.total_cmp(&b.min.x))
                });
                let size = node.size();
                let center = transform.translation;
                let low = center - size * 0.5;
                if let (Some(window), Some(profile)) = (windows.iter().next(), profile.as_ref())
                    && let Some(cursor) = window.physical_cursor_position()
                {
                    let p = (cursor - low) / size;
                    if p.cmpge(Vec2::ZERO).all()
                        && p.cmple(Vec2::ONE).all()
                        && let Some(position) = encode_position(&slots, cursor)
                    {
                        share.0 = Some(terra_network::protocol::social::ChestCursorInput {
                            anchor: container.anchor,
                            position,
                            color: config.color(profile.uuid),
                        });
                    }
                }
                if let Some(social) = social.as_ref().filter(|s| {
                    s.received
                        .is_some_and(|at| at.elapsed().as_secs_f32() < 1.0)
                }) {
                    for cursor in social
                        .cursors
                        .iter()
                        .filter(|c| c.cursor.anchor == container.anchor)
                        .take(32)
                    {
                        let Some(p) = decode_position(&slots, cursor.cursor.position) else {
                            continue;
                        };
                        let received = social.received.unwrap_or_else(std::time::Instant::now);
                        let track = motion.entry(cursor.player).or_insert(CursorMotion {
                            anchor: container.anchor,
                            from: p,
                            target: p,
                            received,
                        });
                        if track.anchor != container.anchor
                            || track.target.distance_squared(p) > 400.0_f32.powi(2)
                        {
                            *track = CursorMotion {
                                anchor: container.anchor,
                                from: p,
                                target: p,
                                received,
                            };
                        } else if track.received != received {
                            track.from = track.position();
                            track.target = p;
                            track.received = received;
                        }
                        let p = track.position();
                        let [r, g, b] = cursor.cursor.color;
                        next.cursors.push(ChestPointer {
                            id: cursor.player.0.to_string(),
                            name: cursor.name.clone(),
                            style: format!(
                                "left: {:.1}px; top: {:.1}px; color: #{r:02x}{g:02x}{b:02x};",
                                p.x * node.inverse_scale_factor,
                                p.y * node.inverse_scale_factor
                            ),
                            color: format!("Shared cursor {r:02x}{g:02x}{b:02x}"),
                            item: cursor.item.is_some(),
                            preview: cursor.item.map_or_else(String::new, |s| {
                                crate::inventory::preview_key(s.item)
                            }),
                            count: cursor
                                .item
                                .map_or_else(String::new, |s| s.count.to_string()),
                        });
                    }
                }
            }
        }
    }
    motion.retain(|id, _| next.cursors.iter().any(|c| c.id == id.0.to_string()));
    if *hud != next {
        *hud = next;
    }
}

fn encode_position(slots: &[Rect], cursor: Vec2) -> Option<[u16; 2]> {
    let (index, slot) = slots.iter().enumerate().min_by(|(_, a), (_, b)| {
        cursor
            .distance_squared(cursor.clamp(a.min, a.max))
            .total_cmp(&cursor.distance_squared(cursor.clamp(b.min, b.max)))
    })?;
    if index >= 36 {
        return None;
    }
    let p = ((cursor - slot.min) / slot.size()).clamp(Vec2::ZERO, Vec2::ONE);
    Some([
        ((index as u16) << 10) | (p.x * 1023.0).round() as u16,
        (p.y * 65535.0).round() as u16,
    ])
}
fn decode_position(slots: &[Rect], position: [u16; 2]) -> Option<Vec2> {
    let slot = slots.get(usize::from(position[0] >> 10))?;
    Some(
        slot.min
            + Vec2::new(
                f32::from(position[0] & 1023) / 1023.0,
                f32::from(position[1]) / 65535.0,
            ) * slot.size(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursors_stay_on_the_same_slot_with_different_columns_and_scales() {
        let grid = |columns: usize, scale: f32| {
            (0..36)
                .map(|i| {
                    Rect::from_corners(
                        Vec2::new((i % columns) as f32, (i / columns) as f32) * 70.0 * scale,
                        (Vec2::new((i % columns) as f32, (i / columns) as f32) * 70.0
                            + Vec2::splat(66.0))
                            * scale,
                    )
                })
                .collect::<Vec<_>>()
        };
        let from = grid(6, 1.5);
        let to = grid(9, 0.8);
        let encoded = encode_position(&from, from[17].center()).unwrap();
        assert_eq!(encoded[0] >> 10, 17);
        assert!(
            decode_position(&to, encoded)
                .unwrap()
                .abs_diff_eq(to[17].center(), 0.1)
        );
    }
    #[test]
    fn nameplates_and_shared_cursors_use_supported_tilt_styles() {
        assert!(tilt_ui::Icon::from_name("users-round").is_some());
        assert!(tilt_ui::Icon::from_name("send").is_some());
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../../../src-ui/components/multiplayer-hud.component.css"
        ))
        .unwrap();
    }
    #[test]
    fn remote_cursor_interpolates_instead_of_snapping() {
        let track = CursorMotion {
            anchor: [0; 3],
            from: Vec2::ZERO,
            target: Vec2::new(100.0, 0.0),
            received: std::time::Instant::now() - std::time::Duration::from_millis(50),
        };
        assert!((45.0..60.0).contains(&track.position().x));
    }
}
