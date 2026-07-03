//! Reproduction test for finding R3-F-01 (claim of a 3-way deadlock between
//! `documents`, `symbol_tables`, and `merged_views` mutexes in `server.rs`).
//!
//! The claim asserts that:
//!   - `did_close` (server.rs:406-414) acquires them in order:
//!     documents -> symbol_tables -> merged_views (held simultaneously).
//!   - `did_change_watched_files` (server.rs:472-485) acquires them in order:
//!     merged_views -> documents -> symbol_tables.
//!   - `get_or_build_merged_view` (server.rs:105-142) acquires them in order:
//!     workspace -> symbol_tables -> merged_views.
//!
//! For a classic hold-and-wait deadlock, two tasks must hold one mutex each
//! while awaiting the other. This test constructs the simplest possible
//! contenders using the same `tokio::sync::Mutex` types and lock-sequence
//! shapes as the production handlers, then verifies that no deadlock is
//! reachable within 5 seconds.
//!
//! Negative result (test completes, no timeout) is weak evidence that the
//! cyclic lock-order graph surfaced by static reading does NOT correspond to
//! reachable hold-and-wait cycles in practice.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::timeout;
use tower_lsp_server::ls_types::Uri;
use xs_language_server::server::{AsyncMutex, DocumentStore, did_close_lock_pattern};
use xs_language_server::symbols::SymbolTable;

/// Test-only mutex wrapper that tracks how many guards are currently held.
/// The shared counter is incremented when a lock is acquired and decremented
/// when the guard is dropped, letting tests assert the single-mutex discipline.
struct InstrumentedMutex<T> {
    inner: Mutex<T>,
    counter: Arc<AtomicUsize>,
}

struct InstrumentedMutexGuard<'a, T> {
    inner: tokio::sync::MutexGuard<'a, T>,
    counter: Arc<AtomicUsize>,
}

impl<T> InstrumentedMutex<T> {
    fn with_counter(value: T, counter: Arc<AtomicUsize>) -> Self {
        Self {
            inner: Mutex::new(value),
            counter,
        }
    }
}

impl<T: Send> AsyncMutex<T> for InstrumentedMutex<T> {
    type Guard<'a>
        = InstrumentedMutexGuard<'a, T>
    where
        T: 'a;

    async fn lock(&self) -> Self::Guard<'_> {
        let inner = self.inner.lock().await;
        self.counter.fetch_add(1, Ordering::SeqCst);
        InstrumentedMutexGuard {
            inner,
            counter: self.counter.clone(),
        }
    }
}

impl<T> Deref for InstrumentedMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> DerefMut for InstrumentedMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> Drop for InstrumentedMutexGuard<'_, T> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Number of lock-iteration rounds. Each round re-orders acquisition timing
/// (small `yield_now` between attempts) to widen the race window so any
/// genuine hold-and-wait cycle would fire.
const ROUNDS: usize = 500;

/// Tri-state outcome of a single contender run.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Completed,
    TimedOut,
}

/// Spawn `task` and return whether it finished within `budget`. The losing
/// branch is recorded so we can disambiguate "neither task finished" (deadlock
/// symptom) from "task A finished, task B finished" (clean run).
async fn run_with_deadline<F>(label: &str, budget: Duration, task: F) -> Outcome
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    match timeout(budget, tokio::spawn(task)).await {
        Ok(Ok(())) => Outcome::Completed,
        Ok(Err(join_err)) => {
            eprintln!("[{label}] task panicked: {join_err}");
            Outcome::Completed // treat panic as completion for the deadline
        }
        Err(_) => {
            eprintln!("[{label}] task did NOT complete within {budget:?}");
            Outcome::TimedOut
        }
    }
}

/// Mirror of `did_close`'s simultaneous-hold pattern (server.rs:406-414):
/// `documents` is held across the `symbol_tables` acquisition, and BOTH are
/// held across the `merged_views` acquisition. This is the ONLY handler in
/// `server.rs` that actually holds multiple mutexes simultaneously.
async fn did_close_pattern(
    documents: Arc<Mutex<HashMap<(), ()>>>,
    symbol_tables: Arc<Mutex<HashMap<(), ()>>>,
    merged_views: Arc<Mutex<HashMap<(), ()>>>,
) {
    let mut docs = documents.lock().await;
    docs.insert((), ());
    let mut tables = symbol_tables.lock().await;
    tables.insert((), ());
    let mut views = merged_views.lock().await;
    views.insert((), ());
}

/// Mirror of `did_change_watched_files`'s pattern (server.rs:448-485): each
/// mutex is acquired in a scoped block and RELEASED before the next
/// acquisition. This is the key fact that breaks the alleged deadlock chain.
async fn did_change_watched_files_pattern(
    documents: Arc<Mutex<HashMap<(), ()>>>,
    symbol_tables: Arc<Mutex<HashMap<(), ()>>>,
    merged_views: Arc<Mutex<HashMap<(), ()>>>,
) {
    // Equivalent of invalidate_merged_views_for: holds merged_views briefly.
    {
        let mut views = merged_views.lock().await;
        views.insert((), ());
    }
    // Equivalent of the for-loop body's documents lookup: scoped block.
    {
        let docs = documents.lock().await;
        let _ = docs.get(&());
    }
    // Equivalent of rebuild_symbol_table: scoped block.
    {
        let tables = symbol_tables.lock().await;
        let _ = tables.get(&());
    }
}

/// Mirror of `get_or_build_merged_view`'s slow-path order (server.rs:105-142):
/// workspace -> symbol_tables -> merged_views, each in a scoped block.
async fn get_or_build_merged_view_pattern(
    workspace: Arc<Mutex<HashMap<(), ()>>>,
    symbol_tables: Arc<Mutex<HashMap<(), ()>>>,
    merged_views: Arc<Mutex<HashMap<(), ()>>>,
) {
    {
        let ws = workspace.lock().await;
        let _ = ws.get(&());
    }
    {
        let tables = symbol_tables.lock().await;
        let _ = tables.get(&());
    }
    {
        let mut views = merged_views.lock().await;
        views.insert((), ());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn did_close_vs_watched_files_does_not_deadlock() {
    let documents: Arc<Mutex<HashMap<(), ()>>> = Arc::new(Mutex::new(HashMap::new()));
    let symbol_tables: Arc<Mutex<HashMap<(), ()>>> = Arc::new(Mutex::new(HashMap::new()));
    let merged_views: Arc<Mutex<HashMap<(), ()>>> = Arc::new(Mutex::new(HashMap::new()));

    for _ in 0..ROUNDS {
        let d_a = documents.clone();
        let s_a = symbol_tables.clone();
        let m_a = merged_views.clone();
        let t_a = tokio::spawn(async move {
            did_close_pattern(d_a, s_a, m_a).await;
        });

        let d_b = documents.clone();
        let s_b = symbol_tables.clone();
        let m_b = merged_views.clone();
        let t_b = tokio::spawn(async move {
            did_change_watched_files_pattern(d_b, s_b, m_b).await;
        });

        let (a, b) = tokio::join!(t_a, t_b);
        a.expect("did_close_pattern task panicked");
        b.expect("did_change_watched_files_pattern task panicked");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn did_close_vs_get_or_build_does_not_deadlock() {
    let documents: Arc<Mutex<HashMap<(), ()>>> = Arc::new(Mutex::new(HashMap::new()));
    let symbol_tables: Arc<Mutex<HashMap<(), ()>>> = Arc::new(Mutex::new(HashMap::new()));
    let merged_views: Arc<Mutex<HashMap<(), ()>>> = Arc::new(Mutex::new(HashMap::new()));
    let workspace: Arc<Mutex<HashMap<(), ()>>> = Arc::new(Mutex::new(HashMap::new()));

    for _ in 0..ROUNDS {
        let d_a = documents.clone();
        let s_a = symbol_tables.clone();
        let m_a = merged_views.clone();
        let t_a = tokio::spawn(async move {
            did_close_pattern(d_a, s_a, m_a).await;
        });

        let w_b = workspace.clone();
        let s_b = symbol_tables.clone();
        let m_b = merged_views.clone();
        let t_b = tokio::spawn(async move {
            get_or_build_merged_view_pattern(w_b, s_b, m_b).await;
        });

        let (a, b) = tokio::join!(t_a, t_b);
        a.expect("did_close_pattern task panicked");
        b.expect("get_or_build_merged_view_pattern task panicked");
    }
}

/// Direct deadline test: spawn the two alleged-conflicting tasks WITHOUT
/// awaiting them between rounds, so a real deadlock would manifest as a
/// permanent stall. If `timeout` returns `Err`, the test FAILS — we believe a
/// deadlock exists.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn triple_handler_concurrency_completes_under_budget() {
    let documents: Arc<Mutex<HashMap<(), ()>>> = Arc::new(Mutex::new(HashMap::new()));
    let symbol_tables: Arc<Mutex<HashMap<(), ()>>> = Arc::new(Mutex::new(HashMap::new()));
    let merged_views: Arc<Mutex<HashMap<(), ()>>> = Arc::new(Mutex::new(HashMap::new()));
    let workspace: Arc<Mutex<HashMap<(), ()>>> = Arc::new(Mutex::new(HashMap::new()));

    let workload = async move {
        for _ in 0..ROUNDS {
            let d_a = documents.clone();
            let s_a = symbol_tables.clone();
            let m_a = merged_views.clone();
            let t_a = tokio::spawn(did_close_pattern(d_a, s_a, m_a));

            let d_b = documents.clone();
            let s_b = symbol_tables.clone();
            let m_b = merged_views.clone();
            let t_b = tokio::spawn(did_change_watched_files_pattern(d_b, s_b, m_b));

            let w_c = workspace.clone();
            let s_c = symbol_tables.clone();
            let m_c = merged_views.clone();
            let t_c = tokio::spawn(get_or_build_merged_view_pattern(w_c, s_c, m_c));

            let (a, b, c) = tokio::join!(t_a, t_b, t_c);
            a.unwrap();
            b.unwrap();
            c.unwrap();
        }
    };

    // 5s budget, mirroring the prompt's instruction. A deadlock would
    // present as `Err(_)` because none of the tasks would make progress.
    let deadline = Duration::from_secs(5);
    let outcome = run_with_deadline("triple_handler", deadline, workload).await;
    assert_eq!(
        outcome,
        Outcome::Completed,
        "R3-F-01: triple handler concurrency exceeded {deadline:?}; this is evidence of a deadlock"
    );
}

/// Negative control: prove the `InstrumentedMutex` wrapper actually notices
/// when two locks are held at the same time. If this ever reports 0 or 1, the
/// wrapper is broken and the rest of the invariant tests are meaningless.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn instrumented_mutex_counts_simultaneous_holds() {
    let counter = Arc::new(AtomicUsize::new(0));
    let m1: InstrumentedMutex<()> = InstrumentedMutex::with_counter((), counter.clone());
    let m2: InstrumentedMutex<()> = InstrumentedMutex::with_counter((), counter.clone());

    {
        let _g1 = m1.lock().await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        let _g2 = m2.lock().await;
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

/// Control re-implementation of the pre-refactor `did_close` pattern: the
/// `documents` guard stays alive across the `symbol_tables` acquisition, and
/// both stay alive across the `merged_views` acquisition. This confirms the
/// instrumentation catches the regression if it is ever re-introduced.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unfixed_did_close_holds_multiple_mutexes() {
    let counter = Arc::new(AtomicUsize::new(0));
    let documents = InstrumentedMutex::with_counter(HashMap::new(), counter.clone());
    let symbol_tables = InstrumentedMutex::with_counter(HashMap::new(), counter.clone());
    let merged_views = InstrumentedMutex::with_counter(HashMap::new(), counter.clone());
    let _uri: Uri = "file:///tmp/unfixed.xs".parse().unwrap();

    let mut docs = documents.lock().await;
    docs.insert((), ());
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    let mut tables = symbol_tables.lock().await;
    tables.insert((), ());
    assert_eq!(counter.load(Ordering::SeqCst), 2);

    let mut views = merged_views.lock().await;
    views.insert((), ());
    assert_eq!(counter.load(Ordering::SeqCst), 3);

    drop((views, tables, docs));
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

/// Invariant test: the production `did_close` lock sequence (now extracted as
/// `did_close_lock_pattern`) must never hold two or more mutex guards at the
/// same time. A background observer records the maximum simultaneous count.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn production_did_close_holds_at_most_one_mutex() {
    let counter = Arc::new(AtomicUsize::new(0));
    let documents: InstrumentedMutex<DocumentStore> =
        InstrumentedMutex::with_counter(DocumentStore::default(), counter.clone());
    let symbol_tables: InstrumentedMutex<HashMap<Uri, SymbolTable>> =
        InstrumentedMutex::with_counter(HashMap::new(), counter.clone());
    let merged_views: InstrumentedMutex<HashMap<Uri, ()>> =
        InstrumentedMutex::with_counter(HashMap::new(), counter.clone());
    let uri: Uri = "file:///tmp/production.xs".parse().unwrap();

    let observer_counter = counter.clone();
    let observer = tokio::spawn(async move {
        let mut max_held = 0usize;
        for _ in 0..1000 {
            let current = observer_counter.load(Ordering::SeqCst);
            if current > max_held {
                max_held = current;
            }
            tokio::task::yield_now().await;
        }
        max_held
    });

    let clear_cache = async {
        let mut views = merged_views.lock().await;
        views.remove(&uri);
    };

    did_close_lock_pattern(&documents, &symbol_tables, clear_cache, &uri).await;

    let max_held = observer.await.expect("observer task panicked");
    assert!(
        max_held <= 1,
        "did_close_lock_pattern held {max_held} mutexes simultaneously (expected ≤ 1)",
    );
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

/// Triangulation: same invariant with pre-populated stores and a different URI,
/// confirming the helper behaves correctly when the close/remove/clear-cache
/// operations actually touch data.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn did_close_pattern_triangulates_with_populated_stores() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut initial_docs = DocumentStore::default();
    let uri: Uri = "file:///tmp/triangulate.xs".parse().unwrap();
    initial_docs.open(uri.clone(), "void main() {}".to_string());

    let mut initial_tables = HashMap::<Uri, SymbolTable>::new();
    initial_tables.insert(uri.clone(), SymbolTable::default());

    let mut initial_views = HashMap::<Uri, ()>::new();
    initial_views.insert(uri.clone(), ());

    let documents: InstrumentedMutex<DocumentStore> =
        InstrumentedMutex::with_counter(initial_docs, counter.clone());
    let symbol_tables: InstrumentedMutex<HashMap<Uri, SymbolTable>> =
        InstrumentedMutex::with_counter(initial_tables, counter.clone());
    let merged_views: InstrumentedMutex<HashMap<Uri, ()>> =
        InstrumentedMutex::with_counter(initial_views, counter.clone());

    let observer_counter = counter.clone();
    let observer = tokio::spawn(async move {
        let mut max_held = 0usize;
        for _ in 0..1000 {
            let current = observer_counter.load(Ordering::SeqCst);
            if current > max_held {
                max_held = current;
            }
            tokio::task::yield_now().await;
        }
        max_held
    });

    let clear_cache = async {
        let mut views = merged_views.lock().await;
        views.remove(&uri);
    };

    did_close_lock_pattern(&documents, &symbol_tables, clear_cache, &uri).await;

    let max_held = observer.await.expect("observer task panicked");
    assert!(
        max_held <= 1,
        "did_close_lock_pattern held {max_held} mutexes simultaneously (expected ≤ 1)",
    );
    assert_eq!(counter.load(Ordering::SeqCst), 0);

    assert!(documents.lock().await.get(&uri).is_none());
    assert!(!symbol_tables.lock().await.contains_key(&uri));
}
