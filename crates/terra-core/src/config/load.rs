use super::ConfigError;
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub trait Config: Default + for<'de> Deserialize<'de> {
    fn validate(&self) -> Result<(), ConfigError>;
    fn save(&self, path: impl AsRef<Path>) -> Result<(), ConfigError>
    where
        Self: serde::Serialize,
    {
        self.validate()?;
        super::persist::save_toml(self, path)
    }
    fn load_or_create(path: impl AsRef<Path>) -> Result<Self, ConfigError>
    where
        Self: serde::Serialize,
    {
        let path = path.as_ref();
        match Self::load(path) {
            Err(ConfigError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                let config = Self::default();
                config.save(path)?;
                Ok(config)
            }
            result => result,
        }
    }
    fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(text).map_err(|source| ConfigError::Parse {
            path: PathBuf::from("<input>"),
            source,
        })?;
        config.validate()?;
        Ok(config)
    }
    /// Missing files are errors. Defaults apply only to omitted TOML fields.
    fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.into(),
            source,
        })?;
        let config: Self = toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.into(),
            source,
        })?;
        config.validate()?;
        Ok(config)
    }
}
pub fn menu_config_directory() -> PathBuf {
    std::env::var_os("TERRA_MENU_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(config_directory)
}
/// Select the config directory without relying on the process working directory.
pub fn config_directory() -> PathBuf {
    std::env::var_os("TERRA_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::installation_root().join("configs"))
}
