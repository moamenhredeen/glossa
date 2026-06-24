//! GUI pages. Each page exposes a `view(&App) -> Element<Message>` function;
//! state lives on the top-level `App` and message handling is in `main`'s
//! `update`.

pub mod chrome;
pub mod palette;
pub mod search;
pub mod settings;
