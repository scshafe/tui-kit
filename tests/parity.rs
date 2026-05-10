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
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc, Mutex,
};
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
    let det_completions: Vec<(u64, i32)> = det
        .drain()
        .into_iter()
        .map(|c| (c.id, *c.result.as_ref().unwrap()))
        .collect();
    assert_eq!(det_completions, vec![(1, 1)]);
    det.request(1, Priority::Active, 3); // also ignored — already completed
    assert_eq!(det.queued_len(), 0);

    let real = dedup_by_id_on_real_scheduler();
    assert_eq!(real, det_completions);
}

fn dedup_by_id_on_real_scheduler() -> Vec<(u64, i32)> {
    let (tx, rx) = mpsc::channel();
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
    sched.request(1, Priority::Background, 1);
    sched.request(1, Priority::Background, 2); // ignored while queued
    release_tx.send(()).unwrap();

    for _ in 0..2 {
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
    }
    let completions: Vec<(u64, i32)> = sched
        .drain()
        .into_iter()
        .filter_map(|c| c.result.ok().map(|out| (c.id, out)))
        .filter(|(id, _)| *id == 1)
        .collect();

    sched.request(1, Priority::Background, 3); // ignored after successful completion
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());

    completions
}

#[test]
fn failed_id_resubmission_matches() {
    let mut attempts = 0;
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(move |item| {
        attempts += 1;
        if attempts == 1 {
            Err(anyhow::anyhow!("boom"))
        } else {
            Ok(*item)
        }
    });
    det.request(7, Priority::Active, 10);
    assert_eq!(det.run_one(), Some(7));
    assert!(det.drain().pop().unwrap().result.is_err());
    det.request(7, Priority::Active, 20);
    assert_eq!(det.run_one(), Some(7));
    let det_completion = det.drain().pop().unwrap();
    assert_eq!(det_completion.id, 7);
    assert_eq!(*det_completion.result.as_ref().unwrap(), 20);

    let (tx, rx) = mpsc::channel();
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_executor = attempts.clone();
    let mut real: Scheduler<i32, i32> =
        Scheduler::new(NonZeroUsize::new(1).unwrap(), tx, move |item: &i32| {
            if attempts_for_executor.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(anyhow::anyhow!("boom"))
            } else {
                Ok(*item)
            }
        });
    real.request(7, Priority::Active, 10);
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(real.drain().pop().unwrap().result.is_err());
    real.request(7, Priority::Active, 20);
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let real_completion = real.drain().pop().unwrap();
    assert_eq!(real_completion.id, 7);
    assert_eq!(*real_completion.result.as_ref().unwrap(), 20);
}

#[test]
fn cancelled_queued_id_resubmission_matches() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request(7, Priority::Active, 10);
    let report = det.cancel_id(7);
    assert_eq!(report.queued, 1);
    assert_eq!(report.in_flight, 0);
    det.request(7, Priority::Active, 20);
    assert_eq!(det.run_one(), Some(7));
    let det_completion = det.drain().pop().unwrap();
    assert_eq!(det_completion.id, 7);
    assert_eq!(*det_completion.result.as_ref().unwrap(), 20);

    let real = resubmit_cancelled_queued_id_on_real_scheduler();
    assert_eq!(real.report, report);
    assert_eq!(real.completions, vec![(7, 20)]);
}

#[test]
fn cancelled_in_flight_id_resubmission_matches() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request(7, Priority::Active, 10);
    assert_eq!(det.begin_one(), Some(7));
    let report = det.cancel_id(7);
    assert_eq!(report.queued, 0);
    assert_eq!(report.in_flight, 1);
    assert_eq!(det.finish_in_flight(), None);
    assert!(det.drain().is_empty());
    det.request(7, Priority::Active, 20);
    assert_eq!(det.run_one(), Some(7));
    let det_completion = det.drain().pop().unwrap();
    assert_eq!(det_completion.id, 7);
    assert_eq!(*det_completion.result.as_ref().unwrap(), 20);

    let real = resubmit_cancelled_in_flight_id_on_real_scheduler();
    assert_eq!(real.report, report);
    assert_eq!(real.completions, vec![(7, 20)]);
}

struct ResubmissionOutcome {
    report: CancellationReport,
    completions: Vec<(u64, i32)>,
}

fn resubmit_cancelled_queued_id_on_real_scheduler() -> ResubmissionOutcome {
    let (tx, rx) = mpsc::channel();
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
    sched.request(7, Priority::Background, 10);
    let report = sched.cancel_id(7);
    sched.request(7, Priority::Background, 20);
    release_tx.send(()).unwrap();

    for _ in 0..2 {
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
    }
    let completions = sched
        .drain()
        .into_iter()
        .filter_map(|c| c.result.ok().map(|out| (c.id, out)))
        .filter(|(id, _)| *id == 7)
        .collect();

    ResubmissionOutcome {
        report,
        completions,
    }
}

fn resubmit_cancelled_in_flight_id_on_real_scheduler() -> ResubmissionOutcome {
    let (tx, rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let mut sched: Scheduler<i32, i32> =
        Scheduler::new(NonZeroUsize::new(1).unwrap(), tx, move |item: &i32| {
            if *item == 10 {
                started_tx.send(()).unwrap();
                release_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap();
            }
            Ok(*item)
        });

    sched.request(7, Priority::Active, 10);
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let report = sched.cancel_id(7);
    release_tx.send(()).unwrap();
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(sched.drain().is_empty());

    sched.request(7, Priority::Active, 20);
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let completions = sched
        .drain()
        .into_iter()
        .filter_map(|c| c.result.ok().map(|out| (c.id, out)))
        .collect();

    ResubmissionOutcome {
        report,
        completions,
    }
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

#[test]
fn in_flight_id_cancellation_report_and_completion_drop_match() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request(7, Priority::Active, 7);
    assert_eq!(det.begin_one(), Some(7));
    let report = det.cancel_id(7);
    assert_eq!(report.queued, 0);
    assert_eq!(report.in_flight, 1);
    assert_eq!(det.finish_in_flight(), None);
    assert!(det.drain().is_empty());

    let real = cancel_scoped_in_flight_on_real_scheduler(RequestScope::default(), |sched| {
        sched.cancel_id(7)
    });
    assert_eq!(real.report, report);
    assert!(real.completions.is_empty());
}

#[test]
fn in_flight_source_cancellation_report_and_completion_drop_match() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request_scoped(
        7,
        Priority::Active,
        7,
        RequestScope::default().source("stale"),
    );
    assert_eq!(det.begin_one(), Some(7));
    let report = det.cancel_source("stale");
    assert_eq!(report.queued, 0);
    assert_eq!(report.in_flight, 1);
    assert_eq!(det.finish_in_flight(), None);
    assert!(det.drain().is_empty());

    let real = cancel_scoped_in_flight_on_real_scheduler(
        RequestScope::default().source("stale"),
        |sched| sched.cancel_source("stale"),
    );
    assert_eq!(real.report, report);
    assert!(real.completions.is_empty());
}

#[test]
fn in_flight_epoch_invalidation_report_and_completion_drop_match() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request_scoped(
        7,
        Priority::Active,
        7,
        RequestScope::default().epoch_namespace("thumbnail"),
    );
    assert_eq!(det.begin_one(), Some(7));
    let report = det.invalidate_epoch_namespace("thumbnail");
    assert_eq!(report.queued, 0);
    assert_eq!(report.in_flight, 1);
    assert_eq!(det.finish_in_flight(), None);
    assert!(det.drain().is_empty());

    let real = cancel_scoped_in_flight_on_real_scheduler(
        RequestScope::default().epoch_namespace("thumbnail"),
        |sched| sched.invalidate_epoch_namespace("thumbnail"),
    );
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
    cancel_scoped_in_flight_on_real_scheduler(RequestScope::default().group("stale"), cancel)
}

fn cancel_scoped_in_flight_on_real_scheduler(
    scope: RequestScope,
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

    sched.request_scoped(7, Priority::Active, 7, scope);
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
fn invalidate_all_drops_queued_and_in_flight_work_in_both() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request(1, Priority::Active, 1);
    det.request(2, Priority::Active, 2);
    assert_eq!(det.begin_one(), Some(1));
    det.invalidate_all();
    det.request(3, Priority::Active, 3);
    assert_eq!(det.finish_in_flight(), None);

    let ran = det.run_all();
    assert_eq!(ran, vec![3]);
    assert_eq!(det.cancelled_total(), 2);
    assert_eq!(
        det.drain().into_iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![3]
    );

    let real = invalidate_all_on_real_scheduler();
    assert_eq!(real.cancelled_total, det.cancelled_total());
    assert_eq!(real.completions, vec![3]);
}

#[test]
fn invalidate_all_resets_successful_id_dedup_in_both() {
    let mut det: DeterministicScheduler<i32, i32> = DeterministicScheduler::new(|item| Ok(*item));
    det.request(7, Priority::Active, 10);
    assert_eq!(det.run_one(), Some(7));
    assert_eq!(
        det.drain()
            .into_iter()
            .map(|c| (c.id, *c.result.as_ref().unwrap()))
            .collect::<Vec<_>>(),
        vec![(7, 10)]
    );

    det.request(7, Priority::Active, 20);
    assert_eq!(det.run_one(), None, "completed IDs dedup until invalidated");

    det.invalidate_all();
    det.request(7, Priority::Active, 30);
    assert_eq!(det.run_one(), Some(7));
    let det_completions: Vec<(u64, i32)> = det
        .drain()
        .into_iter()
        .map(|c| (c.id, *c.result.as_ref().unwrap()))
        .collect();
    assert_eq!(det_completions, vec![(7, 30)]);

    let real = resubmit_successful_id_after_invalidate_all_on_real_scheduler();
    assert_eq!(real, det_completions);
}

struct InvalidateAllOutcome {
    cancelled_total: usize,
    completions: Vec<u64>,
}

fn invalidate_all_on_real_scheduler() -> InvalidateAllOutcome {
    let (tx, rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let mut sched: Scheduler<i32, i32> =
        Scheduler::new(NonZeroUsize::new(1).unwrap(), tx, move |item: &i32| {
            if *item == 1 {
                started_tx.send(()).unwrap();
                release_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap();
            }
            Ok(*item)
        });

    sched.request(1, Priority::Active, 1);
    sched.request(2, Priority::Active, 2);
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    sched.invalidate_all();
    sched.request(3, Priority::Active, 3);
    release_tx.send(()).unwrap();

    for _ in 0..2 {
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
    }

    let cancelled_total = sched.stats().cancelled_total;
    let completions = sched.drain().into_iter().map(|c| c.id).collect();
    InvalidateAllOutcome {
        cancelled_total,
        completions,
    }
}

fn resubmit_successful_id_after_invalidate_all_on_real_scheduler() -> Vec<(u64, i32)> {
    let (tx, rx) = mpsc::channel();
    let mut sched: Scheduler<i32, i32> =
        Scheduler::new(NonZeroUsize::new(1).unwrap(), tx, |item: &i32| Ok(*item));

    sched.request(7, Priority::Active, 10);
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        sched
            .drain()
            .into_iter()
            .map(|c| (c.id, *c.result.as_ref().unwrap()))
            .collect::<Vec<_>>(),
        vec![(7, 10)]
    );

    sched.request(7, Priority::Active, 20);
    assert!(
        rx.recv_timeout(Duration::from_millis(100)).is_err(),
        "completed IDs dedup until invalidated"
    );

    sched.invalidate_all();
    sched.request(7, Priority::Active, 30);
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    sched
        .drain()
        .into_iter()
        .map(|c| (c.id, *c.result.as_ref().unwrap()))
        .collect()
}
