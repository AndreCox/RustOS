use core::fmt;

use crate::{screen_print, serial_print};

pub struct DualWriter;

impl fmt::Write for DualWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        serial_print!("{}", s);
        screen_print!("{}", s);
        Ok(())
    }
}

pub fn _dual_print(args: fmt::Arguments) {
    use core::fmt::Write;
    let mut writer = DualWriter;
    writer
        .write_fmt(args)
        .expect("Failed to write to dual writer");
}
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::io::writer::_dual_print(format_args!($($arg)*));
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
