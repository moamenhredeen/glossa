//! Round-trip test: build a tiny multi-language DB, then verify language-scoped
//! lookup, the languages metadata, and case-insensitive matching.

use glossa::db;
use glossa::model::entry::{Entry, Example, Sense, Sound};

fn insert(conn: &rusqlite::Connection, word: &str, lang: &str, gloss: &str) {
    let entry = Entry {
        word: word.to_string(),
        lang_code: lang.to_string(),
        pos: "noun".to_string(),
        etymology_text: None,
        senses: vec![Sense {
            glosses: vec![gloss.to_string()],
            examples: vec![Example {
                text: "An example.".to_string(),
            }],
            links: vec![vec!["related".to_string(), "related#X".to_string()]],
        }],
        sounds: vec![Sound {
            ipa: Some("/x/".to_string()),
        }],
    };
    let data = serde_json::to_string(&entry).unwrap();
    conn.execute(
        "INSERT INTO entries (word, word_norm, lang_code, pos, data) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![word, word.to_lowercase(), lang, "noun", data],
    )
    .unwrap();
}

#[test]
fn lookup_is_language_scoped() {
    let path = std::env::temp_dir().join("wick_test_lookup2.db");
    let _ = std::fs::remove_file(&path);

    let conn = db::open(&path).unwrap();
    db::init_schema(&conn).unwrap();

    insert(&conn, "Dog", "en", "A domesticated canine.");
    insert(&conn, "Hund", "de", "Ein Hund (German word, English gloss).");
    insert(&conn, "set", "en", "A collection.");

    // languages metadata (normally filled by the importer).
    conn.execute(
        "INSERT INTO languages (code, name, count) VALUES ('en','English',2),('de','German',1)",
        [],
    )
    .unwrap();
    db::build_index(&conn).unwrap();

    // Case-insensitive, language-scoped.
    let en = db::lookup(&conn, "en", "DOG").unwrap();
    assert_eq!(en.len(), 1);
    assert_eq!(en[0].word, "Dog");
    assert_eq!(en[0].senses[0].glosses[0], "A domesticated canine.");

    // Same word in the wrong language → nothing.
    assert!(db::lookup(&conn, "de", "dog").unwrap().is_empty());

    // German entry found only under "de".
    assert_eq!(db::lookup(&conn, "de", "hund").unwrap().len(), 1);

    // Language list, most populous first.
    let langs = db::list_languages(&conn).unwrap();
    assert_eq!(langs[0].code, "en");
    assert_eq!(langs[0].count, 2);
    assert_eq!(langs.len(), 2);

    let _ = std::fs::remove_file(&path);
}
