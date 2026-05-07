//! Policy-light scrollable list state and viewport math.
//!
//! [`ListState`] owns reusable list mechanics only: optional selection,
//! scroll offset, viewport calculation, wrapping/truncation policy, and
//! configurable key actions. Applications decide what a selection means and
//! map outcomes into domain commands.

use crate::config::{ConfigError, Validate};
use crate::input::Key;
use crate::keymap::{KeyMap, KeyTrigger, SpecialKey};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListItemId(pub String);

impl ListItemId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem<T = ()> {
    pub id: ListItemId,
    pub label: String,
    pub payload: T,
}

impl<T> ListItem<T> {
    pub fn new(id: impl Into<String>, label: impl Into<String>, payload: T) -> Self {
        Self {
            id: ListItemId::new(id),
            label: label.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListSelectionMode {
    None,
    Single,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListTextOverflow {
    Truncate,
    Wrap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListAction {
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    ScrollUp,
    ScrollDown,
    ScrollToTop,
    ScrollToBottom,
    Select,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct ListConfig {
    pub selection: ListSelectionMode,
    pub text_overflow: ListTextOverflow,
    pub page_stride: u16,
    pub actions: KeyMap<ListAction>,
}

impl ListConfig {
    pub fn explicit(actions: KeyMap<ListAction>) -> Self {
        Self {
            selection: ListSelectionMode::Single,
            text_overflow: ListTextOverflow::Truncate,
            page_stride: 10,
            actions,
        }
    }

    pub fn default_navigation() -> Self {
        let mut actions = KeyMap::new();
        actions
            .bind(KeyTrigger::Special(SpecialKey::Up), ListAction::MoveUp)
            .bind(KeyTrigger::Special(SpecialKey::Down), ListAction::MoveDown)
            .bind(KeyTrigger::Char('k'), ListAction::MoveUp)
            .bind(KeyTrigger::Char('j'), ListAction::MoveDown)
            .bind(KeyTrigger::Special(SpecialKey::Enter), ListAction::Select);
        Self::explicit(actions)
    }
}

impl Default for ListConfig {
    fn default() -> Self {
        Self::default_navigation()
    }
}

impl Validate for ListConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.page_stride == 0 {
            return Err(ConfigError::new(
                "ListConfig.page_stride",
                "must be greater than zero",
            ));
        }
        if self.actions.is_empty() {
            return Err(ConfigError::new(
                "ListConfig.actions",
                "must install an explicit key policy; use ListConfig::default_navigation() for the built-in preset",
            ));
        }
        for binding in self.actions.bindings() {
            if let ListAction::Custom(name) = &binding.command {
                if name.trim().is_empty() {
                    return Err(ConfigError::new(
                        "ListConfig.actions[].Custom",
                        "must not be empty",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListViewport {
    pub offset: usize,
    pub height: u16,
    pub start: usize,
    pub end_exclusive: usize,
    pub total_items: usize,
    pub selected_index: Option<usize>,
    pub can_scroll_up: bool,
    pub can_scroll_down: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListOutcome {
    Continue,
    Selected(ListItemId),
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct ListState<T = ()> {
    items: Vec<ListItem<T>>,
    config: ListConfig,
    selected_index: Option<usize>,
    scroll_offset: usize,
}

impl<T> ListState<T> {
    pub fn new(items: Vec<ListItem<T>>, config: ListConfig) -> Self {
        Self::try_new(items, config).unwrap_or_else(|err| panic!("invalid ListConfig: {err}"))
    }

    pub fn try_new(items: Vec<ListItem<T>>, config: ListConfig) -> Result<Self, ConfigError> {
        config.validate()?;
        let selected_index = match config.selection {
            ListSelectionMode::None => None,
            ListSelectionMode::Single if items.is_empty() => None,
            ListSelectionMode::Single => Some(0),
        };
        Ok(Self {
            items,
            config,
            selected_index,
            scroll_offset: 0,
        })
    }

    pub fn items(&self) -> &[ListItem<T>] {
        &self.items
    }

    pub fn config(&self) -> &ListConfig {
        &self.config
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn selected_item(&self) -> Option<&ListItem<T>> {
        self.selected_index.and_then(|index| self.items.get(index))
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn handle_key(&mut self, key: Key, viewport_height: u16) -> ListOutcome {
        if let Some(action) = self.config.actions.lookup(key) {
            self.handle_action(action, viewport_height)
        } else {
            ListOutcome::Continue
        }
    }

    pub fn handle_action(&mut self, action: ListAction, viewport_height: u16) -> ListOutcome {
        match action {
            ListAction::MoveUp => self.move_selection_or_scroll(-1, viewport_height),
            ListAction::MoveDown => self.move_selection_or_scroll(1, viewport_height),
            ListAction::PageUp => self.page(-1, viewport_height),
            ListAction::PageDown => self.page(1, viewport_height),
            ListAction::ScrollUp => self.scroll_by(-1, viewport_height),
            ListAction::ScrollDown => self.scroll_by(1, viewport_height),
            ListAction::ScrollToTop => {
                self.scroll_offset = 0;
                if self.config.selection == ListSelectionMode::Single && !self.items.is_empty() {
                    self.selected_index = Some(0);
                }
            }
            ListAction::ScrollToBottom => {
                let max_offset = self.max_offset(viewport_height);
                self.scroll_offset = max_offset;
                if self.config.selection == ListSelectionMode::Single && !self.items.is_empty() {
                    self.selected_index = Some(self.items.len() - 1);
                }
            }
            ListAction::Select => {
                if let Some(item) = self.selected_item() {
                    return ListOutcome::Selected(item.id.clone());
                }
            }
            ListAction::Custom(name) => return ListOutcome::Custom(name),
        }
        ListOutcome::Continue
    }

    pub fn viewport(&self, height: u16) -> ListViewport {
        let total_items = self.items.len();
        let max_offset = max_offset(total_items, height);
        let offset = self.scroll_offset.min(max_offset);
        let start = offset.min(total_items);
        let visible = usize::from(height);
        let end_exclusive = start.saturating_add(visible).min(total_items);
        ListViewport {
            offset,
            height,
            start,
            end_exclusive,
            total_items,
            selected_index: self.selected_index,
            can_scroll_up: offset > 0,
            can_scroll_down: offset < max_offset,
        }
    }

    pub fn set_scroll_offset(&mut self, offset: usize, viewport_height: u16) {
        self.scroll_offset = offset.min(self.max_offset(viewport_height));
    }

    fn move_selection_or_scroll(&mut self, delta: isize, viewport_height: u16) {
        if self.config.selection == ListSelectionMode::None {
            self.scroll_by(delta, viewport_height);
            return;
        }
        let Some(current) = self.selected_index else {
            if !self.items.is_empty() {
                self.selected_index = Some(0);
            }
            return;
        };
        let next = current
            .saturating_add_signed(delta)
            .min(self.items.len().saturating_sub(1));
        self.selected_index = Some(next);
        self.keep_selection_visible(viewport_height);
    }

    fn page(&mut self, direction: isize, viewport_height: u16) {
        let stride = usize::from(self.config.page_stride.min(viewport_height.max(1)));
        let delta = direction.saturating_mul(stride as isize);
        if self.config.selection == ListSelectionMode::Single {
            self.move_selection_or_scroll(delta, viewport_height);
        } else {
            self.scroll_by(delta, viewport_height);
        }
    }

    fn scroll_by(&mut self, delta: isize, viewport_height: u16) {
        let max_offset = self.max_offset(viewport_height);
        self.scroll_offset = self
            .scroll_offset
            .saturating_add_signed(delta)
            .min(max_offset);
    }

    fn keep_selection_visible(&mut self, viewport_height: u16) {
        let Some(selected) = self.selected_index else {
            return;
        };
        if viewport_height == 0 {
            self.scroll_offset = selected.min(self.max_offset(viewport_height));
            return;
        }
        let height = usize::from(viewport_height);
        if selected < self.scroll_offset {
            self.scroll_offset = selected;
        } else if selected >= self.scroll_offset.saturating_add(height) {
            self.scroll_offset = selected.saturating_add(1).saturating_sub(height);
        }
        self.scroll_offset = self.scroll_offset.min(self.max_offset(viewport_height));
    }

    fn max_offset(&self, viewport_height: u16) -> usize {
        max_offset(self.items.len(), viewport_height)
    }
}

pub fn max_offset(total_items: usize, viewport_height: u16) -> usize {
    total_items.saturating_sub(usize::from(viewport_height))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(count: usize) -> Vec<ListItem> {
        (0..count)
            .map(|idx| ListItem::new(format!("item-{idx}"), format!("Item {idx}"), ()))
            .collect()
    }

    #[test]
    fn validation_requires_explicit_actions() {
        let err = ListConfig::explicit(KeyMap::new()).validate().unwrap_err();
        assert_eq!(err.path, "ListConfig.actions");
    }

    #[test]
    fn selection_is_optional() {
        let mut config = ListConfig::default_navigation();
        config.selection = ListSelectionMode::None;
        let mut list = ListState::new(items(5), config);
        assert_eq!(list.selected_index(), None);
        list.handle_action(ListAction::MoveDown, 2);
        assert_eq!(list.scroll_offset(), 1);
    }

    #[test]
    fn selection_scrolls_into_view() {
        let mut list = ListState::new(items(10), ListConfig::default_navigation());
        for _ in 0..4 {
            list.handle_action(ListAction::MoveDown, 3);
        }
        assert_eq!(list.selected_index(), Some(4));
        assert_eq!(list.viewport(3).start, 2);
        assert_eq!(list.viewport(3).end_exclusive, 5);
    }

    #[test]
    fn viewport_reports_scrollability() {
        let mut list = ListState::new(items(4), ListConfig::default_navigation());
        assert!(!list.viewport(2).can_scroll_up);
        assert!(list.viewport(2).can_scroll_down);
        list.set_scroll_offset(99, 2);
        let viewport = list.viewport(2);
        assert_eq!(viewport.offset, 2);
        assert_eq!(viewport.start, 2);
        assert_eq!(viewport.end_exclusive, 4);
        assert!(!viewport.can_scroll_down);
    }

    #[test]
    fn select_outcome_returns_stable_id() {
        let mut list = ListState::new(items(3), ListConfig::default_navigation());
        list.handle_action(ListAction::MoveDown, 5);
        assert_eq!(
            list.handle_action(ListAction::Select, 5),
            ListOutcome::Selected(ListItemId::new("item-1"))
        );
    }

    #[test]
    fn key_actions_are_configurable() {
        let mut actions = KeyMap::new();
        actions.bind(
            KeyTrigger::Special(SpecialKey::Tab),
            ListAction::Custom("next-pane".into()),
        );
        let mut list = ListState::new(items(1), ListConfig::explicit(actions));
        assert_eq!(
            list.handle_key(Key::Tab, 1),
            ListOutcome::Custom("next-pane".into())
        );
    }
}
