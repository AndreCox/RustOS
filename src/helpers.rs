use core::arch::asm;

// Halt and catch fire function
pub fn hcf() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

pub unsafe fn enable_sse() {
    use core::arch::asm;

    let mut cr0: u64;
    asm!("mov {}, cr0", out(reg) cr0);
    cr0 &= !(1 << 2); // Clear EM bit (no emulation)
    cr0 |= 1 << 1; // Set MP bit (monitor co-processor)
    asm!("mov cr0, {}", in(reg) cr0);

    let mut cr4: u64;
    asm!("mov {}, cr4", out(reg) cr4);
    cr4 |= 1 << 9; // Set OSFXSR bit (FXSAVE/FXRSTOR support)
    cr4 |= 1 << 10; // Set OSXMMEXCPT bit (SIMD exception support)
    asm!("mov cr4, {}", in(reg) cr4);
}
