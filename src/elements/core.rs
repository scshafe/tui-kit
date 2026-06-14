use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders};
use serde::{Deserialize, Serialize};

use crate::component::{BufferComponent, ComponentId, ComponentOutcome, DirtyReason, DirtyState};
use crate::input::KeyEvent;
use crate::keymap::KeyMap;

use super::decorators::{Bordered, Focusable, KeyMapped, Padded, ScrollY};

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

    pub(super) fn block(&self) -> Block<'_> {
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
