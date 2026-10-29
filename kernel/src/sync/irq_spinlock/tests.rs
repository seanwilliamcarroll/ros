// kernel/src/sync/irq_spinlock/tests.rs
//
// Wire up with, at the bottom of sync/irq_spinlock.rs:
//
//     #[cfg(test)]
//     mod tests;
//
// The test kernel has no IDT or PIC setup yet, so turning interrupts ON would
// triple-fault at the first pending interrupt. Every test here therefore
// starts by turning interrupts OFF, which is always safe, and checks that the
// lock never turns them back on. Tests that need interrupts enabled are
// listed as TODOs at the bottom.

use super::{IrqSpinLock, IrqSpinLockGuard};
use core::cell::Cell;
use x86_64::instructions::interrupts;

/// Put the core in a known state. Disabling never faults.
fn start_disabled() {
    interrupts::disable();
    assert!(!interrupts::are_enabled());
}

fn assert_disabled() {
    assert!(!interrupts::are_enabled(), "interrupts were re-enabled");
}

// ---- Compile-time: auto-traits ----
// The same checks as SpinLock's. If you'd rather not duplicate the macro,
// move it to a shared `sync/test_util.rs` behind `#[cfg(test)]`.

const fn assert_sync<T: Sync>() {}

/// Fails to compile if `$ty` implements `$trait`. If it does, two impls
/// apply, and inference of `_` becomes ambiguous.
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
    assert_sync::<IrqSpinLock<u32>>();
    // T needs only Send: one core at a time, same as SpinLock.
    assert_sync::<IrqSpinLock<Cell<u32>>>();
    assert_sync::<IrqSpinLockGuard<'static, u32>>();
};

// The guard holds THIS core's saved interrupt state. It must never move.
assert_not_impl!(IrqSpinLockGuard<'static, u32>, Send);
// Sharing a guard shares &T, so it needs T: Sync.
assert_not_impl!(IrqSpinLockGuard<'static, Cell<u32>>, Sync);
assert_not_impl!(IrqSpinLock<*const u32>, Sync);

// ---- Interrupt state (start-disabled cases) ----

#[test_case]
fn lock_records_disabled_state() {
    start_disabled();
    let l = IrqSpinLock::new(0u32);
    let g = l.lock();
    assert!(!g.interrupts_were_enabled);
    assert_disabled();
}

#[test_case]
fn drop_leaves_interrupts_disabled() {
    start_disabled();
    let l = IrqSpinLock::new(0u32);
    drop(l.lock());
    assert_disabled(); // must restore, not blindly `sti`
}

#[test_case]
fn try_lock_success_records_and_restores_disabled() {
    start_disabled();
    let l = IrqSpinLock::new(0u32);
    let g = l.try_lock().expect("lock is free");
    assert!(!g.interrupts_were_enabled);
    drop(g);
    assert_disabled();
}

#[test_case]
fn try_lock_failure_leaves_interrupts_disabled() {
    start_disabled();
    let l = IrqSpinLock::new(0u32);
    let _g = l.lock();
    assert!(l.try_lock().is_none());
    assert_disabled(); // failure path must not `sti` either
}

#[test_case]
fn nested_locks_stay_disabled_throughout() {
    start_disabled();
    let outer = IrqSpinLock::new(0u32);
    let inner = IrqSpinLock::new(0u32);

    let go = outer.lock();
    let gi = inner.lock();
    assert_disabled();

    drop(gi);
    assert_disabled();
    assert!(outer.try_lock().is_none(), "outer must still be held");

    drop(go);
    assert_disabled();
}

#[test_case]
fn many_cycles_never_enable() {
    start_disabled();
    let l = IrqSpinLock::new(0u32);
    for i in 0..10_000u32 {
        if i % 2 == 0 {
            *l.lock() += 1;
        } else {
            *l.try_lock().expect("lock is free") += 1;
        }
        assert_disabled();
    }
    assert_eq!(*l.lock(), 10_000);
}

// ---- Access through the guard ----

#[test_case]
fn guard_reads_and_writes() {
    start_disabled();
    let l = IrqSpinLock::new(1u32);
    {
        let mut g = l.lock();
        assert_eq!(*g, 1);
        *g = 2;
    }
    assert_eq!(*l.lock(), 2);
}

#[test_case]
fn try_lock_guard_gives_access() {
    start_disabled();
    let l = IrqSpinLock::new(0u32);
    *l.try_lock().expect("lock is free") = 5;
    assert_eq!(*l.lock(), 5);
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

    start_disabled();
    let l = IrqSpinLock::new(Counter { hits: 0 });
    l.lock().hit();
    l.lock().hit();
    assert_eq!(l.lock().hits, 2);
}

#[test_case]
fn works_with_non_copy_data() {
    start_disabled();
    let l = IrqSpinLock::new([0u8; 64]);
    l.lock()[63] = 0xAB;
    assert_eq!(l.lock()[63], 0xAB);
    assert_eq!(l.lock()[0], 0);
}

#[test_case]
fn works_with_zero_sized_data() {
    start_disabled();
    let l = IrqSpinLock::new(());
    let g = l.lock();
    assert!(l.try_lock().is_none());
    drop(g);
    assert!(l.try_lock().is_some());
}

#[test_case]
fn cell_inside_lock() {
    start_disabled();
    let l = IrqSpinLock::new(Cell::new(0u32));
    let g = l.lock();
    g.set(g.get() + 1);
    drop(g);
    assert_eq!(l.lock().get(), 1);
}

#[test_case]
fn get_mut_and_into_inner() {
    let mut l = IrqSpinLock::new(1u32);
    *l.get_mut() = 3; // no locking: &mut proves exclusivity
    assert_eq!(l.into_inner(), 3);
}

// ---- Exclusion and release ----

#[test_case]
fn try_lock_none_while_guard_alive() {
    start_disabled();
    let l = IrqSpinLock::new(0u32);
    let _g = l.lock();
    assert!(l.try_lock().is_none());
}

#[test_case]
fn scope_end_releases() {
    start_disabled();
    let l = IrqSpinLock::new(0u32);
    {
        let _g = l.lock();
        assert!(l.try_lock().is_none());
    }
    assert!(l.try_lock().is_some());
}

#[test_case]
fn temporary_guard_releases_at_end_of_statement() {
    start_disabled();
    let l = IrqSpinLock::new(0u32);
    *l.lock() = 5;
    assert!(l.try_lock().is_some());
}

#[test_case]
fn failed_try_lock_does_not_disturb_holder() {
    start_disabled();
    let l = IrqSpinLock::new(0u32);
    let mut g = l.lock();
    for _ in 0..100 {
        assert!(l.try_lock().is_none());
    }
    *g = 9;
    drop(g);
    assert_eq!(*l.lock(), 9); // would hang if a failure took a ticket
}

// ---- Statics ----

static IRQ_COUNTER: IrqSpinLock<u32> = IrqSpinLock::new(0);

#[test_case]
fn static_lock_is_usable() {
    start_disabled();
    *IRQ_COUNTER.lock() += 1;
    let g = IRQ_COUNTER.lock();
    assert!(IRQ_COUNTER.try_lock().is_none());
    assert_eq!(*g, 1);
}

// ---- TODO: once the test entry runs IDT + PIC init and enables interrupts ----
//
// Each of these should start with `interrupts::enable()` (or rely on init
// having done it) and assert `interrupts::are_enabled()` first.
//
// 1. lock_from_enabled_disables_then_restores
//    Enabled -> lock: assert disabled and `g.interrupts_were_enabled`.
//    Drop: assert enabled again.
//
// 2. try_lock_success_from_enabled_restores
//    Same as 1, via try_lock.
//
// 3. try_lock_failure_from_enabled_restores_immediately
//    Hold the lock with interrupts off inside `without_interrupts`, re-enable,
//    then try_lock: it returns None AND interrupts are enabled again. Tests
//    the failure path's cleanup.
//
// 4. nested_from_enabled_inner_drop_keeps_disabled
//    Enabled -> outer.lock() -> inner.lock(). Drop inner: still DISABLED,
//    because inner saved `false`. Drop outer: enabled. This is the bug a
//    blind `sti` in Drop would cause.
//
// 5. timer_cannot_fire_while_held
//    Have the timer handler bump a static AtomicU64 TICKS. Hold an IrqSpinLock
//    and busy-wait long enough for several ticks (~100 ms at the default PIT
//    rate). Assert TICKS hasn't changed. Drop, then `hlt` a few times and
//    assert TICKS has increased. This is the core property: no interrupt can
//    arrive while the lock is held.
//
// 6. handler_can_take_lock_without_deadlock
//    The timer handler does `*SHARED.lock() += 1` on a static IrqSpinLock.
//    The test loops locking SHARED for a while. If the test finishes, no
//    self-deadlock happened. Assert the counter advanced. With a plain
//    SpinLock this test would hang, so it's a good regression guard.
//
// 7. exceptions_still_fire_while_held (documents a limitation)
//    With an int3 handler that bumps a counter: hold the lock, execute
//    `x86_64::instructions::interrupts::int3()`, and assert the counter
//    advanced. `cli` masks hardware interrupts, not exceptions. Exception
//    handlers must therefore never take an IrqSpinLock.
//
// 8. out_of_order_drop (documents a limitation; don't assert it "works")
//    Enabled -> a = A.lock() (saved true) -> b = B.lock() (saved false).
//    drop(a) FIRST: this restores interrupts to ENABLED while `b` is still
//    held, which reopens the deadlock window. Nothing in the types prevents
//    this. Linux has the same rule: irqsave/irqrestore must nest. Consider
//    a test asserting `are_enabled()` after drop(a), so the behavior is
//    pinned down and visible, plus a doc note on IrqSpinLock.
