use core::arch::asm;

// Halt and catch fire function
pub fn hcf() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
