//! Composed widgets built on top of ratatui primitives.
//!
//! **Stability:** only widgets with a named consumer or a concrete extracted
//! rendering pattern live here. Today that is the presentational dialog widget
//! used by c4tui, [`image_viewport::ImageViewport`], the pixel-space image
//! aperture model, and [`grid::Grid`], the reusable local-coordinate grid
//! container extracted from c4tui's view picker.

pub mod dialog;
pub mod grid;
pub mod image_viewport;
