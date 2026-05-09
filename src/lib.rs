//! tui-kit — opinionated middleware for terminal UI applications.
//!
//! Sits on top of [`ratatui`] and [`crossterm`]. Provides:
//! - Unified [`events`] channel + producers (input, watcher, scheduler).
//! - Declarative [`keymap`] registry.
//! - Cell + pixel [`tty`] metrics.
//! - Image lifecycle surfaces in [`image`].
//! - Generic placement / fit / pan / zoom math in [`layout`].
//! - Slot-aligned, priority-truncated text bars in [`bar`].
//! - Priority-queue [`scheduler`] for async work.
//! - File [`watcher`] integration.
//! - Composed [`widgets`] (dialog) and a [`terminal`] session wrapper.
//!
//! ## Stability
//!
//! All public modules have at least one in-tree consumer, either c4tui or a
//! crate test. The consumer-gate CI job exercises c4tui against the local
//! tui-kit on every push.
//!
//! Module-level docs flag specific surfaces that have not yet been
//! pressure-tested even though their host module has — for example, [`focus`]'s
//! traversal API is unused by c4tui (which uses the modal stack only).
//!
//! See [`prelude`] for the most common imports. See `PLAN_REWRITE.md` for the
//! design discipline.

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
pub mod scheduler;
pub mod terminal;
pub mod testkit;
pub mod tty;
pub mod watcher;
pub mod widgets;
