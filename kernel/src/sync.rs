//! Synchronization primitives.

mod irq_spinlock;
mod raw;
mod spinlock;

pub use irq_spinlock::IrqSpinLock;
pub use raw::RawSpinLock;
pub use spinlock::SpinLock;
