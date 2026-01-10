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

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    println!("Kernel started!");

    println!("Loading IDT and PIC...");
    unsafe {
        init_idt();
        init_pic();
        asm!("sti"); // Enable interrupts
    }
    println!("IDT and PIC loaded.");

    if !BASE_REVISION.is_supported() {
        println!("Limine Base Revision not supported. Halting.");
        hcf();
    }

    if let Some(response) = FRAMEBUFFER_REQUEST.get_response() {
        println!("Framebuffer response received.");
        // .framebuffers() is now an Iterator. Use .next() to get the first one.
        if let Some(framebuffer) = response.framebuffers().next() {
            for i in 0..150 {
                unsafe {
                    // Limine's addr() returns a *mut u8
                    let fb_ptr = framebuffer.addr() as *mut u32;
                    let pitch = framebuffer.pitch() as usize;

                    // index = y * (pitch / 4) + x
                    // Since we're doing a diagonal line, x and y are both 'i'
                    let index = i * (pitch / 4) + i;

                    fb_ptr.add(index).write_volatile(0xFF0000);
                }
            }
        }
    }

    println!("Burn!!!");
    hcf();
}
