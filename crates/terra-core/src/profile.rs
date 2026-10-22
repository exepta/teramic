use crate::{Config, ConfigError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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

/// A persistent local identity, not proof of an authenticated online account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct PlayerProfile {
    pub uuid: uuid::Uuid,
    pub name: String,
    #[serde(default)]
    pub gender: PlayerGender,
}
impl Default for PlayerProfile {
    fn default() -> Self {
        let uuid = uuid::Uuid::new_v4();
        Self {
            uuid,
            name: format!("Player-{}", &uuid.simple().to_string()[..8]),
            gender: PlayerGender::default(),
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
