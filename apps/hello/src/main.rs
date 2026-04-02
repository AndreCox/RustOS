#![no_std]
#![no_main]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.start")]
pub extern "C" fn _start() -> ! {
    let msg = "HELLO FROM USER SPACE!\n";
    for &b in msg.as_bytes() {
        unsafe {
            core::arch::asm!(
                "int 0x80",
                in("rax") 1u64, // Syscall 1: print_char
                in("rdi") b as u64,
                options(nostack, preserves_flags)
            );
        }
    }

    // Call Syscall 2: exit
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") 2u64,
            options(noreturn, nostack)
        );
    }
}
