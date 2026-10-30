#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use bootloader_api::{BootInfo, entry_point};
use core::panic::PanicInfo;
use kernel::prelude::*;
use kernel::{QemuExitCode, exit_qemu};
use spin::Once;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

entry_point!(main);

fn main(_boot_info: &'static mut BootInfo) -> ! {
    serial_print!("stack_overflow::stack_overflow...\t");

    kernel::gdt::init();
    init_test_idt();

    stack_overflow();

    panic!("Execution continued after a stack overflow!");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kernel::test_panic_handler(info)
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    stack_overflow();
    core::hint::black_box(0);
}

static TEST_IDT: Once<InterruptDescriptorTable> = Once::new();

fn init_test_idt() {
    TEST_IDT
        .call_once(|| {
            let mut idt = InterruptDescriptorTable::new();
            unsafe {
                idt.double_fault
                    .set_handler_fn(test_double_fault_handler)
                    .set_stack_index(kernel::gdt::DOUBLE_FAULT_IST_INDEX);
            }
            idt
        })
        .load();
}

extern "x86-interrupt" fn test_double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success)
}
