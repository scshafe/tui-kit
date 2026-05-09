//! Notify-based workspace/file watcher producer for the unified event channel.
//!
//! [`WorkspaceWatcher::spawn`] takes a list of paths and a debounce duration.
//! Relevant filesystem events (Create/Modify/Remove) coalesce into a single
//! [`AppEvent::Watcher`] carrying [`crate::events::WatcherEvent::WorkspaceChanged`].
//!
//! **Stability:** consumed by c4tui's workspace reload path. Named watcher
//! routing was removed because c4tui did not need it; reintroduce source IDs
//! only with a consumer that handles multiple watcher sources.

use crate::events::{AppEvent, AppEventSender};
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct WorkspaceWatcher {
    _watcher: RecommendedWatcher,
}

impl WorkspaceWatcher {
    pub fn spawn<I, P, UserEvent>(
        paths: I,
        debounce: Duration,
        sink: AppEventSender<UserEvent>,
    ) -> Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
        UserEvent: Send + 'static,
    {
        if debounce.is_zero() {
            return Err(anyhow::anyhow!(
                "watcher debounce must be greater than zero"
            ));
        }
        let paths: Vec<PathBuf> = paths
            .into_iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect();
        if paths.is_empty() {
            return Err(anyhow::anyhow!("watcher requires at least one path"));
        }

        let (raw_tx, raw_rx) = std_mpsc::channel::<Event>();
        let mut watcher: RecommendedWatcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    let _ = raw_tx.send(event);
                }
            })
            .context("failed to construct workspace watcher")?;
        let mut classified: Vec<WatchedPath> = Vec::with_capacity(paths.len());
        for path in &paths {
            let is_file = path.is_file();
            let target: &Path = if is_file {
                path.parent().unwrap_or(path)
            } else {
                path
            };
            watcher
                .watch(target, RecursiveMode::NonRecursive)
                .with_context(|| format!("failed to watch {}", target.display()))?;
            classified.push(WatchedPath {
                path: path.clone(),
                is_file,
            });
        }
        thread::Builder::new()
            .name("tui-kit-watcher".to_string())
            .spawn(move || debounce_loop(raw_rx, sink, classified, debounce))
            .context("failed to spawn workspace watcher thread")?;
        Ok(Self { _watcher: watcher })
    }
}

#[derive(Debug, Clone)]
struct WatchedPath {
    path: PathBuf,
    is_file: bool,
}

fn debounce_loop<UserEvent>(
    raw_rx: std_mpsc::Receiver<Event>,
    sink: AppEventSender<UserEvent>,
    watched: Vec<WatchedPath>,
    debounce: Duration,
) {
    let mut pending: Option<Instant> = None;
    loop {
        let event = match pending {
            Some(deadline) => {
                match raw_rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(event) => Some(event),
                    Err(std_mpsc::RecvTimeoutError::Timeout) => None,
                    Err(std_mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            None => match raw_rx.recv() {
                Ok(event) => Some(event),
                Err(_) => return,
            },
        };

        match event {
            Some(event) if is_relevant(&event, &watched) => {
                pending = Some(Instant::now() + debounce);
            }
            None => {
                if sink.send(AppEvent::workspace_changed()).is_err() {
                    return;
                }
                pending = None;
            }
            Some(_) => {}
        }
    }
}

fn is_relevant(event: &Event, watched: &[WatchedPath]) -> bool {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return false;
    }
    if watched.iter().all(|w| !w.is_file) {
        return true;
    }
    event.paths.iter().any(|changed| {
        watched.iter().any(|w| {
            if w.is_file {
                &w.path == changed
            } else {
                changed.starts_with(&w.path)
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::WatcherEvent;
    use notify::event::ModifyKind;
    use std::sync::mpsc;

    #[test]
    fn debounce_loop_emits_workspace_changes_after_debounce() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel::<AppEvent<()>>();

        thread::spawn(move || {
            debounce_loop(
                raw_rx,
                event_tx,
                vec![WatchedPath {
                    path: PathBuf::from("src"),
                    is_file: false,
                }],
                Duration::from_millis(5),
            )
        });

        raw_tx
            .send(
                Event::new(EventKind::Modify(ModifyKind::Any))
                    .add_path(PathBuf::from("src/lib.rs")),
            )
            .unwrap();

        let event = event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(event, AppEvent::Watcher(WatcherEvent::WorkspaceChanged));
    }

    #[test]
    fn spawn_rejects_zero_debounce_and_empty_paths() {
        let (tx, _rx) = mpsc::channel::<AppEvent<()>>();
        assert!(
            WorkspaceWatcher::spawn::<_, &Path, ()>([], Duration::from_millis(50), tx.clone())
                .is_err()
        );
        assert!(WorkspaceWatcher::spawn(["src"], Duration::ZERO, tx).is_err());
    }
}
