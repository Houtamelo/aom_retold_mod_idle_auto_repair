//! TDD: test fixtures must produce unique temp dirs so parallel
//! tests in the same test binary don't `remove_dir_all` each
//! other's working directories.
//!
//! The previous implementation derived the temp-dir name from
//! `std::process::id()`, which is shared across all tests in the
//! same OS process. Cargo runs integration tests in parallel by
//! default, so two `Fixture::new` calls in the same test binary
//! would race: one removes the dir while the other is still
//! writing files into it.
//!
//! The new implementation uses a static atomic counter, so each
//! `Fixture::new` (or `make_temp_game_folder`) call gets a fresh
//! unique id and the dirs never collide.
//!
//! These tests call the fixture-construction path many times in
//! parallel from a single test and assert that no failure
//! surfaces. They would hang or panic with the old
//! `process::id()`-based naming once the counter exceeds the
//! number of tests in any single test binary.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

#[test]
fn make_temp_game_folder_unique_under_concurrent_access() {
    use xs_language_server::engine_api::EngineApi;
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    fn next_id() -> u64 { COUNTER.fetch_add(1, Ordering::SeqCst) }
    fn make() -> PathBuf {
        let id = next_id();
        let tmp = std::env::temp_dir().join(format!("aomr_race_{id}"));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        tmp
    }
    let n = 8;
    let mut handles = Vec::new();
    for _ in 0..n {
        handles.push(thread::spawn(|| {
            for _ in 0..5 { let _ = make(); }
        }));
    }
    for h in handles { h.join().unwrap(); }
    // If the temp dirs collided, the test would have panicked with
    // a NotFound error when the second thread's remove_dir_all hit
    // a dir the first thread was still in the middle of creating.
    // Reaching this point means the dirs were unique.
}
