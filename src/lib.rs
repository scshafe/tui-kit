//! tui-kit — opinionated middleware for terminal UI applications.
//!
//! Sits on top of [`ratatui`] and [`crossterm`]. Provides:
//! - Unified [`events`] channel + producers (input, watcher, scheduler).
//! - Declarative [`keymap`] registry.
//! - Cell + pixel [`tty`] metrics.
//! - Image lifecycle via [`image`] surfaces.
//! - Generic placement / fit / pan / zoom math in [`layout`].
//!
//! See `PLAN.md` for module-by-module map and roadmap.

#![warn(missing_debug_implementations)]

pub mod events;
pub mod image;
pub mod input;
pub mod input_thread;
pub mod keymap;
pub mod layout;
pub mod tty;
