use anyhow::Result;
use crossterm::event::{read, Event, KeyCode, KeyEventKind, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Clear, Paragraph, Widget};
use tui_kit::image::{ImageSurface, MAIN_PLACEMENT_ID};
use tui_kit::layout::PixelSize;
use tui_kit::prelude::{ImageBox, ImageBoxPlan, ImageBoxState, Terminal, TerminalConfig};

const IMAGE_ID: u32 = 4242;
const IMAGE_WIDTH: u32 = 1600;
const IMAGE_HEIGHT: u32 = 1000;

fn main() -> Result<()> {
    let png = demo_png(IMAGE_WIDTH, IMAGE_HEIGHT);
    let image_size = PixelSize::new(IMAGE_WIDTH, IMAGE_HEIGHT);
    let mut terminal = Terminal::enter_with_config(TerminalConfig::strict_wezterm_kitty())?;
    terminal.images().ensure_loaded(IMAGE_ID, &png)?;

    let mut state = ImageBoxState::default();
    let mut show_border = true;
    let mut color_index = 0usize;
    let colors = [
        Color::Cyan,
        Color::Yellow,
        Color::Green,
        Color::Magenta,
        Color::White,
    ];

    loop {
        let metrics = terminal.metrics();
        let full_area = Rect::new(0, 0, metrics.cells.cols, metrics.cells.rows);
        let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(full_area);
        let image_area = chunks[0];
        let status_area = chunks[1];
        let border_color = colors[color_index % colors.len()];
        let image_box = ImageBox::new(IMAGE_ID, MAIN_PLACEMENT_ID, image_size)
            .border(show_border)
            .title(format!(
                " ImageBox visual | zoom x{:.2} | border {} ",
                state.zoom,
                if show_border { "on" } else { "off" }
            ))
            .border_style(Style::default().fg(border_color));
        let plan = image_box.plan(image_area, metrics, &state)?;
        let status = status_text(&plan, show_border);

        terminal.draw(|frame| {
            Clear.render(frame.area(), frame.buffer_mut());
            plan.render(frame.buffer_mut());
            Paragraph::new(status.clone()).render(status_area, frame.buffer_mut());
        })?;
        plan.place(terminal.images())?;
        terminal.images().flush()?;

        match read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('+') | KeyCode::Char('=') => state.zoom_at(&plan, 1.25, 0.5, 0.5),
                KeyCode::Char('-') | KeyCode::Char('_') => state.zoom_at(&plan, 0.8, 0.5, 0.5),
                KeyCode::Char('0') | KeyCode::Char('f') => state.reset_zoom(),
                KeyCode::Char('b') => show_border = !show_border,
                KeyCode::Char('c') => color_index = color_index.wrapping_add(1),
                KeyCode::Left | KeyCode::Char('h') => state.pan(&plan, -0.10, 0.0),
                KeyCode::Right | KeyCode::Char('l') => state.pan(&plan, 0.10, 0.0),
                KeyCode::Up | KeyCode::Char('k') => state.pan(&plan, 0.0, -0.10),
                KeyCode::Down | KeyCode::Char('j') => state.pan(&plan, 0.0, 0.10),
                _ => {}
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => {
                    let (x, y) = mouse_anchor(&plan, mouse.column, mouse.row);
                    state.zoom_at(&plan, 1.15, x, y);
                }
                MouseEventKind::ScrollDown => {
                    let (x, y) = mouse_anchor(&plan, mouse.column, mouse.row);
                    state.zoom_at(&plan, 0.87, x, y);
                }
                _ => {}
            },
            Event::Resize(_, _) => {}
            _ => {}
        }
    }

    terminal.images().delete_placement(MAIN_PLACEMENT_ID)?;
    terminal.images().flush()?;
    Ok(())
}

fn status_text(plan: &ImageBoxPlan, show_border: bool) -> String {
    let placement = plan
        .placement
        .as_ref()
        .map(|p| {
            format!(
                "source {}x{} @ {},{} | cells {}x{} @ {},{}",
                p.source.width,
                p.source.height,
                p.source.x,
                p.source.y,
                p.cell_cols,
                p.cell_rows,
                p.origin.col,
                p.origin.row
            )
        })
        .unwrap_or_else(|| "no placement: terminal area too small".to_owned());
    format!(
        "+/- zoom  arrows/hjkl pan  mouse wheel zoom-at-cursor  0/f reset  b border({})  c color  q quit\n{} | theoretical {}x{} | visible {}x{}",
        if show_border { "on" } else { "off" },
        placement,
        plan.theoretical_pixels.width,
        plan.theoretical_pixels.height,
        plan.visible_pixels.width,
        plan.visible_pixels.height
    )
}

fn mouse_anchor(plan: &ImageBoxPlan, col: u16, row: u16) -> (f32, f32) {
    if plan.image_area.width == 0 || plan.image_area.height == 0 {
        return (0.5, 0.5);
    }
    let x =
        (f32::from(col) + 0.5 - f32::from(plan.image_area.x)) / f32::from(plan.image_area.width);
    let y =
        (f32::from(row) + 0.5 - f32::from(plan.image_area.y)) / f32::from(plan.image_area.height);
    (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))
}

fn demo_png(width: u32, height: u32) -> Vec<u8> {
    let mut raw = Vec::with_capacity(((width * 4 + 1) * height) as usize);
    for y in 0..height {
        raw.push(0);
        for x in 0..width {
            raw.extend_from_slice(&pixel(width, height, x, y));
        }
    }

    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_chunk(&mut png, b"IHDR", &ihdr);
    write_chunk(&mut png, b"IDAT", &zlib_store(&raw));
    write_chunk(&mut png, b"IEND", &[]);
    png
}

fn pixel(width: u32, height: u32, x: u32, y: u32) -> [u8; 4] {
    let mut r = ((x * 255) / width.max(1)) as u8;
    let mut g = ((y * 255) / height.max(1)) as u8;
    let mut b = 120u8.saturating_add((((x / 80) ^ (y / 80)) & 1) as u8 * 45);

    let edge = 55;
    if y < edge {
        r = 245;
        g = 30;
        b = 45;
    } else if y >= height.saturating_sub(edge) {
        r = 35;
        g = 80;
        b = 245;
    } else if x < edge {
        r = 35;
        g = 210;
        b = 80;
    } else if x >= width.saturating_sub(edge) {
        r = 245;
        g = 215;
        b = 35;
    }

    let corner = 180;
    if x < corner && y < corner {
        [255, 0, 0, 255]
    } else if x >= width.saturating_sub(corner) && y < corner {
        [0, 220, 80, 255]
    } else if x < corner && y >= height.saturating_sub(corner) {
        [30, 90, 255, 255]
    } else if x >= width.saturating_sub(corner) && y >= height.saturating_sub(corner) {
        [255, 230, 0, 255]
    } else if x % 100 < 4 || y % 100 < 4 {
        [18, 22, 28, 255]
    } else if diagonal(width, height, x, y) {
        [255, 255, 255, 255]
    } else {
        [r, g, b, 255]
    }
}

fn diagonal(width: u32, height: u32, x: u32, y: u32) -> bool {
    let lhs = x as i64 * height as i64;
    let rhs = y as i64 * width as i64;
    let other = (width.saturating_sub(x)) as i64 * height as i64;
    (lhs - rhs).abs() < width.max(height) as i64 * 3
        || (other - rhs).abs() < width.max(height) as i64 * 3
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut offset = 0;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let len = remaining.min(u16::MAX as usize);
        let final_block = u8::from(offset + len == data.len());
        out.push(final_block);
        let len_u16 = len as u16;
        out.extend_from_slice(&len_u16.to_le_bytes());
        out.extend_from_slice(&(!len_u16).to_le_bytes());
        out.extend_from_slice(&data[offset..offset + len]);
        offset += len;
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn write_chunk(png: &mut Vec<u8>, name: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(name);
    png.extend_from_slice(data);
    let mut crc_data = Vec::with_capacity(name.len() + data.len());
    crc_data.extend_from_slice(name);
    crc_data.extend_from_slice(data);
    png.extend_from_slice(&crc32(&crc_data).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + u32::from(*byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
