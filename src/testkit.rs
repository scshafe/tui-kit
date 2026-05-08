//! Test harness helpers for widgets, event streams, and image side effects.
//!
//! These helpers are deliberately small and deterministic. They let apps and
//! crate tests exercise ratatui-native widgets without opening a terminal,
//! route typed input/resize events through the same [`crate::events::AppEvent`]
//! shape used at runtime, and assert image lifecycle calls without emitting
//! terminal escape sequences.

use crate::events::AppEvent;
use crate::image::{ImageCapabilities, ImageSurface, PlaceOptions};
use crate::input::Key;
use crate::layout::PixelSize;
use crate::scheduler::{Completion, Priority, RequestScope};
use anyhow::Result as AnyhowResult;
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{StatefulWidget, Widget};
use std::collections::{BinaryHeap, HashSet, VecDeque};
use std::convert::Infallible;

type TestWorkExecutor<Item, Out> = Box<dyn Fn(&Item) -> AnyhowResult<Out> + Send + Sync>;

/// Render a ratatui [`Widget`] into an owned [`Buffer`] for snapshot-style tests.
pub fn render_widget<W: Widget>(widget: W, area: Rect) -> Buffer {
    let mut buffer = Buffer::empty(area);
    widget.render(area, &mut buffer);
    buffer
}

/// Render a ratatui [`StatefulWidget`] into an owned [`Buffer`].
pub fn render_stateful_widget<W>(widget: W, area: Rect, state: &mut W::State) -> Buffer
where
    W: StatefulWidget,
{
    let mut buffer = Buffer::empty(area);
    widget.render(area, &mut buffer, state);
    buffer
}

/// A deterministic event script for driving app/widget event handlers in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventScript<UserEvent = Infallible> {
    events: Vec<AppEvent<UserEvent>>,
}

impl<UserEvent> EventScript<UserEvent> {
    pub fn new(events: impl IntoIterator<Item = AppEvent<UserEvent>>) -> Self {
        Self {
            events: events.into_iter().collect(),
        }
    }

    pub fn push(&mut self, event: AppEvent<UserEvent>) {
        self.events.push(event);
    }

    pub fn events(&self) -> &[AppEvent<UserEvent>] {
        &self.events
    }

    pub fn into_events(self) -> Vec<AppEvent<UserEvent>> {
        self.events
    }
}

impl EventScript<Infallible> {
    /// Build a script from keyboard mechanics only.
    pub fn keys(keys: impl IntoIterator<Item = Key>) -> Self {
        Self::new(keys.into_iter().map(AppEvent::input_key))
    }

    /// Build a one-event resize script using the runtime event category.
    pub fn resize(cols: u16, rows: u16) -> Self {
        Self::new([AppEvent::terminal_resize(cols, rows)])
    }
}

/// A single image lifecycle call captured by [`MockImageSurface`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MockImageCall {
    EnsureLoaded { image_id: u32, bytes: usize },
    Place(PlaceOptions),
    DeletePlacement { placement_id: u32 },
    DeleteAllPlacements,
    ForgetAll,
    Flush,
}

/// Image surface test double that records lifecycle calls and emits no escapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockImageSurface {
    capabilities: ImageCapabilities,
    calls: Vec<MockImageCall>,
}

impl MockImageSurface {
    pub fn new(capabilities: ImageCapabilities) -> Self {
        Self {
            capabilities,
            calls: Vec::new(),
        }
    }

    pub fn kitty_like() -> Self {
        Self::new(ImageCapabilities::kitty())
    }

    pub fn noop_like() -> Self {
        Self::new(ImageCapabilities::noop())
    }

    pub fn calls(&self) -> &[MockImageCall] {
        &self.calls
    }

    pub fn take_calls(&mut self) -> Vec<MockImageCall> {
        std::mem::take(&mut self.calls)
    }
}

impl Default for MockImageSurface {
    fn default() -> Self {
        Self::kitty_like()
    }
}

impl ImageSurface for MockImageSurface {
    fn capabilities(&self) -> ImageCapabilities {
        self.capabilities.clone()
    }

    fn ensure_loaded(&mut self, image_id: u32, png: &[u8]) -> Result<()> {
        self.calls.push(MockImageCall::EnsureLoaded {
            image_id,
            bytes: png.len(),
        });
        Ok(())
    }

    fn place(&mut self, opts: PlaceOptions) -> Result<()> {
        self.calls.push(MockImageCall::Place(opts));
        Ok(())
    }

    fn delete_placement(&mut self, placement_id: u32) -> Result<()> {
        self.calls
            .push(MockImageCall::DeletePlacement { placement_id });
        Ok(())
    }

    fn delete_all_placements(&mut self) -> Result<()> {
        self.calls.push(MockImageCall::DeleteAllPlacements);
        Ok(())
    }

    fn forget_all(&mut self) -> Result<()> {
        self.calls.push(MockImageCall::ForgetAll);
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }
}

/// Convert a rectangle width/height into a zero-origin ratatui area.
pub fn test_area(width: u16, height: u16) -> Rect {
    Rect::new(0, 0, width, height)
}

/// Convert terminal dimensions into cell-sized pixel metrics for placement tests.
pub fn test_cell_pixels(width: u32, height: u32) -> PixelSize {
    PixelSize { width, height }
}

/// Deterministic single-threaded scheduler double for tests.
///
/// It mirrors the production scheduler's priority/FIFO ordering and completion
/// shape, but executes work only when [`DeterministicScheduler::run_one`] or
/// [`DeterministicScheduler::run_all`] is called. No threads are spawned and no
/// wakeup events are sent implicitly, which keeps app-shell tests stable.
pub struct DeterministicScheduler<Item, Out, P: Ord + Clone = Priority> {
    queue: BinaryHeap<TestScheduledRequest<Item, P>>,
    queued: HashSet<u64>,
    completed: HashSet<u64>,
    completions: VecDeque<Completion<Out>>,
    executor: TestWorkExecutor<Item, Out>,
    seq: u64,
}

impl<Item, Out, P: Ord + Clone> std::fmt::Debug for DeterministicScheduler<Item, Out, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeterministicScheduler")
            .field("queued", &self.queue.len())
            .field("completed", &self.completed.len())
            .field("completions", &self.completions.len())
            .finish_non_exhaustive()
    }
}

impl<Item, Out, P: Ord + Clone> DeterministicScheduler<Item, Out, P> {
    pub fn new<F>(executor: F) -> Self
    where
        F: Fn(&Item) -> AnyhowResult<Out> + Send + Sync + 'static,
    {
        Self {
            queue: BinaryHeap::new(),
            queued: HashSet::new(),
            completed: HashSet::new(),
            completions: VecDeque::new(),
            executor: Box::new(executor),
            seq: 0,
        }
    }

    /// Queue a request with production scheduler de-duplication semantics.
    pub fn request(&mut self, id: u64, priority: P, item: Item) {
        self.request_scoped(id, priority, item, RequestScope::default());
    }

    /// Queue a scoped request. The scope is retained for inspection and future
    /// extension; this deterministic double intentionally does not own app
    /// cancellation policy beyond explicit request IDs.
    pub fn request_scoped(&mut self, id: u64, priority: P, item: Item, scope: RequestScope) {
        if self.completed.contains(&id) || self.queued.contains(&id) {
            return;
        }
        let seq = self.seq;
        self.seq += 1;
        self.queued.insert(id);
        self.queue.push(TestScheduledRequest {
            priority,
            seq,
            id,
            scope,
            item,
        });
    }

    /// Remove a queued request before execution.
    pub fn cancel_id(&mut self, id: u64) -> bool {
        if !self.queued.remove(&id) {
            return false;
        }
        self.queue = self
            .queue
            .drain()
            .filter(|request| request.id != id)
            .collect();
        true
    }

    /// Execute the next queued request, returning its ID when work ran.
    pub fn run_one(&mut self) -> Option<u64> {
        let request = self.queue.pop()?;
        self.queued.remove(&request.id);
        let id = request.id;
        let result = (self.executor)(&request.item);
        self.completed.insert(id);
        self.completions.push_back(Completion { id, result });
        Some(id)
    }

    /// Execute all currently queued requests in deterministic priority/FIFO
    /// order.
    pub fn run_all(&mut self) -> Vec<u64> {
        let mut ran = Vec::new();
        while let Some(id) = self.run_one() {
            ran.push(id);
        }
        ran
    }

    pub fn drain(&mut self) -> Vec<Completion<Out>> {
        std::mem::take(&mut self.completions).into_iter().collect()
    }

    pub fn queued_len(&self) -> usize {
        self.queue.len()
    }
}

struct TestScheduledRequest<Item, P> {
    priority: P,
    seq: u64,
    id: u64,
    #[allow(dead_code)]
    scope: RequestScope,
    item: Item,
}

impl<Item, P: Ord> PartialEq for TestScheduledRequest<Item, P> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}
impl<Item, P: Ord> Eq for TestScheduledRequest<Item, P> {}
impl<Item, P: Ord> PartialOrd for TestScheduledRequest<Item, P> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<Item, P: Ord> Ord for TestScheduledRequest<Item, P> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Line;

    #[test]
    fn render_widget_returns_deterministic_buffer() {
        let buffer = render_widget(Line::from("hi"), test_area(4, 1));

        assert_eq!(buffer[(0, 0)].symbol(), "h");
        assert_eq!(buffer[(1, 0)].symbol(), "i");
    }

    #[test]
    fn event_script_keeps_typed_event_categories() {
        let script = EventScript::keys([Key::Down, Key::Enter]);

        assert_eq!(script.events()[0], AppEvent::input_key(Key::Down));
        assert_eq!(script.events()[1], AppEvent::input_key(Key::Enter));
    }

    #[test]
    fn mock_image_surface_records_lifecycle_calls() {
        let mut surface = MockImageSurface::default();
        let opts = PlaceOptions {
            image_id: 7,
            placement_id: 9,
            source: crate::layout::PixelRect {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            },
            cell_cols: 5,
            cell_rows: 6,
        };

        surface.ensure_loaded(7, b"png").unwrap();
        surface.place(opts).unwrap();
        surface.delete_placement(9).unwrap();

        assert_eq!(
            surface.calls(),
            &[
                MockImageCall::EnsureLoaded {
                    image_id: 7,
                    bytes: 3
                },
                MockImageCall::Place(opts),
                MockImageCall::DeletePlacement { placement_id: 9 }
            ]
        );
    }

    #[test]
    fn deterministic_scheduler_runs_priority_fifo_without_threads() {
        let mut scheduler = DeterministicScheduler::new(|item: &i32| Ok(item * 10));

        scheduler.request(1, Priority::Background, 1);
        scheduler.request(2, Priority::Active, 2);
        scheduler.request(3, Priority::Active, 3);

        assert_eq!(scheduler.run_all(), vec![2, 3, 1]);
        let completions = scheduler.drain();
        let ids: Vec<_> = completions.iter().map(|completion| completion.id).collect();
        let values: Vec<_> = completions
            .iter()
            .map(|completion| *completion.result.as_ref().unwrap())
            .collect();
        assert_eq!(ids, vec![2, 3, 1]);
        assert_eq!(values, vec![20, 30, 10]);
    }

    #[test]
    fn deterministic_scheduler_removes_cancelled_requests() {
        let mut scheduler = DeterministicScheduler::new(|item: &i32| Ok(*item));

        scheduler.request(1, Priority::Active, 1);
        scheduler.request(2, Priority::Background, 2);

        assert!(scheduler.cancel_id(1));
        assert_eq!(scheduler.run_all(), vec![2]);
        assert_eq!(scheduler.drain()[0].id, 2);
    }
}
