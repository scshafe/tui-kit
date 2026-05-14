//! Render effects: explicit, data-only descriptions of renderer-side work.
//!
//! `TerminalEffect` is the enum a [`super::Element`] can emit to ask its
//! renderer to upload, place, or tear down terminal-side resources (today,
//! Kitty graphics). Effect application lives on the trait below and on the
//! enum's apply methods; effect *description* stays in the enum so that
//! buffer rendering remains pure.

use std::sync::Arc;

use anyhow::Result;
use ratatui::layout::Rect;

use crate::image::{ImageSurface, ImageSurfaceRegistry, PlaceOptions};
use crate::layout::CellOffset;

use super::Element;

/// Explicit terminal side effect produced by an element.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TerminalEffect {
    EnsureImageLoaded {
        image_id: u32,
        png: Arc<[u8]>,
    },
    PlaceImage {
        origin: CellOffset,
        options: PlaceOptions,
    },
    DeleteImagePlacement {
        image_id: u32,
        placement_id: u32,
    },
    DeletePlacement {
        placement_id: u32,
    },
    DeleteAllPlacements,
    ForgetAllImages,
    FlushImages,
}

impl TerminalEffect {
    pub fn placement_id(&self) -> Option<u32> {
        match self {
            Self::PlaceImage { options, .. } => Some(options.placement_id),
            Self::DeleteImagePlacement { placement_id, .. } => Some(*placement_id),
            Self::DeletePlacement { placement_id } => Some(*placement_id),
            _ => None,
        }
    }

    /// Apply the effect to any [`ImageSurface`].
    ///
    /// Generic surfaces do not expose cursor positioning, so [`Self::PlaceImage`]
    /// applies only the placement options. Use [`Self::apply_to_registry`] when
    /// absolute cell origin matters.
    pub fn apply_to_surface<S: ImageSurface>(&self, surface: &mut S) -> Result<()> {
        match self {
            Self::EnsureImageLoaded { image_id, png } => surface.ensure_loaded(*image_id, png),
            Self::PlaceImage { options, .. } => surface.place(*options),
            Self::DeleteImagePlacement {
                image_id,
                placement_id,
            } => surface.delete_image_placement(*image_id, *placement_id),
            Self::DeletePlacement { placement_id } => surface.delete_placement(*placement_id),
            Self::DeleteAllPlacements => surface.delete_all_placements(),
            Self::ForgetAllImages => surface.forget_all(),
            Self::FlushImages => surface.flush(),
        }
    }

    /// Apply the effect to tui-kit's registry, preserving absolute image origin.
    pub fn apply_to_registry(&self, registry: &mut ImageSurfaceRegistry) -> Result<()> {
        match self {
            Self::PlaceImage { origin, options } => registry.place_at(*origin, *options),
            _ => self.apply_to_surface(registry),
        }
    }
}

/// Element that emits terminal side effects explicitly.
pub trait EffectElement: Element {
    fn terminal_effects(&mut self, area: Rect) -> Result<Vec<TerminalEffect>>;

    fn teardown_effects(&mut self) -> Result<Vec<TerminalEffect>> {
        Ok(Vec::new())
    }
}
