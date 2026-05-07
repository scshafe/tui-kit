//! Policy-light table state, column sizing, and viewport math.
//!
//! [`TableState`] owns reusable mechanics only: stable row/column IDs,
//! optional row selection, vertical and horizontal scrolling, explicit column
//! sizing, and configurable key actions. Applications own row meaning, sorting,
//! command semantics, and rendering style.

use crate::config::{ConfigError, Validate};
use crate::input::Key;
use crate::keymap::{KeyMap, KeyTrigger, SpecialKey};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TableRowId(pub String);

impl TableRowId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TableColumnId(pub String);

impl TableColumnId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableColumnSizing {
    Fixed(u16),
    Percentage(u16),
    Content,
    Fill,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumn {
    pub id: TableColumnId,
    pub header: String,
    pub sizing: TableColumnSizing,
    pub min_width: u16,
    pub max_width: Option<u16>,
    pub alignment: TableAlignment,
    pub pinned: bool,
}

impl TableColumn {
    pub fn new(
        id: impl Into<String>,
        header: impl Into<String>,
        sizing: TableColumnSizing,
    ) -> Self {
        Self {
            id: TableColumnId::new(id),
            header: header.into(),
            sizing,
            min_width: 1,
            max_width: None,
            alignment: TableAlignment::Left,
            pinned: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow<T = ()> {
    pub id: TableRowId,
    pub cells: Vec<String>,
    pub payload: T,
}

impl<T> TableRow<T> {
    pub fn new(id: impl Into<String>, cells: Vec<String>, payload: T) -> Self {
        Self {
            id: TableRowId::new(id),
            cells,
            payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSelectionMode {
    None,
    SingleRow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableAction {
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    ScrollLeft,
    ScrollRight,
    ScrollToTop,
    ScrollToBottom,
    Select,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct TableConfig {
    pub selection: TableSelectionMode,
    pub page_stride: u16,
    pub horizontal_scroll_step: u16,
    pub actions: KeyMap<TableAction>,
}

impl TableConfig {
    pub fn explicit(actions: KeyMap<TableAction>) -> Self {
        Self {
            selection: TableSelectionMode::SingleRow,
            page_stride: 10,
            horizontal_scroll_step: 4,
            actions,
        }
    }

    pub fn default_navigation() -> Self {
        let mut actions = KeyMap::new();
        actions
            .bind(KeyTrigger::Special(SpecialKey::Up), TableAction::MoveUp)
            .bind(KeyTrigger::Special(SpecialKey::Down), TableAction::MoveDown)
            .bind(
                KeyTrigger::Special(SpecialKey::Left),
                TableAction::ScrollLeft,
            )
            .bind(
                KeyTrigger::Special(SpecialKey::Right),
                TableAction::ScrollRight,
            )
            .bind(KeyTrigger::Special(SpecialKey::Enter), TableAction::Select);
        Self::explicit(actions)
    }
}

impl Default for TableConfig {
    fn default() -> Self {
        Self::default_navigation()
    }
}

impl Validate for TableConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.page_stride == 0 {
            return Err(ConfigError::new(
                "TableConfig.page_stride",
                "must be greater than zero",
            ));
        }
        if self.horizontal_scroll_step == 0 {
            return Err(ConfigError::new(
                "TableConfig.horizontal_scroll_step",
                "must be greater than zero",
            ));
        }
        if self.actions.is_empty() {
            return Err(ConfigError::new(
                "TableConfig.actions",
                "must install an explicit key policy; use TableConfig::default_navigation() for the built-in preset",
            ));
        }
        for binding in self.actions.bindings() {
            if let TableAction::Custom(name) = &binding.command {
                if name.trim().is_empty() {
                    return Err(ConfigError::new(
                        "TableConfig.actions[].Custom",
                        "must not be empty",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableColumnLayout {
    pub index: usize,
    pub width: u16,
    pub x: u16,
    pub visible_x: u16,
    pub visible_width: u16,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableViewport {
    pub row_offset: usize,
    pub column_offset: u16,
    pub body_height: u16,
    pub width: u16,
    pub row_start: usize,
    pub row_end_exclusive: usize,
    pub total_rows: usize,
    pub selected_row_index: Option<usize>,
    pub columns: Vec<TableColumnLayout>,
    pub total_table_width: u16,
    pub can_scroll_up: bool,
    pub can_scroll_down: bool,
    pub can_scroll_left: bool,
    pub can_scroll_right: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableOutcome {
    Continue,
    Selected(TableRowId),
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct TableState<T = ()> {
    columns: Vec<TableColumn>,
    rows: Vec<TableRow<T>>,
    config: TableConfig,
    selected_row_index: Option<usize>,
    row_offset: usize,
    column_offset: u16,
}

impl<T> TableState<T> {
    pub fn new(columns: Vec<TableColumn>, rows: Vec<TableRow<T>>, config: TableConfig) -> Self {
        Self::try_new(columns, rows, config)
            .unwrap_or_else(|err| panic!("invalid table config: {err}"))
    }

    pub fn try_new(
        columns: Vec<TableColumn>,
        rows: Vec<TableRow<T>>,
        config: TableConfig,
    ) -> Result<Self, ConfigError> {
        config.validate()?;
        validate_columns(&columns)?;
        validate_rows(&columns, &rows)?;
        let selected_row_index = match config.selection {
            TableSelectionMode::None => None,
            TableSelectionMode::SingleRow if rows.is_empty() => None,
            TableSelectionMode::SingleRow => Some(0),
        };
        Ok(Self {
            columns,
            rows,
            config,
            selected_row_index,
            row_offset: 0,
            column_offset: 0,
        })
    }

    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    pub fn rows(&self) -> &[TableRow<T>] {
        &self.rows
    }

    pub fn config(&self) -> &TableConfig {
        &self.config
    }

    pub fn selected_row_index(&self) -> Option<usize> {
        self.selected_row_index
    }

    pub fn selected_row(&self) -> Option<&TableRow<T>> {
        self.selected_row_index
            .and_then(|index| self.rows.get(index))
    }

    pub fn row_offset(&self) -> usize {
        self.row_offset
    }

    pub fn column_offset(&self) -> u16 {
        self.column_offset
    }

    pub fn handle_key(&mut self, key: Key, body_height: u16, width: u16) -> TableOutcome {
        if let Some(action) = self.config.actions.lookup(key) {
            self.handle_action(action, body_height, width)
        } else {
            TableOutcome::Continue
        }
    }

    pub fn handle_action(
        &mut self,
        action: TableAction,
        body_height: u16,
        width: u16,
    ) -> TableOutcome {
        match action {
            TableAction::MoveUp => self.move_selection_or_scroll(-1, body_height),
            TableAction::MoveDown => self.move_selection_or_scroll(1, body_height),
            TableAction::PageUp => self.page(-1, body_height),
            TableAction::PageDown => self.page(1, body_height),
            TableAction::ScrollLeft => {
                self.scroll_columns_by(-(self.config.horizontal_scroll_step as isize), width)
            }
            TableAction::ScrollRight => {
                self.scroll_columns_by(self.config.horizontal_scroll_step as isize, width)
            }
            TableAction::ScrollToTop => {
                self.row_offset = 0;
                if self.config.selection == TableSelectionMode::SingleRow && !self.rows.is_empty() {
                    self.selected_row_index = Some(0);
                }
            }
            TableAction::ScrollToBottom => {
                self.row_offset = self.max_row_offset(body_height);
                if self.config.selection == TableSelectionMode::SingleRow && !self.rows.is_empty() {
                    self.selected_row_index = Some(self.rows.len() - 1);
                }
            }
            TableAction::Select => {
                if let Some(row) = self.selected_row() {
                    return TableOutcome::Selected(row.id.clone());
                }
            }
            TableAction::Custom(name) => return TableOutcome::Custom(name),
        }
        TableOutcome::Continue
    }

    pub fn viewport(&self, body_height: u16, width: u16) -> TableViewport {
        let total_rows = self.rows.len();
        let max_row_offset = max_row_offset(total_rows, body_height);
        let row_offset = self.row_offset.min(max_row_offset);
        let row_start = row_offset.min(total_rows);
        let row_end_exclusive = row_start
            .saturating_add(usize::from(body_height))
            .min(total_rows);
        let column_widths = self.column_widths(width);
        let total_table_width = total_width(&column_widths);
        let max_column_offset = total_table_width.saturating_sub(width);
        let column_offset = self.column_offset.min(max_column_offset);
        let columns = visible_columns(&self.columns, &column_widths, column_offset, width);
        TableViewport {
            row_offset,
            column_offset,
            body_height,
            width,
            row_start,
            row_end_exclusive,
            total_rows,
            selected_row_index: self.selected_row_index,
            columns,
            total_table_width,
            can_scroll_up: row_offset > 0,
            can_scroll_down: row_offset < max_row_offset,
            can_scroll_left: column_offset > 0,
            can_scroll_right: column_offset < max_column_offset,
        }
    }

    pub fn set_row_offset(&mut self, offset: usize, body_height: u16) {
        self.row_offset = offset.min(self.max_row_offset(body_height));
    }

    pub fn set_column_offset(&mut self, offset: u16, width: u16) {
        self.column_offset = offset.min(self.max_column_offset(width));
    }

    pub fn column_widths(&self, viewport_width: u16) -> Vec<u16> {
        column_widths(&self.columns, &self.rows, viewport_width)
    }

    fn move_selection_or_scroll(&mut self, delta: isize, body_height: u16) {
        if self.config.selection == TableSelectionMode::None {
            self.scroll_rows_by(delta, body_height);
            return;
        }
        let Some(current) = self.selected_row_index else {
            if !self.rows.is_empty() {
                self.selected_row_index = Some(0);
            }
            return;
        };
        let next = current
            .saturating_add_signed(delta)
            .min(self.rows.len().saturating_sub(1));
        self.selected_row_index = Some(next);
        self.keep_selection_visible(body_height);
    }

    fn page(&mut self, direction: isize, body_height: u16) {
        let stride = usize::from(self.config.page_stride.min(body_height.max(1)));
        let delta = direction.saturating_mul(stride as isize);
        if self.config.selection == TableSelectionMode::SingleRow {
            self.move_selection_or_scroll(delta, body_height);
        } else {
            self.scroll_rows_by(delta, body_height);
        }
    }

    fn scroll_rows_by(&mut self, delta: isize, body_height: u16) {
        self.row_offset = self
            .row_offset
            .saturating_add_signed(delta)
            .min(self.max_row_offset(body_height));
    }

    fn scroll_columns_by(&mut self, delta: isize, width: u16) {
        let delta = delta.clamp(i16::MIN as isize, i16::MAX as isize) as i16;
        self.column_offset = self
            .column_offset
            .saturating_add_signed(delta)
            .min(self.max_column_offset(width));
    }

    fn keep_selection_visible(&mut self, body_height: u16) {
        let Some(selected) = self.selected_row_index else {
            return;
        };
        if body_height == 0 {
            self.row_offset = selected.min(self.max_row_offset(body_height));
            return;
        }
        let height = usize::from(body_height);
        if selected < self.row_offset {
            self.row_offset = selected;
        } else if selected >= self.row_offset.saturating_add(height) {
            self.row_offset = selected.saturating_add(1).saturating_sub(height);
        }
        self.row_offset = self.row_offset.min(self.max_row_offset(body_height));
    }

    fn max_row_offset(&self, body_height: u16) -> usize {
        max_row_offset(self.rows.len(), body_height)
    }

    fn max_column_offset(&self, width: u16) -> u16 {
        total_width(&self.column_widths(width)).saturating_sub(width)
    }
}

pub fn max_row_offset(total_rows: usize, body_height: u16) -> usize {
    total_rows.saturating_sub(usize::from(body_height))
}

fn validate_columns(columns: &[TableColumn]) -> Result<(), ConfigError> {
    if columns.is_empty() {
        return Err(ConfigError::new(
            "TableState.columns",
            "must contain at least one column",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    for (index, column) in columns.iter().enumerate() {
        let path = format!("TableState.columns[{index}]");
        if column.id.0.trim().is_empty() {
            return Err(ConfigError::new(format!("{path}.id"), "must not be empty"));
        }
        if !ids.insert(column.id.clone()) {
            return Err(ConfigError::new(format!("{path}.id"), "must be unique"));
        }
        if column.min_width == 0 {
            return Err(ConfigError::new(
                format!("{path}.min_width"),
                "must be greater than zero",
            ));
        }
        if let Some(max_width) = column.max_width {
            if max_width < column.min_width {
                return Err(ConfigError::new(
                    format!("{path}.max_width"),
                    "must be at least min_width",
                ));
            }
        }
        match column.sizing {
            TableColumnSizing::Fixed(0) => {
                return Err(ConfigError::new(
                    format!("{path}.sizing"),
                    "fixed width must be greater than zero",
                ));
            }
            TableColumnSizing::Percentage(percent) if percent == 0 || percent > 100 => {
                return Err(ConfigError::new(
                    format!("{path}.sizing"),
                    "percentage must be between 1 and 100",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_rows<T>(columns: &[TableColumn], rows: &[TableRow<T>]) -> Result<(), ConfigError> {
    let mut ids = std::collections::HashSet::new();
    for (index, row) in rows.iter().enumerate() {
        let path = format!("TableState.rows[{index}]");
        if row.id.0.trim().is_empty() {
            return Err(ConfigError::new(format!("{path}.id"), "must not be empty"));
        }
        if !ids.insert(row.id.clone()) {
            return Err(ConfigError::new(format!("{path}.id"), "must be unique"));
        }
        if row.cells.len() != columns.len() {
            return Err(ConfigError::new(
                format!("{path}.cells"),
                "must match column count",
            ));
        }
    }
    Ok(())
}

fn column_widths<T>(
    columns: &[TableColumn],
    rows: &[TableRow<T>],
    viewport_width: u16,
) -> Vec<u16> {
    let mut widths = vec![0; columns.len()];
    let mut fill_count = 0u16;
    for (index, column) in columns.iter().enumerate() {
        widths[index] = match column.sizing {
            TableColumnSizing::Fixed(width) => width,
            TableColumnSizing::Percentage(percent) => viewport_width.saturating_mul(percent) / 100,
            TableColumnSizing::Content => content_width(index, column, rows),
            TableColumnSizing::Fill => {
                fill_count = fill_count.saturating_add(1);
                column.min_width
            }
        };
        widths[index] = clamp_width(widths[index], column);
    }
    if let Some(per_fill) = viewport_width
        .saturating_sub(total_width(&widths))
        .checked_div(fill_count)
    {
        let mut extra = viewport_width.saturating_sub(total_width(&widths)) % fill_count;
        for (index, column) in columns.iter().enumerate() {
            if column.sizing == TableColumnSizing::Fill {
                let bump = per_fill + u16::from(extra > 0);
                extra = extra.saturating_sub(1);
                widths[index] = clamp_width(widths[index].saturating_add(bump), column);
            }
        }
    }
    widths
}

fn content_width<T>(index: usize, column: &TableColumn, rows: &[TableRow<T>]) -> u16 {
    rows.iter()
        .filter_map(|row| row.cells.get(index))
        .map(|cell| cell.chars().count())
        .chain(std::iter::once(column.header.chars().count()))
        .max()
        .unwrap_or(0)
        .min(usize::from(u16::MAX)) as u16
}

fn clamp_width(width: u16, column: &TableColumn) -> u16 {
    let width = width.max(column.min_width);
    column.max_width.map_or(width, |max| width.min(max))
}

fn total_width(widths: &[u16]) -> u16 {
    widths
        .iter()
        .fold(0u16, |total, width| total.saturating_add(*width))
}

fn visible_columns(
    columns: &[TableColumn],
    widths: &[u16],
    column_offset: u16,
    viewport_width: u16,
) -> Vec<TableColumnLayout> {
    let mut out = Vec::new();
    let mut x = 0u16;
    for (index, (column, width)) in columns.iter().zip(widths.iter()).enumerate() {
        let start = x;
        let end = x.saturating_add(*width);
        let (visible_x, visible_width) = if column.pinned {
            (
                start.min(viewport_width),
                (*width).min(viewport_width.saturating_sub(start)),
            )
        } else {
            let viewport_start = column_offset;
            let viewport_end = column_offset.saturating_add(viewport_width);
            if end <= viewport_start || start >= viewport_end {
                x = end;
                continue;
            }
            let clipped_start = start.max(viewport_start);
            let clipped_end = end.min(viewport_end);
            (
                clipped_start.saturating_sub(viewport_start),
                clipped_end.saturating_sub(clipped_start),
            )
        };
        if visible_width > 0 {
            out.push(TableColumnLayout {
                index,
                width: *width,
                x: start,
                visible_x,
                visible_width,
                pinned: column.pinned,
            });
        }
        x = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns() -> Vec<TableColumn> {
        vec![
            TableColumn::new("name", "Name", TableColumnSizing::Content),
            TableColumn::new("status", "Status", TableColumnSizing::Fixed(6)),
            TableColumn::new("detail", "Detail", TableColumnSizing::Fill),
        ]
    }

    fn rows(count: usize) -> Vec<TableRow> {
        (0..count)
            .map(|idx| {
                TableRow::new(
                    format!("row-{idx}"),
                    vec![format!("item-{idx}"), "ok".into(), "detail".into()],
                    (),
                )
            })
            .collect()
    }

    #[test]
    fn validation_requires_columns_and_matching_cells() {
        let err =
            TableState::<()>::try_new(Vec::new(), Vec::new(), TableConfig::default_navigation())
                .unwrap_err();
        assert_eq!(err.path, "TableState.columns");

        let err = TableState::try_new(
            columns(),
            vec![TableRow::new("bad", vec!["one".into()], ())],
            TableConfig::default_navigation(),
        )
        .unwrap_err();
        assert_eq!(err.path, "TableState.rows[0].cells");
    }

    #[test]
    fn validation_rejects_duplicate_ids() {
        let mut columns = columns();
        columns[1].id = columns[0].id.clone();
        let err = TableState::<()>::try_new(columns, Vec::new(), TableConfig::default_navigation())
            .unwrap_err();
        assert_eq!(err.path, "TableState.columns[1].id");
    }

    #[test]
    fn selection_scrolls_into_view() {
        let mut table = TableState::new(columns(), rows(10), TableConfig::default_navigation());
        for _ in 0..4 {
            table.handle_action(TableAction::MoveDown, 3, 40);
        }
        let viewport = table.viewport(3, 40);
        assert_eq!(viewport.selected_row_index, Some(4));
        assert_eq!(viewport.row_start, 2);
        assert_eq!(viewport.row_end_exclusive, 5);
    }

    #[test]
    fn horizontal_scroll_reports_visible_columns() {
        let mut table = TableState::new(columns(), rows(2), TableConfig::default_navigation());
        table.set_column_offset(2, 10);
        let viewport = table.viewport(2, 10);
        assert_eq!(viewport.column_offset, 2);
        assert!(viewport.can_scroll_left);
        assert!(viewport.can_scroll_right);
        assert!(viewport
            .columns
            .iter()
            .any(|column| column.visible_width < column.width));
    }

    #[test]
    fn column_sizing_supports_content_fixed_and_fill() {
        let table = TableState::new(columns(), rows(1), TableConfig::default_navigation());
        assert_eq!(table.column_widths(30), vec![6, 6, 18]);
    }

    #[test]
    fn select_outcome_returns_stable_row_id() {
        let mut table = TableState::new(columns(), rows(3), TableConfig::default_navigation());
        table.handle_action(TableAction::MoveDown, 5, 40);
        assert_eq!(
            table.handle_action(TableAction::Select, 5, 40),
            TableOutcome::Selected(TableRowId::new("row-1"))
        );
    }

    #[test]
    fn key_actions_are_configurable() {
        let mut actions = KeyMap::new();
        actions.bind(
            KeyTrigger::Special(SpecialKey::Tab),
            TableAction::Custom("next-pane".into()),
        );
        let mut table = TableState::new(columns(), rows(1), TableConfig::explicit(actions));
        assert_eq!(
            table.handle_key(Key::Tab, 1, 10),
            TableOutcome::Custom("next-pane".into())
        );
    }
}
