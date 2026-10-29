use crate::sync::RawSpinLock;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

pub struct SpinLock<T> {
    raw: RawSpinLock,
    data: UnsafeCell<T>,
}

#[must_use = "dropping the guard immediately unlocks the SpinLock"]
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
    _marker: PhantomData<&'a mut T>,
}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            raw: RawSpinLock::new(),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        self.raw.lock();
        SpinLockGuard::new(self)
    }

    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        self.raw.try_lock().then(|| SpinLockGuard::new(self))
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<'a, T> SpinLockGuard<'a, T> {
    // Must hold the raw lock
    const fn new(lock: &'a SpinLock<T>) -> Self {
        Self {
            lock,
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        // Safety: A guard only exists while the lock is held
        unsafe { self.lock.raw.unlock() }
    }
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safety: The guard's existence means we own the lock,
        // so no one else can have a ref to data
        // The result borrows self, so it can't outlive the guard
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: Same as deref, plus &mut self means only active borrow of the guard
        unsafe { &mut *self.lock.data.get() }
    }
}

unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> !Send for SpinLockGuard<'_, T> {}

#[cfg(test)]
mod tests;
