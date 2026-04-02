#![no_std]
#![no_main]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.start")]
pub extern "C" fn _start() -> ! {
    let vga = 0xb8000 as *mut u8;
    // Write directly to VGA memory using immediate values
    // to prevent the compiler from generating .rodata absolute accesses!
    unsafe {
        let base = (80 * 20) * 2;
        *vga.offset(base) = b'A';
        *vga.offset(base + 1) = 0x0a;
        *vga.offset(base + 2) = b'P';
        *vga.offset(base + 3) = 0x0a;
        *vga.offset(base + 4) = b'P';
        *vga.offset(base + 5) = 0x0a;
        *vga.offset(base + 6) = b' ';
        *vga.offset(base + 7) = 0x0a;
        *vga.offset(base + 8) = b'R';
        *vga.offset(base + 9) = 0x0a;
        *vga.offset(base + 10) = b'U';
        *vga.offset(base + 11) = 0x0a;
        *vga.offset(base + 12) = b'N';
        *vga.offset(base + 13) = 0x0a;
        *vga.offset(base + 14) = b'N';
        *vga.offset(base + 15) = 0x0a;
        *vga.offset(base + 16) = b'I';
        *vga.offset(base + 17) = 0x0a;
        *vga.offset(base + 18) = b'N';
        *vga.offset(base + 19) = 0x0a;
        *vga.offset(base + 20) = b'G';
        *vga.offset(base + 21) = 0x0a;
        *vga.offset(base + 22) = b'!';
        *vga.offset(base + 23) = 0x0a;
    }
    // Since task has no exit mechanism yet, just loop and we can kill it later if needed
    // or just let it spin
    loop {
        // We don't have yield_now here, so it's a tight loop.
        // This will block the CPU if not preemptive, wait!
        // RustOS has a cooperative scheduler (yield_now) and maybe preemptive timer?
        // Let's use the core::arch::asm!("pause");
        unsafe { core::arch::asm!("pause") }
    }
}
