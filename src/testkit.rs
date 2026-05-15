//! Test harness helpers for widgets, event streams, and image side effects.
//!
//! These helpers are deliberately small and deterministic. They let apps and
//! crate tests exercise ratatui-native widgets without opening a terminal,
//! route typed input/resize events through the same [`crate::events::AppEvent`]
//! shape used at runtime, and assert image lifecycle calls without emitting
//! terminal escape sequences.
//!
//! **Stability:** consumed by tui-kit's parity tests and available to app
//! tests, but intentionally not re-exported from the production prelude. Test
//! doubles must preserve production semantics before they grow convenience API.

use crate::component::BufferComponent;
use crate::elements::RenderEffect;
use crate::events::AppEvent;
use crate::image::{ImageCapabilities, ImageSurface, PlaceOptions};
use crate::input::KeyEvent;
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

/// Render a [`BufferComponent`] into an owned [`Buffer`].
///
/// Collapses the `Buffer::empty(area) + render_buffer(area, &mut buffer)?`
/// pattern that recurs across element and widget tests.
pub fn render_to_buffer<C: BufferComponent>(component: &mut C, area: Rect) -> Result<Buffer> {
    let mut buffer = Buffer::empty(area);
    component.render_buffer(area, &mut buffer)?;
    Ok(buffer)
}

/// Find the first [`RenderEffect::PlaceImage`] in `effects` whose placement id matches.
///
/// Replaces `effects.iter().any(|e| matches!(e, RenderEffect::PlaceImage { options, .. } if options.placement_id == N))`
/// with a single call that also gives the caller the matching [`PlaceOptions`].
pub fn find_place_with_placement_id(
    effects: &[RenderEffect],
    placement_id: u32,
) -> Option<&PlaceOptions> {
    effects.iter().find_map(|effect| match effect {
        RenderEffect::PlaceImage { options, .. } if options.placement_id == placement_id => {
            Some(options)
        }
        _ => None,
    })
}

/// Assert that every placement introduced by `placed` is covered by a teardown in `teardown`.
///
/// A placement `(image_id, placement_id)` from a [`RenderEffect::PlaceImage`]
/// in `placed` is "covered" if `teardown` contains any of:
///
/// - [`RenderEffect::DeleteImagePlacement`] matching both ids,
/// - [`RenderEffect::DeletePlacement`] matching the placement id,
/// - [`RenderEffect::DeleteAllPlacements`] (blanket),
/// - [`RenderEffect::ForgetAllImages`] (blanket).
///
/// Blanket teardowns satisfy every placement at once; tests wanting stricter
/// per-placement assertions should match on `teardown` directly or use
/// [`find_place_with_placement_id`] alongside their own checks. Panics with a
/// descriptive message naming the first uncovered placement.
pub fn assert_teardown_covers(placed: &[RenderEffect], teardown: &[RenderEffect]) {
    let blanket = teardown.iter().any(|effect| {
        matches!(
            effect,
            RenderEffect::DeleteAllPlacements | RenderEffect::ForgetAllImages
        )
    });
    if blanket {
        return;
    }
    for effect in placed {
        if let RenderEffect::PlaceImage { options, .. } = effect {
            let covered = teardown.iter().any(|t| match t {
                RenderEffect::DeleteImagePlacement {
                    image_id,
                    placement_id,
                } => *image_id == options.image_id && *placement_id == options.placement_id,
                RenderEffect::DeletePlacement { placement_id } => {
                    *placement_id == options.placement_id
                }
                _ => false,
            });
            assert!(
                covered,
                "teardown does not cover PlaceImage with image_id={} placement_id={}: teardown={:?}",
                options.image_id, options.placement_id, teardown,
            );
        }
    }
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
    pub fn keys(keys: impl IntoIterator<Item = KeyEvent>) -> Self {
        Self::new(keys.into_iter().map(AppEvent::input_key))
    }

    /// Build a one-event resize script using the runtime event category.
    pub fn resize(cols: u16, rows: u16) -> Self {
        Self::new([AppEvent::terminal_resize(cols, rows)])
    }
}

/// A single image lifecycle call captured by [`MockImageSurface`].
///
/// `ImageSurface::flush` is intentionally not represented here: the trait
/// method takes `&self`, which rules out pushing to the mock's recording
/// vector without interior mutability that would break the derived
/// `Clone + PartialEq + Eq`. Flush is output-buffer flushing, not a
/// lifecycle state change worth asserting on a mock that already records
/// every lifecycle-affecting call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MockImageCall {
    EnsureLoaded { image_id: u32, bytes: usize },
    Place(PlaceOptions),
    DeleteImagePlacement { image_id: u32, placement_id: u32 },
    DeletePlacement { placement_id: u32 },
    DeleteAllPlacements,
    ForgetAll,
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

    fn delete_image_placement(&mut self, image_id: u32, placement_id: u32) -> Result<()> {
        self.calls.push(MockImageCall::DeleteImagePlacement {
            image_id,
            placement_id,
        });
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
/// runs only when [`DeterministicScheduler::run_one`] or [`DeterministicScheduler::run_all`] is called.
pub struct DeterministicScheduler<Item, Out, P: Ord + Clone = Priority> {
    queue: BinaryHeap<TestScheduledRequest<Item, P>>,
    queued: HashSet<u64>,
    in_flight: Option<TestScheduledRequest<Item, P>>,
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
            .field("in_flight", &self.in_flight.is_some())
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
            in_flight: None,
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
        if self.completed.contains(&id)
            || self.queued.contains(&id)
            || self.in_flight.as_ref().is_some_and(|req| req.id == id)
        {
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
        let in_flight = self.cancel_in_flight(|req| req.id == id);
        self.cancelled_total += queued + in_flight;
        CancellationReport { queued, in_flight }
    }

    pub fn cancel_group(&mut self, group: &str) -> CancellationReport {
        let queued = cancel_queued_where(&mut self.queue, &mut self.queued, |req| {
            req.scope.group.as_deref() == Some(group)
        });
        let in_flight = self.cancel_in_flight(|req| req.scope.group.as_deref() == Some(group));
        self.cancelled_total += queued + in_flight;
        CancellationReport { queued, in_flight }
    }

    pub fn cancel_source(&mut self, source: &str) -> CancellationReport {
        let queued = cancel_queued_where(&mut self.queue, &mut self.queued, |req| {
            req.scope.source.as_deref() == Some(source)
        });
        let in_flight = self.cancel_in_flight(|req| req.scope.source.as_deref() == Some(source));
        self.cancelled_total += queued + in_flight;
        CancellationReport { queued, in_flight }
    }

    pub fn invalidate_epoch_namespace(&mut self, namespace: &str) -> CancellationReport {
        *self
            .epoch_namespaces
            .entry(namespace.to_string())
            .or_insert(0) += 1;
        let queued = cancel_queued_where(&mut self.queue, &mut self.queued, |req| {
            req.scope.epoch_namespace.as_deref() == Some(namespace)
        });
        let in_flight =
            self.cancel_in_flight(|req| req.scope.epoch_namespace.as_deref() == Some(namespace));
        self.cancelled_total += queued + in_flight;
        CancellationReport { queued, in_flight }
    }

    pub fn invalidate_all(&mut self) {
        let queued = self.queue.len();
        let in_flight = self
            .in_flight
            .as_ref()
            .is_some_and(|req| self.cancelled_ids.insert(req.id));
        self.epoch += 1;
        self.queue.clear();
        self.queued.clear();
        self.completed.clear();
        self.completions.clear();
        self.cancelled_total += queued + usize::from(in_flight);
    }

    /// Start the next queued request whose epoch is still current, leaving it
    /// in flight until [`DeterministicScheduler::finish_in_flight`] is called.
    ///
    /// This exposes the same cancellation window production workers have after
    /// dequeuing work and before publishing a completion.
    pub fn begin_one(&mut self) -> Option<u64> {
        if self.in_flight.is_some() {
            return None;
        }
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
            let id = request.id;
            self.in_flight = Some(request);
            return Some(id);
        }
        None
    }

    /// Finish the currently in-flight request.
    ///
    /// The executor still runs for cancelled work, matching the production
    /// scheduler's "finish then drop cancelled completion" behavior. Returns
    /// the completed id only when a completion was recorded.
    pub fn finish_in_flight(&mut self) -> Option<u64> {
        let request = self.in_flight.take()?;
        let id = request.id;
        let result = (self.executor)(&request.item);
        let current_namespace_epoch = request
            .scope
            .epoch_namespace
            .as_ref()
            .and_then(|ns| self.epoch_namespaces.get(ns).copied())
            .unwrap_or(0);
        if request.epoch == self.epoch
            && request.namespace_epoch == current_namespace_epoch
            && !self.cancelled_ids.remove(&id)
        {
            self.completed.insert(id);
            self.completions.push_back(Completion { id, result });
            Some(id)
        } else {
            None
        }
    }

    /// Execute the next queued request whose epoch is still current.
    /// Returns the completed id, or None if the queue is empty or the started
    /// request was cancelled/stale before completion.
    pub fn run_one(&mut self) -> Option<u64> {
        self.begin_one()?;
        self.finish_in_flight()
    }

    fn cancel_in_flight(
        &mut self,
        matches: impl Fn(&TestScheduledRequest<Item, P>) -> bool,
    ) -> usize {
        if let Some(request) = &self.in_flight {
            if matches(request) && self.cancelled_ids.insert(request.id) {
                return 1;
            }
        }
        0
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

    pub fn in_flight_len(&self) -> usize {
        usize::from(self.in_flight.is_some())
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
        let script = EventScript::keys([KeyEvent::Down, KeyEvent::Enter]);

        assert_eq!(script.events()[0], AppEvent::input_key(KeyEvent::Down));
        assert_eq!(script.events()[1], AppEvent::input_key(KeyEvent::Enter));
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
        surface.delete_image_placement(7, 9).unwrap();
        surface.delete_placement(9).unwrap();

        assert_eq!(
            surface.calls(),
            &[
                MockImageCall::EnsureLoaded {
                    image_id: 7,
                    bytes: 3
                },
                MockImageCall::Place(opts),
                MockImageCall::DeleteImagePlacement {
                    image_id: 7,
                    placement_id: 9
                },
                MockImageCall::DeletePlacement { placement_id: 9 }
            ]
        );
    }

    #[test]
    fn mock_image_surface_records_main_grid_main_lifecycle() {
        fn opts(image_id: u32, placement_id: u32, cell_cols: u16, cell_rows: u16) -> PlaceOptions {
            PlaceOptions {
                image_id,
                placement_id,
                source: crate::layout::PixelRect {
                    x: 0,
                    y: 0,
                    width: 16,
                    height: 9,
                },
                cell_cols,
                cell_rows,
            }
        }

        let mut surface = MockImageSurface::default();
        let main_first = opts(1, crate::image::MAIN_PLACEMENT_ID, 80, 22);
        let thumb_first = opts(1, crate::image::picker_placement_id(0), 18, 5);
        let thumb_second = opts(2, crate::image::picker_placement_id(1), 18, 5);
        let main_second = opts(2, crate::image::MAIN_PLACEMENT_ID, 80, 22);

        surface.ensure_loaded(1, b"main-1").unwrap();
        surface.place(main_first).unwrap();
        surface
            .delete_image_placement(1, crate::image::MAIN_PLACEMENT_ID)
            .unwrap();
        surface.ensure_loaded(1, b"main-1").unwrap();
        surface.place(thumb_first).unwrap();
        surface.ensure_loaded(2, b"main-2").unwrap();
        surface.place(thumb_second).unwrap();
        surface
            .delete_image_placement(1, crate::image::picker_placement_id(0))
            .unwrap();
        surface
            .delete_image_placement(2, crate::image::picker_placement_id(1))
            .unwrap();
        surface.ensure_loaded(2, b"main-2").unwrap();
        surface.place(main_second).unwrap();
        surface.forget_all().unwrap();

        assert_eq!(
            surface.calls(),
            &[
                MockImageCall::EnsureLoaded {
                    image_id: 1,
                    bytes: 6,
                },
                MockImageCall::Place(main_first),
                MockImageCall::DeleteImagePlacement {
                    image_id: 1,
                    placement_id: crate::image::MAIN_PLACEMENT_ID,
                },
                MockImageCall::EnsureLoaded {
                    image_id: 1,
                    bytes: 6,
                },
                MockImageCall::Place(thumb_first),
                MockImageCall::EnsureLoaded {
                    image_id: 2,
                    bytes: 6,
                },
                MockImageCall::Place(thumb_second),
                MockImageCall::DeleteImagePlacement {
                    image_id: 1,
                    placement_id: crate::image::picker_placement_id(0),
                },
                MockImageCall::DeleteImagePlacement {
                    image_id: 2,
                    placement_id: crate::image::picker_placement_id(1),
                },
                MockImageCall::EnsureLoaded {
                    image_id: 2,
                    bytes: 6,
                },
                MockImageCall::Place(main_second),
                MockImageCall::ForgetAll,
            ]
        );
    }

    fn place_effect(image_id: u32, placement_id: u32) -> RenderEffect {
        RenderEffect::PlaceImage {
            origin: crate::layout::CellOffset { col: 0, row: 0 },
            options: PlaceOptions {
                image_id,
                placement_id,
                source: crate::layout::PixelRect {
                    x: 0,
                    y: 0,
                    width: 4,
                    height: 4,
                },
                cell_cols: 1,
                cell_rows: 1,
            },
        }
    }

    #[test]
    fn find_place_returns_first_match_by_placement_id() {
        let effects = vec![
            place_effect(1, 10),
            place_effect(2, 20),
            place_effect(3, 20),
        ];

        let found = find_place_with_placement_id(&effects, 20).expect("place not found");
        assert_eq!(found.image_id, 2);
        assert_eq!(found.placement_id, 20);

        assert!(find_place_with_placement_id(&effects, 99).is_none());
    }

    #[test]
    fn assert_teardown_covers_accepts_matched_delete_image_placement() {
        let placed = vec![place_effect(7, 9)];
        let teardown = vec![RenderEffect::DeleteImagePlacement {
            image_id: 7,
            placement_id: 9,
        }];

        assert_teardown_covers(&placed, &teardown);
    }

    #[test]
    fn assert_teardown_covers_accepts_delete_placement_by_id() {
        let placed = vec![place_effect(7, 9)];
        let teardown = vec![RenderEffect::DeletePlacement { placement_id: 9 }];

        assert_teardown_covers(&placed, &teardown);
    }

    #[test]
    fn assert_teardown_covers_accepts_blanket_delete_all_placements() {
        let placed = vec![place_effect(1, 10), place_effect(2, 20)];
        let teardown = vec![RenderEffect::DeleteAllPlacements];

        assert_teardown_covers(&placed, &teardown);
    }

    #[test]
    fn assert_teardown_covers_accepts_blanket_forget_all_images() {
        let placed = vec![place_effect(1, 10)];
        let teardown = vec![RenderEffect::ForgetAllImages];

        assert_teardown_covers(&placed, &teardown);
    }

    #[test]
    #[should_panic(expected = "teardown does not cover PlaceImage")]
    fn assert_teardown_covers_panics_when_placement_missing() {
        let placed = vec![place_effect(7, 9)];
        let teardown = vec![RenderEffect::DeletePlacement { placement_id: 8 }];

        assert_teardown_covers(&placed, &teardown);
    }
}
