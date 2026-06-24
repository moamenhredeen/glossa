//! Dictionary data model.
//!
//! Fields are a subset of the kaikki.org (Wiktextract) JSONL schema — unknown
//! fields are ignored on deserialize. The same structs are re-serialized into
//! the SQLite `data` column, so loading from the DB uses the exact same types.

use serde::{Deserialize, Serialize};

/// One dictionary entry (a word + part-of-speech combination).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Entry {
    pub word: String,
    /// A Wiktionary edition publishes hundreds of headword languages; we keep
    /// this so a single edition DB can be filtered by language at query time.
    #[serde(default)]
    pub lang_code: String,
    #[serde(default)]
    pub pos: String,
    #[serde(default)]
    pub etymology_text: Option<String>,
    #[serde(default)]
    pub senses: Vec<Sense>,
    #[serde(default)]
    pub sounds: Vec<Sound>,
}

/// A single sense (meaning) of an entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sense {
    #[serde(default)]
    pub glosses: Vec<String>,
    #[serde(default)]
    pub examples: Vec<Example>,
    /// kaikki `links`: each link is `[display, target]` (target may be empty).
    #[serde(default)]
    pub links: Vec<Vec<String>>,
}

/// A usage example for a sense.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Example {
    #[serde(default)]
    pub text: String,
}

/// Pronunciation information; we only keep IPA.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sound {
    #[serde(default)]
    pub ipa: Option<String>,
}
