//! Command palette: a centered overlay for navigating between pages.
//!
//! Opened with Ctrl/Cmd+K. It is driven entirely by global key events (handled
//! in `main`'s event subscription): type to filter, arrows to move, Enter or
//! click to run, Escape or a backdrop click to dismiss. Because it doesn't rely
//! on widget focus, the query is rendered into a styled "field" rather than a
//! real text input.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, mouse_area, text};
use iced::{Color, Element, Length, Padding, Theme};

use crate::{Message, Screen};

/// A single palette command. For now every command navigates to a page; the
/// struct leaves room to add other action kinds later.
pub struct Command {
    pub label: &'static str,
    pub target: Screen,
}

pub const COMMANDS: &[Command] = &[
    Command {
        label: "Go to Search",
        target: Screen::Search,
    },
    Command {
        label: "Go to Settings",
        target: Screen::Settings,
    },
];

/// Commands matching the (case-insensitive substring) query, in catalog order.
pub fn matches(query: &str) -> Vec<&'static Command> {
    let q = query.trim().to_lowercase();
    COMMANDS
        .iter()
        .filter(|c| q.is_empty() || c.label.to_lowercase().contains(&q))
        .collect()
}

/// The palette overlay (call only when open). Renders a query field plus an
/// arbitrary list of item labels; the caller supplies the items (commands or
/// word suggestions) and the placeholder.
pub fn view(
    query: &str,
    selected: usize,
    items: &[String],
    placeholder: &str,
) -> Element<'static, Message> {
    // Styled "field" showing the current query (or a placeholder).
    let field_text = if query.is_empty() {
        text(placeholder.to_string())
            .size(14)
            .color(Color::from_rgb(0.5, 0.5, 0.5))
    } else {
        text(query.to_string()).size(14)
    };
    let field = container(field_text)
        .width(Length::Fill)
        .padding(8)
        .style(field_style);

    let mut list = column![].spacing(2);
    for (i, label) in items.iter().enumerate() {
        let style = if i == selected {
            button::primary
        } else {
            button::text
        };
        list = list.push(
            button(text(label.clone()).size(14))
                .width(Length::Fill)
                .style(style)
                .on_press(Message::PaletteRunIndex(i)),
        );
    }

    let palette_box = container(column![field, list])
        .width(Length::Fixed(480.0))
        .style(box_style);

    // Backdrop: dim the rest and close on outside click; box sits near the top.
    mouse_area(
        container(palette_box)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Top)
            .padding(Padding::ZERO.top(90))
            .style(backdrop_style),
    )
    .on_press(Message::ClosePalette)
    .into()
}

fn box_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.base.color.into()),
        ..container::Style::default()
    }
}

fn field_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        ..container::Style::default()
    }
}

fn backdrop_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.4).into()),
        ..container::Style::default()
    }
}
