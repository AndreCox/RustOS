#![no_std] // Indicate that we are not using the standard library
#![no_main] // Indicate that we are not using the standard main function

// Enable rust libraries
use core::arch::asm;
use core::panic::PanicInfo;
use limine::request::ExecutableAddressRequest;
use limine::request::HhdmRequest;
use limine::{BaseRevision, request::FramebufferRequest};

// Import modules
mod helpers;
mod interrupts;
mod keyboard;
mod memory;
mod renderer;
mod serial;

// Use functions and structs from modules
use crate::helpers::{enable_sse, hcf};
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

    let mut writer: renderer::FramebufferWriter; // Declare framebuffer writer

    // Get framebuffer response
    if let Some(fb_response) = FRAMEBUFFER_REQUEST.get_response() {
        // Get the first framebuffer
        if let Some(framebuffer) = fb_response.framebuffers().next() {
            let fb_addr = framebuffer.addr();
            let fb_size = (framebuffer.pitch() * framebuffer.height()) as usize;

            let fb_slice: &'static mut [u8] =
                unsafe { core::slice::from_raw_parts_mut(fb_addr as *mut u8, fb_size) };

            writer = renderer::FramebufferWriter::new(
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

    memory::init();

    screen_println!("Starting OOM Test...");

    let mut count = 0;
    while let Some(address) = memory::allocate_frame() {
        count += 1;

        // We don't want to print 130,000 lines (it would be too slow)
        // So let's only print every 1000th page found.
        if count % 1000 == 0 {
            screen_println!("Allocated 1000 pages... Latest: {:#x}", address);
            println!("Allocated 1000 pages... Latest: {:#x}", address);
        }
    }

    println!("OUT OF MEMORY!");
    println!("Total pages allocated: {}", count);

    screen_println!("OUT OF MEMORY!");
    screen_println!("Total pages allocated: {}", count);

    // Calculate how much RAM that actually was
    let megabytes = (count * 4096) / (1024 * 1024);
    screen_println!("Total usable RAM found: {} MiB", megabytes);

    println!("Total usable RAM found: {} MiB", megabytes);

    println!("Burn!!!");
    hcf();
}
