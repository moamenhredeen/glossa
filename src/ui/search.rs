//! Search page: a centered reading column with a hero search box, a compact
//! language switcher, and clean rich-text entries.

use iced::alignment::{Horizontal, Vertical};
use iced::font::{self, Font};
use iced::widget::{
    button, column, combo_box, container, rich_text, row, scrollable, span, text, text_input, Space,
};
use iced::{color, Border, Color, Element, Length, Padding, Theme};

use glossa::db::LanguageInfo;
use glossa::model::entry::Entry;

use crate::{App, Message};

const MUTED: Color = color!(0x888888);
const ACCENT: Color = color!(0x2a9d8f); // links / related words — the only colored thing

/// A selectable headword language for the pick_list (name only).
#[derive(Debug, Clone, PartialEq)]
pub struct Lang {
    pub code: String,
    label: String,
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// Build the selectable language options from an edition's language list.
pub fn lang_options(langs: &[LanguageInfo]) -> Vec<Lang> {
    langs
        .iter()
        .map(|li| Lang {
            code: li.code.clone(),
            label: li.name.clone(),
        })
        .collect()
}

/// Find the option matching a language code.
pub fn find_lang(langs: &[LanguageInfo], code: &str) -> Option<Lang> {
    lang_options(langs).into_iter().find(|l| l.code == code)
}

/// The searchable, borderless headword-language switcher for the title bar.
/// A trailing ▾ caret signals it's a dropdown.
pub fn language_selector(app: &App) -> Element<'_, Message> {
    let combo = combo_box(
        &app.lang_state,
        "Language",
        app.lang_selected.as_ref(),
        |l: Lang| Message::LangSelected(l.code),
    )
    .width(Length::Fixed(140.0))
    .size(13.0)
    .padding([4, 8])
    .input_style(input_style);

    row![combo, text("\u{25BE}").size(12).color(MUTED)]
        .spacing(2)
        .align_y(Vertical::Center)
        .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    // Results, an empty hint, or a status message.
    let body: Element<Message> = if let Some(status) = &app.status {
        centered_hint(status.clone())
    } else if app.results.is_empty() {
        centered_hint("Press Ctrl+P to search a word.".to_string())
    } else {
        let mut col = column![]
            .spacing(28)
            .padding(Padding::from([0, 20]));
        for (i, entry) in app.results.iter().enumerate() {
            if i > 0 {
                col = col.push(divider());
            }
            col = col.push(entry_view(entry));
        }
        scrollable(col).height(Length::Fill).into()
    };

    let mut inner = column![].spacing(16).width(Length::Fill);
    if !app.history.is_empty() {
        inner = inner.push(breadcrumb(app));
    }
    inner = inner.push(body);

    container(inner)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .into()
}

/// Navigation trail: each previous word is a clickable crumb; the current word
/// is shown plain at the end.
fn breadcrumb(app: &App) -> Element<'_, Message> {
    let mut trail = row![].spacing(6).align_y(Vertical::Center);
    for (i, (word, _lang)) in app.history.iter().enumerate() {
        trail = trail.push(
            button(text(word.clone()).size(13).color(ACCENT))
                .style(button::text)
                .padding(0)
                .on_press(Message::GoToCrumb(i)),
        );
        trail = trail.push(text("\u{203A}").size(13).color(MUTED)); // ›
    }
    if let Some((word, _)) = &app.current_view {
        trail = trail.push(text(word.clone()).size(13).color(MUTED));
    }
    trail.into()
}

fn centered_hint(message: String) -> Element<'static, Message> {
    container(text(message).size(15).color(MUTED))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
}

/// Render a single entry: headword + POS, IPA, numbered senses, etymology.
fn entry_view(entry: &Entry) -> Element<'static, Message> {
    let bold = Font {
        weight: font::Weight::Bold,
        ..Font::default()
    };
    let italic = Font {
        style: font::Style::Italic,
        ..Font::default()
    };

    let mut col = column![].spacing(10);

    // Headword (left, large+bold) with POS pushed to the right (muted).
    col = col.push(
        row![
            text(entry.word.clone()).size(30).font(bold).width(Length::Fill),
            text(entry.pos.clone()).size(14).color(MUTED),
        ]
        .align_y(Vertical::Center),
    );

    // IPA, muted monospace.
    let ipas: Vec<String> = entry.sounds.iter().filter_map(|s| s.ipa.clone()).collect();
    if !ipas.is_empty() {
        col = col.push(
            text(ipas.join("   "))
                .size(14)
                .font(Font::MONOSPACE)
                .color(MUTED),
        );
    }

    // Senses.
    let mut n = 0;
    for sense in &entry.senses {
        if sense.glosses.is_empty() {
            continue;
        }
        n += 1;

        let mut sense_col =
            column![text(format!("{n}.  {}", sense.glosses.join("; "))).size(16)].spacing(6);

        // Examples: plain italic muted, indented.
        for ex in &sense.examples {
            if ex.text.is_empty() {
                continue;
            }
            sense_col = sense_col.push(
                container(
                    text(format!("\u{201C}{}\u{201D}", ex.text))
                        .size(14)
                        .font(italic)
                        .color(MUTED),
                )
                .padding(Padding::ZERO.left(18)),
            );
        }

        // Related words: own line, accent-colored clickable links.
        let words: Vec<String> = sense
            .links
            .iter()
            .filter_map(|l| l.first().cloned())
            .filter(|w| !w.is_empty())
            .collect();
        if !words.is_empty() {
            let mut spans = vec![span("related   ").size(13).color(MUTED)];
            for (j, word) in words.iter().enumerate() {
                if j > 0 {
                    spans.push(span(" · ").size(13).color(MUTED));
                }
                spans.push(span(word.clone()).size(13).color(ACCENT).link(word.clone()));
            }
            sense_col = sense_col.push(
                container(rich_text(spans).on_link_click(Message::LinkClicked))
                    .padding(Padding::ZERO.left(18)),
            );
        }

        col = col.push(sense_col);
    }

    // Etymology: small muted italic, no label.
    if let Some(ety) = &entry.etymology_text {
        if !ety.is_empty() {
            col = col.push(text(ety.clone()).size(13).font(italic).color(MUTED));
        }
    }

    col.into()
}

/// A thin horizontal divider between entries.
fn divider() -> Element<'static, Message> {
    container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(theme.extended_palette().background.strong.color.into()),
            ..container::Style::default()
        })
        .into()
}

// --- styles ---------------------------------------------------------------

fn input_style(theme: &Theme, _status: text_input::Status) -> text_input::Style {
    let palette = theme.extended_palette();
    text_input::Style {
        background: Color::TRANSPARENT.into(),
        border: Border::default(),
        icon: palette.background.weak.text,
        placeholder: MUTED,
        value: palette.background.base.text,
        selection: palette.primary.weak.color,
    }
}
