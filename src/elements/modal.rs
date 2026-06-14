use std::fmt;

use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::component::{BufferComponent, ComponentChildren, ComponentId, DirtyReason, DirtyState};
use crate::focus::FocusNode;
use crate::input::KeyEvent;

use super::core::{ContainerElement, Element, ElementOutcome};
use super::effect::{EffectElement, RenderEffect};
use super::window::Window;

/// Modal window convenience wrapper.
pub struct Modal<E>
where
    E: Element,
    E::Message: Clone,
{
    window: Window<E>,
}

impl<E> fmt::Debug for Modal<E>
where
    E: Element,
    E::Message: Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Modal").field(&self.window).finish()
    }
}

impl<E> Modal<E>
where
    E: Element,
    E::Message: Clone,
{
    /// Create and immediately activate a modal window.
    ///
    /// Because activation happens inside the constructor, lifecycle hooks added
    /// through `window_mut` after construction will not observe the initial
    /// prefetch, enter, or focus events. Use [`Self::from_window`] when a modal
    /// needs hooks registered before activation.
    pub fn new(id: impl Into<ComponentId>, child: E) -> Self {
        Self::from_window(Window::new(id, child))
    }

    /// Convert a preconfigured window into an activated modal.
    pub fn from_window(window: Window<E>) -> Self {
        let mut window = window.modal();
        window.activate();
        Self { window }
    }

    pub fn window(&self) -> &Window<E> {
        &self.window
    }

    pub fn window_mut(&mut self) -> &mut Window<E> {
        &mut self.window
    }

    pub fn into_window(self) -> Window<E> {
        self.window
    }
}

impl<E> BufferComponent for Modal<E>
where
    E: Element,
    E::Message: Clone,
{
    type Event = KeyEvent;
    type Message = E::Message;

    fn id(&self) -> &ComponentId {
        self.window.id()
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        self.window.render_buffer(area, buffer)
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        self.window.handle_event(event)
    }

    fn dirty(&self) -> &DirtyState {
        self.window.dirty()
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.window.mark_dirty(reason);
    }

    fn clear_dirty(&mut self) {
        self.window.clear_dirty();
    }

    fn focus_node(&self) -> Option<FocusNode> {
        self.window.focus_node()
    }

    fn children(&self) -> ComponentChildren<'_> {
        self.window.children()
    }
}

impl<E> ContainerElement for Modal<E>
where
    E: Element,
    E::Message: Clone,
{
}

impl<E> EffectElement for Modal<E>
where
    E: EffectElement,
    E::Message: Clone,
{
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        self.window.render_effects(area)
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        self.window.teardown_effects()
    }
}
