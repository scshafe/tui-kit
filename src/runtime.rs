//! Top-level runtime configuration for composing tui-kit subsystems.
//!
//! [`RuntimeConfig`] is intentionally a validated bundle rather than a hidden
//! application runner. It gives apps and agents one machine-readable place to
//! declare terminal, scheduler, theme, tick, and watcher policy while keeping
//! the toolkit policy-light: applications still own their event loop and domain
//! command semantics.

use std::collections::BTreeSet;

use crate::config::{ConfigError, Validate};
use crate::events::AppEventSender;
use crate::scheduler::SchedulerConfig;
use crate::terminal::TerminalConfig;
use crate::theme::ThemeConfig;
use crate::tick::{self, TickConfig, TickHandle};
use crate::watcher::{WatcherConfig, WorkspaceWatcher};

/// Explicit validated policy bundle for a tui-kit application runtime.
///
/// This does not start threads or enter raw mode by itself. Use it as the
/// single preflight object that validates operational choices before wiring the
/// terminal, scheduler, tick producers, watchers, and theme into an app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub terminal: TerminalConfig,
    pub scheduler: SchedulerConfig,
    pub theme: ThemeConfig,
    pub ticks: Vec<TickConfig>,
    pub watchers: Vec<WatcherConfig>,
}

impl RuntimeConfig {
    /// Strict near-term runtime preset for WezTerm/Kitty applications.
    ///
    /// The caller must still opt into concrete tick and watcher producers with
    /// [`RuntimeConfig::with_tick`] and [`RuntimeConfig::with_watcher`].
    pub fn strict_wezterm_kitty(worker_count: usize) -> std::result::Result<Self, ConfigError> {
        let config = Self {
            terminal: TerminalConfig::strict_wezterm_kitty(),
            scheduler: SchedulerConfig::explicit(worker_count),
            theme: ThemeConfig::high_contrast_dark(),
            ticks: Vec::new(),
            watchers: Vec::new(),
        };
        config.validate()?;
        Ok(config)
    }

    /// Explicit no-image/headless preset for deterministic tests and inert
    /// harnesses. It keeps operational policy visible without implying a live
    /// terminal or image backend.
    pub fn headless_test() -> Self {
        Self {
            terminal: TerminalConfig::headless_test(),
            scheduler: SchedulerConfig::single_worker(),
            theme: ThemeConfig::high_contrast_dark(),
            ticks: Vec::new(),
            watchers: Vec::new(),
        }
    }

    /// Add a named tick source and revalidate the whole runtime config.
    pub fn with_tick(mut self, tick: TickConfig) -> std::result::Result<Self, ConfigError> {
        self.ticks.push(tick);
        self.validate()?;
        Ok(self)
    }

    /// Add a named watcher source and revalidate the whole runtime config.
    pub fn with_watcher(
        mut self,
        watcher: WatcherConfig,
    ) -> std::result::Result<Self, ConfigError> {
        self.watchers.push(watcher);
        self.validate()?;
        Ok(self)
    }

    /// Spawn the configured event producers without taking over the app loop.
    ///
    /// This starts only the explicitly declared tick and watcher producers and
    /// returns their handles so applications keep lifecycle ownership. Terminal
    /// entry, scheduler construction, event dispatch, and domain command
    /// handling remain app-owned policy.
    pub fn spawn_producers<UserEvent: Send + 'static>(
        &self,
        sink: AppEventSender<UserEvent>,
    ) -> anyhow::Result<RuntimeProducers> {
        self.validate()?;

        let mut handles = RuntimeProducers::default();
        for tick in &self.ticks {
            handles.ticks.push(tick::spawn(tick.clone(), sink.clone())?);
        }
        for watcher in &self.watchers {
            handles
                .watchers
                .push(WorkspaceWatcher::spawn(watcher.clone(), sink.clone())?);
        }
        Ok(handles)
    }
}

/// Handles for event producers started from a [`RuntimeConfig`].
///
/// Dropping this value stops tick producers via their handles and drops watcher
/// backends. Keep it alive for as long as the app wants runtime events flowing
/// into the unified channel.
#[derive(Debug, Default)]
pub struct RuntimeProducers {
    pub ticks: Vec<TickHandle>,
    pub watchers: Vec<WorkspaceWatcher>,
}

impl RuntimeProducers {
    pub fn tick_count(&self) -> usize {
        self.ticks.len()
    }

    pub fn watcher_count(&self) -> usize {
        self.watchers.len()
    }

    /// Stop producers that expose explicit stop handles.
    ///
    /// Watchers are stopped by dropping their notify backend when this value is
    /// consumed; tick producers receive an explicit stop signal before join.
    pub fn stop(mut self) {
        for tick in self.ticks.drain(..) {
            tick.stop();
        }
    }
}

impl Validate for RuntimeConfig {
    fn validate(&self) -> std::result::Result<(), ConfigError> {
        self.terminal
            .validate()
            .map_err(prefix("runtime.terminal"))?;
        self.scheduler
            .validate()
            .map_err(prefix("runtime.scheduler"))?;
        self.theme.validate().map_err(prefix("runtime.theme"))?;

        let mut tick_ids = BTreeSet::new();
        for (index, tick) in self.ticks.iter().enumerate() {
            tick.validate()
                .map_err(prefix(format!("runtime.ticks[{index}]")))?;
            if !tick_ids.insert(tick.id.as_str().to_owned()) {
                return Err(ConfigError::new(
                    format!("runtime.ticks[{index}].id"),
                    format!(
                        "duplicate tick source id '{}'; runtime producers must be uniquely routable",
                        tick.id.as_str()
                    ),
                ));
            }
        }

        let mut watcher_ids = BTreeSet::new();
        for (index, watcher) in self.watchers.iter().enumerate() {
            watcher
                .validate()
                .map_err(prefix(format!("runtime.watchers[{index}]")))?;
            if !watcher_ids.insert(watcher.id.as_str().to_owned()) {
                return Err(ConfigError::new(
                    format!("runtime.watchers[{index}].id"),
                    format!(
                        "duplicate watcher source id '{}'; runtime producers must be uniquely routable",
                        watcher.id.as_str()
                    ),
                ));
            }
        }

        Ok(())
    }
}

fn prefix(prefix: impl Into<String>) -> impl FnOnce(ConfigError) -> ConfigError {
    let prefix = prefix.into();
    move |err| ConfigError::new(format!("{prefix}.{}", err.path), err.reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{AppEvent, TickEvent};
    use crate::tick::{TickSourceId, TickStartPolicy};
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn strict_runtime_preset_validates_terminal_scheduler_and_theme() {
        let config = RuntimeConfig::strict_wezterm_kitty(2).unwrap();

        assert_eq!(config.terminal, TerminalConfig::strict_wezterm_kitty());
        assert_eq!(config.scheduler, SchedulerConfig::explicit(2));
        config.validate().unwrap();
    }

    #[test]
    fn runtime_reports_nested_scheduler_paths() {
        let err = RuntimeConfig::strict_wezterm_kitty(0).unwrap_err();

        assert_eq!(err.path, "runtime.scheduler.scheduler.worker_count");
        assert!(err.reason.contains("must be at least one"));
    }

    #[test]
    fn runtime_rejects_duplicate_tick_source_ids() {
        let tick = || TickConfig::test_fast("ui", Duration::from_millis(1)).unwrap();

        let err = RuntimeConfig::headless_test()
            .with_tick(tick())
            .unwrap()
            .with_tick(tick())
            .unwrap_err();

        assert_eq!(err.path, "runtime.ticks[1].id");
        assert!(err.reason.contains("duplicate tick source id 'ui'"));
    }

    #[test]
    fn runtime_rejects_duplicate_watcher_source_ids() {
        let watcher = || WatcherConfig::workspace("workspace", ["."]).unwrap();

        let err = RuntimeConfig::headless_test()
            .with_watcher(watcher())
            .unwrap()
            .with_watcher(watcher())
            .unwrap_err();

        assert_eq!(err.path, "runtime.watchers[1].id");
        assert!(err
            .reason
            .contains("duplicate watcher source id 'workspace'"));
    }

    #[test]
    fn runtime_reports_nested_tick_paths() {
        let invalid = TickConfig {
            id: crate::tick::TickSourceId::new("ui").unwrap(),
            interval: Duration::ZERO,
            start: TickStartPolicy::AfterInterval,
            missed_tick_policy: crate::tick::MissedTickPolicy::Coalesce,
            allow_subproduction_interval_for_tests: true,
        };

        let err = RuntimeConfig::headless_test()
            .with_tick(invalid)
            .unwrap_err();

        assert_eq!(err.path, "runtime.ticks[0].TickConfig.interval");
    }

    #[test]
    fn runtime_spawns_configured_tick_producers_without_app_loop_policy() {
        let (tx, rx) = mpsc::channel();
        let tick = TickConfig {
            start: TickStartPolicy::Immediate,
            ..TickConfig::test_fast("paint", Duration::from_secs(1)).unwrap()
        };
        let config = RuntimeConfig::headless_test().with_tick(tick).unwrap();

        let handles = config.spawn_producers(tx).unwrap();
        let event = rx.recv_timeout(Duration::from_secs(1)).unwrap();

        assert_eq!(handles.tick_count(), 1);
        assert_eq!(handles.watcher_count(), 0);
        handles.stop();
        assert_eq!(
            event,
            AppEvent::<()>::Tick(TickEvent::Tick {
                id: TickSourceId::new("paint").unwrap()
            })
        );
    }
}
