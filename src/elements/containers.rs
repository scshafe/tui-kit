use std::fmt;

use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Widget};
use serde::{Deserialize, Serialize};

use crate::component::{
    BufferComponent, ComponentChildren, ComponentId, ComponentOutcome, DirtyReason, DirtyState,
};
use crate::focus::FocusNode;
use crate::input::KeyEvent;

use super::core::{ContainerElement, Element, ElementOutcome, Padding};
use super::effect::{EffectElement, RenderEffect};

/// Dumb visual container around a single child.
#[derive(Debug, Clone)]
pub struct Panel<E: Element> {
    id: ComponentId,
    child: E,
    child_ids: Vec<ComponentId>,
    title: Option<String>,
    footer: Option<String>,
    border: bool,
    padding: Padding,
    border_style: Style,
    dirty: DirtyState,
}

impl<E: Element> Panel<E> {
    pub fn new(id: impl Into<ComponentId>, child: E) -> Self {
        let child_ids = vec![child.id().clone()];
        Self {
            id: id.into(),
            child,
            child_ids,
            title: None,
            footer: None,
            border: true,
            padding: Padding::ZERO,
            border_style: Style::default(),
            dirty: DirtyState::paint(DirtyReason::Explicit),
        }
    }

    pub fn child(&self) -> &E {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut E {
        self.mark_dirty(DirtyReason::Explicit);
        &mut self.child
    }

    pub fn into_child(self) -> E {
        self.child
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    pub fn border(mut self, enabled: bool) -> Self {
        self.border = enabled;
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }

    pub fn child_area(&self, area: Rect) -> Rect {
        let bordered = if self.border {
            self.block().inner(area)
        } else {
            area
        };
        self.padding.inner(bordered)
    }

    fn block(&self) -> Block<'_> {
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.border_style);
        if let Some(title) = &self.title {
            block = block.title(format!(" {} ", title.trim()));
        }
        if let Some(footer) = &self.footer {
            block = block.title_bottom(footer.clone());
        }
        block
    }
}

impl<E: Element> BufferComponent for Panel<E> {
    type Event = KeyEvent;
    type Message = E::Message;

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        if self.border {
            self.block().render(area, buffer);
        }
        let child_area = self.child_area(area);
        self.child.render_buffer(child_area, buffer)?;
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

impl<E: Element> ContainerElement for Panel<E> {}

impl<E> EffectElement for Panel<E>
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

/// Vertical or horizontal stack.
pub struct Stack<M> {
    id: ComponentId,
    direction: StackDirection,
    children: Vec<StackChild<M>>,
    child_ids: Vec<ComponentId>,
    dirty: DirtyState,
}

impl<M> fmt::Debug for Stack<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stack")
            .field("id", &self.id)
            .field("direction", &self.direction)
            .field("children", &self.child_ids)
            .field("dirty", &self.dirty)
            .finish()
    }
}

struct StackChild<M> {
    element: ChildElement<M>,
    constraint: StackConstraint,
}

pub(super) enum ChildElement<M> {
    Buffer(Box<dyn Element<Message = M>>),
    Effect(Box<dyn EffectElement<Message = M>>),
}

impl<M> ChildElement<M> {
    pub(super) fn render(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        match self {
            Self::Buffer(element) => element.render_buffer(area, buffer),
            Self::Effect(element) => element.render_buffer(area, buffer),
        }
    }

    pub(super) fn handle_key(&mut self, event: &KeyEvent) -> Result<ElementOutcome<M>> {
        match self {
            Self::Buffer(element) => element.handle_event(event),
            Self::Effect(element) => element.handle_event(event),
        }
    }

    pub(super) fn clear_dirty(&mut self) {
        match self {
            Self::Buffer(element) => element.clear_dirty(),
            Self::Effect(element) => element.clear_dirty(),
        }
    }

    pub(super) fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        match self {
            Self::Buffer(_) => Ok(Vec::new()),
            Self::Effect(element) => element.render_effects(area),
        }
    }

    pub(super) fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        match self {
            Self::Buffer(_) => Ok(Vec::new()),
            Self::Effect(element) => element.teardown_effects(),
        }
    }
}

impl<M> Stack<M> {
    pub fn vertical(id: impl Into<ComponentId>) -> Self {
        Self::new(id, StackDirection::Vertical)
    }

    pub fn horizontal(id: impl Into<ComponentId>) -> Self {
        Self::new(id, StackDirection::Horizontal)
    }

    pub fn new(id: impl Into<ComponentId>, direction: StackDirection) -> Self {
        Self {
            id: id.into(),
            direction,
            children: Vec::new(),
            child_ids: Vec::new(),
            dirty: DirtyState::layout(DirtyReason::Explicit),
        }
    }

    /// Append a buffer-rendered child to the stack.
    pub fn push<E>(&mut self, element: E, constraint: StackConstraint) -> &mut Self
    where
        E: Element<Message = M> + 'static,
    {
        self.child_ids.push(element.id().clone());
        self.children.push(StackChild {
            element: ChildElement::Buffer(Box::new(element)),
            constraint,
        });
        self.mark_dirty(DirtyReason::Explicit);
        self
    }

    /// Append a child that can also emit terminal effects.
    pub fn push_effect_child<E>(&mut self, element: E, constraint: StackConstraint) -> &mut Self
    where
        E: EffectElement<Message = M> + 'static,
    {
        self.child_ids.push(element.id().clone());
        self.children.push(StackChild {
            element: ChildElement::Effect(Box::new(element)),
            constraint,
        });
        self.mark_dirty(DirtyReason::Explicit);
        self
    }

    pub fn with_child<E>(mut self, element: E, constraint: StackConstraint) -> Self
    where
        E: Element<Message = M> + 'static,
    {
        self.push(element, constraint);
        self
    }

    pub fn with_effect_child<E>(mut self, element: E, constraint: StackConstraint) -> Self
    where
        E: EffectElement<Message = M> + 'static,
    {
        self.push_effect_child(element, constraint);
        self
    }

    pub fn direction(&self) -> StackDirection {
        self.direction
    }

    pub fn layout_areas(&self, area: Rect) -> Vec<Rect> {
        let main = match self.direction {
            StackDirection::Vertical => area.height,
            StackDirection::Horizontal => area.width,
        };
        let lengths = solve_stack_lengths(
            main,
            self.children
                .iter()
                .map(|child| child.constraint)
                .collect::<Vec<_>>()
                .as_slice(),
        );
        let mut offset = 0u16;
        lengths
            .into_iter()
            .map(|length| {
                let rect = match self.direction {
                    StackDirection::Vertical => Rect {
                        x: area.x,
                        y: area.y.saturating_add(offset),
                        width: area.width,
                        height: length,
                    },
                    StackDirection::Horizontal => Rect {
                        x: area.x.saturating_add(offset),
                        y: area.y,
                        width: length,
                        height: area.height,
                    },
                };
                offset = offset.saturating_add(length);
                rect
            })
            .collect()
    }
}

impl<M> BufferComponent for Stack<M> {
    type Event = KeyEvent;
    type Message = M;

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        let areas = self.layout_areas(area);
        for (child, child_area) in self.children.iter_mut().zip(areas) {
            child.element.render(child_area, buffer)?;
        }
        self.clear_dirty();
        Ok(())
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        for child in self.children.iter_mut().rev() {
            let outcome = child.element.handle_key(event)?;
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
        for child in &mut self.children {
            child.element.clear_dirty();
        }
    }

    fn children(&self) -> ComponentChildren<'_> {
        &self.child_ids
    }
}

impl<M> ContainerElement for Stack<M> {}

impl<M> EffectElement for Stack<M> {
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        let areas = self.layout_areas(area);
        let mut effects = Vec::new();
        for (child, child_area) in self.children.iter_mut().zip(areas) {
            effects.extend(child.element.render_effects(child_area)?);
        }
        Ok(effects)
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        let mut effects = Vec::new();
        for child in self.children.iter_mut().rev() {
            effects.extend(child.element.teardown_effects()?);
        }
        Ok(effects)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackDirection {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackConstraint {
    /// Exact terminal-cell length along the stack direction.
    Length(u16),
    /// Percentage of the available stack length, capped at 100.
    Percentage(u16),
    /// Minimum terminal-cell length that also shares remaining space.
    Min(u16),
    /// Weighted share of remaining space after fixed reservations.
    Fill(u16),
}

fn solve_stack_lengths(total: u16, constraints: &[StackConstraint]) -> Vec<u16> {
    if constraints.is_empty() {
        return Vec::new();
    }

    let mut lengths = vec![0u16; constraints.len()];
    let mut reserved = 0u16;
    let mut flex_weight = 0u16;

    for (idx, constraint) in constraints.iter().copied().enumerate() {
        match constraint {
            StackConstraint::Length(value) => {
                let value = value.min(total.saturating_sub(reserved));
                lengths[idx] = value;
                reserved = reserved.saturating_add(value);
            }
            StackConstraint::Percentage(percent) => {
                let value = ((u32::from(total) * u32::from(percent.min(100))) / 100) as u16;
                let value = value.min(total.saturating_sub(reserved));
                lengths[idx] = value;
                reserved = reserved.saturating_add(value);
            }
            StackConstraint::Min(value) => {
                let value = value.min(total.saturating_sub(reserved));
                lengths[idx] = value;
                reserved = reserved.saturating_add(value);
                flex_weight = flex_weight.saturating_add(1);
            }
            StackConstraint::Fill(weight) => {
                flex_weight = flex_weight.saturating_add(weight.max(1));
            }
        }
    }

    let mut remaining = total.saturating_sub(reserved);
    if flex_weight == 0 {
        return lengths;
    }
    for (idx, constraint) in constraints.iter().copied().enumerate() {
        let weight = match constraint {
            StackConstraint::Min(_) => 1,
            StackConstraint::Fill(weight) => weight.max(1),
            StackConstraint::Length(_) | StackConstraint::Percentage(_) => continue,
        };
        let value = if weight == flex_weight {
            remaining
        } else {
            ((u32::from(remaining) * u32::from(weight)) / u32::from(flex_weight)) as u16
        };
        lengths[idx] = lengths[idx].saturating_add(value);
        remaining = remaining.saturating_sub(value);
        flex_weight = flex_weight.saturating_sub(weight);
    }
    lengths
}
