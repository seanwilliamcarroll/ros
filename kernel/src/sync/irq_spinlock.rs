use crate::sync::RawSpinLock;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{Ordering, compiler_fence};

pub struct IrqSpinLock<T> {
    raw: RawSpinLock,
    data: UnsafeCell<T>,
}

#[must_use = "dropping the guard immediately unlocks the IrqSpinLock"]
pub struct IrqSpinLockGuard<'a, T> {
    lock: &'a IrqSpinLock<T>,
    interrupts_were_enabled: bool,
    _marker: PhantomData<&'a mut T>,
}

impl<T> IrqSpinLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            raw: RawSpinLock::new(),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> IrqSpinLockGuard<'_, T> {
        let interrupts_were_enabled = x86_64::instructions::interrupts::are_enabled();
        x86_64::instructions::interrupts::disable();
        compiler_fence(Ordering::SeqCst);
        // Acquire only after we've disabled the interrupts, so no one can interrupt
        self.raw.lock();
        IrqSpinLockGuard::new(self, interrupts_were_enabled)
    }

    pub fn try_lock(&self) -> Option<IrqSpinLockGuard<'_, T>> {
        let interrupts_were_enabled = x86_64::instructions::interrupts::are_enabled();
        x86_64::instructions::interrupts::disable();
        compiler_fence(Ordering::SeqCst);
        if self.raw.try_lock() {
            return Some(IrqSpinLockGuard::new(self, interrupts_were_enabled));
        } else if interrupts_were_enabled {
            // Failed to acquire the lock and we need to reenable
            x86_64::instructions::interrupts::enable();
        }
        None
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<'a, T> IrqSpinLockGuard<'a, T> {
    // Must hold the raw lock
    const fn new(lock: &'a IrqSpinLock<T>, interrupts_were_enabled: bool) -> Self {
        Self {
            lock,
            interrupts_were_enabled,
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for IrqSpinLockGuard<'_, T> {
    fn drop(&mut self) {
        // Safety: A guard only exists while the lock is held
        unsafe { self.lock.raw.unlock() }
        compiler_fence(Ordering::SeqCst);
        if self.interrupts_were_enabled {
            x86_64::instructions::interrupts::enable();
        }
    }
}

impl<T> Deref for IrqSpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safety: The guard's existence means we own the lock,
        // so no one else can have a ref to data
        // The result borrows self, so it can't outlive the guard
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for IrqSpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: Same as deref, plus &mut self means only active borrow of the guard
        unsafe { &mut *self.lock.data.get() }
    }
}

unsafe impl<T: Send> Sync for IrqSpinLock<T> {}

impl<T> !Send for IrqSpinLockGuard<'_, T> {}

#[cfg(test)]
mod tests;
