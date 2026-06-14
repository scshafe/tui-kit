use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use crate::component::{
    BufferComponent, ComponentChildren, ComponentId, ComponentOutcome, DirtyReason, DirtyState,
};
use crate::focus::{FocusId, FocusNode};
use crate::input::KeyEvent;
use crate::keymap::KeyMap;

use super::core::{ContainerElement, Element, ElementBorder, ElementOutcome, Padding};
use super::effect::{EffectElement, RenderEffect};

/// Vertical scroll decorator.
///
/// This wrapper scrolls buffer-rendered output and forwards focus, key, child,
/// and dirty-state behavior. It intentionally does not implement
/// [`EffectElement`] yet: scrolled terminal effects need explicit clipping and
/// source-cropping semantics before image placements can be forwarded safely.
#[derive(Debug, Clone)]
pub struct ScrollY<E: Element> {
    child: E,
    child_ids: Vec<ComponentId>,
    offset: u16,
    dirty: DirtyState,
}

impl<E: Element> ScrollY<E> {
    pub fn new(child: E) -> Self {
        let child_ids = vec![child.id().clone()];
        Self {
            child,
            child_ids,
            offset: 0,
            dirty: DirtyState::paint(DirtyReason::Explicit),
        }
    }

    pub fn with_offset(mut self, offset: u16) -> Self {
        self.offset = offset;
        self
    }

    pub fn offset(&self) -> u16 {
        self.offset
    }

    pub fn scroll_to(&mut self, offset: u16) {
        if self.offset != offset {
            self.offset = offset;
            self.mark_dirty(DirtyReason::Input);
        }
    }

    pub fn scroll_by(&mut self, delta: i16) {
        let next = if delta.is_negative() {
            self.offset.saturating_sub(delta.unsigned_abs())
        } else {
            self.offset.saturating_add(delta as u16)
        };
        self.scroll_to(next);
    }
}

impl<E: Element> BufferComponent for ScrollY<E> {
    type Event = KeyEvent;
    type Message = E::Message;

    fn id(&self) -> &ComponentId {
        self.child.id()
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        if self.offset == 0 {
            self.child.render_buffer(area, buffer)?;
            self.clear_dirty();
            return Ok(());
        }

        let virtual_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_add(self.offset),
        };
        let mut virtual_buffer = Buffer::empty(virtual_area);
        self.child
            .render_buffer(virtual_area, &mut virtual_buffer)?;
        blit_scrolled(&virtual_buffer, buffer, area, self.offset);
        self.clear_dirty();
        Ok(())
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        self.child.handle_event(event)
    }

    fn dirty(&self) -> &DirtyState {
        &self.dirty
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark_paint(reason.clone());
        self.child.mark_dirty(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
        self.child.clear_dirty();
    }

    fn focus_node(&self) -> Option<FocusNode> {
        self.child.focus_node()
    }

    fn children(&self) -> ComponentChildren<'_> {
        &self.child_ids
    }
}

impl<E: Element> ContainerElement for ScrollY<E> {}

fn blit_scrolled(source: &Buffer, target: &mut Buffer, area: Rect, offset: u16) {
    for row in 0..area.height {
        for col in 0..area.width {
            let src = (
                area.x.saturating_add(col),
                area.y.saturating_add(row).saturating_add(offset),
            );
            let dst = (area.x.saturating_add(col), area.y.saturating_add(row));
            if let (Some(source_cell), Some(target_cell)) = (source.cell(src), target.cell_mut(dst))
            {
                *target_cell = source_cell.clone();
            }
        }
    }
}

/// Focusable decorator.
#[derive(Debug, Clone)]
pub struct Focusable<E: Element> {
    child: E,
    child_ids: Vec<ComponentId>,
    node: FocusNode,
    dirty: DirtyState,
}

impl<E: Element> Focusable<E> {
    pub fn new(child: E) -> Self {
        let node = FocusNode::new(FocusId::new(child.id().as_str().to_string()));
        let child_ids = vec![child.id().clone()];
        Self {
            child,
            child_ids,
            node,
            dirty: DirtyState::paint(DirtyReason::Explicit),
        }
    }

    pub fn with_focus_id(mut self, id: impl Into<FocusId>) -> Self {
        self.node.id = id.into();
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.node.enabled = enabled;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.node.visible = visible;
        self
    }
}

impl<E: Element> BufferComponent for Focusable<E> {
    type Event = KeyEvent;
    type Message = E::Message;

    fn id(&self) -> &ComponentId {
        self.child.id()
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        self.child.render_buffer(area, buffer)?;
        self.clear_dirty();
        Ok(())
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        self.child.handle_event(event)
    }

    fn dirty(&self) -> &DirtyState {
        &self.dirty
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark_paint(reason.clone());
        self.child.mark_dirty(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
        self.child.clear_dirty();
    }

    fn focus_node(&self) -> Option<FocusNode> {
        Some(self.node.clone())
    }

    fn children(&self) -> ComponentChildren<'_> {
        &self.child_ids
    }
}

impl<E: Element> ContainerElement for Focusable<E> {}

impl<E> EffectElement for Focusable<E>
where
    E: EffectElement,
{
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        self.child.render_effects(area)
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        self.child.teardown_effects()
    }
}

/// Keymap decorator.
#[derive(Debug, Clone)]
pub struct KeyMapped<E>
where
    E: Element,
    E::Message: Clone,
{
    child: E,
    child_ids: Vec<ComponentId>,
    keymap: KeyMap<E::Message>,
    dirty: DirtyState,
}

impl<E> KeyMapped<E>
where
    E: Element,
    E::Message: Clone,
{
    pub fn new(child: E, keymap: KeyMap<E::Message>) -> Self {
        let child_ids = vec![child.id().clone()];
        Self {
            child,
            child_ids,
            keymap,
            dirty: DirtyState::paint(DirtyReason::Explicit),
        }
    }

    pub fn keymap(&self) -> &KeyMap<E::Message> {
        &self.keymap
    }

    pub fn keymap_mut(&mut self) -> &mut KeyMap<E::Message> {
        &mut self.keymap
    }
}

impl<E> BufferComponent for KeyMapped<E>
where
    E: Element,
    E::Message: Clone,
{
    type Event = KeyEvent;
    type Message = E::Message;

    fn id(&self) -> &ComponentId {
        self.child.id()
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        self.child.render_buffer(area, buffer)?;
        self.clear_dirty();
        Ok(())
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        let child = self.child.handle_event(event)?;
        if child.is_handled() {
            return Ok(child);
        }
        Ok(self
            .keymap
            .lookup(*event)
            .map(ComponentOutcome::Message)
            .unwrap_or(ComponentOutcome::Ignored))
    }

    fn dirty(&self) -> &DirtyState {
        &self.dirty
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark_paint(reason.clone());
        self.child.mark_dirty(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
        self.child.clear_dirty();
    }

    fn focus_node(&self) -> Option<FocusNode> {
        self.child.focus_node()
    }

    fn children(&self) -> ComponentChildren<'_> {
        &self.child_ids
    }
}

impl<E> ContainerElement for KeyMapped<E>
where
    E: Element,
    E::Message: Clone,
{
}

impl<E> EffectElement for KeyMapped<E>
where
    E: EffectElement,
    E::Message: Clone,
{
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        self.child.render_effects(area)
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        self.child.teardown_effects()
    }
}

/// Padding decorator.
#[derive(Debug, Clone)]
pub struct Padded<E: Element> {
    child: E,
    child_ids: Vec<ComponentId>,
    padding: Padding,
    dirty: DirtyState,
}

impl<E: Element> Padded<E> {
    pub fn new(child: E, padding: impl Into<Padding>) -> Self {
        let child_ids = vec![child.id().clone()];
        Self {
            child,
            child_ids,
            padding: padding.into(),
            dirty: DirtyState::paint(DirtyReason::Explicit),
        }
    }

    pub fn padding(&self) -> Padding {
        self.padding
    }

    pub fn child_area(&self, area: Rect) -> Rect {
        self.padding.inner(area)
    }
}

impl<E: Element> BufferComponent for Padded<E> {
    type Event = KeyEvent;
    type Message = E::Message;

    fn id(&self) -> &ComponentId {
        self.child.id()
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        self.child.render_buffer(self.child_area(area), buffer)?;
        self.clear_dirty();
        Ok(())
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        self.child.handle_event(event)
    }

    fn dirty(&self) -> &DirtyState {
        &self.dirty
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark_layout(reason.clone());
        self.child.mark_dirty(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
        self.child.clear_dirty();
    }

    fn focus_node(&self) -> Option<FocusNode> {
        self.child.focus_node()
    }

    fn children(&self) -> ComponentChildren<'_> {
        &self.child_ids
    }
}

impl<E: Element> ContainerElement for Padded<E> {}

impl<E> EffectElement for Padded<E>
where
    E: EffectElement,
{
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        self.child.render_effects(self.child_area(area))
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        self.child.teardown_effects()
    }
}

/// Border decorator.
#[derive(Debug, Clone)]
pub struct Bordered<E: Element> {
    child: E,
    child_ids: Vec<ComponentId>,
    border: ElementBorder,
    dirty: DirtyState,
}

impl<E: Element> Bordered<E> {
    pub fn new(child: E, border: impl Into<ElementBorder>) -> Self {
        let child_ids = vec![child.id().clone()];
        Self {
            child,
            child_ids,
            border: border.into(),
            dirty: DirtyState::paint(DirtyReason::Explicit),
        }
    }

    pub fn child_area(&self, area: Rect) -> Rect {
        self.border.block().inner(area)
    }
}

impl<E: Element> BufferComponent for Bordered<E> {
    type Event = KeyEvent;
    type Message = E::Message;

    fn id(&self) -> &ComponentId {
        self.child.id()
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        self.border.block().render(area, buffer);
        self.child.render_buffer(self.child_area(area), buffer)?;
        self.clear_dirty();
        Ok(())
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        self.child.handle_event(event)
    }

    fn dirty(&self) -> &DirtyState {
        &self.dirty
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark_layout(reason.clone());
        self.child.mark_dirty(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
        self.child.clear_dirty();
    }

    fn focus_node(&self) -> Option<FocusNode> {
        self.child.focus_node()
    }

    fn children(&self) -> ComponentChildren<'_> {
        &self.child_ids
    }
}

impl<E: Element> ContainerElement for Bordered<E> {}

impl<E> EffectElement for Bordered<E>
where
    E: EffectElement,
{
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        self.child.render_effects(self.child_area(area))
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        self.child.teardown_effects()
    }
}
