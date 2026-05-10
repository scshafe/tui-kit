//! Composed widgets built on top of ratatui primitives.
//!
//! **Stability:** only widgets with a named consumer or a concrete extracted
//! rendering pattern live here. Today that is the presentational dialog widget
//! used by c4tui, [`image_box::ImageBox`], a convenience layer over the public
//! image placement primitives, and [`image_viewport::ImageViewport`], the
//! pixel-space image aperture model.

pub mod dialog;
pub mod image_box;
pub mod image_viewport;
