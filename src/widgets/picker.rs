//! Generic list-with-detail-and-thumbnails picker.
//!
//! [`Picker<T>`] is a state machine: it owns items, a filter string,
//! a selection by id, and visibility flags per group. Apps drive it
//! by feeding [`crate::input::Key`]s via [`Picker::handle_key`].
//!
//! [`PickerWidget`] is a ratatui [`Widget`] that renders the picker into
//! a buffer, drawing a bordered box, group headers, items with optional
//! detail line, selection highlight, and scroll arrows. Each visible
//! item's thumbnail position (row, col) is recorded into a caller-
//! supplied vec so the caller can emit Kitty image placements for it
//! after `terminal.draw()` returns.
//!
//! See `c4tui::picker` for the application-side wiring.

use crate::input::Key;
use crate::keymap::{KeyMap, KeyTrigger, SpecialKey};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct PickerItem<T: Clone> {
    pub id: u64,
    pub primary: String,
    pub detail: Option<String>,
    pub group: Option<String>,
    pub searchable: Vec<String>,
    pub payload: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerAction {
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    Select,
    Cancel,
    ClearFilterOrCancel,
    BackspaceFilter,
    AppendFilterChar(char),
    ToggleGroup { group: String },
    ToggleSelectedItemGroup,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct PickerConfig {
    pub title: String,
    pub bottom_hint: String,
    pub thumb_cols: u16,
    pub thumb_rows: u16,
    pub item_row_span: u16,
    pub allow_thumbnails: bool,
    pub actions: KeyMap<PickerAction>,
}

impl PickerConfig {
    pub fn explicit(actions: KeyMap<PickerAction>) -> Self {
        Self {
            title: " Picker ".to_owned(),
            bottom_hint: " type → filter | Enter → select | Esc → cancel ".to_owned(),
            thumb_cols: 12,
            thumb_rows: 3,
            item_row_span: 3,
            allow_thumbnails: true,
            actions,
        }
    }

    pub fn default_navigation() -> Self {
        let mut actions = KeyMap::new();
        actions
            .bind(KeyTrigger::Special(SpecialKey::Up), PickerAction::MoveUp)
            .bind(
                KeyTrigger::Special(SpecialKey::Down),
                PickerAction::MoveDown,
            )
            .bind(KeyTrigger::Special(SpecialKey::Enter), PickerAction::Select)
            .bind(
                KeyTrigger::Special(SpecialKey::Esc),
                PickerAction::ClearFilterOrCancel,
            )
            .bind(KeyTrigger::Special(SpecialKey::CtrlC), PickerAction::Cancel)
            .bind(
                KeyTrigger::Special(SpecialKey::Back),
                PickerAction::BackspaceFilter,
            );
        Self::explicit(actions)
    }
}

impl Default for PickerConfig {
    fn default() -> Self {
        Self::default_navigation()
    }
}

#[derive(Debug)]
pub struct Picker<T: Clone> {
    items: Vec<PickerItem<T>>,
    config: PickerConfig,
    filter: String,
    hidden_groups: HashSet<String>,
    selected_id: u64,
    group_order: Vec<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerOutcome {
    Continue,
    Select(u64),
    Cancel,
    ToggleHiddenGroup(String),
    Custom(String),
}

impl<T: Clone> Picker<T> {
    pub fn new(items: Vec<PickerItem<T>>, config: PickerConfig, initial_selection: u64) -> Self {
        let mut group_order: Vec<Option<String>> = Vec::new();
        for item in &items {
            if !group_order.contains(&item.group) {
                group_order.push(item.group.clone());
            }
        }
        Self {
            items,
            config,
            filter: String::new(),
            hidden_groups: HashSet::new(),
            selected_id: initial_selection,
            group_order,
        }
    }

    pub fn config(&self) -> &PickerConfig {
        &self.config
    }

    pub fn items(&self) -> &[PickerItem<T>] {
        &self.items
    }

    pub fn selected_id(&self) -> u64 {
        self.selected_id
    }

    pub fn selected_payload(&self) -> Option<&T> {
        self.items
            .iter()
            .find(|i| i.id == self.selected_id)
            .map(|i| &i.payload)
    }

    pub fn set_group_hidden(&mut self, group: &str, hidden: bool) {
        if hidden {
            self.hidden_groups.insert(group.to_owned());
        } else {
            self.hidden_groups.remove(group);
        }
        self.clamp_selection();
    }

    pub fn is_group_hidden(&self, group: &str) -> bool {
        self.hidden_groups.contains(group)
    }

    pub fn handle_key(&mut self, key: Key) -> PickerOutcome {
        if let Some(action) = self.config.actions.lookup(key) {
            return self.handle_action(action);
        }
        match key {
            Key::Char(c) => self.handle_action(PickerAction::AppendFilterChar(c)),
            _ => PickerOutcome::Continue,
        }
    }

    pub fn append_filter_char(&mut self, c: char) -> PickerOutcome {
        self.filter.push(c);
        self.clamp_selection();
        PickerOutcome::Continue
    }

    pub fn handle_action(&mut self, action: PickerAction) -> PickerOutcome {
        match action {
            PickerAction::MoveUp => {
                self.move_selection(-1);
                PickerOutcome::Continue
            }
            PickerAction::MoveDown => {
                self.move_selection(1);
                PickerOutcome::Continue
            }
            PickerAction::PageUp => {
                self.move_selection(-10);
                PickerOutcome::Continue
            }
            PickerAction::PageDown => {
                self.move_selection(10);
                PickerOutcome::Continue
            }
            PickerAction::Select => {
                let visible = self.visible_ids();
                if let Some(target) = visible.iter().find(|id| **id == self.selected_id).copied() {
                    PickerOutcome::Select(target)
                } else if let Some(first) = visible.first().copied() {
                    PickerOutcome::Select(first)
                } else {
                    PickerOutcome::Continue
                }
            }
            PickerAction::Cancel => PickerOutcome::Cancel,
            PickerAction::ClearFilterOrCancel => {
                if self.filter.is_empty() {
                    PickerOutcome::Cancel
                } else {
                    self.filter.clear();
                    self.clamp_selection();
                    PickerOutcome::Continue
                }
            }
            PickerAction::BackspaceFilter => {
                self.filter.pop();
                self.clamp_selection();
                PickerOutcome::Continue
            }
            PickerAction::AppendFilterChar(c) => self.append_filter_char(c),
            PickerAction::ToggleGroup { group } => PickerOutcome::ToggleHiddenGroup(group),
            PickerAction::ToggleSelectedItemGroup => self
                .selected_item_group()
                .map(PickerOutcome::ToggleHiddenGroup)
                .unwrap_or(PickerOutcome::Continue),
            PickerAction::Custom(name) => PickerOutcome::Custom(name),
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let visible = self.visible_ids();
        if visible.is_empty() {
            return;
        }
        let current_idx = visible
            .iter()
            .position(|id| *id == self.selected_id)
            .map(|i| i as i32)
            .unwrap_or(0);
        let next = (current_idx + delta).rem_euclid(visible.len() as i32) as usize;
        self.selected_id = visible[next];
    }

    fn clamp_selection(&mut self) {
        let visible = self.visible_ids();
        if visible.contains(&self.selected_id) {
            return;
        }
        if let Some(first) = visible.first().copied() {
            self.selected_id = first;
        }
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn visible_items(&self) -> Vec<&PickerItem<T>> {
        self.items
            .iter()
            .filter(|item| match &item.group {
                Some(g) => !self.hidden_groups.contains(g),
                None => true,
            })
            .filter(|item| matches_filter(&self.filter, item))
            .collect()
    }

    fn visible_ids(&self) -> Vec<u64> {
        self.visible_items().into_iter().map(|i| i.id).collect()
    }

    fn selected_item_group(&self) -> Option<String> {
        self.items
            .iter()
            .find(|item| item.id == self.selected_id)
            .and_then(|item| item.group.clone())
    }

    pub fn matched_search_for<'a>(&self, item: &'a PickerItem<T>) -> Option<&'a str> {
        if self.filter.is_empty() {
            return None;
        }
        let needle = self.filter.to_ascii_lowercase();
        item.searchable
            .iter()
            .find(|s| subsequence_match(&needle, &s.to_ascii_lowercase()))
            .map(String::as_str)
    }
}

fn matches_filter<T: Clone>(filter: &str, item: &PickerItem<T>) -> bool {
    if filter.is_empty() {
        return true;
    }
    let needle = filter.to_ascii_lowercase();
    let mut primary = item.primary.clone();
    if let Some(d) = &item.detail {
        primary.push(' ');
        primary.push_str(d);
    }
    if let Some(g) = &item.group {
        primary.push(' ');
        primary.push_str(g);
    }
    if subsequence_match(&needle, &primary.to_ascii_lowercase()) {
        return true;
    }
    item.searchable
        .iter()
        .any(|s| subsequence_match(&needle, &s.to_ascii_lowercase()))
}

fn subsequence_match(needle: &str, haystack: &str) -> bool {
    let mut h_iter = haystack.chars();
    'outer: for nc in needle.chars() {
        for hc in h_iter.by_ref() {
            if hc == nc {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

#[derive(Debug)]
pub struct ThumbnailRequest {
    pub item_id: u64,
    pub row: u16,
    pub col: u16,
}

pub struct PickerWidget<'a, T: Clone> {
    pub picker: &'a Picker<T>,
    pub thumbnails: &'a mut Vec<ThumbnailRequest>,
}

impl<'a, T: Clone> std::fmt::Debug for PickerWidget<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PickerWidget").finish()
    }
}

#[derive(Debug)]
struct LineSpan {
    kind: LineKind,
    virtual_row: u16,
    span: u16,
}

#[derive(Debug)]
enum LineKind {
    GroupHeader(String),
    Item { id: u64, selected: bool },
    Empty(String),
}

impl<'a, T: Clone> Widget for PickerWidget<'a, T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let cfg = self.picker.config();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", cfg.title.trim()))
            .title_bottom(cfg.bottom_hint.clone());
        let inner = block.inner(area);
        block.render(area, buf);
        if inner.height < 3 || inner.width < 8 {
            return;
        }

        let header_avail = inner.width.saturating_sub(1) as usize;
        let header_text = if self.picker.filter().is_empty() {
            "type to filter…".to_owned()
        } else {
            format!("Filter: {}", self.picker.filter())
        };
        Paragraph::new(truncate(&header_text, header_avail)).render(
            Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: 1,
            },
            buf,
        );

        let body = Rect {
            x: inner.x,
            y: inner.y + 2,
            width: inner.width,
            height: inner.height.saturating_sub(2),
        };

        let layouts = compute_line_spans(self.picker, cfg.item_row_span);
        let total_virtual = layouts.last().map(|l| l.virtual_row + l.span).unwrap_or(0);
        let mut scroll: u16 = 0;
        for layout in &layouts {
            if let LineKind::Item { id, selected: true } = &layout.kind {
                let _ = id;
                let sel_end = layout.virtual_row + layout.span;
                if total_virtual > body.height {
                    if sel_end > scroll + body.height {
                        scroll = sel_end - body.height;
                    }
                    if layout.virtual_row < scroll {
                        scroll = layout.virtual_row;
                    }
                }
                break;
            }
        }

        for (idx, layout) in layouts.iter().enumerate() {
            if layout.virtual_row + layout.span <= scroll {
                continue;
            }
            if layout.virtual_row >= scroll + body.height {
                break;
            }
            let screen_row = body.y + layout.virtual_row.saturating_sub(scroll);
            if screen_row >= body.y + body.height {
                break;
            }
            match &layout.kind {
                LineKind::GroupHeader(text) => {
                    let avail = body.width.saturating_sub(1) as usize;
                    buf.set_string(
                        body.x,
                        screen_row,
                        truncate(text, avail),
                        Style::default().add_modifier(Modifier::BOLD),
                    );
                }
                LineKind::Item { id, selected } => {
                    let _ = idx;
                    let item = self.picker.items().iter().find(|i| i.id == *id).unwrap();
                    let detail_override = self
                        .picker
                        .matched_search_for(item)
                        .map(|m| format!("contains {}", m));
                    let marker = if *selected { ">" } else { " " };
                    let (text_col, thumb_drawn) = if cfg.allow_thumbnails {
                        self.thumbnails.push(ThumbnailRequest {
                            item_id: *id,
                            row: screen_row + 1,
                            col: body.x + 1 + 1,
                        });
                        (body.x + cfg.thumb_cols + 2, true)
                    } else {
                        (body.x, false)
                    };
                    let text_avail = body
                        .width
                        .saturating_sub(if thumb_drawn { cfg.thumb_cols + 2 } else { 0 } + 1)
                        as usize;
                    let primary_text = format!(
                        "{} {}",
                        marker,
                        truncate(&item.primary, text_avail.saturating_sub(2))
                    );
                    let style = if *selected {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    let visible_len = primary_text.chars().count();
                    buf.set_string(text_col, screen_row, &primary_text, style);
                    if *selected {
                        let pad = text_avail.saturating_sub(visible_len);
                        if pad > 0 {
                            buf.set_string(
                                text_col + visible_len as u16,
                                screen_row,
                                " ".repeat(pad),
                                style,
                            );
                        }
                    }
                    let detail_text = detail_override.as_deref().or(item.detail.as_deref());
                    if let Some(d) = detail_text {
                        let detail_row = screen_row + 1;
                        if detail_row < body.y + body.height {
                            let detail_offset: u16 = if thumb_drawn { 4 } else { 2 };
                            let avail = body.width.saturating_sub(
                                if thumb_drawn { cfg.thumb_cols + 4 } else { 0 } + 1,
                            ) as usize;
                            buf.set_string(
                                text_col + detail_offset.saturating_sub(2),
                                detail_row,
                                truncate(d, avail),
                                Style::default().add_modifier(Modifier::DIM),
                            );
                        }
                    }
                }
                LineKind::Empty(text) => {
                    buf.set_string(body.x, screen_row, text, Style::default());
                }
            }
        }

        if scroll > 0 {
            buf.set_string(
                inner.x + inner.width.saturating_sub(1),
                body.y,
                "▲",
                Style::default(),
            );
        }
        if scroll + body.height < total_virtual {
            buf.set_string(
                inner.x + inner.width.saturating_sub(1),
                body.y + body.height.saturating_sub(1),
                "▼",
                Style::default(),
            );
        }
    }
}

fn compute_line_spans<T: Clone>(picker: &Picker<T>, item_span: u16) -> Vec<LineSpan> {
    let visible = picker.visible_items();
    let mut layouts = Vec::new();
    let mut row = 0u16;

    if visible.is_empty() {
        let msg = if picker.items().is_empty() {
            "No items".to_owned()
        } else {
            format!("No matches for '{}'", picker.filter())
        };
        layouts.push(LineSpan {
            kind: LineKind::Empty(msg),
            virtual_row: row,
            span: 1,
        });
        return layouts;
    }

    for group in &picker.group_order {
        let group_items: Vec<&PickerItem<T>> = visible
            .iter()
            .copied()
            .filter(|item| item.group == *group)
            .collect();
        if group_items.is_empty() {
            continue;
        }
        if let Some(name) = group {
            layouts.push(LineSpan {
                kind: LineKind::GroupHeader(format!("── {} ──", name)),
                virtual_row: row,
                span: 1,
            });
            row += 1;
        }
        for item in group_items {
            let selected = item.id == picker.selected_id();
            layouts.push(LineSpan {
                kind: LineKind::Item {
                    id: item.id,
                    selected,
                },
                virtual_row: row,
                span: item_span,
            });
            row += item_span;
        }
    }
    layouts
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        text.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u64, name: &str, group: Option<&str>) -> PickerItem<()> {
        PickerItem {
            id,
            primary: name.to_owned(),
            detail: None,
            group: group.map(str::to_owned),
            searchable: Vec::new(),
            payload: (),
        }
    }

    fn picker() -> Picker<()> {
        Picker::new(
            vec![
                item(0, "Apple", Some("Fruits")),
                item(1, "Banana", Some("Fruits")),
                item(2, "Carrot", Some("Veggies")),
                item(3, "Donut", Some("Snacks")),
            ],
            PickerConfig::default(),
            0,
        )
    }

    #[test]
    fn filter_subsequence_match() {
        let mut p = picker();
        for c in "an".chars() {
            p.append_filter_char(c);
        }
        let visible: Vec<u64> = p.visible_items().into_iter().map(|i| i.id).collect();
        assert!(visible.contains(&1)); // Banana matches a..n
    }

    #[test]
    fn enter_selects_first_visible_when_current_filtered_out() {
        let mut p = picker();
        for c in "donu".chars() {
            p.append_filter_char(c);
        }
        let outcome = p.handle_key(Key::Enter);
        assert_eq!(outcome, PickerOutcome::Select(3));
    }

    #[test]
    fn esc_clears_filter_then_cancels() {
        let mut p = picker();
        p.append_filter_char('a');
        let o = p.handle_key(Key::Esc);
        assert_eq!(o, PickerOutcome::Continue);
        let o = p.handle_key(Key::Esc);
        assert_eq!(o, PickerOutcome::Cancel);
    }

    #[test]
    fn arrow_keys_cycle_visible_items() {
        let mut p = picker();
        p.handle_key(Key::Down);
        assert_eq!(p.selected_id(), 1);
        p.handle_key(Key::Down);
        assert_eq!(p.selected_id(), 2);
        p.handle_key(Key::Up);
        assert_eq!(p.selected_id(), 1);
    }

    #[test]
    fn hidden_group_excludes_items() {
        let mut p = picker();
        p.set_group_hidden("Fruits", true);
        let visible: Vec<u64> = p.visible_items().into_iter().map(|i| i.id).collect();
        assert!(!visible.contains(&0));
        assert!(!visible.contains(&1));
        assert!(visible.contains(&2));
    }

    #[test]
    fn tab_is_unbound_by_default() {
        let mut p = picker();
        assert_eq!(p.handle_key(Key::Tab), PickerOutcome::Continue);
    }

    #[test]
    fn configured_actions_can_toggle_selected_group() {
        let mut cfg = PickerConfig::default_navigation();
        cfg.actions.bind(
            KeyTrigger::Special(SpecialKey::Tab),
            PickerAction::ToggleSelectedItemGroup,
        );
        let mut p = Picker::new(vec![item(0, "Apple", Some("Fruits"))], cfg, 0);
        assert_eq!(
            p.handle_key(Key::Tab),
            PickerOutcome::ToggleHiddenGroup("Fruits".to_owned())
        );
    }
}
