//! tui-kit — opinionated middleware for terminal UI applications.
//!
//! Sits on top of [`ratatui`] and [`crossterm`]. Provides:
//! - Unified [`events`] channel + producers (input, watcher, scheduler, ticks).
//! - Optional [`component`] primitives for IDs, dirty tracking, and retained-ish UI.
//! - Declarative [`keymap`] registry.
//! - Cell + pixel [`tty`] metrics.
//! - Explicit [`image`] configuration and image lifecycle surfaces.
//! - Generic placement / fit / pan / zoom math in [`layout`].
//! - Slot-aligned, priority-truncated text bars in [`bar`].
//! - Priority-queue [`scheduler`] for async work with explicit worker configuration.
//! - Top-level [`runtime`] configuration for validated subsystem policy bundles.
//! - Named-role [`theme`] configuration with noisy validation.
//! - File [`watcher`] integration.
//! - Composed [`widgets`] (list, table, tree, tabs, picker, dialog) and a [`terminal`] session wrapper.
//!
//! See [`prelude`] for the most common imports. See `PLAN.md` in the repo
//! for the module-by-module map and roadmap.

#![warn(missing_debug_implementations)]

pub mod bar;
pub mod component;
pub mod config;
pub mod events;
pub mod focus;
pub mod image;
pub mod input;
pub mod input_thread;
pub mod keymap;
pub mod layout;
pub mod prelude;
pub mod runtime;
pub mod scheduler;
pub mod terminal;
pub mod testkit;
pub mod theme;
pub mod tick;
pub mod tty;
pub mod watcher;
pub mod widgets;
