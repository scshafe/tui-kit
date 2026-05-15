//! Declarative terminal UI elements built on top of ratatui buffers.
//!
//! The element layer keeps rendering buffer-native while giving consumers
//! first-class UI objects for composition, focus/key scoping, lifecycle hooks,
//! and explicit terminal side effects.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Widget};
use serde::{Deserialize, Serialize};

use crate::component::{
    BufferComponent, ComponentChildren, ComponentId, ComponentOutcome, DirtyReason, DirtyState,
};
use crate::focus::{FocusId, FocusNode, FocusScopeKind};
use crate::input::KeyEvent;
use crate::keymap::KeyMap;
use crate::layout::{CanvasMetrics, CellOffset, CellPixel, CellSize};
use crate::widgets::image_viewport::ImageViewportWidget;

/// Alias retained so element event handlers read independently from components.
pub type ElementOutcome<Message> = ComponentOutcome<Message>;

/// Buffer-rendered UI object: a [`BufferComponent`] whose event type is keyboard input.
///
/// `Element` is a marker subtrait. Every implementation is written as
/// `impl BufferComponent for Foo` (with `type Event = KeyEvent;`), and the blanket
/// impl below makes any such type automatically `Element`. The `Element` name
/// remains useful in `dyn Element<Message = M>` and `impl Element<Message = M>`
/// positions to express "buffer-rendered keyboard-driven UI object" without
/// repeating the `BufferComponent<Event = KeyEvent>` bound at every call site.
pub trait Element: BufferComponent<Event = KeyEvent> {}

impl<T> Element for T where T: BufferComponent<Event = KeyEvent> {}

/// Element with inspectable child membership.
pub trait ContainerElement: Element {
    fn child_count(&self) -> usize {
        self.children().len()
    }
}

mod effect;
pub use effect::{EffectElement, RenderEffect};

/// Chainable behavior decorators for elements.
pub trait ElementExt: Element + Sized {
    fn scroll_y(self) -> ScrollY<Self> {
        ScrollY::new(self)
    }

    fn with_scroll_y(self, offset: u16) -> ScrollY<Self> {
        ScrollY::new(self).with_offset(offset)
    }

    fn focusable(self) -> Focusable<Self> {
        Focusable::new(self)
    }

    /// Wrap this element with a local keymap decorator.
    ///
    /// `Window` also has an inherent `with_keymap` method. On a `Window`, Rust
    /// resolves the inherent method first, configuring the window key scope
    /// instead of wrapping it in [`KeyMapped`].
    fn with_keymap(self, keymap: KeyMap<Self::Message>) -> KeyMapped<Self>
    where
        Self::Message: Clone,
    {
        KeyMapped::new(self, keymap)
    }

    fn with_padding(self, padding: impl Into<Padding>) -> Padded<Self> {
        Padded::new(self, padding)
    }

    fn with_border(self, border: impl Into<ElementBorder>) -> Bordered<Self> {
        Bordered::new(self, border)
    }
}

impl<E: Element> ElementExt for E {}

/// Cell padding around an element.
///
/// Tuple conversions use `(horizontal, vertical)` for two values and
/// `(left, right, top, bottom)` for four values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Padding {
    pub left: u16,
    pub right: u16,
    pub top: u16,
    pub bottom: u16,
}

impl Padding {
    pub const ZERO: Self = Self {
        left: 0,
        right: 0,
        top: 0,
        bottom: 0,
    };

    pub const fn uniform(amount: u16) -> Self {
        Self {
            left: amount,
            right: amount,
            top: amount,
            bottom: amount,
        }
    }

    pub const fn symmetric(horizontal: u16, vertical: u16) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }

    pub const fn new(left: u16, right: u16, top: u16, bottom: u16) -> Self {
        Self {
            left,
            right,
            top,
            bottom,
        }
    }

    pub fn inner(self, area: Rect) -> Rect {
        let x_inset = self.left.saturating_add(self.right).min(area.width);
        let y_inset = self.top.saturating_add(self.bottom).min(area.height);
        Rect {
            x: area.x.saturating_add(self.left.min(area.width)),
            y: area.y.saturating_add(self.top.min(area.height)),
            width: area.width.saturating_sub(x_inset),
            height: area.height.saturating_sub(y_inset),
        }
    }
}

impl From<u16> for Padding {
    fn from(value: u16) -> Self {
        Self::uniform(value)
    }
}

impl From<(u16, u16)> for Padding {
    fn from((horizontal, vertical): (u16, u16)) -> Self {
        Self::symmetric(horizontal, vertical)
    }
}

impl From<(u16, u16, u16, u16)> for Padding {
    fn from((left, right, top, bottom): (u16, u16, u16, u16)) -> Self {
        Self::new(left, right, top, bottom)
    }
}

/// Simple border decorator settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementBorder {
    pub title: Option<String>,
    pub footer: Option<String>,
    pub style: Style,
}

impl ElementBorder {
    pub fn new() -> Self {
        Self {
            title: None,
            footer: None,
            style: Style::default(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    fn block(&self) -> Block<'_> {
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.style);
        if let Some(title) = &self.title {
            block = block.title(format!(" {} ", title.trim()));
        }
        if let Some(footer) = &self.footer {
            block = block.title_bottom(footer.clone());
        }
        block
    }
}

impl Default for ElementBorder {
    fn default() -> Self {
        Self::new()
    }
}

impl From<()> for ElementBorder {
    fn from(_: ()) -> Self {
        Self::new()
    }
}

impl From<&str> for ElementBorder {
    fn from(value: &str) -> Self {
        Self::new().title(value)
    }
}

impl From<String> for ElementBorder {
    fn from(value: String) -> Self {
        Self::new().title(value)
    }
}

/// Leaf text element.
#[derive(Debug, Clone)]
pub struct Text {
    id: ComponentId,
    text: String,
    style: Style,
    wrap: bool,
    overflow: TextOverflow,
    dirty: DirtyState,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        Self::with_id("text", text)
    }

    pub fn with_id(id: impl Into<ComponentId>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            style: Style::default(),
            wrap: false,
            overflow: TextOverflow::Clip,
            dirty: DirtyState::paint(DirtyReason::Explicit),
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.mark_dirty(DirtyReason::DataUpdate);
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn wrap(mut self, enabled: bool) -> Self {
        self.wrap = enabled;
        self
    }

    pub fn truncate(mut self, enabled: bool) -> Self {
        self.overflow = if enabled {
            TextOverflow::Ellipsis
        } else {
            TextOverflow::Clip
        };
        self
    }

    pub fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }
}

impl BufferComponent for Text {
    type Event = KeyEvent;
    type Message = ();

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        buffer.set_style(area, self.style);
        render_text_lines(
            &self.text,
            self.wrap,
            self.overflow,
            self.style,
            area,
            buffer,
        );
        self.clear_dirty();
        Ok(())
    }

    fn dirty(&self) -> &DirtyState {
        &self.dirty
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark_paint(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
}

fn render_text_lines(
    text: &str,
    wrap: bool,
    overflow: TextOverflow,
    style: Style,
    area: Rect,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let width = usize::from(area.width);
    let mut row = 0;
    for line in text.lines() {
        if row >= area.height {
            break;
        }
        if wrap {
            let chars: Vec<char> = line.chars().collect();
            if chars.is_empty() {
                row += 1;
                continue;
            }
            for chunk in chars.chunks(width.max(1)) {
                if row >= area.height {
                    break;
                }
                let rendered: String = chunk.iter().collect();
                buffer.set_stringn(area.x, area.y.saturating_add(row), rendered, width, style);
                row += 1;
            }
        } else {
            let rendered = truncate_line(line, width, overflow);
            buffer.set_stringn(area.x, area.y.saturating_add(row), rendered, width, style);
            row += 1;
        }
    }
}

fn truncate_line(line: &str, width: usize, overflow: TextOverflow) -> String {
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= width {
        return line.to_string();
    }
    match overflow {
        TextOverflow::Clip => chars.into_iter().take(width).collect(),
        TextOverflow::Ellipsis if width >= 3 => {
            let mut out: String = chars.into_iter().take(width - 3).collect();
            out.push_str("...");
            out
        }
        TextOverflow::Ellipsis => ".".repeat(width),
    }
}

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

enum ChildElement<M> {
    Buffer(Box<dyn Element<Message = M>>),
    Effect(Box<dyn EffectElement<Message = M>>),
}

impl<M> ChildElement<M> {
    fn render(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        match self {
            Self::Buffer(element) => element.render_buffer(area, buffer),
            Self::Effect(element) => element.render_buffer(area, buffer),
        }
    }

    fn handle_key(&mut self, event: &KeyEvent) -> Result<ElementOutcome<M>> {
        match self {
            Self::Buffer(element) => element.handle_event(event),
            Self::Effect(element) => element.handle_event(event),
        }
    }

    fn clear_dirty(&mut self) {
        match self {
            Self::Buffer(element) => element.clear_dirty(),
            Self::Effect(element) => element.clear_dirty(),
        }
    }

    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        match self {
            Self::Buffer(_) => Ok(Vec::new()),
            Self::Effect(element) => element.render_effects(area),
        }
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
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

/// Window repaint strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowRepaintPolicy {
    Whole,
    ChildCached,
}

/// Inspectable window render counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WindowRenderStats {
    pub whole_repaints: u64,
    pub child_cache_hits: u64,
    pub child_cache_misses: u64,
    pub last_repaint_area: Option<Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowLifecycleEvent {
    Prefetch,
    Enter,
    Exit,
    Focus,
    Blur,
    Resize { old: Option<Rect>, new: Rect },
}

type WindowHook = Box<dyn FnMut(WindowLifecycleEvent)>;

#[derive(Default)]
struct WindowHooks {
    prefetch: Vec<WindowHook>,
    enter: Vec<WindowHook>,
    exit: Vec<WindowHook>,
    focus: Vec<WindowHook>,
    blur: Vec<WindowHook>,
    resize: Vec<WindowHook>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFocusScope {
    pub id: FocusId,
    pub kind: FocusScopeKind,
    pub nodes: Vec<FocusNode>,
}

/// Optional panel-like chrome for a behavioral window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowChrome {
    pub title: Option<String>,
    pub footer: Option<String>,
    pub border: bool,
    pub padding: Padding,
    pub border_style: Style,
}

impl WindowChrome {
    pub fn new() -> Self {
        Self {
            title: None,
            footer: None,
            border: false,
            padding: Padding::ZERO,
            border_style: Style::default(),
        }
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

    fn render(&self, area: Rect, buffer: &mut Buffer) {
        if self.border {
            self.block().render(area, buffer);
        }
    }

    fn child_area(&self, area: Rect) -> Rect {
        let area = if self.border {
            self.block().inner(area)
        } else {
            area
        };
        self.padding.inner(area)
    }
}

impl Default for WindowChrome {
    fn default() -> Self {
        Self::new()
    }
}

/// Behavioral lifecycle/key/effect boundary around a child element.
///
/// `active` means the window participates in its local key scope. `focused`
/// means key input is offered to the child before the window keymap. `entered`
/// means the enter lifecycle hook has fired and the matching exit hook has not.
///
/// `activate` fires prefetch, enter, then focus when needed. `deactivate`
/// fires blur before exit. The lower-level `focus`, `blur`, `enter`, and
/// `exit` methods are public so callers can model unusual focus transitions;
/// as a result, `focused && !active` is valid and routes child-first key input.
pub struct Window<E>
where
    E: Element,
    E::Message: Clone,
{
    id: ComponentId,
    child: E,
    child_ids: Vec<ComponentId>,
    dirty: DirtyState,
    repaint_policy: WindowRepaintPolicy,
    keymap: KeyMap<E::Message>,
    active: bool,
    focused: bool,
    entered: bool,
    focus_scope_kind: FocusScopeKind,
    focus_nodes: Vec<FocusNode>,
    chrome: WindowChrome,
    last_area: Option<Rect>,
    child_cache: Option<Buffer>,
    child_cache_area: Option<Rect>,
    stats: WindowRenderStats,
    effect_placements: BTreeSet<(u32, u32)>,
    hooks: WindowHooks,
}

impl<E> fmt::Debug for Window<E>
where
    E: Element,
    E::Message: Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Window")
            .field("id", &self.id)
            .field("child", self.child.id())
            .field("dirty", &self.dirty)
            .field("repaint_policy", &self.repaint_policy)
            .field("active", &self.active)
            .field("focused", &self.focused)
            .field("entered", &self.entered)
            .field("focus_scope_kind", &self.focus_scope_kind)
            .field("focus_nodes", &self.focus_nodes)
            .field("chrome", &self.chrome)
            .field("last_area", &self.last_area)
            .field("stats", &self.stats)
            .field("effect_placements", &self.effect_placements)
            .finish()
    }
}

impl<E> Window<E>
where
    E: Element,
    E::Message: Clone,
{
    pub fn new(id: impl Into<ComponentId>, child: E) -> Self {
        let child_ids = vec![child.id().clone()];
        Self {
            id: id.into(),
            child,
            child_ids,
            dirty: DirtyState::layout(DirtyReason::Explicit),
            repaint_policy: WindowRepaintPolicy::Whole,
            keymap: KeyMap::new(),
            active: false,
            focused: false,
            entered: false,
            focus_scope_kind: FocusScopeKind::Normal,
            focus_nodes: Vec::new(),
            chrome: WindowChrome::new(),
            last_area: None,
            child_cache: None,
            child_cache_area: None,
            stats: WindowRenderStats::default(),
            effect_placements: BTreeSet::new(),
            hooks: WindowHooks::default(),
        }
    }

    pub fn child(&self) -> &E {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut E {
        self.invalidate_child_cache();
        self.mark_dirty(DirtyReason::Explicit);
        &mut self.child
    }

    pub fn repaint_policy(&self) -> WindowRepaintPolicy {
        self.repaint_policy
    }

    pub fn with_repaint_policy(mut self, policy: WindowRepaintPolicy) -> Self {
        self.repaint_policy = policy;
        self
    }

    pub fn keymap(&self) -> &KeyMap<E::Message> {
        &self.keymap
    }

    pub fn keymap_mut(&mut self) -> &mut KeyMap<E::Message> {
        &mut self.keymap
    }

    /// Set the window-local keymap.
    ///
    /// This is the inherent `Window` method, not the [`ElementExt`] decorator.
    /// Bindings participate only while the window is active or focused.
    pub fn with_keymap(mut self, keymap: KeyMap<E::Message>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn with_focus_scope(
        mut self,
        kind: FocusScopeKind,
        nodes: impl IntoIterator<Item = FocusNode>,
    ) -> Self {
        self.focus_scope_kind = kind;
        self.focus_nodes = nodes.into_iter().collect();
        self
    }

    pub fn modal(mut self) -> Self {
        self.focus_scope_kind = FocusScopeKind::Modal;
        self
    }

    pub fn focus_scope(&self) -> WindowFocusScope {
        WindowFocusScope {
            id: FocusId::new(self.id.as_str().to_string()),
            kind: self.focus_scope_kind,
            nodes: self.focus_nodes.clone(),
        }
    }

    pub fn chrome(&self) -> &WindowChrome {
        &self.chrome
    }

    pub fn with_chrome(mut self, chrome: WindowChrome) -> Self {
        self.chrome = chrome;
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.chrome.title = Some(title.into());
        self.chrome.border = true;
        self
    }

    pub fn with_footer(mut self, footer: impl Into<String>) -> Self {
        self.chrome.footer = Some(footer.into());
        self.chrome.border = true;
        self
    }

    pub fn with_border(mut self, enabled: bool) -> Self {
        self.chrome.border = enabled;
        self
    }

    pub fn with_padding(mut self, padding: impl Into<Padding>) -> Self {
        self.chrome.padding = padding.into();
        self
    }

    pub fn child_area(&self, area: Rect) -> Rect {
        self.chrome.child_area(area)
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn has_entered(&self) -> bool {
        self.entered
    }

    pub fn activate(&mut self) {
        if !self.active {
            self.prefetch();
            self.active = true;
            self.enter();
        }
        self.focus();
    }

    pub fn deactivate(&mut self) {
        self.blur();
        if self.active {
            self.active = false;
            self.exit();
        }
    }

    pub fn enter(&mut self) {
        if !self.entered {
            self.entered = true;
            self.fire(WindowLifecycleEvent::Enter);
        }
    }

    pub fn exit(&mut self) {
        if self.entered {
            self.entered = false;
            self.fire(WindowLifecycleEvent::Exit);
        }
    }

    pub fn focus(&mut self) {
        if !self.focused {
            self.focused = true;
            self.fire(WindowLifecycleEvent::Focus);
        }
    }

    pub fn blur(&mut self) {
        if self.focused {
            self.focused = false;
            self.fire(WindowLifecycleEvent::Blur);
        }
    }

    pub fn prefetch(&mut self) {
        self.fire(WindowLifecycleEvent::Prefetch);
    }

    pub fn on_prefetch<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.prefetch.push(Box::new(hook));
        self
    }

    pub fn on_enter<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.enter.push(Box::new(hook));
        self
    }

    pub fn on_exit<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.exit.push(Box::new(hook));
        self
    }

    pub fn on_focus<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.focus.push(Box::new(hook));
        self
    }

    pub fn on_blur<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.blur.push(Box::new(hook));
        self
    }

    pub fn on_resize<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.resize.push(Box::new(hook));
        self
    }

    pub fn stats(&self) -> WindowRenderStats {
        self.stats
    }

    pub fn key_scope(&self, role: KeyScopeRole) -> KeyScope<E::Message> {
        KeyScope {
            id: self.id.clone(),
            role,
            keymap: self.keymap.clone(),
            active: self.active,
            focused: self.focused,
        }
    }

    /// Image placements currently owned by this window as `(image_id, placement_id)`.
    pub fn registered_effect_placements(&self) -> &BTreeSet<(u32, u32)> {
        &self.effect_placements
    }

    fn fire(&mut self, event: WindowLifecycleEvent) {
        let hooks = match event {
            WindowLifecycleEvent::Prefetch => &mut self.hooks.prefetch,
            WindowLifecycleEvent::Enter => &mut self.hooks.enter,
            WindowLifecycleEvent::Exit => &mut self.hooks.exit,
            WindowLifecycleEvent::Focus => &mut self.hooks.focus,
            WindowLifecycleEvent::Blur => &mut self.hooks.blur,
            WindowLifecycleEvent::Resize { .. } => &mut self.hooks.resize,
        };
        for hook in hooks {
            hook(event);
        }
    }

    fn record_resize(&mut self, area: Rect) {
        if self.last_area != Some(area) {
            let old = self.last_area;
            self.last_area = Some(area);
            self.invalidate_child_cache();
            self.dirty.mark_layout(DirtyReason::Resize);
            self.fire(WindowLifecycleEvent::Resize { old, new: area });
        }
    }

    fn invalidate_child_cache(&mut self) {
        self.child_cache = None;
        self.child_cache_area = None;
    }

    fn render_child_cached(&mut self, child_area: Rect, buffer: &mut Buffer) -> Result<()> {
        let needs_render = self.child_cache.is_none()
            || self.child_cache_area != Some(child_area)
            || !self.child.dirty().is_clean();
        if needs_render {
            let mut child_buffer = Buffer::empty(child_area);
            self.child.render_buffer(child_area, &mut child_buffer)?;
            self.child.clear_dirty();
            self.child_cache = Some(child_buffer);
            self.child_cache_area = Some(child_area);
            self.stats.child_cache_misses += 1;
        } else {
            self.stats.child_cache_hits += 1;
        }
        if let Some(child_buffer) = &self.child_cache {
            blit(child_buffer, buffer);
        }
        Ok(())
    }
}

impl<E> BufferComponent for Window<E>
where
    E: Element,
    E::Message: Clone,
{
    type Event = KeyEvent;
    type Message = E::Message;

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        self.record_resize(area);
        self.chrome.render(area, buffer);
        let child_area = self.child_area(area);
        match self.repaint_policy {
            WindowRepaintPolicy::Whole => {
                self.child.render_buffer(child_area, buffer)?;
                self.child.clear_dirty();
                self.stats.whole_repaints += 1;
                self.stats.last_repaint_area = Some(area);
            }
            WindowRepaintPolicy::ChildCached => {
                self.render_child_cached(child_area, buffer)?;
                self.stats.last_repaint_area = Some(child_area);
            }
        }
        self.clear_dirty();
        Ok(())
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        if !(self.active || self.focused) {
            return Ok(ComponentOutcome::Ignored);
        }
        // Focus controls child-first routing; active alone enables the
        // window-local keymap without handing input to the child.
        if self.focused {
            let child = self.child.handle_event(event)?;
            if child.is_handled() {
                return Ok(child);
            }
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
        if self.repaint_policy == WindowRepaintPolicy::ChildCached {
            self.invalidate_child_cache();
        }
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

impl<E> ContainerElement for Window<E>
where
    E: Element,
    E::Message: Clone,
{
}

impl<E> EffectElement for Window<E>
where
    E: EffectElement,
    E::Message: Clone,
{
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        let child_area = self.child_area(area);
        let effects = self.child.render_effects(child_area)?;
        for effect in &effects {
            match effect {
                RenderEffect::PlaceImage { options, .. } => {
                    self.effect_placements
                        .insert((options.image_id, options.placement_id));
                }
                RenderEffect::DeleteImagePlacement {
                    image_id,
                    placement_id,
                } => {
                    self.effect_placements.remove(&(*image_id, *placement_id));
                }
                RenderEffect::DeletePlacement { placement_id } => {
                    self.effect_placements
                        .retain(|(_, registered)| registered != placement_id);
                }
                RenderEffect::DeleteAllPlacements | RenderEffect::ForgetAllImages => {
                    self.effect_placements.clear();
                }
                RenderEffect::EnsureImageLoaded { .. } | RenderEffect::FlushImages => {}
            }
        }
        Ok(effects)
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        let mut effects = self.child.teardown_effects()?;
        let already_deleted_images: BTreeSet<(u32, u32)> = effects
            .iter()
            .filter_map(|effect| match effect {
                RenderEffect::DeleteImagePlacement {
                    image_id,
                    placement_id,
                } => Some((*image_id, *placement_id)),
                _ => None,
            })
            .collect();
        let already_deleted_placements: BTreeSet<u32> = effects
            .iter()
            .filter_map(|effect| match effect {
                RenderEffect::DeletePlacement { placement_id } => Some(*placement_id),
                _ => None,
            })
            .collect();
        let all_deleted = effects.iter().any(|effect| {
            matches!(
                effect,
                RenderEffect::DeleteAllPlacements | RenderEffect::ForgetAllImages
            )
        });
        for (image_id, placement_id) in std::mem::take(&mut self.effect_placements) {
            if !all_deleted
                && !already_deleted_images.contains(&(image_id, placement_id))
                && !already_deleted_placements.contains(&placement_id)
            {
                effects.push(RenderEffect::DeleteImagePlacement {
                    image_id,
                    placement_id,
                });
            }
        }
        Ok(effects)
    }
}

fn blit(source: &Buffer, target: &mut Buffer) {
    let area = *source.area();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let (Some(src), Some(dst)) = (source.cell((x, y)), target.cell_mut((x, y))) {
                *dst = src.clone();
            }
        }
    }
}

/// Declarative key scope role used by [`KeyScopeResolver`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyScopeRole {
    Root,
    ModalWindow,
    FocusedChild,
    Window,
}

#[derive(Debug, Clone)]
pub struct KeyScope<C: Clone> {
    pub id: ComponentId,
    pub role: KeyScopeRole,
    pub keymap: KeyMap<C>,
    pub active: bool,
    pub focused: bool,
}

impl<C: Clone> KeyScope<C> {
    pub fn new(id: impl Into<ComponentId>, role: KeyScopeRole, keymap: KeyMap<C>) -> Self {
        Self {
            id: id.into(),
            role,
            keymap,
            active: true,
            focused: false,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyResolution<C> {
    pub scope_id: ComponentId,
    pub role: KeyScopeRole,
    pub command: C,
}

#[derive(Debug, Clone)]
pub struct KeyScopeResolver<C: Clone> {
    root: KeyMap<C>,
    scopes: Vec<KeyScope<C>>,
}

impl<C: Clone> KeyScopeResolver<C> {
    pub fn new(root: KeyMap<C>) -> Self {
        Self {
            root,
            scopes: Vec::new(),
        }
    }

    pub fn push_scope(&mut self, scope: KeyScope<C>) -> &mut Self {
        self.scopes.push(scope);
        self
    }

    pub fn with_scope(mut self, scope: KeyScope<C>) -> Self {
        self.push_scope(scope);
        self
    }

    pub fn resolve(&self, key: KeyEvent) -> Option<KeyResolution<C>> {
        self.resolve_role(key, KeyScopeRole::ModalWindow, |scope| scope.active)
            .or_else(|| {
                self.resolve_role(key, KeyScopeRole::FocusedChild, |scope| {
                    scope.active && scope.focused
                })
            })
            .or_else(|| self.resolve_role(key, KeyScopeRole::Window, |scope| scope.active))
            .or_else(|| {
                self.root.lookup(key).map(|command| KeyResolution {
                    scope_id: ComponentId::new("root"),
                    role: KeyScopeRole::Root,
                    command,
                })
            })
    }

    fn resolve_role(
        &self,
        key: KeyEvent,
        role: KeyScopeRole,
        eligible: impl Fn(&KeyScope<C>) -> bool,
    ) -> Option<KeyResolution<C>> {
        self.scopes
            .iter()
            .rev()
            .find(|scope| scope.role == role && eligible(scope))
            .and_then(|scope| {
                scope.keymap.lookup(key).map(|command| KeyResolution {
                    scope_id: scope.id.clone(),
                    role: scope.role,
                    command,
                })
            })
    }
}

/// Effect-aware wrapper for [`ImageViewportWidget`].
#[derive(Debug, Clone)]
pub struct ImageViewportElement {
    id: ComponentId,
    widget: ImageViewportWidget,
    image_id: u32,
    placement_id: u32,
    png: Option<Arc<[u8]>>,
    dirty: DirtyState,
}

impl ImageViewportElement {
    pub fn new(
        id: impl Into<ComponentId>,
        image_id: u32,
        placement_id: u32,
        widget: ImageViewportWidget,
    ) -> Self {
        Self {
            id: id.into(),
            widget,
            image_id,
            placement_id,
            png: None,
            dirty: DirtyState::image_placement(DirtyReason::ImagePlacement),
        }
    }

    pub fn with_png(mut self, png: impl Into<Arc<[u8]>>) -> Self {
        self.png = Some(png.into());
        self
    }

    pub fn widget(&self) -> &ImageViewportWidget {
        &self.widget
    }

    pub fn widget_mut(&mut self) -> &mut ImageViewportWidget {
        self.mark_dirty(DirtyReason::ImagePlacement);
        &mut self.widget
    }

    pub fn image_id(&self) -> u32 {
        self.image_id
    }

    pub fn placement_id(&self) -> u32 {
        self.placement_id
    }

    pub fn update_canvas_for_area(&mut self, area: Rect, cell_pixel: CellPixel) {
        self.widget.update_canvas(CanvasMetrics::new(
            CellSize::new(area.width, area.height),
            cell_pixel,
        ));
        self.mark_dirty(DirtyReason::Resize);
    }
}

impl BufferComponent for ImageViewportElement {
    type Event = KeyEvent;
    type Message = ();

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, _area: Rect, _buffer: &mut Buffer) -> Result<()> {
        Ok(())
    }

    fn dirty(&self) -> &DirtyState {
        &self.dirty
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark_image_placement(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
    }
}

impl EffectElement for ImageViewportElement {
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        let canvas = self.widget.canvas();
        let area_cells = CellSize::new(area.width, area.height);
        if canvas.cells != area_cells {
            self.widget
                .update_canvas(CanvasMetrics::new(area_cells, canvas.cell_pixel));
        }
        let Some(placement) = self.widget.placement()? else {
            return Ok(vec![RenderEffect::DeleteImagePlacement {
                image_id: self.image_id,
                placement_id: self.placement_id,
            }]);
        };
        let mut effects = Vec::new();
        if let Some(png) = &self.png {
            effects.push(RenderEffect::EnsureImageLoaded {
                image_id: self.image_id,
                png: png.clone(),
            });
        }
        effects.push(RenderEffect::PlaceImage {
            origin: CellOffset {
                col: area.x.saturating_add(placement.origin.col),
                row: area.y.saturating_add(placement.origin.row),
            },
            options: placement.place_options(self.image_id, self.placement_id),
        });
        self.clear_dirty();
        Ok(effects)
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        Ok(vec![RenderEffect::DeleteImagePlacement {
            image_id: self.image_id,
            placement_id: self.placement_id,
        }])
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::PlaceOptions;
    use crate::keymap::{KeyTrigger, SpecialKey};
    use crate::layout::{PixelRect, PixelSize};
    use crate::testkit::{assert_teardown_covers, find_place_with_placement_id, render_to_buffer};
    use crate::widgets::image_viewport::{ImageViewport, ViewportImage};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum TestCommand {
        Window,
        Child,
        Modal,
        Base,
        Top,
    }

    type SharedAreas = std::rc::Rc<std::cell::RefCell<Vec<Rect>>>;
    type SharedDirtyEvents = std::rc::Rc<std::cell::RefCell<Vec<DirtyReason>>>;
    type SharedTeardowns = std::rc::Rc<std::cell::RefCell<u32>>;

    #[derive(Debug, Clone)]
    struct ProbeElement {
        id: ComponentId,
        marker: &'static str,
        keymap: KeyMap<TestCommand>,
        dirty: DirtyState,
    }

    impl ProbeElement {
        fn new(id: &'static str, marker: &'static str) -> Self {
            Self {
                id: ComponentId::new(id),
                marker,
                keymap: KeyMap::new(),
                dirty: DirtyState::paint(DirtyReason::Explicit),
            }
        }

        fn with_binding(mut self, trigger: char, command: TestCommand) -> Self {
            self.keymap.bind(KeyTrigger::Char(trigger), command);
            self
        }
    }

    impl BufferComponent for ProbeElement {
        type Event = KeyEvent;
        type Message = TestCommand;

        fn id(&self) -> &ComponentId {
            &self.id
        }

        fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
            buffer.set_stringn(
                area.x,
                area.y,
                self.marker,
                usize::from(area.width),
                Style::default(),
            );
            self.clear_dirty();
            Ok(())
        }

        fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
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
            self.dirty.mark_paint(reason);
        }

        fn clear_dirty(&mut self) {
            self.dirty.clear();
        }
    }

    #[derive(Debug, Clone)]
    struct AreaProbeElement {
        id: ComponentId,
        marker: &'static str,
        areas: SharedAreas,
        dirty_events: SharedDirtyEvents,
        keymap: KeyMap<TestCommand>,
        dirty: DirtyState,
    }

    impl AreaProbeElement {
        fn new(id: &'static str, marker: &'static str) -> (Self, SharedAreas, SharedDirtyEvents) {
            let areas = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let dirty_events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            (
                Self {
                    id: ComponentId::new(id),
                    marker,
                    areas: std::rc::Rc::clone(&areas),
                    dirty_events: std::rc::Rc::clone(&dirty_events),
                    keymap: KeyMap::new(),
                    dirty: DirtyState::paint(DirtyReason::Explicit),
                },
                areas,
                dirty_events,
            )
        }
    }

    impl BufferComponent for AreaProbeElement {
        type Event = KeyEvent;
        type Message = TestCommand;

        fn id(&self) -> &ComponentId {
            &self.id
        }

        fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
            self.areas.borrow_mut().push(area);
            if area.width > 0 && area.height > 0 {
                buffer.set_stringn(
                    area.x,
                    area.y,
                    self.marker,
                    usize::from(area.width),
                    Style::default(),
                );
            }
            self.clear_dirty();
            Ok(())
        }

        fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
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
            self.dirty_events.borrow_mut().push(reason.clone());
            self.dirty.mark_paint(reason);
        }

        fn clear_dirty(&mut self) {
            self.dirty.clear();
        }
    }

    #[derive(Debug, Clone)]
    struct EffectProbeElement {
        id: ComponentId,
        placement_id: u32,
        areas: SharedAreas,
        teardowns: SharedTeardowns,
        dirty: DirtyState,
    }

    impl EffectProbeElement {
        fn new(id: &'static str) -> (Self, SharedAreas, SharedTeardowns) {
            let areas = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let teardowns = std::rc::Rc::new(std::cell::RefCell::new(0));
            (
                Self {
                    id: ComponentId::new(id),
                    placement_id: 42,
                    areas: std::rc::Rc::clone(&areas),
                    teardowns: std::rc::Rc::clone(&teardowns),
                    dirty: DirtyState::paint(DirtyReason::Explicit),
                },
                areas,
                teardowns,
            )
        }

        fn with_placement_id(mut self, placement_id: u32) -> Self {
            self.placement_id = placement_id;
            self
        }
    }

    impl BufferComponent for EffectProbeElement {
        type Event = KeyEvent;
        type Message = TestCommand;

        fn id(&self) -> &ComponentId {
            &self.id
        }

        fn render_buffer(&mut self, _area: Rect, _buffer: &mut Buffer) -> Result<()> {
            self.clear_dirty();
            Ok(())
        }

        fn dirty(&self) -> &DirtyState {
            &self.dirty
        }

        fn mark_dirty(&mut self, reason: DirtyReason) {
            self.dirty.mark_image_placement(reason);
        }

        fn clear_dirty(&mut self) {
            self.dirty.clear();
        }
    }

    impl EffectElement for EffectProbeElement {
        fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
            self.areas.borrow_mut().push(area);
            Ok(vec![RenderEffect::PlaceImage {
                origin: CellOffset {
                    col: area.x,
                    row: area.y,
                },
                options: PlaceOptions {
                    image_id: 1,
                    placement_id: self.placement_id,
                    source: PixelRect {
                        x: 0,
                        y: 0,
                        width: 8,
                        height: 8,
                    },
                    cell_cols: area.width,
                    cell_rows: area.height,
                },
            }])
        }

        fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
            *self.teardowns.borrow_mut() += 1;
            Ok(vec![RenderEffect::DeletePlacement {
                placement_id: self.placement_id,
            }])
        }
    }

    #[derive(Debug, Clone)]
    struct ToggleEffectProbeElement {
        id: ComponentId,
        delete_next: std::rc::Rc<std::cell::Cell<bool>>,
        dirty: DirtyState,
    }

    impl ToggleEffectProbeElement {
        fn new(id: &'static str) -> (Self, std::rc::Rc<std::cell::Cell<bool>>) {
            let delete_next = std::rc::Rc::new(std::cell::Cell::new(false));
            (
                Self {
                    id: ComponentId::new(id),
                    delete_next: std::rc::Rc::clone(&delete_next),
                    dirty: DirtyState::paint(DirtyReason::Explicit),
                },
                delete_next,
            )
        }
    }

    impl BufferComponent for ToggleEffectProbeElement {
        type Event = KeyEvent;
        type Message = TestCommand;

        fn id(&self) -> &ComponentId {
            &self.id
        }

        fn render_buffer(&mut self, _area: Rect, _buffer: &mut Buffer) -> Result<()> {
            self.clear_dirty();
            Ok(())
        }

        fn dirty(&self) -> &DirtyState {
            &self.dirty
        }

        fn mark_dirty(&mut self, reason: DirtyReason) {
            self.dirty.mark_image_placement(reason);
        }

        fn clear_dirty(&mut self) {
            self.dirty.clear();
        }
    }

    impl EffectElement for ToggleEffectProbeElement {
        fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
            if self.delete_next.get() {
                return Ok(vec![RenderEffect::DeleteImagePlacement {
                    image_id: 3,
                    placement_id: 4,
                }]);
            }
            Ok(vec![RenderEffect::PlaceImage {
                origin: CellOffset {
                    col: area.x,
                    row: area.y,
                },
                options: PlaceOptions {
                    image_id: 3,
                    placement_id: 4,
                    source: PixelRect {
                        x: 0,
                        y: 0,
                        width: 8,
                        height: 8,
                    },
                    cell_cols: area.width,
                    cell_rows: area.height,
                },
            }])
        }
    }

    fn command_map(trigger: char, command: TestCommand) -> KeyMap<TestCommand> {
        let mut keymap = KeyMap::new();
        keymap.bind(KeyTrigger::Char(trigger), command);
        keymap
    }

    #[test]
    fn text_wraps_and_truncates() -> Result<()> {
        let mut text = Text::with_id("body", "abcdef").truncate(true);
        let buffer = render_to_buffer(&mut text, Rect::new(0, 0, 4, 1))?;

        assert!(format!("{buffer:?}").contains("a..."));
        assert!(text.dirty().is_clean());
        Ok(())
    }

    #[test]
    fn panel_calculates_child_area_from_border_and_padding() {
        let panel = Panel::new("panel", Text::new("child")).padding((2, 1));

        assert_eq!(
            panel.child_area(Rect::new(0, 0, 20, 8)),
            Rect::new(3, 2, 14, 4)
        );
    }

    #[test]
    fn stack_distributes_fill_space() {
        let stack: Stack<()> = Stack::vertical("stack")
            .with_child(Text::with_id("a", "a"), StackConstraint::Length(1))
            .with_child(Text::with_id("b", "b"), StackConstraint::Fill(1))
            .with_child(Text::with_id("c", "c"), StackConstraint::Fill(1));

        let areas = stack.layout_areas(Rect::new(0, 0, 10, 5));

        assert_eq!(areas[0].height, 1);
        assert_eq!(areas[1].height, 2);
        assert_eq!(areas[2].height, 2);
    }

    #[test]
    fn stack_min_constraints_reserve_then_grow_with_remaining_space() {
        let stack: Stack<()> = Stack::vertical("stack")
            .with_child(Text::with_id("a", "a"), StackConstraint::Min(1))
            .with_child(Text::with_id("b", "b"), StackConstraint::Fill(1));

        let areas = stack.layout_areas(Rect::new(0, 0, 10, 5));

        assert_eq!(areas[0].height, 3);
        assert_eq!(areas[1].height, 2);
    }

    #[test]
    fn stack_forwards_effect_children_through_layout_areas() -> Result<()> {
        let (effect, effect_areas, teardowns) = EffectProbeElement::new("effect");
        let mut stack = Stack::vertical("stack")
            .with_child(
                ProbeElement::new("buffer", "buffer"),
                StackConstraint::Length(1),
            )
            .with_effect_child(effect, StackConstraint::Length(2));

        let effects = stack.render_effects(Rect::new(3, 4, 10, 6))?;

        assert_eq!(effect_areas.borrow().as_slice(), &[Rect::new(3, 5, 10, 2)]);
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            &effects[0],
            RenderEffect::PlaceImage { origin, options }
                if *origin == CellOffset { col: 3, row: 5 }
                    && options.cell_cols == 10
                    && options.cell_rows == 2
        ));
        assert_eq!(
            stack.teardown_effects()?,
            vec![RenderEffect::DeletePlacement { placement_id: 42 }]
        );
        assert_eq!(*teardowns.borrow(), 1);
        Ok(())
    }

    #[test]
    fn stack_tears_down_effect_children_in_reverse_order() -> Result<()> {
        let (first, _first_areas, _first_teardowns) = EffectProbeElement::new("first");
        let (second, _second_areas, _second_teardowns) = EffectProbeElement::new("second");
        let mut stack: Stack<TestCommand> = Stack::horizontal("stack")
            .with_effect_child(first.with_placement_id(1), StackConstraint::Length(1))
            .with_effect_child(second.with_placement_id(2), StackConstraint::Length(1));

        assert_eq!(
            stack.teardown_effects()?,
            vec![
                RenderEffect::DeletePlacement { placement_id: 2 },
                RenderEffect::DeletePlacement { placement_id: 1 },
            ]
        );
        Ok(())
    }

    #[test]
    fn stack_teardown_covers_every_rendered_placement() -> Result<()> {
        let (first, _, _) = EffectProbeElement::new("first");
        let (second, _, _) = EffectProbeElement::new("second");
        let mut stack: Stack<TestCommand> = Stack::horizontal("stack")
            .with_effect_child(first.with_placement_id(11), StackConstraint::Length(2))
            .with_effect_child(second.with_placement_id(22), StackConstraint::Length(2));

        let placed = stack.render_effects(Rect::new(0, 0, 4, 4))?;
        let teardown = stack.teardown_effects()?;

        assert_teardown_covers(&placed, &teardown);
        Ok(())
    }

    #[test]
    fn scroll_y_offsets_buffer_output_and_marks_child_dirty() -> Result<()> {
        let mut element = Text::with_id("body", "one\ntwo\nthree").scroll_y();
        element.scroll_to(1);
        let buffer = render_to_buffer(&mut element, Rect::new(0, 0, 8, 2))?;

        let rendered = format!("{buffer:?}");
        assert!(rendered.contains("two"));
        assert!(rendered.contains("three"));
        assert!(!rendered.contains("one"));
        assert!(element.dirty().is_clean());
        Ok(())
    }

    #[test]
    fn scroll_y_forwards_key_events_to_child() -> Result<()> {
        let mut element = ProbeElement::new("probe", "probe")
            .with_binding('x', TestCommand::Child)
            .scroll_y();

        assert_eq!(
            element.handle_event(&KeyEvent::Char('x'))?,
            ComponentOutcome::Message(TestCommand::Child)
        );
        Ok(())
    }

    #[test]
    fn scroll_y_dirty_propagates_to_child() {
        let (child, _areas, dirty_events) = AreaProbeElement::new("child", "child");
        let mut element = child.scroll_y();

        element.scroll_by(2);

        assert!(element.dirty().paint_dirty());
        assert_eq!(dirty_events.borrow().as_slice(), &[DirtyReason::Input]);
    }

    #[test]
    fn focusable_exposes_stable_focus_node_and_forwards_child() -> Result<()> {
        let mut element = ProbeElement::new("probe", "probe")
            .with_binding('x', TestCommand::Child)
            .focusable()
            .with_focus_id("field")
            .enabled(false)
            .visible(true);

        let node = element.focus_node().expect("focus node");
        assert_eq!(node.id.as_str(), "field");
        assert!(!node.focusable());
        assert_eq!(
            element.handle_event(&KeyEvent::Char('x'))?,
            ComponentOutcome::Message(TestCommand::Child)
        );
        Ok(())
    }

    #[test]
    fn focusable_node_flows_through_window() {
        let window = Window::new(
            "window",
            Text::with_id("body", "body")
                .focusable()
                .with_focus_id("body-focus"),
        );

        let node = window.focus_node().expect("focus node");
        assert_eq!(node.id.as_str(), "body-focus");
    }

    #[test]
    fn keymap_decorator_uses_child_first_then_local_binding() -> Result<()> {
        let mut local = KeyMap::new();
        local.bind(KeyTrigger::Char('x'), TestCommand::Window);
        local.bind(KeyTrigger::Char('y'), TestCommand::Window);
        let mut element = ProbeElement::new("probe", "probe")
            .with_binding('x', TestCommand::Child)
            .with_keymap(local);

        assert_eq!(
            element.handle_event(&KeyEvent::Char('x'))?,
            ComponentOutcome::Message(TestCommand::Child)
        );
        assert_eq!(
            element.handle_event(&KeyEvent::Char('y'))?,
            ComponentOutcome::Message(TestCommand::Window)
        );
        assert_eq!(
            element.handle_event(&KeyEvent::Char('z'))?,
            ComponentOutcome::Ignored
        );
        Ok(())
    }

    #[test]
    fn padded_decorator_calculates_child_area_and_marks_layout_dirty() -> Result<()> {
        let (child, areas, dirty_events) = AreaProbeElement::new("child", "child");
        let mut element = child.with_padding((2, 1));
        let area = Rect::new(0, 0, 10, 5);
        let mut buffer = Buffer::empty(area);

        assert_eq!(element.child_area(area), Rect::new(2, 1, 6, 3));
        element.mark_dirty(DirtyReason::Resize);
        element.render_buffer(area, &mut buffer)?;

        assert_eq!(areas.borrow().as_slice(), &[Rect::new(2, 1, 6, 3)]);
        assert_eq!(dirty_events.borrow().as_slice(), &[DirtyReason::Resize]);
        assert!(element.dirty().is_clean());
        Ok(())
    }

    #[test]
    fn bordered_decorator_renders_border_and_calculates_child_area() -> Result<()> {
        let (child, areas, _dirty_events) = AreaProbeElement::new("child", "child");
        let mut element = child.with_border(ElementBorder::new().title("Title"));
        let area = Rect::new(0, 0, 12, 3);

        assert_eq!(element.child_area(area), Rect::new(1, 1, 10, 1));
        let buffer = render_to_buffer(&mut element, area)?;

        let rendered = format!("{buffer:?}");
        assert!(rendered.contains("Title"));
        assert_eq!(areas.borrow().as_slice(), &[Rect::new(1, 1, 10, 1)]);
        Ok(())
    }

    #[test]
    fn nested_padding_and_border_apply_in_wrapper_order() -> Result<()> {
        let (child, areas, _dirty_events) = AreaProbeElement::new("child", "child");
        let mut element = child.with_padding(1).with_border(());
        let _ = render_to_buffer(&mut element, Rect::new(0, 0, 10, 5))?;

        assert_eq!(areas.borrow().as_slice(), &[Rect::new(2, 2, 6, 1)]);
        Ok(())
    }

    #[test]
    fn padding_and_border_decorators_handle_zero_sized_area() -> Result<()> {
        let (child, areas, _dirty_events) = AreaProbeElement::new("child", "child");
        let mut element = child.with_padding(2).with_border(());
        let area = Rect::new(0, 0, 0, 0);
        let mut buffer = Buffer::empty(area);

        element.render_buffer(area, &mut buffer)?;

        assert_eq!(areas.borrow().as_slice(), &[Rect::new(0, 0, 0, 0)]);
        Ok(())
    }

    #[test]
    fn padding_and_border_forward_effects_to_transformed_child_area() -> Result<()> {
        let (child, areas, teardowns) = EffectProbeElement::new("effect");
        let mut element = child.with_padding(1).with_border(());

        let effects = element.render_effects(Rect::new(10, 5, 10, 5))?;

        assert_eq!(areas.borrow().as_slice(), &[Rect::new(12, 7, 6, 1)]);
        assert!(matches!(
            &effects[0],
            RenderEffect::PlaceImage { origin, options }
                if *origin == CellOffset { col: 12, row: 7 }
                    && options.cell_cols == 6
                    && options.cell_rows == 1
        ));
        assert_eq!(
            element.teardown_effects()?,
            vec![RenderEffect::DeletePlacement { placement_id: 42 }]
        );
        assert_eq!(*teardowns.borrow(), 1);
        Ok(())
    }

    #[test]
    fn focusable_and_keymapped_forward_effects_without_area_change() -> Result<()> {
        let (child, areas, _teardowns) = EffectProbeElement::new("effect");
        let mut element = child.focusable().with_keymap(KeyMap::new());

        let effects = element.render_effects(Rect::new(3, 4, 5, 6))?;

        assert_eq!(areas.borrow().as_slice(), &[Rect::new(3, 4, 5, 6)]);
        assert!(matches!(
            &effects[0],
            RenderEffect::PlaceImage { origin, options }
                if *origin == CellOffset { col: 3, row: 4 }
                    && options.cell_cols == 5
                    && options.cell_rows == 6
        ));
        Ok(())
    }

    #[test]
    fn panel_forwards_effects_to_presentational_child_area() -> Result<()> {
        let (child, areas, _teardowns) = EffectProbeElement::new("effect");
        let mut panel = Panel::new("panel", child).padding(1);

        let effects = panel.render_effects(Rect::new(0, 0, 10, 5))?;

        assert_eq!(areas.borrow().as_slice(), &[Rect::new(2, 2, 6, 1)]);
        assert!(matches!(
            &effects[0],
            RenderEffect::PlaceImage { origin, options }
                if *origin == CellOffset { col: 2, row: 2 }
                    && options.cell_cols == 6
                    && options.cell_rows == 1
        ));
        Ok(())
    }

    #[test]
    fn modal_forwards_window_effects_and_grouped_teardown() -> Result<()> {
        let (child, areas, teardowns) = EffectProbeElement::new("effect");
        let mut modal = Modal::new("modal", child);

        let effects = modal.render_effects(Rect::new(2, 3, 4, 5))?;

        assert_eq!(areas.borrow().as_slice(), &[Rect::new(2, 3, 4, 5)]);
        assert!(matches!(
            &effects[0],
            RenderEffect::PlaceImage { origin, options }
                if *origin == CellOffset { col: 2, row: 3 }
                    && options.placement_id == 42
        ));
        assert!(modal
            .window()
            .registered_effect_placements()
            .contains(&(1, 42)));

        assert_eq!(
            modal.teardown_effects()?,
            vec![RenderEffect::DeletePlacement { placement_id: 42 }]
        );
        assert_eq!(*teardowns.borrow(), 1);
        assert!(modal.window().registered_effect_placements().is_empty());
        Ok(())
    }

    #[test]
    fn overlay_forwards_effect_layers_using_layer_areas() -> Result<()> {
        let (effect, areas, _teardowns) = EffectProbeElement::new("effect");
        let mut overlay = Overlay::new("overlay")
            .with_layer(ProbeElement::new("buffer", "buffer"), None)
            .with_effect_layer(effect, Some(Rect::new(5, 6, 7, 8)));

        let effects = overlay.render_effects(Rect::new(0, 0, 20, 10))?;

        assert_eq!(areas.borrow().as_slice(), &[Rect::new(5, 6, 7, 8)]);
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            &effects[0],
            RenderEffect::PlaceImage { origin, options }
                if *origin == CellOffset { col: 5, row: 6 }
                    && options.cell_cols == 7
                    && options.cell_rows == 8
        ));
        Ok(())
    }

    #[test]
    fn overlay_tears_down_effect_layers_top_down() -> Result<()> {
        let (base, _base_areas, _base_teardowns) = EffectProbeElement::new("base");
        let (top, _top_areas, _top_teardowns) = EffectProbeElement::new("top");
        let mut overlay: Overlay<TestCommand> = Overlay::new("overlay")
            .with_effect_layer(base.with_placement_id(1), None)
            .with_modal_effect_layer(top.with_placement_id(2), None);

        assert_eq!(
            overlay.teardown_effects()?,
            vec![
                RenderEffect::DeletePlacement { placement_id: 2 },
                RenderEffect::DeletePlacement { placement_id: 1 },
            ]
        );
        Ok(())
    }

    #[test]
    fn window_hooks_fire_in_activation_order() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let events = Rc::new(RefCell::new(Vec::new()));
        let mut window = Window::new("window", Text::new("body"))
            .on_prefetch({
                let events = Rc::clone(&events);
                move |event| events.borrow_mut().push(event)
            })
            .on_enter({
                let events = Rc::clone(&events);
                move |event| events.borrow_mut().push(event)
            })
            .on_focus({
                let events = Rc::clone(&events);
                move |event| events.borrow_mut().push(event)
            });

        window.activate();

        assert_eq!(
            *events.borrow(),
            vec![
                WindowLifecycleEvent::Prefetch,
                WindowLifecycleEvent::Enter,
                WindowLifecycleEvent::Focus,
            ]
        );
    }

    #[test]
    fn window_deactivation_blurs_before_exit() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let events = Rc::new(RefCell::new(Vec::new()));
        let mut window = Window::new("window", Text::new("body"))
            .on_blur({
                let events = Rc::clone(&events);
                move |event| events.borrow_mut().push(event)
            })
            .on_exit({
                let events = Rc::clone(&events);
                move |event| events.borrow_mut().push(event)
            });

        window.activate();
        window.deactivate();

        assert_eq!(
            *events.borrow(),
            vec![WindowLifecycleEvent::Blur, WindowLifecycleEvent::Exit]
        );
        assert!(!window.is_active());
        assert!(!window.is_focused());
        assert!(!window.has_entered());
    }

    #[test]
    fn window_resize_hook_fires_only_when_area_changes() -> Result<()> {
        use std::cell::RefCell;
        use std::rc::Rc;

        let first = Rect::new(0, 0, 10, 1);
        let second = Rect::new(0, 0, 12, 1);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut window = Window::new("window", Text::new("body")).on_resize({
            let events = Rc::clone(&events);
            move |event| events.borrow_mut().push(event)
        });
        let mut buffer = Buffer::empty(second);

        window.render_buffer(first, &mut buffer)?;
        window.render_buffer(first, &mut buffer)?;
        window.render_buffer(second, &mut buffer)?;

        assert_eq!(
            *events.borrow(),
            vec![
                WindowLifecycleEvent::Resize {
                    old: None,
                    new: first,
                },
                WindowLifecycleEvent::Resize {
                    old: Some(first),
                    new: second,
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn window_key_scope_participates_only_while_active_or_focused() -> Result<()> {
        let child = ProbeElement::new("child", "child").with_binding('x', TestCommand::Child);
        let mut window =
            Window::new("window", child).with_keymap(command_map('x', TestCommand::Window));

        assert_eq!(
            window.handle_event(&KeyEvent::Char('x'))?,
            ComponentOutcome::Ignored
        );

        window.activate();
        assert_eq!(
            window.handle_event(&KeyEvent::Char('x'))?,
            ComponentOutcome::Message(TestCommand::Child)
        );

        window.blur();
        assert_eq!(
            window.handle_event(&KeyEvent::Char('x'))?,
            ComponentOutcome::Message(TestCommand::Window)
        );

        window.deactivate();
        assert_eq!(
            window.handle_event(&KeyEvent::Char('x'))?,
            ComponentOutcome::Ignored
        );
        Ok(())
    }

    #[test]
    fn window_focus_scope_metadata_is_declarative() {
        let window = Window::new("window", Text::new("body")).with_focus_scope(
            FocusScopeKind::Capturing,
            [FocusNode::new("field"), FocusNode::new("button")],
        );

        let scope = window.focus_scope();
        assert_eq!(scope.id.as_str(), "window");
        assert_eq!(scope.kind, FocusScopeKind::Capturing);
        assert_eq!(scope.nodes.len(), 2);
        assert_eq!(scope.nodes[0].id.as_str(), "field");
    }

    #[test]
    fn window_groups_effect_teardown_without_duplicate_child_teardown() -> Result<()> {
        let image = ViewportImage::new(PixelSize::new(2, 2), vec![255; 16])?;
        let widget = ImageViewportWidget::new(
            ImageViewport::new(image),
            CanvasMetrics::new(CellSize::new(4, 2), CellPixel::new(1, 1)),
        );
        let image = ImageViewportElement::new("image", 7, 9, widget).with_png(b"png".to_vec());
        let mut window = Window::new("window", image);

        let effects = window.render_effects(Rect::new(0, 0, 4, 2))?;
        assert!(find_place_with_placement_id(&effects, 9).is_some());
        assert!(window.registered_effect_placements().contains(&(7, 9)));

        let teardown = window.teardown_effects()?;
        assert_eq!(
            teardown,
            vec![RenderEffect::DeleteImagePlacement {
                image_id: 7,
                placement_id: 9,
            }]
        );
        assert!(window.registered_effect_placements().is_empty());
        Ok(())
    }

    #[test]
    fn window_prunes_effect_tracking_when_child_deletes_during_render() -> Result<()> {
        let (child, delete_next) = ToggleEffectProbeElement::new("effect");
        let mut window = Window::new("window", child);

        window.render_effects(Rect::new(0, 0, 4, 2))?;
        assert!(window.registered_effect_placements().contains(&(3, 4)));

        delete_next.set(true);
        let effects = window.render_effects(Rect::new(0, 0, 4, 2))?;

        assert_eq!(
            effects,
            vec![RenderEffect::DeleteImagePlacement {
                image_id: 3,
                placement_id: 4,
            }]
        );
        assert!(window.registered_effect_placements().is_empty());
        assert!(window.teardown_effects()?.is_empty());
        Ok(())
    }

    #[test]
    fn window_fallback_teardown_uses_image_placement_pair() -> Result<()> {
        let (child, _delete_next) = ToggleEffectProbeElement::new("effect");
        let mut window = Window::new("window", child);

        window.render_effects(Rect::new(0, 0, 4, 2))?;

        assert_eq!(
            window.teardown_effects()?,
            vec![RenderEffect::DeleteImagePlacement {
                image_id: 3,
                placement_id: 4,
            }]
        );
        assert!(window.registered_effect_placements().is_empty());
        Ok(())
    }

    #[test]
    fn child_cached_window_reuses_clean_child_buffer() -> Result<()> {
        let mut window = Window::new("window", Text::new("cached"))
            .with_repaint_policy(WindowRepaintPolicy::ChildCached);
        let area = Rect::new(0, 0, 12, 1);
        let mut first = Buffer::empty(area);
        let mut second = Buffer::empty(area);

        window.render_buffer(area, &mut first)?;
        window.render_buffer(area, &mut second)?;

        assert_eq!(window.stats().child_cache_misses, 1);
        assert_eq!(window.stats().child_cache_hits, 1);
        Ok(())
    }

    #[test]
    fn key_scope_resolver_uses_modal_child_window_root_precedence() {
        #[derive(Debug, Clone, PartialEq, Eq)]
        enum Cmd {
            Root,
            Window,
            Child,
            Modal,
        }

        fn map(trigger: char, cmd: Cmd) -> KeyMap<Cmd> {
            let mut map = KeyMap::new();
            map.bind(KeyTrigger::Char(trigger), cmd);
            map
        }

        let resolver = KeyScopeResolver::new(map('x', Cmd::Root))
            .with_scope(
                KeyScope::new("window", KeyScopeRole::Window, map('x', Cmd::Window)).active(true),
            )
            .with_scope(
                KeyScope::new("child", KeyScopeRole::FocusedChild, map('x', Cmd::Child))
                    .active(true)
                    .focused(true),
            )
            .with_scope(
                KeyScope::new("modal", KeyScopeRole::ModalWindow, map('x', Cmd::Modal))
                    .active(true)
                    .focused(true),
            );

        assert_eq!(
            resolver.resolve(KeyEvent::Char('x')).unwrap().command,
            Cmd::Modal
        );
    }

    #[test]
    fn image_viewport_element_separates_buffer_render_and_effects() -> Result<()> {
        let image = ViewportImage::new(PixelSize::new(2, 2), vec![255; 16])?;
        let widget = ImageViewportWidget::new(
            ImageViewport::new(image),
            CanvasMetrics::new(CellSize::new(4, 2), CellPixel::new(1, 1)),
        );
        let mut element =
            ImageViewportElement::new("image", 7, 9, widget).with_png(b"png".to_vec());

        let effects = element.render_effects(Rect::new(10, 3, 4, 2))?;

        assert!(matches!(
            &effects[0],
            RenderEffect::EnsureImageLoaded { image_id: 7, .. }
        ));
        assert!(matches!(
            &effects[1],
            RenderEffect::PlaceImage { origin, options }
                if *origin == CellOffset { col: 10, row: 3 }
                    && options.image_id == 7
                    && options.placement_id == 9
        ));
        Ok(())
    }

    #[test]
    fn modal_activates_as_modal_focus_boundary() {
        let modal = Modal::new("modal", Text::new("body"));
        let scope = modal.window().focus_scope();

        assert!(modal.window().is_active());
        assert!(modal.window().is_focused());
        assert!(modal.window().has_entered());
        assert_eq!(scope.kind, FocusScopeKind::Modal);
        assert_eq!(scope.id.as_str(), "modal");
    }

    #[test]
    fn modal_from_window_allows_hooks_before_activation() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let events = Rc::new(RefCell::new(Vec::new()));
        let window = Window::new("modal", Text::new("body")).on_enter({
            let events = Rc::clone(&events);
            move |event| events.borrow_mut().push(event)
        });

        let modal = Modal::from_window(window);

        assert!(modal.window().is_active());
        assert_eq!(*events.borrow(), vec![WindowLifecycleEvent::Enter]);
    }

    #[test]
    fn modal_routes_keys_to_focused_child() -> Result<()> {
        let child = ProbeElement::new("child", "child").with_binding('m', TestCommand::Modal);
        let mut modal = Modal::new("modal", child);

        assert_eq!(
            modal.handle_event(&KeyEvent::Char('m'))?,
            ComponentOutcome::Message(TestCommand::Modal)
        );
        Ok(())
    }

    #[test]
    fn overlay_renders_layers_in_order() -> Result<()> {
        let mut overlay = Overlay::new("overlay")
            .with_layer(ProbeElement::new("base", "base"), None)
            .with_layer(ProbeElement::new("top", "top!"), None);
        let area = Rect::new(0, 0, 4, 1);
        let mut buffer = Buffer::empty(area);

        overlay.render_buffer(area, &mut buffer)?;

        assert!(format!("{buffer:?}").contains("top!"));
        Ok(())
    }

    #[test]
    fn overlay_routes_to_topmost_non_modal_layer() -> Result<()> {
        let mut overlay = Overlay::new("overlay")
            .with_layer(
                ProbeElement::new("base", "base").with_binding('x', TestCommand::Base),
                None,
            )
            .with_layer(
                ProbeElement::new("top", "top").with_binding('x', TestCommand::Top),
                None,
            );

        assert_eq!(
            overlay.handle_event(&KeyEvent::Char('x'))?,
            ComponentOutcome::Message(TestCommand::Top)
        );
        Ok(())
    }

    #[test]
    fn overlay_modal_layer_captures_key_routing() -> Result<()> {
        let mut overlay = Overlay::new("overlay")
            .with_layer(
                ProbeElement::new("base", "base").with_binding('x', TestCommand::Base),
                None,
            )
            .with_modal_layer(
                ProbeElement::new("modal", "modal").with_binding('m', TestCommand::Modal),
                None,
            );

        assert_eq!(
            overlay.handle_event(&KeyEvent::Char('x'))?,
            ComponentOutcome::Ignored
        );
        assert_eq!(
            overlay.handle_event(&KeyEvent::Char('m'))?,
            ComponentOutcome::Message(TestCommand::Modal)
        );
        Ok(())
    }

    #[test]
    fn key_mapped_decorator_emits_message() -> Result<()> {
        let mut map = KeyMap::new();
        map.bind(KeyTrigger::Special(SpecialKey::Enter), ());
        let mut element = Text::new("ok").with_keymap(map);

        assert!(element.handle_event(&KeyEvent::Enter)?.is_handled());
        Ok(())
    }
}
