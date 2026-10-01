use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::context::FileChooserContext;
use crate::error::{ConfigError, ConfigErrorItem, ConfigErrorList, PortalError};
use crate::options::FileChooserOptions;
use crate::token::{TokenResolver, validate_template};

const TTYPICKER_REQ: &str = "TTYPICKER_REQ";
const TTYPICKER_METHOD: &str = "TTYPICKER_METHOD";

const RESERVED_ENVS: &[&str] = &[TTYPICKER_REQ, TTYPICKER_METHOD];

/// Environment variable (name and value)
type EnvVar = (String, OsString);

/// Picker command and environment variables
pub struct LaunchArgs {
    pub term_prog: OsString,
    pub term_args: Vec<OsString>,
    pub exec_args: Vec<OsString>,
    pub envs: Vec<EnvVar>,
}

#[derive(Debug, Clone)]
pub struct FileChooserConfig {
    pub default_dir: Option<String>,
    pub in_path: Option<String>,
    pub term: String,
    pub exec: String,
    pub env: HashMap<String, String>,
}

impl FileChooserConfig {
    /// Get default_dir from config
    pub fn get_default_dir(&self) -> Option<PathBuf> {
        self.default_dir.as_ref().map(PathBuf::from)
    }

    /// Get resolved in_path value from config
    pub fn get_in_path(
        &self,
        ctx: &FileChooserContext,
        opts: &FileChooserOptions,
    ) -> Result<Option<PathBuf>, PortalError> {
        match &self.in_path {
            Some(template) => TokenResolver::new(ctx, opts).parse_path(template),
            None => Ok(None),
        }
    }

    /// Build command arguments
    pub fn build_args(
        &self,
        ctx: &FileChooserContext,
        opts: &FileChooserOptions,
    ) -> Result<LaunchArgs, PortalError> {
        let resolver = TokenResolver::new(ctx, opts);

        let mut term_args = resolver.parse_cmd(&self.term)?;
        if term_args.is_empty() {
            return Err(PortalError::InvalidPickerCmd(self.term.clone()));
        }
        let term_prog = term_args.remove(0);

        let exec_args = resolver.parse_cmd(&self.exec)?;

        let mut envs: Vec<EnvVar> = self
            .env
            .iter()
            .map(|(k, v)| {
                let parsed_value = resolver.parse_value(v)?.unwrap_or_default();
                Ok((k.clone(), parsed_value))
            })
            .collect::<Result<_, PortalError>>()?;

        envs.retain(|(k, _)| {
            if RESERVED_ENVS.contains(&k.as_str()) {
                tracing::warn!(env = %k, "User attempted to set reserved environment variable");
                false // drop user value
            } else {
                true
            }
        });

        envs.push((
            TTYPICKER_REQ.to_owned(),
            ctx.req_path.clone().into_os_string(),
        ));
        envs.push((
            TTYPICKER_METHOD.to_owned(),
            OsString::from(ctx.method.clone()),
        ));

        Ok(LaunchArgs {
            term_prog,
            term_args,
            exec_args,
            envs,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ConfigLayer {
    default_dir: Option<String>,
    in_path: Option<String>,
    term: Option<String>,
    exec: Option<String>,
    env: Option<HashMap<String, String>>,
}

fn validate_path(template: &str) -> Result<(), ConfigError> {
    let path = Path::new(template);

    if !path.is_absolute() {
        return Err(ConfigError::NotAbsolute);
    }

    if !path.is_dir() {
        return Err(ConfigError::NotADirectory);
    }
    Ok(())
}

impl ConfigLayer {
    fn apply(&mut self, other: &ConfigLayer) {
        self.default_dir = other
            .default_dir
            .clone()
            .or_else(|| self.default_dir.take());
        self.in_path = other.in_path.clone().or_else(|| self.in_path.take());
        self.term = other.term.clone().or_else(|| self.term.take());
        self.exec = other.exec.clone().or_else(|| self.exec.take());
        if let Some(other_env) = &other.env {
            self.env.get_or_insert_default().extend(other_env.clone());
        }
    }

    fn validate(&self, path: &str, errors: &mut Vec<ConfigErrorItem>) {
        let mut check =
            |field: &str, val: &Option<String>, validator: fn(&str) -> Result<(), ConfigError>| {
                if let Some(template) = val
                    && let Err(error) = validator(template)
                {
                    errors.push(ConfigErrorItem {
                        path: path.to_string(),
                        field: field.to_string(),
                        error,
                    });
                }
            };

        check("default_dir", &self.default_dir, validate_path);
        check("in_path", &self.in_path, validate_template);
        check("term", &self.term, validate_template);
        check("exec", &self.exec, validate_template);

        if let Some(env) = &self.env {
            for (k, v) in env {
                if let Err(error) = validate_template(v) {
                    errors.push(ConfigErrorItem {
                        path: path.to_string(),
                        field: format!("env.{k}"),
                        error,
                    });
                }
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ConfigNode {
    #[serde(flatten)]
    cfg: ConfigLayer,

    #[serde(flatten)]
    children: HashMap<String, ConfigNode>,
}

impl ConfigNode {
    /// Recursively check this node and all of its children
    fn validate(&self, path: &str, errors: &mut Vec<ConfigErrorItem>) {
        self.cfg.validate(path, errors);
        for (child_key, child_node) in &self.children {
            let next_path = format!("{path}.{child_key}");
            child_node.validate(&next_path, errors);
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(flatten)]
    root: HashMap<String, ConfigNode>,
}

impl Config {
    /// Walk the entire config tree and validates all templates
    fn validate_all(&self) -> Result<(), PortalError> {
        let mut errors = Vec::new();
        for (key, node) in &self.root {
            node.validate(key, &mut errors);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(PortalError::InvalidConfigMulti(ConfigErrorList(errors)))
        }
    }

    /// Load and validate config tree
    pub fn load(config_path: &Path) -> Result<Self, PortalError> {
        let content = std::fs::read_to_string(config_path).map_err(|e| {
            PortalError::InvalidConfig(format!("can't read config file at {config_path:?}: {e}"))
        })?;
        let config: Self = toml::from_str(&content)?;

        config.validate_all()?;
        Ok(config)
    }

    /// Resolve config leaf
    fn resolve(&self, path: &[&str]) -> ConfigLayer {
        let mut resolved = ConfigLayer::default();

        let mut current_map = &self.root;

        for part in path {
            if let Some(node) = current_map.get(*part) {
                resolved.apply(&node.cfg);
                current_map = &node.children;
            } else {
                break;
            }
        }
        resolved
    }

    /// Get FileChooser config leaf
    pub fn get_filechooser_config(
        &self,
        interface: &str,
        method: &str,
        app: &str,
    ) -> Result<FileChooserConfig, PortalError> {
        let resolved = self.resolve(&[interface, method, app]);

        Ok(FileChooserConfig {
            default_dir: resolved.default_dir,
            in_path: resolved.in_path,
            term: resolved.term.ok_or(PortalError::InvalidConfig(
                "missing 'term' field".to_string(),
            ))?,
            exec: resolved.exec.ok_or(PortalError::InvalidConfig(
                "missing 'exec' field".to_string(),
            ))?,
            env: resolved.env.unwrap_or_default(),
        })
    }
}
