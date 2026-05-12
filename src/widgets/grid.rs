//! Generic fixed-row grid container.
//!
//! [`Grid`] owns column calculation, selected-item scrolling, visible-cell
//! clipping, and basic cell styling. Cell renderers receive a
//! [`GridCellCanvas`], which accepts local cell coordinates so children do not
//! need to know their absolute terminal row or column.

use crate::component::ComponentOutcome;
use crate::input::KeyEvent;
use crate::layout::CellArea;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

#[derive(Debug, Clone)]
pub struct Grid {
    column_mode: GridColumnMode,
    cell_rows: u16,
    active_index: Option<usize>,
    selected_index: Option<usize>,
    wrap_navigation: bool,
    focus_capture: bool,
    style: GridStyle,
    scroll_indicators: bool,
}

impl Grid {
    /// Create a grid with one terminal row per cell and dynamic columns.
    pub fn new() -> Self {
        Self {
            column_mode: GridColumnMode::DynamicMin { min_cell_cols: 1 },
            cell_rows: 1,
            active_index: None,
            selected_index: None,
            wrap_navigation: false,
            focus_capture: false,
            style: GridStyle::default(),
            scroll_indicators: true,
        }
    }

    /// Set the height of each grid cell in terminal rows.
    pub fn with_cell_rows(mut self, cell_rows: u16) -> Self {
        self.cell_rows = cell_rows.max(1);
        self
    }

    /// Use dynamic column calculation with the given minimum cell width.
    ///
    /// This selects [`GridColumnMode::DynamicMin`] and replaces any fixed
    /// column mode previously configured on the grid.
    pub fn with_min_cell_cols(mut self, min_cell_cols: u16) -> Self {
        self.column_mode = GridColumnMode::DynamicMin {
            min_cell_cols: min_cell_cols.max(1),
        };
        self
    }

    /// Use a fixed number of columns, capped by viewport width and item count.
    ///
    /// This selects [`GridColumnMode::Fixed`] and replaces any dynamic minimum
    /// column width previously configured on the grid.
    pub fn with_columns(mut self, columns: u16) -> Self {
        self.column_mode = GridColumnMode::Fixed {
            columns: columns.max(1),
        };
        self
    }

    pub fn with_column_mode(mut self, mode: GridColumnMode) -> Self {
        self.column_mode = mode.normalized();
        self
    }

    pub fn with_active_index(mut self, active_index: Option<usize>) -> Self {
        self.active_index = active_index;
        self
    }

    pub fn with_selected_index(mut self, selected_index: Option<usize>) -> Self {
        self.selected_index = selected_index;
        self
    }

    pub fn with_style(mut self, style: GridStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_cell_style(mut self, style: Style) -> Self {
        self.style.cell = style;
        self
    }

    pub fn with_active_cell_style(mut self, style: Style) -> Self {
        self.style.active_cell = style;
        self
    }

    pub fn with_selected_cell_style(mut self, style: Style) -> Self {
        self.style.selected_cell = style;
        self
    }

    pub fn with_wrap_navigation(mut self, enabled: bool) -> Self {
        self.wrap_navigation = enabled;
        self
    }

    pub fn with_focus_capture(mut self, enabled: bool) -> Self {
        self.focus_capture = enabled;
        self
    }

    pub fn with_scroll_indicators(mut self, enabled: bool) -> Self {
        self.scroll_indicators = enabled;
        self
    }

    pub fn active_index(&self) -> Option<usize> {
        self.active_index
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn captures_focus(&self) -> bool {
        self.focus_capture
    }

    pub fn cell_rows(&self) -> u16 {
        self.cell_rows
    }

    pub fn column_mode(&self) -> GridColumnMode {
        self.column_mode
    }

    pub fn columns_for_width(&self, width: u16, item_count: usize) -> u16 {
        match self.column_mode.normalized() {
            GridColumnMode::Fixed { columns } if item_count > 0 && width > 0 => columns
                .max(1)
                .min(width)
                .min(item_count.min(usize::from(u16::MAX)) as u16),
            GridColumnMode::Fixed { .. } => 0,
            GridColumnMode::DynamicMin { min_cell_cols } => {
                grid_columns(width, min_cell_cols, item_count)
            }
        }
    }

    pub fn move_active(
        &mut self,
        direction: GridNavigation,
        viewport_width: u16,
        item_count: usize,
    ) -> Option<usize> {
        if item_count == 0 {
            self.active_index = None;
            return None;
        }
        let columns = self.columns_for_width(viewport_width, item_count).max(1);
        let current = self
            .active_index
            .filter(|idx| *idx < item_count)
            .or(self.selected_index.filter(|idx| *idx < item_count))
            .unwrap_or(0);
        let next = next_grid_index(
            current,
            direction,
            columns,
            item_count,
            self.wrap_navigation,
        );
        self.active_index = Some(next);
        self.active_index
    }

    pub fn select_active(&mut self) -> Option<usize> {
        self.selected_index = self.active_index;
        self.selected_index
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        viewport_width: u16,
        item_count: usize,
    ) -> GridInputOutcome {
        if item_count == 0 {
            self.active_index = None;
            self.selected_index = None;
            return GridInputOutcome::Ignored;
        }

        let Some(direction) = GridNavigation::from_key_event(key) else {
            if key == KeyEvent::Enter {
                return match self.select_active() {
                    Some(index) => GridInputOutcome::Selected(index),
                    None => GridInputOutcome::Ignored,
                };
            }
            return GridInputOutcome::Ignored;
        };
        match self.move_active(direction, viewport_width, item_count) {
            Some(index) => GridInputOutcome::Moved(index),
            None => GridInputOutcome::Ignored,
        }
    }

    pub fn handle_key_as_component_outcome(
        &mut self,
        key: KeyEvent,
        viewport_width: u16,
        item_count: usize,
    ) -> ComponentOutcome<GridInputOutcome> {
        match self.handle_key(key, viewport_width, item_count) {
            GridInputOutcome::Ignored => ComponentOutcome::Ignored,
            outcome => ComponentOutcome::Message(outcome),
        }
    }

    /// Render the grid without mutating scroll state.
    ///
    /// Visible scroll is derived from the active index first, then the selected
    /// index. Consumers that need sticky or momentum scroll should own that
    /// state outside this renderer.
    pub fn render<T, F>(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        items: &[T],
        mut render_cell: F,
    ) -> GridRenderState
    where
        F: FnMut(GridCell<'_, T>, &mut GridCellCanvas<'_>),
    {
        let mut state = GridRenderState::default();
        if items.is_empty() || area.width == 0 || area.height == 0 {
            return state;
        }

        let columns = self.columns_for_width(area.width, items.len());
        let cell_rows = self.cell_rows.max(1);
        let total_rows = grid_rows(items.len(), columns);
        let total_virtual_rows = total_rows.saturating_mul(cell_rows);
        let scroll_anchor = self
            .active_index
            .filter(|idx| *idx < items.len())
            .or_else(|| self.selected_index.filter(|idx| *idx < items.len()));
        let scroll = selected_scroll(
            scroll_anchor,
            columns,
            cell_rows,
            area.height,
            total_virtual_rows,
        );

        state.columns = columns;
        state.rows = total_rows;
        state.scroll = scroll;
        state.total_virtual_rows = total_virtual_rows;
        state.active_index = self.active_index.filter(|idx| *idx < items.len());
        state.selected_index = self.selected_index.filter(|idx| *idx < items.len());

        let base_cell_cols = area.width / columns;
        let area_bottom = area.y.saturating_add(area.height);
        for (index, item) in items.iter().enumerate() {
            let column = (index % usize::from(columns)) as u16;
            let row = (index / usize::from(columns)) as u16;
            let virtual_y = row.saturating_mul(cell_rows);
            if virtual_y.saturating_add(cell_rows) <= scroll
                || virtual_y >= scroll.saturating_add(area.height)
            {
                continue;
            }

            let cell_x = area.x.saturating_add(column.saturating_mul(base_cell_cols));
            let cell_y = area.y.saturating_add(virtual_y.saturating_sub(scroll));
            if cell_y >= area_bottom {
                continue;
            }

            let width = if column + 1 == columns {
                area.width
                    .saturating_sub(base_cell_cols.saturating_mul(columns.saturating_sub(1)))
            } else {
                base_cell_cols
            };
            if width == 0 {
                continue;
            }
            let height = cell_rows.min(area_bottom.saturating_sub(cell_y));
            if height == 0 {
                continue;
            }

            let selected = Some(index) == self.selected_index;
            let active = Some(index) == self.active_index;
            let style = if selected {
                self.style.selected_cell
            } else if active {
                self.style.active_cell
            } else {
                self.style.cell
            };
            let cell_area = Rect {
                x: cell_x,
                y: cell_y,
                width,
                height,
            };
            buffer.set_style(cell_area, style);

            let placement = GridCellPlacement {
                index,
                column,
                row,
                active,
                selected,
                area: cell_area,
                cell_area: CellArea::new(cell_x, cell_y, width, height),
            };
            state.visible_cells.push(placement);

            let cell = GridCell {
                index,
                item,
                column,
                row,
                active,
                selected,
            };
            let mut canvas = GridCellCanvas {
                area: cell_area,
                buffer,
                style,
            };
            render_cell(cell, &mut canvas);
        }

        if self.scroll_indicators {
            self.render_scroll_indicators(area, buffer, &state);
        }

        state
    }

    fn render_scroll_indicators(&self, area: Rect, buffer: &mut Buffer, state: &GridRenderState) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let marker_col = area.x.saturating_add(area.width.saturating_sub(1));
        if state.has_overflow_above() {
            buffer.set_string(
                marker_col,
                area.y,
                self.style.scroll_up,
                self.style.scroll_indicator,
            );
        }
        if state.has_overflow_below(area.height) {
            buffer.set_string(
                marker_col,
                area.y.saturating_add(area.height.saturating_sub(1)),
                self.style.scroll_down,
                self.style.scroll_indicator,
            );
        }
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new()
    }
}

/// Column calculation strategy for [`Grid`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridColumnMode {
    /// Use exactly this many columns, capped by viewport width and item count.
    Fixed { columns: u16 },
    /// Derive as many columns as fit while each cell has at least this width.
    DynamicMin { min_cell_cols: u16 },
}

impl GridColumnMode {
    fn normalized(self) -> Self {
        match self {
            Self::Fixed { columns } => Self::Fixed {
                columns: columns.max(1),
            },
            Self::DynamicMin { min_cell_cols } => Self::DynamicMin {
                min_cell_cols: min_cell_cols.max(1),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct GridStyle {
    pub cell: Style,
    pub active_cell: Style,
    pub selected_cell: Style,
    pub scroll_indicator: Style,
    pub scroll_up: &'static str,
    pub scroll_down: &'static str,
}

impl Default for GridStyle {
    fn default() -> Self {
        Self {
            cell: Style::default(),
            active_cell: Style::default(),
            selected_cell: Style::default(),
            scroll_indicator: Style::default(),
            scroll_up: "^",
            scroll_down: "v",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GridRenderState {
    pub columns: u16,
    pub rows: u16,
    pub scroll: u16,
    pub total_virtual_rows: u16,
    pub active_index: Option<usize>,
    pub selected_index: Option<usize>,
    pub visible_cells: Vec<GridCellPlacement>,
}

impl GridRenderState {
    pub fn has_overflow_above(&self) -> bool {
        self.scroll > 0
    }

    pub fn has_overflow_below(&self, visible_rows: u16) -> bool {
        self.scroll.saturating_add(visible_rows) < self.total_virtual_rows
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridCellPlacement {
    pub index: usize,
    pub column: u16,
    pub row: u16,
    pub active: bool,
    pub selected: bool,
    pub area: Rect,
    pub cell_area: CellArea,
}

#[derive(Debug, Clone, Copy)]
pub struct GridCell<'a, T> {
    pub index: usize,
    pub item: &'a T,
    pub column: u16,
    pub row: u16,
    pub active: bool,
    pub selected: bool,
}

#[derive(Debug)]
pub struct GridCellCanvas<'a> {
    area: Rect,
    buffer: &'a mut Buffer,
    style: Style,
}

impl GridCellCanvas<'_> {
    pub fn area(&self) -> Rect {
        self.area
    }

    pub fn cell_area(&self) -> CellArea {
        CellArea::new(self.area.x, self.area.y, self.area.width, self.area.height)
    }

    pub fn width(&self) -> u16 {
        self.area.width
    }

    pub fn height(&self) -> u16 {
        self.area.height
    }

    pub fn style(&self) -> Style {
        self.style
    }

    pub fn local_rect(&self, col: u16, row: u16, cols: u16, rows: u16) -> Rect {
        if col >= self.area.width || row >= self.area.height {
            return Rect {
                x: self.area.x.saturating_add(self.area.width),
                y: self.area.y.saturating_add(self.area.height),
                width: 0,
                height: 0,
            };
        }
        let width = cols.min(self.area.width.saturating_sub(col));
        let height = rows.min(self.area.height.saturating_sub(row));
        Rect {
            x: self.area.x.saturating_add(col),
            y: self.area.y.saturating_add(row),
            width,
            height,
        }
    }

    pub fn local_cell_area(&self, col: u16, row: u16, cols: u16, rows: u16) -> CellArea {
        let rect = self.local_rect(col, row, cols, rows);
        CellArea::new(rect.x, rect.y, rect.width, rect.height)
    }

    pub fn set_string(&mut self, col: u16, row: u16, text: impl AsRef<str>, style: Style) {
        if col >= self.area.width || row >= self.area.height {
            return;
        }
        let max_cols = usize::from(self.area.width.saturating_sub(col));
        self.buffer.set_stringn(
            self.area.x.saturating_add(col),
            self.area.y.saturating_add(row),
            text.as_ref(),
            max_cols,
            style,
        );
    }

    pub fn render_widget<W: Widget>(&mut self, widget: W, local_area: Rect) {
        widget.render(
            self.local_rect(
                local_area.x,
                local_area.y,
                local_area.width,
                local_area.height,
            ),
            self.buffer,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridNavigation {
    Up,
    Down,
    Left,
    Right,
    First,
    Last,
}

impl GridNavigation {
    pub fn from_key_event(key: KeyEvent) -> Option<Self> {
        Some(match key {
            KeyEvent::Up => Self::Up,
            KeyEvent::Down => Self::Down,
            KeyEvent::Left => Self::Left,
            KeyEvent::Right => Self::Right,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridInputOutcome {
    Ignored,
    Moved(usize),
    Selected(usize),
}

fn grid_columns(width: u16, min_cell_cols: u16, items: usize) -> u16 {
    if items == 0 || width == 0 {
        return 0;
    }
    let capped_items = items.min(usize::from(u16::MAX)) as u16;
    (width / min_cell_cols.max(1)).max(1).min(capped_items)
}

fn grid_rows(items: usize, columns: u16) -> u16 {
    if items == 0 || columns == 0 {
        return 0;
    }
    let rows = items.div_ceil(usize::from(columns));
    rows.min(usize::from(u16::MAX)) as u16
}

fn next_grid_index(
    current: usize,
    direction: GridNavigation,
    columns: u16,
    item_count: usize,
    wrap: bool,
) -> usize {
    if item_count == 0 {
        return 0;
    }
    let columns = usize::from(columns.max(1));
    match direction {
        GridNavigation::First => 0,
        GridNavigation::Last => item_count.saturating_sub(1),
        GridNavigation::Left => {
            if current == 0 {
                if wrap {
                    item_count.saturating_sub(1)
                } else {
                    0
                }
            } else if !wrap && current.is_multiple_of(columns) {
                current
            } else {
                current - 1
            }
        }
        GridNavigation::Right => {
            if current + 1 >= item_count {
                if wrap {
                    0
                } else {
                    current
                }
            } else if !wrap && current % columns == columns.saturating_sub(1) {
                current
            } else {
                current + 1
            }
        }
        GridNavigation::Up => {
            if current >= columns {
                current - columns
            } else if wrap {
                let column = current % columns;
                let rows = item_count.div_ceil(columns);
                (rows.saturating_sub(1) * columns + column).min(item_count.saturating_sub(1))
            } else {
                current
            }
        }
        GridNavigation::Down => {
            let next = current.saturating_add(columns);
            if next < item_count {
                next
            } else if wrap {
                current % columns
            } else {
                current
            }
        }
    }
}

fn selected_scroll(
    selected_index: Option<usize>,
    columns: u16,
    cell_rows: u16,
    visible_rows: u16,
    total_virtual_rows: u16,
) -> u16 {
    let Some(index) = selected_index else {
        return 0;
    };
    if columns == 0 || total_virtual_rows <= visible_rows {
        return 0;
    }
    let selected_row = (index / usize::from(columns)).min(usize::from(u16::MAX)) as u16;
    let selected_bottom = selected_row
        .saturating_mul(cell_rows)
        .saturating_add(cell_rows);
    if selected_bottom > visible_rows {
        selected_bottom.saturating_sub(visible_rows)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::ImageSurface;
    use crate::layout::{CanvasMetrics, CellPixel, PixelSize};
    use crate::testkit::{MockImageCall, MockImageSurface};
    use crate::widgets::image_viewport::{
        ImageViewportInitialScale, ImageViewportOptions, ImageViewportWidget, ResizePolicy,
        ViewportImage,
    };
    use ratatui::style::Modifier;

    #[test]
    fn render_text_cells_with_local_coordinates() {
        let items = ["one", "two", "three"];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 12));
        let mut image_areas = Vec::new();

        let state = Grid::new().with_cell_rows(3).with_min_cell_cols(10).render(
            Rect::new(10, 4, 36, 6),
            &mut buffer,
            &items,
            |cell, canvas| {
                canvas.set_string(0, 0, *cell.item, canvas.style());
                image_areas.push(canvas.local_cell_area(1, 1, 2, 1));
            },
        );

        assert_eq!(state.columns, 3);
        assert_eq!(image_areas[0], CellArea::new(11, 5, 2, 1));
        assert_eq!(image_areas[1], CellArea::new(23, 5, 2, 1));
        assert_eq!(image_areas[2], CellArea::new(35, 5, 2, 1));
        let rendered = format!("{buffer:?}");
        assert!(rendered.contains("one"));
        assert!(rendered.contains("two"));
        assert!(rendered.contains("three"));
    }

    #[test]
    fn empty_grid_renders_default_state_and_ignores_input() {
        let items: [usize; 0] = [];
        let mut grid = Grid::new().with_cell_rows(2).with_active_index(Some(0));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 4));

        let state = grid.render(Rect::new(0, 0, 10, 4), &mut buffer, &items, |_, _| {});

        assert_eq!(state, GridRenderState::default());
        assert_eq!(
            grid.handle_key(KeyEvent::Enter, 10, items.len()),
            GridInputOutcome::Ignored
        );
        assert_eq!(grid.active_index(), None);
        assert_eq!(grid.columns_for_width(10, items.len()), 0);
    }

    #[test]
    fn render_image_and_text_cells_with_local_canvas_areas() {
        #[derive(Debug)]
        struct ImageGridItem {
            image_id: u32,
            label: &'static str,
            png: &'static [u8],
        }

        fn image() -> ViewportImage {
            ViewportImage::new(PixelSize::new(4, 2), vec![255; 4 * 2 * 4]).unwrap()
        }

        let items = [
            ImageGridItem {
                image_id: 7,
                label: "alpha",
                png: b"png-a",
            },
            ImageGridItem {
                image_id: 8,
                label: "beta",
                png: b"png-b",
            },
            ImageGridItem {
                image_id: 9,
                label: "gamma",
                png: b"png-c",
            },
        ];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 48, 8));
        let mut surface = MockImageSurface::default();
        let mut image_areas = Vec::new();

        let state = Grid::new().with_cell_rows(4).with_min_cell_cols(10).render(
            Rect::new(5, 2, 30, 4),
            &mut buffer,
            &items,
            |cell, canvas| {
                canvas.set_string(0, 0, cell.item.label, canvas.style());
                let image_area = canvas.local_cell_area(1, 1, canvas.width().saturating_sub(2), 2);
                image_areas.push(image_area);

                let widget = ImageViewportWidget::from_image_with_options(
                    image(),
                    CanvasMetrics::new(image_area.size, CellPixel::new(1, 1)),
                    ImageViewportOptions {
                        initial_scale: ImageViewportInitialScale::FitToBox,
                        resize_policy: ResizePolicy::PreserveTopLeft,
                    },
                )
                .unwrap();
                let placement = widget.placement().unwrap().unwrap();
                surface
                    .ensure_loaded(cell.item.image_id, cell.item.png)
                    .unwrap();
                surface
                    .place(placement.place_options(cell.item.image_id, 100 + cell.index as u32))
                    .unwrap();
            },
        );

        assert_eq!(state.columns, 3);
        assert_eq!(
            image_areas,
            vec![
                CellArea::new(6, 3, 8, 2),
                CellArea::new(16, 3, 8, 2),
                CellArea::new(26, 3, 8, 2),
            ]
        );
        assert_eq!(surface.calls().len(), 6);
        assert!(matches!(
            &surface.calls()[0],
            MockImageCall::EnsureLoaded {
                image_id: 7,
                bytes: 5
            }
        ));
        assert!(matches!(
            &surface.calls()[1],
            MockImageCall::Place(opts)
                if opts.image_id == 7
                    && opts.placement_id == 100
                    && opts.cell_cols == 4
                    && opts.cell_rows == 2
        ));
        assert!(matches!(
            surface.calls().last().unwrap(),
            MockImageCall::Place(opts)
                if opts.image_id == 9
                    && opts.placement_id == 102
                    && opts.cell_cols == 4
                    && opts.cell_rows == 2
        ));
        let rendered = format!("{buffer:?}");
        assert!(rendered.contains("alpha"));
        assert!(rendered.contains("beta"));
        assert!(rendered.contains("gamma"));
    }

    #[test]
    fn render_scrolls_selected_cell_into_view() {
        let items = [0, 1, 2, 3, 4, 5, 6, 7];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 5));

        let state = Grid::new()
            .with_cell_rows(4)
            .with_min_cell_cols(10)
            .with_selected_index(Some(7))
            .render(
                Rect::new(0, 0, 20, 5),
                &mut buffer,
                &items,
                |cell, canvas| {
                    canvas.set_string(0, 0, cell.item.to_string(), canvas.style());
                },
            );

        assert!(state.has_overflow_above());
        assert_eq!(state.total_virtual_rows, 16);
        assert!(state.visible_cells.iter().any(|cell| cell.index == 7));
    }

    #[test]
    fn one_column_grid_behaves_like_a_list() {
        let items = ["a", "b", "c"];
        let mut grid = Grid::new().with_columns(1).with_active_index(Some(0));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));

        let state = grid.render(
            Rect::new(0, 0, 10, 3),
            &mut buffer,
            &items,
            |cell, canvas| {
                canvas.set_string(0, 0, *cell.item, canvas.style());
            },
        );

        assert_eq!(state.columns, 1);
        assert_eq!(state.visible_cells[1].row, 1);
        assert_eq!(
            grid.handle_key(KeyEvent::Down, 10, items.len()),
            GridInputOutcome::Moved(1)
        );
        assert_eq!(
            grid.handle_key(KeyEvent::Right, 10, items.len()),
            GridInputOutcome::Moved(1)
        );
    }

    #[test]
    fn fixed_columns_are_capped_to_narrow_viewport_width() {
        let items = [0, 1, 2, 3, 4];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 3));

        let state = Grid::new().with_columns(3).render(
            Rect::new(0, 0, 2, 3),
            &mut buffer,
            &items,
            |cell, canvas| {
                canvas.set_string(0, 0, cell.item.to_string(), canvas.style());
            },
        );

        assert_eq!(state.columns, 2);
        assert!(state.visible_cells.iter().all(|cell| cell.area.width > 0));
    }

    #[test]
    fn navigation_without_wrap_stays_at_row_boundaries() {
        let mut grid = Grid::new().with_columns(3).with_active_index(Some(2));

        assert_eq!(
            grid.handle_key(KeyEvent::Right, 30, 5),
            GridInputOutcome::Moved(2)
        );
        assert_eq!(grid.active_index(), Some(2));

        grid = grid.with_active_index(Some(3));
        assert_eq!(
            grid.handle_key(KeyEvent::Left, 30, 5),
            GridInputOutcome::Moved(3)
        );
        assert_eq!(grid.active_index(), Some(3));
    }

    #[test]
    fn render_scrolls_active_cell_into_view_without_selection() {
        let items = [0, 1, 2, 3, 4, 5, 6, 7];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 5));

        let state = Grid::new()
            .with_cell_rows(4)
            .with_min_cell_cols(10)
            .with_active_index(Some(7))
            .render(
                Rect::new(0, 0, 20, 5),
                &mut buffer,
                &items,
                |cell, canvas| {
                    canvas.set_string(0, 0, cell.item.to_string(), canvas.style());
                },
            );

        assert!(state.has_overflow_above());
        assert_eq!(state.active_index, Some(7));
        assert!(state
            .visible_cells
            .iter()
            .any(|cell| cell.index == 7 && cell.active));
    }

    #[test]
    fn render_scroll_indicators_for_clipped_cells() {
        let items = [0, 1, 2, 3, 4, 5, 6, 7];
        let area = Rect::new(0, 0, 20, 5);
        let mut top_buffer = Buffer::empty(area);
        let mut bottom_buffer = Buffer::empty(area);

        Grid::new().with_cell_rows(4).with_min_cell_cols(10).render(
            area,
            &mut top_buffer,
            &items,
            |cell, canvas| {
                canvas.set_string(0, 0, cell.item.to_string(), canvas.style());
            },
        );
        Grid::new()
            .with_cell_rows(4)
            .with_min_cell_cols(10)
            .with_active_index(Some(7))
            .render(area, &mut bottom_buffer, &items, |cell, canvas| {
                canvas.set_string(0, 0, cell.item.to_string(), canvas.style());
            });

        assert_eq!(top_buffer.cell((19, 4)).unwrap().symbol(), "v");
        assert_eq!(bottom_buffer.cell((19, 0)).unwrap().symbol(), "^");
    }

    #[test]
    fn render_applies_selected_cell_style() {
        let items = ["a", "b"];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 2));

        Grid::new()
            .with_min_cell_cols(10)
            .with_selected_index(Some(1))
            .with_selected_cell_style(Style::default().add_modifier(Modifier::REVERSED))
            .render(
                Rect::new(0, 0, 20, 2),
                &mut buffer,
                &items,
                |cell, canvas| {
                    canvas.set_string(0, 0, *cell.item, canvas.style());
                },
            );

        let selected_cell = buffer.cell((10, 0)).expect("selected cell exists");
        assert!(selected_cell
            .style()
            .add_modifier
            .contains(Modifier::REVERSED));
    }

    #[test]
    fn keyboard_navigation_moves_and_selects_active_cell() {
        let mut grid = Grid::new()
            .with_columns(3)
            .with_active_index(Some(0))
            .with_wrap_navigation(true);

        assert_eq!(
            grid.handle_key(KeyEvent::Right, 30, 5),
            GridInputOutcome::Moved(1)
        );
        assert_eq!(
            grid.handle_key(KeyEvent::Down, 30, 5),
            GridInputOutcome::Moved(4)
        );
        assert_eq!(
            grid.handle_key(KeyEvent::Right, 30, 5),
            GridInputOutcome::Moved(0)
        );
        assert_eq!(
            grid.handle_key(KeyEvent::Enter, 30, 5),
            GridInputOutcome::Selected(0)
        );
        assert_eq!(grid.selected_index(), Some(0));
    }

    #[test]
    fn render_applies_active_cell_style_when_not_selected() {
        let items = ["a", "b"];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 2));

        Grid::new()
            .with_min_cell_cols(10)
            .with_active_index(Some(1))
            .with_active_cell_style(Style::default().add_modifier(Modifier::BOLD))
            .render(
                Rect::new(0, 0, 20, 2),
                &mut buffer,
                &items,
                |cell, canvas| {
                    canvas.set_string(0, 0, *cell.item, canvas.style());
                },
            );

        let active_cell = buffer.cell((10, 0)).expect("active cell exists");
        assert!(active_cell.style().add_modifier.contains(Modifier::BOLD));
    }
}
