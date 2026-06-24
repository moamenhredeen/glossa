//! Runtime view of what's installed on disk, plus persisted user selection
//! (active edition + active headword language).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::catalog::{self, Edition};
use crate::paths;

/// Persisted user selection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Code of the active edition, if any (e.g. "en").
    #[serde(default)]
    pub active_edition: Option<String>,
    /// Active headword language within the active edition (e.g. "en").
    #[serde(default)]
    pub active_lang: Option<String>,
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(paths::settings_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(dir) = paths::ensure_data_dir() {
            let _ = dir; // dir creation is the side effect we need
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(paths::settings_path(), json);
        }
    }
}

/// Which catalog editions are currently installed (have a DB on disk) plus the
/// persisted selection.
#[derive(Debug, Clone)]
pub struct Library {
    /// Edition codes present on disk.
    installed: Vec<String>,
    pub settings: Settings,
}

impl Library {
    /// Scan the data directory and load settings.
    pub fn load() -> Self {
        let installed = scan_installed(&paths::data_dir());
        Library {
            installed,
            settings: Settings::load(),
        }
    }

    /// Re-scan installed editions (call after an install/uninstall).
    pub fn rescan(&mut self) {
        self.installed = scan_installed(&paths::data_dir());
    }

    pub fn is_installed(&self, code: &str) -> bool {
        self.installed.iter().any(|c| c == code)
    }

    /// Installed editions, resolved against the catalog.
    pub fn installed_editions(&self) -> Vec<&'static Edition> {
        self.installed
            .iter()
            .filter_map(|c| catalog::edition(c))
            .collect()
    }

    pub fn active_edition(&self) -> Option<&'static Edition> {
        self.settings
            .active_edition
            .as_deref()
            .and_then(catalog::edition)
    }

    pub fn active_lang(&self) -> Option<&str> {
        self.settings.active_lang.as_deref()
    }

    pub fn set_active_edition(&mut self, code: &str) {
        self.settings.active_edition = Some(code.to_string());
        self.settings.save();
    }

    pub fn set_active_lang(&mut self, code: &str) {
        self.settings.active_lang = Some(code.to_string());
        self.settings.save();
    }

    /// Clear the active edition (e.g. after uninstalling it).
    pub fn clear_active_edition(&mut self) {
        self.settings.active_edition = None;
        self.settings.active_lang = None;
        self.settings.save();
    }
}

/// Find `*.db` files in `dir` whose stem matches a known edition code.
fn scan_installed(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("db") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            if catalog::edition(stem).is_some() {
                out.push(stem.to_string());
            }
        }
    }
    out.sort();
    out
}
