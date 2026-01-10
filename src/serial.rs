use core::fmt::{self, Write};
use spin::Mutex;

const SERIAL_PORT: u16 = 0x3F8; // COM1 port address

// This is wrapped in Mutex to allow safe concurrent access
pub static WRITER: Mutex<SerialWriter> = Mutex::new(SerialWriter);

// A simple serial writer struct
pub struct SerialWriter;
impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            serial_write_byte(byte);
        }
        Ok(())
    }
}

fn serial_write_byte(byte: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") SERIAL_PORT,
            in("al") byte,
            options(nomem, nostack, preserves_flags)
        );
    }
}

// create print macros
#[doc(hidden)] // Hide from documentation
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER
        .lock()
        .write_fmt(args)
        .expect("Failed to write to serial"); // Panic on failure
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n");
    };
    ($fmt:expr) => {
        $crate::print!(concat!($fmt, "\n"));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::print!(concat!($fmt, "\n"), $($arg)*);
    };
}
