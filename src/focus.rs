//! Explicit focus traversal and modal capture primitives.
//!
//! **Stability: experimental.** No in-tree consumer yet drives `FocusManager`.
//! The first c4tui port (rewiring mode-based event routing through this module)
//! will pressure-test the scope/traversal model.
//!
//! Focus remains policy-light: apps decide which events mean "move focus" or
//! "activate"; `tui-kit` provides stable IDs, inspectable scopes, traversal
//! mechanics, and noisy validation for ambiguous focus graphs.

use serde::{Deserialize, Serialize};

use crate::config::{ConfigError, Validate};

/// Stable identifier for a focusable UI node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FocusId(String);

impl FocusId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for FocusId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for FocusId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for FocusId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable focusable node metadata exposed by components and managers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusNode {
    pub id: FocusId,
    pub enabled: bool,
    pub visible: bool,
}

impl FocusNode {
    pub fn new(id: impl Into<FocusId>) -> Self {
        Self {
            id: id.into(),
            enabled: true,
            visible: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    pub fn focusable(&self) -> bool {
        self.enabled && self.visible
    }
}

/// Scope behavior for a group of focusable nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FocusScopeKind {
    /// Participates in normal traversal.
    Normal,
    /// Captures focus until the scope is popped, then restores prior focus.
    Modal,
    /// Captures and consumes focus routing without implying dialog semantics.
    Capturing,
}

/// App-selected traversal intent. Apps own key bindings; this enum owns only mechanics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FocusTraversal {
    Forward,
    Backward,
    Explicit(FocusId),
}

/// Explicit focus traversal configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusConfig {
    pub wrap: bool,
    pub restore_on_scope_pop: bool,
    pub require_initial_focus: bool,
}

impl FocusConfig {
    pub fn explicit() -> Self {
        Self {
            wrap: true,
            restore_on_scope_pop: true,
            require_initial_focus: true,
        }
    }

    pub fn headless_test() -> Self {
        Self {
            wrap: false,
            restore_on_scope_pop: false,
            require_initial_focus: false,
        }
    }
}

impl Validate for FocusConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FocusScope {
    id: FocusId,
    kind: FocusScopeKind,
    nodes: Vec<FocusNode>,
    previous_focus: Option<FocusId>,
}

/// Inspectable focus manager with stack-based modal/capturing scopes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusManager {
    config: FocusConfig,
    scopes: Vec<FocusScope>,
    current: Option<FocusId>,
}

impl FocusManager {
    pub fn new(config: FocusConfig, nodes: Vec<FocusNode>) -> Result<Self, ConfigError> {
        config.validate()?;
        let root = FocusScope {
            id: FocusId::new("root"),
            kind: FocusScopeKind::Normal,
            nodes,
            previous_focus: None,
        };
        validate_scope(&root, "focus.root")?;
        let current = root
            .nodes
            .iter()
            .find(|node| node.focusable())
            .map(|node| node.id.clone());
        if config.require_initial_focus && current.is_none() && !root.nodes.is_empty() {
            return Err(ConfigError::new(
                "focus.current",
                "initial focus is required but no visible enabled node exists",
            ));
        }
        Ok(Self {
            config,
            scopes: vec![root],
            current,
        })
    }

    pub fn current(&self) -> Option<&FocusId> {
        self.current.as_ref()
    }

    pub fn active_scope_kind(&self) -> FocusScopeKind {
        self.scopes
            .last()
            .map(|scope| scope.kind)
            .unwrap_or(FocusScopeKind::Normal)
    }

    pub fn push_scope(
        &mut self,
        id: impl Into<FocusId>,
        kind: FocusScopeKind,
        nodes: Vec<FocusNode>,
    ) -> Result<(), ConfigError> {
        let scope = FocusScope {
            id: id.into(),
            kind,
            nodes,
            previous_focus: self.current.clone(),
        };
        validate_scope(&scope, "focus.scope")?;
        let next_current = scope
            .nodes
            .iter()
            .find(|node| node.focusable())
            .map(|node| node.id.clone());
        if self.config.require_initial_focus && next_current.is_none() && !scope.nodes.is_empty() {
            return Err(ConfigError::new(
                "focus.current",
                "pushed scope has no visible enabled node for initial focus",
            ));
        }
        self.current = next_current;
        self.scopes.push(scope);
        Ok(())
    }

    pub fn pop_scope(&mut self) -> Option<FocusId> {
        if self.scopes.len() <= 1 {
            return None;
        }
        let popped = self.scopes.pop()?;
        if self.config.restore_on_scope_pop {
            self.current = popped.previous_focus;
        } else {
            self.current = self.active_nodes().first().map(|node| node.id.clone());
        }
        Some(popped.id)
    }

    pub fn traverse(&mut self, traversal: FocusTraversal) -> Option<&FocusId> {
        let next = match traversal {
            FocusTraversal::Explicit(id) => self
                .active_nodes()
                .iter()
                .find(|node| node.id == id && node.focusable())
                .map(|node| node.id.clone()),
            FocusTraversal::Forward => self.relative(1),
            FocusTraversal::Backward => self.relative(-1),
        };
        if let Some(next) = next {
            self.current = Some(next);
        }
        self.current()
    }

    fn relative(&self, delta: isize) -> Option<FocusId> {
        let nodes: Vec<&FocusNode> = self
            .active_nodes()
            .iter()
            .filter(|node| node.focusable())
            .collect();
        if nodes.is_empty() {
            return None;
        }
        let current_index = self
            .current
            .as_ref()
            .and_then(|current| nodes.iter().position(|node| &node.id == current))
            .unwrap_or(0);
        let raw = current_index as isize + delta;
        let next = if self.config.wrap {
            raw.rem_euclid(nodes.len() as isize) as usize
        } else if raw < 0 || raw >= nodes.len() as isize {
            current_index
        } else {
            raw as usize
        };
        Some(nodes[next].id.clone())
    }

    fn active_nodes(&self) -> &[FocusNode] {
        self.scopes
            .last()
            .map(|scope| scope.nodes.as_slice())
            .unwrap_or(&[])
    }
}

fn validate_scope(scope: &FocusScope, path: &'static str) -> Result<(), ConfigError> {
    if scope.id.as_str().trim().is_empty() {
        return Err(ConfigError::new(path, "focus scope IDs must not be empty"));
    }
    for node in &scope.nodes {
        if node.id.as_str().trim().is_empty() {
            return Err(ConfigError::new(path, "focus node IDs must not be empty"));
        }
    }
    for (index, node) in scope.nodes.iter().enumerate() {
        if scope.nodes[..index].iter().any(|seen| seen.id == node.id) {
            return Err(ConfigError::new(
                path,
                format!("duplicate focus node ID `{}`", node.id),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodes(ids: &[&str]) -> Vec<FocusNode> {
        ids.iter().map(|id| FocusNode::new(*id)).collect()
    }

    #[test]
    fn traverses_focus_forward_and_backward_with_wrap() {
        let mut manager =
            FocusManager::new(FocusConfig::explicit(), nodes(&["a", "b", "c"])).unwrap();

        assert_eq!(manager.current().unwrap().as_str(), "a");
        assert_eq!(
            manager.traverse(FocusTraversal::Forward).unwrap().as_str(),
            "b"
        );
        assert_eq!(
            manager.traverse(FocusTraversal::Forward).unwrap().as_str(),
            "c"
        );
        assert_eq!(
            manager.traverse(FocusTraversal::Forward).unwrap().as_str(),
            "a"
        );
        assert_eq!(
            manager.traverse(FocusTraversal::Backward).unwrap().as_str(),
            "c"
        );
    }

    #[test]
    fn modal_scope_captures_and_restores_focus() {
        let mut manager =
            FocusManager::new(FocusConfig::explicit(), nodes(&["main", "side"])).unwrap();
        manager.traverse(FocusTraversal::Explicit(FocusId::new("side")));

        manager
            .push_scope("dialog", FocusScopeKind::Modal, nodes(&["ok", "cancel"]))
            .unwrap();
        assert_eq!(manager.active_scope_kind(), FocusScopeKind::Modal);
        assert_eq!(manager.current().unwrap().as_str(), "ok");
        assert_eq!(
            manager.traverse(FocusTraversal::Forward).unwrap().as_str(),
            "cancel"
        );

        assert_eq!(manager.pop_scope().unwrap().as_str(), "dialog");
        assert_eq!(manager.current().unwrap().as_str(), "side");
    }

    #[test]
    fn validation_rejects_duplicate_nodes() {
        let error = FocusManager::new(FocusConfig::explicit(), nodes(&["dup", "dup"])).unwrap_err();
        assert_eq!(error.path, "focus.root");
        assert!(error.reason.contains("duplicate"));
    }

    #[test]
    fn skips_disabled_and_hidden_nodes() {
        let mut manager = FocusManager::new(
            FocusConfig::explicit(),
            vec![
                FocusNode::new("a").disabled(),
                FocusNode::new("b"),
                FocusNode::new("c").hidden(),
            ],
        )
        .unwrap();

        assert_eq!(manager.current().unwrap().as_str(), "b");
        assert_eq!(
            manager.traverse(FocusTraversal::Forward).unwrap().as_str(),
            "b"
        );
    }
}
