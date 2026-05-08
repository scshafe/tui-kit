//! tui-kit — opinionated middleware for terminal UI applications.
//!
//! Sits on top of [`ratatui`] and [`crossterm`]. Provides:
//! - Unified [`events`] channel + producers (input, watcher, scheduler, ticks).
//! - Declarative [`keymap`] registry.
//! - Cell + pixel [`tty`] metrics.
//! - Image lifecycle surfaces in [`image`].
//! - Generic placement / fit / pan / zoom math in [`layout`].
//! - Slot-aligned, priority-truncated text bars in [`bar`].
//! - Priority-queue [`scheduler`] for async work.
//! - File [`watcher`] integration.
//! - Composed [`widgets`] (picker, dialog) and a [`terminal`] session wrapper.
//!
//! ## Stability
//!
//! Modules used by c4tui (the in-tree consumer) are stable in shape: `bar`,
//! `events`, `image`, `input`, `input_thread`, `keymap`, `layout`, `scheduler`,
//! `terminal`, `tty`, `watcher`, `widgets::picker`, `widgets::dialog`.
//!
//! Modules marked **experimental** at the module level are speculative until a
//! consumer drives their shape: [`component`], [`focus`], [`tick`]. Their APIs
//! are likely to change. See `PLAN_REWRITE.md` for the design discipline.
//!
//! See [`prelude`] for the most common imports.

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
pub mod tick;
pub mod tty;
pub mod watcher;
pub mod widgets;
