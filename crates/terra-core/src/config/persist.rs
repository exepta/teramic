use super::{ClientConfig, Config, ConfigError};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static SAVE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

impl ClientConfig {
    /// Validate and replace one TOML file without truncating the previous version.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        self.validate()?;
        save_toml(self, path)
    }
}

pub fn save_toml<T: serde::Serialize + ?Sized>(
    value: &T,
    path: impl AsRef<Path>,
) -> Result<(), ConfigError> {
    let path = path.as_ref();
    let text = toml::to_string_pretty(value)
        .map_err(|error| ConfigError::Validation(format!("Cannot serialize TOML: {error}")))?;
    let name = path
        .file_name()
        .ok_or_else(|| ConfigError::Validation("Configuration path has no file name".into()))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let io_error = |source| ConfigError::Io {
        path: path.into(),
        source,
    };
    fs::create_dir_all(parent).map_err(io_error)?;
    let permissions = match fs::metadata(path) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(io_error(error)),
    };
    let mut temporary = parent.join(name);
    let mut file = None;
    for _ in 0..32 {
        let sequence = SAVE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut temporary_name = std::ffi::OsString::from(".");
        temporary_name.push(name);
        temporary_name.push(format!(".{}.{sequence}.tmp", std::process::id()));
        temporary = parent.join(temporary_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(opened) => {
                file = Some(opened);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(error)),
        }
    }
    let Some(mut file) = file else {
        return Err(ConfigError::Validation(
            "No unused temporary configuration path".into(),
        ));
    };
    let result = (|| {
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(io_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "terra-config-save-{}-{}",
            std::process::id(),
            SAVE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn save_roundtrips_and_invalid_updates_preserve_the_previous_file() {
        let directory = directory();
        let path = directory.join("client.toml");
        let mut config = ClientConfig {
            fov_degrees: 87.0,
            view_distance: 8,
            ..Default::default()
        };
        config.save(&path).unwrap();
        assert_eq!(ClientConfig::load(&path).unwrap(), config);
        let previous = fs::read(&path).unwrap();
        config.fov_degrees = f32::NAN;
        assert!(config.save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), previous);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn failed_replacement_cleans_up_and_does_not_replace_a_directory() {
        let directory = directory();
        let path = directory.join("client.toml");
        fs::create_dir(&path).unwrap();
        assert!(ClientConfig::default().save(&path).is_err());
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
}
