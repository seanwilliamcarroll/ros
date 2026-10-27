#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

pub mod framebuffer;
pub mod serial;

use core::panic::PanicInfo;

pub mod prelude {
    pub use crate::{print, println, serial_print, serial_println};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(code: QemuExitCode) -> ! {
    use x86_64::instructions::port::Port;
    // SAFETY: the runner always attaches isa-debug-exit at 0xf4.
    unsafe { Port::new(0xf4).write(code as u32) };
    // Only reached if the device is missing (e.g. booted outside the runner).
    loop {
        core::hint::spin_loop();
    }
}

pub trait Testable {
    fn run(&self);
}

impl<T: Fn()> Testable for T {
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {info}\n");
    exit_qemu(QemuExitCode::Failed);
}

// ---- Entry point for `cargo test --lib` (the lib's own unit tests) ----

#[cfg(test)]
bootloader_api::entry_point!(test_kernel_main);

#[cfg(test)]
fn test_kernel_main(_boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    test_main();
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
