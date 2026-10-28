use core::sync::atomic::{AtomicU32, Ordering};

// Purely a lock, does not own anything
pub struct RawSpinLock {
    next: AtomicU32,
    serving: AtomicU32,
}

impl RawSpinLock {
    pub const fn new() -> Self {
        Self {
            next: AtomicU32::new(0),
            serving: AtomicU32::new(0),
        }
    }

    pub fn lock(&self) {
        let my_ticket = self.next.fetch_add(1, Ordering::Relaxed);
        while my_ticket != self.serving.load(Ordering::Acquire) {
            // Spin
            core::hint::spin_loop();
        }
        // We've acquired the lock
    }

    pub fn try_lock(&self) -> bool {
        let currently_serving = self.serving.load(Ordering::Acquire);
        // Even if currently_serving were stale, it is guarenteed to be
        // less than or equal to next
        // So a stale value at most means we incorrectly do not acquire the lock,
        // never that we acquire the lock while someone else does as well
        self.next
            .compare_exchange(
                currently_serving,
                currently_serving.wrapping_add(1),
                Ordering::Relaxed,
                Ordering::Relaxed,
            )
            .is_ok()
    }

    /// # Safety
    /// The caller must currently hold the lock to call this
    pub unsafe fn unlock(&self) {
        let serving = self.serving.load(Ordering::Relaxed);
        self.serving
            .store(serving.wrapping_add(1), Ordering::Release);
    }
}

#[cfg(test)]
mod tests;
