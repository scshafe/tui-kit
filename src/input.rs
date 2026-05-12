//! Keyboard, mouse, and window-resize abstraction.
//!
//! Three types form the input surface:
//! - [`KeyEvent`]: keyboard-only events (characters, navigation, modifiers).
//! - [`MouseEvent`]: mouse-only events (clicks, drags, wheel, release).
//! - [`InputEvent`]: the union of the two plus window-resize.
//!
//! [`read_input_event`] blocks on the next crossterm event and translates it.
//! Designed to be called from the dedicated input thread spawned by
//! [`crate::input_thread::spawn`].
//!
//! **Stability:** consumed by c4tui key handling and by tui-kit's input
//! producer. Keep this module as a thin translation layer; app command meaning
//! belongs in keymaps and application dispatch.

use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent as CtKeyEvent, KeyEventKind, KeyModifiers,
    MouseButton, MouseEvent as CtMouseEvent, MouseEventKind,
};

/// Keyboard input. Mouse and resize events live in their own enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Char(char),
    Up,
    Down,
    Left,
    Right,
    Enter,
    Tab,
    Back,
    Esc,
    CtrlC,
    Unknown,
}

/// Mouse input in terminal-cell coordinates (1-indexed; the input thread adds
/// one to crossterm's 0-indexed columns and rows on translation). Conversion to
/// any normalized "canvas" coordinate system is the consumer's responsibility:
/// tui-kit owns terminal cells, not canvases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEvent {
    Click { x: u16, y: u16 },
    Drag { x: u16, y: u16 },
    WheelUp { x: u16, y: u16 },
    WheelDown { x: u16, y: u16 },
    Release,
}

/// Union of every input event the producer can deliver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize { cols: u16, rows: u16 },
}

/// Blocks until the next crossterm event translates to an [`InputEvent`].
/// Events that translate to `None` (key releases, unmapped mouse events,
/// platform-specific events we don't care about) are silently skipped.
pub fn read_input_event() -> Result<InputEvent> {
    loop {
        let event = event::read()?;
        if let Some(input) = translate_event(event) {
            return Ok(input);
        }
    }
}

fn translate_event(event: Event) -> Option<InputEvent> {
    match event {
        Event::Key(ct) => translate_key_event(ct).map(InputEvent::Key),
        Event::Mouse(ct) => translate_mouse(ct).map(InputEvent::Mouse),
        Event::Resize(cols, rows) => Some(InputEvent::Resize { cols, rows }),
        _ => None,
    }
}

fn translate_key_event(event: CtKeyEvent) -> Option<KeyEvent> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    Some(match (event.code, event.modifiers) {
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => KeyEvent::CtrlC,
        (KeyCode::Char('C'), m) if m.contains(KeyModifiers::CONTROL) => KeyEvent::CtrlC,
        (KeyCode::Char(c), _) => KeyEvent::Char(c),
        (KeyCode::Up, _) => KeyEvent::Up,
        (KeyCode::Down, _) => KeyEvent::Down,
        (KeyCode::Left, _) => KeyEvent::Left,
        (KeyCode::Right, _) => KeyEvent::Right,
        (KeyCode::Enter, _) => KeyEvent::Enter,
        (KeyCode::Tab, _) => KeyEvent::Tab,
        (KeyCode::BackTab, _) => KeyEvent::Tab,
        (KeyCode::Backspace, _) => KeyEvent::Back,
        (KeyCode::Esc, _) => KeyEvent::Esc,
        _ => KeyEvent::Unknown,
    })
}

fn translate_mouse(event: CtMouseEvent) -> Option<MouseEvent> {
    let x = event.column.saturating_add(1);
    let y = event.row.saturating_add(1);
    Some(match event.kind {
        MouseEventKind::Down(MouseButton::Left) => MouseEvent::Click { x, y },
        MouseEventKind::Drag(MouseButton::Left) => MouseEvent::Drag { x, y },
        MouseEventKind::Up(_) => MouseEvent::Release,
        MouseEventKind::ScrollUp => MouseEvent::WheelUp { x, y },
        MouseEventKind::ScrollDown => MouseEvent::WheelDown { x, y },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent as CtKeyEvent, KeyModifiers};

    #[test]
    fn ctrl_c_translates_to_ctrl_c() {
        let event = CtKeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(translate_key_event(event), Some(KeyEvent::CtrlC));
    }

    #[test]
    fn release_events_are_filtered() {
        let mut event = CtKeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        event.kind = KeyEventKind::Release;
        assert_eq!(translate_key_event(event), None);
    }

    #[test]
    fn arrow_keys_translate() {
        for (code, expected) in [
            (KeyCode::Up, KeyEvent::Up),
            (KeyCode::Down, KeyEvent::Down),
            (KeyCode::Left, KeyEvent::Left),
            (KeyCode::Right, KeyEvent::Right),
        ] {
            let event = CtKeyEvent::new(code, KeyModifiers::NONE);
            assert_eq!(translate_key_event(event), Some(expected));
        }
    }

    #[test]
    fn mouse_click_adds_one_for_one_indexing() {
        let event = CtMouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 9,
            row: 19,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(translate_mouse(event), Some(MouseEvent::Click { x: 10, y: 20 }));
    }

    #[test]
    fn unmapped_mouse_events_translate_to_none() {
        let event = CtMouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: 5,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(translate_mouse(event), None);
    }

    #[test]
    fn translate_event_returns_resize_directly() {
        assert_eq!(
            translate_event(Event::Resize(120, 40)),
            Some(InputEvent::Resize { cols: 120, rows: 40 })
        );
    }
}
