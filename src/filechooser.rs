use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
use std::{io::ErrorKind, sync::Arc};
use tokio::process::Command;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::config::{Config, LaunchArgs};
use crate::context::FileChooserContext;
use crate::error::PortalError;
use crate::options::FileChooserOptions;
use crate::state::State;
use crate::utils;

const FILENAME_CONFIG: &str = "config.toml";
const FILENAME_STATE: &str = "state.toml";

type Response = u32;
type FileChooserResult = HashMap<String, OwnedValue>;
struct PortalResponse;

impl PortalResponse {
    pub const SUCCESS: Response = 0;
    pub const CANCELLED: Response = 1;
    pub const OTHER: Response = 2;
}

fn get_home_dir() -> Result<PathBuf, PortalError> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or(PortalError::Environment(
            "can't determine home directory".to_string(),
        ))
}

fn get_config_path() -> Result<PathBuf, PortalError> {
    let base = match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        Some(v) => PathBuf::from(v),
        None => get_home_dir()?.join(".config"),
    };

    Ok(base.join(crate::TTYPICKER).join(FILENAME_CONFIG))
}

fn get_state_path() -> Result<PathBuf, PortalError> {
    let base = match std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        Some(v) => PathBuf::from(v),
        None => get_home_dir()?.join(".local").join("state"),
    };

    Ok(base.join(crate::TTYPICKER).join(FILENAME_STATE))
}

async fn create_unique_path(
    file_path: &Path,
    dir: bool,
    fallback_dir: &Path,
) -> Result<PathBuf, PortalError> {
    const MAX_ATTEMPTS: u32 = 9_999;
    let invalid = || PortalError::InvalidPath(file_path.into());

    let mut current_parent = file_path.parent().ok_or_else(invalid)?.to_path_buf();

    let mut n = 0u32;

    loop {
        let candidate = if dir {
            let name = file_path.file_name().ok_or_else(invalid)?;
            current_parent.join(utils::candidate_name(name, None, n))
        } else {
            let stem: &OsStr = file_path.file_stem().ok_or_else(invalid)?;
            let ext: Option<&OsStr> = file_path.extension();
            current_parent.join(utils::candidate_name(stem, ext, n))
        };

        let io_error = if dir {
            tokio::fs::create_dir(&candidate).await.err()
        } else {
            tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
                .await
                .err()
        };

        match io_error {
            None => return Ok(candidate),
            Some(e) => match e.kind() {
                ErrorKind::AlreadyExists if n < MAX_ATTEMPTS => n += 1,

                ErrorKind::NotFound => {
                    if current_parent == fallback_dir {
                        return Err(e.into());
                    }
                    if let Some(p) = current_parent.parent() {
                        current_parent = p.to_path_buf();
                    } else {
                        current_parent = fallback_dir.to_path_buf();
                    }
                    n = 0;
                }

                ErrorKind::PermissionDenied => {
                    if current_parent == fallback_dir {
                        return Err(e.into());
                    }
                    current_parent = fallback_dir.to_path_buf();
                    n = 0;
                }

                _ => return Err(e.into()),
            },
        }
    }
}

async fn read_picker_output(out_path: &Path) -> Result<Vec<PathBuf>, PortalError> {
    let raw: Vec<u8> = match tokio::fs::read(out_path).await {
        Ok(data) => data,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(PortalError::NoPickerOutput);
        }
        Err(e) => return Err(e.into()),
    };

    raw.split(|&b| b == b'\n')
        .filter(|s| !s.is_empty())
        .map(|bytes| {
            let path = PathBuf::from(OsStr::from_bytes(bytes));
            if path.is_absolute() {
                Ok(path)
            } else {
                Err(PortalError::InvalidPickerOutput(format!(
                    "picker output is not absolute: {}",
                    path.display()
                )))
            }
        })
        .collect()
}

fn encode_response(uris: Vec<String>) -> Result<FileChooserResult, PortalError> {
    tracing::info!("Sending response, response={uris:?}");
    let value = Value::from(uris)
        .try_into()
        .map_err(|e| PortalError::Response(format!("failed to encode uris: {e}")))?;
    Ok(HashMap::from([("uris".into(), value)]))
}

pub struct FileChooser {
    config: Arc<Config>,
    state: Arc<State>,
}

impl FileChooser {
    const INTERFACE: &str = "FileChooser";

    pub fn try_new() -> Result<Self, PortalError> {
        let config_path = get_config_path()?;
        let state_path = get_state_path()?;

        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let config = Arc::new(Config::load(&config_path)?);
        let state = Arc::new(State::load(state_path));

        Ok(Self { config, state })
    }

    /// FileChooser OpenFile : launch user-defined tty command and return selected URIs.
    /// Supported keys in the options vardict include:
    /// handle_token (s), accept_label (s), modal (b), multiple (b), directory (b),
    /// filters (a(sa(us))), current_filter ((sa(us))), choices (a(ssa(ss)s)),
    /// current_folder (ay)
    #[tracing::instrument(skip_all, fields(%token))]
    async fn open_file_impl(
        &self,
        token: &str,
        app_id: &str,
        window_identifier: &str,
        title: &str,
        options: FileChooserOptions,
    ) -> Result<FileChooserResult, PortalError> {
        const METHOD_NAME: &str = "OpenFile";
        tracing::info!("Received {METHOD_NAME} request, app_id={app_id}, token={token}");

        let cfg = self
            .config
            .get_filechooser_config(Self::INTERFACE, METHOD_NAME, app_id)?;

        let default_dir = cfg
            .get_default_dir()
            .filter(|p| p.is_absolute())
            .unwrap_or_else(std::env::temp_dir);

        let (last, last_app) = self.state.get_last_dir(app_id);
        let last_dir = last.unwrap_or_else(|| default_dir.clone());
        let app_last_dir = last_app.unwrap_or_else(|| default_dir.clone());
        let last_path = last_dir.clone();
        let app_last_path = app_last_dir.clone();

        let suggested_path = options
            .current_folder_path()
            .unwrap_or_else(|| default_dir.clone());

        let temp_dir = tempfile::Builder::new()
            .prefix(crate::PREFIX_TEMP)
            .tempdir()?;
        let mut ctx = FileChooserContext {
            method: METHOD_NAME.to_string(),
            app_id: app_id.to_string(),
            window_id: window_identifier.to_string(),
            title: title.to_string(),
            last_dir,
            app_last_dir,
            suggested_path: suggested_path.clone(),
            last_path,
            app_last_path,
            default_dir,
            ..FileChooserContext::new(temp_dir.path())?
        };

        ctx.in_path = cfg.get_in_path(&ctx, &options)?;
        tracing::trace!("Built context: {ctx:?}");

        let req_json = ctx.build_json(&options);
        tokio::fs::write(&ctx.req_path, &req_json)
            .await
            .map_err(PortalError::Io)?;
        tracing::debug!("Serialized original request in file://{}", ctx.req_path.display());

        let LaunchArgs {
            term_prog,
            term_args,
            exec_args,
            envs,
        } = cfg.build_args(&ctx, &options)?;
        tracing::debug!(
            "Launching picker command, cmd={term_prog:?} {term_args:?} {exec_args:?}, env={envs:?}"
        );

        let status = Command::new(term_prog)
            .args(term_args)
            .args(exec_args)
            .envs(envs)
            .status()
            .await?;

        if !status.success() {
            tracing::warn!("Picker command failed, deferring to response check, status={status:?}");
        }

        let picker_output = read_picker_output(&ctx.out_path).await?;

        if picker_output.is_empty() {
            return Err(PortalError::EmptyPickerOutput);
        }

        if let Some(dir) = picker_output[0].parent()
            && let Err(e) = self.state.set_last_dir(app_id, dir)
        {
            tracing::warn!("Failed to save last_dir for {app_id}: {e}");
        }

        let uris = picker_output
            .iter()
            .map(|p| utils::path_to_uri(p).expect("path is absolute"))
            .collect::<Vec<_>>();

        encode_response(uris)
    }

    /// FileChooser SaveFile : launch user-defined tty command and return selected URI.
    /// Supported keys in the options vardict include:
    /// handle_token (s), accept_label (s), modal (b),
    /// filters (a(sa(us))), current_filter ((sa(us))), choices (a(ssa(ss)s)),
    /// current_name (s), current_folder (ay), current_file (ay)
    #[tracing::instrument(skip_all, fields(%token))]
    async fn save_file_impl(
        &self,
        token: &str,
        app_id: &str,
        window_identifier: &str,
        title: &str,
        options: FileChooserOptions,
    ) -> Result<FileChooserResult, PortalError> {
        const METHOD_NAME: &str = "SaveFile";
        tracing::info!("Received {METHOD_NAME} request, app_id={app_id}, token={token}");

        let cfg = self
            .config
            .get_filechooser_config(Self::INTERFACE, METHOD_NAME, app_id)?;

        let default_dir = cfg
            .get_default_dir()
            .filter(|p| p.is_absolute())
            .unwrap_or_else(std::env::temp_dir);

        let suggested_path = options.current_file_path().unwrap_or_else(|| {
            let folder = options
                .current_folder_path()
                .unwrap_or_else(|| default_dir.clone());
            let name = options.current_name.as_deref().unwrap_or("output");
            folder.join(name)
        });
        let suggested_name = suggested_path
            .file_name()
            .ok_or_else(|| PortalError::InvalidRequest("no file name provided"))?;

        let (last, last_app) = self.state.get_last_dir(app_id);
        let last_dir = last.unwrap_or_else(|| default_dir.clone());
        let app_last_dir = last_app.unwrap_or_else(|| default_dir.clone());
        let last_path = last_dir.join(suggested_name);
        let app_last_path = app_last_dir.join(suggested_name);

        let temp_dir = tempfile::Builder::new()
            .prefix(crate::PREFIX_TEMP)
            .tempdir()?;
        let mut ctx = FileChooserContext {
            method: METHOD_NAME.to_string(),
            app_id: app_id.to_string(),
            window_id: window_identifier.to_string(),
            title: title.to_string(),
            last_dir,
            app_last_dir,
            suggested_path,
            last_path,
            app_last_path,
            default_dir,
            ..FileChooserContext::new(temp_dir.path())?
        };

        // create placeholder file
        let in_path = cfg.get_in_path(&ctx, &options)?;
        ctx.in_path = match in_path {
            Some(path) => Some(create_unique_path(&path, false, &ctx.default_dir).await?),
            None => None,
        };
        tracing::trace!("Built context: {ctx:?}");

        let req_json = ctx.build_json(&options);
        tokio::fs::write(&ctx.req_path, &req_json)
            .await
            .map_err(PortalError::Io)?;
        tracing::debug!("Serialized original request in file://{}", ctx.req_path.display());

        let LaunchArgs {
            term_prog,
            term_args,
            exec_args,
            envs,
        } = cfg.build_args(&ctx, &options)?;
        tracing::debug!(
            "Launching picker command, cmd={term_prog:?} {term_args:?} {exec_args:?}, env={envs:?}"
        );

        let status = Command::new(term_prog)
            .args(term_args)
            .args(exec_args)
            .envs(envs)
            .status()
            .await?;

        // remove placeholder file if empty
        if let Some(p) = &ctx.in_path {
            let _ = tokio::fs::remove_file(p).await;
        }

        if !status.success() {
            tracing::warn!("Picker command failed, deferring to response check, status={status:?}");
        }

        let picker_output = read_picker_output(&ctx.out_path).await?;
        let selected_file = match picker_output.as_slice() {
            [] => return Err(PortalError::EmptyPickerOutput),
            [path] => path,
            _ => {
                return Err(PortalError::InvalidPickerOutput(
                    "expected exactly one path".to_string(),
                ));
            }
        };

        if let Some(dir) = selected_file.parent()
            && let Err(e) = self.state.set_last_dir(app_id, dir)
        {
            tracing::warn!("Failed to save last_dir for {app_id}: {e}");
        }

        let uri = utils::path_to_uri(selected_file).expect("path is absolute");

        encode_response(vec![uri])
    }

    /// FileChooser SaveFiles : launch user-defined tty command and return selected URI.
    /// Supported keys in the options vardict include:
    /// handle_token (s), accept_label (s), modal (b),
    /// choices (a(ssa(ss)s)),
    /// current_folder (ay), files (aay)
    #[tracing::instrument(skip_all, fields(%token))]
    async fn save_files_impl(
        &self,
        token: &str,
        app_id: &str,
        window_identifier: &str,
        title: &str,
        options: FileChooserOptions,
    ) -> Result<FileChooserResult, PortalError> {
        const METHOD_NAME: &str = "SaveFiles";
        tracing::info!("Received {METHOD_NAME} request, app_id={app_id}, token={token}");

        let cfg = self
            .config
            .get_filechooser_config(Self::INTERFACE, METHOD_NAME, app_id)?;

        let default_dir = cfg
            .get_default_dir()
            .filter(|p| p.is_absolute())
            .unwrap_or_else(std::env::temp_dir);

        let suggested_path = options
            .current_folder_path()
            .unwrap_or_else(|| default_dir.join("output"));
        let suggested_name = suggested_path
            .file_name()
            .ok_or_else(|| PortalError::InvalidRequest("no folder name provided"))?;

        let (last, last_app) = self.state.get_last_dir(app_id);
        let last_dir = last.unwrap_or_else(|| default_dir.clone());
        let app_last_dir = last_app.unwrap_or_else(|| default_dir.clone());
        let last_path = last_dir.join(suggested_name);
        let app_last_path = app_last_dir.join(suggested_name);

        let temp_dir = tempfile::Builder::new()
            .prefix(crate::PREFIX_TEMP)
            .tempdir()?;
        let mut ctx = FileChooserContext {
            method: METHOD_NAME.to_string(),
            app_id: app_id.to_string(),
            window_id: window_identifier.to_string(),
            title: title.to_string(),
            last_dir,
            app_last_dir,
            suggested_path,
            last_path,
            app_last_path,
            default_dir,
            ..FileChooserContext::new(temp_dir.path())?
        };

        // create placeholder directory
        let in_path = cfg.get_in_path(&ctx, &options)?;
        ctx.in_path = match in_path {
            Some(path) => Some(create_unique_path(&path, true, &ctx.default_dir).await?),
            None => None,
        };
        tracing::trace!("Built context: {ctx:?}");

        let req_json = ctx.build_json(&options);
        tokio::fs::write(&ctx.req_path, &req_json)
            .await
            .map_err(PortalError::Io)?;
        tracing::debug!("Serialized original request in file://{}", ctx.req_path.display());

        // list of files to save
        let filenames = options.files_filenames().unwrap_or_default();
        if filenames.is_empty() {
            tracing::warn!("No file to save, rejecting");
            return Err(PortalError::InvalidRequest("no files to save"));
        }

        let LaunchArgs {
            term_prog,
            term_args,
            exec_args,
            envs,
        } = cfg.build_args(&ctx, &options)?;
        tracing::debug!(
            "Launching picker command, cmd={term_prog:?} {term_args:?} {exec_args:?}, env={envs:?}"
        );

        let status = Command::new(term_prog)
            .args(term_args)
            .args(exec_args)
            .envs(envs)
            .status()
            .await?;

        // remove placeholder directory if empty
        if let Some(p) = &ctx.in_path
            && let Ok(m) = tokio::fs::metadata(p).await
            && m.len() == 0
        {
            let _ = tokio::fs::remove_dir(p).await;
        }

        if !status.success() {
            tracing::warn!("Picker command failed, deferring to picker output, status={status:?}");
        }

        let picker_output = read_picker_output(&ctx.out_path).await?;
        let selected_folder = match picker_output.as_slice() {
            [] => return Err(PortalError::EmptyPickerOutput),
            [path] => path,
            _ => {
                return Err(PortalError::InvalidPickerOutput(
                    "expected exactly one path".to_string(),
                ));
            }
        };

        if let Some(dir) = selected_folder.parent()
            && let Err(e) = self.state.set_last_dir(app_id, dir)
        {
            tracing::warn!("Failed to save last_dir for {app_id}: {e}");
        }

        let uris = filenames
            .iter()
            .map(|filename| selected_folder.join(filename))
            .map(|p| utils::path_to_uri(&p).expect("path is absolute"))
            .collect::<Vec<_>>();

        encode_response(uris)
    }
}

fn parse_handle(handle: &OwnedObjectPath) -> Option<(zbus::names::UniqueName<'static>, &str)> {
    let handle_str = handle.as_str();
    let handle_parts: Vec<&str> = handle_str.split('/').collect();

    // /org/freedesktop/portal/desktop/request/SENDER/TOKEN
    if handle_parts.len() != 8 || handle_parts[5] != "request" {
        tracing::debug!("Unexpected request handle: {handle_str}");
        return None;
    }

    let sender_str = format!(":{}", handle_parts[6].replace('_', "."));
    let sender = match zbus::names::UniqueName::try_from(sender_str.as_str()) {
        Ok(s) => s.to_owned(),
        Err(_) => {
            tracing::debug!("Failed to parse sender '{sender_str}' from request handle");
            return None;
        }
    };

    Some((sender, handle_parts[7]))
}

async fn read_proc(
    connection: &zbus::Connection,
    sender: &zbus::names::UniqueName<'_>,
) -> Option<String> {
    let dbus_proxy = zbus::fdo::DBusProxy::new(connection).await.ok()?;
    let pid = dbus_proxy
        .get_connection_unix_process_id(sender.clone().into())
        .await
        .ok()?;

    let comm = tokio::fs::read_to_string(format!("/proc/{pid}/comm"))
        .await
        .ok()?
        .trim()
        .to_string();

    Some(comm)
}

async fn parse_request<'h>(
    connection: &zbus::Connection,
    handle: &'h OwnedObjectPath,
    app_id: &str,
) -> Option<(&'h str, String)> {
    let (sender, token) = parse_handle(handle)?;

    let resolved_app_id = if !app_id.is_empty() {
        app_id.to_string()
    } else {
        match read_proc(connection, &sender).await {
            Some(resolved) => {
                tracing::debug!(%token, "Resolved empty app_id to '{resolved}' via sender {sender}");
                resolved
            }
            None => {
                tracing::debug!(%token, "Failed to resolve empty app_id for sender {sender}");
                String::new()
            }
        }
    };

    Some((token, resolved_app_id))
}

#[tracing::instrument(skip_all, fields(%token))]
async fn handle_response(
    token: &str,
    fut: impl std::future::Future<Output = Result<FileChooserResult, PortalError>>,
) -> zbus::fdo::Result<(Response, FileChooserResult)> {
    match fut.await {
        Ok(results) => Ok((PortalResponse::SUCCESS, results)),
        Err(e @ (PortalError::NoPickerOutput | PortalError::EmptyPickerOutput)) => {
            tracing::info!("Request cancelled, {e}");
            Ok((PortalResponse::CANCELLED, HashMap::new()))
        }
        Err(e) => {
            tracing::error!("Request failed: {e}");
            Ok((PortalResponse::OTHER, HashMap::new()))
        }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    #[zbus(out_args("response", "results"))]
    async fn open_file(
        &self,
        #[zbus(connection)] connection: &zbus::Connection,
        handle: OwnedObjectPath,
        app_id: &str,
        window_identifier: &str,
        title: &str,
        options: FileChooserOptions,
    ) -> zbus::fdo::Result<(Response, FileChooserResult)> {
        let Some((token, resolved_app_id)) = parse_request(connection, &handle, app_id).await
        else {
            return Ok((PortalResponse::OTHER, HashMap::new()));
        };

        handle_response(
            token,
            self.open_file_impl(token, &resolved_app_id, window_identifier, title, options),
        )
        .await
    }

    #[zbus(out_args("response", "results"))]
    async fn save_file(
        &self,
        #[zbus(connection)] connection: &zbus::Connection,
        handle: OwnedObjectPath,
        app_id: &str,
        window_identifier: &str,
        title: &str,
        options: FileChooserOptions,
    ) -> zbus::fdo::Result<(Response, FileChooserResult)> {
        let Some((token, resolved_app_id)) = parse_request(connection, &handle, app_id).await
        else {
            return Ok((PortalResponse::OTHER, HashMap::new()));
        };

        handle_response(
            token,
            self.save_file_impl(token, &resolved_app_id, window_identifier, title, options),
        )
        .await
    }

    #[zbus(out_args("response", "results"))]
    async fn save_files(
        &self,
        #[zbus(connection)] connection: &zbus::Connection,
        handle: OwnedObjectPath,
        app_id: &str,
        window_identifier: &str,
        title: &str,
        options: FileChooserOptions,
    ) -> zbus::fdo::Result<(Response, FileChooserResult)> {
        let Some((token, resolved_app_id)) = parse_request(connection, &handle, app_id).await
        else {
            return Ok((PortalResponse::OTHER, HashMap::new()));
        };

        handle_response(
            token,
            self.save_files_impl(token, &resolved_app_id, window_identifier, title, options),
        )
        .await
    }
}
