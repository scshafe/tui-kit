//! Declarative key bindings.
//!
//! [`KeyMap<C>`] maps [`KeyTrigger`]s to a user-defined command type `C`.
//! `C` is typically an enum the application defines for its own commands.
//! Last binding wins, so user overrides applied after `defaults()` take
//! precedence.
//!
//! **Stability:** consumed by c4tui via `KeyMap<PendingCommand>`. tui-kit owns
//! trigger-to-command lookup mechanics only; default bindings and command
//! semantics stay in applications.

use crate::input::KeyEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyTrigger {
    Char(char),
    CharCaseInsensitive(char),
    Special(SpecialKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialKey {
    Up,
    Down,
    Left,
    Right,
    Back,
    Enter,
    Tab,
    Esc,
    CtrlC,
}

#[derive(Debug, Clone)]
pub struct KeyBinding<C: Clone> {
    pub trigger: KeyTrigger,
    pub command: C,
}

#[derive(Debug, Clone)]
pub struct KeyMap<C: Clone> {
    bindings: Vec<KeyBinding<C>>,
}

impl<C: Clone> Default for KeyMap<C> {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }
}

impl<C: Clone> KeyMap<C> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(&mut self, trigger: KeyTrigger, command: C) -> &mut Self {
        self.bindings.retain(|b| b.trigger != trigger);
        self.bindings.push(KeyBinding { trigger, command });
        self
    }

    pub fn lookup(&self, key: KeyEvent) -> Option<C> {
        self.bindings
            .iter()
            .rev()
            .find(|binding| binding.trigger.matches(key))
            .map(|binding| binding.command.clone())
    }

    pub fn bindings(&self) -> &[KeyBinding<C>] {
        &self.bindings
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl KeyTrigger {
    pub fn matches(self, key: KeyEvent) -> bool {
        match (self, key) {
            (Self::Char(want), KeyEvent::Char(got)) => want == got,
            (Self::CharCaseInsensitive(want), KeyEvent::Char(got)) => want.eq_ignore_ascii_case(&got),
            (Self::Special(want), key) => SpecialKey::from_key_event(key) == Some(want),
            _ => false,
        }
    }
}

impl SpecialKey {
    pub fn from_key_event(key: KeyEvent) -> Option<Self> {
        Some(match key {
            KeyEvent::Up => Self::Up,
            KeyEvent::Down => Self::Down,
            KeyEvent::Left => Self::Left,
            KeyEvent::Right => Self::Right,
            KeyEvent::Back => Self::Back,
            KeyEvent::Enter => Self::Enter,
            KeyEvent::Tab => Self::Tab,
            KeyEvent::Esc => Self::Esc,
            KeyEvent::CtrlC => Self::CtrlC,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Cmd {
        Quit,
        Up,
        Confirm,
    }

    fn map() -> KeyMap<Cmd> {
        let mut m = KeyMap::new();
        m.bind(KeyTrigger::CharCaseInsensitive('q'), Cmd::Quit);
        m.bind(KeyTrigger::Special(SpecialKey::Up), Cmd::Up);
        m.bind(KeyTrigger::Special(SpecialKey::Enter), Cmd::Confirm);
        m
    }

    #[test]
    fn case_insensitive_matches_both_cases() {
        let m = map();
        assert_eq!(m.lookup(KeyEvent::Char('q')), Some(Cmd::Quit));
        assert_eq!(m.lookup(KeyEvent::Char('Q')), Some(Cmd::Quit));
    }

    #[test]
    fn special_keys_match_exactly() {
        let m = map();
        assert_eq!(m.lookup(KeyEvent::Up), Some(Cmd::Up));
        assert_eq!(m.lookup(KeyEvent::Enter), Some(Cmd::Confirm));
    }

    #[test]
    fn last_binding_wins() {
        let mut m = map();
        m.bind(KeyTrigger::Char('q'), Cmd::Confirm);
        assert_eq!(m.lookup(KeyEvent::Char('q')), Some(Cmd::Confirm));
    }

    #[test]
    fn unmatched_key_returns_none() {
        assert!(map().lookup(KeyEvent::Char('z')).is_none());
    }
}
