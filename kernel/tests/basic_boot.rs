#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use bootloader_api::{BootInfo, entry_point};
use core::panic::PanicInfo;
use kernel::prelude::*;

entry_point!(main);

fn main(_boot_info: &'static mut BootInfo) -> ! {
    test_main();
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kernel::test_panic_handler(info)
}

#[test_case]
fn serial_works_without_init() {
    kernel::serial_println!("serial output before any init");
}

#[test_case]
fn println_before_framebuffer_init_is_a_noop() {
    // WRITER is still None here; this must not panic or hang.
    kernel::println!("dropped on the floor");
}
