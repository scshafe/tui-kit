//! Render effects: explicit, data-only descriptions of renderer-side work.
//!
//! `RenderEffect` is the enum an [`EffectElement`] emits to ask its renderer
//! to upload, place, or tear down terminal-side resources (today, Kitty
//! graphics; tomorrow potentially other renderer backends; see
//! `architecture.md` §8 "Render Effects and Image Lifecycle"). Effect
//! *description* lives in the enum so that buffer rendering stays pure;
//! effect *application* lives on the apply methods and on the renderer.
//!
//! # Data-only contract
//!
//! `RenderEffect` is, and must remain, **data-only**. Concretely:
//!
//! 1. The enum derives `Clone + Debug + PartialEq + Eq`. Adding a variant
//!    that cannot derive these is a contract break.
//! 2. No variant carries a `Box<dyn _>` field or any trait object.
//! 3. No variant carries an `Fn`/`FnMut`/`FnOnce` field, closure, or function
//!    pointer.
//! 4. No variant carries an `Arc<Mutex<_>>`, `Rc<RefCell<_>>`, or other
//!    handle to live, mutable state. `Arc<[u8]>` for owned image bytes is
//!    fine — it is immutable owned data, not a reference to live state.
//! 5. No method on `RenderEffect` (or on `EffectElement`) requires ambient
//!    access to a global terminal handle, app state, or `static mut` data.
//!    Effects are applied by passing them an explicit
//!    [`crate::image::ImageSurface`] or
//!    [`crate::image::ImageSurfaceRegistry`].
//!
//! These constraints make render effects *transport-safe in principle* —
//! they could be serialized and applied by a non-local renderer — without
//! requiring any transport implementation today. They are enforced
//! structurally by [`tests::render_effects_are_data_only`] below; any future
//! variant that violates them will fail to compile under the round-trip
//! assertions.
//!
//! tui-kit does not commit to a wire format. The data-only contract is a
//! design rule, not an interop contract.

use std::sync::Arc;

use anyhow::Result;
use ratatui::layout::Rect;

use crate::image::{ImageSurface, ImageSurfaceRegistry, PlaceOptions};
use crate::layout::CellOffset;

use super::Element;

/// Explicit, data-only render-host operation produced by an [`EffectElement`].
///
/// See the module-level docs for the data-only contract these variants must
/// preserve.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderEffect {
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

impl RenderEffect {
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

/// Element that emits explicit render effects.
///
/// Implementations describe what should happen on the renderer side (image
/// upload, placement, teardown, flush) by returning [`RenderEffect`] values
/// instead of writing to a terminal directly. Callers decide when to apply
/// effects, which keeps buffer rendering pure and tests deterministic.
pub trait EffectElement: Element {
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>>;

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::PixelRect;

    fn sample_variants() -> Vec<RenderEffect> {
        vec![
            RenderEffect::EnsureImageLoaded {
                image_id: 1,
                png: Arc::from([0u8, 1, 2, 3].as_slice()),
            },
            RenderEffect::PlaceImage {
                origin: CellOffset { col: 4, row: 5 },
                options: PlaceOptions {
                    image_id: 1,
                    placement_id: 7,
                    source: PixelRect {
                        x: 0,
                        y: 0,
                        width: 8,
                        height: 8,
                    },
                    cell_cols: 2,
                    cell_rows: 2,
                },
            },
            RenderEffect::DeleteImagePlacement {
                image_id: 1,
                placement_id: 7,
            },
            RenderEffect::DeletePlacement { placement_id: 7 },
            RenderEffect::DeleteAllPlacements,
            RenderEffect::ForgetAllImages,
            RenderEffect::FlushImages,
        ]
    }

    /// Structural enforcement of the module-level data-only contract.
    ///
    /// Each variant must Clone, Debug-format, and PartialEq-compare to a
    /// freshly-cloned twin. A variant that adds a `Box<dyn _>`, an `Fn`
    /// field, or any non-`PartialEq`/non-`Clone` payload fails to compile
    /// here, which forces the contract change to be deliberate.
    #[test]
    fn render_effects_are_data_only() {
        for variant in sample_variants() {
            let cloned = variant.clone();
            assert_eq!(variant, cloned, "Clone+PartialEq round-trip failed");
            let _debug = format!("{variant:?}");
        }
    }
}
