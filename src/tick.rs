//! Periodic tick producers for the unified event channel.
//!
//! Ticks are named so applications and agents can route timer wake-ups without
//! guessing which loop or subsystem produced them. A [`TickHandle`] stops the
//! background thread explicitly on drop or via [`TickHandle::stop`].

use crate::config::{ConfigError, Validate};
use crate::events::{AppEvent, AppEventSender};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MIN_PRODUCTION_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TickSourceId(String);

impl TickSourceId {
    pub fn new(id: impl Into<String>) -> Result<Self, ConfigError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ConfigError::new(
                "TickSourceId",
                "must not be empty or whitespace",
            ));
        }
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TickSourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TickStartPolicy {
    Immediate,
    AfterInterval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MissedTickPolicy {
    /// Send at most one tick after each interval wait. If the receiver is slow,
    /// ticks naturally coalesce at channel-drain time instead of accumulating
    /// catch-up bursts from this producer.
    Coalesce,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickConfig {
    pub id: TickSourceId,
    pub interval: Duration,
    pub start: TickStartPolicy,
    pub missed_tick_policy: MissedTickPolicy,
    pub allow_subproduction_interval_for_tests: bool,
}

impl TickConfig {
    pub fn production(
        id: impl Into<String>,
        interval: Duration,
        start: TickStartPolicy,
    ) -> Result<Self, ConfigError> {
        let config = Self {
            id: TickSourceId::new(id)?,
            interval,
            start,
            missed_tick_policy: MissedTickPolicy::Coalesce,
            allow_subproduction_interval_for_tests: false,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn test_fast(id: impl Into<String>, interval: Duration) -> Result<Self, ConfigError> {
        let config = Self {
            id: TickSourceId::new(id)?,
            interval,
            start: TickStartPolicy::AfterInterval,
            missed_tick_policy: MissedTickPolicy::Coalesce,
            allow_subproduction_interval_for_tests: true,
        };
        config.validate()?;
        Ok(config)
    }
}

impl Validate for TickConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        self.id.validate()?;
        if self.interval.is_zero() {
            return Err(ConfigError::new(
                "TickConfig.interval",
                "must be greater than zero",
            ));
        }
        if !self.allow_subproduction_interval_for_tests && self.interval < MIN_PRODUCTION_INTERVAL {
            return Err(ConfigError::new(
                "TickConfig.interval",
                "must be at least 10ms unless explicitly marked as a test-fast tick source",
            ));
        }
        Ok(())
    }
}

impl Validate for TickSourceId {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.0.trim().is_empty() {
            return Err(ConfigError::new(
                "TickSourceId",
                "must not be empty or whitespace",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct TickHandle {
    stopped: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl TickHandle {
    pub fn stop(mut self) {
        self.stop_inner();
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    fn stop_inner(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for TickHandle {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

pub fn spawn<UserEvent: Send + 'static>(
    config: TickConfig,
    sink: AppEventSender<UserEvent>,
) -> Result<TickHandle, ConfigError> {
    config.validate()?;

    let stopped = Arc::new(AtomicBool::new(false));
    let thread_stopped = Arc::clone(&stopped);
    let thread = thread::Builder::new()
        .name(format!("tui-kit-tick-{}", config.id.as_str()))
        .spawn(move || run(config, sink, thread_stopped))
        .map_err(|err| ConfigError::new("TickSource.thread", err.to_string()))?;

    Ok(TickHandle {
        stopped,
        thread: Some(thread),
    })
}

fn run<UserEvent>(config: TickConfig, sink: AppEventSender<UserEvent>, stopped: Arc<AtomicBool>) {
    if config.start == TickStartPolicy::Immediate
        && sink.send(AppEvent::tick(config.id.clone())).is_err()
    {
        return;
    }

    while !stopped.load(Ordering::Relaxed) {
        thread::sleep(config.interval);
        if stopped.load(Ordering::Relaxed) {
            break;
        }
        match config.missed_tick_policy {
            MissedTickPolicy::Coalesce => {
                if sink.send(AppEvent::tick(config.id.clone())).is_err() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{AppEvent, TickEvent};
    use std::sync::mpsc;

    #[test]
    fn validates_named_nonzero_tick_sources() {
        let err =
            TickConfig::production(" ", Duration::from_millis(100), TickStartPolicy::Immediate)
                .unwrap_err();
        assert_eq!(err.path, "TickSourceId");

        let err =
            TickConfig::production("ui", Duration::ZERO, TickStartPolicy::Immediate).unwrap_err();
        assert_eq!(err.path, "TickConfig.interval");

        let err =
            TickConfig::production("ui", Duration::from_millis(1), TickStartPolicy::Immediate)
                .unwrap_err();
        assert_eq!(err.path, "TickConfig.interval");

        TickConfig::test_fast("ui", Duration::from_millis(1)).unwrap();
    }

    #[test]
    fn immediate_tick_source_sends_named_tick() {
        let (tx, rx) = mpsc::channel();
        let config = TickConfig::test_fast("paint", Duration::from_millis(50)).unwrap();
        let config = TickConfig {
            start: TickStartPolicy::Immediate,
            ..config
        };

        let handle = spawn(config, tx).unwrap();
        let event = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        handle.stop();

        assert_eq!(
            event,
            AppEvent::<()>::Tick(TickEvent::Tick {
                id: TickSourceId::new("paint").unwrap()
            })
        );
    }

    #[test]
    fn after_interval_tick_source_waits_before_first_tick() {
        let (tx, rx) = mpsc::channel();
        let config = TickConfig::test_fast("slow", Duration::from_secs(1)).unwrap();

        let handle = spawn(config, tx).unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
        let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        handle.stop();

        assert_eq!(
            event,
            AppEvent::<()>::Tick(TickEvent::Tick {
                id: TickSourceId::new("slow").unwrap()
            })
        );
    }
}
