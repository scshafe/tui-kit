//! Priority-queue work scheduler with epoch-based cancellation.
//!
//! [`Scheduler<Item, Out>`] runs an `Fn(&Item) -> Result<Out>` executor on
//! a pool of worker threads. Submissions are de-duplicated by `id`,
//! prioritised by [`Priority`], and cancellable in bulk via
//! [`Scheduler::invalidate_all`] (in-flight items finish but their results
//! are silently dropped).
//!
//! Completions are buffered internally; the application drains them with
//! [`Scheduler::drain`] after receiving an [`crate::events::AppEvent::SchedulerComplete`]
//! wake-up.

use crate::events::{AppEvent, AppEventSender};
use anyhow::Result;
use std::collections::{BinaryHeap, HashSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub completed: usize,
    pub failed: usize,
    pub pending: usize,
    pub total: usize,
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
    in_flight: HashSet<u64>,
    completed: HashSet<u64>,
    failed: HashSet<u64>,
    completions: VecDeque<Completion<Out>>,
    epoch: u64,
    seq: u64,
    shutdown: bool,
}

struct PrioritizedRequest<Item> {
    priority: Priority,
    seq: u64,
    id: u64,
    epoch: u64,
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
                in_flight: HashSet::new(),
                completed: HashSet::new(),
                failed: HashSet::new(),
                completions: VecDeque::new(),
                epoch: 0,
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
        let mut state = self.inner.state.lock().unwrap();
        if state.completed.contains(&id)
            || state.in_flight.contains(&id)
            || state.queued.contains(&id)
        {
            return;
        }
        let epoch = state.epoch;
        let seq = state.seq;
        state.seq += 1;
        state.queued.insert(id);
        state.queue.push(PrioritizedRequest {
            priority,
            seq,
            id,
            epoch,
            item,
        });
        self.last_known_total += 1;
        self.inner.cv.notify_one();
    }

    pub fn invalidate_all(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        state.epoch += 1;
        state.queue.clear();
        state.queued.clear();
        state.completed.clear();
        state.failed.clear();
        state.completions.clear();
        self.last_known_total = 0;
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

    pub fn is_completed(&self, id: u64) -> bool {
        self.inner.state.lock().unwrap().completed.contains(&id)
    }
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
                    state.in_flight.insert(req.id);
                    break req;
                }
                state = inner.cv.wait(state).unwrap();
            }
        };
        let result = (inner.executor)(&request.item);
        {
            let mut state = inner.state.lock().unwrap();
            state.in_flight.remove(&request.id);
            if request.epoch == state.epoch {
                match &result {
                    Ok(_) => {
                        state.completed.insert(request.id);
                    }
                    Err(_) => {
                        state.failed.insert(request.id);
                    }
                }
                state.completions.push_back(Completion {
                    id: request.id,
                    result,
                });
            }
        }
        let _ = inner.sink.send(AppEvent::SchedulerComplete);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn higher_priority_pops_first() {
        let mut heap: BinaryHeap<PrioritizedRequest<()>> = BinaryHeap::new();
        let req = |id: u64, priority: Priority, seq: u64| PrioritizedRequest {
            priority,
            seq,
            id,
            epoch: 0,
            item: (),
        };
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
            heap.push(PrioritizedRequest {
                priority: Priority::Hover,
                seq,
                id: seq,
                epoch: 0,
                item: (),
            });
        }
        let popped: Vec<_> = std::iter::from_fn(|| heap.pop().map(|r| r.id)).collect();
        assert_eq!(popped, vec![0, 1, 2]);
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
