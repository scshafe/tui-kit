//! Presentational modal dialog widget.
//!
//! [`Dialog`] renders title, message, and footer text. Confirmation,
//! cancellation, focus routing, and command meaning stay in applications until
//! a real consumer asks tui-kit to own reusable modal state.

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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn renders_title_message_and_footer() {
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);

        Dialog::new(" Confirm ", "Delete item?")
            .with_footer(" Esc cancels ")
            .render(area, &mut buf);

        let rendered = format!("{buf:?}");
        assert!(rendered.contains("Confirm"));
        assert!(rendered.contains("Delete item?"));
        assert!(rendered.contains("Esc cancels"));
    }
}
