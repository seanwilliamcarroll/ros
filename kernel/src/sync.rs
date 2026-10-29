//! Synchronization primitives.

mod raw;
mod spinlock;

pub use raw::RawSpinLock;
pub use spinlock::SpinLock;
