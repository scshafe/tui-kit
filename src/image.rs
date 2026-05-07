//! Image-on-text-cells lifecycle management.
//!
//! Currently only the Kitty graphics protocol is implemented. The
//! [`ImageSurface`] trait is the seam for future Sixel and iTerm2 backends:
//! same `ensure_loaded → place → delete` lifecycle, different wire format.
//!
//! ## Lifecycle
//!
//! 1. `ensure_loaded(image_id, png_bytes)` — uploads the PNG to the
//!    terminal once. Idempotent for the same `image_id`.
//! 2. `place(opts)` — emits a placement at the current cursor position.
//!    Multiple placements per image (different `placement_id`s) are fine.
//! 3. `delete_placement(id)` / `delete_placements_in(ids)` — removes
//!    specific placements. Image data stays loaded so subsequent
//!    `place()` calls don't need to re-transmit.
//! 4. `forget_all()` — frees both placements and loaded image data.
//!    Use on workspace reload, not picker-close.
//! 5. `shutdown()` — emits a global "delete all everything" escape.
//!    Drop-time cleanup only.

use crate::layout::PixelRect;
use crate::tty::write_stdout_all;
use anyhow::Result;
use base64::Engine;
use std::collections::HashSet;
use std::io::{self, Write};

/// A surface that owns the image lifecycle for a particular protocol.
///
/// Implementations: [`KittyImageRegistry`]. Sixel/iTerm2 surfaces will
/// implement this trait when added.
pub trait ImageSurface {
    fn ensure_loaded(&mut self, image_id: u32, png: &[u8]) -> Result<()>;
    fn place(&mut self, opts: PlaceOptions) -> Result<()>;
    fn delete_placement(&mut self, placement_id: u32) -> Result<()>;
    fn delete_all_placements(&mut self) -> Result<()>;
    fn forget_all(&mut self) -> Result<()>;
    fn flush(&self) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub struct PlaceOptions {
    pub image_id: u32,
    pub placement_id: u32,
    pub source: PixelRect,
    pub cell_cols: u16,
    pub cell_rows: u16,
}

#[derive(Debug, Default)]
pub struct KittyImageRegistry {
    loaded: HashSet<u32>,
    placements: HashSet<u32>,
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

    /// Drop-time cleanup. Emits a global delete-all-with-data so we leave
    /// the terminal in a clean state. Don't call mid-session — use
    /// [`forget_all`] if you want to reset images cleanly.
    pub fn shutdown(&mut self) {
        let _ = write_stdout_all(b"\x1b_Ga=d,d=A,q=2;\x1b\\");
    }
}

impl ImageSurface for KittyImageRegistry {
    fn ensure_loaded(&mut self, image_id: u32, png: &[u8]) -> Result<()> {
        if self.loaded.contains(&image_id) {
            return Ok(());
        }
        transmit_png(image_id, png)?;
        self.loaded.insert(image_id);
        Ok(())
    }

    fn place(&mut self, opts: PlaceOptions) -> Result<()> {
        write!(
            io::stdout().lock(),
            "\x1b_Ga=p,i={i},p={p},q=2,X={x},Y={y},W={w},H={h},c={c},r={r};\x1b\\",
            i = opts.image_id,
            p = opts.placement_id,
            x = opts.source.x,
            y = opts.source.y,
            w = opts.source.width,
            h = opts.source.height,
            c = opts.cell_cols,
            r = opts.cell_rows,
        )?;
        self.placements.insert(opts.placement_id);
        Ok(())
    }

    fn delete_placement(&mut self, placement_id: u32) -> Result<()> {
        if !self.placements.remove(&placement_id) {
            return Ok(());
        }
        write!(
            io::stdout().lock(),
            "\x1b_Ga=d,d=p,p={placement_id},q=2;\x1b\\"
        )?;
        Ok(())
    }

    fn delete_all_placements(&mut self) -> Result<()> {
        let to_delete: Vec<u32> = self.placements.iter().copied().collect();
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

/// Conventional placement id reserved for an app's main view image.
pub const MAIN_PLACEMENT_ID: u32 = 1;

/// Base for picker thumbnail placement ids; per-item id is
/// `PICKER_PLACEMENT_ID_BASE + index`.
pub const PICKER_PLACEMENT_ID_BASE: u32 = 100;

pub fn picker_placement_id(item_index: usize) -> u32 {
    PICKER_PLACEMENT_ID_BASE + (item_index as u32)
}
