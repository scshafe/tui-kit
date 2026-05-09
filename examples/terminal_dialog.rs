use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use tui_kit::bar::layout_status_line;
use tui_kit::prelude::*;

fn main() -> Result<()> {
    let mut terminal = Terminal::enter_with_config(TerminalConfig::degraded_no_images())?;

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(Block::default().borders(Borders::ALL), area);

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(1)])
                .split(area);

            let dialog_area = centered_rect(rows[0], 48, 7);
            frame.render_widget(
                Dialog::new("tui-kit terminal example", "This example enters raw mode, renders a tui-kit Dialog, and exits cleanly on q or Esc.")
                    .with_footer(" q/Esc exits "),
                dialog_area,
            );

            let status = layout_status_line(
                vec![
                    StatusFragment::new("tui-kit").with_priority(255),
                    StatusFragment::new("degraded_no_images").with_priority(160),
                ],
                vec![StatusFragment::new("q/Esc exits").with_priority(200)],
                rows[1].width as usize,
                " | ",
                "…",
            );
            frame.render_widget(
                Paragraph::new(status).style(Style::default().fg(Color::Gray)),
                rows[1],
            );

        })?;

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) => break,
                _ => {}
            }
        }
    }

    Ok(())
}

fn centered_rect(area: ratatui::layout::Rect, width: u16, height: u16) -> ratatui::layout::Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    ratatui::layout::Rect::new(x, y, width, height)
}
