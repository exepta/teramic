use crate::{Config, ConfigError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayerGender {
    #[default]
    Male,
    Female,
}

impl PlayerGender {
    /// Command-line selection is validated before the client starts its renderer.
    pub fn from_arguments(
        arguments: impl IntoIterator<Item = String>,
    ) -> Result<Option<Self>, ConfigError> {
        let mut selected = None;
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            let value =
                if argument == "--gender" {
                    Some(arguments.next().ok_or_else(|| {
                        ConfigError::Validation("--gender requires f or m".into())
                    })?)
                } else {
                    argument.strip_prefix("--gender=").map(str::to_owned)
                };
            if let Some(value) = value {
                if selected.is_some() {
                    return Err(ConfigError::Validation("Use --gender only once".into()));
                }
                selected = Some(match value.as_str() {
                    "f" => Self::Female,
                    "m" => Self::Male,
                    _ => return Err(ConfigError::Validation("--gender must be f or m".into())),
                });
            }
        }
        Ok(selected)
    }
}

/// Cosmetic garment surfaces. These never grant armor or crafting statistics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClothingMaterial {
    #[default]
    Linen,
    Canvas,
    Leather,
    Wool,
}

impl ClothingMaterial {
    pub const ALL: [Self; 4] = [Self::Linen, Self::Canvas, Self::Leather, Self::Wool];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Linen => "linen",
            Self::Canvas => "canvas",
            Self::Leather => "leather",
            Self::Wool => "wool",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Linen => "Leinen",
            Self::Canvas => "Segeltuch",
            Self::Leather => "Leder",
            Self::Wool => "Wolle",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|material| material.key() == value)
    }
}

/// Portable character choices; equipment and account identity remain separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CharacterAppearance {
    /// Zero-based index into the selected body's authored hairstyle collection.
    pub hairstyle: u8,
    pub hair_color: [u8; 3],
    pub hair_highlights: bool,
    pub hair_highlight_color: [u8; 3],
    pub skin_color: [u8; 3],
    /// Left eye; old profiles used this color for both eyes.
    pub eye_color: [u8; 3],
    /// Missing legacy values inherit the existing eye color, not a new default.
    pub right_eye_color: Option<[u8; 3]>,
    pub boots_color: [u8; 3],
    pub cape_color: [u8; 3],
    pub tunic: bool,
    pub boots: bool,
    pub cape: bool,
    pub shirt_color: [u8; 3],
    pub pants_color: [u8; 3],
    pub shirt_material: ClothingMaterial,
    pub pants_material: ClothingMaterial,
    pub boots_material: ClothingMaterial,
    pub cape_material: ClothingMaterial,
}

impl Default for CharacterAppearance {
    fn default() -> Self {
        Self {
            hairstyle: 0,
            hair_color: [111, 65, 36],
            hair_highlights: false,
            hair_highlight_color: [207, 170, 91],
            skin_color: [252, 179, 122],
            eye_color: [82, 51, 31],
            right_eye_color: None,
            boots_color: [93, 55, 31],
            cape_color: [131, 31, 31],
            tunic: false,
            boots: false,
            cape: false,
            shirt_color: [235, 216, 185],
            pants_color: [109, 80, 54],
            shirt_material: ClothingMaterial::Linen,
            pants_material: ClothingMaterial::Canvas,
            boots_material: ClothingMaterial::Leather,
            cape_material: ClothingMaterial::Wool,
        }
    }
}

impl CharacterAppearance {
    pub const fn hairstyle_count(gender: PlayerGender) -> u8 {
        match gender {
            PlayerGender::Male => 12,
            PlayerGender::Female => 14,
        }
    }

    pub const fn resolved_right_eye_color(self) -> [u8; 3] {
        match self.right_eye_color {
            Some(color) => color,
            None => self.eye_color,
        }
    }

    pub fn validate_for(self, gender: PlayerGender) -> Result<(), ConfigError> {
        if self.hairstyle >= Self::hairstyle_count(gender) {
            return Err(ConfigError::Validation(
                "Hairstyle is not available for the selected character body".into(),
            ));
        }
        Ok(())
    }
}

/// A server-specific visual selection cannot replace the player's UUID or name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CharacterSelection {
    pub gender: PlayerGender,
    pub appearance: CharacterAppearance,
}

impl CharacterSelection {
    pub fn validate(self) -> Result<(), ConfigError> {
        self.appearance.validate_for(self.gender)
    }

    pub fn apply_to(self, profile: &mut PlayerProfile) {
        profile.gender = self.gender;
        profile.appearance = self.appearance;
    }
}

impl From<&PlayerProfile> for CharacterSelection {
    fn from(profile: &PlayerProfile) -> Self {
        Self {
            gender: profile.gender,
            appearance: profile.appearance,
        }
    }
}

/// A persistent local identity, not proof of an authenticated online account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct PlayerProfile {
    pub uuid: uuid::Uuid,
    pub name: String,
    #[serde(default)]
    pub gender: PlayerGender,
    #[serde(default)]
    pub appearance: CharacterAppearance,
}
impl Default for PlayerProfile {
    fn default() -> Self {
        let uuid = uuid::Uuid::new_v4();
        Self {
            uuid,
            name: format!("Player-{}", &uuid.simple().to_string()[..8]),
            gender: PlayerGender::default(),
            appearance: CharacterAppearance::default(),
        }
    }
}
impl PlayerProfile {
    pub fn load_for_client(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let gender = PlayerGender::from_arguments(std::env::args().skip(1))?;
        Self::load_with_gender(path, gender)
    }

    fn load_with_gender(
        path: impl AsRef<Path>,
        gender: Option<PlayerGender>,
    ) -> Result<Self, ConfigError> {
        let mut profile = Self::load_or_create(&path)?;
        if let Some(gender) = gender
            && profile.gender != gender
        {
            profile.gender = gender;
            if profile.appearance.validate_for(gender).is_err() {
                profile.appearance.hairstyle = 0;
            }
            profile.save(path)?;
        }
        Ok(profile)
    }
    /// Named local profiles keep independent persistent UUIDs. This never
    /// changes the identity rules or duplicate-login protection of a server.
    pub fn selected_path(configs: &Path) -> Result<PathBuf, ConfigError> {
        let mut arguments = std::env::args_os().skip(1);
        let mut name = None;
        while let Some(argument) = arguments.next() {
            if argument == "--profile" {
                if name.is_some() {
                    return Err(ConfigError::Validation("Use --profile only once".into()));
                }
                name = Some(arguments.next().ok_or_else(|| {
                    ConfigError::Validation("--profile requires a local profile name".into())
                })?);
            }
        }
        profile_path(
            configs,
            name.as_deref(),
            std::env::var_os("TERRA_PROFILE_FILE").as_deref(),
        )
    }
}

fn profile_path(
    configs: &Path,
    name: Option<&std::ffi::OsStr>,
    override_path: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, ConfigError> {
    if let Some(name) = name {
        if override_path.is_some() {
            return Err(ConfigError::Validation(
                "Choose either --profile or TERRA_PROFILE_FILE, not both".into(),
            ));
        }
        let name = name.to_str().filter(|name| {
            !name.is_empty()
                && name.len() <= 32
                && name.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
                && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        }).ok_or_else(|| ConfigError::Validation(
            "Profile names need 1..32 ASCII letters, digits, underscores or hyphens, starting with a letter or digit".into(),
        ))?;
        return Ok(configs.join("profiles").join(format!("{name}.toml")));
    }
    if let Some(path) = override_path {
        if path.is_empty() {
            return Err(ConfigError::Validation(
                "TERRA_PROFILE_FILE cannot be empty".into(),
            ));
        }
        return Ok(path.into());
    }
    Ok(configs.join("profile.toml"))
}
impl Config for PlayerProfile {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.uuid.is_nil()
            || self.name.trim().is_empty()
            || self.name.chars().count() > 24
            || self.name.chars().any(char::is_control)
        {
            return Err(ConfigError::Validation(
                "A nonzero player UUID and 1..24 printable name characters are required".into(),
            ));
        }
        self.appearance.validate_for(self.gender)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_eye_color_is_preserved_for_both_eyes_until_a_separate_color_is_selected() {
        let old = r#"uuid = "91f252f2-d68e-4d9f-968d-c9144f0ddfa7"
name = "Old eyes"
[appearance]
eye_color = [21, 103, 72]
"#;
        let mut profile: PlayerProfile = toml::from_str(old).unwrap();
        assert_eq!(profile.appearance.resolved_right_eye_color(), [21, 103, 72]);
        assert!(!profile.appearance.hair_highlights);
        profile.appearance.right_eye_color = Some([134, 69, 20]);
        profile.appearance.hair_highlights = true;
        profile.appearance.hair_highlight_color = [176, 73, 140];
        let saved = toml::to_string(&profile).unwrap();
        assert_eq!(PlayerProfile::from_toml(&saved).unwrap(), profile);
        assert_eq!(profile.appearance.eye_color, [21, 103, 72]);
        assert_eq!(profile.appearance.resolved_right_eye_color(), [134, 69, 20]);
    }

    #[test]
    fn garment_materials_roundtrip_and_legacy_profiles_keep_distinct_defaults() {
        let old = r#"uuid = "91f252f2-d68e-4d9f-968d-c9144f0ddfa7"
name = "Tailor"
[appearance]
shirt_color = [30, 80, 100]
"#;
        let profile: PlayerProfile = toml::from_str(old).unwrap();
        assert_eq!(profile.appearance.shirt_material, ClothingMaterial::Linen);
        assert_eq!(profile.appearance.pants_material, ClothingMaterial::Canvas);
        assert_eq!(profile.appearance.boots_material, ClothingMaterial::Leather);
        assert_eq!(profile.appearance.cape_material, ClothingMaterial::Wool);
        for material in ClothingMaterial::ALL {
            let mut selected = profile.clone();
            selected.appearance.shirt_material = material;
            selected.appearance.pants_material = material;
            selected.appearance.boots_material = material;
            selected.appearance.cape_material = material;
            let encoded = toml::to_string(&selected).unwrap();
            let decoded: PlayerProfile = toml::from_str(&encoded).unwrap();
            assert_eq!(decoded, selected);
            assert_eq!(decoded.uuid, profile.uuid);
            assert_eq!(decoded.appearance.shirt_color, [30, 80, 100]);
        }
        assert!(
            toml::from_str::<PlayerProfile>(&format!("{old}shirt_material = \"steel\"\n")).is_err()
        );
    }

    #[test]
    fn every_authored_hairstyle_and_all_color_channels_roundtrip() {
        for gender in [PlayerGender::Male, PlayerGender::Female] {
            for hairstyle in 0..CharacterAppearance::hairstyle_count(gender) {
                let profile = PlayerProfile {
                    gender,
                    appearance: CharacterAppearance {
                        hairstyle,
                        hair_color: [0, 127, 255],
                        shirt_color: [255, 20, 60],
                        pants_color: [18, 64, 210],
                        skin_color: [95, 57, 32],
                        eye_color: [30, 100, 170],
                        right_eye_color: Some([140, 55, 70]),
                        hair_highlights: true,
                        hair_highlight_color: [210, 70, 150],
                        boots: true,
                        cape: true,
                        tunic: true,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                assert_eq!(
                    PlayerProfile::from_toml(&toml::to_string(&profile).unwrap()).unwrap(),
                    profile
                );
            }
            let invalid = PlayerProfile {
                gender,
                appearance: CharacterAppearance {
                    hairstyle: CharacterAppearance::hairstyle_count(gender),
                    ..Default::default()
                },
                ..Default::default()
            };
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn legacy_gender_override_preserves_colors_and_resets_only_unavailable_hair() {
        let root = std::env::temp_dir().join(format!("terra-appearance-{}", uuid::Uuid::new_v4()));
        let path = root.join("profile.toml");
        let female = PlayerProfile {
            gender: PlayerGender::Female,
            appearance: CharacterAppearance {
                hairstyle: 13,
                hair_color: [255, 0, 0],
                shirt_color: [0, 255, 0],
                pants_color: [0, 0, 255],
                ..Default::default()
            },
            ..Default::default()
        };
        female.save(&path).unwrap();
        let male = PlayerProfile::load_with_gender(&path, Some(PlayerGender::Male)).unwrap();
        assert_eq!(male.uuid, female.uuid);
        assert_eq!(male.name, female.name);
        assert_eq!(
            male.appearance,
            CharacterAppearance {
                hairstyle: 0,
                ..female.appearance
            }
        );
        assert_eq!(PlayerProfile::load(&path).unwrap(), male);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn gender_cli_is_strict_and_old_profiles_keep_their_identity() {
        for (arguments, expected) in [
            (vec!["--gender=f"], Some(PlayerGender::Female)),
            (vec!["--gender=m"], Some(PlayerGender::Male)),
            (vec!["--gender", "f"], Some(PlayerGender::Female)),
            (vec!["--profile", "tester"], None),
        ] {
            assert_eq!(
                PlayerGender::from_arguments(arguments.into_iter().map(str::to_owned)).unwrap(),
                expected
            );
        }
        for arguments in [
            vec!["--gender"],
            vec!["--gender="],
            vec!["--gender=x"],
            vec!["--gender=f", "--gender=m"],
        ] {
            assert!(
                PlayerGender::from_arguments(arguments.into_iter().map(str::to_owned)).is_err()
            );
        }
        let root = std::env::temp_dir().join(format!("terra-gender-{}", uuid::Uuid::new_v4()));
        let path = root.join("profile.toml");
        std::fs::create_dir_all(&root).unwrap();
        let uuid = uuid::Uuid::new_v4();
        std::fs::write(
            &path,
            format!("uuid = '{uuid}'\nname = 'Existing Player'\n"),
        )
        .unwrap();
        let old = PlayerProfile::load(&path).unwrap();
        assert_eq!(old.gender, PlayerGender::Male);
        let female = PlayerProfile::load_with_gender(&path, Some(PlayerGender::Female)).unwrap();
        assert_eq!(female.uuid, uuid);
        assert_eq!(female.name, old.name);
        assert_eq!(
            PlayerProfile::load_with_gender(&path, None).unwrap(),
            female
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn named_test_profiles_are_separate_persistent_identities_without_touching_the_default() {
        let root =
            std::env::temp_dir().join(format!("terra-test-profiles-{}", uuid::Uuid::new_v4()));
        let default = PlayerProfile::load_or_create(root.join("profile.toml")).unwrap();
        let a_path = profile_path(&root, Some("tester-a".as_ref()), None).unwrap();
        let b_path = profile_path(&root, Some("tester-b".as_ref()), None).unwrap();
        let a = PlayerProfile::load_or_create(&a_path).unwrap();
        let b = PlayerProfile::load_or_create(&b_path).unwrap();
        assert_ne!(a.uuid, b.uuid);
        assert_ne!(a.uuid, default.uuid);
        assert_eq!(PlayerProfile::load_or_create(a_path).unwrap(), a);
        assert_eq!(PlayerProfile::load_or_create(b_path).unwrap(), b);
        assert_eq!(
            PlayerProfile::load_or_create(root.join("profile.toml")).unwrap(),
            default
        );
        for name in [
            "",
            "../player",
            "/tmp/player",
            "a/b",
            "a\\b",
            "bad name",
            "--connect",
        ] {
            assert!(
                profile_path(&root, Some(name.as_ref()), None).is_err(),
                "{name}"
            );
        }
        assert!(
            profile_path(&root, Some("tester".as_ref()), Some("custom.toml".as_ref())).is_err()
        );
        assert_eq!(
            profile_path(&root, None, Some("custom.toml".as_ref())).unwrap(),
            PathBuf::from("custom.toml")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn local_identity_roundtrips_and_is_not_a_session_counter() {
        let a = PlayerProfile::default();
        let b = PlayerProfile::default();
        assert_ne!(a.uuid, b.uuid);
        assert!(a.validate().is_ok());
        let text = toml::to_string(&a).unwrap();
        assert_eq!(toml::from_str::<PlayerProfile>(&text).unwrap(), a);
        let mut invalid = a;
        invalid.uuid = uuid::Uuid::nil();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn file_reload_preserves_identity_and_invalid_profiles_are_not_overwritten() {
        let root = std::env::temp_dir().join(format!("terra-profile-{}", uuid::Uuid::new_v4()));
        let path = root.join("profile.toml");
        let profile = PlayerProfile::load_or_create(&path).unwrap();
        assert_eq!(PlayerProfile::load_or_create(&path).unwrap(), profile);
        std::fs::write(&path, "uuid = 'invalid'\nname = 'Player'").unwrap();
        assert!(PlayerProfile::load_or_create(&path).is_err());
        assert!(std::fs::read_to_string(&path).unwrap().contains("invalid"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
