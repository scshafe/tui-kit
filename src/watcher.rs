//! Notify-based workspace/file watcher producer for the unified event channel.
//!
//! Watchers are explicitly named so applications and agents can route change
//! notifications without guessing which producer fired. [`WatcherConfig`]
//! validates paths and debounce policy before the notify backend is created.

use crate::config::{ConfigError, Validate};
use crate::events::{AppEvent, AppEventSender};
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WatcherSourceId(String);

impl WatcherSourceId {
    pub fn new(id: impl Into<String>) -> Result<Self, ConfigError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ConfigError::new(
                "WatcherSourceId",
                "must not be empty or whitespace",
            ));
        }
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WatcherSourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Validate for WatcherSourceId {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.0.trim().is_empty() {
            return Err(ConfigError::new(
                "WatcherSourceId",
                "must not be empty or whitespace",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatcherConfig {
    pub id: WatcherSourceId,
    pub paths: Vec<PathBuf>,
    pub debounce: Duration,
}

impl WatcherConfig {
    pub fn workspace<P, I>(id: impl Into<String>, paths: I) -> Result<Self, ConfigError>
    where
        P: Into<PathBuf>,
        I: IntoIterator<Item = P>,
    {
        let config = Self {
            id: WatcherSourceId::new(id)?,
            paths: paths.into_iter().map(Into::into).collect(),
            debounce: Duration::from_millis(75),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn with_debounce(mut self, debounce: Duration) -> Result<Self, ConfigError> {
        self.debounce = debounce;
        self.validate()?;
        Ok(self)
    }
}

impl Validate for WatcherConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        self.id.validate()?;
        if self.paths.is_empty() {
            return Err(ConfigError::new(
                "WatcherConfig.paths",
                "must include at least one file or directory",
            ));
        }
        if self.paths.iter().any(|path| path.as_os_str().is_empty()) {
            return Err(ConfigError::new(
                "WatcherConfig.paths",
                "must not include an empty path",
            ));
        }
        if self.debounce.is_zero() {
            return Err(ConfigError::new(
                "WatcherConfig.debounce",
                "must be greater than zero",
            ));
        }
        Ok(())
    }
}

pub struct WorkspaceWatcher {
    id: WatcherSourceId,
    _watcher: RecommendedWatcher,
}

impl std::fmt::Debug for WorkspaceWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceWatcher")
            .field("id", &self.id)
            .finish()
    }
}

impl WorkspaceWatcher {
    pub fn spawn<UserEvent: Send + 'static>(
        config: WatcherConfig,
        sink: AppEventSender<UserEvent>,
    ) -> Result<Self> {
        config.validate()?;
        let (raw_tx, raw_rx) = std_mpsc::channel::<Event>();
        let mut watcher: RecommendedWatcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    let _ = raw_tx.send(event);
                }
            })
            .context("failed to construct workspace watcher")?;
        for path in &config.paths {
            let target: &Path = if path.is_file() {
                path.parent().unwrap_or(path)
            } else {
                path
            };
            watcher
                .watch(target, RecursiveMode::NonRecursive)
                .with_context(|| format!("failed to watch {}", target.display()))?;
        }
        let id = config.id.clone();
        let thread_id = id.clone();
        thread::Builder::new()
            .name(format!("tui-kit-watcher-{}", thread_id.as_str()))
            .spawn(move || debounce_loop(raw_rx, sink, config.paths, config.debounce, thread_id))
            .context("failed to spawn workspace watcher thread")?;
        Ok(Self {
            id,
            _watcher: watcher,
        })
    }

    pub fn id(&self) -> &WatcherSourceId {
        &self.id
    }
}

fn debounce_loop<UserEvent>(
    raw_rx: std_mpsc::Receiver<Event>,
    sink: AppEventSender<UserEvent>,
    watched_paths: Vec<PathBuf>,
    debounce: Duration,
    id: WatcherSourceId,
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
            Some(event) if is_relevant(&event, &watched_paths) => {
                pending = Some(Instant::now() + debounce);
            }
            None => {
                if sink.send(AppEvent::workspace_changed(id.clone())).is_err() {
                    return;
                }
                pending = None;
            }
            Some(_) => {}
        }
    }
}

fn is_relevant(event: &Event, watched: &[PathBuf]) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::WatcherEvent;
    use notify::event::ModifyKind;
    use std::sync::mpsc;

    #[test]
    fn watcher_config_validates_named_sources_paths_and_debounce() {
        let err = WatcherConfig::workspace(" ", vec![PathBuf::from("src")]).unwrap_err();
        assert_eq!(err.path, "WatcherSourceId");

        let err = WatcherConfig::workspace("workspace", Vec::<PathBuf>::new()).unwrap_err();
        assert_eq!(err.path, "WatcherConfig.paths");

        let err = WatcherConfig::workspace("workspace", vec![PathBuf::from("src")])
            .unwrap()
            .with_debounce(Duration::ZERO)
            .unwrap_err();
        assert_eq!(err.path, "WatcherConfig.debounce");
    }

    #[test]
    fn debounce_loop_emits_named_workspace_changes() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let id = WatcherSourceId::new("workspace").unwrap();
        let thread_id = id.clone();

        thread::spawn(move || {
            debounce_loop(
                raw_rx,
                event_tx,
                vec![PathBuf::from("src")],
                Duration::from_millis(5),
                thread_id,
            )
        });

        raw_tx
            .send(
                Event::new(EventKind::Modify(ModifyKind::Any))
                    .add_path(PathBuf::from("src/lib.rs")),
            )
            .unwrap();

        let event = event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(
            event,
            AppEvent::<()>::Watcher(WatcherEvent::WorkspaceChanged { id })
        );
    }
}
