//! Detached thread that drains crossterm events into the unified
//! [`AppEventSender`] channel. Spawn after [`crate::terminal::Terminal::enter`]
//! has put the terminal into raw mode.
//!
//! **Stability:** consumed by c4tui as the production input producer. This
//! module should stay policy-light: it translates terminal input into events,
//! but does not interpret commands or own the app loop.

use crate::events::{AppEvent, AppEventSender};
use crate::input::{read_key, Key};
use std::thread;

pub fn spawn(sink: AppEventSender) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        match read_key() {
            Ok(Key::Resize { cols, rows }) => {
                if sink.send(AppEvent::terminal_resize(cols, rows)).is_err() {
                    return;
                }
            }
            Ok(key) => {
                if sink.send(AppEvent::input_key(key)).is_err() {
                    return;
                }
            }
            Err(error) => {
                log::warn!("input thread terminating: {error:#}");
                return;
            }
        }
    })
}
