use bevy::{input::mouse::MouseWheel, prelude::*};

#[derive(Clone, Debug)]
pub struct BlockChoice {
    pub id: u16,
    pub name: String,
    pub identifier: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct CreativeCategory {
    pub id: String,
    pub name: String,
    pub icon: terra_core::ItemKind,
    pub order: i32,
}

#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct ActiveItem(pub Option<terra_core::ItemKind>);

#[derive(Resource, Debug)]
pub struct BlockPalette {
    pub choices: Vec<BlockChoice>,
    pub selected: usize,
    pub prop_descriptions: std::collections::HashMap<terra_core::ResourcePropKind, String>,
    pub creative_categories: Vec<CreativeCategory>,
    pub item_categories: std::collections::HashMap<terra_core::ItemKind, String>,
    pub item_metadata: std::collections::HashMap<terra_core::ItemKind, (String, String)>,
}

impl Default for BlockPalette {
    fn default() -> Self {
        Self {
            choices: vec![BlockChoice {
                id: 3,
                name: "Stone".into(),
                identifier: "teramic::stone".into(),
                description: String::new(),
            }],
            selected: 0,
            prop_descriptions: default(),
            creative_categories: default(),
            item_categories: default(),
            item_metadata: default(),
        }
    }
}

impl BlockPalette {
    pub fn selected(&self) -> Option<&BlockChoice> {
        self.choices.get(self.selected)
    }
}

pub(crate) fn select_block(
    mut wheel: MessageReader<MouseWheel>,
    input: Res<crate::CameraInputState>,
    blocked: Res<crate::PlayerInputBlock>,
    mut palette: ResMut<BlockPalette>,
    active: Option<ResMut<ActiveItem>>,
    slot: Option<ResMut<terra_network::client::SelectedInventorySlot>>,
    session: Option<Res<terra_network::client::ClientSession>>,
) {
    let movement: f32 = wheel.read().map(|e| e.y).sum();
    if session.as_ref().is_some_and(|s| s.ready)
        || !input.captured
        || blocked.0
        || movement == 0.0
        || palette.choices.is_empty()
    {
        return;
    }
    let count = palette.choices.len();
    if let Some(mut active) = active {
        active.0 = None;
    }
    if let Some(mut slot) = slot {
        slot.0 = None;
    }
    palette.selected = if movement > 0.0 {
        (palette.selected + count - 1) % count
    } else {
        (palette.selected + 1) % count
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(crate::CameraInputState {
            captured: true,
            ..default()
        })
        .init_resource::<crate::PlayerInputBlock>()
        .insert_resource(BlockPalette {
            choices: [1, 2, 3]
                .map(|id| BlockChoice {
                    id,
                    name: format!("Block {id}"),
                    identifier: terra_core::ItemKind::Block(id).identifier(),
                    description: String::new(),
                })
                .to_vec(),
            selected: 0,
            prop_descriptions: default(),
            ..default()
        })
        .add_message::<MouseWheel>()
        .add_systems(Update, select_block);
        app
    }
    fn wheel(app: &mut App, y: f32) {
        app.world_mut().write_message(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }
    #[test]
    fn gameplay_wheel_wraps_the_material_selection() {
        let mut app = app();
        wheel(&mut app, 1.0);
        app.update();
        assert_eq!(
            app.world()
                .resource::<BlockPalette>()
                .selected()
                .unwrap()
                .id,
            3
        );
        wheel(&mut app, -1.0);
        app.update();
        assert_eq!(
            app.world()
                .resource::<BlockPalette>()
                .selected()
                .unwrap()
                .id,
            1
        );
    }
    #[test]
    fn ready_session_leaves_hotbar_item_and_slot_to_inventory_navigation() {
        for mode in [
            terra_core::GameMode::Survival,
            terra_core::GameMode::Creative,
        ] {
            let mut app = app();
            let mut session = terra_network::client::ClientSession::default();
            session.ready = true;
            session.game_mode = mode;
            app.insert_resource(session);
            let plant = terra_core::ItemKind::Prop(terra_core::ResourcePropKind::Flower);
            app.insert_resource(ActiveItem(Some(plant)))
                .insert_resource(terra_network::client::SelectedInventorySlot(Some(2)));
            wheel(&mut app, -1.0);
            app.update();
            assert_eq!(app.world().resource::<BlockPalette>().selected, 0);
            assert_eq!(app.world().resource::<ActiveItem>().0, Some(plant));
            assert_eq!(
                app.world()
                    .resource::<terra_network::client::SelectedInventorySlot>()
                    .0,
                Some(2)
            );
            app.world_mut()
                .resource_mut::<terra_network::client::ClientSession>()
                .ready = false;
            app.update();
            assert_eq!(app.world().resource::<BlockPalette>().selected, 0);
        }
    }

    #[test]
    fn menu_wheel_is_consumed_without_changing_or_deferring_material_selection() {
        let mut app = app();
        app.world_mut().resource_mut::<crate::PlayerInputBlock>().0 = true;
        wheel(&mut app, -1.0);
        app.update();
        assert_eq!(app.world().resource::<BlockPalette>().selected, 0);
        app.world_mut().resource_mut::<crate::PlayerInputBlock>().0 = false;
        app.update();
        assert_eq!(app.world().resource::<BlockPalette>().selected, 0);
        app.world_mut()
            .resource_mut::<crate::CameraInputState>()
            .captured = false;
        wheel(&mut app, -1.0);
        app.update();
        assert_eq!(app.world().resource::<BlockPalette>().selected, 0);
    }
}
