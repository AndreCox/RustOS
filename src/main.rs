#![no_std] // Indicate that we are not using the standard library
#![no_main] // Indicate that we are not using the standard main function

// Enable rust libraries
use core::arch::asm;
use core::fmt::Write;
use core::panic::PanicInfo;
use limine::framebuffer;
use limine::request::ExecutableAddressRequest;
use limine::request::HhdmRequest;
use limine::{BaseRevision, request::FramebufferRequest};

// Import modules
mod helpers;
mod interrupts;
mod keyboard;
mod renderer;
mod serial;

// Use functions and structs from modules
use crate::helpers::hcf;
use crate::interrupts::{init_idt, init_pic};

#[used]
#[unsafe(link_section = ".limine_requests")]
pub static EXECUTABLE_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
pub static BASE_REVISION: BaseRevision = BaseRevision::new();

// setup limine framebuffer request
#[used]
#[unsafe(link_section = ".limine_requests")]
pub static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

// setup limine HHDM request this is for higher half direct mapping
// it maps the physical memory to a higher half virtual address space
#[used]
#[unsafe(link_section = ".limine_requests")]
pub static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    hcf();
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

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    unsafe {
        enable_sse();
    }
    if !BASE_REVISION.is_supported() {
        println!("Limine Base Revision not supported. Halting.");
        hcf();
    }

    println!("Kernel started!");

    println!("Loading IDT and PIC...");
    unsafe {
        init_idt();
        init_pic();
        asm!("sti"); // Enable interrupts
    }
    println!("IDT and PIC loaded.");

    // Get framebuffer response
    if let Some(fb_response) = FRAMEBUFFER_REQUEST.get_response() {
        // Get the first framebuffer
        if let Some(framebuffer) = fb_response.framebuffers().next() {
            let fb_addr = framebuffer.addr();
            let fb_size = (framebuffer.pitch() * framebuffer.height()) as usize;

            let fb_slice: &'static mut [u8] =
                unsafe { core::slice::from_raw_parts_mut(fb_addr as *mut u8, fb_size) };

            let mut writer = renderer::FramebufferWriter::new(
                fb_slice,
                framebuffer.pitch(),
                framebuffer.width(),
                framebuffer.height(),
            );

            renderer::init(writer);

            screen_println!("Framebuffer found:");
            screen_println!("  Width: {}", framebuffer.width());
            screen_println!("  Height: {}", framebuffer.height());
            screen_println!("  Pitch: {}", framebuffer.pitch());
        } else {
            println!("No framebuffer found. Halting.");
            hcf();
        }
    } else {
        println!("Failed to get framebuffer response. Halting.");
        hcf();
    }

    println!("Burn!!!");
    hcf();
}
