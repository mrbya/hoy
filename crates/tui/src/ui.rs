//! Terminal rendering.
//!
//! [`draw`] is the single entry point called every frame. It is a pure
//! function: given a [`Frame`] and the current [`TuiState`] it produces
//! output and nothing else.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::state::{ChatMessage, ConnectionStatus, TuiState};

/// Width of the room list panel including its border.
const ROOM_PANEL_WIDTH: u16 = 22;

// ── Renderer ─────────────────--───────────────────────────────────────────────

/**
 * Renders the full TUI for one frame.
 *
 * Layout (top to bottom):
 * 1. Status bar — 1 line, no border
 * 2. Main area — room list + message view side by side
 * 3. Input bar — 3 lines with border
 *
 * # Arguments
 * - `frame`: ratatui frame to render into,
 * - `state`: current display state.
 *
 * # Panics
 * Should not panic, but can, if trying to render in too small a window.
 */
pub fn draw(frame: &mut Frame<'_>, state: &TuiState) {
    let area = frame.area();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    draw_status_bar(frame, state, *rows.first().expect("Failed to split frame"));
    draw_main(frame, state, *rows.get(1).expect("Failed to split frame"));
    draw_input(frame, state, *rows.get(2).expect("Failed to split frame"));
}

// ── Status bar ────────────────────────────────────────────────────────────────

/**
 * Renders the single-line status bar.
 *
 * Format: `hoy  │  <username>@#<room>  │  <status>  [notification]`
 *
 * # Arguments
 * - `frame`: render target,
 * - `state`: current display state,
 * - `area`: allocated area.
 */
fn draw_status_bar(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    let (status_style, status_label) = match state.status {
        ConnectionStatus::Connected => (Style::default().fg(Color::Green), "connected"),
        ConnectionStatus::Connecting => (Style::default().fg(Color::Yellow), "connecting..."),
        ConnectionStatus::Disconnected => (Style::default().fg(Color::Red), "disconnected"),
    };

    let identity = match (&state.username, &state.current_room) {
        (Some(u), Some(r)) => format!("{u} @ #{r}"),
        (Some(u), None) => u.clone(),
        _ => String::from("..."),
    };

    let notification_span = state.notification.as_deref().map_or_else(
        || Span::raw(""),
        |n| Span::styled(format!("  {n}"), Style::default().fg(Color::Yellow)),
    );

    let line = Line::from(vec![
        Span::styled("hoy", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  |  "),
        Span::raw(identity),
        Span::raw("  |  "),
        Span::styled(status_label, status_style),
        notification_span,
    ]);

    frame.render_widget(Paragraph::new(line), area);
}

// ── Main area ─────────────────────────────────────────────────────────────────

/**
 * Renders the main area: room list on the left, messages on the right.
 *
 * # Arguments
 * - `frame`: render target,
 * - `state`: current display state,
 * - `area`: allocated area.
 *
 * # Panics
 * Should not panic, but can, if trying to render in too small a window.
 */
fn draw_main(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(ROOM_PANEL_WIDTH), Constraint::Min(0)])
        .split(area);

    draw_rooms(
        frame,
        state,
        *cols.first().expect("Failed to split main area"),
    );
    draw_messages(
        frame,
        state,
        *cols.get(1).expect("Failed to split main area"),
    );
}

// ── Room list ─────────────────────────────────────────────────────────────────

/**
 * Renders the room list panel.
 *
 * The current room is highlighted in bold. If no `ListRooms` response has
 * been received yet, only the current room (if any) is shown.
 *
 * # Arguments
 * - `frame`: render target,
 * - `state`: current display state,
 * - `area`: allocated area.
 */
fn draw_rooms(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Rooms");

    let rooms: Vec<ListItem<'_>> = if state.rooms.is_empty() {
        state
            .current_room
            .iter()
            .map(|r| room_list_item(r, true))
            .collect()
    } else {
        state
            .rooms
            .iter()
            .map(|r| {
                let active = state.current_room.as_deref() == Some(r.as_str());
                room_list_item(r, active)
            })
            .collect()
    };

    frame.render_widget(List::new(rooms).block(block), area);
}

/// Builds a single room list item, highlighting it if it is the active room.
fn room_list_item(name: &str, active: bool) -> ListItem<'_> {
    let prefix = if active { "> " } else { "  " };
    let style = if active {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    ListItem::new(Line::from(Span::styled(format!("{prefix}{name}"), style)))
}

// ── Message view ──────────────────────────────────────────────────────────────

/**
 * Renders the message history for the current room.
 *
 * Shows as many messages as fit in the available height, respecting the
 * per-room scroll offset stored in [`TuiState`].
 *
 * # Arguments
 * - `frame`: render target,
 * - `state`: current display state,
 * - `area`: allocated area.
 */
fn draw_messages(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    let title = state
        .current_room
        .as_deref()
        .map_or_else(|| String::from("Messages"), |r| format!("#{r}"));

    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let messages = state.current_messages();
    let total = messages.len();
    let visible_height = usize::from(inner.height);
    let scroll = state.current_scroll();

    // Cap scroll so no empty window is shown above the first message.
    let max_scroll = total.saturating_add(scroll);
    let scroll = scroll.min(max_scroll);

    let end = total.saturating_sub(scroll);
    let start = end.saturating_sub(visible_height);

    let lines: Vec<Line<'_>> = messages
        .get(start..end)
        .unwrap_or_default()
        .iter()
        .map(render_message)
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

/// Converts one [`ChatMessage`] to a ratatui [`Line`].
fn render_message(msg: &ChatMessage) -> Line<'_> {
    match msg {
        ChatMessage::User { from, text } => Line::from(vec![
            Span::styled(
                format!("{from}: "),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(text.as_str()),
        ]),

        ChatMessage::System { text } => Line::from(Span::styled(
            format!("* {text}"),
            Style::default().fg(Color::DarkGray),
        )),
    }
}

// ── Input bar ─────────────────────────────────────────────────────────────────

/**
 * Renders the input bar and positions the cursor.
 *
 * The cursor is placed at the visual column matching [`TuiState::cursor_pos`].
 * Because the input may contain multi-byte characters, the cursor column is
 * derived from the count of Unicode scalar values before the byte offset
 * rather than the raw byte position.
 *
 * # Arguments
 * - `frame`: render target,
 * - `state`: current display state,
 * - `area`: allocated area.
 */
fn draw_input(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Message");
    let inner = block.inner(area);

    let display = format!("> {}", state.input);
    let para = Paragraph::new(display.as_str());
    frame.render_widget(para.block(block), area);

    // Position the cursor: prefix "> " + up to the byte cursor_pos.
    let char_offset: u16 = state
        .input
        .get(..state.cursor_pos)
        .unwrap_or_default()
        .chars()
        .count()
        .try_into()
        .unwrap_or(u16::MAX);

    let cursor_x = inner.x.saturating_add(2).saturating_add(char_offset);
    let cursor_y = inner.y;

    if cursor_x < inner.x.saturating_add(inner.width) {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
