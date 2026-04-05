#![no_std]
#![no_main]

use rustos_user::{exit, print_str};

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.start")]
pub extern "C" fn _start() -> ! {
    print_str("HELLO FROM USER SPACE!\n");
    exit()
}
