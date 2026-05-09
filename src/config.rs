//! Shared configuration validation primitives.
//!
//! Runtime-facing configuration should be explicit and validated before use.
//! Subsystems implement [`Validate`] to report machine-readable field paths
//! and clear reasons instead of silently accepting ambiguous policy.
//!
//! **Stability:** shared by validated public configs that survived the
//! feedback-loop correction, currently terminal/image/focus/placement policy.
//! New config wrappers should not be added unless a consumer needs the bundle;
//! prefer direct subsystem validation with non-doubled paths.

use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub path: String,
    pub reason: String,
}

impl ConfigError {
    pub fn new(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid config at {}: {}", self.path, self.reason)
    }
}

impl Error for ConfigError {}

pub trait Validate {
    fn validate(&self) -> Result<(), ConfigError>;
}
