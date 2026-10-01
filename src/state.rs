use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{io::Write, sync::RwLock};
use url::Url;

use crate::error::PortalError;

pub struct State {
    inner: RwLock<AppState>,
    path: PathBuf,
}

impl State {
    /// Load state file
    pub fn load(path: PathBuf) -> Self {
        let state = match std::fs::read_to_string(&path) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_else(|e| {
                tracing::warn!("Failed to parse state file {path:?}: {e}");
                AppState::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppState::default(),
            Err(e) => {
                tracing::warn!("Failed to read state file {path:?}: {e}");
                AppState::default()
            }
        };

        Self {
            inner: RwLock::new(state),
            path,
        }
    }

    fn read_state(&self) -> Option<std::sync::RwLockReadGuard<'_, AppState>> {
        self.inner
            .read()
            .inspect_err(|_| {
                tracing::warn!("State file lock poisoned");
            })
            .ok()
    }

    /// Get last directory usewd by a given app, and global last directory
    pub fn get_last_dir(&self, app_id: &str) -> (Option<PathBuf>, Option<PathBuf>) {
        match self.read_state() {
            Some(state) => (state.get_last_dir(), state.get_app_last_dir(app_id)),
            None => (None, None),
        }
    }
    /// Set last directory for a given application
    pub fn set_last_dir(&self, app_id: &str, dir_path: &Path) -> Result<(), PortalError> {
        let contents = {
            let mut state = self.inner.write()?;
            state.set_last_dir(app_id, dir_path)?;
            toml::to_string_pretty(&*state)?
        };

        let dir = self.path.parent().expect("state path created at startup");

        let mut tmp = tempfile::Builder::new()
            .prefix(".state-")
            .suffix(".tmp")
            .tempfile_in(dir)?;

        tmp.write_all(contents.as_bytes())?;
        tmp.flush()?;
        tmp.persist(&self.path)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerAppState {
    pub last_dir: Option<Url>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    #[serde(default)]
    pub global: PerAppState,
    #[serde(default, flatten, skip_serializing_if = "HashMap::is_empty")]
    pub apps: HashMap<String, PerAppState>,
}

impl AppState {
    fn url_to_path(url: &Url) -> Option<PathBuf> {
        match url.to_file_path() {
            Ok(path) => Some(path),
            Err(_) => {
                tracing::warn!("Stored last_dir is not a valid path: {url:?}");
                None
            }
        }
    }

    fn get_last_dir(&self) -> Option<PathBuf> {
        let url = self.global.last_dir.as_ref()?;
        Self::url_to_path(url)
    }

    fn get_app_last_dir(&self, app_id: &str) -> Option<PathBuf> {
        let url = self
            .apps
            .get(app_id)
            .and_then(|a| a.last_dir.as_ref())
            .or(self.global.last_dir.as_ref())?;
        Self::url_to_path(url)
    }

    fn set_last_dir(&mut self, app_id: &str, path: &Path) -> Result<(), PortalError> {
        let url =
            Url::from_file_path(path).map_err(|_| PortalError::InvalidPath(path.to_path_buf()))?;

        self.global.last_dir = Some(url.clone());

        if let Some(app) = self.apps.get_mut(app_id) {
            app.last_dir = Some(url);
        } else {
            self.apps.insert(
                app_id.to_string(),
                PerAppState {
                    last_dir: Some(url),
                },
            );
        }
        Ok(())
    }
}
