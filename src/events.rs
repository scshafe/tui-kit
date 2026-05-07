//! Unified application event channel.
//!
//! All producers (input thread, file watcher, scheduler, ticker) push
//! [`AppEvent`] values into a single [`AppEventSender`]; the application's
//! main loop drains the matching [`AppEventReceiver`].
//!
//! Scheduler completions are signalled by [`AppEvent::SchedulerComplete`]
//! (a wake-up only). The scheduler buffers completion data internally;
//! the app drains it via the scheduler's own API. This keeps the event
//! enum fully concrete and free of scheduler-specific generics.

use crate::input::Key;
use std::sync::mpsc::{Receiver, Sender};

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(Key),
    Resize { cols: u16, rows: u16 },
    SchedulerComplete,
    WorkspaceChanged,
    Heartbeat,
}

pub type AppEventSender = Sender<AppEvent>;
pub type AppEventReceiver = Receiver<AppEvent>;
