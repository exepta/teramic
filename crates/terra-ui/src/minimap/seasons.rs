//! One shared TiltUI material for the authoritative annual calendar ring.
use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct CalendarMaterial {
    #[uniform(0)]
    calendar: Vec4,
}
impl UiMaterial for CalendarMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_calendar_ui.wesl".into()
    }
}

#[derive(Resource, Default)]
struct Calendar(Handle<CalendarMaterial>);

struct CalendarPlugin;
pub(super) fn install(app: &mut App) {
    app.add_plugins(CalendarPlugin);
}
impl Plugin for CalendarPlugin {
    fn build(&self, app: &mut App) {
        crate::layout_classes::install(app);
        app.init_resource::<Calendar>()
            .add_systems(PostUpdate, bind.before(bevy::ui::UiSystems::Prepare))
            .add_systems(
                PostUpdate,
                center_cardinal_glyphs
                    .run_if(crate::layout_classes::active_cardinals)
                    .after(bevy::ui::UiSystems::PostLayout)
                    .before(bevy::ui::UiSystems::Stack),
            );
    }
    fn finish(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<CalendarMaterial>::default());
    }
}

fn center_cardinal_glyphs(world: &mut World) {
    world.resource_scope(|world, index: Mut<crate::layout_classes::LayoutClasses>| {
        center(world, &index.cardinals);
    });
}

fn center(world: &mut World, labels: &[Entity]) {
    for &label in labels {
        if world.get::<tilt_ui::ParkedUi>(label).is_some() {
            continue;
        }
        let mut pending = vec![label];
        while let Some(entity) = pending.pop() {
            if let Some(children) = world.get::<Children>(entity) {
                pending.extend(children.iter());
            }
            let Some(node) = world.get::<ComputedNode>(entity).filter(|n| !n.is_empty()) else {
                continue;
            };
            let content_origin = node.content_box().min;
            let Some(mut info) = world.get_mut::<bevy::text::TextLayoutInfo>(entity) else {
                continue;
            };
            let bounds = info.glyphs.iter().fold(
                Rect {
                    min: Vec2::splat(f32::INFINITY),
                    max: Vec2::splat(f32::NEG_INFINITY),
                },
                |bounds, glyph| {
                    let half = glyph.atlas_info.rect.size() * 0.5;
                    Rect {
                        min: bounds.min.min(glyph.position - half),
                        max: bounds.max.max(glyph.position + half),
                    }
                },
            );
            if info.glyphs.is_empty() {
                continue;
            }
            // Font ascenders/descenders center the line box, not the visible letter.
            let delta = -content_origin - bounds.center();
            if delta.length_squared() > 0.0001 {
                for glyph in &mut info.glyphs {
                    glyph.position += delta;
                }
            }
        }
    }
}

fn bind(
    mut commands: Commands,
    view: Res<super::MiniMapView>,
    mut state: ResMut<Calendar>,
    mut materials: ResMut<Assets<CalendarMaterial>>,
    mut nodes: Query<(
        Entity,
        &tilt_ui::ElementId,
        &mut ImageNode,
        Option<&MaterialNode<CalendarMaterial>>,
    )>,
) {
    if !view.visible || !view.expanded || !view.seasons {
        return;
    }
    let value = Vec4::new(view.season_cycle, view.frame_rotation, 0.0, 0.0);
    if let Some(mut material) = materials.get_mut(&state.0) {
        if material.calendar != value {
            material.calendar = value;
        }
    } else {
        state.0 = materials.add(CalendarMaterial { calendar: value });
    }
    for (entity, id, mut image, material) in &mut nodes {
        if id.0 != "map-season-ring" {
            continue;
        }
        if image.color != Color::NONE {
            image.color = Color::NONE;
        }
        if material.is_none_or(|m| m.0 != state.0) {
            commands
                .entity(entity)
                .insert(MaterialNode(state.0.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_cardinals_center_current_glyphs_after_class_and_park_changes() {
        use bevy::text::{GlyphAtlasInfo, PositionedGlyph, TextLayoutInfo};
        use tilt_ui::{ElementClasses, ParkedUiBindings};
        let mut app = App::new();
        crate::layout_classes::install(&mut app);
        app.add_systems(
            PostUpdate,
            center_cardinal_glyphs
                .after(crate::layout_classes::track)
                .run_if(crate::layout_classes::active_cardinals),
        );
        let label = app
            .world_mut()
            .spawn((Node::default(), ElementClasses::default()))
            .id();
        let layout = |scale: f32| TextLayoutInfo {
            scale_factor: scale,
            glyphs: vec![PositionedGlyph {
                position: Vec2::splat(3.),
                section_index: 0,
                line_index: 0,
                atlas_info: GlyphAtlasInfo {
                    texture: Default::default(),
                    rect: Rect::from_corners(Vec2::ZERO, Vec2::splat(7. * scale)),
                    offset: Vec2::ZERO,
                    is_alpha_mask: true,
                },
            }],
            ..default()
        };
        let glyph = app
            .world_mut()
            .spawn((
                Node::default(),
                ComputedNode {
                    size: Vec2::splat(20.),
                    ..default()
                },
                layout(1.),
                ChildOf(label),
            ))
            .id();
        let pos = |app: &App| app.world().get::<TextLayoutInfo>(glyph).unwrap().glyphs[0].position;
        let goal = |app: &App| {
            -app.world()
                .get::<ComputedNode>(glyph)
                .unwrap()
                .content_box()
                .min
        };
        app.update();
        assert_eq!(pos(&app), Vec2::splat(3.));
        app.world_mut()
            .get_mut::<ElementClasses>(label)
            .unwrap()
            .classes
            .push("map-cardinal".into());
        app.update();
        assert_eq!(pos(&app), goal(&app));
        let parked = ParkedUiBindings::take(app.world_mut(), label);
        app.world_mut().entity_mut(glyph).insert(layout(2.));
        app.world_mut().get_mut::<ComputedNode>(glyph).unwrap().size *= 2.;
        app.update();
        assert_eq!(pos(&app), Vec2::splat(3.));
        parked.restore(app.world_mut());
        app.update();
        assert_eq!(pos(&app), goal(&app));
        app.world_mut().entity_mut(label).remove::<ElementClasses>();
        app.world_mut().entity_mut(glyph).insert(layout(1.));
        app.update();
        assert_eq!(pos(&app), Vec2::splat(3.));
    }

    #[test]
    fn native_calendar_motion_preserves_shared_html_state_and_live_text_still_updates() {
        use super::super::MiniMapView;
        use tilt_ui::{TiltText, TiltUiCodePlugin, TiltUiComponentRuntimeSet, UiSharedValues};

        let mut app = App::new();
        app.add_plugins(TiltUiCodePlugin)
            .init_resource::<Calendar>()
            .init_resource::<Assets<CalendarMaterial>>()
            .insert_resource(MiniMapView {
                visible: true,
                expanded: true,
                seasons: true,
                clock: "10:00 Spring".into(),
                ..default()
            })
            .add_systems(Update, bind.after(TiltUiComponentRuntimeSet::Reactivity));
        let ring = app
            .world_mut()
            .spawn((
                tilt_ui::ElementId("map-season-ring".into()),
                ImageNode::default(),
            ))
            .id();
        let label = app
            .world_mut()
            .spawn((
                Text::new(""),
                TiltText {
                    value: "{{ miniMapView.clock }}".into(),
                },
            ))
            .id();
        app.update();
        let shared_tick = app
            .world()
            .get_resource_ref::<UiSharedValues>()
            .unwrap()
            .last_changed();
        let snapshot = app
            .world()
            .resource::<UiSharedValues>()
            .get("miniMapView")
            .unwrap()
            .clone();
        assert!(snapshot.get("season_cycle").is_none());
        assert!(snapshot.get("frame_rotation").is_none());
        assert_eq!(app.world().get::<Text>(label).unwrap().0, "10:00 Spring");
        let material = app
            .world()
            .get::<MaterialNode<CalendarMaterial>>(ring)
            .unwrap()
            .0
            .clone();

        for frame in 1..=120 {
            let phase = frame as f32 / 123.0;
            let rotation = frame as f32 * -0.013;
            {
                let mut view = app.world_mut().resource_mut::<MiniMapView>();
                view.season_cycle = phase;
                view.frame_rotation = rotation;
            }
            app.update();
            // This is the actual TiltUI shared resource: native uniforms must
            // not advance its change tick or replace its serialized state.
            assert_eq!(
                app.world()
                    .get_resource_ref::<UiSharedValues>()
                    .unwrap()
                    .last_changed(),
                shared_tick
            );
            assert_eq!(
                app.world().resource::<UiSharedValues>().get("miniMapView"),
                Some(&snapshot)
            );
            assert_eq!(app.world().get::<Text>(label).unwrap().0, "10:00 Spring");
            assert_eq!(
                app.world()
                    .get::<MaterialNode<CalendarMaterial>>(ring)
                    .unwrap()
                    .0,
                material
            );
            assert_eq!(
                app.world()
                    .resource::<Assets<CalendarMaterial>>()
                    .get(&material)
                    .unwrap()
                    .calendar,
                Vec4::new(phase, rotation, 0.0, 0.0)
            );
        }
        app.world_mut().resource_mut::<MiniMapView>().clock = "10:01 Spring".into();
        app.update();
        assert_ne!(
            app.world()
                .get_resource_ref::<UiSharedValues>()
                .unwrap()
                .last_changed(),
            shared_tick
        );
        assert_eq!(app.world().get::<Text>(label).unwrap().0, "10:01 Spring");
        assert_eq!(
            app.world()
                .resource::<UiSharedValues>()
                .get("miniMapView")
                .unwrap()["clock"],
            "10:01 Spring"
        );
        assert_eq!(app.world().resource::<Assets<CalendarMaterial>>().len(), 1);
    }

    #[test]
    fn calendar_uses_one_asset_across_seasons_and_recreated_tilt_nodes() {
        let mut app = App::new();
        app.init_resource::<Calendar>()
            .init_resource::<Assets<CalendarMaterial>>()
            .insert_resource(super::super::MiniMapView {
                visible: true,
                expanded: true,
                seasons: true,
                ..default()
            })
            .add_systems(Update, bind);
        for phase in [0.0, 0.249, 0.25, 0.5, 0.75, 0.999, 0.0] {
            app.world_mut()
                .resource_mut::<super::super::MiniMapView>()
                .season_cycle = phase;
            let node = app
                .world_mut()
                .spawn((
                    tilt_ui::ElementId("map-season-ring".into()),
                    ImageNode::default(),
                ))
                .id();
            app.update();
            let material = app
                .world()
                .get::<MaterialNode<CalendarMaterial>>(node)
                .unwrap();
            assert_eq!(
                app.world()
                    .resource::<Assets<CalendarMaterial>>()
                    .get(&material.0)
                    .unwrap()
                    .calendar
                    .x,
                phase
            );
            assert_eq!(app.world().resource::<Assets<CalendarMaterial>>().len(), 1);
            app.world_mut().despawn(node);
        }
    }
}
