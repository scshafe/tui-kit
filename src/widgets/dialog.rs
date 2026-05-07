//! Modal dialog widget — bordered box with title, message, footer hint.
//!
//! Apps in dialog mode render a [`Dialog`] in their `terminal.draw()`
//! closure. Dismissal logic (any-key / Esc-only / no-dismiss) is the
//! app's responsibility — this widget is purely presentational.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

#[derive(Debug, Clone)]
pub struct Dialog {
    pub title: String,
    pub message: String,
    pub footer: String,
}

impl Dialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            footer: String::new(),
        }
    }

    pub fn with_footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = footer.into();
        self
    }
}

impl Widget for Dialog {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.title.trim()))
            .title_bottom(self.footer.clone());
        let inner = block.inner(area);
        block.render(area, buf);
        Paragraph::new(self.message.clone())
            .wrap(Wrap { trim: false })
            .render(inner, buf);
    }
}
