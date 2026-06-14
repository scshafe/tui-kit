use std::fmt;

use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::component::{
    BufferComponent, ComponentChildren, ComponentId, ComponentOutcome, DirtyReason, DirtyState,
};
use crate::input::KeyEvent;

use super::containers::ChildElement;
use super::core::{ContainerElement, Element, ElementOutcome};
use super::effect::{EffectElement, RenderEffect};

/// Layered overlay surface.
///
/// Modal layers capture key routing, but they do not hide or suppress rendering
/// and terminal effects from lower layers. Every layer remains visually and
/// effectually active unless the caller removes it.
pub struct Overlay<M> {
    id: ComponentId,
    layers: Vec<OverlayLayer<M>>,
    child_ids: Vec<ComponentId>,
    dirty: DirtyState,
}

impl<M> fmt::Debug for Overlay<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Overlay")
            .field("id", &self.id)
            .field("layers", &self.child_ids)
            .field("dirty", &self.dirty)
            .finish()
    }
}

struct OverlayLayer<M> {
    element: ChildElement<M>,
    area: Option<Rect>,
    modal: bool,
}

impl<M> Overlay<M> {
    pub fn new(id: impl Into<ComponentId>) -> Self {
        Self {
            id: id.into(),
            layers: Vec::new(),
            child_ids: Vec::new(),
            dirty: DirtyState::layout(DirtyReason::Explicit),
        }
    }

    /// Push a non-modal buffer-rendered layer.
    pub fn push_layer<E>(&mut self, element: E, area: Option<Rect>) -> &mut Self
    where
        E: Element<Message = M> + 'static,
    {
        self.child_ids.push(element.id().clone());
        self.layers.push(OverlayLayer {
            element: ChildElement::Buffer(Box::new(element)),
            area,
            modal: false,
        });
        self.mark_dirty(DirtyReason::Explicit);
        self
    }

    /// Push a non-modal layer that can also emit terminal effects.
    pub fn push_effect_layer<E>(&mut self, element: E, area: Option<Rect>) -> &mut Self
    where
        E: EffectElement<Message = M> + 'static,
    {
        self.child_ids.push(element.id().clone());
        self.layers.push(OverlayLayer {
            element: ChildElement::Effect(Box::new(element)),
            area,
            modal: false,
        });
        self.mark_dirty(DirtyReason::Explicit);
        self
    }

    /// Push a modal buffer-rendered layer that captures key routing.
    pub fn push_modal_layer<E>(&mut self, element: E, area: Option<Rect>) -> &mut Self
    where
        E: Element<Message = M> + 'static,
    {
        self.child_ids.push(element.id().clone());
        self.layers.push(OverlayLayer {
            element: ChildElement::Buffer(Box::new(element)),
            area,
            modal: true,
        });
        self.mark_dirty(DirtyReason::Explicit);
        self
    }

    /// Push a modal layer that captures key routing and emits terminal effects.
    pub fn push_modal_effect_layer<E>(&mut self, element: E, area: Option<Rect>) -> &mut Self
    where
        E: EffectElement<Message = M> + 'static,
    {
        self.child_ids.push(element.id().clone());
        self.layers.push(OverlayLayer {
            element: ChildElement::Effect(Box::new(element)),
            area,
            modal: true,
        });
        self.mark_dirty(DirtyReason::Explicit);
        self
    }

    pub fn with_layer<E>(mut self, element: E, area: Option<Rect>) -> Self
    where
        E: Element<Message = M> + 'static,
    {
        self.push_layer(element, area);
        self
    }

    pub fn with_effect_layer<E>(mut self, element: E, area: Option<Rect>) -> Self
    where
        E: EffectElement<Message = M> + 'static,
    {
        self.push_effect_layer(element, area);
        self
    }

    pub fn with_modal_layer<E>(mut self, element: E, area: Option<Rect>) -> Self
    where
        E: Element<Message = M> + 'static,
    {
        self.push_modal_layer(element, area);
        self
    }

    pub fn with_modal_effect_layer<E>(mut self, element: E, area: Option<Rect>) -> Self
    where
        E: EffectElement<Message = M> + 'static,
    {
        self.push_modal_effect_layer(element, area);
        self
    }
}

impl<M> BufferComponent for Overlay<M> {
    type Event = KeyEvent;
    type Message = M;

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        for layer in &mut self.layers {
            layer.element.render(layer.area.unwrap_or(area), buffer)?;
        }
        self.clear_dirty();
        Ok(())
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        if let Some(layer) = self.layers.iter_mut().rev().find(|layer| layer.modal) {
            return layer.element.handle_key(event);
        }
        for layer in self.layers.iter_mut().rev() {
            let outcome = layer.element.handle_key(event)?;
            if outcome.is_handled() {
                return Ok(outcome);
            }
        }
        Ok(ComponentOutcome::Ignored)
    }

    fn dirty(&self) -> &DirtyState {
        &self.dirty
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark_layout(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
        for layer in &mut self.layers {
            layer.element.clear_dirty();
        }
    }

    fn children(&self) -> ComponentChildren<'_> {
        &self.child_ids
    }
}

impl<M> ContainerElement for Overlay<M> {}

impl<M> EffectElement for Overlay<M> {
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        let mut effects = Vec::new();
        for layer in &mut self.layers {
            effects.extend(layer.element.render_effects(layer.area.unwrap_or(area))?);
        }
        Ok(effects)
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        let mut effects = Vec::new();
        for layer in self.layers.iter_mut().rev() {
            effects.extend(layer.element.teardown_effects()?);
        }
        Ok(effects)
    }
}
