//! Custom window chrome for a borderless, transparent window:
//! a rounded opaque panel, a custom title bar (drag + min/max/close), and
//! transparent edge/corner resize handles overlaid via a `stack`.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, container, mouse_area, row, stack, text, Space};
use iced::window::Direction;
use iced::{border, color, Element, Length, Theme};

use crate::{Message, Screen};

/// Edge/corner hit thickness for resize handles, in pixels.
const HANDLE: f32 = 6.0;

/// Wrap the app content in the rounded panel + resize-handle overlay.
pub fn window<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    let panel = container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_style);

    // Transparent overlay: edges/corners capture resize drags; the center is a
    // plain Space that does not capture the mouse, so clicks fall through to the
    // panel below.
    let overlay = iced::widget::column![
        row![
            corner(Direction::NorthWest),
            h_edge(Direction::North),
            corner(Direction::NorthEast),
        ]
        .height(Length::Fixed(HANDLE)),
        row![
            v_edge(Direction::West),
            Space::new().width(Length::Fill).height(Length::Fill),
            v_edge(Direction::East),
        ]
        .height(Length::Fill),
        row![
            corner(Direction::SouthWest),
            h_edge(Direction::South),
            corner(Direction::SouthEast),
        ]
        .height(Length::Fixed(HANDLE)),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    stack![panel, overlay].into()
}

/// The custom title bar:
/// left = navigation icons (search, settings, back), center = draggable title,
/// right = window controls.
pub fn title_bar<'a>(
    active: Screen,
    can_go_back: bool,
    lang_selector: Element<'a, Message>,
) -> Element<'a, Message> {
    let nav = row![
        nav_icon("\u{1F50D}", Message::Navigate(Screen::Search), active == Screen::Search), // 🔍
        nav_icon("\u{2699}", Message::Navigate(Screen::Settings), active == Screen::Settings), // ⚙
        back_icon(can_go_back),
    ]
    .spacing(2);

    // The centered title doubles as the window-drag handle.
    let title = mouse_area(
        container(text("Glossa").size(14))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center),
    )
    .on_press(Message::WindowDrag);

    let controls = row![
        win_button("\u{2013}", Message::WindowMinimize), // – minimize
        win_button("\u{25A1}", Message::WindowMaximize), // □ maximize
        win_button("\u{2715}", Message::WindowClose),    // ✕ close
    ];

    container(
        row![nav, title, lang_selector, controls]
            .spacing(6)
            .height(Length::Fill)
            .align_y(Vertical::Center),
    )
    .height(Length::Fixed(36.0))
    .width(Length::Fill)
    .padding([0, 6])
    .into()
}

fn nav_icon(glyph: &'static str, msg: Message, active: bool) -> Element<'static, Message> {
    // No background — the active page is marked only by an accent glyph color.
    let label = if active {
        text(glyph).size(16).color(color!(0x4a90d9))
    } else {
        text(glyph).size(16)
    };
    button(label)
        .padding([4, 8])
        .style(button::text)
        .on_press(msg)
        .into()
}

/// Back button — disabled (no `on_press`) when there's no word history.
fn back_icon(enabled: bool) -> Element<'static, Message> {
    let btn = button(text("\u{2190}").size(16)) // ←
        .padding([4, 8])
        .style(button::text);
    if enabled {
        btn.on_press(Message::Back).into()
    } else {
        btn.into()
    }
}

fn win_button(glyph: &'static str, msg: Message) -> Element<'static, Message> {
    button(text(glyph).size(14))
        .padding([4, 12])
        .style(button::text)
        .on_press(msg)
        .into()
}

fn panel_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.base.color.into()),
        border: border::rounded(10.0),
        ..container::Style::default()
    }
}

// --- resize handles -------------------------------------------------------

fn h_edge(dir: Direction) -> Element<'static, Message> {
    resize_area(Length::Fill, Length::Fixed(HANDLE), dir)
}

fn v_edge(dir: Direction) -> Element<'static, Message> {
    resize_area(Length::Fixed(HANDLE), Length::Fill, dir)
}

fn corner(dir: Direction) -> Element<'static, Message> {
    resize_area(Length::Fixed(HANDLE), Length::Fixed(HANDLE), dir)
}

fn resize_area(w: Length, h: Length, dir: Direction) -> Element<'static, Message> {
    mouse_area(Space::new().width(w).height(h))
        .on_press(Message::WindowResize(dir))
        .into()
}
