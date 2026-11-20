use super::{Config, ConfigError, ensure};
use bevy::{ecs::system::SystemParam, prelude::*};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameAction {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    FlyDown,
    Sprint,
    Precision,
    Break,
    Place,
    Cursor,
    Pause,
    Flight,
    Camera,
    Inventory,
    DebugOverlay,
    ToggleHud,
    DebugBounds,
    Chat,
    Command,
    PickBlock,
    Drop,
    DropStack,
    Use,
    DebugCreature,
    MiniMap,
    MapZoomIn,
    MapZoomOut,
    FullMap,
    QuickWaypoint,
    Dismount,
    DebugMobBounds,
    DebugChunkBorders,
    Kingdom,
    BuildMode,
}
impl GameAction {
    pub const ALL: [Self; 34] = [
        Self::Forward,
        Self::Backward,
        Self::Left,
        Self::Right,
        Self::Jump,
        Self::FlyDown,
        Self::Sprint,
        Self::Precision,
        Self::Break,
        Self::Place,
        Self::Cursor,
        Self::Pause,
        Self::Camera,
        Self::Inventory,
        Self::DebugOverlay,
        Self::ToggleHud,
        Self::DebugBounds,
        Self::Chat,
        Self::Command,
        Self::PickBlock,
        Self::Drop,
        Self::DropStack,
        Self::Use,
        Self::DebugCreature,
        Self::MiniMap,
        Self::MapZoomIn,
        Self::MapZoomOut,
        Self::FullMap,
        Self::QuickWaypoint,
        Self::Dismount,
        Self::DebugMobBounds,
        Self::DebugChunkBorders,
        Self::Kingdom,
        Self::BuildMode,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Forward => "Move Forward",
            Self::Backward => "Move Backward",
            Self::Left => "Move Left",
            Self::Right => "Move Right",
            Self::Jump => "Jump / Fly Up (Double Tap: Flight)",
            Self::FlyDown => "Fly Down / Mount Sprint",
            Self::Sprint => "Sprint / Flight Boost / Dive",
            Self::Precision => "Precision Flight",
            Self::Break => "Break Block / Prop",
            Self::Place => "Place Block / Eat",
            Self::Cursor => "Release Cursor",
            Self::Pause => "Pause / Menu",
            Self::Flight => "Toggle Creative Flight",
            Self::Camera => "Camera Perspective",
            Self::Inventory => "Inventory / Materials",
            Self::DebugOverlay => "Debug Overlay",
            Self::ToggleHud => "Toggle HUD",
            Self::DebugBounds => "Debug Placement Bounds",
            Self::Chat => "Open Chat",
            Self::Command => "Open Command",
            Self::PickBlock => "Copy Target Block (Creative)",
            Self::Drop => "Drop One Item",
            Self::DropStack => "Drop Stack",
            Self::Use => "Use / Eat Selected Item",
            Self::DebugCreature => "Inspect Animal Stats",
            Self::MiniMap => "Toggle MiniMap",
            Self::MapZoomIn => "Map Zoom In",
            Self::MapZoomOut => "Map Zoom Out",
            Self::FullMap => "Open Full Map",
            Self::QuickWaypoint => "Create Waypoint",
            Self::Dismount => "Dismount",
            Self::DebugMobBounds => "Mob Hitboxes",
            Self::DebugChunkBorders => "Chunk Borders",
            Self::Kingdom => "Open Kingdom",
            Self::BuildMode => "Construction Mode",
        }
    }
    pub fn default_binding(self) -> InputBinding {
        InputBinding::Key(match self {
            Self::Forward => KeyCode::KeyW,
            Self::Backward => KeyCode::KeyS,
            Self::Left => KeyCode::KeyA,
            Self::Right => KeyCode::KeyD,
            Self::Jump => KeyCode::Space,
            Self::FlyDown => KeyCode::ShiftLeft,
            Self::Sprint => KeyCode::ControlLeft,
            Self::Precision => KeyCode::AltLeft,
            Self::Place => KeyCode::KeyB,
            Self::Cursor => KeyCode::Backquote,
            Self::Pause => KeyCode::Escape,
            Self::Flight => KeyCode::KeyF,
            Self::Camera => KeyCode::F5,
            Self::Inventory => KeyCode::KeyE,
            Self::MiniMap => KeyCode::KeyJ,
            Self::FullMap => KeyCode::KeyM,
            Self::QuickWaypoint => KeyCode::KeyN,
            Self::Dismount => KeyCode::KeyX,
            Self::DebugChunkBorders => KeyCode::F9,
            Self::Kingdom => KeyCode::KeyK,
            Self::BuildMode => KeyCode::Tab,
            Self::DebugMobBounds => {
                return InputBinding::Chord {
                    modifier: KeyCode::F1,
                    key: KeyCode::KeyH,
                };
            }
            Self::MapZoomIn => KeyCode::Equal,
            Self::MapZoomOut => KeyCode::Minus,
            Self::DebugOverlay => KeyCode::F1,
            Self::ToggleHud => KeyCode::F2,
            Self::DebugBounds => {
                return InputBinding::Chord {
                    modifier: KeyCode::F1,
                    key: KeyCode::KeyB,
                };
            }
            Self::DebugCreature => {
                return InputBinding::Chord {
                    modifier: KeyCode::F3,
                    key: KeyCode::KeyE,
                };
            }
            Self::Chat => KeyCode::KeyT,
            Self::Command => KeyCode::Slash,
            Self::PickBlock => return InputBinding::Mouse(MouseButton::Middle),
            Self::Drop => KeyCode::KeyQ,
            Self::Use => KeyCode::KeyR,
            Self::DropStack => {
                return InputBinding::Chord {
                    modifier: KeyCode::ShiftLeft,
                    key: KeyCode::KeyQ,
                };
            }
            Self::Break => return InputBinding::Mouse(MouseButton::Left),
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputBinding {
    Key(KeyCode),
    Mouse(MouseButton),
    Chord { modifier: KeyCode, key: KeyCode },
}
impl InputBinding {
    fn modifier_keys(key: KeyCode) -> [KeyCode; 2] {
        match key {
            KeyCode::ShiftLeft | KeyCode::ShiftRight => [KeyCode::ShiftLeft, KeyCode::ShiftRight],
            KeyCode::ControlLeft | KeyCode::ControlRight => {
                [KeyCode::ControlLeft, KeyCode::ControlRight]
            }
            KeyCode::AltLeft | KeyCode::AltRight => [KeyCode::AltLeft, KeyCode::AltRight],
            KeyCode::SuperLeft | KeyCode::SuperRight => [KeyCode::SuperLeft, KeyCode::SuperRight],
            other => [other, other],
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Chord { modifier, key } => format!(
                "{} + {}",
                Self::Key(modifier).label(),
                Self::Key(key).label()
            ),
            Self::Mouse(button) => format!("Mouse {button:?}"),
            Self::Key(key) => format!("{key:?}")
                .strip_prefix("Key")
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{key:?}")),
        }
    }

    pub fn pressed(
        self,
        keys: &ButtonInput<KeyCode>,
        mouse: Option<&ButtonInput<MouseButton>>,
    ) -> bool {
        match self {
            Self::Key(key) => keys.pressed(key),
            Self::Mouse(button) => mouse.is_some_and(|m| m.pressed(button)),
            Self::Chord { modifier, key } => {
                Self::modifier_keys(modifier)
                    .into_iter()
                    .any(|m| keys.pressed(m))
                    && keys.pressed(key)
            }
        }
    }

    pub fn just_pressed(
        self,
        keys: &ButtonInput<KeyCode>,
        mouse: Option<&ButtonInput<MouseButton>>,
    ) -> bool {
        match self {
            Self::Key(key) => keys.just_pressed(key),
            Self::Mouse(button) => mouse.is_some_and(|m| m.just_pressed(button)),
            Self::Chord { modifier, key } => {
                self.pressed(keys, mouse)
                    && (Self::modifier_keys(modifier)
                        .into_iter()
                        .any(|m| keys.just_pressed(m))
                        || keys.just_pressed(key))
            }
        }
    }

    pub fn just_released(
        self,
        keys: &ButtonInput<KeyCode>,
        mouse: Option<&ButtonInput<MouseButton>>,
    ) -> bool {
        match self {
            Self::Key(key) => keys.just_released(key),
            Self::Mouse(button) => mouse.is_some_and(|m| m.just_released(button)),
            Self::Chord { modifier, key } => {
                let modifiers = Self::modifier_keys(modifier);
                (modifiers.into_iter().any(|m| keys.just_released(m))
                    && !modifiers.into_iter().any(|m| keys.pressed(m))
                    && (keys.pressed(key) || keys.just_released(key)))
                    || (keys.just_released(key)
                        && modifiers
                            .into_iter()
                            .any(|m| keys.pressed(m) || keys.just_released(m)))
            }
        }
    }

    fn valid(self) -> bool {
        let known = |key| !matches!(key, KeyCode::Unidentified(_));
        match self {
            Self::Key(key) => known(key),
            Self::Mouse(_) => true,
            Self::Chord { modifier, key } => {
                Self::modifier_keys(modifier)[0] != Self::modifier_keys(key)[0]
                    && known(modifier)
                    && known(key)
            }
        }
    }

    fn conflicts(self, other: Self) -> bool {
        self == other
            || match (self, other) {
                (
                    Self::Chord {
                        modifier: a,
                        key: b,
                    },
                    Self::Chord {
                        modifier: c,
                        key: d,
                    },
                ) => {
                    let [a, b, c, d] = [a, b, c, d].map(|key| Self::modifier_keys(key)[0]);
                    (a == c && b == d) || (a == d && b == c)
                }
                _ => false,
            }
    }

    fn consumes(self, single: Self, keys: &ButtonInput<KeyCode>) -> bool {
        match (self, single) {
            (Self::Chord { modifier, key }, Self::Key(single)) => {
                (single == modifier || single == key)
                    && Self::modifier_keys(modifier)
                        .into_iter()
                        .any(|m| keys.pressed(m) || keys.just_released(m))
                    && (keys.pressed(key) || keys.just_released(key))
            }
            _ => false,
        }
    }
}
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ControlsConfig {
    #[serde(deserialize_with = "read_bindings")]
    pub bindings: BTreeMap<GameAction, InputBinding>,
    pub mouse_sensitivity: f32,
    pub auto_climb: bool,
}

fn read_bindings<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<GameAction, InputBinding>, D::Error> {
    let mut bindings = BTreeMap::<GameAction, InputBinding>::deserialize(deserializer)?;
    // Legacy F bindings remain readable; flight now follows the jump double tap.
    bindings.remove(&GameAction::Flight);
    // TAB now opens the construction camera. Only migrate the previous cursor
    // default; unrelated custom bindings keep their keys.
    if !bindings.contains_key(&GameAction::BuildMode)
        && bindings.get(&GameAction::Cursor) == Some(&InputBinding::Key(KeyCode::Tab))
    {
        bindings.remove(&GameAction::Cursor);
    }
    // N used to toggle the MiniMap; its old default now opens a quick waypoint.
    // An explicitly rebound MiniMap or any unrelated custom N remains intact.
    if !bindings.contains_key(&GameAction::QuickWaypoint)
        && bindings.get(&GameAction::MiniMap) == Some(&InputBinding::Key(KeyCode::KeyN))
    {
        bindings.remove(&GameAction::MiniMap);
    }
    // Older defaults used M for the MiniMap. Free it for the new full map while
    // keeping custom bindings, including a user-assigned N, intact.
    if !bindings.contains_key(&GameAction::FullMap)
        && bindings.get(&GameAction::MiniMap) == Some(&InputBinding::Key(KeyCode::KeyM))
    {
        bindings.remove(&GameAction::MiniMap);
    }
    if !bindings.contains_key(&GameAction::Camera) {
        let binding = [
            KeyCode::F5,
            KeyCode::F6,
            KeyCode::F7,
            KeyCode::F8,
            KeyCode::F9,
        ]
        .into_iter()
        .map(InputBinding::Key)
        .find(|binding| !bindings.values().any(|value| value == binding))
        .ok_or_else(|| serde::de::Error::custom("No free camera key; add a camera binding"))?;
        bindings.insert(GameAction::Camera, binding);
    }
    if !bindings.contains_key(&GameAction::Inventory) {
        let binding = [KeyCode::KeyE, KeyCode::KeyI, KeyCode::KeyG, KeyCode::KeyH]
            .into_iter()
            .map(InputBinding::Key)
            .find(|binding| !bindings.values().any(|value| value == binding))
            .ok_or_else(|| {
                serde::de::Error::custom("No free inventory key; add an inventory binding")
            })?;
        bindings.insert(GameAction::Inventory, binding);
    }
    for action in [
        GameAction::Cursor,
        GameAction::BuildMode,
        GameAction::DebugOverlay,
        GameAction::ToggleHud,
        GameAction::Chat,
        GameAction::Command,
        GameAction::PickBlock,
        GameAction::Drop,
        GameAction::DropStack,
        GameAction::FullMap,
        GameAction::MiniMap,
        GameAction::QuickWaypoint,
        GameAction::MapZoomIn,
        GameAction::MapZoomOut,
        GameAction::Dismount,
    ] {
        if bindings.contains_key(&action) {
            continue;
        }
        let binding = std::iter::once(action.default_binding())
            .chain(
                [
                    KeyCode::F2,
                    KeyCode::F3,
                    KeyCode::F4,
                    KeyCode::F6,
                    KeyCode::F7,
                    KeyCode::F8,
                    KeyCode::F9,
                    KeyCode::F10,
                    KeyCode::F11,
                    KeyCode::F12,
                ]
                .into_iter()
                .map(InputBinding::Key),
            )
            .find(|binding| !bindings.values().any(|value| value.conflicts(*binding)))
            .ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "No free key for {}; add a binding",
                    action.label()
                ))
            })?;
        bindings.insert(action, binding);
    }
    if !bindings.contains_key(&GameAction::DebugBounds) {
        let modifier = match bindings[&GameAction::DebugOverlay] {
            InputBinding::Key(key) => key,
            _ => KeyCode::F1,
        };
        let binding = [KeyCode::KeyB, KeyCode::KeyG, KeyCode::KeyH]
            .into_iter()
            .filter(|key| *key != modifier)
            .map(|key| InputBinding::Chord { modifier, key })
            .find(|binding| !bindings.values().any(|value| value.conflicts(*binding)))
            .ok_or_else(|| serde::de::Error::custom("No free debug bounds chord; add a binding"))?;
        bindings.insert(GameAction::DebugBounds, binding);
    }
    if !bindings.contains_key(&GameAction::DebugCreature) {
        let binding = [KeyCode::F3, KeyCode::F4, KeyCode::F6]
            .into_iter()
            .map(|modifier| InputBinding::Chord {
                modifier,
                key: KeyCode::KeyE,
            })
            .find(|candidate| !bindings.values().any(|value| value.conflicts(*candidate)))
            .ok_or_else(|| {
                serde::de::Error::custom("No free creature inspection chord; add a binding")
            })?;
        bindings.insert(GameAction::DebugCreature, binding);
    }
    if !bindings.contains_key(&GameAction::DebugMobBounds) {
        let binding = [KeyCode::KeyH, KeyCode::KeyG, KeyCode::KeyJ]
            .into_iter()
            .map(|key| InputBinding::Chord {
                modifier: KeyCode::F1,
                key,
            })
            .find(|candidate| !bindings.values().any(|value| value.conflicts(*candidate)))
            .ok_or_else(|| serde::de::Error::custom("No free mob hitbox chord; add a binding"))?;
        bindings.insert(GameAction::DebugMobBounds, binding);
    }
    if !bindings.contains_key(&GameAction::DebugChunkBorders) {
        let binding = [KeyCode::F9, KeyCode::F10, KeyCode::F8, KeyCode::F7]
            .into_iter()
            .map(InputBinding::Key)
            .find(|candidate| !bindings.values().any(|value| value.conflicts(*candidate)))
            .ok_or_else(|| serde::de::Error::custom("No free chunk border key; add a binding"))?;
        bindings.insert(GameAction::DebugChunkBorders, binding);
    }
    if !bindings.contains_key(&GameAction::Kingdom) {
        let binding = [
            KeyCode::KeyK,
            KeyCode::KeyL,
            KeyCode::F10,
            KeyCode::F8,
            KeyCode::F7,
        ]
        .into_iter()
        .map(InputBinding::Key)
        .find(|candidate| !bindings.values().any(|value| value.conflicts(*candidate)))
        .ok_or_else(|| serde::de::Error::custom("No free kingdom key; add a binding"))?;
        bindings.insert(GameAction::Kingdom, binding);
    }
    Ok(bindings)
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    #[test]
    fn old_default_cursor_tab_becomes_construction_without_losing_custom_bindings() {
        let mut old = ControlsConfig::default();
        old.bindings.remove(&GameAction::BuildMode);
        old.bindings
            .insert(GameAction::Cursor, InputBinding::Key(KeyCode::Tab));
        let loaded = ControlsConfig::from_toml(&toml::to_string(&old).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::BuildMode),
            InputBinding::Key(KeyCode::Tab)
        );
        assert_eq!(
            loaded.binding(GameAction::Cursor),
            InputBinding::Key(KeyCode::Backquote)
        );
        loaded.validate().unwrap();
        old.bindings
            .insert(GameAction::Cursor, InputBinding::Key(KeyCode::Backquote));
        old.bindings
            .insert(GameAction::Inventory, InputBinding::Key(KeyCode::Tab));
        let loaded = ControlsConfig::from_toml(&toml::to_string(&old).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::Inventory),
            InputBinding::Key(KeyCode::Tab)
        );
        assert_ne!(
            loaded.binding(GameAction::BuildMode),
            InputBinding::Key(KeyCode::Tab)
        );
        loaded.validate().unwrap();
    }
    #[test]
    fn kingdom_key_is_added_to_old_profiles_without_replacing_a_custom_k_binding() {
        let mut old = ControlsConfig::default();
        old.bindings.remove(&GameAction::Kingdom);
        let loaded = ControlsConfig::from_toml(&toml::to_string(&old).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::Kingdom),
            InputBinding::Key(KeyCode::KeyK)
        );
        old.bindings
            .insert(GameAction::Chat, InputBinding::Key(KeyCode::KeyK));
        let loaded = ControlsConfig::from_toml(&toml::to_string(&old).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::Chat),
            InputBinding::Key(KeyCode::KeyK)
        );
        assert_eq!(
            loaded.binding(GameAction::Kingdom),
            InputBinding::Key(KeyCode::KeyL)
        );
        loaded.validate().unwrap();
    }

    #[test]
    fn hud_binding_migrates_without_replacing_an_existing_f2_binding() {
        let mut old = ControlsConfig::default();
        old.bindings.remove(&GameAction::ToggleHud);
        let loaded = ControlsConfig::from_toml(&toml::to_string(&old).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::ToggleHud),
            InputBinding::Key(KeyCode::F2)
        );
        old.bindings
            .insert(GameAction::Place, InputBinding::Key(KeyCode::F2));
        let loaded = ControlsConfig::from_toml(&toml::to_string(&old).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::Place),
            InputBinding::Key(KeyCode::F2)
        );
        assert_ne!(
            loaded.binding(GameAction::ToggleHud),
            InputBinding::Key(KeyCode::F2)
        );
        loaded.validate().unwrap();
    }
    #[test]
    fn chunk_border_binding_migrates_without_stealing_an_existing_key() {
        let mut old = ControlsConfig::default();
        old.bindings.remove(&GameAction::DebugChunkBorders);
        let loaded = ControlsConfig::from_toml(&toml::to_string(&old).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::DebugChunkBorders),
            InputBinding::Key(KeyCode::F9)
        );
        old.bindings
            .insert(GameAction::Place, InputBinding::Key(KeyCode::F9));
        let loaded = ControlsConfig::from_toml(&toml::to_string(&old).unwrap()).unwrap();
        assert_ne!(
            loaded.binding(GameAction::DebugChunkBorders),
            InputBinding::Key(KeyCode::F9)
        );
        loaded.validate().unwrap();
    }
    #[test]
    fn old_controls_gain_dismount_x_and_nonconflicting_mob_hitbox_chord() {
        let mut old = ControlsConfig::default();
        old.bindings.remove(&GameAction::Dismount);
        old.bindings.remove(&GameAction::DebugMobBounds);
        let loaded = ControlsConfig::from_toml(&toml::to_string(&old).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::Dismount),
            InputBinding::Key(KeyCode::KeyX)
        );
        assert_eq!(
            loaded.binding(GameAction::DebugMobBounds),
            InputBinding::Chord {
                modifier: KeyCode::F1,
                key: KeyCode::KeyH
            }
        );
        old.bindings
            .insert(GameAction::Place, InputBinding::Key(KeyCode::KeyX));
        old.bindings.insert(
            GameAction::DebugBounds,
            InputBinding::Chord {
                modifier: KeyCode::F1,
                key: KeyCode::KeyH,
            },
        );
        let loaded = ControlsConfig::from_toml(&toml::to_string(&old).unwrap()).unwrap();
        assert_ne!(
            loaded.binding(GameAction::Dismount),
            InputBinding::Key(KeyCode::KeyX)
        );
        assert_ne!(
            loaded.binding(GameAction::DebugMobBounds),
            loaded.binding(GameAction::DebugBounds)
        );
        loaded.validate().unwrap();
    }
    #[test]
    fn auto_climb_is_opt_in_and_round_trips() {
        let legacy = ControlsConfig::from_toml("mouse_sensitivity = 0.002").unwrap();
        assert!(!legacy.auto_climb);
        let enabled = ControlsConfig {
            auto_climb: true,
            ..legacy
        };
        let loaded = ControlsConfig::from_toml(&toml::to_string(&enabled).unwrap()).unwrap();
        assert_eq!(enabled, loaded);
    }
    #[test]
    fn legacy_flight_bindings_are_retired_and_free_the_camera_key() {
        let mut legacy = ControlsConfig::default();
        legacy.bindings.remove(&GameAction::Camera);
        legacy
            .bindings
            .insert(GameAction::Flight, InputBinding::Key(KeyCode::F5));
        let text = toml::to_string(&legacy).unwrap();
        let loaded: ControlsConfig = toml::from_str(&text).unwrap();
        loaded.validate().unwrap();
        assert_eq!(
            loaded.binding(GameAction::Camera),
            InputBinding::Key(KeyCode::F5)
        );
        assert!(!loaded.bindings.contains_key(&GameAction::Flight));
        assert!(!GameAction::ALL.contains(&GameAction::Flight));
        assert_eq!(
            ControlsConfig::from_toml(&toml::to_string(&loaded).unwrap()).unwrap(),
            loaded
        );
    }
    #[test]
    fn legacy_bindings_gain_a_nonconflicting_inventory_key_and_rebind_persists() {
        let mut legacy = ControlsConfig::default();
        legacy.bindings.remove(&GameAction::Inventory);
        legacy
            .bindings
            .insert(GameAction::Place, InputBinding::Key(KeyCode::KeyE));
        let mut loaded = ControlsConfig::from_toml(&toml::to_string(&legacy).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::Inventory),
            InputBinding::Key(KeyCode::KeyI)
        );
        assert!(
            loaded
                .rebind(GameAction::Inventory, InputBinding::Key(KeyCode::KeyE))
                .is_err()
        );
        loaded
            .rebind(GameAction::Inventory, InputBinding::Key(KeyCode::KeyG))
            .unwrap();
        assert_eq!(
            loaded,
            ControlsConfig::from_toml(&toml::to_string(&loaded).unwrap()).unwrap()
        );
    }
}
impl Default for ControlsConfig {
    fn default() -> Self {
        Self {
            bindings: GameAction::ALL
                .into_iter()
                .map(|a| (a, a.default_binding()))
                .collect(),
            mouse_sensitivity: 0.0022,
            auto_climb: false,
        }
    }
}
impl ControlsConfig {
    pub fn binding(&self, action: GameAction) -> InputBinding {
        self.bindings
            .get(&action)
            .copied()
            .unwrap_or_else(|| action.default_binding())
    }
    pub fn rebind(&mut self, action: GameAction, binding: InputBinding) -> Result<(), String> {
        if let Some((&other, _)) = self
            .bindings
            .iter()
            .find(|(a, b)| **a != action && b.conflicts(binding))
        {
            return Err(format!(
                "{} is already assigned to {}",
                binding.label(),
                other.label()
            ));
        }
        if !binding.valid() {
            return Err("Unrecognized key or repeated key in chord".into());
        }
        self.bindings.insert(action, binding);
        Ok(())
    }
}
impl Config for ControlsConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        ensure(
            self.mouse_sensitivity.is_finite() && (0.0001..=0.02).contains(&self.mouse_sensitivity),
            "Mouse sensitivity must be 0.0001..0.02",
        )?;
        ensure(
            self.bindings.len() == GameAction::ALL.len(),
            "All gameplay actions need a binding",
        )?;
        for action in GameAction::ALL {
            let binding = self.bindings.get(&action).ok_or_else(|| {
                ConfigError::Validation(format!("Missing {} binding", action.label()))
            })?;
            ensure(binding.valid(), "Unrecognized key or repeated key in chord")?;
            ensure(
                !self
                    .bindings
                    .iter()
                    .any(|(&a, b)| a != action && b.conflicts(*binding)),
                "Duplicate input bindings",
            )?;
        }
        Ok(())
    }
}
#[derive(SystemParam)]
pub struct GameplayInput<'w> {
    pub keys: Res<'w, ButtonInput<KeyCode>>,
    pub mouse: Option<Res<'w, ButtonInput<MouseButton>>>,
    pub config: Option<Res<'w, ControlsConfig>>,
}
impl GameplayInput<'_> {
    pub fn binding(&self, a: GameAction) -> InputBinding {
        self.config
            .as_ref()
            .map_or_else(|| a.default_binding(), |c| c.binding(a))
    }
    fn inspection_alias(&self) -> Option<InputBinding> {
        let alias = InputBinding::Chord {
            modifier: KeyCode::F1,
            key: KeyCode::KeyE,
        };
        (self.binding(GameAction::DebugCreature) == GameAction::DebugCreature.default_binding()
            && self.binding(GameAction::DebugOverlay) == GameAction::DebugOverlay.default_binding()
            && self.binding(GameAction::Inventory) == GameAction::Inventory.default_binding()
            && !GameAction::ALL
                .into_iter()
                .any(|a| a != GameAction::DebugCreature && self.binding(a).conflicts(alias)))
        .then_some(alias)
    }
    fn consumed_by_chord(&self, a: GameAction) -> bool {
        (a != GameAction::DebugCreature
            && self
                .inspection_alias()
                .is_some_and(|alias| alias.consumes(self.binding(a), &self.keys)))
            || GameAction::ALL.into_iter().any(|other| {
                other != a && self.binding(other).consumes(self.binding(a), &self.keys)
            })
    }
    pub fn pressed(&self, a: GameAction) -> bool {
        if a == GameAction::DebugCreature
            && self
                .inspection_alias()
                .is_some_and(|alias| alias.pressed(&self.keys, self.mouse.as_deref()))
        {
            return true;
        }
        if self.config.is_none()
            && match a {
                GameAction::Sprint => self.keys.pressed(KeyCode::ControlRight),
                GameAction::FlyDown => self.keys.pressed(KeyCode::ShiftRight),
                GameAction::Precision => self.keys.pressed(KeyCode::AltRight),
                _ => false,
            }
        {
            return true;
        }
        !self.consumed_by_chord(a) && self.binding(a).pressed(&self.keys, self.mouse.as_deref())
    }
    pub fn just_pressed(&self, a: GameAction) -> bool {
        if a == GameAction::DebugCreature
            && self
                .inspection_alias()
                .is_some_and(|alias| alias.just_pressed(&self.keys, self.mouse.as_deref()))
        {
            return true;
        }
        !self.consumed_by_chord(a)
            && self
                .binding(a)
                .just_pressed(&self.keys, self.mouse.as_deref())
    }

    pub fn consume_pressed(&self) -> bool {
        let shift = |key| matches!(key, KeyCode::ShiftLeft | KeyCode::ShiftRight);
        let shift_binding = match self.binding(GameAction::Place) {
            InputBinding::Key(key) => shift(key),
            InputBinding::Chord { modifier, key } => shift(modifier) || shift(key),
            InputBinding::Mouse(_) => false,
        };
        self.pressed(GameAction::Use)
            || (self.pressed(GameAction::Place)
                && (shift_binding
                    || (!self.keys.pressed(KeyCode::ShiftLeft)
                        && !self.keys.pressed(KeyCode::ShiftRight))))
    }
}

#[cfg(test)]
mod debug_input_tests {
    use super::*;

    #[test]
    fn f1_e_alias_consumes_inventory_but_respects_explicit_inspection_rebinding() {
        #[derive(Resource, Default)]
        struct Keys(bool, bool);
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ControlsConfig>()
            .init_resource::<Keys>()
            .add_systems(Update, |input: GameplayInput, mut keys: ResMut<Keys>| {
                keys.0 = input.just_pressed(GameAction::DebugCreature);
                keys.1 = input.just_pressed(GameAction::Inventory);
            });
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F1);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        app.update();
        assert!(app.world().resource::<Keys>().0 && !app.world().resource::<Keys>().1);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ControlsConfig>()
            .rebind(
                GameAction::DebugCreature,
                InputBinding::Chord {
                    modifier: KeyCode::F4,
                    key: KeyCode::KeyG,
                },
            )
            .unwrap();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F1);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        app.update();
        assert!(!app.world().resource::<Keys>().0 && app.world().resource::<Keys>().1);
    }

    #[test]
    fn eating_follows_rebound_place_and_use_and_preserves_shift_placement() {
        #[derive(Resource, Default)]
        struct Eating(bool);
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ControlsConfig>()
            .init_resource::<Eating>()
            .add_systems(
                Update,
                |input: GameplayInput, mut eating: ResMut<Eating>| {
                    eating.0 = input.consume_pressed();
                },
            );
        for binding in [
            InputBinding::Key(KeyCode::KeyU),
            InputBinding::Mouse(MouseButton::Right),
            InputBinding::Chord {
                modifier: KeyCode::ShiftLeft,
                key: KeyCode::KeyU,
            },
        ] {
            app.world_mut()
                .resource_mut::<ControlsConfig>()
                .rebind(GameAction::Place, binding)
                .unwrap();
            match binding {
                InputBinding::Key(key) => app
                    .world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(key),
                InputBinding::Mouse(button) => app
                    .world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(button),
                InputBinding::Chord { modifier, key } => {
                    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                    keys.press(modifier);
                    keys.press(key);
                }
            }
            app.update();
            assert!(app.world().resource::<Eating>().0, "{binding:?}");
            for shift in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(shift);
                app.update();
                assert_eq!(
                    app.world().resource::<Eating>().0,
                    matches!(binding, InputBinding::Chord { .. }),
                );
            }
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .reset_all();
            app.update();
            assert!(!app.world().resource::<Eating>().0);
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        assert!(app.world().resource::<Eating>().0);
    }

    #[test]
    fn dropping_with_either_shift_consumes_plain_q_and_conflicts_are_detected() {
        for shift in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
            let mut keys = ButtonInput::default();
            keys.press(shift);
            keys.press(KeyCode::KeyQ);
            let binding = GameAction::DropStack.default_binding();
            assert!(binding.just_pressed(&keys, None));
            assert!(binding.consumes(GameAction::Drop.default_binding(), &keys));
        }
        let left = GameAction::DropStack.default_binding();
        let right = InputBinding::Chord {
            modifier: KeyCode::ShiftRight,
            key: KeyCode::KeyQ,
        };
        assert!(left.conflicts(right));
        let mut config = ControlsConfig::default();
        assert!(config.rebind(GameAction::PickBlock, right).is_err());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn old_controls_gain_map_actions_without_replacing_custom_keys() {
        let mut config = ControlsConfig::default();
        for action in [
            GameAction::MiniMap,
            GameAction::FullMap,
            GameAction::MapZoomIn,
            GameAction::MapZoomOut,
        ] {
            config.bindings.remove(&action);
        }
        config
            .bindings
            .insert(GameAction::Camera, InputBinding::Key(KeyCode::KeyM));
        let loaded = ControlsConfig::from_toml(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::Camera),
            InputBinding::Key(KeyCode::KeyM)
        );
        assert_ne!(
            loaded.binding(GameAction::MiniMap),
            InputBinding::Key(KeyCode::KeyM)
        );
        assert_ne!(
            loaded.binding(GameAction::FullMap),
            InputBinding::Key(KeyCode::KeyM)
        );
        loaded.validate().unwrap();
        assert_eq!(
            loaded,
            ControlsConfig::from_toml(&toml::to_string(&loaded).unwrap()).unwrap()
        );
    }

    #[test]
    fn legacy_default_map_key_migrates_and_custom_bindings_survive_reload() {
        let mut legacy = ControlsConfig::default();
        legacy.bindings.remove(&GameAction::FullMap);
        legacy.bindings.remove(&GameAction::QuickWaypoint);
        legacy
            .bindings
            .insert(GameAction::MiniMap, InputBinding::Key(KeyCode::KeyM));
        let loaded = ControlsConfig::from_toml(&toml::to_string(&legacy).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::FullMap),
            InputBinding::Key(KeyCode::KeyM)
        );
        assert_eq!(
            loaded.binding(GameAction::MiniMap),
            InputBinding::Key(KeyCode::KeyJ)
        );
        legacy
            .bindings
            .insert(GameAction::Camera, InputBinding::Key(KeyCode::KeyN));
        let loaded = ControlsConfig::from_toml(&toml::to_string(&legacy).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::Camera),
            InputBinding::Key(KeyCode::KeyN)
        );
        assert_eq!(
            loaded.binding(GameAction::FullMap),
            InputBinding::Key(KeyCode::KeyM)
        );
        assert_ne!(
            loaded.binding(GameAction::MiniMap),
            InputBinding::Key(KeyCode::KeyN)
        );
        loaded.validate().unwrap();

        legacy
            .bindings
            .insert(GameAction::MiniMap, InputBinding::Key(KeyCode::KeyL));
        let mut custom = ControlsConfig::from_toml(&toml::to_string(&legacy).unwrap()).unwrap();
        assert_eq!(
            custom.binding(GameAction::MiniMap),
            InputBinding::Key(KeyCode::KeyL)
        );
        custom
            .rebind(GameAction::FullMap, InputBinding::Key(KeyCode::KeyO))
            .unwrap();
        let reloaded = ControlsConfig::from_toml(&toml::to_string(&custom).unwrap()).unwrap();
        assert_eq!(custom, reloaded);
        reloaded.validate().unwrap();
    }
    #[test]
    fn previous_minimap_n_default_migrates_for_quick_waypoints_without_losing_custom_keys() {
        let mut previous = ControlsConfig::default();
        previous.bindings.remove(&GameAction::QuickWaypoint);
        previous
            .bindings
            .insert(GameAction::MiniMap, InputBinding::Key(KeyCode::KeyN));
        let migrated = ControlsConfig::from_toml(&toml::to_string(&previous).unwrap()).unwrap();
        assert_eq!(
            migrated.binding(GameAction::MiniMap),
            InputBinding::Key(KeyCode::KeyJ)
        );
        assert_eq!(
            migrated.binding(GameAction::QuickWaypoint),
            InputBinding::Key(KeyCode::KeyN)
        );
        migrated.validate().unwrap();
        previous
            .bindings
            .insert(GameAction::MiniMap, InputBinding::Key(KeyCode::KeyL));
        let custom = ControlsConfig::from_toml(&toml::to_string(&previous).unwrap()).unwrap();
        assert_eq!(
            custom.binding(GameAction::MiniMap),
            InputBinding::Key(KeyCode::KeyL)
        );
        assert_eq!(
            custom.binding(GameAction::QuickWaypoint),
            InputBinding::Key(KeyCode::KeyN)
        );
        custom.validate().unwrap();
    }

    #[test]
    fn old_controls_gain_drop_and_pick_without_overwriting_existing_keys() {
        let mut config = ControlsConfig::default();
        for action in [
            GameAction::Drop,
            GameAction::DropStack,
            GameAction::PickBlock,
        ] {
            config.bindings.remove(&action);
        }
        config
            .bindings
            .insert(GameAction::Forward, InputBinding::Key(KeyCode::KeyQ));
        let loaded = ControlsConfig::from_toml(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(
            loaded.binding(GameAction::Forward),
            InputBinding::Key(KeyCode::KeyQ)
        );
        assert_ne!(
            loaded.binding(GameAction::Drop),
            InputBinding::Key(KeyCode::KeyQ)
        );
        assert_eq!(
            loaded.binding(GameAction::PickBlock),
            InputBinding::Mouse(MouseButton::Middle)
        );
        loaded.validate().unwrap();
    }

    #[derive(Resource, Default)]
    struct Sample {
        place: bool,
        place_edge: bool,
        raw_b: bool,
        raw_f1: bool,
        inventory: bool,
        creature: bool,
    }

    fn sample(input: GameplayInput, mut result: ResMut<Sample>) {
        result.place = input.pressed(GameAction::Place);
        result.place_edge = input.just_pressed(GameAction::Place);
        result.raw_b = input.keys.pressed(KeyCode::KeyB);
        result.raw_f1 = input.keys.pressed(KeyCode::F1);
        result.inventory = input.just_pressed(GameAction::Inventory);
        result.creature = input.just_pressed(GameAction::DebugCreature);
    }

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ControlsConfig>()
            .init_resource::<Sample>()
            .add_systems(Update, sample);
        app
    }

    #[test]
    fn inspection_migrates_and_consumes_inventory_even_on_the_opening_frame() {
        let mut config = ControlsConfig::default();
        config.bindings.remove(&GameAction::DebugCreature);
        let migrated = ControlsConfig::from_toml(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(
            migrated.binding(GameAction::DebugCreature),
            GameAction::DebugCreature.default_binding()
        );
        let mut app = app();
        app.insert_resource(migrated);
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::F3);
            keys.press(KeyCode::KeyE);
        }
        app.update();
        let result = app.world().resource::<Sample>();
        assert!(result.creature && !result.inventory);
    }

    #[test]
    fn debug_chord_cannot_place_but_plain_b_and_raw_ui_input_are_preserved() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        assert!(app.world().resource::<Sample>().place);
        assert!(app.world().resource::<Sample>().place_edge);

        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(KeyCode::F1);
            keys.press(KeyCode::KeyB);
        }
        app.update();
        let sample = app.world().resource::<Sample>();
        assert!(!sample.place && !sample.place_edge);
        assert!(sample.raw_b && sample.raw_f1);

        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear();
            keys.release(KeyCode::F1);
        }
        app.update();
        assert!(!app.world().resource::<Sample>().place);
        assert!(!app.world().resource::<Sample>().place_edge);
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(KeyCode::KeyB);
        }
        app.update();
        assert!(app.world().resource::<Sample>().place_edge);
    }

    #[test]
    fn legacy_f1_binding_is_preserved_and_debug_gets_a_free_key() {
        let mut app = app();
        let mut legacy = ControlsConfig::default();
        for action in [
            GameAction::DebugOverlay,
            GameAction::ToggleHud,
            GameAction::DebugBounds,
            GameAction::Chat,
            GameAction::Command,
        ] {
            legacy.bindings.remove(&action);
        }
        legacy
            .bindings
            .insert(GameAction::Place, InputBinding::Key(KeyCode::F1));
        let migrated = ControlsConfig::from_toml(&toml::to_string(&legacy).unwrap()).unwrap();
        assert_eq!(
            migrated.binding(GameAction::DebugOverlay),
            InputBinding::Key(KeyCode::F2)
        );
        assert_eq!(
            migrated.binding(GameAction::DebugBounds),
            InputBinding::Chord {
                modifier: KeyCode::F2,
                key: KeyCode::KeyB
            }
        );
        assert_eq!(
            migrated,
            ControlsConfig::from_toml(&toml::to_string(&migrated).unwrap()).unwrap()
        );
        app.insert_resource(migrated);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F1);
        app.update();
        let sample = app.world().resource::<Sample>();
        assert!(sample.place && sample.place_edge);
        assert!(sample.raw_f1);
    }

    #[test]
    fn debug_bindings_are_rebindable_but_duplicate_and_reverse_chords_are_rejected() {
        let mut config = ControlsConfig::default();
        let previous = config.binding(GameAction::Place);
        let error = config
            .rebind(GameAction::Place, InputBinding::Key(KeyCode::F1))
            .unwrap_err();
        assert!(error.contains("Debug Overlay"));
        assert_eq!(config.binding(GameAction::Place), previous);
        config
            .rebind(GameAction::DebugOverlay, InputBinding::Key(KeyCode::F3))
            .unwrap();
        config
            .rebind(GameAction::Place, InputBinding::Key(KeyCode::F1))
            .unwrap();
        assert!(
            config
                .rebind(
                    GameAction::Chat,
                    InputBinding::Chord {
                        modifier: KeyCode::KeyB,
                        key: KeyCode::F1
                    }
                )
                .is_err()
        );
        assert!(
            config
                .rebind(
                    GameAction::Chat,
                    InputBinding::Chord {
                        modifier: KeyCode::KeyB,
                        key: KeyCode::KeyB
                    }
                )
                .is_err()
        );
        assert!(config.validate().is_ok());
    }
}
