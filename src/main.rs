#![no_std] // Indicate that we are not using the standard library
#![no_main] // Indicate that we are not using the standard main function

extern crate alloc; // Import the alloc crate for heap allocations

// Enable rust libraries
use core::arch::asm;
use core::panic::PanicInfo;
use limine::request::ExecutableAddressRequest;
use limine::request::HhdmRequest;
use limine::{BaseRevision, request::FramebufferRequest};

// Import modules
mod globals;
mod helpers;
mod interrupts;
mod io;
mod memory;
mod multitasker;
mod screen;
mod timer;

// Use functions and structs from modules
use crate::helpers::{enable_sse, hcf};
use crate::interrupts::{init_idt, init_pic};
use crate::io::keyboard::{SCANCODE_QUEUE, scancode_to_char};

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
    println!("KERNEL PANIC: {}", _info);
    // disable interrupts
    unsafe {
        asm!("cli");
    }
    hcf();
}

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    // Enable SSE support for floating point operations
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
        init_idt(); // start the IDT
        init_pic();
    }
    println!("IDT and PIC loaded.");

    println!("Initializing Memory...");
    memory::init();
    println!("Memory initialized.");

    println!("Setting up Framebuffer...");
    let writer: screen::renderer::FramebufferWriter; // Declare framebuffer writer

    // Get framebuffer response
    if let Some(fb_response) = FRAMEBUFFER_REQUEST.get_response() {
        // Get the first framebuffer
        if let Some(framebuffer) = fb_response.framebuffers().next() {
            let fb_addr = framebuffer.addr();
            let fb_size = (framebuffer.pitch() * framebuffer.height()) as usize;

            let hardware_fb: &'static mut [u8] =
                unsafe { core::slice::from_raw_parts_mut(fb_addr as *mut u8, fb_size) };

            use alloc::vec::Vec;

            println!("Allocating buffer 0...");
            let mut buffer_0_vec = Vec::with_capacity(fb_size);
            buffer_0_vec.resize(fb_size, 0u8);
            let buffer_0: &'static mut [u8] = buffer_0_vec.leak();
            println!("Buffer 0 at: {:#x}", buffer_0.as_ptr() as u64);

            println!("Allocating buffer 1...");
            let mut buffer_1_vec = Vec::with_capacity(fb_size);
            buffer_1_vec.resize(fb_size, 0u8);
            let buffer_1: &'static mut [u8] = buffer_1_vec.leak();
            println!("Buffer 1 at: {:#x}", buffer_1.as_ptr() as u64);

            writer = screen::renderer::FramebufferWriter::new(
                hardware_fb,
                buffer_0,
                buffer_1,
                framebuffer.pitch(),
                framebuffer.width(),
                framebuffer.height(),
            );

            screen::renderer::init(writer);

            println!("Framebuffer found:");
            println!("  Width: {}", framebuffer.width());
            println!("  Height: {}", framebuffer.height());
            println!("  Pitch: {}", framebuffer.pitch());
        } else {
            println!("No framebuffer found. Halting.");
            hcf();
        }
    } else {
        println!("Failed to get framebuffer response. Halting.");
        hcf();
    }
    println!("Framebuffer setup complete.");

    println!("Setting up Multitasking");
    multitasker::init_multitasking();
    println!("Multitasking setup complete.");

    let idle_task = crate::multitasker::task::Task::new(0, crate::multitasker::idle_task as u64);
    let t1 = crate::multitasker::task::Task::new(1, task_a as u64);
    let t2 = crate::multitasker::task::Task::new(2, task_b as u64);
    let _compositor_task =
        crate::multitasker::task::Task::new(3, crate::screen::compositor_task as u64);

    let mut sched = crate::multitasker::scheduler::SCHEDULER.lock();
    if let Some(ref mut scheduler) = *sched {
        scheduler.add_task(idle_task);
        scheduler.add_task(_compositor_task);
        // scheduler.add_task(t1);
    }
    drop(sched);

    println!("Setting up Timer...");
    timer::init_timer();
    println!("Timer setup complete.");

    // Now safe to enable interrupts
    println!("Enabling interrupts...");
    unsafe {
        asm!("sti"); // Enable interrupts
    }
    loop {
        while let Some(scancode) = SCANCODE_QUEUE.pop() {
            if let Some(character) = scancode_to_char(scancode) {
                print!("{}", character);
            }
        }

        crate::multitasker::yield_now();
    }
}

fn task_a() -> ! {
    loop {
        let cpu_percent = interrupts::get_cpu_usage();
        println!("CPU Usage: {}%", cpu_percent);
        // timer::sleep_ms(16);
    }
}

fn task_b() -> ! {
    loop {
        println!("Task B is running.");
        timer::sleep_ms(1000);
    }
}
