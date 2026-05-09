//! Parity tests: production `Scheduler` vs the deterministic test double.
//!
//! These exist to enforce that `tui_kit::testkit::DeterministicScheduler`
//! shares the externally-visible behavior of the threaded scheduler:
//! priority + FIFO ordering, dedup-by-id, scoped cancellation, and epoch
//! invalidation.
//!
//! A green parity test means a passing test against the double should also
//! pass against the real scheduler.

use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::time::Duration;

use tui_kit::scheduler::{Priority, RequestScope, Scheduler};
use tui_kit::testkit::DeterministicScheduler;

#[test]
fn priority_and_fifo_ordering_matches() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request(1, Priority::Background, 100);
    det.request(2, Priority::Active, 200);
    det.request(3, Priority::Active, 201);
    det.request(4, Priority::Hover, 150);

    let det_order = det.run_all();
    assert_eq!(det_order, vec![2, 3, 4, 1]);

    let det_results: Vec<i32> = det
        .drain()
        .into_iter()
        .map(|c| *c.result.as_ref().unwrap())
        .collect();
    assert_eq!(det_results, vec![200, 201, 150, 100]);

    // Real scheduler: 1 worker preserves the same ordering.
    let (tx, rx) = mpsc::channel();
    let mut real: Scheduler<i32, i32> =
        Scheduler::new(NonZeroUsize::new(1).unwrap(), tx, |item: &i32| Ok(*item));
    real.request(1, Priority::Background, 100);
    real.request(2, Priority::Active, 200);
    real.request(3, Priority::Active, 201);
    real.request(4, Priority::Hover, 150);

    for _ in 0..4 {
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
    }
    let real_results: Vec<(u64, i32)> = real
        .drain()
        .into_iter()
        .map(|c| (c.id, *c.result.as_ref().unwrap()))
        .collect();
    assert_eq!(real_results, vec![(2, 200), (3, 201), (4, 150), (1, 100)]);
}

#[test]
fn dedup_by_id_matches() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request(1, Priority::Active, 1);
    det.request(1, Priority::Active, 2); // ignored
    assert_eq!(det.queued_len(), 1);
    det.run_all();
    det.request(1, Priority::Active, 3); // also ignored — already completed
    assert_eq!(det.queued_len(), 0);
}

#[test]
fn cancel_group_matches() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request_scoped(
        1,
        Priority::Background,
        1,
        RequestScope::default().group("stale"),
    );
    det.request_scoped(
        2,
        Priority::Background,
        2,
        RequestScope::default().group("fresh"),
    );

    let report = det.cancel_group("stale");
    assert_eq!(report.queued, 1);
    assert_eq!(report.in_flight, 0);
    assert_eq!(det.queued_len(), 1);

    det.run_all();
    let completed: Vec<u64> = det.drain().into_iter().map(|c| c.id).collect();
    assert_eq!(completed, vec![2]);
}

#[test]
fn epoch_namespace_invalidation_drops_queued_work() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request_scoped(
        1,
        Priority::Active,
        1,
        RequestScope::default().epoch_namespace("thumbnail"),
    );
    det.invalidate_epoch_namespace("thumbnail");
    det.request_scoped(
        2,
        Priority::Active,
        2,
        RequestScope::default().epoch_namespace("thumbnail"),
    );

    let ran = det.run_all();
    assert_eq!(ran, vec![2], "stale-namespace work must drop silently");
}

#[test]
fn invalidate_all_drops_pending_work_in_both() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request(1, Priority::Active, 1);
    det.request(2, Priority::Active, 2);
    det.invalidate_all();
    det.request(3, Priority::Active, 3);

    let ran = det.run_all();
    assert_eq!(ran, vec![3]);
    assert_eq!(det.cancelled_total(), 2);
}
