use crate::events::{AppEvent, AppEventSender};
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub struct WorkspaceWatcher {
    _watcher: RecommendedWatcher,
}

impl std::fmt::Debug for WorkspaceWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceWatcher").finish()
    }
}

impl WorkspaceWatcher {
    pub fn spawn(paths: &[&Path], sink: AppEventSender, debounce: Duration) -> Result<Self> {
        let (raw_tx, raw_rx) = std_mpsc::channel::<Event>();
        let mut watcher: RecommendedWatcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    let _ = raw_tx.send(event);
                }
            })
            .context("failed to construct workspace watcher")?;
        for path in paths {
            let target: &Path = if path.is_file() {
                path.parent().unwrap_or(path)
            } else {
                path
            };
            watcher
                .watch(target, RecursiveMode::NonRecursive)
                .with_context(|| format!("failed to watch {}", target.display()))?;
        }
        let owned_paths: Vec<std::path::PathBuf> =
            paths.iter().map(|p| p.to_path_buf()).collect();
        thread::spawn(move || debounce_loop(raw_rx, sink, owned_paths, debounce));
        Ok(Self { _watcher: watcher })
    }
}

fn debounce_loop(
    raw_rx: std_mpsc::Receiver<Event>,
    sink: AppEventSender,
    watched_paths: Vec<std::path::PathBuf>,
    debounce: Duration,
) {
    let mut pending: Option<Instant> = None;
    loop {
        let event = match pending {
            Some(deadline) => match raw_rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(event) => Some(event),
                Err(std_mpsc::RecvTimeoutError::Timeout) => None,
                Err(std_mpsc::RecvTimeoutError::Disconnected) => return,
            },
            None => match raw_rx.recv() {
                Ok(event) => Some(event),
                Err(_) => return,
            },
        };

        match event {
            Some(event) if is_relevant(&event, &watched_paths) => {
                pending = Some(Instant::now() + debounce);
            }
            None => {
                if sink.send(AppEvent::WorkspaceChanged).is_err() {
                    return;
                }
                pending = None;
            }
            Some(_) => {}
        }
    }
}

fn is_relevant(event: &Event, watched: &[std::path::PathBuf]) -> bool {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return false;
    }
    if watched.iter().all(|p| p.is_dir()) {
        return true;
    }
    event.paths.iter().any(|changed| {
        watched.iter().any(|w| {
            if w.is_file() {
                w == changed
            } else {
                changed.starts_with(w)
            }
        })
    })
}
