//! Unified application event channel.
//!
//! All producers (input thread, file watcher, scheduler, and app-defined
//! producers) push typed [`AppEvent`] categories into a single
//! [`AppEventSender`]; the application's main loop drains the matching
//! [`AppEventReceiver`].
//!
//! Scheduler completions are signalled by [`AppEvent::Scheduler`] carrying a
//! [`SchedulerEvent::Complete`] wake-up. The
//! scheduler buffers completion data internally; the app drains it via the
//! scheduler's own API. This keeps event delivery unified without letting the
//! top-level event enum become an unstructured junk drawer.

use crate::input::Key;
use std::convert::Infallible;
use std::sync::mpsc::{Receiver, Sender};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AppEvent<UserEvent = Infallible> {
    Input(InputEvent),
    Terminal(TerminalEvent),
    Scheduler(SchedulerEvent),
    Watcher(WatcherEvent),
    User(UserEvent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputEvent {
    Key(Key),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TerminalEvent {
    Resize { cols: u16, rows: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SchedulerEvent {
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WatcherEvent {
    WorkspaceChanged,
}

impl<UserEvent> AppEvent<UserEvent> {
    pub fn input_key(key: Key) -> Self {
        Self::Input(InputEvent::Key(key))
    }

    pub fn terminal_resize(cols: u16, rows: u16) -> Self {
        Self::Terminal(TerminalEvent::Resize { cols, rows })
    }

    pub fn scheduler_complete() -> Self {
        Self::Scheduler(SchedulerEvent::Complete)
    }

    pub fn workspace_changed() -> Self {
        Self::Watcher(WatcherEvent::WorkspaceChanged)
    }
}

pub type AppEventSender<UserEvent = Infallible> = Sender<AppEvent<UserEvent>>;
pub type AppEventReceiver<UserEvent = Infallible> = Receiver<AppEvent<UserEvent>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_keep_events_in_typed_categories() {
        assert_eq!(
            AppEvent::<Infallible>::input_key(Key::Enter),
            AppEvent::Input(InputEvent::Key(Key::Enter))
        );
        assert_eq!(
            AppEvent::<Infallible>::terminal_resize(80, 24),
            AppEvent::Terminal(TerminalEvent::Resize { cols: 80, rows: 24 })
        );
        assert_eq!(
            AppEvent::<Infallible>::scheduler_complete(),
            AppEvent::Scheduler(SchedulerEvent::Complete)
        );
        assert_eq!(
            AppEvent::<Infallible>::workspace_changed(),
            AppEvent::Watcher(WatcherEvent::WorkspaceChanged)
        );
    }

    #[test]
    fn user_events_do_not_require_forking_the_enum() {
        #[derive(Debug, Clone, PartialEq, Eq)]
        enum DomainEvent {
            SaveRequested,
        }

        let event: AppEvent<DomainEvent> = AppEvent::User(DomainEvent::SaveRequested);

        assert_eq!(event, AppEvent::User(DomainEvent::SaveRequested));
    }
}
