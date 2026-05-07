//! Optional component primitives for retained-ish, inspectable UI state.
//!
//! The component layer is intentionally small: it gives applications stable
//! IDs, explicit dirty-state tracking, and a trait shape for reusable UI
//! mechanics without hiding ratatui or requiring apps to adopt a framework.

use std::fmt;

use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

/// Stable identifier for a component instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ComponentId(String);

impl ComponentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ComponentId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ComponentId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for ComponentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable identifier for a focusable node owned by a component.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FocusNode {
    pub id: String,
}

impl FocusNode {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// Why a component needs work before its next render.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DirtyReason {
    Input,
    Resize,
    DataUpdate,
    Timer,
    Theme,
    ImagePlacement,
    Explicit,
    Custom(String),
}

/// Inspectable invalidation state for reusable UI components.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyState {
    paint: bool,
    layout: bool,
    image_placement: bool,
    reasons: Vec<DirtyReason>,
}

impl DirtyState {
    pub fn clean() -> Self {
        Self {
            paint: false,
            layout: false,
            image_placement: false,
            reasons: Vec::new(),
        }
    }

    pub fn paint(reason: DirtyReason) -> Self {
        let mut state = Self::clean();
        state.mark_paint(reason);
        state
    }

    pub fn layout(reason: DirtyReason) -> Self {
        let mut state = Self::clean();
        state.mark_layout(reason);
        state
    }

    pub fn image_placement(reason: DirtyReason) -> Self {
        let mut state = Self::clean();
        state.mark_image_placement(reason);
        state
    }

    pub fn is_clean(&self) -> bool {
        !self.paint && !self.layout && !self.image_placement
    }

    pub fn paint_dirty(&self) -> bool {
        self.paint
    }

    pub fn layout_dirty(&self) -> bool {
        self.layout
    }

    pub fn image_placement_dirty(&self) -> bool {
        self.image_placement
    }

    pub fn reasons(&self) -> &[DirtyReason] {
        &self.reasons
    }

    pub fn mark_paint(&mut self, reason: DirtyReason) {
        self.paint = true;
        self.push_reason(reason);
    }

    pub fn mark_layout(&mut self, reason: DirtyReason) {
        self.layout = true;
        self.paint = true;
        self.push_reason(reason);
    }

    pub fn mark_image_placement(&mut self, reason: DirtyReason) {
        self.image_placement = true;
        self.paint = true;
        self.push_reason(reason);
    }

    pub fn clear(&mut self) {
        *self = Self::clean();
    }

    fn push_reason(&mut self, reason: DirtyReason) {
        if !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
    }
}

impl Default for DirtyState {
    fn default() -> Self {
        Self::clean()
    }
}

/// Result of routing an event to a component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ComponentOutcome<Message> {
    Ignored,
    Handled,
    Message(Message),
    Messages(Vec<Message>),
}

impl<Message> ComponentOutcome<Message> {
    pub fn is_handled(&self) -> bool {
        !matches!(self, Self::Ignored)
    }
}

/// Lightweight child list for component tree inspection.
pub type ComponentChildren<'a> = &'a [ComponentId];

/// Optional trait for reusable UI mechanics.
///
/// Rendering remains ratatui-native. Apps may ignore this trait entirely and
/// continue to draw directly when a retained component model is not useful.
pub trait Component {
    type Event;
    type Message;

    fn id(&self) -> &ComponentId;

    fn render(&mut self, frame: &mut ratatui::Frame<'_>, area: Rect) -> anyhow::Result<()>;

    fn handle_event(
        &mut self,
        event: &Self::Event,
    ) -> anyhow::Result<ComponentOutcome<Self::Message>>;

    fn dirty(&self) -> &DirtyState;

    fn mark_dirty(&mut self, reason: DirtyReason);

    fn clear_dirty(&mut self);

    fn focus_node(&self) -> Option<FocusNode> {
        None
    }

    fn children(&self) -> ComponentChildren<'_> {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_state_tracks_distinct_invalidation_kinds() {
        let mut state = DirtyState::clean();
        assert!(state.is_clean());

        state.mark_layout(DirtyReason::Resize);
        state.mark_image_placement(DirtyReason::ImagePlacement);

        assert!(state.layout_dirty());
        assert!(state.paint_dirty());
        assert!(state.image_placement_dirty());
        assert_eq!(
            state.reasons(),
            &[DirtyReason::Resize, DirtyReason::ImagePlacement]
        );
    }

    #[test]
    fn repeated_dirty_reasons_are_deduplicated() {
        let mut state = DirtyState::clean();
        state.mark_paint(DirtyReason::Input);
        state.mark_paint(DirtyReason::Input);

        assert_eq!(state.reasons(), &[DirtyReason::Input]);
    }

    #[test]
    fn clearing_dirty_state_removes_flags_and_reasons() {
        let mut state = DirtyState::image_placement(DirtyReason::Explicit);
        state.clear();

        assert!(state.is_clean());
        assert!(state.reasons().is_empty());
    }

    #[test]
    fn component_outcome_reports_handling() {
        assert!(!ComponentOutcome::<()>::Ignored.is_handled());
        assert!(ComponentOutcome::<()>::Handled.is_handled());
    }
}
