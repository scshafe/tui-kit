//! Optional component primitives for retained-ish, inspectable UI state.
//!
//! **Stability:** consumed by c4tui's `ViewPicker` (via [`BufferComponent`] +
//! [`Cached`]). The trait shape has been pressure-tested against a real
//! consumer; further breaking changes should be motivated by additional ports.
//!
//! The component layer is intentionally small: it gives applications stable
//! IDs, explicit dirty-state tracking, and a trait shape for reusable UI
//! mechanics without hiding ratatui or requiring apps to adopt a framework.

use std::fmt;

use ratatui::{buffer::Buffer, layout::Rect};
use serde::{Deserialize, Serialize};

use crate::focus::FocusNode;

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

/// Buffer-native component shape used by [`Cached`].
///
/// Most components should implement [`Component`] directly. Implement this
/// trait when rendering can be represented fully in a ratatui [`Buffer`] and is
/// therefore safe to replay without re-running component-specific render logic.
/// Terminal side effects such as image placement should remain explicit dirty
/// reasons and should not be hidden behind this cache.
pub trait BufferComponent {
    type Event;
    type Message;

    fn id(&self) -> &ComponentId;

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> anyhow::Result<()>;

    fn handle_event(
        &mut self,
        _event: &Self::Event,
    ) -> anyhow::Result<ComponentOutcome<Self::Message>> {
        Ok(ComponentOutcome::Ignored)
    }

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

impl<E, M> BufferComponent for Box<dyn BufferComponent<Event = E, Message = M>> {
    type Event = E;
    type Message = M;

    fn id(&self) -> &ComponentId {
        self.as_ref().id()
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> anyhow::Result<()> {
        self.as_mut().render_buffer(area, buffer)
    }

    fn handle_event(
        &mut self,
        event: &Self::Event,
    ) -> anyhow::Result<ComponentOutcome<Self::Message>> {
        self.as_mut().handle_event(event)
    }

    fn dirty(&self) -> &DirtyState {
        self.as_ref().dirty()
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.as_mut().mark_dirty(reason);
    }

    fn clear_dirty(&mut self) {
        self.as_mut().clear_dirty();
    }

    fn focus_node(&self) -> Option<FocusNode> {
        self.as_ref().focus_node()
    }

    fn children(&self) -> ComponentChildren<'_> {
        self.as_ref().children()
    }
}

/// Machine-readable render cache counters for [`Cached`].
///
/// `cache_misses` counts inner-component renders that populated the cache.
/// `cache_hits` counts replays that served the cached buffer without re-rendering.
/// Their sum is the total number of [`Cached::render_to_buffer`] calls.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedRenderStats {
    pub cache_misses: u64,
    pub cache_hits: u64,
}

/// Cached wrapper for buffer-native components.
///
/// The wrapper only reuses output for components that opt into
/// [`BufferComponent`], making the safety boundary explicit. Dirty state and
/// area changes invalidate the cache; clean components with an unchanged area
/// replay the previous buffer into the current frame.
#[derive(Debug, Clone)]
pub struct Cached<C> {
    inner: C,
    cache: Option<Buffer>,
    cached_area: Option<Rect>,
    stats: CachedRenderStats,
}

impl<C> Cached<C> {
    pub fn new(inner: C) -> Self {
        Self {
            inner,
            cache: None,
            cached_area: None,
            stats: CachedRenderStats::default(),
        }
    }

    pub fn inner(&self) -> &C {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut C {
        self.invalidate();
        &mut self.inner
    }

    pub fn into_inner(self) -> C {
        self.inner
    }

    pub fn cached_area(&self) -> Option<Rect> {
        self.cached_area
    }

    pub fn stats(&self) -> CachedRenderStats {
        self.stats
    }

    pub fn invalidate(&mut self) {
        self.cache = None;
        self.cached_area = None;
    }
}

impl<C> Cached<C>
where
    C: BufferComponent,
{
    pub fn handle_event(
        &mut self,
        event: &C::Event,
    ) -> anyhow::Result<ComponentOutcome<C::Message>> {
        self.inner.handle_event(event)
    }

    pub fn render_to_buffer(&mut self, area: Rect, target: &mut Buffer) -> anyhow::Result<()> {
        let needs_render = {
            let area_changed = self.cached_area != Some(area);
            self.cache.is_none() || area_changed || !self.inner.dirty().is_clean()
        };
        if needs_render {
            let mut buffer = Buffer::empty(area);
            self.inner.render_buffer(area, &mut buffer)?;
            self.inner.clear_dirty();
            self.cache = Some(buffer);
            self.cached_area = Some(area);
            self.stats.cache_misses += 1;
        } else {
            self.stats.cache_hits += 1;
        }
        if let Some(cache) = &self.cache {
            blit(cache, target);
        }
        Ok(())
    }
}

fn blit(source: &Buffer, target: &mut Buffer) {
    let area = *source.area();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let (Some(src), Some(dst)) = (source.cell((x, y)), target.cell_mut((x, y))) {
                *dst = src.clone();
            }
        }
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

    #[derive(Debug)]
    struct CountingBufferComponent {
        id: ComponentId,
        dirty: DirtyState,
        renders: usize,
    }

    impl CountingBufferComponent {
        fn new() -> Self {
            Self {
                id: ComponentId::new("counter"),
                dirty: DirtyState::paint(DirtyReason::Explicit),
                renders: 0,
            }
        }
    }

    impl BufferComponent for CountingBufferComponent {
        type Event = ();
        type Message = ();

        fn id(&self) -> &ComponentId {
            &self.id
        }

        fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> anyhow::Result<()> {
            self.renders += 1;
            let symbol = self.renders.to_string();
            if let Some(cell) = buffer.cell_mut((area.x, area.y)) {
                cell.set_symbol(&symbol);
            }
            Ok(())
        }

        fn handle_event(&mut self, _event: &Self::Event) -> anyhow::Result<ComponentOutcome<()>> {
            Ok(ComponentOutcome::Ignored)
        }

        fn dirty(&self) -> &DirtyState {
            &self.dirty
        }

        fn mark_dirty(&mut self, reason: DirtyReason) {
            self.dirty.mark_paint(reason);
        }

        fn clear_dirty(&mut self) {
            self.dirty.clear();
        }
    }

    #[test]
    fn cached_buffer_component_replays_clean_same_area_output() -> anyhow::Result<()> {
        let area = Rect::new(0, 0, 3, 1);
        let mut cached = Cached::new(CountingBufferComponent::new());
        let mut first = Buffer::empty(area);
        let mut second = Buffer::empty(area);

        cached.render_to_buffer(area, &mut first)?;
        cached.render_to_buffer(area, &mut second)?;

        assert_eq!(cached.inner().renders, 1);
        assert_eq!(cached.stats().cache_misses, 1);
        assert_eq!(cached.stats().cache_hits, 1);
        assert_eq!(first.cell((0, 0)), second.cell((0, 0)));
        Ok(())
    }

    #[test]
    fn cached_buffer_component_invalidates_on_dirty_or_area_change() -> anyhow::Result<()> {
        let area = Rect::new(0, 0, 3, 1);
        let larger = Rect::new(0, 0, 4, 1);
        let mut cached = Cached::new(CountingBufferComponent::new());
        let mut target = Buffer::empty(larger);

        cached.render_to_buffer(area, &mut target)?;
        cached.inner_mut().mark_dirty(DirtyReason::DataUpdate);
        cached.render_to_buffer(area, &mut target)?;
        cached.render_to_buffer(larger, &mut target)?;

        assert_eq!(cached.inner().renders, 3);
        assert_eq!(cached.stats().cache_misses, 3);
        assert_eq!(cached.stats().cache_hits, 0);
        assert_eq!(cached.cached_area(), Some(larger));
        Ok(())
    }

    #[test]
    fn boxed_buffer_component_forwards_trait_methods() -> anyhow::Result<()> {
        let area = Rect::new(0, 0, 1, 1);
        fn assert_buffer_component<T: BufferComponent>(_t: &T) {}
        let mut boxed: Box<dyn BufferComponent<Event = (), Message = ()>> =
            Box::new(CountingBufferComponent::new());
        assert_buffer_component(&boxed);

        assert_eq!(boxed.id().as_str(), "counter");
        assert!(!boxed.dirty().is_clean());

        let mut buffer = Buffer::empty(area);
        boxed.render_buffer(area, &mut buffer)?;
        boxed.clear_dirty();
        assert!(boxed.dirty().is_clean());

        boxed.mark_dirty(DirtyReason::Input);
        assert!(!boxed.dirty().is_clean());

        let outcome = boxed.handle_event(&())?;
        assert!(matches!(outcome, ComponentOutcome::Ignored));

        Ok(())
    }
}
