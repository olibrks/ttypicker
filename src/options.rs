use serde_json::json;
use std::ffi::OsString;
use std::path::PathBuf;
use zbus::zvariant::{DeserializeDict, Type};

use crate::utils::{bytes_to_osstring, bytes_to_pathbuf, lossy_string};

/// filter pattern (us)
/// Either a glob-style pattern (indicated by 0)
/// or a MIME type (indicated by 1) which allows wildcards for the subtype.
type FilterPattern = (u32, String);
/// file filter (a(us))
/// The first string is a user-visible name for the filter. The array specifies a list of filter patterns
type FileFilter = (String, Vec<FilterPattern>);
/// choice option (ss)
type ChoiceOption = (String, String);
/// combo box choice (ssa(ss))
type Choice = (String, String, Vec<ChoiceOption>);
/// file path (ay)
/// The path is using the same encoding as the file system. The byte array is expected to be terminated by a nul byte.
type PathBytes = Vec<u8>;
/// filename (ay)
/// The file name is using the same encoding as the file system. The byte array is expected to be terminated by a nul byte.
type FileNameBytes = Vec<u8>;

/// Convert a file filter to JSON
fn filter_to_json((name, rules): &FileFilter) -> serde_json::Value {
    let patterns: Vec<&str> = rules.iter().map(|(_, pattern)| pattern.as_str()).collect();
    json!([name, patterns])
}

/// Convert a choice combo box to JSON
fn choice_to_json((id, label, opts): &Choice) -> serde_json::Value {
    let opts: Vec<_> = opts.iter().map(|(k, v)| json!([k, v])).collect();
    json!([id, label, opts])
}

#[derive(DeserializeDict, Type, Debug)]
#[zvariant(signature = "dict")]
pub struct FileChooserOptions {
    /// accept_label (s)
    /// Label for the accept button. Mnemonic underlines are allowed.
    pub accept_label: Option<String>,
    /// modal (b)
    /// Whether the dialog should be modal. Default is yes.
    pub modal: Option<bool>,
    /// multiple (b)
    /// Whether multiple files can be selected or not. Default is single-selection.
    pub multiple: Option<bool>,
    /// directory (b)
    /// Whether to select for folders instead of files. Default is to select files.
    pub directory: Option<bool>,
    /// filters (a(sa(us)))
    /// List of serialized file filters.
    pub filters: Option<Vec<FileFilter>>,
    /// current_filter ((sa(us)))
    /// Request that this filter be set by default at dialog creation.
    pub current_filter: Option<FileFilter>,
    /// choices (a(ssa(ss)s))
    /// List of serialized combo boxes. The final string is the initial selection, or “”.
    pub choices: Option<(Vec<Choice>, String)>,
    /// current_name (s)
    /// Suggested name of the file.
    pub current_name: Option<String>,
    /// current_folder (ay)
    pub current_folder: Option<PathBytes>,
    /// files (aay)
    /// An array of file names to be saved.
    pub files: Option<Vec<FileNameBytes>>,
    /// current_file (ay)
    /// The current file (when saving an existing file).
    pub current_file: Option<PathBytes>,
}

impl FileChooserOptions {
    pub fn current_folder_path(&self) -> Option<PathBuf> {
        self.current_folder.as_deref().map(bytes_to_pathbuf)
    }

    pub fn current_file_path(&self) -> Option<PathBuf> {
        self.current_file.as_deref().map(bytes_to_pathbuf)
    }

    pub fn files_filenames(&self) -> Option<Vec<OsString>> {
        self.files
            .as_ref()
            .map(|list| list.iter().map(bytes_to_osstring).collect())
    }

    pub fn to_map(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut map = serde_json::Map::new();

        if let Some(v) = &self.accept_label {
            map.insert("accept_label".into(), json!(v));
        }
        if let Some(v) = &self.modal {
            map.insert("modal".into(), json!(v));
        }
        if let Some(v) = &self.multiple {
            map.insert("multiple".into(), json!(v));
        }
        if let Some(v) = &self.directory {
            map.insert("directory".into(), json!(v));
        }
        if let Some(list) = &self.filters {
            map.insert(
                "filters".into(),
                json!(list.iter().map(filter_to_json).collect::<Vec<_>>()),
            );
        }
        if let Some(filter) = &self.current_filter {
            map.insert("current_filter".into(), filter_to_json(filter));
        }
        if let Some((list, initial)) = &self.choices {
            map.insert(
                "choices".into(),
                json!([list.iter().map(choice_to_json).collect::<Vec<_>>(), initial]),
            );
        }
        if let Some(v) = &self.current_name {
            map.insert("current_name".into(), json!(v));
        }
        if let Some(p) = self.current_folder_path() {
            map.insert("current_folder".into(), json!(lossy_string(p)));
        }
        if let Some(p) = self.current_file_path() {
            map.insert("current_file".into(), json!(lossy_string(p)));
        }
        if let Some(paths) = self.files_filenames() {
            map.insert(
                "files".into(),
                json!(paths.iter().map(lossy_string).collect::<Vec<_>>()),
            );
        }
        map
    }
}
