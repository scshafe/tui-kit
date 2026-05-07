//! Priority-queue work scheduler with scoped cancellation and instrumentation.
//!
//! [`Scheduler<Item, Out>`] runs an `Fn(&Item) -> Result<Out>` executor on
//! a pool of worker threads. Submissions are de-duplicated by `id`,
//! prioritised by [`Priority`], and cancellable by id, group, source, epoch
//! namespace, or all work. In-flight items finish but cancelled results are
//! silently dropped.
//!
//! Completions are buffered internally; the application drains them with
//! [`Scheduler::drain`] after receiving an [`crate::events::AppEvent::Scheduler`]
//! event carrying [`crate::events::SchedulerEvent::Complete`].

use crate::events::{AppEvent, AppEventSender};
use anyhow::Result;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Background,
    Hover,
    Active,
}

#[derive(Debug)]
pub struct Completion<Out> {
    pub id: u64,
    pub result: Result<Out>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RequestScope {
    pub group: Option<String>,
    pub source: Option<String>,
    pub epoch_namespace: Option<String>,
}

impl RequestScope {
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn epoch_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.epoch_namespace = Some(namespace.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub completed: usize,
    pub failed: usize,
    pub pending: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerStats {
    pub pending: usize,
    pub active_workers: usize,
    pub queued: usize,
    pub completed_total: usize,
    pub failed_total: usize,
    pub cancelled_total: usize,
    pub oldest_queued_age: Option<Duration>,
    pub newest_queued_age: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancellationReport {
    pub queued: usize,
    pub in_flight: usize,
}

impl CancellationReport {
    pub fn total(self) -> usize {
        self.queued + self.in_flight
    }
}

pub struct Scheduler<Item: Send + 'static, Out: Send + 'static> {
    inner: Arc<SchedulerInner<Item, Out>>,
    handles: Vec<JoinHandle<()>>,
    last_known_total: usize,
}

impl<Item: Send + 'static, Out: Send + 'static> std::fmt::Debug for Scheduler<Item, Out> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler")
            .field("workers", &self.handles.len())
            .field("last_known_total", &self.last_known_total)
            .finish()
    }
}

struct SchedulerInner<Item, Out> {
    state: Mutex<SchedulerState<Item, Out>>,
    cv: Condvar,
    executor: WorkExecutor<Item, Out>,
    sink: AppEventSender,
}

type WorkExecutor<Item, Out> = Box<dyn Fn(&Item) -> Result<Out> + Send + Sync>;

struct SchedulerState<Item, Out> {
    queue: BinaryHeap<PrioritizedRequest<Item>>,
    queued: HashSet<u64>,
    in_flight: HashMap<u64, RequestScope>,
    completed: HashSet<u64>,
    failed: HashSet<u64>,
    completed_total: usize,
    failed_total: usize,
    cancelled_ids: HashSet<u64>,
    cancelled_total: usize,
    completions: VecDeque<Completion<Out>>,
    epoch: u64,
    epoch_namespaces: HashMap<String, u64>,
    seq: u64,
    shutdown: bool,
}

struct PrioritizedRequest<Item> {
    priority: Priority,
    seq: u64,
    id: u64,
    epoch: u64,
    namespace_epoch: u64,
    scope: RequestScope,
    queued_at: Instant,
    item: Item,
}

impl<Item> PartialEq for PrioritizedRequest<Item> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}
impl<Item> Eq for PrioritizedRequest<Item> {}
impl<Item> PartialOrd for PrioritizedRequest<Item> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<Item> Ord for PrioritizedRequest<Item> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl<Item: Send + 'static, Out: Send + 'static> Scheduler<Item, Out> {
    pub fn new<F>(workers: usize, sink: AppEventSender, executor: F) -> Self
    where
        F: Fn(&Item) -> Result<Out> + Send + Sync + 'static,
    {
        let workers = workers.max(1);
        let inner = Arc::new(SchedulerInner {
            state: Mutex::new(SchedulerState {
                queue: BinaryHeap::new(),
                queued: HashSet::new(),
                in_flight: HashMap::new(),
                completed: HashSet::new(),
                failed: HashSet::new(),
                completed_total: 0,
                failed_total: 0,
                cancelled_ids: HashSet::new(),
                cancelled_total: 0,
                completions: VecDeque::new(),
                epoch: 0,
                epoch_namespaces: HashMap::new(),
                seq: 0,
                shutdown: false,
            }),
            cv: Condvar::new(),
            executor: Box::new(executor),
            sink,
        });
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let inner_t = inner.clone();
            handles.push(thread::spawn(move || worker_loop(inner_t)));
        }
        Self {
            inner,
            handles,
            last_known_total: 0,
        }
    }

    pub fn request(&mut self, id: u64, priority: Priority, item: Item) {
        self.request_scoped(id, priority, item, RequestScope::default());
    }

    pub fn request_scoped(&mut self, id: u64, priority: Priority, item: Item, scope: RequestScope) {
        let mut state = self.inner.state.lock().unwrap();
        if state.completed.contains(&id)
            || state.in_flight.contains_key(&id)
            || state.queued.contains(&id)
        {
            return;
        }
        let epoch = state.epoch;
        let namespace_epoch = scope
            .epoch_namespace
            .as_ref()
            .and_then(|namespace| state.epoch_namespaces.get(namespace).copied())
            .unwrap_or(0);
        let seq = state.seq;
        state.seq += 1;
        state.queued.insert(id);
        state.queue.push(PrioritizedRequest {
            priority,
            seq,
            id,
            epoch,
            namespace_epoch,
            scope,
            queued_at: Instant::now(),
            item,
        });
        self.last_known_total += 1;
        self.inner.cv.notify_one();
    }

    pub fn invalidate_all(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        let queued = state.queue.len();
        let in_flight = state.in_flight.len();
        state.epoch += 1;
        state.queue.clear();
        state.queued.clear();
        state.completed.clear();
        state.failed.clear();
        let cancelled_ids: Vec<_> = state.in_flight.keys().copied().collect();
        state.cancelled_ids.extend(cancelled_ids);
        state.cancelled_total += queued + in_flight;
        state.completions.clear();
        self.last_known_total = 0;
    }

    pub fn cancel_id(&mut self, id: u64) -> CancellationReport {
        let mut state = self.inner.state.lock().unwrap();
        let queued = cancel_queued_where(&mut state, |req| req.id == id);
        let in_flight =
            usize::from(state.in_flight.contains_key(&id) && state.cancelled_ids.insert(id));
        state.cancelled_total += queued + in_flight;
        CancellationReport { queued, in_flight }
    }

    pub fn cancel_group(&mut self, group: &str) -> CancellationReport {
        self.cancel_scope(|scope| scope.group.as_deref() == Some(group))
    }

    pub fn cancel_source(&mut self, source: &str) -> CancellationReport {
        self.cancel_scope(|scope| scope.source.as_deref() == Some(source))
    }

    pub fn invalidate_epoch_namespace(&mut self, namespace: &str) -> CancellationReport {
        let mut state = self.inner.state.lock().unwrap();
        *state
            .epoch_namespaces
            .entry(namespace.to_string())
            .or_insert(0) += 1;
        let queued = cancel_queued_where(&mut state, |req| {
            req.scope.epoch_namespace.as_deref() == Some(namespace)
        });
        let ids: Vec<_> = state
            .in_flight
            .iter()
            .filter_map(|(id, scope)| {
                (scope.epoch_namespace.as_deref() == Some(namespace)).then_some(*id)
            })
            .collect();
        let newly_cancelled = ids
            .into_iter()
            .filter(|id| state.cancelled_ids.insert(*id))
            .count();
        state.cancelled_total += queued + newly_cancelled;
        CancellationReport {
            queued,
            in_flight: newly_cancelled,
        }
    }

    fn cancel_scope(&mut self, matches: impl Fn(&RequestScope) -> bool) -> CancellationReport {
        let mut state = self.inner.state.lock().unwrap();
        let queued = cancel_queued_where(&mut state, |req| matches(&req.scope));
        let ids: Vec<_> = state
            .in_flight
            .iter()
            .filter_map(|(id, scope)| matches(scope).then_some(*id))
            .collect();
        let newly_cancelled = ids
            .into_iter()
            .filter(|id| state.cancelled_ids.insert(*id))
            .count();
        state.cancelled_total += queued + newly_cancelled;
        CancellationReport {
            queued,
            in_flight: newly_cancelled,
        }
    }

    pub fn drain(&self) -> Vec<Completion<Out>> {
        let mut state = self.inner.state.lock().unwrap();
        std::mem::take(&mut state.completions).into_iter().collect()
    }

    pub fn progress(&self) -> Progress {
        let state = self.inner.state.lock().unwrap();
        let pending = state.queue.len() + state.in_flight.len();
        Progress {
            completed: state.completed.len(),
            failed: state.failed.len(),
            pending,
            total: self.last_known_total,
        }
    }

    pub fn stats(&self) -> SchedulerStats {
        let state = self.inner.state.lock().unwrap();
        let now = Instant::now();
        let mut ages = state
            .queue
            .iter()
            .map(|req| now.saturating_duration_since(req.queued_at));
        let first = ages.next();
        let (oldest_queued_age, newest_queued_age) = first.map_or((None, None), |first| {
            let mut oldest = first;
            let mut newest = first;
            for age in ages {
                oldest = oldest.max(age);
                newest = newest.min(age);
            }
            (Some(oldest), Some(newest))
        });
        SchedulerStats {
            pending: state.queue.len() + state.in_flight.len(),
            active_workers: state.in_flight.len(),
            queued: state.queue.len(),
            completed_total: state.completed_total,
            failed_total: state.failed_total,
            cancelled_total: state.cancelled_total,
            oldest_queued_age,
            newest_queued_age,
        }
    }

    pub fn is_completed(&self, id: u64) -> bool {
        self.inner.state.lock().unwrap().completed.contains(&id)
    }
}

fn cancel_queued_where<Item, Out>(
    state: &mut SchedulerState<Item, Out>,
    matches: impl Fn(&PrioritizedRequest<Item>) -> bool,
) -> usize {
    let mut kept = BinaryHeap::new();
    let mut cancelled = 0;
    for req in state.queue.drain() {
        if matches(&req) {
            state.queued.remove(&req.id);
            cancelled += 1;
        } else {
            kept.push(req);
        }
    }
    state.queue = kept;
    cancelled
}

impl<Item: Send + 'static, Out: Send + 'static> Drop for Scheduler<Item, Out> {
    fn drop(&mut self) {
        {
            let mut state = self.inner.state.lock().unwrap();
            state.shutdown = true;
        }
        self.inner.cv.notify_all();
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}

fn worker_loop<Item: Send + 'static, Out: Send + 'static>(inner: Arc<SchedulerInner<Item, Out>>) {
    loop {
        let request = {
            let mut state = inner.state.lock().unwrap();
            loop {
                if state.shutdown {
                    return;
                }
                if let Some(req) = state.queue.pop() {
                    state.queued.remove(&req.id);
                    state.in_flight.insert(req.id, req.scope.clone());
                    break req;
                }
                state = inner.cv.wait(state).unwrap();
            }
        };
        let result = (inner.executor)(&request.item);
        {
            let mut state = inner.state.lock().unwrap();
            state.in_flight.remove(&request.id);
            let current_namespace_epoch = request
                .scope
                .epoch_namespace
                .as_ref()
                .and_then(|namespace| state.epoch_namespaces.get(namespace).copied())
                .unwrap_or(0);
            if request.epoch == state.epoch
                && request.namespace_epoch == current_namespace_epoch
                && !state.cancelled_ids.remove(&request.id)
            {
                match &result {
                    Ok(_) => {
                        state.completed.insert(request.id);
                        state.completed_total += 1;
                    }
                    Err(_) => {
                        state.failed.insert(request.id);
                        state.failed_total += 1;
                    }
                }
                state.completions.push_back(Completion {
                    id: request.id,
                    result,
                });
            }
        }
        let _ = inner.sink.send(AppEvent::scheduler_complete());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    fn req(id: u64, priority: Priority, seq: u64) -> PrioritizedRequest<()> {
        PrioritizedRequest {
            priority,
            seq,
            id,
            epoch: 0,
            namespace_epoch: 0,
            scope: RequestScope::default(),
            queued_at: Instant::now(),
            item: (),
        }
    }

    #[test]
    fn higher_priority_pops_first() {
        let mut heap: BinaryHeap<PrioritizedRequest<()>> = BinaryHeap::new();
        heap.push(req(0, Priority::Background, 0));
        heap.push(req(1, Priority::Hover, 1));
        heap.push(req(2, Priority::Active, 2));
        let popped: Vec<_> = std::iter::from_fn(|| heap.pop().map(|r| r.priority)).collect();
        assert_eq!(
            popped,
            vec![Priority::Active, Priority::Hover, Priority::Background]
        );
    }

    #[test]
    fn fifo_within_priority() {
        let mut heap: BinaryHeap<PrioritizedRequest<()>> = BinaryHeap::new();
        for seq in 0..3u64 {
            heap.push(req(seq, Priority::Hover, seq));
        }
        let popped: Vec<_> = std::iter::from_fn(|| heap.pop().map(|r| r.id)).collect();
        assert_eq!(popped, vec![0, 1, 2]);
    }

    #[test]
    fn cancel_group_removes_queued_work_and_updates_stats() {
        let (tx, _rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let mut sched: Scheduler<i32, i32> = Scheduler::new(1, tx, move |item: &i32| {
            if *item == 0 {
                started_tx.send(()).unwrap();
                release_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(1))
                    .unwrap();
            }
            Ok(item * 2)
        });
        sched.request(0, Priority::Active, 0);
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        sched.request_scoped(
            1,
            Priority::Background,
            1,
            RequestScope::default().group("stale"),
        );
        sched.request_scoped(
            2,
            Priority::Background,
            2,
            RequestScope::default().group("fresh"),
        );

        let report = sched.cancel_group("stale");

        assert_eq!(report.total(), 1);
        let stats = sched.stats();
        assert_eq!(stats.cancelled_total, 1);
        assert_eq!(stats.queued, 1);
        release_tx.send(()).unwrap();
    }

    #[test]
    fn invalidating_epoch_namespace_drops_late_completion() {
        let (tx, rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let mut sched: Scheduler<i32, i32> = Scheduler::new(1, tx, move |item: &i32| {
            started_tx.send(()).unwrap();
            release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(1))
                .unwrap();
            Ok(*item)
        });
        sched.request_scoped(
            1,
            Priority::Active,
            7,
            RequestScope::default().epoch_namespace("thumbnail"),
        );
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        let report = sched.invalidate_epoch_namespace("thumbnail");
        release_tx.send(()).unwrap();
        let _ = rx.recv_timeout(Duration::from_secs(1)).unwrap();

        assert_eq!(report.in_flight, 1);
        assert!(sched.drain().is_empty());
        assert_eq!(sched.stats().cancelled_total, 1);
    }

    #[test]
    fn scheduler_executes_and_returns_completions() {
        let (tx, rx) = mpsc::channel();
        let mut sched: Scheduler<i32, i32> = Scheduler::new(1, tx, |item: &i32| Ok(item * 2));
        sched.request(1, Priority::Active, 21);
        // wait for completion event
        let _ = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let completions = sched.drain();
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].id, 1);
        assert_eq!(completions[0].result.as_ref().unwrap(), &42);
    }
}
