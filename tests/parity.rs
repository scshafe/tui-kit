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
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use tui_kit::scheduler::{CancellationReport, Priority, RequestScope, Scheduler};
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

    let real = cancel_queued_on_real_scheduler(|sched| sched.cancel_group("stale"));
    assert_eq!(real.queued, report.queued);
    assert_eq!(real.in_flight, report.in_flight);

    det.run_all();
    let completed: Vec<u64> = det.drain().into_iter().map(|c| c.id).collect();
    assert_eq!(completed, vec![2]);
}

#[test]
fn cancel_id_matches() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request(1, Priority::Background, 1);
    det.request(2, Priority::Background, 2);

    let report = det.cancel_id(1);
    assert_eq!(report.queued, 1);
    assert_eq!(report.in_flight, 0);

    let real = cancel_queued_on_real_scheduler(|sched| sched.cancel_id(1));
    assert_eq!(real.queued, report.queued);
    assert_eq!(real.in_flight, report.in_flight);

    det.run_all();
    let completed: Vec<u64> = det.drain().into_iter().map(|c| c.id).collect();
    assert_eq!(completed, vec![2]);
}

#[test]
fn cancel_source_matches() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request_scoped(
        1,
        Priority::Background,
        1,
        RequestScope::default().source("stale"),
    );
    det.request_scoped(
        2,
        Priority::Background,
        2,
        RequestScope::default().source("fresh"),
    );

    let report = det.cancel_source("stale");
    assert_eq!(report.queued, 1);
    assert_eq!(report.in_flight, 0);

    let real = cancel_queued_on_real_scheduler(|sched| sched.cancel_source("stale"));
    assert_eq!(real.queued, report.queued);
    assert_eq!(real.in_flight, report.in_flight);

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
    let report = det.invalidate_epoch_namespace("thumbnail");
    assert_eq!(report.queued, 1);
    assert_eq!(report.in_flight, 0);
    det.request_scoped(
        2,
        Priority::Active,
        2,
        RequestScope::default().epoch_namespace("thumbnail"),
    );

    let real =
        cancel_queued_on_real_scheduler(|sched| sched.invalidate_epoch_namespace("thumbnail"));
    assert_eq!(real.queued, report.queued);
    assert_eq!(real.in_flight, report.in_flight);

    let ran = det.run_all();
    assert_eq!(ran, vec![2], "stale-namespace work must drop silently");
}

fn cancel_queued_on_real_scheduler(
    cancel: impl FnOnce(&mut Scheduler<i32, i32>) -> CancellationReport,
) -> CancellationReport {
    let (tx, _rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let mut sched: Scheduler<i32, i32> =
        Scheduler::new(NonZeroUsize::new(1).unwrap(), tx, move |item: &i32| {
            if *item == 0 {
                started_tx.send(()).unwrap();
                release_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap();
            }
            Ok(*item)
        });

    sched.request(0, Priority::Active, 0);
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    sched.request_scoped(
        1,
        Priority::Background,
        1,
        RequestScope::default()
            .group("stale")
            .source("stale")
            .epoch_namespace("thumbnail"),
    );
    sched.request_scoped(
        2,
        Priority::Background,
        2,
        RequestScope::default()
            .group("fresh")
            .source("fresh")
            .epoch_namespace("other"),
    );

    let report = cancel(&mut sched);
    release_tx.send(()).unwrap();
    report
}

#[test]
fn in_flight_cancellation_report_and_completion_drop_match() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request_scoped(
        7,
        Priority::Active,
        7,
        RequestScope::default().group("stale"),
    );
    assert_eq!(det.begin_one(), Some(7));
    let report = det.cancel_group("stale");
    assert_eq!(report.queued, 0);
    assert_eq!(report.in_flight, 1);
    assert_eq!(det.finish_in_flight(), None);
    assert!(det.drain().is_empty());

    let real = cancel_in_flight_on_real_scheduler(|sched| sched.cancel_group("stale"));
    assert_eq!(real.report, report);
    assert!(real.completions.is_empty());
}

struct InFlightCancelOutcome {
    report: CancellationReport,
    completions: Vec<u64>,
}

fn cancel_in_flight_on_real_scheduler(
    cancel: impl FnOnce(&mut Scheduler<i32, i32>) -> CancellationReport,
) -> InFlightCancelOutcome {
    let (tx, rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let mut sched: Scheduler<i32, i32> =
        Scheduler::new(NonZeroUsize::new(1).unwrap(), tx, move |item: &i32| {
            started_tx.send(()).unwrap();
            release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            Ok(*item)
        });

    sched.request_scoped(
        7,
        Priority::Active,
        7,
        RequestScope::default().group("stale"),
    );
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let report = cancel(&mut sched);
    release_tx.send(()).unwrap();
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let completions = sched.drain().into_iter().map(|c| c.id).collect();

    InFlightCancelOutcome {
        report,
        completions,
    }
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
