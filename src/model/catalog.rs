//! Static catalog of installable Wiktionary editions, plus a small map from
//! language codes to display names for the search-page language switcher.

/// A downloadable Wiktionary edition. `code` is the edition's gloss language;
/// the built database lives at `<data_dir>/{code}.db` and contains every
/// headword language that edition defines.
#[derive(Debug, Clone, Copy)]
pub struct Edition {
    /// Gloss-language code, e.g. "en". Also the DB filename stem.
    pub code: &'static str,
    /// Human-readable edition name, e.g. "English".
    pub name: &'static str,
    /// Full download URL of the `.jsonl.gz` dump.
    pub url: &'static str,
    /// Approximate compressed download size, for display.
    pub size: &'static str,
}

/// All editions the app knows how to install.
///
/// The English edition uses the combined raw Wiktextract dump; the others use
/// per-edition extracts. Base URL: `https://kaikki.org/dictionary/`.
pub const EDITIONS: &[Edition] = &[
    Edition {
        code: "en",
        name: "English",
        url: "https://kaikki.org/dictionary/raw-wiktextract-data.jsonl.gz",
        size: "2.6 GB",
    },
    Edition {
        code: "fr",
        name: "French",
        url: "https://kaikki.org/dictionary/downloads/fr/fr-extract.jsonl.gz",
        size: "672 MB",
    },
    Edition {
        code: "de",
        name: "German",
        url: "https://kaikki.org/dictionary/downloads/de/de-extract.jsonl.gz",
        size: "285 MB",
    },
    Edition {
        code: "ru",
        name: "Russian",
        url: "https://kaikki.org/dictionary/downloads/ru/ru-extract.jsonl.gz",
        size: "274 MB",
    },
    Edition {
        code: "zh",
        name: "Chinese",
        url: "https://kaikki.org/dictionary/downloads/zh/zh-extract.jsonl.gz",
        size: "213 MB",
    },
    Edition {
        code: "pl",
        name: "Polish",
        url: "https://kaikki.org/dictionary/downloads/pl/pl-extract.jsonl.gz",
        size: "124 MB",
    },
    Edition {
        code: "nl",
        name: "Dutch",
        url: "https://kaikki.org/dictionary/downloads/nl/nl-extract.jsonl.gz",
        size: "120 MB",
    },
    Edition {
        code: "es",
        name: "Spanish",
        url: "https://kaikki.org/dictionary/downloads/es/es-extract.jsonl.gz",
        size: "96 MB",
    },
    Edition {
        code: "ja",
        name: "Japanese",
        url: "https://kaikki.org/dictionary/downloads/ja/ja-extract.jsonl.gz",
        size: "57 MB",
    },
    Edition {
        code: "it",
        name: "Italian",
        url: "https://kaikki.org/dictionary/downloads/it/it-extract.jsonl.gz",
        size: "38 MB",
    },
    Edition {
        code: "pt",
        name: "Portuguese",
        url: "https://kaikki.org/dictionary/downloads/pt/pt-extract.jsonl.gz",
        size: "33 MB",
    },
];

/// Look up an edition by its code.
pub fn edition(code: &str) -> Option<&'static Edition> {
    EDITIONS.iter().find(|e| e.code == code)
}

/// Display name for a (headword) language code; falls back to the raw code.
pub fn lang_name(code: &str) -> &str {
    LANG_NAMES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| *name)
        .unwrap_or(code)
}

/// Common ISO-639 codes → English names. Not exhaustive; unknown codes show raw.
const LANG_NAMES: &[(&str, &str)] = &[
    ("en", "English"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("ru", "Russian"),
    ("pl", "Polish"),
    ("la", "Latin"),
    ("grc", "Ancient Greek"),
    ("el", "Greek"),
    ("ja", "Japanese"),
    ("zh", "Chinese"),
    ("ko", "Korean"),
    ("ar", "Arabic"),
    ("he", "Hebrew"),
    ("hi", "Hindi"),
    ("fa", "Persian"),
    ("tr", "Turkish"),
    ("sv", "Swedish"),
    ("no", "Norwegian"),
    ("da", "Danish"),
    ("fi", "Finnish"),
    ("cs", "Czech"),
    ("uk", "Ukrainian"),
    ("ga", "Irish"),
    ("cy", "Welsh"),
    ("eo", "Esperanto"),
    ("vi", "Vietnamese"),
];
