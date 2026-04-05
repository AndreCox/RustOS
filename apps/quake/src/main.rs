#![no_std]
#![no_main]

use core::panic::PanicInfo;
use rustos_user::{exit, print_str};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    print_str("[quake] panic\n");
    exit()
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    print_str("[quake] stub start\n");

    exit()
}
