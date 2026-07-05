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
    /// Grammatical tags for the headword as a whole, e.g. `"masculine"` for a
    /// German noun.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Inflected forms (a declension/conjugation table), when the source
    /// records one.
    #[serde(default)]
    pub forms: Vec<Form>,
}

impl Entry {
    /// The nominative-singular definite article for this headword, if the
    /// source data recorded one (e.g. German nouns: "der"/"die"/"das").
    pub fn article(&self) -> Option<&str> {
        self.forms
            .iter()
            .find(|f| {
                f.tags.iter().any(|t| t == "nominative") && f.tags.iter().any(|t| t == "singular")
            })
            .and_then(|f| f.article.as_deref())
    }
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

/// One inflected form — a single row of a declension or conjugation table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Form {
    #[serde(default)]
    pub form: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The definite article accompanying this form, when the language has one.
    #[serde(default)]
    pub article: Option<String>,
}
