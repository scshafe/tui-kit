//! Streamlined image viewport widget.
//!
//! [`ImageBox`] is the convenience layer for the common terminal image case:
//! keep source image dimensions, apply zoom to derive theoretical dimensions,
//! and crop that theoretical image to the available box. Lower-level consumers
//! can still use [`crate::layout`] and [`crate::image`] directly.

use crate::config::ConfigError;
use crate::image::{ImageSurface, PlaceOptions};
use crate::layout::{CanvasMetrics, CellOffset, CellSize, ImagePoint, PixelRect, PixelSize};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 16.0;

#[derive(Debug, Clone, PartialEq)]
pub struct ImageBox {
    image_id: u32,
    placement_id: u32,
    image_size: PixelSize,
    style: ImageBoxStyle,
}

impl ImageBox {
    pub fn new(image_id: u32, placement_id: u32, image_size: PixelSize) -> Self {
        Self {
            image_id,
            placement_id,
            image_size,
            style: ImageBoxStyle::default(),
        }
    }

    pub fn border(mut self, enabled: bool) -> Self {
        self.style.border = enabled;
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.style.title = Some(title.into());
        self
    }

    pub fn clear_title(mut self) -> Self {
        self.style.title = None;
        self
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.style.border_style = style;
        self
    }

    pub fn style(mut self, style: ImageBoxStyle) -> Self {
        self.style = style;
        self
    }

    pub fn image_id(&self) -> u32 {
        self.image_id
    }

    pub fn placement_id(&self) -> u32 {
        self.placement_id
    }

    pub fn image_size(&self) -> PixelSize {
        self.image_size
    }

    pub fn style_ref(&self) -> &ImageBoxStyle {
        &self.style
    }

    pub fn plan(
        &self,
        area: Rect,
        terminal_metrics: CanvasMetrics,
        state: &ImageBoxState,
    ) -> Result<ImageBoxPlan, ConfigError> {
        let image_area = self.style.image_area(area);
        let canvas = CanvasMetrics::new(
            CellSize::new(image_area.width, image_area.height),
            terminal_metrics.cell_pixel.or_fallback(),
        );
        let zoom = sanitize_zoom(state.zoom);
        let theoretical_pixels = theoretical_pixels(self.image_size, zoom);
        let box_pixels = canvas.pixels();
        let offset_x = state.offset_x.unwrap_or_else(|| {
            centered_offset(theoretical_pixels.width as f32, box_pixels.width as f32)
        });
        let offset_y = state.offset_y.unwrap_or_else(|| {
            centered_offset(theoretical_pixels.height as f32, box_pixels.height as f32)
        });
        let offset_x = clamp_offset(
            offset_x,
            theoretical_pixels.width as f32,
            box_pixels.width as f32,
        );
        let offset_y = clamp_offset(
            offset_y,
            theoretical_pixels.height as f32,
            box_pixels.height as f32,
        );
        let crop = crop_theoretical(theoretical_pixels, box_pixels, offset_x, offset_y);
        let placement = self.compute_placement(image_area, canvas, zoom, crop);

        Ok(ImageBoxPlan {
            area,
            image_area,
            image_size: self.image_size,
            canvas,
            zoom,
            offset_x,
            offset_y,
            theoretical_pixels,
            visible_pixels: crop.visible_pixels,
            style: self.style.clone(),
            placement,
        })
    }

    fn compute_placement(
        &self,
        image_area: Rect,
        canvas: CanvasMetrics,
        zoom: f32,
        crop: TheoreticalCrop,
    ) -> Option<ImageBoxPlacement> {
        if image_area.width == 0
            || image_area.height == 0
            || self.image_size.width == 0
            || self.image_size.height == 0
            || crop.visible_pixels.width == 0
            || crop.visible_pixels.height == 0
        {
            return None;
        }

        let cell_pixel = canvas.cell_pixel.or_fallback();
        let origin_col = image_area.x.saturating_add(round_to_cell_offset(
            crop.destination_offset_x,
            cell_pixel.width,
        ));
        let origin_row = image_area.y.saturating_add(round_to_cell_offset(
            crop.destination_offset_y,
            cell_pixel.height,
        ));
        let cell_cols = round_to_cells(crop.visible_pixels.width, cell_pixel.width).min(
            image_area
                .width
                .saturating_sub(origin_col.saturating_sub(image_area.x)),
        );
        let cell_rows = round_to_cells(crop.visible_pixels.height, cell_pixel.height).min(
            image_area
                .height
                .saturating_sub(origin_row.saturating_sub(image_area.y)),
        );
        if cell_cols == 0 || cell_rows == 0 {
            return None;
        }

        let mut source = PixelRect {
            x: (crop.left / zoom).round().max(0.0) as u32,
            y: (crop.top / zoom).round().max(0.0) as u32,
            width: (crop.visible_pixels.width as f32 / zoom)
                .round()
                .max(1.0)
                .min(self.image_size.width as f32) as u32,
            height: (crop.visible_pixels.height as f32 / zoom)
                .round()
                .max(1.0)
                .min(self.image_size.height as f32) as u32,
        };
        source.x = source.x.min(self.image_size.width.saturating_sub(1));
        source.y = source.y.min(self.image_size.height.saturating_sub(1));
        source.width = source
            .width
            .min(self.image_size.width.saturating_sub(source.x));
        source.height = source
            .height
            .min(self.image_size.height.saturating_sub(source.y));

        Some(ImageBoxPlacement {
            image_id: self.image_id,
            placement_id: self.placement_id,
            source,
            origin: CellOffset {
                col: origin_col,
                row: origin_row,
            },
            cell_cols,
            cell_rows,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImageBoxStyle {
    pub border: bool,
    pub border_style: Style,
    pub title: Option<String>,
}

impl ImageBoxStyle {
    fn image_area(&self, area: Rect) -> Rect {
        if !self.border {
            return area;
        }
        if area.width <= 2 || area.height <= 2 {
            return Rect::new(area.x.saturating_add(1), area.y.saturating_add(1), 0, 0);
        }
        Rect::new(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageBoxState {
    pub zoom: f32,
    pub offset_x: Option<f32>,
    pub offset_y: Option<f32>,
}

impl ImageBoxState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset_zoom(&mut self) {
        *self = Self::default();
    }

    pub fn zoom_at(&mut self, plan: &ImageBoxPlan, factor: f32, anchor_x: f32, anchor_y: f32) {
        if plan.placement.is_none() {
            return;
        }
        let box_pixels = plan.canvas.pixels();
        let old_zoom = sanitize_zoom(plan.zoom);
        let new_zoom = sanitize_zoom(old_zoom * factor);
        let anchor_x = anchor_x.clamp(0.0, 1.0) * box_pixels.width as f32;
        let anchor_y = anchor_y.clamp(0.0, 1.0) * box_pixels.height as f32;
        let source_x = (plan.offset_x + anchor_x) / old_zoom;
        let source_y = (plan.offset_y + anchor_y) / old_zoom;
        let theoretical = theoretical_pixels(plan.image_size, new_zoom);
        self.zoom = new_zoom;
        self.offset_x = Some(clamp_offset(
            source_x * new_zoom - anchor_x,
            theoretical.width as f32,
            box_pixels.width as f32,
        ));
        self.offset_y = Some(clamp_offset(
            source_y * new_zoom - anchor_y,
            theoretical.height as f32,
            box_pixels.height as f32,
        ));
    }

    pub fn pan(&mut self, plan: &ImageBoxPlan, dx_fraction: f32, dy_fraction: f32) {
        if plan.placement.is_none() {
            return;
        }
        let box_pixels = plan.canvas.pixels();
        self.zoom = plan.zoom;
        self.offset_x = Some(clamp_offset(
            plan.offset_x + dx_fraction * box_pixels.width as f32,
            plan.theoretical_pixels.width as f32,
            box_pixels.width as f32,
        ));
        self.offset_y = Some(clamp_offset(
            plan.offset_y + dy_fraction * box_pixels.height as f32,
            plan.theoretical_pixels.height as f32,
            box_pixels.height as f32,
        ));
    }
}

impl Default for ImageBoxState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            offset_x: None,
            offset_y: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageBoxPlan {
    pub area: Rect,
    pub image_area: Rect,
    pub image_size: PixelSize,
    pub canvas: CanvasMetrics,
    pub zoom: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub theoretical_pixels: PixelSize,
    pub visible_pixels: PixelSize,
    pub style: ImageBoxStyle,
    pub placement: Option<ImageBoxPlacement>,
}

impl ImageBoxPlan {
    pub fn render(&self, buf: &mut Buffer) {
        clear_rect(buf, self.image_area);
        if self.style.border {
            draw_border(buf, self.area, &self.style);
        }
    }

    pub fn placement_options(&self) -> Option<PlaceOptions> {
        self.placement
            .as_ref()
            .map(ImageBoxPlacement::place_options)
    }

    pub fn place(&self, surface: &mut impl ImageSurface) -> Result<()> {
        let Some(placement) = &self.placement else {
            return Ok(());
        };
        position_cursor(placement.origin)?;
        surface.place(placement.place_options())
    }

    pub fn normalized_to_image(&self, x: f32, y: f32) -> ImagePoint {
        let Some(placement) = &self.placement else {
            return ImagePoint {
                x: 0.0,
                y: 0.0,
                inside: false,
            };
        };
        image_point_in_placement(placement, x, y, self.image_area, self.canvas, self.zoom)
    }

    pub fn cell_to_image(&self, col: u16, row: u16) -> ImagePoint {
        if self.image_area.width == 0 || self.image_area.height == 0 {
            return ImagePoint {
                x: 0.0,
                y: 0.0,
                inside: false,
            };
        }
        let local_x = (f32::from(col) + 0.5 - f32::from(self.image_area.x))
            / f32::from(self.image_area.width);
        let local_y = (f32::from(row) + 0.5 - f32::from(self.image_area.y))
            / f32::from(self.image_area.height);
        self.normalized_to_image(local_x, local_y)
    }
}

fn image_point_in_placement(
    placement: &ImageBoxPlacement,
    canvas_x: f32,
    canvas_y: f32,
    image_area: Rect,
    canvas: CanvasMetrics,
    zoom: f32,
) -> ImagePoint {
    let cell_pixel = canvas.cell_pixel.or_fallback();
    let canvas_pixels = canvas.pixels();
    let cursor_pixel_x = canvas_x * canvas_pixels.width as f32;
    let cursor_pixel_y = canvas_y * canvas_pixels.height as f32;
    let origin_pixel_x =
        f32::from(placement.origin.col.saturating_sub(image_area.x)) * f32::from(cell_pixel.width);
    let origin_pixel_y =
        f32::from(placement.origin.row.saturating_sub(image_area.y)) * f32::from(cell_pixel.height);
    let target_pixel_w = f32::from(placement.cell_cols) * f32::from(cell_pixel.width);
    let target_pixel_h = f32::from(placement.cell_rows) * f32::from(cell_pixel.height);
    let local_x = (cursor_pixel_x - origin_pixel_x) / target_pixel_w.max(1.0);
    let local_y = (cursor_pixel_y - origin_pixel_y) / target_pixel_h.max(1.0);
    let inside = (0.0..=1.0).contains(&local_x) && (0.0..=1.0).contains(&local_y);
    let local_x = local_x.clamp(0.0, 1.0);
    let local_y = local_y.clamp(0.0, 1.0);
    ImagePoint {
        x: placement.source.x as f32 + local_x * target_pixel_w / zoom,
        y: placement.source.y as f32 + local_y * target_pixel_h / zoom,
        inside,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageBoxPlacement {
    pub image_id: u32,
    pub placement_id: u32,
    pub source: crate::layout::PixelRect,
    /// Absolute terminal-cell origin, using ratatui's zero-based coordinates.
    pub origin: CellOffset,
    pub cell_cols: u16,
    pub cell_rows: u16,
}

impl ImageBoxPlacement {
    pub fn place_options(&self) -> PlaceOptions {
        PlaceOptions {
            image_id: self.image_id,
            placement_id: self.placement_id,
            source: self.source,
            cell_cols: self.cell_cols,
            cell_rows: self.cell_rows,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TheoreticalCrop {
    left: f32,
    top: f32,
    visible_pixels: PixelSize,
    destination_offset_x: f32,
    destination_offset_y: f32,
}

fn sanitize_zoom(zoom: f32) -> f32 {
    if !zoom.is_finite() {
        return 1.0;
    }
    zoom.clamp(MIN_ZOOM, MAX_ZOOM)
}

fn theoretical_pixels(image: PixelSize, zoom: f32) -> PixelSize {
    PixelSize::new(
        (image.width as f32 * zoom)
            .round()
            .max(1.0)
            .min(u32::MAX as f32) as u32,
        (image.height as f32 * zoom)
            .round()
            .max(1.0)
            .min(u32::MAX as f32) as u32,
    )
}

fn centered_offset(theoretical: f32, available: f32) -> f32 {
    (theoretical - available) / 2.0
}

fn clamp_offset(offset: f32, theoretical: f32, available: f32) -> f32 {
    if theoretical <= available {
        centered_offset(theoretical, available)
    } else {
        offset.clamp(0.0, theoretical - available)
    }
}

fn crop_theoretical(
    theoretical: PixelSize,
    available: PixelSize,
    offset_x: f32,
    offset_y: f32,
) -> TheoreticalCrop {
    let left = offset_x.max(0.0);
    let top = offset_y.max(0.0);
    let right = (offset_x + available.width as f32).min(theoretical.width as f32);
    let bottom = (offset_y + available.height as f32).min(theoretical.height as f32);
    let width = (right - left).round().max(0.0) as u32;
    let height = (bottom - top).round().max(0.0) as u32;

    TheoreticalCrop {
        left,
        top,
        visible_pixels: PixelSize::new(width, height),
        destination_offset_x: (left - offset_x).max(0.0),
        destination_offset_y: (top - offset_y).max(0.0),
    }
}

fn round_to_cell_offset(pixels: f32, cell_pixels: u16) -> u16 {
    (pixels / f32::from(cell_pixels.max(1)))
        .round()
        .max(0.0)
        .min(u16::MAX as f32) as u16
}

fn round_to_cells(pixels: u32, cell_pixels: u16) -> u16 {
    (pixels as f32 / f32::from(cell_pixels.max(1)))
        .round()
        .max(1.0)
        .min(u16::MAX as f32) as u16
}

fn draw_border(buf: &mut Buffer, area: Rect, style: &ImageBoxStyle) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let right = rect_right(area);
    let bottom = rect_bottom(area);
    for x in area.x..right {
        write_cell(buf, x, area.y, "-", style.border_style);
        if area.height > 1 {
            write_cell(buf, x, bottom.saturating_sub(1), "-", style.border_style);
        }
    }

    if area.width > 1 && area.height > 2 {
        for y in area.y.saturating_add(1)..bottom.saturating_sub(1) {
            write_cell(buf, area.x, y, "|", style.border_style);
            write_cell(buf, right.saturating_sub(1), y, "|", style.border_style);
        }
    }

    let Some(title) = style.title.as_deref() else {
        return;
    };
    let max_title_width = area.width.saturating_sub(2) as usize;
    if max_title_width == 0 {
        return;
    }
    let title = format!(" {} ", title.trim());
    for (idx, ch) in title.chars().take(max_title_width).enumerate() {
        let mut encoded = [0; 4];
        let symbol = ch.encode_utf8(&mut encoded);
        write_cell(
            buf,
            area.x.saturating_add(1).saturating_add(idx as u16),
            area.y,
            symbol,
            style.border_style,
        );
    }
}

fn clear_rect(buf: &mut Buffer, area: Rect) {
    for y in area.y..rect_bottom(area) {
        for x in area.x..rect_right(area) {
            write_cell(buf, x, y, " ", Style::default());
        }
    }
}

fn write_cell(buf: &mut Buffer, x: u16, y: u16, symbol: &str, style: Style) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_symbol(symbol).set_style(style);
    }
}

fn rect_right(area: Rect) -> u16 {
    area.x.saturating_add(area.width)
}

fn rect_bottom(area: Rect) -> u16 {
    area.y.saturating_add(area.height)
}

#[cfg(not(test))]
fn position_cursor(origin: CellOffset) -> Result<()> {
    use std::io::Write as _;

    write!(
        std::io::stdout().lock(),
        "\x1b[{};{}H",
        origin.row.saturating_add(1).max(1),
        origin.col.saturating_add(1).max(1)
    )?;
    Ok(())
}

#[cfg(test)]
fn position_cursor(_origin: CellOffset) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{CellPixel, CellSize};
    use crate::testkit::{MockImageCall, MockImageSurface};
    use ratatui::style::Color;

    fn metrics(cols: u16, rows: u16) -> CanvasMetrics {
        CanvasMetrics::new(CellSize::new(cols, rows), CellPixel::new(8, 16))
    }

    fn image_box() -> ImageBox {
        ImageBox::new(7, 9, PixelSize::new(100, 200))
    }

    fn pixel_metrics(cols: u16, rows: u16) -> CanvasMetrics {
        CanvasMetrics::new(CellSize::new(cols, rows), CellPixel::new(1, 1))
    }

    #[test]
    fn image_box_initial_view_shows_full_source_when_it_is_smaller_than_box() {
        let state = ImageBoxState::default();
        let plan = image_box()
            .plan(Rect::new(0, 0, 200, 200), pixel_metrics(200, 200), &state)
            .unwrap();
        let placement = plan.placement.unwrap();

        assert_eq!(plan.theoretical_pixels, PixelSize::new(100, 200));
        assert_eq!(plan.visible_pixels, PixelSize::new(100, 200));
        assert_eq!(placement.source.x, 0);
        assert_eq!(placement.source.y, 0);
        assert_eq!(placement.source.width, 100);
        assert_eq!(placement.source.height, 200);
        assert_eq!(placement.origin.col, 50);
        assert_eq!(placement.origin.row, 0);
        assert_eq!(placement.cell_cols, 100);
        assert_eq!(placement.cell_rows, 200);
    }

    #[test]
    fn image_box_zoom_in_crops_source() {
        let state = ImageBoxState {
            zoom: 2.0,
            ..Default::default()
        };
        let plan = image_box()
            .plan(Rect::new(0, 0, 200, 200), pixel_metrics(200, 200), &state)
            .unwrap();
        let placement = plan.placement.unwrap();

        assert_eq!(plan.theoretical_pixels, PixelSize::new(200, 400));
        assert_eq!(plan.visible_pixels, PixelSize::new(200, 200));
        assert_eq!(placement.source.x, 0);
        assert_eq!(placement.source.y, 50);
        assert_eq!(placement.source.width, 100);
        assert_eq!(placement.source.height, 100);
        assert_eq!(placement.cell_cols, 200);
        assert_eq!(placement.cell_rows, 200);
    }

    #[test]
    fn image_box_zoom_crops_theoretical_image_to_box() {
        let state = ImageBoxState {
            zoom: 1.25,
            ..Default::default()
        };
        let zoomed = image_box()
            .plan(Rect::new(0, 0, 200, 200), pixel_metrics(200, 200), &state)
            .unwrap();
        let placement = zoomed.placement.as_ref().unwrap();

        assert_eq!(zoomed.theoretical_pixels, PixelSize::new(125, 250));
        assert_eq!(zoomed.visible_pixels, PixelSize::new(125, 200));
        assert_eq!(placement.cell_cols, 125);
        assert_eq!(placement.cell_rows, 200);
        assert_eq!(placement.source.width, 100);
        assert_eq!(placement.source.height, 160);
        assert_eq!(placement.source.x, 0);
        assert_eq!(placement.source.y, 20);
    }

    #[test]
    fn image_box_zoom_two_and_a_half_uses_centered_crop() {
        let state = ImageBoxState {
            zoom: 2.5,
            ..Default::default()
        };
        let plan = image_box()
            .plan(Rect::new(0, 0, 200, 200), pixel_metrics(200, 200), &state)
            .unwrap();
        let placement = plan.placement.unwrap();

        assert_eq!(plan.theoretical_pixels, PixelSize::new(250, 500));
        assert_eq!(plan.visible_pixels, PixelSize::new(200, 200));
        assert_eq!(placement.source.x, 10);
        assert_eq!(placement.source.y, 60);
        assert_eq!(placement.source.width, 80);
        assert_eq!(placement.source.height, 80);
        assert_eq!(placement.cell_cols, 200);
        assert_eq!(placement.cell_rows, 200);
    }

    #[test]
    fn image_box_zoom_shrinks_source_crop_after_target_cells_fill_canvas() {
        let canvas = CanvasMetrics::new(CellSize::new(80, 30), CellPixel::new(10, 20));
        let image = ImageBox::new(7, 9, PixelSize::new(1600, 1000));
        let area = Rect::new(0, 0, 80, 30);

        let native = image
            .plan(
                area,
                canvas,
                &ImageBoxState {
                    zoom: 1.0,
                    ..Default::default()
                },
            )
            .unwrap()
            .placement
            .unwrap();
        let zoomed = image
            .plan(
                area,
                canvas,
                &ImageBoxState {
                    zoom: 2.0,
                    ..Default::default()
                },
            )
            .unwrap()
            .placement
            .unwrap();
        let zoomed_again = image
            .plan(
                area,
                canvas,
                &ImageBoxState {
                    zoom: 4.0,
                    ..Default::default()
                },
            )
            .unwrap()
            .placement
            .unwrap();

        assert_eq!((native.cell_cols, native.cell_rows), (80, 30));
        assert_eq!(
            (zoomed.cell_cols, zoomed.cell_rows),
            (native.cell_cols, native.cell_rows)
        );
        assert_eq!(
            (zoomed_again.cell_cols, zoomed_again.cell_rows),
            (native.cell_cols, native.cell_rows)
        );
        assert!(zoomed.source.width < native.source.width);
        assert!(zoomed.source.height < native.source.height);
        assert!(zoomed_again.source.width < zoomed.source.width);
        assert!(zoomed_again.source.height < zoomed.source.height);
    }

    #[test]
    fn image_box_pan_moves_source_crop() {
        let mut state = ImageBoxState {
            zoom: 2.5,
            ..Default::default()
        };
        let initial = image_box()
            .plan(Rect::new(0, 0, 100, 100), pixel_metrics(100, 100), &state)
            .unwrap();
        let before = initial.placement.as_ref().unwrap().source;

        state.pan(&initial, 0.25, 0.25);
        let after_plan = image_box()
            .plan(Rect::new(0, 0, 100, 100), pixel_metrics(100, 100), &state)
            .unwrap();
        let after = after_plan.placement.unwrap().source;

        assert!(after.x > before.x);
        assert!(after.y > before.y);
    }

    #[test]
    fn image_box_zoom_at_preserves_anchor() {
        let mut state = ImageBoxState::default();
        let plan = image_box()
            .plan(Rect::new(0, 0, 100, 100), pixel_metrics(100, 100), &state)
            .unwrap();
        let before = plan.normalized_to_image(0.25, 0.5);

        state.zoom_at(&plan, 2.0, 0.25, 0.5);
        let after_plan = image_box()
            .plan(Rect::new(0, 0, 100, 100), pixel_metrics(100, 100), &state)
            .unwrap();
        let after = after_plan.normalized_to_image(0.25, 0.5);

        assert!(before.inside);
        assert!(after.inside);
        assert!(
            (before.x - after.x).abs() < 16.0,
            "x {} -> {}",
            before.x,
            after.x
        );
        assert!(
            (before.y - after.y).abs() < 16.0,
            "y {} -> {}",
            before.y,
            after.y
        );
    }

    #[test]
    fn image_box_border_insets_image_area() {
        let state = ImageBoxState::default();
        let plan = image_box()
            .border(true)
            .plan(Rect::new(5, 6, 20, 10), metrics(20, 10), &state)
            .unwrap();

        assert_eq!(plan.image_area, Rect::new(6, 7, 18, 8));
    }

    #[test]
    fn image_box_border_renders_ascii_border_and_title() {
        let state = ImageBoxState::default();
        let plan = image_box()
            .border(true)
            .title("Diagram")
            .border_style(Style::default().fg(Color::Cyan))
            .plan(Rect::new(0, 0, 16, 5), metrics(16, 5), &state)
            .unwrap();
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 5));

        plan.render(&mut buf);

        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "-");
        assert_eq!(buf.cell((15, 0)).unwrap().symbol(), "-");
        assert_eq!(buf.cell((0, 2)).unwrap().symbol(), "|");
        assert_eq!(buf.cell((15, 2)).unwrap().symbol(), "|");
        let rendered = format!("{buf:?}");
        assert!(rendered.contains("Diagram"));
    }

    #[test]
    fn image_box_title_clips_without_panic() {
        let state = ImageBoxState::default();
        let plan = image_box()
            .border(true)
            .title("title too long for the box")
            .plan(Rect::new(0, 0, 3, 3), metrics(3, 3), &state)
            .unwrap();
        let mut buf = Buffer::empty(Rect::new(0, 0, 3, 3));

        plan.render(&mut buf);

        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "-");
        assert_eq!(buf.cell((2, 0)).unwrap().symbol(), "-");
    }

    #[test]
    fn image_box_plan_places_image_at_origin() {
        let state = ImageBoxState::default();
        let plan = image_box()
            .border(true)
            .plan(Rect::new(10, 20, 30, 20), metrics(30, 20), &state)
            .unwrap();
        let placement = plan.placement.as_ref().unwrap();
        let mut surface = MockImageSurface::default();

        plan.place(&mut surface).unwrap();

        assert!(placement.origin.col >= plan.image_area.x);
        assert!(placement.origin.row >= plan.image_area.y);
        assert_eq!(
            surface.calls(),
            &[MockImageCall::Place(placement.place_options())]
        );
    }

    #[test]
    fn image_box_cell_to_image_marks_cells_outside_image_area() {
        let state = ImageBoxState::default();
        let plan = image_box()
            .border(true)
            .plan(Rect::new(10, 20, 30, 20), metrics(30, 20), &state)
            .unwrap();

        assert!(!plan.cell_to_image(10, 20).inside);
    }

    #[test]
    fn image_box_no_border_uses_full_area() {
        let state = ImageBoxState::default();
        let plan = image_box()
            .plan(Rect::new(2, 3, 40, 12), metrics(40, 12), &state)
            .unwrap();

        assert_eq!(plan.image_area, Rect::new(2, 3, 40, 12));
    }

    #[test]
    fn image_box_tiny_area_degrades_safely() {
        let state = ImageBoxState::default();
        for area in [
            Rect::new(0, 0, 0, 0),
            Rect::new(0, 0, 1, 1),
            Rect::new(0, 0, 2, 2),
        ] {
            let plan = image_box()
                .border(true)
                .plan(area, metrics(area.width, area.height), &state)
                .unwrap();
            let mut buf = Buffer::empty(area);
            plan.render(&mut buf);
            assert!(plan.placement.is_none());
        }
    }

    #[test]
    fn image_box_reset_returns_to_default_zoom() {
        let mut state = ImageBoxState {
            zoom: 2.0,
            ..Default::default()
        };
        let zoomed = image_box()
            .plan(Rect::new(0, 0, 200, 200), pixel_metrics(200, 200), &state)
            .unwrap();
        assert!(zoomed.placement.as_ref().unwrap().source.height < 200);

        state.reset_zoom();
        let reset = image_box()
            .plan(Rect::new(0, 0, 200, 200), pixel_metrics(200, 200), &state)
            .unwrap();

        assert_eq!(reset.placement.unwrap().source.height, 200);
    }
}
