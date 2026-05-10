//! Owns the lifecycle of the live terminal: raw mode, alternate screen,
//! mouse capture, [`ratatui::Terminal`] backed by [`crossterm`], and
//! the selected [`ImageSurfaceRegistry`] for image placements.
//!
//! The expected usage:
//!
//! ```ignore
//! use tui_kit::terminal::{Terminal, TerminalConfig};
//!
//! let mut terminal = Terminal::enter_with_config(TerminalConfig::strict_wezterm_kitty())?;
//! terminal.draw(|frame| {
//!     // build widgets here
//! })?;
//! terminal.images().place(opts)?;
//! ```
//!
//! Drop restores the user's terminal: leaves alt-screen, disables mouse
//! capture, restores cursor visibility, exits raw mode, and emits a
//! selected image backend cleanup so no image placements leak.
//!
//! **Stability:** consumed by c4tui's terminal session wrapper. tui-kit owns
//! raw-mode/alt-screen/image-registry mechanics; app-specific chrome,
//! lifecycle policy, and workspace state remain outside this module.

use crate::config::{ConfigError, Validate};
use crate::image::{ImageBackendPreference, ImageSurfaceRegistry};
use crate::layout::CanvasMetrics;
use crate::tty::terminal_metrics;
use anyhow::Result;
use ratatui::backend::CrosstermBackend;
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use std::io::{self, Stdout};

type Inner = ratatui::Terminal<CrosstermBackend<Stdout>>;

/// Explicit runtime policy for entering a live terminal session.
///
/// Use named constructors instead of invisible defaults so applications can
/// choose image behavior deliberately before raw mode or the alternate screen
/// are enabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalConfig {
    pub image_backend: ImageBackendPreference,
}

impl TerminalConfig {
    /// Strict near-term runtime for Kitty-compatible terminals, including
    /// WezTerm's Kitty graphics support.
    pub fn strict_wezterm_kitty() -> Self {
        Self {
            image_backend: ImageBackendPreference::strict_kitty(),
        }
    }

    /// Explicit no-image mode for tests or degraded terminals.
    pub fn degraded_no_images() -> Self {
        Self {
            image_backend: ImageBackendPreference::degraded_no_images(),
        }
    }

    /// Headless/inert test preset. This validates like degraded mode and is
    /// intended for code paths that need a terminal-shaped config without
    /// assuming image support.
    pub fn headless_test() -> Self {
        Self::degraded_no_images()
    }
}

impl Validate for TerminalConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        self.image_backend
            .validate()
            .map_err(|error| terminal_image_backend_error(error.path, error.reason))
    }
}

fn terminal_image_backend_error(path: String, reason: String) -> ConfigError {
    let leaf = path
        .strip_prefix("image.backend.")
        .or_else(|| path.strip_prefix("image."))
        .unwrap_or(path.as_str());
    ConfigError::new(format!("terminal.image_backend.{leaf}"), reason)
}

pub struct Terminal {
    inner: Option<Inner>,
    images: ImageSurfaceRegistry,
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
    /// ratatui terminal with the strict WezTerm/Kitty preset.
    /// Restored on drop.
    pub fn enter() -> Result<Self> {
        Self::enter_with_config(TerminalConfig::strict_wezterm_kitty())
    }

    /// Enter raw mode with an explicit terminal configuration.
    ///
    /// This keeps backend selection noisy and machine-readable: invalid or
    /// currently unimplemented protocols fail before terminal setup begins.
    pub fn enter_with_config(config: TerminalConfig) -> Result<Self> {
        config.validate()?;
        let images = ImageSurfaceRegistry::from_preference(config.image_backend)
            .map_err(|error| terminal_image_backend_error(error.path, error.reason))?;
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
            images,
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

    pub fn images(&mut self) -> &mut ImageSurfaceRegistry {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::ImageProtocol;

    #[test]
    fn terminal_config_presets_are_explicit() {
        let strict = TerminalConfig::strict_wezterm_kitty();
        assert_eq!(strict.image_backend, ImageBackendPreference::KittyOnly);

        let headless = TerminalConfig::headless_test();
        assert_eq!(headless.image_backend, ImageBackendPreference::Disabled);
    }

    #[test]
    fn terminal_config_validates_image_backend_policy() {
        let error = TerminalConfig {
            image_backend: ImageBackendPreference::AutoDetect { order: vec![] },
        }
        .validate()
        .unwrap_err();

        assert_eq!(error.path, "terminal.image_backend.order");
    }

    #[test]
    fn terminal_config_rejects_noop_as_detectable_protocol() {
        let error = TerminalConfig {
            image_backend: ImageBackendPreference::AutoDetect {
                order: vec![ImageProtocol::Noop],
            },
        }
        .validate()
        .unwrap_err();

        assert_eq!(error.path, "terminal.image_backend.order");
        assert!(error.reason.contains("degraded fallback"));
    }

    #[test]
    fn terminal_config_rejects_unimplemented_explicit_protocol_before_entry() {
        let error = TerminalConfig {
            image_backend: ImageBackendPreference::Explicit(ImageProtocol::Sixel),
        }
        .validate()
        .unwrap_err();

        assert_eq!(error.path, "terminal.image_backend.protocol");
        assert!(error.reason.contains("not implemented"));
    }

    #[test]
    fn terminal_config_rewrites_image_backend_paths_without_doubling() {
        let error = terminal_image_backend_error(
            "image.backend.protocol".to_string(),
            "not implemented".to_string(),
        );

        assert_eq!(error.path, "terminal.image_backend.protocol");
        assert!(!error.path.contains("backend.backend"));
    }
}
