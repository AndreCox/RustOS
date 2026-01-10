#![no_std] // Indicate that we are not using the standard library
#![no_main] // Indicate that we are not using the standard main function

use core::panic::PanicInfo;
use limine::request::ExecutableAddressRequest;
use limine::request::HhdmRequest;
use limine::{BaseRevision, request::FramebufferRequest};

mod interrupts;

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

// Halt and catch fire function
fn hcf() -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    hcf();
}

const SERIAL_PORT: u16 = 0x3F8;

fn serial_write_byte(byte: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") SERIAL_PORT,
            in("al") byte,
            options(nomem, nostack, preserves_flags)
        );
    }
}

fn serial_write_str(s: &str) {
    for byte in s.bytes() {
        serial_write_byte(byte);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    serial_write_str("Kernel started!\n");

    if !BASE_REVISION.is_supported() {
        hcf();
    }

    if let Some(response) = FRAMEBUFFER_REQUEST.get_response() {
        serial_write_str("Framebuffer response received.\n");
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

    serial_write_str("Burn!!!");
    hcf();
}
