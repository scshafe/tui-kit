//! Owns the lifecycle of the live terminal: raw mode, alternate screen,
//! explicitly configured input features, [`ratatui::Terminal`] backed by
//! [`crossterm`], and the selected [`ImageSurfaceRegistry`] for image
//! placements.
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
//! selected image backend cleanup so no image placements leak.

use crate::config::{ConfigError, Validate};
use crate::image::{ImageBackendPreference, ImageConfig, ImageSurfaceRegistry};
use crate::layout::CanvasMetrics;
use crate::tty::terminal_metrics;
use anyhow::Result;
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use std::io::{self, Stdout};

type Inner = ratatui::Terminal<CrosstermBackend<Stdout>>;

/// Explicit runtime policy for entering a live terminal session.
///
/// Use named constructors instead of invisible defaults so applications and
/// agents can choose image behavior deliberately before raw mode or the
/// alternate screen are enabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalConfig {
    pub image: ImageConfig,
    pub features: TerminalFeatures,
}

/// Explicit terminal session feature policy.
///
/// These features affect process-global terminal state, so they are configured
/// at session entry and restored on drop rather than hidden inside widgets or
/// input producers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalFeatures {
    pub mouse_capture: bool,
    pub bracketed_paste: bool,
}

impl TerminalConfig {
    /// Strict near-term runtime for Kitty-compatible terminals, including
    /// WezTerm's Kitty graphics support.
    pub fn strict_wezterm_kitty() -> Self {
        Self {
            image: ImageConfig::strict_wezterm_kitty(),
            features: TerminalFeatures::interactive(),
        }
    }

    /// Explicit no-image mode for tests or degraded terminals.
    pub fn degraded_no_images() -> Self {
        Self {
            image: ImageConfig::degraded_no_images(),
            features: TerminalFeatures::interactive(),
        }
    }

    /// Headless/inert test preset. This validates like degraded mode and is
    /// intended for code paths that need a terminal-shaped config without
    /// assuming image support.
    pub fn headless_test() -> Self {
        Self {
            image: ImageConfig::degraded_no_images(),
            features: TerminalFeatures::headless_test(),
        }
    }
}

impl TerminalFeatures {
    /// Interactive terminal preset for apps that want mouse reporting and
    /// bracketed paste events as part of the input stream.
    pub fn interactive() -> Self {
        Self {
            mouse_capture: true,
            bracketed_paste: true,
        }
    }

    /// Inert preset for tests and config preflight where no live terminal
    /// affordances should be implied.
    pub fn headless_test() -> Self {
        Self {
            mouse_capture: false,
            bracketed_paste: false,
        }
    }
}

impl Validate for TerminalConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        self.image.validate()?;
        self.features.validate()
    }
}

impl Validate for TerminalFeatures {
    fn validate(&self) -> Result<(), ConfigError> {
        Ok(())
    }
}

pub struct Terminal {
    inner: Option<Inner>,
    images: ImageSurfaceRegistry,
    features: TerminalFeatures,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("active", &self.inner.is_some())
            .field("features", &self.features)
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
        let images = ImageSurfaceRegistry::from_preference(config.image.backend)?;
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        stdout.execute(crossterm::terminal::EnterAlternateScreen)?;
        stdout.execute(crossterm::cursor::Hide)?;
        if config.features.mouse_capture {
            stdout.execute(crossterm::event::EnableMouseCapture)?;
        }
        if config.features.bracketed_paste {
            stdout.execute(crossterm::event::EnableBracketedPaste)?;
        }
        let backend = CrosstermBackend::new(io::stdout());
        let inner = ratatui::Terminal::new(backend)?;
        Ok(Self {
            inner: Some(inner),
            images,
            features: config.features,
        })
    }

    /// Enter raw mode with an explicit image backend preference.
    pub fn enter_with_image_backend(image_backend: ImageBackendPreference) -> Result<Self> {
        Self::enter_with_config(TerminalConfig {
            image: ImageConfig {
                backend: image_backend,
            },
            features: TerminalFeatures::interactive(),
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
        let mut stdout = io::stdout();
        let _ = io::Write::flush(&mut stdout);
        if self.features.bracketed_paste {
            let _ = stdout.execute(crossterm::event::DisableBracketedPaste);
        }
        if self.features.mouse_capture {
            let _ = stdout.execute(crossterm::event::DisableMouseCapture);
        }
        let _ = stdout.execute(crossterm::cursor::Show);
        let _ = stdout.execute(crossterm::terminal::LeaveAlternateScreen);
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
        assert_eq!(strict.image.backend, ImageBackendPreference::KittyOnly);
        assert_eq!(strict.features, TerminalFeatures::interactive());

        let headless = TerminalConfig::headless_test();
        assert_eq!(headless.image.backend, ImageBackendPreference::Disabled);
        assert_eq!(headless.features, TerminalFeatures::headless_test());
    }

    #[test]
    fn terminal_feature_presets_are_explicit() {
        assert_eq!(
            TerminalFeatures::interactive(),
            TerminalFeatures {
                mouse_capture: true,
                bracketed_paste: true,
            }
        );
        assert_eq!(
            TerminalFeatures::headless_test(),
            TerminalFeatures {
                mouse_capture: false,
                bracketed_paste: false,
            }
        );
    }

    #[test]
    fn terminal_config_validates_image_backend_policy() {
        let error = TerminalConfig {
            image: ImageConfig {
                backend: ImageBackendPreference::AutoDetect { order: vec![] },
            },
            features: TerminalFeatures::interactive(),
        }
        .validate()
        .unwrap_err();

        assert_eq!(error.path, "image.backend.order");
    }

    #[test]
    fn terminal_config_rejects_noop_as_detectable_protocol() {
        let error = TerminalConfig {
            image: ImageConfig {
                backend: ImageBackendPreference::AutoDetect {
                    order: vec![ImageProtocol::Noop],
                },
            },
            features: TerminalFeatures::interactive(),
        }
        .validate()
        .unwrap_err();

        assert_eq!(error.path, "image.backend.order");
    }
}
