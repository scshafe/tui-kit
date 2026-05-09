//! Terminal probing and raw stdout helpers.
//!
//! [`terminal_metrics`] uses TIOCGWINSZ to read both cell dimensions and
//! pixel dimensions. The pixel dimensions matter for image-based UIs that
//! need to compute "fit" math against actual rendered pixels.
//!
//! **Stability:** consumed by c4tui and by terminal setup. This module should
//! stay at the probing/raw-stdout boundary; higher-level terminal policy belongs
//! in [`crate::terminal`].

use crate::layout::{CanvasMetrics, CellPixel, CellSize};
use std::io::{self, IsTerminal, Write};

pub fn stdin_is_terminal() -> bool {
    io::stdin().is_terminal()
}

pub fn stdout_is_terminal() -> bool {
    io::stdout().is_terminal()
}

pub fn write_stdout_all(bytes: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(bytes)?;
    stdout.flush()
}

pub fn terminal_metrics() -> CanvasMetrics {
    let mut size = std::mem::MaybeUninit::<libc::winsize>::zeroed();
    if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, size.as_mut_ptr()) } == -1 {
        return CanvasMetrics::new(CellSize::new(80, 24), CellPixel::FALLBACK);
    }
    let raw = unsafe { size.assume_init() };
    let cols = raw.ws_col.max(1);
    let rows = raw.ws_row.max(2);
    let cell_pixel = if raw.ws_col > 0 && raw.ws_row > 0 && raw.ws_xpixel > 0 && raw.ws_ypixel > 0 {
        CellPixel::new(raw.ws_xpixel / raw.ws_col, raw.ws_ypixel / raw.ws_row)
    } else {
        CellPixel::FALLBACK
    };
    CanvasMetrics::new(CellSize::new(cols, rows), cell_pixel.or_fallback())
}

pub fn get_termios(fd: libc::c_int) -> io::Result<libc::termios> {
    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    if unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { termios.assume_init() })
    }
}

pub fn set_termios(fd: libc::c_int, termios: &libc::termios) -> io::Result<()> {
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, termios) } == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub const fn make_raw(termios: &mut libc::termios) {
    termios.c_iflag &=
        !(libc::BRKINT | libc::ICRNL | libc::INPCK | libc::ISTRIP | libc::IXON | libc::PARMRK);
    termios.c_oflag &= !libc::OPOST;
    termios.c_cflag |= libc::CS8;
    termios.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);
    termios.c_cc[libc::VMIN] = 0;
    termios.c_cc[libc::VTIME] = 0;
}
