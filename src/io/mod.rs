use alloc::fmt;

pub mod keyboard;
pub mod log_buffer;
pub mod serial;

/******************************
 * SET UP SERIAL PRINT MACROS *
 ******************************/
#[doc(hidden)]
pub fn _serial_print(args: fmt::Arguments) {
    log_buffer::_log_print(args, log_buffer::LogTarget::Serial);
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::io::_serial_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_println {
    () => {
        $crate::serial_print!("\n");
    };
    ($fmt:expr) => {
        $crate::serial_print!(concat!($fmt, "\n"));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::serial_print!(concat!($fmt, "\n"), $($arg)*);
    };
}

/******************************
 * SET UP SCREEN PRINT MACROS *
 ******************************/
#[doc(hidden)]
pub fn _screen_print(args: core::fmt::Arguments) {
    log_buffer::_log_print(args, log_buffer::LogTarget::Display);
}

#[macro_export]
macro_rules! screen_print {
    ($($arg:tt)*) => ($crate::io::_screen_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! screen_println {
    () => ($crate::screen_print!("\n"));
    ($($arg:tt)*) => ($crate::screen_print!("{}\n", format_args!($($arg)*)));
}

/******************************************************************
 * SET UP DUAL PRINT MACROS THAT PRINT TO BOTH SERIAL AND DISPLAY *
 ******************************************************************/
#[doc(hidden)]
pub fn _dual_print(args: fmt::Arguments) {
    log_buffer::_log_print(args, log_buffer::LogTarget::Both);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::io::_dual_print(format_args!($($arg)*))
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
