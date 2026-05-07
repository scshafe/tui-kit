//! Policy-light tree state and flattened viewport math.
//!
//! [`TreeState`] owns reusable tree mechanics only: stable node IDs,
//! expand/collapse state, visible flattened projection, optional selection,
//! optional tri-state checkboxes, and configurable key actions. Applications
//! own node meaning, lazy loading, command semantics, and rendering style.

use crate::config::{ConfigError, Validate};
use crate::input::Key;
use crate::keymap::{KeyMap, KeyTrigger, SpecialKey};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TreeNodeId(pub String);

impl TreeNodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeCheckboxState {
    Unchecked,
    Checked,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode<T = ()> {
    pub id: TreeNodeId,
    pub label: String,
    pub payload: T,
    pub children: Vec<TreeNode<T>>,
    pub children_loaded: bool,
    pub checkbox: Option<TreeCheckboxState>,
}

impl<T> TreeNode<T> {
    pub fn leaf(id: impl Into<String>, label: impl Into<String>, payload: T) -> Self {
        Self {
            id: TreeNodeId::new(id),
            label: label.into(),
            payload,
            children: Vec::new(),
            children_loaded: true,
            checkbox: None,
        }
    }

    pub fn branch(
        id: impl Into<String>,
        label: impl Into<String>,
        payload: T,
        children: Vec<TreeNode<T>>,
    ) -> Self {
        Self {
            id: TreeNodeId::new(id),
            label: label.into(),
            payload,
            children,
            children_loaded: true,
            checkbox: None,
        }
    }

    pub fn lazy_branch(id: impl Into<String>, label: impl Into<String>, payload: T) -> Self {
        Self {
            id: TreeNodeId::new(id),
            label: label.into(),
            payload,
            children: Vec::new(),
            children_loaded: false,
            checkbox: None,
        }
    }

    pub fn with_checkbox(mut self, state: TreeCheckboxState) -> Self {
        self.checkbox = Some(state);
        self
    }

    pub fn is_expandable(&self) -> bool {
        !self.children_loaded || !self.children.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeSelectionMode {
    None,
    Single,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeCheckboxMode {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeAction {
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    Expand,
    Collapse,
    ToggleExpanded,
    ScrollToTop,
    ScrollToBottom,
    Select,
    ToggleCheckbox,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct TreeConfig {
    pub selection: TreeSelectionMode,
    pub checkbox_mode: TreeCheckboxMode,
    pub page_stride: u16,
    pub actions: KeyMap<TreeAction>,
}

impl TreeConfig {
    pub fn explicit(actions: KeyMap<TreeAction>) -> Self {
        Self {
            selection: TreeSelectionMode::Single,
            checkbox_mode: TreeCheckboxMode::Disabled,
            page_stride: 10,
            actions,
        }
    }

    pub fn default_navigation() -> Self {
        let mut actions = KeyMap::new();
        actions
            .bind(KeyTrigger::Special(SpecialKey::Up), TreeAction::MoveUp)
            .bind(KeyTrigger::Special(SpecialKey::Down), TreeAction::MoveDown)
            .bind(KeyTrigger::Special(SpecialKey::Right), TreeAction::Expand)
            .bind(KeyTrigger::Special(SpecialKey::Left), TreeAction::Collapse)
            .bind(KeyTrigger::Special(SpecialKey::Enter), TreeAction::Select)
            .bind(KeyTrigger::Char(' '), TreeAction::ToggleExpanded);
        Self::explicit(actions)
    }
}

impl Default for TreeConfig {
    fn default() -> Self {
        Self::default_navigation()
    }
}

impl Validate for TreeConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.page_stride == 0 {
            return Err(ConfigError::new(
                "TreeConfig.page_stride",
                "must be greater than zero",
            ));
        }
        if self.actions.is_empty() {
            return Err(ConfigError::new(
                "TreeConfig.actions",
                "must install an explicit key policy; use TreeConfig::default_navigation() for the built-in preset",
            ));
        }
        for binding in self.actions.bindings() {
            if let TreeAction::Custom(name) = &binding.command {
                if name.trim().is_empty() {
                    return Err(ConfigError::new(
                        "TreeConfig.actions[].Custom",
                        "must not be empty",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeVisibleNode {
    pub id: TreeNodeId,
    pub depth: u16,
    pub index_path: Vec<usize>,
    pub row_index: usize,
    pub is_expanded: bool,
    pub is_expandable: bool,
    pub children_loaded: bool,
    pub checkbox: Option<TreeCheckboxState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeViewport {
    pub offset: usize,
    pub height: u16,
    pub start: usize,
    pub end_exclusive: usize,
    pub total_visible_nodes: usize,
    pub selected_row_index: Option<usize>,
    pub visible_nodes: Vec<TreeVisibleNode>,
    pub can_scroll_up: bool,
    pub can_scroll_down: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeOutcome {
    Continue,
    Selected(TreeNodeId),
    ExpansionChanged {
        id: TreeNodeId,
        expanded: bool,
    },
    NeedsChildren(TreeNodeId),
    CheckboxChanged {
        id: TreeNodeId,
        state: TreeCheckboxState,
    },
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct TreeState<T = ()> {
    roots: Vec<TreeNode<T>>,
    config: TreeConfig,
    expanded: std::collections::HashSet<TreeNodeId>,
    selected_row_index: Option<usize>,
    scroll_offset: usize,
}

impl<T> TreeState<T> {
    pub fn new(roots: Vec<TreeNode<T>>, config: TreeConfig) -> Self {
        Self::try_new(roots, config).unwrap_or_else(|err| panic!("invalid tree config: {err}"))
    }

    pub fn try_new(roots: Vec<TreeNode<T>>, config: TreeConfig) -> Result<Self, ConfigError> {
        config.validate()?;
        validate_nodes(&roots)?;
        let selected_row_index = match config.selection {
            TreeSelectionMode::None => None,
            TreeSelectionMode::Single if roots.is_empty() => None,
            TreeSelectionMode::Single => Some(0),
        };
        Ok(Self {
            roots,
            config,
            expanded: std::collections::HashSet::new(),
            selected_row_index,
            scroll_offset: 0,
        })
    }

    pub fn roots(&self) -> &[TreeNode<T>] {
        &self.roots
    }

    pub fn config(&self) -> &TreeConfig {
        &self.config
    }

    pub fn selected_row_index(&self) -> Option<usize> {
        self.selected_row_index
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn is_expanded(&self, id: &TreeNodeId) -> bool {
        self.expanded.contains(id)
    }

    pub fn selected_node(&self) -> Option<&TreeNode<T>> {
        let visible = self.flatten_all();
        let selected = visible.get(self.selected_row_index?)?;
        get_node(&self.roots, &selected.index_path)
    }

    pub fn handle_key(&mut self, key: Key, viewport_height: u16) -> TreeOutcome {
        if let Some(action) = self.config.actions.lookup(key) {
            self.handle_action(action, viewport_height)
        } else {
            TreeOutcome::Continue
        }
    }

    pub fn handle_action(&mut self, action: TreeAction, viewport_height: u16) -> TreeOutcome {
        match action {
            TreeAction::MoveUp => self.move_selection_or_scroll(-1, viewport_height),
            TreeAction::MoveDown => self.move_selection_or_scroll(1, viewport_height),
            TreeAction::PageUp => self.page(-1, viewport_height),
            TreeAction::PageDown => self.page(1, viewport_height),
            TreeAction::Expand => return self.expand_selected(),
            TreeAction::Collapse => return self.collapse_selected(),
            TreeAction::ToggleExpanded => return self.toggle_selected_expanded(),
            TreeAction::ScrollToTop => {
                self.scroll_offset = 0;
                if self.config.selection == TreeSelectionMode::Single && !self.roots.is_empty() {
                    self.selected_row_index = Some(0);
                }
            }
            TreeAction::ScrollToBottom => {
                let total = self.flatten_all().len();
                self.scroll_offset = max_offset(total, viewport_height);
                if self.config.selection == TreeSelectionMode::Single && total > 0 {
                    self.selected_row_index = Some(total - 1);
                }
            }
            TreeAction::Select => {
                if let Some(node) = self.selected_node() {
                    return TreeOutcome::Selected(node.id.clone());
                }
            }
            TreeAction::ToggleCheckbox => return self.toggle_selected_checkbox(),
            TreeAction::Custom(name) => return TreeOutcome::Custom(name),
        }
        TreeOutcome::Continue
    }

    pub fn viewport(&self, height: u16) -> TreeViewport {
        let flattened = self.flatten_all();
        let total_visible_nodes = flattened.len();
        let max_offset = max_offset(total_visible_nodes, height);
        let offset = self.scroll_offset.min(max_offset);
        let start = offset.min(total_visible_nodes);
        let end_exclusive = start
            .saturating_add(usize::from(height))
            .min(total_visible_nodes);
        TreeViewport {
            offset,
            height,
            start,
            end_exclusive,
            total_visible_nodes,
            selected_row_index: self.selected_row_index,
            visible_nodes: flattened[start..end_exclusive].to_vec(),
            can_scroll_up: offset > 0,
            can_scroll_down: offset < max_offset,
        }
    }

    pub fn set_scroll_offset(&mut self, offset: usize, viewport_height: u16) {
        self.scroll_offset = offset.min(max_offset(self.flatten_all().len(), viewport_height));
    }

    pub fn set_expanded(&mut self, id: TreeNodeId, expanded: bool, viewport_height: u16) {
        if expanded {
            self.expanded.insert(id);
        } else {
            self.expanded.remove(&id);
        }
        self.clamp_after_projection_change(viewport_height);
    }

    fn expand_selected(&mut self) -> TreeOutcome {
        let Some(node) = self.selected_node() else {
            return TreeOutcome::Continue;
        };
        if !node.is_expandable() {
            return TreeOutcome::Continue;
        }
        let id = node.id.clone();
        let needs_children = !node.children_loaded;
        self.expanded.insert(id.clone());
        if needs_children {
            TreeOutcome::NeedsChildren(id)
        } else {
            TreeOutcome::ExpansionChanged { id, expanded: true }
        }
    }

    fn collapse_selected(&mut self) -> TreeOutcome {
        let visible = self.flatten_all();
        let Some(selected) = visible.get(self.selected_row_index.unwrap_or(0)) else {
            return TreeOutcome::Continue;
        };
        if self.expanded.remove(&selected.id) {
            TreeOutcome::ExpansionChanged {
                id: selected.id.clone(),
                expanded: false,
            }
        } else if selected.depth > 0 {
            let parent_path = &selected.index_path[..selected.index_path.len() - 1];
            if let Some(parent_row) = visible
                .iter()
                .position(|candidate| candidate.index_path == parent_path)
            {
                self.selected_row_index = Some(parent_row);
            }
            TreeOutcome::Continue
        } else {
            TreeOutcome::Continue
        }
    }

    fn toggle_selected_expanded(&mut self) -> TreeOutcome {
        let Some(node) = self.selected_node() else {
            return TreeOutcome::Continue;
        };
        if self.expanded.contains(&node.id) {
            self.collapse_selected()
        } else {
            self.expand_selected()
        }
    }

    fn toggle_selected_checkbox(&mut self) -> TreeOutcome {
        if self.config.checkbox_mode == TreeCheckboxMode::Disabled {
            return TreeOutcome::Continue;
        }
        let visible = self.flatten_all();
        let Some(selected) = visible.get(self.selected_row_index.unwrap_or(0)) else {
            return TreeOutcome::Continue;
        };
        let Some(node) = get_node_mut(&mut self.roots, &selected.index_path) else {
            return TreeOutcome::Continue;
        };
        let Some(current) = node.checkbox else {
            return TreeOutcome::Continue;
        };
        let next = match current {
            TreeCheckboxState::Unchecked | TreeCheckboxState::Mixed => TreeCheckboxState::Checked,
            TreeCheckboxState::Checked => TreeCheckboxState::Unchecked,
        };
        node.checkbox = Some(next);
        TreeOutcome::CheckboxChanged {
            id: node.id.clone(),
            state: next,
        }
    }

    fn move_selection_or_scroll(&mut self, delta: isize, viewport_height: u16) {
        let total = self.flatten_all().len();
        if self.config.selection == TreeSelectionMode::None {
            self.scroll_by(delta, viewport_height);
            return;
        }
        let Some(current) = self.selected_row_index else {
            if total > 0 {
                self.selected_row_index = Some(0);
            }
            return;
        };
        if total == 0 {
            self.selected_row_index = None;
            return;
        }
        self.selected_row_index = Some(current.saturating_add_signed(delta).min(total - 1));
        self.keep_selection_visible(viewport_height);
    }

    fn page(&mut self, direction: isize, viewport_height: u16) {
        let stride = usize::from(self.config.page_stride.min(viewport_height.max(1)));
        let delta = direction.saturating_mul(stride as isize);
        if self.config.selection == TreeSelectionMode::Single {
            self.move_selection_or_scroll(delta, viewport_height);
        } else {
            self.scroll_by(delta, viewport_height);
        }
    }

    fn scroll_by(&mut self, delta: isize, viewport_height: u16) {
        let total = self.flatten_all().len();
        self.scroll_offset = self
            .scroll_offset
            .saturating_add_signed(delta)
            .min(max_offset(total, viewport_height));
    }

    fn keep_selection_visible(&mut self, viewport_height: u16) {
        let Some(selected) = self.selected_row_index else {
            return;
        };
        let total = self.flatten_all().len();
        if viewport_height == 0 {
            self.scroll_offset = selected.min(max_offset(total, viewport_height));
            return;
        }
        let height = usize::from(viewport_height);
        if selected < self.scroll_offset {
            self.scroll_offset = selected;
        } else if selected >= self.scroll_offset.saturating_add(height) {
            self.scroll_offset = selected.saturating_add(1).saturating_sub(height);
        }
        self.scroll_offset = self.scroll_offset.min(max_offset(total, viewport_height));
    }

    fn clamp_after_projection_change(&mut self, viewport_height: u16) {
        let total = self.flatten_all().len();
        self.scroll_offset = self.scroll_offset.min(max_offset(total, viewport_height));
        if let Some(selected) = self.selected_row_index {
            self.selected_row_index = if total == 0 {
                None
            } else {
                Some(selected.min(total - 1))
            };
        }
    }

    fn flatten_all(&self) -> Vec<TreeVisibleNode> {
        let mut flattened = Vec::new();
        flatten_nodes(
            &self.roots,
            &self.expanded,
            0,
            &mut Vec::new(),
            &mut flattened,
        );
        flattened
    }
}

pub fn max_offset(total_visible_nodes: usize, viewport_height: u16) -> usize {
    total_visible_nodes.saturating_sub(usize::from(viewport_height))
}

fn flatten_nodes<T>(
    nodes: &[TreeNode<T>],
    expanded: &std::collections::HashSet<TreeNodeId>,
    depth: u16,
    path: &mut Vec<usize>,
    flattened: &mut Vec<TreeVisibleNode>,
) {
    for (index, node) in nodes.iter().enumerate() {
        path.push(index);
        let is_expanded = expanded.contains(&node.id);
        let row_index = flattened.len();
        flattened.push(TreeVisibleNode {
            id: node.id.clone(),
            depth,
            index_path: path.clone(),
            row_index,
            is_expanded,
            is_expandable: node.is_expandable(),
            children_loaded: node.children_loaded,
            checkbox: node.checkbox,
        });
        if is_expanded && node.children_loaded {
            flatten_nodes(
                &node.children,
                expanded,
                depth.saturating_add(1),
                path,
                flattened,
            );
        }
        path.pop();
    }
}

fn validate_nodes<T>(nodes: &[TreeNode<T>]) -> Result<(), ConfigError> {
    let mut ids = std::collections::HashSet::new();
    validate_nodes_at(nodes, &mut ids, "TreeState.roots")
}

fn validate_nodes_at<T>(
    nodes: &[TreeNode<T>],
    ids: &mut std::collections::HashSet<TreeNodeId>,
    path: &str,
) -> Result<(), ConfigError> {
    for (index, node) in nodes.iter().enumerate() {
        let node_path = format!("{path}[{index}]");
        if node.id.0.trim().is_empty() {
            return Err(ConfigError::new(
                format!("{node_path}.id"),
                "must not be empty",
            ));
        }
        if !ids.insert(node.id.clone()) {
            return Err(ConfigError::new(
                format!("{node_path}.id"),
                "must be unique",
            ));
        }
        if !node.children_loaded && !node.children.is_empty() {
            return Err(ConfigError::new(
                format!("{node_path}.children_loaded"),
                "lazy nodes must not contain preloaded children",
            ));
        }
        validate_nodes_at(&node.children, ids, &format!("{node_path}.children"))?;
    }
    Ok(())
}

fn get_node<'a, T>(nodes: &'a [TreeNode<T>], path: &[usize]) -> Option<&'a TreeNode<T>> {
    let (first, rest) = path.split_first()?;
    let node = nodes.get(*first)?;
    if rest.is_empty() {
        Some(node)
    } else {
        get_node(&node.children, rest)
    }
}

fn get_node_mut<'a, T>(
    nodes: &'a mut [TreeNode<T>],
    path: &[usize],
) -> Option<&'a mut TreeNode<T>> {
    let (first, rest) = path.split_first()?;
    let node = nodes.get_mut(*first)?;
    if rest.is_empty() {
        Some(node)
    } else {
        get_node_mut(&mut node.children, rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Vec<TreeNode> {
        vec![
            TreeNode::branch(
                "root",
                "Root",
                (),
                vec![
                    TreeNode::leaf("child-a", "Child A", ())
                        .with_checkbox(TreeCheckboxState::Unchecked),
                    TreeNode::lazy_branch("child-b", "Child B", ()),
                ],
            ),
            TreeNode::leaf("sibling", "Sibling", ()),
        ]
    }

    #[test]
    fn validation_requires_explicit_actions() {
        let err = TreeConfig::explicit(KeyMap::new()).validate().unwrap_err();
        assert_eq!(err.path, "TreeConfig.actions");
    }

    #[test]
    fn validation_rejects_duplicate_ids() {
        let roots = vec![TreeNode::branch(
            "dup",
            "Root",
            (),
            vec![TreeNode::leaf("dup", "Child", ())],
        )];
        let err = TreeState::try_new(roots, TreeConfig::default_navigation()).unwrap_err();
        assert_eq!(err.path, "TreeState.roots[0].children[0].id");
    }

    #[test]
    fn expansion_updates_flattened_projection() {
        let mut state = TreeState::new(tree(), TreeConfig::default_navigation());
        assert_eq!(state.viewport(10).total_visible_nodes, 2);
        assert_eq!(
            state.handle_action(TreeAction::Expand, 10),
            TreeOutcome::ExpansionChanged {
                id: TreeNodeId::new("root"),
                expanded: true
            }
        );
        let viewport = state.viewport(10);
        assert_eq!(viewport.total_visible_nodes, 4);
        assert_eq!(viewport.visible_nodes[1].id, TreeNodeId::new("child-a"));
        assert_eq!(viewport.visible_nodes[1].depth, 1);
    }

    #[test]
    fn lazy_expansion_reports_needs_children() {
        let mut state = TreeState::new(tree(), TreeConfig::default_navigation());
        state.handle_action(TreeAction::Expand, 10);
        state.handle_action(TreeAction::MoveDown, 10);
        state.handle_action(TreeAction::MoveDown, 10);
        assert_eq!(
            state.handle_action(TreeAction::Expand, 10),
            TreeOutcome::NeedsChildren(TreeNodeId::new("child-b"))
        );
    }

    #[test]
    fn selection_scrolls_into_view() {
        let roots = (0..8)
            .map(|idx| TreeNode::leaf(format!("node-{idx}"), format!("Node {idx}"), ()))
            .collect();
        let mut state = TreeState::new(roots, TreeConfig::default_navigation());
        for _ in 0..4 {
            state.handle_action(TreeAction::MoveDown, 3);
        }
        let viewport = state.viewport(3);
        assert_eq!(state.selected_row_index(), Some(4));
        assert_eq!(viewport.start, 2);
        assert_eq!(viewport.end_exclusive, 5);
    }

    #[test]
    fn checkbox_toggle_is_explicitly_enabled() {
        let mut config = TreeConfig::default_navigation();
        config.checkbox_mode = TreeCheckboxMode::Enabled;
        let mut state = TreeState::new(tree(), config);
        state.handle_action(TreeAction::Expand, 10);
        state.handle_action(TreeAction::MoveDown, 10);
        assert_eq!(
            state.handle_action(TreeAction::ToggleCheckbox, 10),
            TreeOutcome::CheckboxChanged {
                id: TreeNodeId::new("child-a"),
                state: TreeCheckboxState::Checked,
            }
        );
    }

    #[test]
    fn key_actions_are_configurable() {
        let mut actions = KeyMap::new();
        actions.bind(
            KeyTrigger::Special(SpecialKey::Tab),
            TreeAction::Custom("next-pane".into()),
        );
        let mut state = TreeState::new(tree(), TreeConfig::explicit(actions));
        assert_eq!(
            state.handle_key(Key::Tab, 1),
            TreeOutcome::Custom("next-pane".into())
        );
    }
}
