#![allow(dead_code)]

//! Pixel/cell layout math for image-heavy terminal UIs.
//!
//! This module owns deterministic fit, zoom, pan, clipping, placement, and
//! tail-following viewport calculations while leaving product policy (which
//! image to show, when to zoom, which controls map to pan, how log rows render)
//! in applications.
//!
//! **Stability:** consumed by c4tui's image canvas, placement code, and in-app
//! log viewer. The explicit placement policy enums and [`TailViewport`] are
//! retained because c4tui needs precise WezTerm/Kitty image behavior and
//! reusable tail-scroll math; new policy axes should be driven by concrete
//! consumer rendering cases.

use crate::config::{ConfigError, Validate};
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

impl PixelSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub const fn area(self) -> u64 {
        (self.width as u64) * (self.height as u64)
    }

    pub fn aspect_ratio(self) -> f32 {
        if self.height == 0 {
            return 1.0;
        }
        self.width as f32 / self.height as f32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellSize {
    pub cols: u16,
    pub rows: u16,
}

impl CellSize {
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellPixel {
    pub width: u16,
    pub height: u16,
}

impl CellPixel {
    pub const FALLBACK: Self = Self {
        width: 8,
        height: 16,
    };

    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    pub fn or_fallback(self) -> Self {
        Self {
            width: if self.width == 0 {
                Self::FALLBACK.width
            } else {
                self.width
            },
            height: if self.height == 0 {
                Self::FALLBACK.height
            } else {
                self.height
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasMetrics {
    pub cells: CellSize,
    pub cell_pixel: CellPixel,
}

impl CanvasMetrics {
    pub const fn new(cells: CellSize, cell_pixel: CellPixel) -> Self {
        Self { cells, cell_pixel }
    }

    pub fn pixels(self) -> PixelSize {
        let pixel = self.cell_pixel.or_fallback();
        PixelSize::new(
            u32::from(self.cells.cols) * u32::from(pixel.width),
            u32::from(self.cells.rows) * u32::from(pixel.height),
        )
    }
}

/// A deterministic viewport for tail-following collections such as logs.
///
/// `scroll_back == 0` means the viewport is pinned to the newest items at the
/// end of the collection. Larger values move the window toward older items.
/// tui-kit owns only the math; applications still own item storage, keymaps,
/// rendering, copy/yank behavior, and product language.
///
/// **Stability:** consumed by c4tui's in-app log viewer. Keep this primitive
/// limited to reusable tail viewport mechanics unless another consumer asks for
/// richer list-widget behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailViewport {
    pub total_items: usize,
    pub height: usize,
    pub scroll_back: usize,
    pub start: usize,
    pub end: usize,
    pub can_scroll_up: bool,
    pub can_scroll_down: bool,
}

impl TailViewport {
    pub fn new(total_items: usize, height: usize, scroll_back: usize) -> Self {
        let height = height.max(1);
        let max_scroll_back = total_items.saturating_sub(height);
        let scroll_back = scroll_back.min(max_scroll_back);
        let end = total_items.saturating_sub(scroll_back);
        let start = end.saturating_sub(height);
        Self {
            total_items,
            height,
            scroll_back,
            start,
            end,
            can_scroll_up: start > 0,
            can_scroll_down: scroll_back > 0,
        }
    }

    pub fn visible_range(self) -> Range<usize> {
        self.start..self.end
    }

    pub fn max_scroll_back(self) -> usize {
        self.total_items.saturating_sub(self.height)
    }

    pub fn scroll_by(self, delta_up: isize) -> usize {
        if delta_up > 0 {
            self.scroll_back
                .saturating_add(delta_up as usize)
                .min(self.max_scroll_back())
        } else {
            self.scroll_back.saturating_sub(delta_up.unsigned_abs())
        }
    }

    pub fn scroll_to_top(self) -> usize {
        self.max_scroll_back()
    }

    pub const fn scroll_to_bottom() -> usize {
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRect {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellOffset {
    pub col: u16,
    pub row: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub source: PixelRect,
    pub size: CellRect,
    pub origin: CellOffset,
    pub effective_scale: f32,
    pub fit_scale: f32,
    pub visible_pixels: PixelSize,
    pub unclipped_display_pixels: PixelSize,
    pub clipped_sides: ClippedSides,
    pub anchor: PlacementAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClippedSides {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl ClippedSides {
    pub const NONE: Self = Self {
        left: false,
        right: false,
        top: false,
        bottom: false,
    };

    pub const fn any(self) -> bool {
        self.left || self.right || self.top || self.bottom
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlacementAnchor {
    pub image_x: f32,
    pub image_y: f32,
    pub normalized_x: f32,
    pub normalized_y: f32,
}

pub const MIN_SCALE: f32 = 0.1;
pub const MAX_SCALE: f32 = 16.0;
pub const DEFAULT_CENTER: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ImageZoomLimitPolicy {
    ClampScale { min: f32, max: f32 },
    ClampAtFitBounds,
    AllowUnboundedWithinProtocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ImageOverflowPolicy {
    FitWithinArea,
    CropSourceToArea,
    OverflowAndClipDestination,
    PreventZoomBeyondArea,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ImageScaleBasis {
    NativePixels,
    FitToArea,
    FillArea,
    ExplicitScale(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ImageAnchorPolicy {
    Center,
    PreserveCursorAnchor,
    PreserveImagePoint { x: f32, y: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CellRoundingPolicy {
    Nearest,
    Floor,
    Ceil,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacementPolicy {
    pub scale_basis: ImageScaleBasis,
    pub zoom_limit: ImageZoomLimitPolicy,
    pub overflow: ImageOverflowPolicy,
    pub anchor: ImageAnchorPolicy,
    pub min_visible_pixels: PixelSize,
    pub cell_rounding: CellRoundingPolicy,
}

impl PlacementPolicy {
    pub const fn crop_fit_centered() -> Self {
        Self {
            scale_basis: ImageScaleBasis::FitToArea,
            zoom_limit: ImageZoomLimitPolicy::ClampScale {
                min: MIN_SCALE,
                max: MAX_SCALE,
            },
            overflow: ImageOverflowPolicy::CropSourceToArea,
            anchor: ImageAnchorPolicy::Center,
            min_visible_pixels: PixelSize::new(1, 1),
            cell_rounding: CellRoundingPolicy::Nearest,
        }
    }
}

impl Validate for PlacementPolicy {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.min_visible_pixels.width == 0 || self.min_visible_pixels.height == 0 {
            return Err(ConfigError::new(
                "layout.placement_policy.min_visible_pixels",
                "width and height must both be non-zero",
            ));
        }
        if let ImageScaleBasis::ExplicitScale(scale) = self.scale_basis {
            validate_positive_finite("layout.placement_policy.scale_basis", scale)?;
        }
        if let ImageZoomLimitPolicy::ClampScale { min, max } = self.zoom_limit {
            validate_positive_finite("layout.placement_policy.zoom_limit.min", min)?;
            validate_positive_finite("layout.placement_policy.zoom_limit.max", max)?;
            if min > max {
                return Err(ConfigError::new(
                    "layout.placement_policy.zoom_limit",
                    "min must be less than or equal to max",
                ));
            }
        }
        if let ImageAnchorPolicy::PreserveImagePoint { x, y } = self.anchor {
            if !x.is_finite() || !y.is_finite() {
                return Err(ConfigError::new(
                    "layout.placement_policy.anchor",
                    "image point coordinates must be finite",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlacementEngine {
    policy: PlacementPolicy,
}

impl PlacementEngine {
    pub fn new(policy: PlacementPolicy) -> Result<Self, ConfigError> {
        policy.validate()?;
        Ok(Self { policy })
    }

    pub fn place(
        &self,
        image: PixelSize,
        canvas: CanvasMetrics,
        transform: ViewTransform,
    ) -> Placement {
        place_with_policy(image, canvas, transform, &self.policy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    pub scale: f32,
    pub center_x: f32,
    pub center_y: f32,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self::fit()
    }
}

impl ViewTransform {
    pub const fn fit() -> Self {
        Self {
            scale: 1.0,
            center_x: DEFAULT_CENTER,
            center_y: DEFAULT_CENTER,
        }
    }

    pub fn with_scale(self, scale: f32) -> Self {
        Self {
            scale: clamp_scale(scale),
            ..self
        }
    }

    pub fn place(self, image: PixelSize, canvas: CanvasMetrics) -> Placement {
        PlacementEngine::new(PlacementPolicy::crop_fit_centered())
            .expect("built-in placement policy is valid")
            .place(image, canvas, self)
    }

    pub fn zoomed_at(
        self,
        factor: f32,
        anchor_canvas_x: f32,
        anchor_canvas_y: f32,
        image: PixelSize,
        canvas: CanvasMetrics,
    ) -> Self {
        let new_scale = clamp_scale(self.scale * factor);
        if (new_scale - self.scale).abs() < f32::EPSILON {
            return self;
        }
        let anchor_image = self.canvas_to_image(anchor_canvas_x, anchor_canvas_y, image, canvas);
        let pivoted = Self {
            scale: new_scale,
            center_x: self.center_x,
            center_y: self.center_y,
        };
        pivoted.recenter_so_anchor_stays(
            anchor_image,
            anchor_canvas_x,
            anchor_canvas_y,
            image,
            canvas,
        )
    }

    pub fn panned(
        self,
        dx_canvas_fraction: f32,
        dy_canvas_fraction: f32,
        image: PixelSize,
        canvas: CanvasMetrics,
    ) -> Self {
        let placement = self.place(image, canvas);
        let dx_image_pixels = dx_canvas_fraction * placement.source.width as f32;
        let dy_image_pixels = dy_canvas_fraction * placement.source.height as f32;
        let dx_fraction = dx_image_pixels / image.width.max(1) as f32;
        let dy_fraction = dy_image_pixels / image.height.max(1) as f32;
        Self {
            scale: self.scale,
            center_x: (self.center_x + dx_fraction).clamp(0.0, 1.0),
            center_y: (self.center_y + dy_fraction).clamp(0.0, 1.0),
        }
    }

    pub fn canvas_to_image(
        self,
        canvas_x: f32,
        canvas_y: f32,
        image: PixelSize,
        canvas: CanvasMetrics,
    ) -> ImagePoint {
        let placement = self.place(image, canvas);
        let cell_pixel = canvas.cell_pixel.or_fallback();
        let canvas_x = canvas_x.clamp(0.0, 1.0);
        let canvas_y = canvas_y.clamp(0.0, 1.0);
        let canvas_pixels = canvas.pixels();
        let cursor_pixel_x = canvas_x * canvas_pixels.width as f32;
        let cursor_pixel_y = canvas_y * canvas_pixels.height as f32;
        let origin_pixel_x = f32::from(placement.origin.col) * f32::from(cell_pixel.width);
        let origin_pixel_y = f32::from(placement.origin.row) * f32::from(cell_pixel.height);
        let target_pixel_w = f32::from(placement.size.cols) * f32::from(cell_pixel.width);
        let target_pixel_h = f32::from(placement.size.rows) * f32::from(cell_pixel.height);
        let local_x = (cursor_pixel_x - origin_pixel_x) / target_pixel_w.max(1.0);
        let local_y = (cursor_pixel_y - origin_pixel_y) / target_pixel_h.max(1.0);
        let inside = (0.0..=1.0).contains(&local_x) && (0.0..=1.0).contains(&local_y);
        let local_x = local_x.clamp(0.0, 1.0);
        let local_y = local_y.clamp(0.0, 1.0);
        ImagePoint {
            x: placement.source.x as f32 + local_x * placement.source.width as f32,
            y: placement.source.y as f32 + local_y * placement.source.height as f32,
            inside,
        }
    }

    fn recenter_so_anchor_stays(
        self,
        anchor_image: ImagePoint,
        anchor_canvas_x: f32,
        anchor_canvas_y: f32,
        image: PixelSize,
        canvas: CanvasMetrics,
    ) -> Self {
        let placement_at_new_scale = Self {
            scale: self.scale,
            center_x: anchor_image.x / image.width.max(1) as f32,
            center_y: anchor_image.y / image.height.max(1) as f32,
        }
        .place(image, canvas);
        let cell_pixel = canvas.cell_pixel.or_fallback();
        let canvas_pixels = canvas.pixels();
        let anchor_pixel_x = anchor_canvas_x.clamp(0.0, 1.0) * canvas_pixels.width as f32;
        let anchor_pixel_y = anchor_canvas_y.clamp(0.0, 1.0) * canvas_pixels.height as f32;
        let origin_pixel_x =
            f32::from(placement_at_new_scale.origin.col) * f32::from(cell_pixel.width);
        let origin_pixel_y =
            f32::from(placement_at_new_scale.origin.row) * f32::from(cell_pixel.height);
        let target_pixel_w =
            f32::from(placement_at_new_scale.size.cols) * f32::from(cell_pixel.width);
        let target_pixel_h =
            f32::from(placement_at_new_scale.size.rows) * f32::from(cell_pixel.height);
        let local_x = ((anchor_pixel_x - origin_pixel_x) / target_pixel_w.max(1.0)).clamp(0.0, 1.0);
        let local_y = ((anchor_pixel_y - origin_pixel_y) / target_pixel_h.max(1.0)).clamp(0.0, 1.0);
        let center_image_x =
            anchor_image.x - (local_x - 0.5) * placement_at_new_scale.source.width as f32;
        let center_image_y =
            anchor_image.y - (local_y - 0.5) * placement_at_new_scale.source.height as f32;
        Self {
            scale: self.scale,
            center_x: (center_image_x / image.width.max(1) as f32).clamp(0.0, 1.0),
            center_y: (center_image_y / image.height.max(1) as f32).clamp(0.0, 1.0),
        }
    }
}

fn place_with_policy(
    image: PixelSize,
    canvas: CanvasMetrics,
    transform: ViewTransform,
    policy: &PlacementPolicy,
) -> Placement {
    let canvas_pixels = canvas.pixels();
    let cell_pixel = canvas.cell_pixel.or_fallback();
    let fit = fit_scale(image, canvas_pixels);
    let base = scale_basis(policy.scale_basis, image, canvas_pixels, fit);
    let scale = apply_zoom_limit(transform.scale, policy.zoom_limit, fit);
    let mut effective = (base * scale).max(f32::EPSILON);

    if matches!(
        policy.overflow,
        ImageOverflowPolicy::FitWithinArea | ImageOverflowPolicy::PreventZoomBeyondArea
    ) {
        effective = effective.min(fit).max(f32::EPSILON);
    }

    let display_w = image.width as f32 * effective;
    let display_h = image.height as f32 * effective;
    let unclipped_display_pixels = PixelSize::new(
        display_w.round().max(1.0).min(u32::MAX as f32) as u32,
        display_h.round().max(1.0).min(u32::MAX as f32) as u32,
    );
    let max_visible_w = display_w.min(canvas_pixels.width as f32);
    let max_visible_h = display_h.min(canvas_pixels.height as f32);
    let visible_w = max_visible_w.max(policy.min_visible_pixels.width as f32);
    let visible_h = max_visible_h.max(policy.min_visible_pixels.height as f32);
    let visible_pixels = PixelSize::new(
        visible_w.round().max(1.0).min(u32::MAX as f32) as u32,
        visible_h.round().max(1.0).min(u32::MAX as f32) as u32,
    );

    let (src_w, src_h) = match policy.overflow {
        ImageOverflowPolicy::FitWithinArea
        | ImageOverflowPolicy::OverflowAndClipDestination
        | ImageOverflowPolicy::PreventZoomBeyondArea => (image.width.max(1), image.height.max(1)),
        ImageOverflowPolicy::CropSourceToArea => (
            ((visible_w / effective).round() as u32).clamp(1, image.width.max(1)),
            ((visible_h / effective).round() as u32).clamp(1, image.height.max(1)),
        ),
    };
    let max_x = image.width.saturating_sub(src_w);
    let max_y = image.height.saturating_sub(src_h);

    let (center_x, center_y) = match policy.anchor {
        ImageAnchorPolicy::Center | ImageAnchorPolicy::PreserveCursorAnchor => {
            (transform.center_x, transform.center_y)
        }
        ImageAnchorPolicy::PreserveImagePoint { x, y } => (
            x / image.width.max(1) as f32,
            y / image.height.max(1) as f32,
        ),
    };
    let center_x = center_x.clamp(0.0, 1.0);
    let center_y = center_y.clamp(0.0, 1.0);
    let center_image_x = center_x * image.width as f32;
    let center_image_y = center_y * image.height as f32;
    let anchor = PlacementAnchor {
        image_x: center_image_x,
        image_y: center_image_y,
        normalized_x: center_x,
        normalized_y: center_y,
    };
    let src_x = (center_image_x - src_w as f32 / 2.0)
        .round()
        .max(0.0)
        .min(max_x as f32) as u32;
    let src_y = (center_image_y - src_h as f32 / 2.0)
        .round()
        .max(0.0)
        .min(max_y as f32) as u32;

    let cell_w = cell_pixel.width.max(1) as f32;
    let cell_h = cell_pixel.height.max(1) as f32;
    let target_cols =
        round_cells(visible_w / cell_w, policy.cell_rounding).clamp(1, canvas.cells.cols.max(1));
    let target_rows =
        round_cells(visible_h / cell_h, policy.cell_rounding).clamp(1, canvas.cells.rows.max(1));

    let origin_col = canvas.cells.cols.saturating_sub(target_cols) / 2;
    let origin_row = canvas.cells.rows.saturating_sub(target_rows) / 2;
    let clipped_sides = clipped_sides(
        image,
        canvas_pixels,
        src_x,
        src_y,
        src_w,
        src_h,
        display_w,
        display_h,
        visible_w,
        visible_h,
        policy.overflow,
    );

    Placement {
        source: PixelRect {
            x: src_x,
            y: src_y,
            width: src_w,
            height: src_h,
        },
        size: CellRect {
            cols: target_cols,
            rows: target_rows,
        },
        origin: CellOffset {
            col: origin_col,
            row: origin_row,
        },
        effective_scale: effective,
        fit_scale: fit,
        visible_pixels,
        unclipped_display_pixels,
        clipped_sides,
        anchor,
    }
}

#[allow(clippy::too_many_arguments)]
fn clipped_sides(
    image: PixelSize,
    canvas_pixels: PixelSize,
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
    display_w: f32,
    display_h: f32,
    visible_w: f32,
    visible_h: f32,
    overflow: ImageOverflowPolicy,
) -> ClippedSides {
    match overflow {
        ImageOverflowPolicy::CropSourceToArea => ClippedSides {
            left: src_x > 0,
            right: src_x.saturating_add(src_w) < image.width,
            top: src_y > 0,
            bottom: src_y.saturating_add(src_h) < image.height,
        },
        ImageOverflowPolicy::OverflowAndClipDestination => {
            let horizontal = display_w > visible_w || display_w > canvas_pixels.width as f32;
            let vertical = display_h > visible_h || display_h > canvas_pixels.height as f32;
            ClippedSides {
                left: horizontal,
                right: horizontal,
                top: vertical,
                bottom: vertical,
            }
        }
        ImageOverflowPolicy::FitWithinArea | ImageOverflowPolicy::PreventZoomBeyondArea => {
            ClippedSides::NONE
        }
    }
}

fn scale_basis(
    basis: ImageScaleBasis,
    image: PixelSize,
    canvas_pixels: PixelSize,
    fit: f32,
) -> f32 {
    match basis {
        ImageScaleBasis::NativePixels => 1.0,
        ImageScaleBasis::FitToArea => fit,
        ImageScaleBasis::FillArea => {
            if image.width == 0 || image.height == 0 {
                1.0
            } else {
                let fit_w = canvas_pixels.width.max(1) as f32 / image.width as f32;
                let fit_h = canvas_pixels.height.max(1) as f32 / image.height as f32;
                fit_w.max(fit_h).max(f32::EPSILON)
            }
        }
        ImageScaleBasis::ExplicitScale(scale) => scale,
    }
}

fn apply_zoom_limit(scale: f32, limit: ImageZoomLimitPolicy, fit: f32) -> f32 {
    if scale.is_nan() {
        return 1.0;
    }
    match limit {
        ImageZoomLimitPolicy::ClampScale { min, max } => scale.clamp(min, max),
        ImageZoomLimitPolicy::ClampAtFitBounds => {
            scale.min(1.0 / fit.max(f32::EPSILON)).max(MIN_SCALE)
        }
        ImageZoomLimitPolicy::AllowUnboundedWithinProtocol => scale.max(f32::EPSILON),
    }
}

fn round_cells(value: f32, policy: CellRoundingPolicy) -> u16 {
    let rounded = match policy {
        CellRoundingPolicy::Nearest => value.round(),
        CellRoundingPolicy::Floor => value.floor(),
        CellRoundingPolicy::Ceil => value.ceil(),
    };
    rounded.max(1.0).min(u16::MAX as f32) as u16
}

fn validate_positive_finite(path: &'static str, value: f32) -> Result<(), ConfigError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ConfigError::new(
            path,
            "value must be finite and greater than zero",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImagePoint {
    pub x: f32,
    pub y: f32,
    pub inside: bool,
}

pub fn fit_scale(image: PixelSize, canvas_pixels: PixelSize) -> f32 {
    if image.width == 0 || image.height == 0 {
        return 1.0;
    }
    let fit_w = canvas_pixels.width.max(1) as f32 / image.width as f32;
    let fit_h = canvas_pixels.height.max(1) as f32 / image.height as f32;
    fit_w.min(fit_h).max(f32::EPSILON)
}

pub fn clamp_scale(scale: f32) -> f32 {
    if scale.is_nan() {
        return 1.0;
    }
    scale.clamp(MIN_SCALE, MAX_SCALE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(cols: u16, rows: u16) -> CanvasMetrics {
        CanvasMetrics::new(CellSize::new(cols, rows), CellPixel::new(8, 16))
    }

    #[test]
    fn tail_viewport_tracks_newest_items_by_default() {
        let viewport = TailViewport::new(10, 4, 0);

        assert_eq!(viewport.visible_range(), 6..10);
        assert_eq!(viewport.scroll_back, 0);
        assert!(viewport.can_scroll_up);
        assert!(!viewport.can_scroll_down);
    }

    #[test]
    fn tail_viewport_clamps_scrollback_and_zero_height() {
        let viewport = TailViewport::new(3, 0, 99);

        assert_eq!(viewport.height, 1);
        assert_eq!(viewport.scroll_back, 2);
        assert_eq!(viewport.visible_range(), 0..1);
        assert_eq!(viewport.scroll_to_top(), 2);
        assert_eq!(TailViewport::scroll_to_bottom(), 0);
    }

    #[test]
    fn tail_viewport_scroll_math_is_saturating() {
        let viewport = TailViewport::new(10, 4, 2);

        assert_eq!(viewport.scroll_by(3), 5);
        assert_eq!(viewport.scroll_by(99), 6);
        assert_eq!(viewport.scroll_by(-1), 1);
        assert_eq!(viewport.scroll_by(-99), 0);
    }

    #[test]
    fn fit_scale_centers_image_smaller_than_canvas() {
        let image = PixelSize::new(400, 200);
        let placement = ViewTransform::fit().place(image, canvas(200, 50));
        assert_eq!(
            placement.source,
            PixelRect {
                x: 0,
                y: 0,
                width: 400,
                height: 200
            }
        );
        assert!(placement.size.cols <= 200 && placement.size.rows <= 50);
        assert!(placement.origin.col + placement.size.cols <= 200);
    }

    #[test]
    fn fit_scale_at_one_lets_zoom_out_below() {
        let image = PixelSize::new(400, 200);
        let canvas = canvas(200, 50);
        let half = ViewTransform::fit().with_scale(0.5).place(image, canvas);
        let full = ViewTransform::fit().place(image, canvas);
        assert!(half.size.cols < full.size.cols);
        assert!(half.size.rows < full.size.rows);
        assert_eq!(half.source.width, image.width);
        assert_eq!(half.source.height, image.height);
    }

    #[test]
    fn zoom_above_one_crops_source() {
        let image = PixelSize::new(1000, 800);
        let canvas = canvas(100, 50);
        let zoomed = ViewTransform::fit().with_scale(2.0).place(image, canvas);
        assert!(zoomed.source.width < image.width);
        assert!(zoomed.source.height < image.height);
    }

    #[test]
    fn placement_engine_uses_explicit_policy() {
        let image = PixelSize::new(400, 200);
        let canvas = canvas(50, 20);
        let policy = PlacementPolicy {
            scale_basis: ImageScaleBasis::NativePixels,
            zoom_limit: ImageZoomLimitPolicy::ClampScale { min: 1.0, max: 4.0 },
            overflow: ImageOverflowPolicy::OverflowAndClipDestination,
            anchor: ImageAnchorPolicy::Center,
            min_visible_pixels: PixelSize::new(1, 1),
            cell_rounding: CellRoundingPolicy::Floor,
        };
        let placement = PlacementEngine::new(policy).unwrap().place(
            image,
            canvas,
            ViewTransform::fit().with_scale(2.0),
        );

        assert_eq!(placement.source.width, image.width);
        assert_eq!(placement.source.height, image.height);
        assert_eq!(placement.effective_scale, 2.0);
        assert_eq!(placement.size.cols, canvas.cells.cols);
        assert_eq!(placement.size.rows, canvas.cells.rows);
        assert_eq!(placement.visible_pixels, canvas.pixels());
        assert_eq!(placement.unclipped_display_pixels, PixelSize::new(800, 400));
        assert_eq!(
            placement.clipped_sides,
            ClippedSides {
                left: true,
                right: true,
                top: true,
                bottom: true,
            }
        );
        assert!(placement.clipped_sides.any());
        assert_eq!(placement.anchor.normalized_x, 0.5);
        assert_eq!(placement.anchor.normalized_y, 0.5);
    }

    #[test]
    fn crop_source_reports_clipped_sides() {
        let image = PixelSize::new(1000, 800);
        let placement = ViewTransform::fit()
            .with_scale(2.0)
            .place(image, canvas(100, 50));

        assert!(placement.clipped_sides.any());
        assert!(placement.clipped_sides.left || placement.clipped_sides.right);
        assert!(placement.clipped_sides.top || placement.clipped_sides.bottom);
        assert!(placement.visible_pixels.width <= placement.unclipped_display_pixels.width);
        assert_eq!(placement.anchor.normalized_x, 0.5);
        assert_eq!(placement.anchor.normalized_y, 0.5);
    }

    #[test]
    fn fit_within_area_reports_no_clipping() {
        let image = PixelSize::new(400, 200);
        let policy = PlacementPolicy {
            overflow: ImageOverflowPolicy::FitWithinArea,
            ..PlacementPolicy::crop_fit_centered()
        };
        let placement = PlacementEngine::new(policy).unwrap().place(
            image,
            canvas(100, 30),
            ViewTransform::fit().with_scale(4.0),
        );

        assert_eq!(placement.source.width, image.width);
        assert_eq!(placement.source.height, image.height);
        assert_eq!(placement.clipped_sides, ClippedSides::NONE);
        assert!(!placement.clipped_sides.any());
    }

    #[test]
    fn placement_policy_validation_reports_contract_paths() {
        let mut policy = PlacementPolicy {
            min_visible_pixels: PixelSize::new(0, 1),
            ..PlacementPolicy::crop_fit_centered()
        };
        let err = PlacementEngine::new(policy.clone()).unwrap_err();
        assert_eq!(err.path, "layout.placement_policy.min_visible_pixels");
        assert!(err.reason.contains("non-zero"));

        policy.min_visible_pixels = PixelSize::new(1, 1);
        policy.scale_basis = ImageScaleBasis::ExplicitScale(f32::NAN);
        let err = PlacementEngine::new(policy.clone()).unwrap_err();
        assert_eq!(err.path, "layout.placement_policy.scale_basis");
        assert!(err.reason.contains("finite and greater than zero"));

        policy.scale_basis = ImageScaleBasis::FitToArea;
        policy.zoom_limit = ImageZoomLimitPolicy::ClampScale { min: 2.0, max: 1.0 };
        let err = PlacementEngine::new(policy.clone()).unwrap_err();
        assert_eq!(err.path, "layout.placement_policy.zoom_limit");
        assert!(err.reason.contains("less than or equal"));

        policy.zoom_limit = ImageZoomLimitPolicy::ClampScale {
            min: 1.0,
            max: f32::NAN,
        };
        let err = PlacementEngine::new(policy.clone()).unwrap_err();
        assert_eq!(err.path, "layout.placement_policy.zoom_limit.max");
        assert!(err.reason.contains("finite and greater than zero"));

        policy.zoom_limit = ImageZoomLimitPolicy::ClampScale { min: 1.0, max: 2.0 };
        policy.anchor = ImageAnchorPolicy::PreserveImagePoint {
            x: f32::NAN,
            y: 0.5,
        };
        let err = PlacementEngine::new(policy).unwrap_err();
        assert_eq!(err.path, "layout.placement_policy.anchor");
        assert!(err.reason.contains("finite"));
    }

    #[test]
    fn prevent_zoom_beyond_area_keeps_full_source_visible() {
        let image = PixelSize::new(1000, 800);
        let canvas = canvas(100, 50);
        let policy = PlacementPolicy {
            overflow: ImageOverflowPolicy::PreventZoomBeyondArea,
            ..PlacementPolicy::crop_fit_centered()
        };
        let placement = PlacementEngine::new(policy).unwrap().place(
            image,
            canvas,
            ViewTransform::fit().with_scale(4.0),
        );

        assert_eq!(placement.source.width, image.width);
        assert_eq!(placement.source.height, image.height);
        assert!(placement.size.cols <= canvas.cells.cols);
        assert!(placement.size.rows <= canvas.cells.rows);
    }

    #[test]
    fn zoom_clamps_to_bounds() {
        let t = ViewTransform::fit().with_scale(100.0);
        assert_eq!(t.scale, MAX_SCALE);
        let t = ViewTransform::fit().with_scale(0.001);
        assert_eq!(t.scale, MIN_SCALE);
    }

    #[test]
    fn panning_below_fit_does_not_move_source_origin() {
        let image = PixelSize::new(400, 200);
        let canvas = canvas(200, 50);
        let panned = ViewTransform::fit()
            .with_scale(0.5)
            .panned(0.5, 0.5, image, canvas);
        let placement = panned.place(image, canvas);
        assert_eq!(placement.source.x, 0);
        assert_eq!(placement.source.y, 0);
    }

    #[test]
    fn panning_when_zoomed_in_moves_source() {
        let image = PixelSize::new(1000, 1000);
        let canvas = canvas(100, 50);
        let panned = ViewTransform::fit()
            .with_scale(2.0)
            .panned(0.25, 0.25, image, canvas);
        let placement = panned.place(image, canvas);
        assert!(placement.source.x > 0);
        assert!(placement.source.y > 0);
    }

    #[test]
    fn canvas_to_image_outside_target_marks_inside_false() {
        let image = PixelSize::new(400, 200);
        let canvas = canvas(200, 50);
        let placement = ViewTransform::fit().with_scale(0.5).place(image, canvas);
        assert!(placement.origin.col > 0 || placement.origin.row > 0);
        let p = ViewTransform::fit()
            .with_scale(0.5)
            .canvas_to_image(0.0, 0.0, image, canvas);
        assert!(!p.inside);
    }

    #[test]
    fn canvas_to_image_inside_target_recovers_image_coordinate() {
        let image = PixelSize::new(400, 200);
        let canvas = canvas(200, 50);
        let placement = ViewTransform::fit().place(image, canvas);
        let mid_col = f32::from(placement.origin.col) + f32::from(placement.size.cols) / 2.0;
        let mid_row = f32::from(placement.origin.row) + f32::from(placement.size.rows) / 2.0;
        let canvas_x = mid_col / f32::from(canvas.cells.cols);
        let canvas_y = mid_row / f32::from(canvas.cells.rows);
        let p = ViewTransform::fit().canvas_to_image(canvas_x, canvas_y, image, canvas);
        assert!(p.inside);
        assert!((p.x - 200.0).abs() < 8.0);
        assert!((p.y - 100.0).abs() < 16.0);
    }
}
