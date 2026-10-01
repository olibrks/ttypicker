use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use url::Url;

/// Convert OS string to String
pub fn lossy_string<S: AsRef<OsStr>>(s: S) -> String {
    s.as_ref().to_string_lossy().into_owned()
}

/// Convert OS bytes to PathBuf
pub fn bytes_to_pathbuf(bytes: impl AsRef<[u8]>) -> PathBuf {
    let bytes = bytes.as_ref();
    PathBuf::from(OsStr::from_bytes(
        bytes.strip_suffix(b"\0").unwrap_or(bytes),
    ))
}

/// Convert OS bytes (nul-terminated per the XDG spec) to an OsString.
pub fn bytes_to_osstring(bytes: impl AsRef<[u8]>) -> OsString {
    let bytes = bytes.as_ref();
    OsStr::from_bytes(bytes.strip_suffix(b"\0").unwrap_or(bytes)).to_os_string()
}

/// Converts a path to a `file://` URI string.
pub fn path_to_uri(path: &Path) -> Option<String> {
    Url::from_file_path(path).map(|u| u.to_string()).ok()
}

/// Add '_x' to a filename
pub fn candidate_name(stem: &OsStr, ext: Option<&OsStr>, n: u32) -> OsString {
    let mut bytes = stem.as_bytes().to_vec();
    if n > 0 {
        bytes.push(b'_');
        bytes.extend_from_slice(n.to_string().as_bytes());
    }
    if let Some(e) = ext {
        bytes.push(b'.');
        bytes.extend_from_slice(e.as_bytes());
    }
    OsString::from_vec(bytes)
}
