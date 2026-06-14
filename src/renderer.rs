//! Renderer backend contract for buffer/effect frames.
//!
//! The contract is intentionally small: a [`RenderFrame`] contains a ratatui
//! [`Buffer`] plus explicit [`RenderEffect`] values, and a
//! [`RendererBackend`] consumes that frame. Local terminal rendering is one
//! backend; tests and future transports can implement the same trait without
//! reaching into terminal globals.

use anyhow::Result;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::image::ImageSurface;

pub use crate::elements::RenderEffect;

/// One renderer-neutral frame: buffer cells plus renderer-side effects.
///
/// The buffer is pure ratatui output. Effects describe image upload,
/// placement, teardown, and flush operations that cannot be represented as
/// cells. Keeping the two together makes a frame testable in-process and
/// transport-safe in principle without defining a wire format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderFrame {
    buffer: Buffer,
    effects: Vec<RenderEffect>,
}

impl RenderFrame {
    pub fn new(buffer: Buffer, effects: impl IntoIterator<Item = RenderEffect>) -> Self {
        Self {
            buffer,
            effects: effects.into_iter().collect(),
        }
    }

    pub fn empty(area: Rect) -> Self {
        Self::new(Buffer::empty(area), [])
    }

    pub fn area(&self) -> Rect {
        *self.buffer.area()
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn effects(&self) -> &[RenderEffect] {
        &self.effects
    }

    pub fn into_parts(self) -> (Buffer, Vec<RenderEffect>) {
        (self.buffer, self.effects)
    }
}

/// Backend that can consume a complete buffer/effect frame.
pub trait RendererBackend {
    fn render_frame(&mut self, frame: &RenderFrame) -> Result<()>;
}

/// Renderer adapter for an [`ImageSurface`].
///
/// This adapter is useful for tests and non-terminal render paths that need to
/// apply render effects while separately inspecting the buffer. Generic image
/// surfaces do not expose cursor positioning, so `PlaceImage` effects apply
/// their placement options without using the effect origin. The local terminal
/// adapter preserves origins through `ImageSurfaceRegistry`.
#[derive(Debug, Clone)]
pub struct ImageSurfaceRenderer<S> {
    surface: S,
    last_buffer: Option<Buffer>,
}

impl<S> ImageSurfaceRenderer<S> {
    pub fn new(surface: S) -> Self {
        Self {
            surface,
            last_buffer: None,
        }
    }

    pub fn surface(&self) -> &S {
        &self.surface
    }

    pub fn surface_mut(&mut self) -> &mut S {
        &mut self.surface
    }

    pub fn last_buffer(&self) -> Option<&Buffer> {
        self.last_buffer.as_ref()
    }

    pub fn into_surface(self) -> S {
        self.surface
    }
}

impl<S: ImageSurface> RendererBackend for ImageSurfaceRenderer<S> {
    fn render_frame(&mut self, frame: &RenderFrame) -> Result<()> {
        self.last_buffer = Some(frame.buffer().clone());
        for effect in frame.effects() {
            effect.apply_to_surface(&mut self.surface)?;
        }
        self.surface.flush()?;
        Ok(())
    }
}
