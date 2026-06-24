//! Filesystem locations for the app's data: the per-edition databases and the
//! persisted settings, all under the OS data directory.

use std::path::PathBuf;

/// `%APPDATA%/glossa` on Windows (platform data dir elsewhere).
/// Falls back to `./data` if no data dir can be determined.
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .map(|d| d.join("glossa"))
        .unwrap_or_else(|| PathBuf::from("data"))
}

/// Path to an edition's database, e.g. `<data_dir>/en.db`.
pub fn db_path(code: &str) -> PathBuf {
    data_dir().join(format!("{code}.db"))
}

/// Path to the persisted settings file.
pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

/// Ensure the data directory exists; returns it.
pub fn ensure_data_dir() -> std::io::Result<PathBuf> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
