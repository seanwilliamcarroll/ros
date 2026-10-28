// Everything runs single-core, so these test the state machine
// (tickets, hand-off, wraparound), not real contention. States that need a
// second core (a queued waiter) are constructed directly through the private
// fields, which this child module can see.
//
// SAFETY (for every `unlock` below): each call is paired with a successful
// `lock`/`try_lock`, or stands in for the holder of a manufactured state.

use super::RawSpinLock;
use core::sync::atomic::{AtomicU32, Ordering::Relaxed};

/// A free lock whose counters both start at `start`.
fn at(start: u32) -> RawSpinLock {
    RawSpinLock {
        next: AtomicU32::new(start),
        serving: AtomicU32::new(start),
    }
}

/// (next, serving)
fn state(l: &RawSpinLock) -> (u32, u32) {
    (l.next.load(Relaxed), l.serving.load(Relaxed))
}

// ---- Compile-time checks ----

// Must be Sync to live in a static; Send for good measure.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RawSpinLock>();
};

// `new` must be const so it can initialize a static.
static GLOBAL: RawSpinLock = RawSpinLock::new();

// ---- Basic acquire / release ----

#[test_case]
fn new_starts_unlocked() {
    let l = RawSpinLock::new();
    assert_eq!(state(&l), (0, 0));
    assert!(l.try_lock());
    unsafe { l.unlock() };
}

#[test_case]
fn lock_takes_ticket_and_unlock_serves_it() {
    let l = RawSpinLock::new();
    l.lock();
    assert_eq!(state(&l), (1, 0));
    unsafe { l.unlock() };
    assert_eq!(state(&l), (1, 1));
}

#[test_case]
fn try_lock_fails_while_held_by_lock() {
    let l = RawSpinLock::new();
    l.lock();
    assert!(!l.try_lock());
    unsafe { l.unlock() };
}

#[test_case]
fn try_lock_fails_while_held_by_try_lock() {
    let l = RawSpinLock::new();
    assert!(l.try_lock());
    assert!(!l.try_lock());
    unsafe { l.unlock() };
}

#[test_case]
fn try_lock_succeeds_after_unlock() {
    let l = RawSpinLock::new();
    l.lock();
    unsafe { l.unlock() };
    assert!(l.try_lock());
    unsafe { l.unlock() };
}

#[test_case]
fn lock_succeeds_after_try_lock_released() {
    let l = RawSpinLock::new();
    assert!(l.try_lock());
    unsafe { l.unlock() };
    l.lock(); // would hang if try_lock left the counters unbalanced
    unsafe { l.unlock() };
    assert_eq!(state(&l), (2, 2));
}

// ---- Ticket accounting ----

// A failed try_lock must not take a ticket: nobody would ever redeem it,
// and every later `lock` would spin forever.
#[test_case]
fn failed_try_lock_does_not_take_ticket() {
    let l = RawSpinLock::new();
    l.lock();
    let before = state(&l);
    for _ in 0..100 {
        assert!(!l.try_lock());
    }
    assert_eq!(state(&l), before);
    unsafe { l.unlock() };
}

#[test_case]
fn many_cycles_keep_counters_balanced() {
    const N: u32 = 10_000;
    let l = RawSpinLock::new();
    for i in 0..N {
        if i % 2 == 0 {
            l.lock();
        } else {
            assert!(l.try_lock());
        }
        let (next, serving) = state(&l);
        assert_eq!(next, serving + 1, "held: exactly one ticket outstanding");
        unsafe { l.unlock() };
        let (next, serving) = state(&l);
        assert_eq!(next, serving, "free: counters equal");
    }
    assert_eq!(state(&l), (N, N));
}

// ---- Queue behaviour (manufactured multi-core states) ----

// Holder has ticket 0, a waiter on another core has ticket 1.
fn held_with_one_waiter() -> RawSpinLock {
    RawSpinLock {
        next: AtomicU32::new(2),
        serving: AtomicU32::new(0),
    }
}

#[test_case]
fn try_lock_fails_with_waiter_queued() {
    let l = held_with_one_waiter();
    assert!(!l.try_lock());
    assert_eq!(state(&l), (2, 0));
}

// Unlock hands the lock to ticket 1, not to whoever calls try_lock next:
// try_lock can't jump the queue.
#[test_case]
fn unlock_hands_off_to_next_ticket() {
    let l = held_with_one_waiter();
    unsafe { l.unlock() }; // holder (ticket 0) leaves
    assert_eq!(state(&l), (2, 1));
    assert!(!l.try_lock(), "ticket 1 is being served; lock is not free");
    unsafe { l.unlock() }; // stand in for the waiter (ticket 1) leaving
    assert_eq!(state(&l), (2, 2));
    assert!(l.try_lock());
    unsafe { l.unlock() };
}

// ---- Wraparound ----
// Test builds have overflow checks on, so a stray `+ 1` panics here.

#[test_case]
fn lock_wraps_past_u32_max() {
    let l = at(u32::MAX - 2);
    for _ in 0..5 {
        l.lock();
        unsafe { l.unlock() };
    }
    assert_eq!(state(&l), (2, 2));
}

#[test_case]
fn try_lock_wraps_past_u32_max() {
    let l = at(u32::MAX - 2);
    for _ in 0..5 {
        assert!(l.try_lock());
        assert!(!l.try_lock());
        unsafe { l.unlock() };
    }
    assert_eq!(state(&l), (2, 2));
}

#[test_case]
fn held_exactly_at_u32_max_boundary() {
    let l = at(u32::MAX);
    l.lock();
    assert_eq!(state(&l), (0, u32::MAX)); // next wrapped, holder is served
    assert!(!l.try_lock());
    unsafe { l.unlock() };
    assert_eq!(state(&l), (0, 0));
    assert!(l.try_lock());
    unsafe { l.unlock() };
}

// ---- Statics ----

#[test_case]
fn static_lock_is_usable() {
    GLOBAL.lock();
    assert!(!GLOBAL.try_lock());
    unsafe { GLOBAL.unlock() };
    assert!(GLOBAL.try_lock());
    unsafe { GLOBAL.unlock() };
}
