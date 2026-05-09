//! Keyboard and mouse abstraction. [`Key`] is the unified input enum
//! delivered to applications via [`crate::events::InputEvent::Key`].
//!
//! [`read_key`] blocks on the next crossterm event and translates it.
//! Designed to be called from the dedicated input thread spawned by
//! [`crate::input_thread::spawn`].
//!
//! **Stability:** consumed by c4tui key handling and by tui-kit's input
//! producer. Keep this module as a thin translation layer; app command meaning
//! belongs in keymaps and application dispatch.

use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
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
    MouseClick { x: u16, y: u16 },
    MouseWheelUp { x: u16, y: u16 },
    MouseWheelDown { x: u16, y: u16 },
    MouseDrag { x: u16, y: u16 },
    MouseRelease,
    Resize { cols: u16, rows: u16 },
    Unknown,
}

pub fn read_key() -> Result<Key> {
    loop {
        let event = event::read()?;
        if let Some(key) = translate_event(event) {
            return Ok(key);
        }
    }
}

fn translate_event(event: Event) -> Option<Key> {
    match event {
        Event::Key(key_event) => translate_key_event(key_event),
        Event::Mouse(mouse) => Some(translate_mouse(mouse)),
        Event::Resize(cols, rows) => Some(Key::Resize { cols, rows }),
        _ => None,
    }
}

fn translate_key_event(event: KeyEvent) -> Option<Key> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    Some(match (event.code, event.modifiers) {
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Key::CtrlC,
        (KeyCode::Char('C'), m) if m.contains(KeyModifiers::CONTROL) => Key::CtrlC,
        (KeyCode::Char(c), _) => Key::Char(c),
        (KeyCode::Up, _) => Key::Up,
        (KeyCode::Down, _) => Key::Down,
        (KeyCode::Left, _) => Key::Left,
        (KeyCode::Right, _) => Key::Right,
        (KeyCode::Enter, _) => Key::Enter,
        (KeyCode::Tab, _) => Key::Tab,
        (KeyCode::BackTab, _) => Key::Tab,
        (KeyCode::Backspace, _) => Key::Back,
        (KeyCode::Esc, _) => Key::Esc,
        _ => Key::Unknown,
    })
}

fn translate_mouse(event: MouseEvent) -> Key {
    let x = event.column.saturating_add(1);
    let y = event.row.saturating_add(1);
    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => Key::MouseClick { x, y },
        MouseEventKind::Drag(MouseButton::Left) => Key::MouseDrag { x, y },
        MouseEventKind::Up(_) => Key::MouseRelease,
        MouseEventKind::ScrollUp => Key::MouseWheelUp { x, y },
        MouseEventKind::ScrollDown => Key::MouseWheelDown { x, y },
        _ => Key::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    #[test]
    fn ctrl_c_translates_to_ctrl_c() {
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(translate_key_event(event), Some(Key::CtrlC));
    }

    #[test]
    fn release_events_are_filtered() {
        let mut event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        event.kind = KeyEventKind::Release;
        assert_eq!(translate_key_event(event), None);
    }

    #[test]
    fn arrow_keys_translate() {
        for (code, expected) in [
            (KeyCode::Up, Key::Up),
            (KeyCode::Down, Key::Down),
            (KeyCode::Left, Key::Left),
            (KeyCode::Right, Key::Right),
        ] {
            let event = KeyEvent::new(code, KeyModifiers::NONE);
            assert_eq!(translate_key_event(event), Some(expected));
        }
    }

    #[test]
    fn mouse_click_adds_one_for_one_indexing() {
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 9,
            row: 19,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(translate_mouse(event), Key::MouseClick { x: 10, y: 20 });
    }
}
