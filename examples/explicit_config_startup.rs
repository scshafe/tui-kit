use std::sync::mpsc;
use std::time::Duration;

use tui_kit::events::{AppEvent, RuntimeEvent, TickEvent, WatcherEvent};
use tui_kit::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start from an explicit named preset. This validates terminal, scheduler,
    // image, and theme policy before any raw-mode terminal side effects occur.
    let config = RuntimeConfig::strict_wezterm_kitty(2)?
        .with_tick(TickConfig::production(
            "ui-refresh",
            Duration::from_millis(250),
            TickStartPolicy::AfterInterval,
        )?)?
        .with_watcher(WatcherConfig::workspace("workspace", ["."])?)?;

    // Apps still own the event loop and domain command semantics. tui-kit only
    // provides named producers and typed event categories on one channel.
    config.validate()?;

    let (tx, rx) = mpsc::channel::<AppEvent<AppCommand>>();
    tx.send(AppEvent::heartbeat())?;
    tx.send(AppEvent::tick(config.ticks[0].id.clone()))?;
    tx.send(AppEvent::workspace_changed(config.watchers[0].id.clone()))?;
    tx.send(AppEvent::User(AppCommand::SaveRequested))?;

    for event in rx.try_iter() {
        match event {
            AppEvent::Runtime(RuntimeEvent::Heartbeat) => println!("runtime heartbeat"),
            AppEvent::Tick(TickEvent::Tick { id }) => println!("tick from {id}"),
            AppEvent::Watcher(WatcherEvent::WorkspaceChanged { id }) => {
                println!("workspace changed from {id}")
            }
            AppEvent::User(AppCommand::SaveRequested) => println!("app command: save requested"),
            other => println!("unhandled toolkit event: {other:?}"),
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AppCommand {
    SaveRequested,
}
