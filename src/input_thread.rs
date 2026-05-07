//! Detached thread that drains crossterm events into the unified
//! [`AppEventSender`] channel. Spawn after [`crate::terminal::Terminal::enter`]
//! has put the terminal into raw mode.

use crate::events::{AppEvent, AppEventSender};
use crate::input::{read_key, Key};
use std::thread;

pub fn spawn(sink: AppEventSender) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        match read_key() {
            Ok(Key::Resize { cols, rows }) => {
                if sink.send(AppEvent::Resize { cols, rows }).is_err() {
                    return;
                }
            }
            Ok(key) => {
                if sink.send(AppEvent::Key(key)).is_err() {
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
