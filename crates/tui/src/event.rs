//! TUI input event types and key mapping.
//!
//! [`AppEvent`](crate::event::AppEvent) abstracts over raw crossterm key events so `app.rs` does not
//! need to match on key codes directly.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// High-level app event derived from terminal key presses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    /// User requested to quit.
    Quit,
    /// User confirmed input (Ret).
    Submit,
    /// (Backspace).
    DeleteCharBack,
    /// (Delete).
    DeleteCharForward,
    /// Insert a character at the cursor position.
    InsertChar(char),
    /// Move cursor one char left.
    MoveCursorLeft,
    /// Move cursor one char right.
    MoveCursorRight,
    /// (Home).
    MoveCursorStart,
    /// (End).
    MoveCursorEnd,
    /// Move cursor up one line.
    ScrollUp,
    /// Move cursor down one line.
    ScrollDown,
    /// A terminal resize occured; need to re-draw canvas.
    Resize,
}
/**
 * Maps a raw crossterm [`KeyEvent`] to an [`AppEvent`].
 *
 * # Arguments
 * - `key`: raw key event from crossterm.
 *
 * # Returns
 * `Some(AppEvent)` for recognised keys, `None` for unhandled keys.
 */
#[must_use]
pub const fn map_key_event(key: KeyEvent) -> Option<AppEvent> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Char('c') if ctrl => Some(AppEvent::Quit),
        KeyCode::Char('a') if ctrl => Some(AppEvent::MoveCursorStart),
        KeyCode::Home => Some(AppEvent::MoveCursorStart),
        KeyCode::Char('e') if ctrl => Some(AppEvent::MoveCursorEnd),
        KeyCode::End => Some(AppEvent::MoveCursorEnd),
        KeyCode::Enter => Some(AppEvent::Submit),
        KeyCode::Backspace => Some(AppEvent::DeleteCharBack),
        KeyCode::Delete => Some(AppEvent::DeleteCharForward),
        KeyCode::Left => Some(AppEvent::MoveCursorLeft),
        KeyCode::Right => Some(AppEvent::MoveCursorRight),
        KeyCode::Up => Some(AppEvent::ScrollUp),
        KeyCode::Down => Some(AppEvent::ScrollDown),
        KeyCode::Char(c) => Some(AppEvent::InsertChar(c)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::event::{AppEvent, map_key_event};

    #[test]
    fn ctrl_c_maps_to_quit() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(map_key_event(key), Some(AppEvent::Quit));
    }

    #[test]
    fn printable_char_maps_to_insert() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(map_key_event(key), Some(AppEvent::InsertChar('a')));
    }

    #[test]
    fn unrecognised_key_maps_to_none() {
        let key = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
        assert_eq!(map_key_event(key), None);
    }
}
