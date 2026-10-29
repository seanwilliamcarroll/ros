// kernel/src/sync/spinlock/tests.rs
//
// Wire up with, at the bottom of sync/spinlock.rs:
//
//     #[cfg(test)]
//     mod tests;
//
// RawSpinLock's own battery covers the ticket mechanics; these cover the
// wrapper: access through the guard, release on drop, and the auto-traits
// that make it sound.

use super::{SpinLock, SpinLockGuard};
use core::cell::Cell;

// ---- Compile-time: auto-traits ----

const fn assert_sync<T: Sync>() {}

/// Fails to compile if `$ty` implements `$trait`. If it does, two impls
/// below apply, and inference of `_` becomes ambiguous.
/// (Same technique as the `static_assertions` crate.)
macro_rules! assert_not_impl {
    ($ty:ty, $trait:path) => {
        const _: fn() = || {
            trait AmbiguousIfImpl<A> {
                fn some_item() {}
            }
            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
            #[allow(dead_code)]
            struct Invalid;
            impl<T: ?Sized + $trait> AmbiguousIfImpl<Invalid> for T {}
            let _ = <$ty as AmbiguousIfImpl<_>>::some_item;
        };
    };
}

const _: () = {
    // Shareable (so it can be a static) whenever T can be handed between cores.
    assert_sync::<SpinLock<u32>>();
    // T only needs Send, not Sync: the lock lets one core in at a time.
    assert_sync::<SpinLock<Cell<u32>>>();
    // Sharing a guard shares &T, which is fine when T: Sync.
    assert_sync::<SpinLockGuard<'static, u32>>();
};

// Can't be handed to another core and unlocked there (the negative impl).
assert_not_impl!(SpinLockGuard<'static, u32>, Send);
// Sharing a guard to a Cell would let two cores call `set` at once.
assert_not_impl!(SpinLockGuard<'static, Cell<u32>>, Sync);
// Raw pointers aren't Send, so the lock must not become shareable.
assert_not_impl!(SpinLock<*const u32>, Sync);

// ---- Access through the guard ----

#[test_case]
fn guard_reads_initial_value() {
    let l = SpinLock::new(42u32);
    assert_eq!(*l.lock(), 42);
}

#[test_case]
fn guard_writes_through_deref_mut() {
    let l = SpinLock::new(0u32);
    {
        let mut g = l.lock();
        *g = 7;
        *g += 1;
    }
    assert_eq!(*l.lock(), 8);
}

#[test_case]
fn method_calls_auto_deref() {
    struct Counter {
        hits: u32,
    }
    impl Counter {
        fn hit(&mut self) {
            self.hits += 1;
        }
    }

    let l = SpinLock::new(Counter { hits: 0 });
    l.lock().hit();
    l.lock().hit();
    assert_eq!(l.lock().hits, 2);
}

#[test_case]
fn try_lock_guard_gives_access() {
    let l = SpinLock::new(1u32);
    {
        let mut g = l.try_lock().expect("lock is free");
        *g = 2;
    }
    assert_eq!(*l.lock(), 2);
}

#[test_case]
fn works_with_non_copy_data() {
    let l = SpinLock::new([0u8; 64]);
    l.lock()[63] = 0xAB;
    assert_eq!(l.lock()[63], 0xAB);
    assert_eq!(l.lock()[0], 0);
}

#[test_case]
fn works_with_zero_sized_data() {
    let l = SpinLock::new(());
    let g = l.lock();
    assert!(l.try_lock().is_none());
    drop(g);
    assert!(l.try_lock().is_some());
}

#[test_case]
fn cell_inside_lock() {
    let l = SpinLock::new(Cell::new(0u32));
    let g = l.lock();
    g.set(g.get() + 1); // writes through a shared &Cell, guarded by the lock
    drop(g);
    assert_eq!(l.lock().get(), 1);
}

// ---- Exclusion and release ----

#[test_case]
fn try_lock_none_while_guard_alive() {
    let l = SpinLock::new(0u32);
    let _g = l.lock();
    assert!(l.try_lock().is_none());
}

#[test_case]
fn try_lock_none_while_try_lock_guard_alive() {
    let l = SpinLock::new(0u32);
    let _g = l.try_lock().expect("lock is free");
    assert!(l.try_lock().is_none());
}

#[test_case]
fn scope_end_releases() {
    let l = SpinLock::new(0u32);
    {
        let _g = l.lock();
        assert!(l.try_lock().is_none());
    }
    assert!(l.try_lock().is_some());
}

#[test_case]
fn explicit_drop_releases() {
    let l = SpinLock::new(0u32);
    let g = l.lock();
    drop(g);
    let _g2 = l.lock(); // would hang if drop didn't unlock
}

#[test_case]
fn temporary_guard_releases_at_end_of_statement() {
    let l = SpinLock::new(0u32);
    *l.lock() = 5; // guard is a temporary, dropped at the `;`
    assert!(l.try_lock().is_some());
    assert_eq!(*l.lock(), 5);
}

#[test_case]
fn failed_try_lock_does_not_disturb_holder() {
    let l = SpinLock::new(0u32);
    let mut g = l.lock();
    for _ in 0..100 {
        assert!(l.try_lock().is_none());
    }
    *g = 9; // holder still has access
    drop(g);
    assert_eq!(*l.lock(), 9);
}

#[test_case]
fn many_cycles() {
    const N: u32 = 10_000;
    let l = SpinLock::new(0u32);
    for i in 0..N {
        if i % 2 == 0 {
            *l.lock() += 1;
        } else {
            *l.try_lock().expect("lock is free") += 1;
        }
    }
    assert_eq!(*l.lock(), N);
}

// ---- Statics ----

static COUNTER: SpinLock<u32> = SpinLock::new(0);

#[test_case]
fn static_lock_is_usable() {
    *COUNTER.lock() += 1;
    let g = COUNTER.lock();
    assert!(COUNTER.try_lock().is_none());
    assert_eq!(*g, 1);
}
