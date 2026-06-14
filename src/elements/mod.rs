//! Declarative terminal UI elements built on top of ratatui buffers.
//!
//! The element layer keeps rendering buffer-native while giving consumers
//! first-class UI objects for composition, focus/key scoping, lifecycle hooks,
//! and explicit terminal side effects.

mod containers;
mod core;
mod decorators;
mod effect;
mod image;
mod modal;
mod overlay;
mod window;

pub use containers::{Panel, Stack, StackConstraint, StackDirection};
pub use core::{
    ContainerElement, Element, ElementBorder, ElementExt, ElementOutcome, Padding, Text,
    TextOverflow,
};
pub use decorators::{Bordered, Focusable, KeyMapped, Padded, ScrollY};
pub use effect::{EffectElement, RenderEffect};
pub use image::ImageViewportElement;
pub use modal::Modal;
pub use overlay::Overlay;
pub use window::{
    KeyResolution, KeyScope, KeyScopeResolver, KeyScopeRole, Window, WindowChrome,
    WindowFocusScope, WindowLifecycleEvent, WindowRenderStats, WindowRepaintPolicy,
};

#[cfg(test)]
mod tests;
