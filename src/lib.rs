//! Shared library for the Glossa client: data model, catalog, on-disk
//! library, SQLite access, and the in-app importer. The GUI binary (`main.rs`)
//! builds its UI on top of these; nothing here depends on `iced`.

pub mod db;
pub mod importer;
pub mod paths;

pub mod model;
