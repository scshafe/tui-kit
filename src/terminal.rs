//! Owns the lifecycle of the live terminal: raw mode, alternate screen,
//! mouse capture, [`ratatui::Terminal`] backed by [`crossterm`], and
//! the [`KittyImageRegistry`] for image placements.
//!
//! The expected usage:
//!
//! ```ignore
//! let mut terminal = tui_kit::terminal::Terminal::enter()?;
//! terminal.draw(|frame| {
//!     // build widgets here
//! })?;
//! terminal.images().place(opts)?;
//! ```
//!
//! Drop restores the user's terminal: leaves alt-screen, disables mouse
//! capture, restores cursor visibility, exits raw mode, and emits a
//! global Kitty `delete-all` so no image placements leak.

use crate::image::KittyImageRegistry;
use crate::layout::CanvasMetrics;
use crate::tty::terminal_metrics;
use anyhow::Result;
use ratatui::backend::CrosstermBackend;
use ratatui::Frame;
use std::io::{self, Stdout};

type Inner = ratatui::Terminal<CrosstermBackend<Stdout>>;

pub struct Terminal {
    inner: Option<Inner>,
    images: KittyImageRegistry,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("active", &self.inner.is_some())
            .finish()
    }
}

impl Terminal {
    /// Enter raw mode, alt screen, mouse capture, and construct a
    /// ratatui terminal. Restored on drop.
    pub fn enter() -> Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(
            stdout,
            crossterm::terminal::EnterAlternateScreen,
            crossterm::cursor::Hide,
            crossterm::event::EnableMouseCapture,
        )?;
        let backend = CrosstermBackend::new(io::stdout());
        let inner = ratatui::Terminal::new(backend)?;
        Ok(Self {
            inner: Some(inner),
            images: KittyImageRegistry::default(),
        })
    }

    pub fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Frame<'_>),
    {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("terminal session not initialised"))?;
        inner.draw(|frame| f(frame))?;
        Ok(())
    }

    pub fn images(&mut self) -> &mut KittyImageRegistry {
        &mut self.images
    }

    pub fn metrics(&self) -> CanvasMetrics {
        terminal_metrics()
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.inner.take();
        self.images.shutdown();
        let _ = io::Write::flush(&mut io::stdout());
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::event::DisableMouseCapture,
            crossterm::cursor::Show,
            crossterm::terminal::LeaveAlternateScreen,
        );
        let _ = crossterm::terminal::disable_raw_mode();
    }
}
