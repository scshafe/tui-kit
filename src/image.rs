//! Image-on-text-cells lifecycle management.
//!
//! Currently the Kitty graphics protocol and an explicit no-op degraded
//! surface are implemented. The [`ImageSurface`] trait is the seam for terminal
//! image protocols: same `ensure_loaded → place → delete` lifecycle,
//! different wire format and capabilities. Additional protocols may be added
//! through this trait.
//!
//! ## Lifecycle
//!
//! 1. `ensure_loaded(image_id, png_bytes)` — uploads the PNG to the
//!    terminal once. Idempotent for the same `image_id`.
//! 2. `place(opts)` — emits a placement at the current cursor position.
//!    Multiple placements per image (different `placement_id`s) are fine.
//! 3. `delete_image_placement(image_id, placement_id)` or the tracked
//!    `delete_placement(id)` / `delete_placements_in(ids)` helpers — remove
//!    specific placements. Image data stays loaded so subsequent `place()`
//!    calls don't need to re-transmit.
//! 4. `forget_all()` — frees both placements and loaded image data.
//!    Use on workspace reload, not picker-close.
//! 5. `shutdown()` — emits a global "delete all everything" escape.
//!    Drop-time cleanup only.
//!
//! **Stability:** consumed by c4tui through `Terminal` and image placement
//! paths. Kitty and explicit no-op surfaces are the earned API today; additional
//! protocols should land with a terminal consumer or integration test that uses
//! the same lifecycle.

use crate::config::{ConfigError, Validate};
use crate::layout::{CellOffset, PixelRect, PixelSize};
use crate::tty::write_stdout_all;
use anyhow::Result;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

/// A surface that owns the image lifecycle for a particular protocol.
///
/// Implementations: [`KittyImageRegistry`], [`NoopImageSurface`], and
/// [`ImageSurfaceRegistry`].
pub trait ImageSurface {
    fn capabilities(&self) -> ImageCapabilities;
    fn ensure_loaded(&mut self, image_id: u32, png: &[u8]) -> Result<()>;
    fn place(&mut self, opts: PlaceOptions) -> Result<()>;
    fn delete_image_placement(&mut self, _image_id: u32, placement_id: u32) -> Result<()> {
        self.delete_placement(placement_id)
    }
    fn delete_placement(&mut self, placement_id: u32) -> Result<()>;
    fn delete_all_placements(&mut self) -> Result<()>;
    fn forget_all(&mut self) -> Result<()>;
    fn flush(&self) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ImageProtocol {
    Kitty,
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ImageBackendPreference {
    KittyOnly,
    AutoDetect { order: Vec<ImageProtocol> },
    Explicit(ImageProtocol),
    Disabled,
}

impl ImageBackendPreference {
    pub fn strict_kitty() -> Self {
        Self::KittyOnly
    }

    pub fn degraded_no_images() -> Self {
        Self::Disabled
    }
}

#[derive(Debug)]
enum SelectedImageSurface {
    Kitty(KittyImageRegistry),
    Noop(NoopImageSurface),
}

/// Selected image surface plus the preference that produced it.
///
/// This is intentionally small and explicit for now: runtime probing can be
/// added behind this seam later, while apps already get a machine-readable
/// place to configure, inspect, and degrade image support.
#[derive(Debug)]
pub struct ImageSurfaceRegistry {
    preference: ImageBackendPreference,
    surface: SelectedImageSurface,
}

impl ImageSurfaceRegistry {
    pub fn from_preference(preference: ImageBackendPreference) -> Result<Self, ConfigError> {
        preference.validate()?;
        let surface = match &preference {
            ImageBackendPreference::KittyOnly
            | ImageBackendPreference::Explicit(ImageProtocol::Kitty) => {
                SelectedImageSurface::Kitty(KittyImageRegistry::default())
            }
            ImageBackendPreference::Disabled => SelectedImageSurface::Noop(NoopImageSurface),
            ImageBackendPreference::Explicit(ImageProtocol::Noop) => {
                unreachable!("Explicit(Noop) is rejected by Validate::validate above")
            }
            ImageBackendPreference::AutoDetect { order } => select_auto_detect_surface(order)?,
        };
        Ok(Self {
            preference,
            surface,
        })
    }

    pub fn strict_kitty() -> Self {
        Self::from_preference(ImageBackendPreference::strict_kitty())
            .expect("strict Kitty image backend is a valid built-in preference")
    }

    pub fn degraded_no_images() -> Self {
        Self::from_preference(ImageBackendPreference::degraded_no_images())
            .expect("disabled image backend is a valid built-in preference")
    }

    pub fn preference(&self) -> &ImageBackendPreference {
        &self.preference
    }

    pub fn delete_placements_in<I: IntoIterator<Item = u32>>(
        &mut self,
        placement_ids: I,
    ) -> Result<()> {
        for id in placement_ids {
            self.delete_placement(id)?;
        }
        Ok(())
    }

    pub fn delete_image_placement(&mut self, image_id: u32, placement_id: u32) -> Result<()> {
        ImageSurface::delete_image_placement(self, image_id, placement_id)
    }

    pub fn delete_image_placements_in<I: IntoIterator<Item = (u32, u32)>>(
        &mut self,
        placements: I,
    ) -> Result<()> {
        for (image_id, placement_id) in placements {
            self.delete_image_placement(image_id, placement_id)?;
        }
        Ok(())
    }

    pub fn place_at(&mut self, origin: CellOffset, opts: PlaceOptions) -> Result<()> {
        match &mut self.surface {
            SelectedImageSurface::Kitty(surface) => surface.place_at(origin, opts),
            SelectedImageSurface::Noop(surface) => surface.place(opts),
        }
    }

    /// Drop-time cleanup for image data owned by the selected protocol.
    pub fn shutdown(&mut self) {
        if let SelectedImageSurface::Kitty(surface) = &mut self.surface {
            surface.shutdown();
        }
    }
}

impl Default for ImageSurfaceRegistry {
    fn default() -> Self {
        Self::strict_kitty()
    }
}

impl ImageSurface for ImageSurfaceRegistry {
    fn capabilities(&self) -> ImageCapabilities {
        match &self.surface {
            SelectedImageSurface::Kitty(surface) => surface.capabilities(),
            SelectedImageSurface::Noop(surface) => surface.capabilities(),
        }
    }

    fn ensure_loaded(&mut self, image_id: u32, png: &[u8]) -> Result<()> {
        match &mut self.surface {
            SelectedImageSurface::Kitty(surface) => surface.ensure_loaded(image_id, png),
            SelectedImageSurface::Noop(surface) => surface.ensure_loaded(image_id, png),
        }
    }

    fn place(&mut self, opts: PlaceOptions) -> Result<()> {
        match &mut self.surface {
            SelectedImageSurface::Kitty(surface) => surface.place(opts),
            SelectedImageSurface::Noop(surface) => surface.place(opts),
        }
    }

    fn delete_image_placement(&mut self, image_id: u32, placement_id: u32) -> Result<()> {
        match &mut self.surface {
            SelectedImageSurface::Kitty(surface) => {
                surface.delete_image_placement(image_id, placement_id)
            }
            SelectedImageSurface::Noop(surface) => {
                surface.delete_image_placement(image_id, placement_id)
            }
        }
    }

    fn delete_placement(&mut self, placement_id: u32) -> Result<()> {
        match &mut self.surface {
            SelectedImageSurface::Kitty(surface) => surface.delete_placement(placement_id),
            SelectedImageSurface::Noop(surface) => surface.delete_placement(placement_id),
        }
    }

    fn delete_all_placements(&mut self) -> Result<()> {
        match &mut self.surface {
            SelectedImageSurface::Kitty(surface) => surface.delete_all_placements(),
            SelectedImageSurface::Noop(surface) => surface.delete_all_placements(),
        }
    }

    fn forget_all(&mut self) -> Result<()> {
        match &mut self.surface {
            SelectedImageSurface::Kitty(surface) => surface.forget_all(),
            SelectedImageSurface::Noop(surface) => surface.forget_all(),
        }
    }

    fn flush(&self) -> Result<()> {
        match &self.surface {
            SelectedImageSurface::Kitty(surface) => surface.flush(),
            SelectedImageSurface::Noop(surface) => surface.flush(),
        }
    }
}

fn select_auto_detect_surface(
    order: &[ImageProtocol],
) -> Result<SelectedImageSurface, ConfigError> {
    for protocol in order {
        if *protocol == ImageProtocol::Kitty {
            return Ok(SelectedImageSurface::Kitty(KittyImageRegistry::default()));
        }
    }
    Err(ConfigError::new(
        "image.backend.order",
        "auto-detect order contains no implemented terminal image protocol",
    ))
}

fn explicit_noop_error(path: &'static str) -> ConfigError {
    ConfigError::new(
        path,
        "Noop is a degraded fallback, not a terminal image protocol; use Disabled instead",
    )
}

impl Validate for ImageBackendPreference {
    fn validate(&self) -> Result<(), ConfigError> {
        match self {
            Self::Explicit(ImageProtocol::Noop) => {
                Err(explicit_noop_error("image.backend.protocol"))
            }
            Self::AutoDetect { order } if order.is_empty() => Err(ConfigError::new(
                "image.backend.order",
                "auto-detect backend preference requires at least one protocol",
            )),
            Self::AutoDetect { order } if order.contains(&ImageProtocol::Noop) => {
                Err(explicit_noop_error("image.backend.order"))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageCapabilities {
    pub protocol: ImageProtocol,
    pub placements: bool,
    pub deletion: bool,
    pub source_cropping: bool,
    pub max_pixels: Option<PixelSize>,
    pub transparency: TransparencySupport,
}

impl ImageCapabilities {
    pub fn kitty() -> Self {
        Self {
            protocol: ImageProtocol::Kitty,
            placements: true,
            deletion: true,
            source_cropping: true,
            max_pixels: None,
            transparency: TransparencySupport::Alpha,
        }
    }

    pub fn noop() -> Self {
        Self {
            protocol: ImageProtocol::Noop,
            placements: false,
            deletion: false,
            source_cropping: false,
            max_pixels: Some(PixelSize {
                width: 0,
                height: 0,
            }),
            transparency: TransparencySupport::Unsupported,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TransparencySupport {
    Alpha,
    OpaqueOnly,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceOptions {
    pub image_id: u32,
    pub placement_id: u32,
    pub source: PixelRect,
    pub cell_cols: u16,
    pub cell_rows: u16,
}

#[derive(Debug, Default)]
pub struct NoopImageSurface;

impl ImageSurface for NoopImageSurface {
    fn capabilities(&self) -> ImageCapabilities {
        ImageCapabilities::noop()
    }

    fn ensure_loaded(&mut self, _image_id: u32, _png: &[u8]) -> Result<()> {
        Ok(())
    }

    fn place(&mut self, _opts: PlaceOptions) -> Result<()> {
        Ok(())
    }

    fn delete_placement(&mut self, _placement_id: u32) -> Result<()> {
        Ok(())
    }

    fn delete_all_placements(&mut self) -> Result<()> {
        Ok(())
    }

    fn forget_all(&mut self) -> Result<()> {
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct KittyImageRegistry {
    loaded: HashSet<u32>,
    placements: HashMap<u32, u32>,
}

impl KittyImageRegistry {
    pub fn delete_placements_in<I: IntoIterator<Item = u32>>(
        &mut self,
        placement_ids: I,
    ) -> Result<()> {
        for id in placement_ids {
            self.delete_placement(id)?;
        }
        Ok(())
    }

    pub fn place_at(&mut self, origin: CellOffset, opts: PlaceOptions) -> Result<()> {
        position_cursor(origin)?;
        self.place(opts)
    }

    /// Drop-time cleanup. Emits a global delete-all-with-data so we leave
    /// the terminal in a clean state. Don't call mid-session — use
    /// [`ImageSurface::forget_all`] if you want to reset images cleanly.
    pub fn shutdown(&mut self) {
        let _ = write_stdout_all(b"\x1b_Ga=d,d=A,q=2;\x1b\\");
    }
}

impl ImageSurface for KittyImageRegistry {
    fn capabilities(&self) -> ImageCapabilities {
        ImageCapabilities::kitty()
    }

    fn ensure_loaded(&mut self, image_id: u32, png: &[u8]) -> Result<()> {
        if self.loaded.contains(&image_id) {
            return Ok(());
        }
        transmit_png(image_id, png)?;
        self.loaded.insert(image_id);
        Ok(())
    }

    fn place(&mut self, opts: PlaceOptions) -> Result<()> {
        if self.placements.contains_key(&opts.placement_id) {
            self.delete_placement(opts.placement_id)?;
        }
        write!(io::stdout().lock(), "{}", kitty_place_escape(opts))?;
        self.placements.insert(opts.placement_id, opts.image_id);
        Ok(())
    }

    fn delete_image_placement(&mut self, image_id: u32, placement_id: u32) -> Result<()> {
        if self.placements.get(&placement_id).copied() == Some(image_id) {
            self.placements.remove(&placement_id);
        }
        write!(
            io::stdout().lock(),
            "{}",
            kitty_delete_placement_escape(image_id, placement_id)
        )?;
        Ok(())
    }

    fn delete_placement(&mut self, placement_id: u32) -> Result<()> {
        let Some(image_id) = self.placements.remove(&placement_id) else {
            return Ok(());
        };
        write!(
            io::stdout().lock(),
            "{}",
            kitty_delete_placement_escape(image_id, placement_id)
        )?;
        Ok(())
    }

    fn delete_all_placements(&mut self) -> Result<()> {
        let to_delete: Vec<u32> = self.placements.keys().copied().collect();
        for id in to_delete {
            self.delete_placement(id)?;
        }
        Ok(())
    }

    fn forget_all(&mut self) -> Result<()> {
        for id in self.loaded.iter() {
            write!(io::stdout().lock(), "\x1b_Ga=d,d=I,i={id},q=2;\x1b\\")?;
        }
        self.loaded.clear();
        self.placements.clear();
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        io::stdout().flush()?;
        Ok(())
    }
}

fn position_cursor(origin: CellOffset) -> Result<()> {
    write!(
        io::stdout().lock(),
        "\x1b[{};{}H",
        origin.row.saturating_add(1).max(1),
        origin.col.saturating_add(1).max(1)
    )?;
    Ok(())
}

fn transmit_png(image_id: u32, png: &[u8]) -> Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(png);
    let mut chunks = encoded.as_bytes().chunks(4096).peekable();
    let mut first = true;
    while let Some(chunk) = chunks.next() {
        let more = u8::from(chunks.peek().is_some());
        if first {
            write!(
                io::stdout().lock(),
                "\x1b_Ga=t,f=100,i={image_id},m={more};{}\x1b\\",
                std::str::from_utf8(chunk)?
            )?;
            first = false;
        } else {
            write!(
                io::stdout().lock(),
                "\x1b_Gi={image_id},m={more};{}\x1b\\",
                std::str::from_utf8(chunk)?
            )?;
        }
        io::stdout().flush()?;
    }
    Ok(())
}

fn kitty_delete_placement_escape(image_id: u32, placement_id: u32) -> String {
    format!("\x1b_Ga=d,d=i,i={image_id},p={placement_id},q=2;\x1b\\")
}

fn kitty_place_escape(opts: PlaceOptions) -> String {
    // Kitty uses lower-case x/y/w/h for the source crop. Upper-case X/Y are
    // sub-cell destination offsets and would cause zoom crops to be ignored.
    format!(
        "\x1b_Ga=p,i={i},p={p},q=2,x={x},y={y},w={w},h={h},c={c},r={r};\x1b\\",
        i = opts.image_id,
        p = opts.placement_id,
        x = opts.source.x,
        y = opts.source.y,
        w = opts.source.width,
        h = opts.source.height,
        c = opts.cell_cols,
        r = opts.cell_rows,
    )
}

/// Conventional placement id reserved for an app's main view image.
pub const MAIN_PLACEMENT_ID: u32 = 1;

/// Base for picker thumbnail placement ids; per-item id is
/// `PICKER_PLACEMENT_ID_BASE + index`.
pub const PICKER_PLACEMENT_ID_BASE: u32 = 100;

pub fn picker_placement_id(item_index: usize) -> u32 {
    PICKER_PLACEMENT_ID_BASE + (item_index as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_preference_rejects_empty_auto_detect_order() {
        let error = ImageBackendPreference::AutoDetect { order: vec![] }
            .validate()
            .unwrap_err();

        assert_eq!(error.path, "image.backend.order");
    }

    #[test]
    fn backend_preference_rejects_noop_auto_detect_protocol() {
        let error = ImageBackendPreference::AutoDetect {
            order: vec![ImageProtocol::Noop],
        }
        .validate()
        .unwrap_err();

        assert_eq!(error.path, "image.backend.order");
        assert!(error.reason.contains("degraded fallback"));
        assert!(error.reason.contains("use Disabled"));
    }

    #[test]
    fn surfaces_report_machine_readable_capabilities() {
        let kitty = KittyImageRegistry::default().capabilities();
        assert_eq!(kitty.protocol, ImageProtocol::Kitty);
        assert!(kitty.placements);
        assert!(kitty.source_cropping);

        let registry = ImageSurfaceRegistry::strict_kitty().capabilities();
        assert_eq!(registry.protocol, ImageProtocol::Kitty);
        assert!(registry.deletion);

        let noop = NoopImageSurface.capabilities();
        assert_eq!(noop.protocol, ImageProtocol::Noop);
        assert!(!noop.placements);
        assert_eq!(
            noop.max_pixels,
            Some(PixelSize {
                width: 0,
                height: 0
            })
        );
    }

    #[test]
    fn surface_registry_selects_disabled_backend_as_noop() {
        let registry =
            ImageSurfaceRegistry::from_preference(ImageBackendPreference::Disabled).unwrap();

        assert_eq!(registry.preference(), &ImageBackendPreference::Disabled);
        assert_eq!(registry.capabilities().protocol, ImageProtocol::Noop);
    }

    #[test]
    fn surface_registry_auto_detects_kitty_from_order() {
        let registry = ImageSurfaceRegistry::from_preference(ImageBackendPreference::AutoDetect {
            order: vec![ImageProtocol::Kitty],
        })
        .unwrap();

        assert_eq!(registry.capabilities().protocol, ImageProtocol::Kitty);
    }

    #[test]
    fn noop_surface_accepts_full_lifecycle_without_io() {
        let mut surface = NoopImageSurface;
        surface.ensure_loaded(1, b"not really png").unwrap();
        surface
            .place(PlaceOptions {
                image_id: 1,
                placement_id: 10,
                source: PixelRect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                cell_cols: 1,
                cell_rows: 1,
            })
            .unwrap();
        surface.delete_placement(10).unwrap();
        surface.delete_all_placements().unwrap();
        surface.forget_all().unwrap();
        surface.flush().unwrap();
    }

    #[test]
    fn kitty_delete_placement_targets_image_placement_pair() {
        assert_eq!(
            kitty_delete_placement_escape(7, 9),
            "\x1b_Ga=d,d=i,i=7,p=9,q=2;\x1b\\"
        );
    }

    #[test]
    fn kitty_place_escape_uses_source_crop_keys() {
        let escape = kitty_place_escape(PlaceOptions {
            image_id: 7,
            placement_id: 9,
            source: PixelRect {
                x: 11,
                y: 13,
                width: 17,
                height: 19,
            },
            cell_cols: 23,
            cell_rows: 29,
        });

        assert_eq!(
            escape,
            "\x1b_Ga=p,i=7,p=9,q=2,x=11,y=13,w=17,h=19,c=23,r=29;\x1b\\"
        );
    }

    fn lifecycle_opts(
        image_id: u32,
        placement_id: u32,
        cell_cols: u16,
        cell_rows: u16,
    ) -> PlaceOptions {
        PlaceOptions {
            image_id,
            placement_id,
            source: PixelRect {
                x: 0,
                y: 0,
                width: 16,
                height: 9,
            },
            cell_cols,
            cell_rows,
        }
    }

    #[test]
    fn mock_records_load_then_place_sequence() {
        use crate::testkit::{MockImageCall, MockImageSurface};

        let mut surface = MockImageSurface::default();
        let opts = lifecycle_opts(1, 10, 8, 4);

        surface.ensure_loaded(1, b"png-bytes").unwrap();
        surface.place(opts).unwrap();

        assert_eq!(
            surface.calls(),
            &[
                MockImageCall::EnsureLoaded {
                    image_id: 1,
                    bytes: 9,
                },
                MockImageCall::Place(opts),
            ]
        );
    }

    #[test]
    fn mock_records_place_then_resize_with_stable_placement_id() {
        use crate::testkit::{MockImageCall, MockImageSurface};

        let mut surface = MockImageSurface::default();
        let small = lifecycle_opts(1, 10, 4, 2);
        let large = lifecycle_opts(1, 10, 8, 4);

        surface.place(small).unwrap();
        surface.place(large).unwrap();

        assert_eq!(
            surface.calls(),
            &[MockImageCall::Place(small), MockImageCall::Place(large)]
        );
    }

    #[test]
    fn mock_records_teardown_then_place_round_trip() {
        use crate::testkit::{MockImageCall, MockImageSurface};

        let mut surface = MockImageSurface::default();
        let opts = lifecycle_opts(1, 10, 4, 2);

        surface.place(opts).unwrap();
        surface.delete_image_placement(1, 10).unwrap();
        surface.place(opts).unwrap();

        assert_eq!(
            surface.calls(),
            &[
                MockImageCall::Place(opts),
                MockImageCall::DeleteImagePlacement {
                    image_id: 1,
                    placement_id: 10,
                },
                MockImageCall::Place(opts),
            ]
        );
    }

    #[test]
    fn mock_records_repeated_place_with_identical_options() {
        use crate::testkit::{MockImageCall, MockImageSurface};

        let mut surface = MockImageSurface::default();
        let opts = lifecycle_opts(1, 10, 4, 2);

        surface.place(opts).unwrap();
        surface.place(opts).unwrap();
        surface.place(opts).unwrap();

        assert_eq!(
            surface.calls(),
            &[
                MockImageCall::Place(opts),
                MockImageCall::Place(opts),
                MockImageCall::Place(opts),
            ]
        );
    }

    #[test]
    fn mock_records_forget_all_cycle_requiring_reload() {
        use crate::testkit::{MockImageCall, MockImageSurface};

        let mut surface = MockImageSurface::default();
        let opts = lifecycle_opts(1, 10, 4, 2);

        surface.ensure_loaded(1, b"png").unwrap();
        surface.place(opts).unwrap();
        surface.forget_all().unwrap();
        surface.ensure_loaded(1, b"png").unwrap();
        surface.place(opts).unwrap();

        assert_eq!(
            surface.calls(),
            &[
                MockImageCall::EnsureLoaded {
                    image_id: 1,
                    bytes: 3,
                },
                MockImageCall::Place(opts),
                MockImageCall::ForgetAll,
                MockImageCall::EnsureLoaded {
                    image_id: 1,
                    bytes: 3,
                },
                MockImageCall::Place(opts),
            ]
        );
    }

    #[test]
    fn disabled_registry_full_lifecycle_does_not_panic() {
        let mut registry =
            ImageSurfaceRegistry::from_preference(ImageBackendPreference::Disabled).unwrap();
        let opts = lifecycle_opts(1, 10, 4, 2);

        registry.ensure_loaded(1, b"png").unwrap();
        registry.place(opts).unwrap();
        registry.delete_image_placement(1, 10).unwrap();
        registry.place(opts).unwrap();
        registry.delete_placement(10).unwrap();
        registry.delete_all_placements().unwrap();
        registry.forget_all().unwrap();
        registry.flush().unwrap();

        assert_eq!(registry.preference(), &ImageBackendPreference::Disabled);
        assert_eq!(registry.capabilities().protocol, ImageProtocol::Noop);
    }
}
