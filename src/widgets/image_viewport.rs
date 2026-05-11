//! Pixel-space image viewport model.
//!
//! [`ImageViewport`] owns the small model used by terminal image widgets:
//! source image pixels, one uniform scale, a scaled-pixel offset, and a
//! widget-sized crop aperture. Terminal cells are converted to pixels before
//! export; terminal cell aspect ratio is not baked into exported pixels.

use crate::image::PlaceOptions;
use crate::layout::{CanvasMetrics, CellOffset, ImagePoint, PixelRect, PixelSize};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportImage {
    size: PixelSize,
    rgba: Arc<[u8]>,
}

impl ViewportImage {
    pub fn new(size: PixelSize, rgba: impl Into<Arc<[u8]>>) -> Result<Self, ImageViewportError> {
        if size.width == 0 || size.height == 0 {
            return Err(ImageViewportError::InvalidImageSize(size));
        }
        let rgba = rgba.into();
        let expected = rgba_len(size)?;
        if rgba.len() != expected {
            return Err(ImageViewportError::InvalidRgbaLength {
                size,
                expected,
                actual: rgba.len(),
            });
        }
        Ok(Self { size, rgba })
    }

    pub fn size(&self) -> PixelSize {
        self.size
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    size: PixelSize,
    rgba: Arc<[u8]>,
}

impl RgbaImage {
    pub fn new(size: PixelSize, rgba: impl Into<Arc<[u8]>>) -> Result<Self, ImageViewportError> {
        let rgba = rgba.into();
        let expected = rgba_len(size)?;
        if rgba.len() != expected {
            return Err(ImageViewportError::InvalidRgbaLength {
                size,
                expected,
                actual: rgba.len(),
            });
        }
        Ok(Self { size, rgba })
    }

    pub fn size(&self) -> PixelSize {
        self.size
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageViewport {
    image: ViewportImage,
    scale: ImageScale,
    offset: ScaledPixelOffset,
    step_x: PixelDistance,
    step_y: PixelDistance,
    zoom_factor: ZoomFactor,
}

impl ImageViewport {
    pub fn new(image: ViewportImage) -> Self {
        Self {
            image,
            scale: ImageScale::ONE,
            offset: ScaledPixelOffset::ZERO,
            step_x: PixelDistance::new(1),
            step_y: PixelDistance::new(1),
            zoom_factor: ZoomFactor::DEFAULT,
        }
    }

    pub fn image(&self) -> &ViewportImage {
        &self.image
    }

    pub fn scale(&self) -> ImageScale {
        self.scale
    }

    pub fn zoom_factor(&self) -> ZoomFactor {
        self.zoom_factor
    }

    pub fn offset(&self) -> ScaledPixelOffset {
        self.offset
    }

    pub fn step(&self, axis: ViewportAxis) -> PixelDistance {
        match axis {
            ViewportAxis::X => self.step_x,
            ViewportAxis::Y => self.step_y,
        }
    }

    /// Set absolute scale. Offset remains in scaled-image pixels and is not
    /// changed; use [`Self::apply_zoom`] for center-preserving zoom changes.
    pub fn set_scale(&mut self, scale: ImageScale) {
        self.scale = scale;
    }

    /// Set absolute scale by specifying the new scaled width or height.
    pub fn set_scale_from_dimension(
        &mut self,
        basis: ScaleBasis,
        new_pixels: PixelExtent,
    ) -> Result<(), ImageViewportError> {
        let source_pixels = match basis {
            ScaleBasis::Width => self.image.size.width,
            ScaleBasis::Height => self.image.size.height,
        };
        self.scale = ImageScale::new(new_pixels.get() as f64 / source_pixels as f64)?;
        Ok(())
    }

    /// Set the relative factor used by [`Self::apply_zoom`].
    pub fn set_zoom(&mut self, factor: ZoomFactor) {
        self.zoom_factor = factor;
    }

    pub fn set_offset(&mut self, offset: ScaledPixelOffset) {
        self.offset = offset;
    }

    /// Set offset in source-image pixels, converted through the current scale.
    pub fn set_unscaled_offset(&mut self, offset: UnscaledPixelOffset) {
        self.offset = ScaledPixelOffset {
            x: round_i64(offset.x as f64 * self.scale.get()),
            y: round_i64(offset.y as f64 * self.scale.get()),
        };
    }

    pub fn set_step(&mut self, axis: ViewportAxis, amount: PixelDistance) {
        match axis {
            ViewportAxis::X => self.step_x = amount,
            ViewportAxis::Y => self.step_y = amount,
        }
    }

    pub fn apply_step(&mut self, axis: ViewportAxis, direction: StepDirection) {
        let amount = self.step(axis).get() as i64;
        let delta = match direction {
            StepDirection::Negative => -amount,
            StepDirection::Positive => amount,
        };
        match axis {
            ViewportAxis::X => self.offset.x = self.offset.x.saturating_add(delta),
            ViewportAxis::Y => self.offset.y = self.offset.y.saturating_add(delta),
        }
    }

    /// Apply the configured zoom factor, anchored to the center of the
    /// widget aperture.
    pub fn apply_zoom(
        &mut self,
        direction: ZoomDirection,
        widget_pixels: PixelSize,
    ) -> Result<(), ImageViewportError> {
        let old_scale = self.scale.get();
        let zoom_factor = self.zoom_factor.get();
        let new_scale = match direction {
            ZoomDirection::In => old_scale * zoom_factor,
            ZoomDirection::Out => old_scale / zoom_factor,
        };
        let new_scale = ImageScale::new(new_scale)?;

        let center_x = widget_pixels.width as f64 / 2.0;
        let center_y = widget_pixels.height as f64 / 2.0;
        let source_center_x = (self.offset.x as f64 + center_x) / old_scale;
        let source_center_y = (self.offset.y as f64 + center_y) / old_scale;

        self.scale = new_scale;
        self.offset = ScaledPixelOffset {
            x: round_i64(source_center_x * new_scale.get() - center_x),
            y: round_i64(source_center_y * new_scale.get() - center_y),
        };
        Ok(())
    }

    pub fn theoretical_size(&self) -> Result<PixelSize, ImageViewportError> {
        scaled_size(self.image.size, self.scale)
    }

    pub fn export_rgba(&self, widget_pixels: PixelSize) -> Result<RgbaImage, ImageViewportError> {
        let len = rgba_len(widget_pixels)?;
        let theoretical = self.theoretical_size()?;
        let mut out = vec![0; len];
        let scale = self.scale.get();

        for y in 0..widget_pixels.height {
            let scaled_y = self.offset.y + i64::from(y);
            if scaled_y < 0 || scaled_y >= i64::from(theoretical.height) {
                continue;
            }
            let src_y = scaled_to_source(scaled_y, self.image.size.height, scale);
            for x in 0..widget_pixels.width {
                let scaled_x = self.offset.x + i64::from(x);
                if scaled_x < 0 || scaled_x >= i64::from(theoretical.width) {
                    continue;
                }
                let src_x = scaled_to_source(scaled_x, self.image.size.width, scale);
                copy_rgba(
                    self.image.rgba(),
                    self.image.size.width,
                    &mut out,
                    widget_pixels.width,
                    src_x,
                    src_y,
                    x,
                    y,
                );
            }
        }

        RgbaImage::new(widget_pixels, out)
    }

    pub fn export_rgba_for_canvas(
        &self,
        canvas: CanvasMetrics,
    ) -> Result<RgbaImage, ImageViewportError> {
        self.export_rgba(canvas.pixels())
    }

    pub fn normalized_to_image(
        &self,
        x: f32,
        y: f32,
        widget_pixels: PixelSize,
    ) -> Result<ImagePoint, ImageViewportError> {
        let widget_x = x.clamp(0.0, 1.0) as f64 * widget_pixels.width as f64;
        let widget_y = y.clamp(0.0, 1.0) as f64 * widget_pixels.height as f64;
        self.widget_pixel_to_image(widget_x, widget_y)
    }

    pub fn widget_pixel_to_image(
        &self,
        widget_x: f64,
        widget_y: f64,
    ) -> Result<ImagePoint, ImageViewportError> {
        let theoretical = self.theoretical_size()?;
        let scaled_x = self.offset.x as f64 + widget_x;
        let scaled_y = self.offset.y as f64 + widget_y;
        let inside = scaled_x >= 0.0
            && scaled_y >= 0.0
            && scaled_x < theoretical.width as f64
            && scaled_y < theoretical.height as f64;
        Ok(ImagePoint {
            x: (scaled_x / self.scale.get()).clamp(0.0, self.image.size.width as f64) as f32,
            y: (scaled_y / self.scale.get()).clamp(0.0, self.image.size.height as f64) as f32,
            inside,
        })
    }

    pub fn placement(
        &self,
        canvas: CanvasMetrics,
    ) -> Result<Option<ImageViewportPlacement>, ImageViewportError> {
        let widget_pixels = canvas.pixels();
        if widget_pixels.width == 0 || widget_pixels.height == 0 {
            return Ok(None);
        }

        let theoretical = self.theoretical_size()?;
        let crop = viewport_crop(self.offset, theoretical, widget_pixels);
        if crop.visible.width == 0 || crop.visible.height == 0 {
            return Ok(None);
        }

        let cell_pixel = canvas.cell_pixel.or_fallback();
        let origin = CellOffset {
            col: round_to_cell_offset(crop.destination_offset_x, cell_pixel.width)
                .min(canvas.cells.cols),
            row: round_to_cell_offset(crop.destination_offset_y, cell_pixel.height)
                .min(canvas.cells.rows),
        };
        let available_cols = canvas
            .cells
            .cols
            .saturating_sub(origin.col.min(canvas.cells.cols));
        let available_rows = canvas
            .cells
            .rows
            .saturating_sub(origin.row.min(canvas.cells.rows));
        let cell_cols = round_to_cells(crop.visible.width, cell_pixel.width).min(available_cols);
        let cell_rows = round_to_cells(crop.visible.height, cell_pixel.height).min(available_rows);
        if cell_cols == 0 || cell_rows == 0 {
            return Ok(None);
        }

        Ok(Some(ImageViewportPlacement {
            source: crop.source(self.image.size, self.scale),
            origin,
            cell_cols,
            cell_rows,
            visible_pixels: crop.visible,
            theoretical_pixels: theoretical,
        }))
    }

    fn source_point_at_widget_pixel(&self, widget_pixel: (f64, f64)) -> (f64, f64) {
        (
            (self.offset.x as f64 + widget_pixel.0) / self.scale.get(),
            (self.offset.y as f64 + widget_pixel.1) / self.scale.get(),
        )
    }

    fn set_source_point_at_widget_pixel(
        &mut self,
        source_point: (f64, f64),
        widget_pixel: (f64, f64),
    ) {
        self.offset = ScaledPixelOffset {
            x: round_i64(source_point.0 * self.scale.get() - widget_pixel.0),
            y: round_i64(source_point.1 * self.scale.get() - widget_pixel.1),
        };
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageViewportWidget {
    viewport: ImageViewport,
    canvas: CanvasMetrics,
    resize_policy: ResizePolicy,
}

impl ImageViewportWidget {
    pub fn new(viewport: ImageViewport, canvas: CanvasMetrics) -> Self {
        Self {
            viewport,
            canvas,
            resize_policy: ResizePolicy::PreserveTopLeft,
        }
    }

    pub fn from_image(image: ViewportImage, canvas: CanvasMetrics) -> Self {
        Self::new(ImageViewport::new(image), canvas)
    }

    pub fn from_image_with_options(
        image: ViewportImage,
        canvas: CanvasMetrics,
        options: ImageViewportOptions,
    ) -> Result<Self, ImageViewportError> {
        let mut widget = Self::from_image(image, canvas);
        widget.set_resize_policy(options.resize_policy);
        widget.apply_initial_scale(options.initial_scale)?;
        Ok(widget)
    }

    pub fn viewport(&self) -> &ImageViewport {
        &self.viewport
    }

    pub fn viewport_mut(&mut self) -> &mut ImageViewport {
        &mut self.viewport
    }

    pub fn into_viewport(self) -> ImageViewport {
        self.viewport
    }

    pub fn canvas(&self) -> CanvasMetrics {
        self.canvas
    }

    pub fn widget_pixels(&self) -> PixelSize {
        self.canvas.pixels()
    }

    pub fn resize_policy(&self) -> ResizePolicy {
        self.resize_policy
    }

    pub fn set_resize_policy(&mut self, policy: ResizePolicy) {
        self.resize_policy = policy;
    }

    /// Update terminal geometry and immediately apply the configured resize
    /// policy if either cell count or cell-pixel dimensions changed.
    pub fn update_canvas(&mut self, canvas: CanvasMetrics) -> CanvasUpdate {
        let update = CanvasUpdate::new(self.canvas, canvas);
        if !update.changed {
            return update;
        }

        self.apply_resize_policy(update.old_pixels, update.new_pixels);
        self.canvas = canvas;
        update
    }

    pub fn set_scale(&mut self, scale: ImageScale) {
        self.viewport.set_scale(scale);
    }

    pub fn apply_initial_scale(
        &mut self,
        initial_scale: ImageViewportInitialScale,
    ) -> Result<(), ImageViewportError> {
        match initial_scale {
            ImageViewportInitialScale::Native => {
                self.viewport.set_scale(ImageScale::ONE);
                self.viewport.set_offset(ScaledPixelOffset::ZERO);
            }
            ImageViewportInitialScale::FitToBox => {
                self.viewport.set_scale(ImageScale::new(fit_scale_to_box(
                    self.viewport.image().size(),
                    self.widget_pixels(),
                ))?);
                let theoretical = self.viewport.theoretical_size()?;
                self.viewport
                    .set_offset(centered_offset(theoretical, self.widget_pixels()));
            }
        }
        Ok(())
    }

    pub fn set_scale_from_dimension(
        &mut self,
        basis: ScaleBasis,
        new_pixels: PixelExtent,
    ) -> Result<(), ImageViewportError> {
        self.viewport.set_scale_from_dimension(basis, new_pixels)
    }

    pub fn set_zoom(&mut self, factor: ZoomFactor) {
        self.viewport.set_zoom(factor);
    }

    pub fn set_offset(&mut self, offset: ScaledPixelOffset) {
        self.viewport.set_offset(offset);
    }

    pub fn set_unscaled_offset(&mut self, offset: UnscaledPixelOffset) {
        self.viewport.set_unscaled_offset(offset);
    }

    pub fn set_step(&mut self, axis: ViewportAxis, amount: PixelDistance) {
        self.viewport.set_step(axis, amount);
    }

    pub fn apply_step(&mut self, axis: ViewportAxis, direction: StepDirection) {
        self.viewport.apply_step(axis, direction);
    }

    pub fn apply_zoom(&mut self, direction: ZoomDirection) -> Result<(), ImageViewportError> {
        self.viewport.apply_zoom(direction, self.widget_pixels())
    }

    pub fn export_rgba(&self) -> Result<RgbaImage, ImageViewportError> {
        self.viewport.export_rgba_for_canvas(self.canvas)
    }

    pub fn normalized_to_image(&self, x: f32, y: f32) -> Result<ImagePoint, ImageViewportError> {
        self.viewport
            .normalized_to_image(x, y, self.widget_pixels())
    }

    pub fn placement(&self) -> Result<Option<ImageViewportPlacement>, ImageViewportError> {
        self.viewport.placement(self.canvas)
    }

    fn apply_resize_policy(&mut self, old_pixels: PixelSize, new_pixels: PixelSize) {
        match self.resize_policy {
            ResizePolicy::PreserveTopLeft => {}
            ResizePolicy::PreserveCenter => {
                let source_center = self
                    .viewport
                    .source_point_at_widget_pixel(widget_center(old_pixels));
                self.viewport
                    .set_source_point_at_widget_pixel(source_center, widget_center(new_pixels));
            }
            ResizePolicy::PreserveSourcePoint { x, y } => {
                self.viewport
                    .set_source_point_at_widget_pixel((x, y), widget_center(new_pixels));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasUpdate {
    pub old: CanvasMetrics,
    pub new: CanvasMetrics,
    pub old_pixels: PixelSize,
    pub new_pixels: PixelSize,
    pub changed: bool,
    pub cell_size_changed: bool,
    pub cell_pixel_changed: bool,
}

impl CanvasUpdate {
    pub fn new(old: CanvasMetrics, new: CanvasMetrics) -> Self {
        Self {
            old,
            new,
            old_pixels: old.pixels(),
            new_pixels: new.pixels(),
            changed: old != new,
            cell_size_changed: old.cells != new.cells,
            cell_pixel_changed: old.cell_pixel != new.cell_pixel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ResizePolicy {
    PreserveTopLeft,
    PreserveCenter,
    PreserveSourcePoint { x: f64, y: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImageViewportOptions {
    pub initial_scale: ImageViewportInitialScale,
    pub resize_policy: ResizePolicy,
}

impl Default for ImageViewportOptions {
    fn default() -> Self {
        Self {
            initial_scale: ImageViewportInitialScale::Native,
            resize_policy: ResizePolicy::PreserveTopLeft,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageViewportInitialScale {
    Native,
    FitToBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageViewportPlacement {
    pub source: PixelRect,
    pub origin: CellOffset,
    pub cell_cols: u16,
    pub cell_rows: u16,
    pub visible_pixels: PixelSize,
    pub theoretical_pixels: PixelSize,
}

impl ImageViewportPlacement {
    pub fn place_options(self, image_id: u32, placement_id: u32) -> PlaceOptions {
        PlaceOptions {
            image_id,
            placement_id,
            source: self.source,
            cell_cols: self.cell_cols,
            cell_rows: self.cell_rows,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ViewportCrop {
    left: u32,
    top: u32,
    visible: PixelSize,
    destination_offset_x: u32,
    destination_offset_y: u32,
}

impl ViewportCrop {
    fn source(self, image: PixelSize, scale: ImageScale) -> PixelRect {
        let scale = scale.get();
        let x = (self.left as f64 / scale).round().max(0.0) as u32;
        let y = (self.top as f64 / scale).round().max(0.0) as u32;
        let width = (self.visible.width as f64 / scale)
            .round()
            .max(1.0)
            .min(image.width as f64) as u32;
        let height = (self.visible.height as f64 / scale)
            .round()
            .max(1.0)
            .min(image.height as f64) as u32;
        let x = x.min(image.width.saturating_sub(1));
        let y = y.min(image.height.saturating_sub(1));
        PixelRect {
            x,
            y,
            width: width.min(image.width.saturating_sub(x)),
            height: height.min(image.height.saturating_sub(y)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewportAxis {
    X,
    Y,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepDirection {
    Negative,
    Positive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZoomDirection {
    In,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleBasis {
    Width,
    Height,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImageScale(f64);

impl ImageScale {
    pub const ONE: Self = Self(1.0);

    pub fn new(value: f64) -> Result<Self, ImageViewportError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(ImageViewportError::InvalidScale(value));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ZoomFactor(f64);

impl ZoomFactor {
    pub const DEFAULT: Self = Self(1.25);

    pub fn new(value: f64) -> Result<Self, ImageViewportError> {
        if !value.is_finite() || value <= 1.0 {
            return Err(ImageViewportError::InvalidZoomFactor(value));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelDistance(u32);

impl PixelDistance {
    pub const fn new(pixels: u32) -> Self {
        Self(pixels)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelExtent(u32);

impl PixelExtent {
    pub fn new(pixels: u32) -> Result<Self, ImageViewportError> {
        if pixels == 0 {
            return Err(ImageViewportError::InvalidPixelExtent(pixels));
        }
        Ok(Self(pixels))
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaledPixelOffset {
    pub x: i64,
    pub y: i64,
}

impl ScaledPixelOffset {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub const fn new(x: i64, y: i64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnscaledPixelOffset {
    pub x: i64,
    pub y: i64,
}

impl UnscaledPixelOffset {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub const fn new(x: i64, y: i64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImageViewportError {
    InvalidImageSize(PixelSize),
    InvalidRgbaLength {
        size: PixelSize,
        expected: usize,
        actual: usize,
    },
    InvalidScale(f64),
    InvalidZoomFactor(f64),
    InvalidPixelExtent(u32),
    PixelCountOverflow(PixelSize),
    ScaledSizeOverflow {
        size: PixelSize,
        scale: f64,
    },
}

impl fmt::Display for ImageViewportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidImageSize(size) => {
                write!(
                    f,
                    "image size must be non-zero, got {}x{}",
                    size.width, size.height
                )
            }
            Self::InvalidRgbaLength {
                size,
                expected,
                actual,
            } => write!(
                f,
                "rgba length for {}x{} image must be {}, got {}",
                size.width, size.height, expected, actual
            ),
            Self::InvalidScale(scale) => {
                write!(f, "image scale must be positive and finite, got {scale}")
            }
            Self::InvalidZoomFactor(factor) => {
                write!(
                    f,
                    "zoom factor must be finite and greater than 1, got {factor}"
                )
            }
            Self::InvalidPixelExtent(pixels) => {
                write!(f, "pixel extent must be greater than zero, got {pixels}")
            }
            Self::PixelCountOverflow(size) => write!(
                f,
                "pixel buffer size overflow for {}x{} rgba image",
                size.width, size.height
            ),
            Self::ScaledSizeOverflow { size, scale } => write!(
                f,
                "scaled size overflow for {}x{} image at scale {}",
                size.width, size.height, scale
            ),
        }
    }
}

impl Error for ImageViewportError {}

fn scaled_size(size: PixelSize, scale: ImageScale) -> Result<PixelSize, ImageViewportError> {
    Ok(PixelSize::new(
        scaled_extent(size.width, size, scale)?,
        scaled_extent(size.height, size, scale)?,
    ))
}

fn scaled_extent(
    source_extent: u32,
    size: PixelSize,
    scale: ImageScale,
) -> Result<u32, ImageViewportError> {
    let scaled = source_extent as f64 * scale.get();
    if !scaled.is_finite() || scaled > u32::MAX as f64 {
        return Err(ImageViewportError::ScaledSizeOverflow {
            size,
            scale: scale.get(),
        });
    }
    Ok(round_u32(scaled).max(1))
}

fn rgba_len(size: PixelSize) -> Result<usize, ImageViewportError> {
    let bytes = size
        .area()
        .checked_mul(4)
        .ok_or(ImageViewportError::PixelCountOverflow(size))?;
    if bytes > usize::MAX as u64 {
        return Err(ImageViewportError::PixelCountOverflow(size));
    }
    Ok(bytes as usize)
}

fn scaled_to_source(scaled_coordinate: i64, source_extent: u32, scale: f64) -> u32 {
    ((scaled_coordinate as f64 / scale).floor() as i64)
        .clamp(0, i64::from(source_extent.saturating_sub(1))) as u32
}

fn viewport_crop(
    offset: ScaledPixelOffset,
    theoretical: PixelSize,
    widget: PixelSize,
) -> ViewportCrop {
    let left = offset.x.max(0).min(i64::from(theoretical.width)) as u32;
    let top = offset.y.max(0).min(i64::from(theoretical.height)) as u32;
    let right = offset
        .x
        .saturating_add(i64::from(widget.width))
        .max(0)
        .min(i64::from(theoretical.width)) as u32;
    let bottom = offset
        .y
        .saturating_add(i64::from(widget.height))
        .max(0)
        .min(i64::from(theoretical.height)) as u32;

    ViewportCrop {
        left,
        top,
        visible: PixelSize::new(right.saturating_sub(left), bottom.saturating_sub(top)),
        destination_offset_x: offset
            .x
            .saturating_neg()
            .max(0)
            .min(i64::from(widget.width)) as u32,
        destination_offset_y: offset
            .y
            .saturating_neg()
            .max(0)
            .min(i64::from(widget.height)) as u32,
    }
}

fn widget_center(size: PixelSize) -> (f64, f64) {
    (size.width as f64 / 2.0, size.height as f64 / 2.0)
}

fn fit_scale_to_box(image: PixelSize, widget: PixelSize) -> f64 {
    let width = widget.width.max(1) as f64 / image.width.max(1) as f64;
    let height = widget.height.max(1) as f64 / image.height.max(1) as f64;
    width.min(height).max(f64::EPSILON)
}

fn centered_offset(theoretical: PixelSize, widget: PixelSize) -> ScaledPixelOffset {
    ScaledPixelOffset::new(
        (i64::from(theoretical.width) - i64::from(widget.width)) / 2,
        (i64::from(theoretical.height) - i64::from(widget.height)) / 2,
    )
}

fn round_to_cell_offset(pixels: u32, cell_pixels: u16) -> u16 {
    (pixels as f64 / f64::from(cell_pixels.max(1)))
        .round()
        .max(0.0)
        .min(u16::MAX as f64) as u16
}

fn round_to_cells(pixels: u32, cell_pixels: u16) -> u16 {
    if pixels == 0 {
        return 0;
    }
    (pixels as f64 / f64::from(cell_pixels.max(1)))
        .round()
        .max(1.0)
        .min(u16::MAX as f64) as u16
}

#[allow(clippy::too_many_arguments)]
fn copy_rgba(
    src: &[u8],
    src_width: u32,
    dst: &mut [u8],
    dst_width: u32,
    src_x: u32,
    src_y: u32,
    dst_x: u32,
    dst_y: u32,
) {
    let src_i = (((u64::from(src_y) * u64::from(src_width)) + u64::from(src_x)) * 4) as usize;
    let dst_i = (((u64::from(dst_y) * u64::from(dst_width)) + u64::from(dst_x)) * 4) as usize;
    dst[dst_i..dst_i + 4].copy_from_slice(&src[src_i..src_i + 4]);
}

fn round_u32(value: f64) -> u32 {
    (value + 0.5).floor() as u32
}

fn round_i64(value: f64) -> i64 {
    if value >= 0.0 {
        (value + 0.5).floor() as i64
    } else {
        (value - 0.5).ceil() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{CellPixel, CellSize};

    #[test]
    fn export_rgba_uses_scaled_offset_and_widget_pixels() {
        let image = source_image();
        let cases = [
            (PixelSize::new(200, 150), ScaledPixelOffset::new(0, 0), 1.0),
            (PixelSize::new(100, 75), ScaledPixelOffset::new(0, 0), 1.0),
            (PixelSize::new(100, 75), ScaledPixelOffset::new(0, 0), 0.5),
            (
                PixelSize::new(100, 75),
                ScaledPixelOffset::new(460, 300),
                1.0,
            ),
            (
                PixelSize::new(200, 150),
                ScaledPixelOffset::new(920, 600),
                2.0,
            ),
            (PixelSize::new(200, 150), ScaledPixelOffset::new(0, 0), 0.25),
            (
                PixelSize::new(1600, 1200),
                ScaledPixelOffset::new(0, 0),
                2.0,
            ),
            (
                PixelSize::new(1600, 1200),
                ScaledPixelOffset::new(920, 600),
                2.0,
            ),
            (
                PixelSize::new(800, 600),
                ScaledPixelOffset::new(0, 0),
                10.0 / 7.0,
            ),
        ];

        for (widget, offset, scale) in cases {
            let mut viewport = ImageViewport::new(image.clone());
            viewport.set_scale(ImageScale::new(scale).unwrap());
            viewport.set_offset(offset);

            let actual = viewport.export_rgba(widget).unwrap();
            let expected = reference_crop(&image, widget, offset, scale);

            assert_eq!(actual.size(), widget);
            assert_eq!(
                actual.rgba(),
                expected.rgba(),
                "case {widget:?} {offset:?} {scale}"
            );
        }
    }

    #[test]
    fn set_unscaled_offset_converts_through_current_scale() {
        let mut viewport = ImageViewport::new(source_image());
        viewport.set_scale(ImageScale::new(2.0).unwrap());

        viewport.set_unscaled_offset(UnscaledPixelOffset::new(460, 300));

        assert_eq!(viewport.offset(), ScaledPixelOffset::new(920, 600));
    }

    #[test]
    fn apply_zoom_preserves_widget_center() {
        let image = source_image();
        let widget = PixelSize::new(200, 150);
        let mut viewport = ImageViewport::new(image);
        viewport.set_offset(ScaledPixelOffset::new(100, 50));
        viewport.set_zoom(ZoomFactor::new(2.0).unwrap());

        let before = source_at_widget_center(&viewport, widget);
        viewport.apply_zoom(ZoomDirection::In, widget).unwrap();
        let after = source_at_widget_center(&viewport, widget);

        assert!((before.0 - after.0).abs() < f64::EPSILON);
        assert!((before.1 - after.1).abs() < f64::EPSILON);
        assert_eq!(viewport.scale(), ImageScale::new(2.0).unwrap());
    }

    #[test]
    fn apply_step_uses_axis_and_direction_enums() {
        let mut viewport = ImageViewport::new(source_image());
        viewport.set_step(ViewportAxis::X, PixelDistance::new(25));
        viewport.set_step(ViewportAxis::Y, PixelDistance::new(10));

        viewport.apply_step(ViewportAxis::X, StepDirection::Positive);
        viewport.apply_step(ViewportAxis::Y, StepDirection::Negative);

        assert_eq!(viewport.offset(), ScaledPixelOffset::new(25, -10));
    }

    #[test]
    fn set_scale_from_dimension_uses_uniform_scale() {
        let mut viewport = ImageViewport::new(source_image());

        viewport
            .set_scale_from_dimension(ScaleBasis::Width, PixelExtent::new(400).unwrap())
            .unwrap();

        assert_eq!(viewport.scale(), ImageScale::new(0.5).unwrap());
        assert_eq!(
            viewport.theoretical_size().unwrap(),
            PixelSize::new(400, 300)
        );
    }

    #[test]
    fn export_rgba_for_canvas_converts_cells_to_pixels() {
        let viewport = ImageViewport::new(source_image());
        let canvas = CanvasMetrics::new(CellSize::new(10, 5), CellPixel::new(20, 30));

        let exported = viewport.export_rgba_for_canvas(canvas).unwrap();

        assert_eq!(exported.size(), PixelSize::new(200, 150));
    }

    #[test]
    fn placement_reports_source_crop_and_destination_cells() {
        let mut viewport = ImageViewport::new(source_image());
        viewport.set_scale(ImageScale::new(2.0).unwrap());
        viewport.set_offset(ScaledPixelOffset::new(920, 600));

        let placement = viewport
            .placement(CanvasMetrics::new(
                CellSize::new(20, 10),
                CellPixel::new(10, 15),
            ))
            .unwrap()
            .unwrap();

        assert_eq!(placement.theoretical_pixels, PixelSize::new(1600, 1200));
        assert_eq!(placement.visible_pixels, PixelSize::new(200, 150));
        assert_eq!(
            placement.source,
            PixelRect {
                x: 460,
                y: 300,
                width: 100,
                height: 75,
            }
        );
        assert_eq!(placement.origin, CellOffset { col: 0, row: 0 });
        assert_eq!(placement.cell_cols, 20);
        assert_eq!(placement.cell_rows, 10);
    }

    #[test]
    fn placement_centers_when_scaled_image_is_smaller_than_canvas() {
        let mut viewport = ImageViewport::new(source_image());
        viewport.set_scale(ImageScale::new(0.25).unwrap());
        viewport.set_offset(ScaledPixelOffset::new(0, 0));

        let placement = viewport
            .placement(CanvasMetrics::new(
                CellSize::new(40, 20),
                CellPixel::new(10, 15),
            ))
            .unwrap()
            .unwrap();

        assert_eq!(placement.visible_pixels, PixelSize::new(200, 150));
        assert_eq!(placement.origin, CellOffset { col: 0, row: 0 });

        viewport.set_offset(ScaledPixelOffset::new(-100, -75));
        let centered = viewport
            .placement(CanvasMetrics::new(
                CellSize::new(40, 20),
                CellPixel::new(10, 15),
            ))
            .unwrap()
            .unwrap();
        assert_eq!(centered.visible_pixels, PixelSize::new(200, 150));
        assert_eq!(centered.origin, CellOffset { col: 10, row: 5 });
    }

    #[test]
    fn normalized_to_image_marks_points_outside_scaled_image() {
        let mut viewport = ImageViewport::new(source_image());
        viewport.set_scale(ImageScale::new(0.25).unwrap());
        viewport.set_offset(ScaledPixelOffset::new(-100, -75));

        let inside = viewport
            .normalized_to_image(0.5, 0.5, PixelSize::new(400, 300))
            .unwrap();
        let outside = viewport
            .normalized_to_image(0.05, 0.05, PixelSize::new(400, 300))
            .unwrap();

        assert!(inside.inside);
        assert!(!outside.inside);
        assert!((inside.x - 400.0).abs() < 1.0, "x={}", inside.x);
        assert!((inside.y - 300.0).abs() < 1.0, "y={}", inside.y);
    }

    #[test]
    fn widget_update_canvas_tracks_cell_count_and_cell_pixel_changes() {
        let mut widget = ImageViewportWidget::from_image(
            source_image(),
            CanvasMetrics::new(CellSize::new(10, 5), CellPixel::new(20, 30)),
        );

        let update = widget.update_canvas(CanvasMetrics::new(
            CellSize::new(5, 5),
            CellPixel::new(20, 15),
        ));
        let exported = widget.export_rgba().unwrap();

        assert!(update.changed);
        assert!(update.cell_size_changed);
        assert!(update.cell_pixel_changed);
        assert_eq!(update.old_pixels, PixelSize::new(200, 150));
        assert_eq!(update.new_pixels, PixelSize::new(100, 75));
        assert_eq!(widget.widget_pixels(), PixelSize::new(100, 75));
        assert_eq!(exported.size(), PixelSize::new(100, 75));
    }

    #[test]
    fn widget_update_canvas_preserves_top_left_by_default() {
        let mut widget = ImageViewportWidget::from_image(
            source_image(),
            CanvasMetrics::new(CellSize::new(10, 5), CellPixel::new(20, 30)),
        );
        widget.set_offset(ScaledPixelOffset::new(120, 80));

        widget.update_canvas(CanvasMetrics::new(
            CellSize::new(20, 10),
            CellPixel::new(20, 30),
        ));

        assert_eq!(widget.viewport().offset(), ScaledPixelOffset::new(120, 80));
    }

    #[test]
    fn widget_options_can_fit_image_to_box() {
        let widget = ImageViewportWidget::from_image_with_options(
            source_image(),
            CanvasMetrics::new(CellSize::new(20, 10), CellPixel::new(10, 15)),
            ImageViewportOptions {
                initial_scale: ImageViewportInitialScale::FitToBox,
                resize_policy: ResizePolicy::PreserveTopLeft,
            },
        )
        .unwrap();

        assert_eq!(widget.viewport().scale(), ImageScale::new(0.25).unwrap());
        assert_eq!(widget.viewport().offset(), ScaledPixelOffset::ZERO);
        assert_eq!(
            widget.viewport().theoretical_size().unwrap(),
            PixelSize::new(200, 150)
        );
    }

    #[test]
    fn widget_update_canvas_can_preserve_center_source_point() {
        let old_canvas = CanvasMetrics::new(CellSize::new(10, 5), CellPixel::new(20, 30));
        let new_canvas = CanvasMetrics::new(CellSize::new(20, 10), CellPixel::new(20, 30));
        let mut widget = ImageViewportWidget::from_image(source_image(), old_canvas);
        widget.set_offset(ScaledPixelOffset::new(120, 80));
        widget.set_resize_policy(ResizePolicy::PreserveCenter);

        let before = source_at_widget_center(widget.viewport(), old_canvas.pixels());
        widget.update_canvas(new_canvas);
        let after = source_at_widget_center(widget.viewport(), new_canvas.pixels());

        assert!((before.0 - after.0).abs() < f64::EPSILON);
        assert!((before.1 - after.1).abs() < f64::EPSILON);
    }

    #[test]
    fn widget_update_canvas_can_pin_a_source_point_to_center() {
        let mut widget = ImageViewportWidget::from_image(
            source_image(),
            CanvasMetrics::new(CellSize::new(10, 10), CellPixel::new(10, 10)),
        );
        widget.set_resize_policy(ResizePolicy::PreserveSourcePoint { x: 460.0, y: 300.0 });

        widget.update_canvas(CanvasMetrics::new(
            CellSize::new(20, 20),
            CellPixel::new(10, 10),
        ));

        assert_eq!(widget.viewport().offset(), ScaledPixelOffset::new(360, 200));
    }

    #[test]
    fn widget_apply_zoom_uses_current_canvas_center() {
        let canvas = CanvasMetrics::new(CellSize::new(10, 5), CellPixel::new(20, 30));
        let mut widget = ImageViewportWidget::from_image(source_image(), canvas);
        widget.set_offset(ScaledPixelOffset::new(100, 50));
        widget.set_zoom(ZoomFactor::new(2.0).unwrap());

        let before = source_at_widget_center(widget.viewport(), canvas.pixels());
        widget.apply_zoom(ZoomDirection::In).unwrap();
        let after = source_at_widget_center(widget.viewport(), canvas.pixels());

        assert!((before.0 - after.0).abs() < f64::EPSILON);
        assert!((before.1 - after.1).abs() < f64::EPSILON);
        assert_eq!(widget.viewport().scale(), ImageScale::new(2.0).unwrap());
    }

    #[test]
    fn pixels_outside_the_scaled_image_are_transparent() {
        let mut viewport = ImageViewport::new(source_image());
        viewport.set_scale(ImageScale::new(2.0).unwrap());
        viewport.set_offset(ScaledPixelOffset::new(920, 600));

        let exported = viewport.export_rgba(PixelSize::new(1600, 1200)).unwrap();

        assert_eq!(pixel(exported.rgba(), exported.size(), 0, 0)[3], 255);
        assert_eq!(
            pixel(exported.rgba(), exported.size(), 680, 0),
            [0, 0, 0, 0]
        );
        assert_eq!(
            pixel(exported.rgba(), exported.size(), 0, 600),
            [0, 0, 0, 0]
        );
    }

    fn source_at_widget_center(viewport: &ImageViewport, widget: PixelSize) -> (f64, f64) {
        (
            (viewport.offset.x as f64 + widget.width as f64 / 2.0) / viewport.scale.get(),
            (viewport.offset.y as f64 + widget.height as f64 / 2.0) / viewport.scale.get(),
        )
    }

    fn source_image() -> ViewportImage {
        let size = PixelSize::new(800, 600);
        let mut rgba = vec![0; rgba_len(size).unwrap()];
        for y in 0..size.height {
            for x in 0..size.width {
                let color = source_fixture_pixel(x, y);
                let i = (((u64::from(y) * u64::from(size.width)) + u64::from(x)) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(&color);
            }
        }
        ViewportImage::new(size, rgba).unwrap()
    }

    fn source_fixture_pixel(x: u32, y: u32) -> [u8; 4] {
        if x < 200 && y < 150 {
            return [0, 220, 255, 255];
        }
        if (460..560).contains(&x) && (300..375).contains(&y) {
            return [238, 35, 238, 255];
        }
        if y < 16 {
            return [246, 64, 64, 255];
        }
        if y >= 584 {
            return [55, 105, 245, 255];
        }
        if x < 16 {
            return [35, 190, 100, 255];
        }
        if x >= 784 {
            return [245, 212, 55, 255];
        }
        [236, 238, 240, 255]
    }

    fn reference_crop(
        image: &ViewportImage,
        widget: PixelSize,
        offset: ScaledPixelOffset,
        scale: f64,
    ) -> RgbaImage {
        let mut out = vec![0; rgba_len(widget).unwrap()];
        let theoretical = PixelSize::new(
            round_u32(image.size().width as f64 * scale).max(1),
            round_u32(image.size().height as f64 * scale).max(1),
        );
        for y in 0..widget.height {
            let scaled_y = offset.y + i64::from(y);
            if scaled_y < 0 || scaled_y >= i64::from(theoretical.height) {
                continue;
            }
            let src_y = scaled_to_source(scaled_y, image.size().height, scale);
            for x in 0..widget.width {
                let scaled_x = offset.x + i64::from(x);
                if scaled_x < 0 || scaled_x >= i64::from(theoretical.width) {
                    continue;
                }
                let src_x = scaled_to_source(scaled_x, image.size().width, scale);
                copy_rgba(
                    image.rgba(),
                    image.size().width,
                    &mut out,
                    widget.width,
                    src_x,
                    src_y,
                    x,
                    y,
                );
            }
        }
        RgbaImage::new(widget, out).unwrap()
    }

    fn pixel(rgba: &[u8], size: PixelSize, x: u32, y: u32) -> [u8; 4] {
        let i = (((u64::from(y) * u64::from(size.width)) + u64::from(x)) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    }
}
