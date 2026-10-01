use serde_json::json;
use std::path::{Path, PathBuf};

use crate::error::PortalError;
use crate::options::FileChooserOptions;

const FILENAME_OPTIONS: &str = "options.json";
const FILENAME_OUT: &str = "chooser.txt";

fn serialize_map(map: serde_json::Map<String, serde_json::Value>) -> String {
    serde_json::to_string_pretty(&serde_json::Value::Object(map))
        .unwrap_or_else(|_| json!({"error": "Failed to serialize"}).to_string())
}

#[derive(Debug, Default)]
pub struct FileChooserContext {
    pub method: String,
    pub app_id: String,
    pub window_id: String,
    pub title: String,

    pub req_path: PathBuf,
    pub out_path: PathBuf,
    pub in_path: Option<PathBuf>,

    pub home_dir: PathBuf,
    pub default_dir: PathBuf,
    pub last_dir: PathBuf,
    pub app_last_dir: PathBuf,

    pub suggested_path: PathBuf,
    pub last_path: PathBuf,
    pub app_last_path: PathBuf,
}

impl FileChooserContext {
    pub fn new(temp_dir: &Path) -> Result<Self, PortalError> {
        let home_dir = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                PortalError::Environment("can't determine home directory".to_string())
            })?;

        Ok(Self {
            req_path: temp_dir.join(FILENAME_OPTIONS),
            out_path: temp_dir.join(FILENAME_OUT),
            home_dir,
            ..Default::default()
        })
    }

    pub fn build_json(&self, opts: &FileChooserOptions) -> String {
        let mut map = serde_json::Map::new();
        map.insert("method".into(), json!(self.method));
        map.insert("app_id".into(), json!(self.app_id));
        map.insert("window_id".into(), json!(self.window_id));
        map.insert("title".into(), json!(self.title));
        map.insert("options".into(), serde_json::Value::Object(opts.to_map()));

        format!("{}\n", serialize_map(map))
    }
}
