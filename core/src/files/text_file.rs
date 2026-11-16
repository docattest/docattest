//! Reading and writing a whole text file, with failures worded for the user.

use std::fmt::Display;
use std::io;
use std::path::Path;

/// Why an action on a file failed, for a message: `cannot <action> <file>: <reason>`.
pub fn failure(action: &str, file: impl Display, err: &io::Error) -> String {
    format!("cannot {action} {file}: {err}")
}

/// The file's text, decoded with replacement; or why it cannot be read.
pub fn read_text(path: &Path) -> Result<String, String> {
    std::fs::read(path).map(decode).map_err(|e| failure("read", path.display(), &e))
}

/// Bytes as text: UTF-8, with each invalid sequence replaced by U+FFFD.
pub fn decode(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }
}

/// Write `text` to `path`, creating the directories it needs; or say why it cannot be written.
pub fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| failure("create", dir.display(), &e))?;
    }
    std::fs::write(path, text).map_err(|e| failure("write", path.display(), &e))
}
