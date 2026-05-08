//! Data/update subscription primitives.
//!
//! This module intentionally stops short of a reactive framework. It provides
//! stable source/subscription identifiers, explicit subscription bookkeeping,
//! and machine-readable update events that can flow through the unified event
//! channel. Applications remain responsible for deciding what changed and how
//! to refresh domain state.

use crate::config::ConfigError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceId(String);

impl SourceId {
    pub fn new(id: impl Into<String>) -> Result<Self, ConfigError> {
        let id = id.into();
        validate_id("source.id", &id)?;
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SubscriptionId(String);

impl SubscriptionId {
    pub fn new(id: impl Into<String>) -> Result<Self, ConfigError> {
        let id = id.into();
        validate_id("subscription.id", &id)?;
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SubscriptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum UpdateEvent {
    SourceChanged { source: SourceId },
    SourceError { source: SourceId, message: String },
    SourceEnded { source: SourceId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionHandle {
    pub id: SubscriptionId,
    pub source: SourceId,
}

impl SubscriptionHandle {
    pub fn unsubscribe(self, registry: &mut SubscriptionRegistry) -> SubscriptionReport {
        registry.unsubscribe(&self.id)
    }
}

/// Declarative request for updates from a data source.
///
/// Components can expose these requests for app/runtime code to inspect and
/// apply explicitly. A request is inert data: constructing or dropping it does
/// not subscribe, unsubscribe, spawn workers, or imply a dependency graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionRequest {
    pub id: SubscriptionId,
    pub source: SourceId,
}

impl SubscriptionRequest {
    pub fn new(id: SubscriptionId, source: SourceId) -> Self {
        Self { id, source }
    }
}

/// Explicit subscription bookkeeping.
///
/// Dropping a [`SubscriptionHandle`] has no side effects. Apps must call
/// [`SubscriptionRegistry::unsubscribe`] (or [`SubscriptionHandle::unsubscribe`])
/// so subscription lifetime is visible in code and tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubscriptionRegistry {
    subscriptions: BTreeMap<SubscriptionId, SourceId>,
}

impl SubscriptionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(
        &mut self,
        id: SubscriptionId,
        source: SourceId,
    ) -> Result<SubscriptionHandle, ConfigError> {
        if self.subscriptions.contains_key(&id) {
            return Err(ConfigError::new(
                format!("subscriptions.{}", id.as_str()),
                "duplicate subscription id",
            ));
        }

        self.subscriptions.insert(id.clone(), source.clone());
        Ok(SubscriptionHandle { id, source })
    }

    pub fn subscribe_request(
        &mut self,
        request: SubscriptionRequest,
    ) -> Result<SubscriptionHandle, ConfigError> {
        self.subscribe(request.id, request.source)
    }

    pub fn unsubscribe(&mut self, id: &SubscriptionId) -> SubscriptionReport {
        match self.subscriptions.remove(id) {
            Some(source) => SubscriptionReport {
                id: id.clone(),
                source: Some(source),
                removed: true,
            },
            None => SubscriptionReport {
                id: id.clone(),
                source: None,
                removed: false,
            },
        }
    }

    pub fn is_subscribed(&self, id: &SubscriptionId) -> bool {
        self.subscriptions.contains_key(id)
    }

    pub fn source_for(&self, id: &SubscriptionId) -> Option<&SourceId> {
        self.subscriptions.get(id)
    }

    pub fn subscriptions_for_source<'a>(
        &'a self,
        source: &'a SourceId,
    ) -> impl Iterator<Item = &'a SubscriptionId> + 'a {
        self.subscriptions
            .iter()
            .filter_map(move |(id, candidate)| (candidate == source).then_some(id))
    }

    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionReport {
    pub id: SubscriptionId,
    pub source: Option<SourceId>,
    pub removed: bool,
}

fn validate_id(path: &'static str, id: &str) -> Result<(), ConfigError> {
    if id.trim().is_empty() {
        return Err(ConfigError::new(path, "id must not be empty"));
    }

    if id.chars().any(char::is_whitespace) {
        return Err(ConfigError::new(path, "id must not contain whitespace"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_validate_empty_and_whitespace_values() {
        assert!(SourceId::new("workspace").is_ok());
        assert_eq!(
            SourceId::new(" ").unwrap_err(),
            ConfigError::new("source.id", "id must not be empty")
        );
        assert_eq!(
            SubscriptionId::new("side panel").unwrap_err(),
            ConfigError::new("subscription.id", "id must not contain whitespace")
        );
    }

    #[test]
    fn registry_tracks_explicit_subscribe_and_unsubscribe() {
        let source = SourceId::new("workspace").unwrap();
        let id = SubscriptionId::new("thumbnail-cache").unwrap();
        let mut registry = SubscriptionRegistry::new();

        let handle = registry.subscribe(id.clone(), source.clone()).unwrap();

        assert!(registry.is_subscribed(&id));
        assert_eq!(registry.source_for(&id), Some(&source));
        assert_eq!(
            registry
                .subscriptions_for_source(&source)
                .collect::<Vec<_>>(),
            vec![&id]
        );

        let report = handle.unsubscribe(&mut registry);

        assert_eq!(
            report,
            SubscriptionReport {
                id: id.clone(),
                source: Some(source),
                removed: true,
            }
        );
        assert!(!registry.is_subscribed(&id));
    }

    #[test]
    fn registry_rejects_duplicate_subscription_ids() {
        let source = SourceId::new("workspace").unwrap();
        let id = SubscriptionId::new("view").unwrap();
        let mut registry = SubscriptionRegistry::new();

        registry.subscribe(id.clone(), source.clone()).unwrap();
        let err = registry.subscribe(id, source).unwrap_err();

        assert_eq!(
            err,
            ConfigError::new("subscriptions.view", "duplicate subscription id")
        );
    }

    #[test]
    fn subscription_requests_are_inert_until_explicitly_applied() {
        let source = SourceId::new("workspace").unwrap();
        let id = SubscriptionId::new("side-panel").unwrap();
        let request = SubscriptionRequest::new(id.clone(), source.clone());
        let mut registry = SubscriptionRegistry::new();

        assert!(registry.is_empty());

        let handle = registry.subscribe_request(request).unwrap();

        assert_eq!(handle, SubscriptionHandle { id, source });
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn missing_unsubscribe_is_machine_readable_noop() {
        let id = SubscriptionId::new("missing").unwrap();
        let mut registry = SubscriptionRegistry::new();

        assert_eq!(
            registry.unsubscribe(&id),
            SubscriptionReport {
                id,
                source: None,
                removed: false,
            }
        );
    }
}
