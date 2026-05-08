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
use crate::scheduler::{CancellationReport, Completion, Priority, RequestScope};
use anyhow::Result;
use anyhow::Result as AnyhowResult;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{StatefulWidget, Widget};
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::convert::Infallible;

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
/// Mirrors the production [`crate::scheduler::Scheduler`]'s externally-visible
/// behavior: priority + FIFO ordering, dedup-by-id, scoped cancellation
/// (`group`/`source`/`epoch_namespace`), epoch invalidation, completion drain
/// semantics, and cancellation reports. The only difference is execution: work
/// runs only when [`DeterministicScheduler::run_one`] or [`run_all`] is called.
pub struct DeterministicScheduler<Item, Out, P: Ord + Clone = Priority> {
    queue: BinaryHeap<TestScheduledRequest<Item, P>>,
    queued: HashSet<u64>,
    completed: HashSet<u64>,
    cancelled_ids: HashSet<u64>,
    cancelled_total: usize,
    completions: VecDeque<Completion<Out>>,
    epoch: u64,
    epoch_namespaces: HashMap<String, u64>,
    executor: TestExecutor<Item, Out>,
    seq: u64,
}

type TestExecutor<Item, Out> = Box<dyn FnMut(&Item) -> AnyhowResult<Out>>;

impl<Item, Out, P: Ord + Clone> std::fmt::Debug for DeterministicScheduler<Item, Out, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeterministicScheduler")
            .field("queued", &self.queue.len())
            .field("completed", &self.completed.len())
            .field("completions", &self.completions.len())
            .field("cancelled_total", &self.cancelled_total)
            .finish_non_exhaustive()
    }
}

impl<Item, Out, P: Ord + Clone> DeterministicScheduler<Item, Out, P> {
    pub fn new<F>(executor: F) -> Self
    where
        F: FnMut(&Item) -> AnyhowResult<Out> + 'static,
    {
        Self {
            queue: BinaryHeap::new(),
            queued: HashSet::new(),
            completed: HashSet::new(),
            cancelled_ids: HashSet::new(),
            cancelled_total: 0,
            completions: VecDeque::new(),
            epoch: 0,
            epoch_namespaces: HashMap::new(),
            executor: Box::new(executor),
            seq: 0,
        }
    }

    pub fn request(&mut self, id: u64, priority: P, item: Item) {
        self.request_scoped(id, priority, item, RequestScope::default());
    }

    pub fn request_scoped(&mut self, id: u64, priority: P, item: Item, scope: RequestScope) {
        if self.completed.contains(&id) || self.queued.contains(&id) {
            return;
        }
        let namespace_epoch = scope
            .epoch_namespace
            .as_ref()
            .and_then(|ns| self.epoch_namespaces.get(ns).copied())
            .unwrap_or(0);
        let seq = self.seq;
        self.seq += 1;
        self.queued.insert(id);
        self.queue.push(TestScheduledRequest {
            priority,
            seq,
            id,
            epoch: self.epoch,
            namespace_epoch,
            scope,
            item,
        });
    }

    pub fn cancel_id(&mut self, id: u64) -> CancellationReport {
        let queued = cancel_queued_where(&mut self.queue, &mut self.queued, |req| req.id == id);
        self.cancelled_total += queued;
        CancellationReport {
            queued,
            in_flight: 0,
        }
    }

    pub fn cancel_group(&mut self, group: &str) -> CancellationReport {
        let queued = cancel_queued_where(&mut self.queue, &mut self.queued, |req| {
            req.scope.group.as_deref() == Some(group)
        });
        self.cancelled_total += queued;
        CancellationReport {
            queued,
            in_flight: 0,
        }
    }

    pub fn cancel_source(&mut self, source: &str) -> CancellationReport {
        let queued = cancel_queued_where(&mut self.queue, &mut self.queued, |req| {
            req.scope.source.as_deref() == Some(source)
        });
        self.cancelled_total += queued;
        CancellationReport {
            queued,
            in_flight: 0,
        }
    }

    pub fn invalidate_epoch_namespace(&mut self, namespace: &str) -> CancellationReport {
        *self
            .epoch_namespaces
            .entry(namespace.to_string())
            .or_insert(0) += 1;
        let queued = cancel_queued_where(&mut self.queue, &mut self.queued, |req| {
            req.scope.epoch_namespace.as_deref() == Some(namespace)
        });
        self.cancelled_total += queued;
        CancellationReport {
            queued,
            in_flight: 0,
        }
    }

    pub fn invalidate_all(&mut self) {
        let queued = self.queue.len();
        self.epoch += 1;
        self.queue.clear();
        self.queued.clear();
        self.completed.clear();
        self.completions.clear();
        self.cancelled_total += queued;
    }

    /// Execute the next queued request whose epoch is still current.
    /// Returns the executed id, or None if the queue is empty (cancelled
    /// requests are silently dropped).
    pub fn run_one(&mut self) -> Option<u64> {
        while let Some(request) = self.queue.pop() {
            self.queued.remove(&request.id);
            if request.epoch != self.epoch {
                continue;
            }
            let current_namespace_epoch = request
                .scope
                .epoch_namespace
                .as_ref()
                .and_then(|ns| self.epoch_namespaces.get(ns).copied())
                .unwrap_or(0);
            if request.namespace_epoch != current_namespace_epoch {
                continue;
            }
            if self.cancelled_ids.remove(&request.id) {
                continue;
            }
            let id = request.id;
            let result = (self.executor)(&request.item);
            self.completed.insert(id);
            self.completions.push_back(Completion { id, result });
            return Some(id);
        }
        None
    }

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

    pub fn cancelled_total(&self) -> usize {
        self.cancelled_total
    }
}

fn cancel_queued_where<Item, P: Ord>(
    queue: &mut BinaryHeap<TestScheduledRequest<Item, P>>,
    queued: &mut HashSet<u64>,
    matches: impl Fn(&TestScheduledRequest<Item, P>) -> bool,
) -> usize {
    let mut kept = BinaryHeap::new();
    let mut cancelled = 0;
    for req in queue.drain() {
        if matches(&req) {
            queued.remove(&req.id);
            cancelled += 1;
        } else {
            kept.push(req);
        }
    }
    *queue = kept;
    cancelled
}

struct TestScheduledRequest<Item, P> {
    priority: P,
    seq: u64,
    id: u64,
    epoch: u64,
    namespace_epoch: u64,
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
}
