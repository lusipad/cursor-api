//! Replicating iterator: N−1 clones followed by moving the original.
//!
//! When you need several identical values built from one source, the naïve
//! approach clones all N. This crate clones N−1 and **moves** the original
//! on the final iteration — saving one clone for heap-backed types like
//! `Vec`, `String`, or `Arc`-wrapped data.
//!
//! Two iterator types are provided:
//!
//! | Type | Length known up front? | Who controls the count? |
//! |------|----------------------|------------------------|
//! | [`RepMove`] | Yes — [`ExactSizeIterator`] | Fixed at construction |
//! | [`UncheckedRepMove`] | No | Closure can stop early |
//!
//! # Quick start
//!
//! ```
//! use rep_move::RepMove;
//!
//! let original = vec![1, 2, 3];
//! let copies: Vec<_> = RepMove::new(original, Vec::clone, 4).collect();
//! // 3 heap allocations instead of 4 — the last one was moved for free.
//! assert_eq!(copies.len(), 4);
//! ```

#![no_std]
#![feature(const_destruct)]
#![feature(const_trait_impl)]

use core::{
    fmt,
    iter::FusedIterator,
    marker::{Destruct, PhantomData},
    mem,
};

// ━━━━━━━━━━━━━━━━━━━━━ Internal state ━━━━━━━━━━━━━━━━━━━━━

/// Backing representation shared by all replicating iterators.
///
/// `Active` owns the source and replication machinery.
/// `Done` is a zero-size tombstone — the source has been moved out and
/// no heap-weight or closure state remains.
enum State<T, R> {
    Active { source: T, remaining: usize, rep_fn: R },
    Done,
}

impl<T, R> State<T, R> {
    /// Extracts the source value, transitioning to `Done`.
    ///
    /// Called exactly once per iterator lifetime, on the final `next()`.
    /// Callers always guard this behind a state check, so the `else`
    /// branch is structurally unreachable.
    #[inline]
    fn take_source(&mut self) -> T {
        let State::Active { source, .. } = mem::replace(self, State::Done) else {
            unreachable!()
        };
        source
    }
}

// ━━━━━━━━━━━━━━━━━━━━━ Replicator trait ━━━━━━━━━━━━━━━━━━━━━

/// Strategy for producing a replica from a borrowed source.
///
/// You won't implement this directly — blanket impls cover two closure shapes:
///
/// - **`FnMut(&T) -> T`** — simple cloner, ignores position.
/// - **`FnMut(&T, usize) -> T`** — receives the remaining replica count
///   (counts down toward 0, at which point the original is moved instead).
///
/// The `Args` type parameter exists solely to disambiguate these two impls
/// at the trait level; it has no runtime cost.
pub const trait Replicator<Args, T> {
    fn replicate(&mut self, source: &T, remaining: usize) -> T;
}

impl<T, F> const Replicator<(&T,), T> for F
where
    F: [const] FnMut(&T) -> T,
{
    #[inline]
    fn replicate(&mut self, source: &T, _remaining: usize) -> T {
        self(source)
    }
}

impl<T, F> const Replicator<(&T, usize), T> for F
where
    F: [const] FnMut(&T, usize) -> T,
{
    #[inline]
    fn replicate(&mut self, source: &T, remaining: usize) -> T {
        self(source, remaining)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━ RepMove ━━━━━━━━━━━━━━━━━━━━━

/// Fixed-length replicating iterator.
///
/// Produces `count − 1` replicas via `rep_fn`, then yields the original
/// by move. The total output is always exactly `count` items.
///
/// # Closure signatures
///
/// | Signature | Use when… |
/// |-----------|-----------|
/// | `(&T) -> T` | You just need copies — pass `Clone::clone` |
/// | `(&T, usize) -> T` | You want to vary output based on position |
///
/// # Examples
///
/// Three identical vectors, only two of which allocate:
///
/// ```
/// # use rep_move::RepMove;
/// let v = vec![1, 2, 3];
/// let mut iter = RepMove::new(v, Vec::clone, 3);
///
/// assert_eq!(iter.next(), Some(vec![1, 2, 3])); // clone
/// assert_eq!(iter.next(), Some(vec![1, 2, 3])); // clone
/// assert_eq!(iter.next(), Some(vec![1, 2, 3])); // moved — no allocation
/// assert_eq!(iter.next(), None);
/// ```
///
/// Remaining-aware replication — the `usize` counts down:
///
/// ```
/// # use rep_move::RepMove;
/// let s = String::from("item");
/// let tagged: Vec<_> = RepMove::new(
///     s,
///     |s: &String, n| format!("{s}-{n}"),
///     3,
/// ).collect();
///
/// assert_eq!(tagged, ["item-2", "item-1", "item"]);
/// ```
#[must_use = "iterators do nothing unless consumed"]
pub struct RepMove<Args, T, R: Replicator<Args, T>> {
    state: State<T, R>,
    _marker: PhantomData<Args>,
}

impl<Args, T, R: Replicator<Args, T>> RepMove<Args, T, R> {
    /// Creates an iterator that yields `count` items total.
    ///
    /// When `count` is 0, `source` and `rep_fn` are dropped immediately
    /// and the iterator is born exhausted.
    #[inline]
    pub const fn new(source: T, rep_fn: R, count: usize) -> Self
    where
        T: [const] Destruct,
        R: [const] Destruct,
    {
        match count.checked_sub(1) {
            Some(remaining) => Self {
                state: State::Active { source, remaining, rep_fn },
                _marker: PhantomData,
            },
            None => Self::empty(),
        }
    }

    /// An already-exhausted iterator. Yields nothing, carries no data.
    #[inline]
    pub const fn empty() -> Self {
        Self { state: State::Done, _marker: PhantomData }
    }

    /// Recovers the source value without iterating.
    ///
    /// Returns `None` if the iterator is already exhausted (the source
    /// was either moved out by `next()` or dropped by a zero-count `new()`).
    #[inline]
    pub fn into_inner(self) -> Option<T> {
        match self.state {
            State::Active { source, .. } => Some(source),
            State::Done => None,
        }
    }
}

impl<Args, T, R: Replicator<Args, T>> Iterator for RepMove<Args, T, R> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // Phase 1: produce a replica while borrowing the source in-place.
        // The mutable borrow of self.state lives only inside this match.
        match &mut self.state {
            State::Done => return None,
            State::Active { source, remaining, rep_fn } => {
                if let Some(next) = remaining.checked_sub(1) {
                    let item = rep_fn.replicate(source, *remaining);
                    *remaining = next;
                    return Some(item);
                }
            }
        }
        // Phase 2: remaining hit 0 — move the original out.
        // Borrow from phase 1 is released, so we can transition state.
        Some(self.state.take_source())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<Args, T, R: Replicator<Args, T>> ExactSizeIterator for RepMove<Args, T, R> {
    #[inline]
    fn len(&self) -> usize {
        match self.state {
            // `remaining` counts only replicas; +1 for the original.
            // Overflow is impossible: remaining ≤ count−1 by construction.
            State::Active { remaining, .. } => remaining + 1,
            State::Done => 0,
        }
    }
}

impl<Args, T, R: Replicator<Args, T>> FusedIterator for RepMove<Args, T, R> {}

impl<Args, T: fmt::Debug, R: Replicator<Args, T>> fmt::Debug for RepMove<Args, T, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.state {
            State::Active { source, remaining, .. } => f
                .debug_struct("RepMove")
                .field("source", source)
                .field("items_left", &(*remaining + 1))
                .finish_non_exhaustive(),
            State::Done => write!(f, "RepMove(exhausted)"),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━ UncheckedRepMove ━━━━━━━━━━━━━━━━━━━━━

/// Dynamically-controlled replicating iterator.
///
/// Unlike [`RepMove`], the closure receives `&mut usize` pointing at the
/// remaining replica count and is responsible for decrementing it. This
/// enables early termination based on conditions discovered *during*
/// replication — e.g. resource limits, error thresholds, or content-dependent
/// stopping criteria.
///
/// # Contract
///
/// - The closure **must not increase** the remaining count.
///   Doing so risks unbounded iteration.
/// - The closure **must** eventually bring it to zero,
///   at which point the original is moved out as the final item.
///
/// Does *not* implement [`ExactSizeIterator`] because the closure can
/// alter the count unpredictably.
///
/// # Examples
///
/// Standard countdown — functionally equivalent to [`RepMove`]:
///
/// ```
/// # use rep_move::UncheckedRepMove;
/// let v = vec![1, 2, 3];
/// let items: Vec<_> = UncheckedRepMove::new(
///     v,
///     |v: &Vec<i32>, n: &mut usize| { *n -= 1; v.clone() },
///     4,
/// ).collect();
/// assert_eq!(items.len(), 4); // 3 clones + 1 move
/// ```
///
/// Early exit — closure zeroes the count to stop immediately:
///
/// ```
/// # use rep_move::UncheckedRepMove;
/// let v = vec![1, 2, 3];
/// let mut iter = UncheckedRepMove::new(
///     v,
///     |_v: &Vec<i32>, n: &mut usize| { *n = 0; Vec::new() },
///     100,
/// );
/// assert!(iter.next().is_some());  // one replica (remaining set to 0)
/// assert!(iter.next().is_some());  // original moved out
/// assert!(iter.next().is_none());  // done
/// ```
#[must_use = "iterators do nothing unless consumed"]
pub struct UncheckedRepMove<T, R> {
    state: State<T, R>,
}

impl<T, R: FnMut(&T, &mut usize) -> T> UncheckedRepMove<T, R> {
    /// Creates a dynamically-controlled replicating iterator.
    ///
    /// See the [type-level docs](Self) for the contract on the closure.
    #[inline]
    pub const fn new(source: T, rep_fn: R, count: usize) -> Self
    where
        T: [const] Destruct,
        R: [const] Destruct,
    {
        match count.checked_sub(1) {
            Some(remaining) => Self {
                state: State::Active { source, remaining, rep_fn },
            },
            None => Self::empty(),
        }
    }

    /// An already-exhausted iterator. Yields nothing, carries no data.
    #[inline]
    pub const fn empty() -> Self {
        Self { state: State::Done }
    }

    /// Recovers the source value without iterating.
    ///
    /// Returns `None` if the iterator is already exhausted.
    #[inline]
    pub fn into_inner(self) -> Option<T> {
        match self.state {
            State::Active { source, .. } => Some(source),
            State::Done => None,
        }
    }
}

impl<T, R: FnMut(&T, &mut usize) -> T> Iterator for UncheckedRepMove<T, R> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            State::Done => return None,
            State::Active { source, remaining, rep_fn } => {
                if *remaining > 0 {
                    // Closure is responsible for decrementing `remaining`.
                    return Some(rep_fn(source, remaining));
                }
            }
        }
        Some(self.state.take_source())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.state {
            // Lower bound: at least the original will be yielded.
            // Upper bound: remaining replicas + the original (may overflow to None).
            State::Active { remaining, .. } => (1, remaining.checked_add(1)),
            State::Done => (0, Some(0)),
        }
    }
}

impl<T, R: FnMut(&T, &mut usize) -> T> FusedIterator for UncheckedRepMove<T, R> {}

impl<T: fmt::Debug, R> fmt::Debug for UncheckedRepMove<T, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.state {
            State::Active { source, remaining, .. } => f
                .debug_struct("UncheckedRepMove")
                .field("source", source)
                .field("remaining", remaining)
                .finish_non_exhaustive(),
            State::Done => write!(f, "UncheckedRepMove(exhausted)"),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━ Tests ━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    extern crate alloc;

    use alloc::{
        format,
        string::{String, ToString as _},
        vec,
        vec::Vec,
    };

    // ── RepMove ──

    #[test]
    fn clone_three_times() {
        let v = vec![1, 2, 3];
        let mut iter = RepMove::new(v, Vec::clone, 3);

        assert_eq!(iter.len(), 3);
        assert_eq!(iter.next(), Some(vec![1, 2, 3]));
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.next(), Some(vec![1, 2, 3]));
        assert_eq!(iter.len(), 1);
        assert_eq!(iter.next(), Some(vec![1, 2, 3])); // moved
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn remaining_aware_closure() {
        let s = String::from("test");
        let mut iter = RepMove::new(s, |s: &String, n: usize| format!("{s}-{n}"), 2);

        assert_eq!(iter.next(), Some("test-1".to_string()));
        assert_eq!(iter.next(), Some("test".to_string())); // original moved
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn zero_count_drops_source() {
        let v = vec![1, 2, 3];
        let mut iter = RepMove::new(v, Vec::clone, 0);
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn single_item_is_moved_not_cloned() {
        let v = vec![42];
        let mut iter = RepMove::new(v, Vec::clone, 1);
        assert_eq!(iter.len(), 1);
        assert_eq!(iter.next(), Some(vec![42]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn empty_yields_nothing() {
        let iter = RepMove::<_, i32, fn(&i32) -> i32>::empty();
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.into_inner(), None);
    }

    #[test]
    fn into_inner_recovers_source() {
        let v = vec![1, 2, 3];
        let iter = RepMove::new(v, Vec::clone, 5);
        assert_eq!(iter.into_inner(), Some(vec![1, 2, 3]));
    }

    #[test]
    fn into_inner_after_exhaust() {
        let mut iter = RepMove::new(42i32, |x: &i32| *x, 1);
        iter.next();
        assert_eq!(iter.into_inner(), None);
    }

    #[test]
    fn fused_after_exhaustion() {
        let mut iter = RepMove::new(42i32, |x: &i32| *x, 2);
        assert_eq!(iter.next(), Some(42));
        assert_eq!(iter.next(), Some(42));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn size_hint_always_exact() {
        let mut iter = RepMove::new(0u8, |x: &u8| *x, 3);
        assert_eq!(iter.size_hint(), (3, Some(3)));
        iter.next();
        assert_eq!(iter.size_hint(), (2, Some(2)));
        iter.next();
        assert_eq!(iter.size_hint(), (1, Some(1)));
        iter.next();
        assert_eq!(iter.size_hint(), (0, Some(0)));
    }

    #[test]
    fn debug_active_and_exhausted() {
        let iter = RepMove::new(vec![1, 2], Vec::clone, 2);
        let dbg = format!("{iter:?}");
        assert!(dbg.contains("RepMove"));
        assert!(dbg.contains("[1, 2]"));

        let iter = RepMove::new(0u8, |x: &u8| *x, 0);
        let dbg = format!("{iter:?}");
        assert!(dbg.contains("exhausted"));
    }

    // ── UncheckedRepMove ──

    #[test]
    fn unchecked_early_stop() {
        let v = vec![1, 2, 3];
        let mut iter = UncheckedRepMove::new(
            v,
            |v: &Vec<i32>, remaining: &mut usize| {
                *remaining = 0; // stop after this replica
                v.clone()
            },
            5,
        );

        assert_eq!(iter.next(), Some(vec![1, 2, 3])); // clone, remaining→0
        assert_eq!(iter.next(), Some(vec![1, 2, 3])); // original moved
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn unchecked_gradual_countdown() {
        let v = vec![1, 2, 3];
        let mut iter = UncheckedRepMove::new(
            v,
            |v: &Vec<i32>, remaining: &mut usize| {
                *remaining = remaining.saturating_sub(1);
                v.clone()
            },
            4,
        );

        assert_eq!(iter.next(), Some(vec![1, 2, 3]));
        assert_eq!(iter.next(), Some(vec![1, 2, 3]));
        assert_eq!(iter.next(), Some(vec![1, 2, 3]));
        assert_eq!(iter.next(), Some(vec![1, 2, 3])); // original moved
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn unchecked_size_hint_conservative() {
        let mut iter = UncheckedRepMove::new(
            0u8,
            |x: &u8, remaining: &mut usize| {
                *remaining = remaining.saturating_sub(1);
                *x
            },
            3,
        );
        // Lower bound always 1 while active; upper bound shrinks.
        assert_eq!(iter.size_hint(), (1, Some(3)));
        iter.next();
        assert_eq!(iter.size_hint(), (1, Some(2)));
    }

    #[test]
    fn unchecked_empty() {
        let iter = UncheckedRepMove::<i32, fn(&i32, &mut usize) -> i32>::empty();
        assert_eq!(iter.into_inner(), None);
    }
}
