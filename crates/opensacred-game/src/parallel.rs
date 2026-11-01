//! Work spread over a pool of threads while a frame waits for it: a hero that arrives somewhere
//! waits for the sectors in view (`sectors`), and their tile sheets, their atlases' rows and their
//! sprites go over the cores (#1654). Bevy's own pools are kept small so that the frames run
//! steady (`main`'s `COMPUTE_THREADS`); this pool's threads sleep but while a frame waits anyway,
//! and take the work from one another, so that the sectors in view are done about together.

use std::sync::OnceLock;

use bevy::log::warn;
use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};

/// Whether a work runs on the calling thread alone, as a sector built in the background does, or
/// spreads over the pool of the frame's wait.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spread {
    Alone,
    Wide,
}

/// The threads that a frame's wait spreads over: half the logical cores, so that the render
/// thread, the sound's and the rest of the system keep theirs.
fn waiting_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |cores| (cores.get() / 2).max(1))
}

/// The pool of the frame's wait, made on its first use; none where it could not be made, and
/// the work then runs on the calling thread.
fn pool() -> Option<&'static ThreadPool> {
    static POOL: OnceLock<Option<ThreadPool>> = OnceLock::new();
    POOL.get_or_init(|| {
        ThreadPoolBuilder::new()
            .num_threads(waiting_threads())
            .thread_name(|index| format!("Waiting work ({index})"))
            .build()
            .inspect_err(|e| warn!("no pool of threads for the waits: {e}"))
            .ok()
    })
    .as_ref()
}

/// Maps `items` through `f`, the results in the items' order, spread as `spread` says.
pub fn map<T: Send, R: Send>(
    items: Vec<T>,
    spread: Spread,
    f: impl Fn(T) -> R + Sync + Send,
) -> Vec<R> {
    match (spread, pool()) {
        (Spread::Wide, Some(pool)) => pool.install(|| items.into_par_iter().map(f).collect()),
        _ => items.into_iter().map(f).collect(),
    }
}

/// Runs `a` and `b`, at once where `spread` spreads them; answers both.
pub fn join<A: Send, B: Send>(
    spread: Spread,
    a: impl FnOnce() -> A + Send,
    b: impl FnOnce() -> B + Send,
) -> (A, B) {
    match (spread, pool()) {
        (Spread::Wide, Some(pool)) => pool.install(|| rayon::join(a, b)),
        _ => (a(), b()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex;

    #[test]
    fn a_map_keeps_the_items_order_whatever_thread_maps_them() {
        let items: Vec<usize> = (0..1000).collect();
        let squares: Vec<usize> = items.iter().map(|i| i * i).collect();
        for spread in [Spread::Alone, Spread::Wide] {
            assert_eq!(map(items.clone(), spread, |i| i * i), squares, "{spread:?}");
        }
        assert_eq!(
            map(Vec::<usize>::new(), Spread::Wide, |i| i),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn a_work_alone_stays_on_the_calling_thread() {
        let caller = std::thread::current().id();
        let threads = Mutex::new(HashSet::new());
        map((0..64).collect(), Spread::Alone, |_: usize| {
            threads.lock().unwrap().insert(std::thread::current().id());
        });
        assert_eq!(*threads.lock().unwrap(), HashSet::from([caller]));
    }

    #[test]
    fn a_join_answers_both_parts() {
        for spread in [Spread::Alone, Spread::Wide] {
            assert_eq!(join(spread, || 1, || "b"), (1, "b"));
        }
    }

    #[test]
    #[should_panic(expected = "the item's own")]
    fn a_threads_panic_comes_back_to_the_caller() {
        map((0..8).collect(), Spread::Wide, |i: usize| {
            assert!(i != 5, "the item's own");
        });
    }
}
