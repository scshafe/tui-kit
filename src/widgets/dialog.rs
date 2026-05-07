//! Modal dialog widget state and rendering.
//!
//! [`Dialog`] stays ratatui-presentational: it renders title, message, and
//! footer text. [`DialogState`] owns reusable modal mechanics only: explicit
//! confirm/cancel key policy, optional focus trapping between dialog targets,
//! and machine-readable outcomes. Applications decide what confirmation or
//! cancellation means.

use crate::config::{ConfigError, Validate};
use crate::input::Key;
use crate::keymap::{KeyMap, KeyTrigger, SpecialKey};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DialogFocusId(pub String);

impl DialogFocusId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogAction {
    Confirm,
    Cancel,
    FocusNext,
    FocusPrevious,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogDismissPolicy {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogFocusPolicy {
    None,
    Trap { targets: Vec<DialogFocusId> },
}

#[derive(Debug, Clone)]
pub struct DialogConfig {
    pub confirm: DialogDismissPolicy,
    pub cancel: DialogDismissPolicy,
    pub focus: DialogFocusPolicy,
    pub actions: KeyMap<DialogAction>,
}

impl DialogConfig {
    pub fn explicit(actions: KeyMap<DialogAction>) -> Self {
        Self {
            confirm: DialogDismissPolicy::Enabled,
            cancel: DialogDismissPolicy::Enabled,
            focus: DialogFocusPolicy::None,
            actions,
        }
    }

    pub fn confirm_cancel() -> Self {
        let mut actions = KeyMap::new();
        actions
            .bind(
                KeyTrigger::Special(SpecialKey::Enter),
                DialogAction::Confirm,
            )
            .bind(KeyTrigger::Special(SpecialKey::Esc), DialogAction::Cancel);
        Self::explicit(actions)
    }

    pub fn confirm_cancel_trapped(targets: Vec<DialogFocusId>) -> Self {
        let mut config = Self::confirm_cancel();
        config.actions.bind(
            KeyTrigger::Special(SpecialKey::Tab),
            DialogAction::FocusNext,
        );
        config.focus = DialogFocusPolicy::Trap { targets };
        config
    }
}

impl Default for DialogConfig {
    fn default() -> Self {
        Self::confirm_cancel()
    }
}

impl Validate for DialogConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.actions.is_empty() {
            return Err(ConfigError::new(
                "DialogConfig.actions",
                "must install an explicit key policy; use DialogConfig::confirm_cancel() for the built-in preset",
            ));
        }

        if let DialogFocusPolicy::Trap { targets } = &self.focus {
            if targets.is_empty() {
                return Err(ConfigError::new(
                    "DialogConfig.focus.targets",
                    "focus trapping requires at least one target",
                ));
            }
            for (index, target) in targets.iter().enumerate() {
                if target.0.trim().is_empty() {
                    return Err(ConfigError::new(
                        format!("DialogConfig.focus.targets[{index}]"),
                        "focus target ID must not be empty",
                    ));
                }
                if targets[..index].iter().any(|seen| seen == target) {
                    return Err(ConfigError::new(
                        format!("DialogConfig.focus.targets[{index}]"),
                        "duplicate focus target ID",
                    ));
                }
            }
        }

        for binding in self.actions.bindings() {
            match &binding.command {
                DialogAction::Confirm if self.confirm == DialogDismissPolicy::Disabled => {
                    return Err(ConfigError::new(
                        "DialogConfig.actions[].Confirm",
                        "confirm action is bound while confirm policy is disabled",
                    ));
                }
                DialogAction::Cancel if self.cancel == DialogDismissPolicy::Disabled => {
                    return Err(ConfigError::new(
                        "DialogConfig.actions[].Cancel",
                        "cancel action is bound while cancel policy is disabled",
                    ));
                }
                DialogAction::FocusNext | DialogAction::FocusPrevious
                    if self.focus == DialogFocusPolicy::None =>
                {
                    return Err(ConfigError::new(
                        "DialogConfig.actions[].Focus",
                        "focus traversal action requires DialogFocusPolicy::Trap",
                    ));
                }
                DialogAction::Custom(name) if name.trim().is_empty() => {
                    return Err(ConfigError::new(
                        "DialogConfig.actions[].Custom",
                        "must not be empty",
                    ));
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogOutcome {
    Continue,
    Confirmed,
    Cancelled,
    FocusChanged(DialogFocusId),
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct DialogState {
    config: DialogConfig,
    focused_index: Option<usize>,
}

impl DialogState {
    pub fn new(config: DialogConfig) -> Self {
        Self::try_new(config).unwrap_or_else(|err| panic!("invalid DialogConfig: {err}"))
    }

    pub fn try_new(config: DialogConfig) -> Result<Self, ConfigError> {
        config.validate()?;
        let focused_index = match &config.focus {
            DialogFocusPolicy::None => None,
            DialogFocusPolicy::Trap { targets } if targets.is_empty() => None,
            DialogFocusPolicy::Trap { .. } => Some(0),
        };
        Ok(Self {
            config,
            focused_index,
        })
    }

    pub fn config(&self) -> &DialogConfig {
        &self.config
    }

    pub fn focused(&self) -> Option<&DialogFocusId> {
        self.focused_index
            .and_then(|index| self.focus_targets().get(index))
    }

    pub fn focus_targets(&self) -> &[DialogFocusId] {
        match &self.config.focus {
            DialogFocusPolicy::None => &[],
            DialogFocusPolicy::Trap { targets } => targets,
        }
    }

    pub fn handle_key(&mut self, key: Key) -> DialogOutcome {
        if let Some(action) = self.config.actions.lookup(key) {
            self.handle_action(action)
        } else {
            DialogOutcome::Continue
        }
    }

    pub fn handle_action(&mut self, action: DialogAction) -> DialogOutcome {
        match action {
            DialogAction::Confirm if self.config.confirm == DialogDismissPolicy::Enabled => {
                DialogOutcome::Confirmed
            }
            DialogAction::Cancel if self.config.cancel == DialogDismissPolicy::Enabled => {
                DialogOutcome::Cancelled
            }
            DialogAction::FocusNext => self.move_focus(1),
            DialogAction::FocusPrevious => self.move_focus(-1),
            DialogAction::Custom(name) => DialogOutcome::Custom(name),
            _ => DialogOutcome::Continue,
        }
    }

    fn move_focus(&mut self, direction: isize) -> DialogOutcome {
        let target_count = self.focus_targets().len();
        if target_count == 0 {
            return DialogOutcome::Continue;
        }
        let current = self.focused_index.unwrap_or(0);
        let next = if direction < 0 {
            current.checked_sub(1).unwrap_or(target_count - 1)
        } else {
            (current + 1) % target_count
        };
        self.focused_index = Some(next);
        DialogOutcome::FocusChanged(self.focus_targets()[next].clone())
    }
}

#[derive(Debug, Clone)]
pub struct Dialog {
    pub title: String,
    pub message: String,
    pub footer: String,
}

impl Dialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            footer: String::new(),
        }
    }

    pub fn with_footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = footer.into();
        self
    }
}

impl Widget for Dialog {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.title.trim()))
            .title_bottom(self.footer.clone());
        let inner = block.inner(area);
        block.render(area, buf);
        Paragraph::new(self.message.clone())
            .wrap(Wrap { trim: false })
            .render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_cancel_preset_maps_enter_and_escape() {
        let mut state = DialogState::new(DialogConfig::confirm_cancel());
        assert_eq!(state.handle_key(Key::Enter), DialogOutcome::Confirmed);
        assert_eq!(state.handle_key(Key::Esc), DialogOutcome::Cancelled);
    }

    #[test]
    fn focus_trap_cycles_targets() {
        let mut state = DialogState::new(DialogConfig::confirm_cancel_trapped(vec![
            DialogFocusId::new("cancel"),
            DialogFocusId::new("confirm"),
        ]));
        assert_eq!(state.focused(), Some(&DialogFocusId::new("cancel")));
        assert_eq!(
            state.handle_key(Key::Tab),
            DialogOutcome::FocusChanged(DialogFocusId::new("confirm"))
        );
        assert_eq!(
            state.handle_key(Key::Tab),
            DialogOutcome::FocusChanged(DialogFocusId::new("cancel"))
        );
    }

    #[test]
    fn validation_rejects_disabled_bound_confirm() {
        let mut config = DialogConfig::confirm_cancel();
        config.confirm = DialogDismissPolicy::Disabled;
        let err = config.validate().unwrap_err();
        assert_eq!(err.path, "DialogConfig.actions[].Confirm");
    }

    #[test]
    fn validation_rejects_focus_action_without_trap() {
        let mut actions = KeyMap::new();
        actions.bind(
            KeyTrigger::Special(SpecialKey::Tab),
            DialogAction::FocusNext,
        );
        let config = DialogConfig::explicit(actions);
        let err = config.validate().unwrap_err();
        assert_eq!(err.path, "DialogConfig.actions[].Focus");
    }
}
