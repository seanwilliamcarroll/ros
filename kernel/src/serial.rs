use crate::sync::IrqSpinLock;
use core::fmt::{self, Write};
use uart_16550::SerialPort;

static SERIAL1: IrqSpinLock<Option<SerialPort>> = IrqSpinLock::new(None);

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    let mut guard = SERIAL1.lock();
    let port = guard.get_or_insert_with(|| {
        // SAFETY: 0x3F8 is COM1 and nothing else in the kernel touches it.
        let mut port = unsafe { SerialPort::new(0x3F8) };
        port.init();
        port
    });
    port.write_fmt(args).expect("printing to serial failed");
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ($crate::serial::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}
