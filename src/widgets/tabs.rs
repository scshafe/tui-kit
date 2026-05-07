//! Policy-light tab state and split-pane layout primitives.
//!
//! This module owns reusable mechanics only: selected tab state,
//! configurable tab actions, close/reorder requests, pane split sizing, and
//! inspectable layout results. Applications decide what a tab or pane means and
//! how requested close/reorder operations map into domain commands.

use std::collections::HashSet;

use ratatui::layout::Rect;

use crate::config::{ConfigError, Validate};
use crate::input::Key;
use crate::keymap::{KeyMap, KeyTrigger, SpecialKey};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TabId(pub String);

impl TabId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabItem<T = ()> {
    pub id: TabId,
    pub label: String,
    pub payload: T,
    pub closeable: bool,
}

impl<T> TabItem<T> {
    pub fn new(id: impl Into<String>, label: impl Into<String>, payload: T) -> Self {
        Self {
            id: TabId::new(id),
            label: label.into(),
            payload,
            closeable: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabReorderDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabAction {
    SelectPrevious,
    SelectNext,
    SelectFirst,
    SelectLast,
    CloseSelected,
    ReorderSelectedLeft,
    ReorderSelectedRight,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct TabConfig {
    pub allow_close_requests: bool,
    pub allow_reorder_requests: bool,
    pub wrap_selection: bool,
    pub actions: KeyMap<TabAction>,
}

impl TabConfig {
    pub fn explicit(actions: KeyMap<TabAction>) -> Self {
        Self {
            allow_close_requests: true,
            allow_reorder_requests: true,
            wrap_selection: true,
            actions,
        }
    }

    pub fn default_navigation() -> Self {
        let mut actions = KeyMap::new();
        actions
            .bind(
                KeyTrigger::Special(SpecialKey::Left),
                TabAction::SelectPrevious,
            )
            .bind(
                KeyTrigger::Special(SpecialKey::Right),
                TabAction::SelectNext,
            )
            .bind(KeyTrigger::Char('['), TabAction::SelectPrevious)
            .bind(KeyTrigger::Char(']'), TabAction::SelectNext)
            .bind(KeyTrigger::Char('x'), TabAction::CloseSelected);
        Self::explicit(actions)
    }
}

impl Default for TabConfig {
    fn default() -> Self {
        Self::default_navigation()
    }
}

impl Validate for TabConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.actions.is_empty() {
            return Err(ConfigError::new(
                "TabConfig.actions",
                "must install an explicit key policy; use TabConfig::default_navigation() for the built-in preset",
            ));
        }
        for binding in self.actions.bindings() {
            match &binding.command {
                TabAction::CloseSelected if !self.allow_close_requests => {
                    return Err(ConfigError::new(
                        "TabConfig.actions[].CloseSelected",
                        "cannot be bound when allow_close_requests is false",
                    ));
                }
                TabAction::ReorderSelectedLeft | TabAction::ReorderSelectedRight
                    if !self.allow_reorder_requests =>
                {
                    return Err(ConfigError::new(
                        "TabConfig.actions[].ReorderSelected",
                        "cannot be bound when allow_reorder_requests is false",
                    ));
                }
                TabAction::Custom(name) if name.trim().is_empty() => {
                    return Err(ConfigError::new(
                        "TabConfig.actions[].Custom",
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
pub enum TabOutcome {
    Continue,
    Selected(TabId),
    CloseRequested(TabId),
    ReorderRequested {
        id: TabId,
        direction: TabReorderDirection,
    },
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabViewport {
    pub selected_index: Option<usize>,
    pub selected_id: Option<TabId>,
    pub total_tabs: usize,
    pub can_select_previous: bool,
    pub can_select_next: bool,
}

#[derive(Debug, Clone)]
pub struct TabState<T = ()> {
    tabs: Vec<TabItem<T>>,
    config: TabConfig,
    selected_index: Option<usize>,
}

impl<T> TabState<T> {
    pub fn new(tabs: Vec<TabItem<T>>, config: TabConfig) -> Self {
        Self::try_new(tabs, config).unwrap_or_else(|err| panic!("invalid TabState: {err}"))
    }

    pub fn try_new(tabs: Vec<TabItem<T>>, config: TabConfig) -> Result<Self, ConfigError> {
        config.validate()?;
        validate_tabs(&tabs)?;
        let selected_index = if tabs.is_empty() { None } else { Some(0) };
        Ok(Self {
            tabs,
            config,
            selected_index,
        })
    }

    pub fn tabs(&self) -> &[TabItem<T>] {
        &self.tabs
    }

    pub fn config(&self) -> &TabConfig {
        &self.config
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn selected_tab(&self) -> Option<&TabItem<T>> {
        self.selected_index.and_then(|index| self.tabs.get(index))
    }

    pub fn handle_key(&mut self, key: Key) -> TabOutcome {
        if let Some(action) = self.config.actions.lookup(key) {
            self.handle_action(action)
        } else {
            TabOutcome::Continue
        }
    }

    pub fn handle_action(&mut self, action: TabAction) -> TabOutcome {
        match action {
            TabAction::SelectPrevious => self.select_relative(-1),
            TabAction::SelectNext => self.select_relative(1),
            TabAction::SelectFirst => self.select_index(0),
            TabAction::SelectLast => {
                if self.tabs.is_empty() {
                    TabOutcome::Continue
                } else {
                    self.select_index(self.tabs.len() - 1)
                }
            }
            TabAction::CloseSelected => {
                if !self.config.allow_close_requests {
                    return TabOutcome::Continue;
                }
                match self.selected_tab() {
                    Some(tab) if tab.closeable => TabOutcome::CloseRequested(tab.id.clone()),
                    _ => TabOutcome::Continue,
                }
            }
            TabAction::ReorderSelectedLeft => self.reorder_request(TabReorderDirection::Left),
            TabAction::ReorderSelectedRight => self.reorder_request(TabReorderDirection::Right),
            TabAction::Custom(name) => TabOutcome::Custom(name),
        }
    }

    pub fn viewport(&self) -> TabViewport {
        let total_tabs = self.tabs.len();
        let selected_id = self.selected_tab().map(|tab| tab.id.clone());
        TabViewport {
            selected_index: self.selected_index,
            selected_id,
            total_tabs,
            can_select_previous: total_tabs > 1
                && (self.config.wrap_selection || self.selected_index != Some(0)),
            can_select_next: total_tabs > 1
                && (self.config.wrap_selection || self.selected_index != Some(total_tabs - 1)),
        }
    }

    fn select_relative(&mut self, delta: isize) -> TabOutcome {
        let Some(current) = self.selected_index else {
            return TabOutcome::Continue;
        };
        if self.tabs.is_empty() {
            return TabOutcome::Continue;
        }
        let len = self.tabs.len() as isize;
        let mut next = current as isize + delta;
        if self.config.wrap_selection {
            next = next.rem_euclid(len);
        } else {
            next = next.clamp(0, len - 1);
        }
        self.select_index(next as usize)
    }

    fn select_index(&mut self, index: usize) -> TabOutcome {
        if index >= self.tabs.len() || self.selected_index == Some(index) {
            return TabOutcome::Continue;
        }
        self.selected_index = Some(index);
        TabOutcome::Selected(self.tabs[index].id.clone())
    }

    fn reorder_request(&self, direction: TabReorderDirection) -> TabOutcome {
        if !self.config.allow_reorder_requests {
            return TabOutcome::Continue;
        }
        let Some(tab) = self.selected_tab() else {
            return TabOutcome::Continue;
        };
        TabOutcome::ReorderRequested {
            id: tab.id.clone(),
            direction,
        }
    }
}

fn validate_tabs<T>(tabs: &[TabItem<T>]) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();
    for (index, tab) in tabs.iter().enumerate() {
        if tab.id.0.trim().is_empty() {
            return Err(ConfigError::new(
                format!("TabState.tabs[{index}].id"),
                "must not be empty",
            ));
        }
        if !seen.insert(tab.id.clone()) {
            return Err(ConfigError::new(
                format!("TabState.tabs[{index}].id"),
                "must be unique",
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PaneId(pub String);

impl PaneId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSizePolicy {
    Fixed(u16),
    Percentage(u16),
    Flex { min: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneNode<T = ()> {
    Leaf {
        id: PaneId,
        focus: Option<crate::focus::FocusId>,
        payload: T,
    },
    Split {
        axis: PaneSplitAxis,
        children: Vec<PaneChild<T>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneChild<T = ()> {
    pub size: PaneSizePolicy,
    pub node: PaneNode<T>,
}

impl<T> PaneChild<T> {
    pub fn new(size: PaneSizePolicy, node: PaneNode<T>) -> Self {
        Self { size, node }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneLayoutEntry {
    pub id: PaneId,
    pub focus: Option<crate::focus::FocusId>,
    pub area: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneLayout {
    pub root_area: Rect,
    pub entries: Vec<PaneLayoutEntry>,
}

#[derive(Debug, Clone)]
pub struct PaneLayoutEngine;

impl PaneLayoutEngine {
    pub fn layout<T>(root: &PaneNode<T>, area: Rect) -> Result<PaneLayout, ConfigError> {
        validate_panes(root)?;
        let mut entries = Vec::new();
        layout_node(root, area, &mut entries);
        Ok(PaneLayout {
            root_area: area,
            entries,
        })
    }
}

fn validate_panes<T>(root: &PaneNode<T>) -> Result<(), ConfigError> {
    let mut ids = HashSet::new();
    validate_pane_node(root, "PaneNode", &mut ids)
}

fn validate_pane_node<T>(
    node: &PaneNode<T>,
    path: &str,
    ids: &mut HashSet<PaneId>,
) -> Result<(), ConfigError> {
    match node {
        PaneNode::Leaf { id, .. } => {
            if id.0.trim().is_empty() {
                return Err(ConfigError::new(format!("{path}.id"), "must not be empty"));
            }
            if !ids.insert(id.clone()) {
                return Err(ConfigError::new(format!("{path}.id"), "must be unique"));
            }
        }
        PaneNode::Split { children, .. } => {
            if children.is_empty() {
                return Err(ConfigError::new(
                    format!("{path}.children"),
                    "split panes must contain at least one child",
                ));
            }
            let percentage_total: u16 = children
                .iter()
                .filter_map(|child| match child.size {
                    PaneSizePolicy::Percentage(pct) => Some(pct),
                    _ => None,
                })
                .sum();
            if percentage_total > 100 {
                return Err(ConfigError::new(
                    format!("{path}.children[].size"),
                    "percentage sizes in a split must not exceed 100",
                ));
            }
            for (index, child) in children.iter().enumerate() {
                match child.size {
                    PaneSizePolicy::Fixed(0) => {
                        return Err(ConfigError::new(
                            format!("{path}.children[{index}].size"),
                            "fixed pane size must be greater than zero",
                        ));
                    }
                    PaneSizePolicy::Percentage(0) => {
                        return Err(ConfigError::new(
                            format!("{path}.children[{index}].size"),
                            "percentage pane size must be greater than zero",
                        ));
                    }
                    _ => {}
                }
                validate_pane_node(&child.node, &format!("{path}.children[{index}].node"), ids)?;
            }
        }
    }
    Ok(())
}

fn layout_node<T>(node: &PaneNode<T>, area: Rect, entries: &mut Vec<PaneLayoutEntry>) {
    match node {
        PaneNode::Leaf { id, focus, .. } => entries.push(PaneLayoutEntry {
            id: id.clone(),
            focus: focus.clone(),
            area,
        }),
        PaneNode::Split { axis, children } => {
            let lengths = split_lengths(*axis, children, area);
            let mut cursor_x = area.x;
            let mut cursor_y = area.y;
            for (child, length) in children.iter().zip(lengths) {
                let child_area = match axis {
                    PaneSplitAxis::Horizontal => {
                        let rect = Rect::new(cursor_x, area.y, length, area.height);
                        cursor_x = cursor_x.saturating_add(length);
                        rect
                    }
                    PaneSplitAxis::Vertical => {
                        let rect = Rect::new(area.x, cursor_y, area.width, length);
                        cursor_y = cursor_y.saturating_add(length);
                        rect
                    }
                };
                layout_node(&child.node, child_area, entries);
            }
        }
    }
}

fn split_lengths<T>(axis: PaneSplitAxis, children: &[PaneChild<T>], area: Rect) -> Vec<u16> {
    let total = match axis {
        PaneSplitAxis::Horizontal => area.width,
        PaneSplitAxis::Vertical => area.height,
    };
    let mut lengths = vec![0; children.len()];
    let mut used = 0u16;
    let mut flex_indices = Vec::new();

    for (index, child) in children.iter().enumerate() {
        match child.size {
            PaneSizePolicy::Fixed(size) => lengths[index] = size.min(total.saturating_sub(used)),
            PaneSizePolicy::Percentage(percent) => {
                let requested = ((u32::from(total) * u32::from(percent)) / 100) as u16;
                lengths[index] = requested.min(total.saturating_sub(used));
            }
            PaneSizePolicy::Flex { .. } => flex_indices.push(index),
        }
        used = used.saturating_add(lengths[index]).min(total);
    }

    let mut remaining = total.saturating_sub(used);
    if !flex_indices.is_empty() {
        let mut remaining_flex = flex_indices.len() as u16;
        for index in flex_indices {
            let min = match children[index].size {
                PaneSizePolicy::Flex { min } => min,
                _ => 0,
            };
            let share = remaining.checked_div(remaining_flex).unwrap_or(0);
            let size = share.max(min).min(remaining);
            lengths[index] = size;
            remaining = remaining.saturating_sub(size);
            remaining_flex = remaining_flex.saturating_sub(1);
        }
    }

    let assigned: u16 = lengths.iter().copied().sum();
    if assigned < total {
        if let Some(last) = lengths.last_mut() {
            *last = last.saturating_add(total - assigned);
        }
    }
    lengths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabs() -> Vec<TabItem> {
        vec![
            TabItem::new("one", "One", ()),
            TabItem::new("two", "Two", ()),
            TabItem::new("three", "Three", ()),
        ]
    }

    #[test]
    fn tab_selection_wraps_with_explicit_policy() {
        let mut state = TabState::new(tabs(), TabConfig::default_navigation());

        assert_eq!(
            state.handle_action(TabAction::SelectPrevious),
            TabOutcome::Selected(TabId::new("three"))
        );
        assert_eq!(state.selected_index(), Some(2));
    }

    #[test]
    fn close_is_a_request_not_domain_policy() {
        let mut state = TabState::new(tabs(), TabConfig::default_navigation());

        assert_eq!(
            state.handle_action(TabAction::CloseSelected),
            TabOutcome::CloseRequested(TabId::new("one"))
        );
        assert_eq!(state.tabs().len(), 3);
    }

    #[test]
    fn disabled_close_binding_validates_loudly() {
        let mut actions = KeyMap::new();
        actions.bind(KeyTrigger::Char('x'), TabAction::CloseSelected);
        let mut config = TabConfig::explicit(actions);
        config.allow_close_requests = false;

        let err = config.validate().unwrap_err();
        assert_eq!(err.path, "TabConfig.actions[].CloseSelected");
    }

    #[test]
    fn duplicate_tab_ids_are_rejected() {
        let err = TabState::try_new(
            vec![
                TabItem::new("dup", "One", ()),
                TabItem::new("dup", "Two", ()),
            ],
            TabConfig::default_navigation(),
        )
        .unwrap_err();

        assert_eq!(err.path, "TabState.tabs[1].id");
    }

    #[test]
    fn pane_layout_is_inspectable_and_deterministic() {
        let root = PaneNode::Split {
            axis: PaneSplitAxis::Horizontal,
            children: vec![
                PaneChild::new(
                    PaneSizePolicy::Fixed(10),
                    PaneNode::Leaf {
                        id: PaneId::new("nav"),
                        focus: None,
                        payload: (),
                    },
                ),
                PaneChild::new(
                    PaneSizePolicy::Flex { min: 5 },
                    PaneNode::Leaf {
                        id: PaneId::new("main"),
                        focus: None,
                        payload: (),
                    },
                ),
            ],
        };

        let layout = PaneLayoutEngine::layout(&root, Rect::new(0, 0, 30, 5)).unwrap();

        assert_eq!(layout.entries.len(), 2);
        assert_eq!(layout.entries[0].area, Rect::new(0, 0, 10, 5));
        assert_eq!(layout.entries[1].area, Rect::new(10, 0, 20, 5));
    }

    #[test]
    fn pane_validation_rejects_ambiguous_percentages() {
        let root = PaneNode::Split {
            axis: PaneSplitAxis::Vertical,
            children: vec![
                PaneChild::new(
                    PaneSizePolicy::Percentage(60),
                    PaneNode::Leaf {
                        id: PaneId::new("a"),
                        focus: None,
                        payload: (),
                    },
                ),
                PaneChild::new(
                    PaneSizePolicy::Percentage(60),
                    PaneNode::Leaf {
                        id: PaneId::new("b"),
                        focus: None,
                        payload: (),
                    },
                ),
            ],
        };

        let err = PaneLayoutEngine::layout(&root, Rect::new(0, 0, 10, 10)).unwrap_err();
        assert_eq!(err.path, "PaneNode.children[].size");
    }
}
