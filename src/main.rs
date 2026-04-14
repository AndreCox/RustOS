#![no_std] // Indicate that we are not using the standard library
#![no_main] // Indicate that we are not using the standard main function
#![feature(alloc_error_handler)]
#![feature(c_variadic)]
#![allow(warnings)]

extern crate alloc; // Import the alloc crate for heap allocations

// Enable rust libraries
use core::alloc::Layout;
use core::arch::asm;
use core::panic::PanicInfo;
use limine::request::ExecutableAddressRequest;
use limine::request::HhdmRequest;
use limine::{BaseRevision, request::FramebufferRequest};

// Import modules

mod fs; // Filesystem handling (FAT32)
mod globals; // Global variables and constants
mod helpers; // Helper function
mod interrupts; // GDT and IDT setup
mod io; // Input/Output handling (keyboard, mouse, etc.)
mod memory; // Memory management (paging, heap, etc.)
mod multitasker; // Multitasking and scheduler
pub mod program_loader; // Program loading functionality
mod screen; // Screen rendering and framebuffer management
pub mod shell; // Simple Shell
mod timer; // Timer and sleep functions

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
    // 1. Force a newline and print a clear marker to serial
    crate::io::serial::serial_write_str("\n\n!!!!! KERNEL PANIC !!!!!\n");

    // 2. Try to print the panic message to serial directly
    // Since PanicInfo doesn't easily convert to &str without formatting,
    // we use a simple loop or just print that a panic occurred.
    // For more detail, we can try to use core::fmt to write to serial.
    struct SerialWriter;
    impl core::fmt::Write for SerialWriter {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            crate::io::serial::serial_write_str(s);
            Ok(())
        }
    }
    let mut sw = SerialWriter;
    let _ = core::fmt::write(&mut sw, format_args!("{}", _info));
    crate::io::serial::serial_write_str("\n!!!!!!!!!!!!!!!!!!!!!!!!\n");

    // 3. Fallback to normal println for screen (might not work if compositor hung)
    println!("KERNEL PANIC: {}", _info);

    // 4. Disable interrupts and halt
    unsafe {
        asm!("cli");
    }
    hcf();
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!(
        "Allocation error: size {} bytes, alignment {} bytes",
        layout.size(),
        layout.align()
    );
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

    println!("Initializing PS/2 mouse...");
    crate::io::mouse::init_ps2_mouse();
    println!("PS/2 mouse initialized.");

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

    println!("Initializing Filesystem...");
    fs::init_fs();
    println!("Filesystem initialized.");

    println!("Setting up Multitasking");
    multitasker::init_multitasking();
    println!("Multitasking setup complete.");

    let idle_task = crate::multitasker::task::Task::new(
        0,
        crate::multitasker::idle_task as *const () as u64,
        0,
        None,
    );
    let _compositor_task = crate::multitasker::task::Task::new(
        3,
        crate::screen::compositor_task as *const () as u64,
        0,
        None,
    );
    let task_a: multitasker::task::Task =
        crate::multitasker::task::Task::new(5, task_a as *const () as u64, 0, None);
    let task_shell = crate::multitasker::task::Task::new(
        6,
        crate::shell::task_shell as *const () as u64,
        0,
        None,
    );
    let task_serial = crate::multitasker::task::Task::new(
        7,
        crate::screen::serial_task as *const () as u64,
        0,
        None,
    );

    let mut sched = crate::multitasker::scheduler::SCHEDULER.lock();
    if let Some(ref mut scheduler) = *sched {
        scheduler.add_task(idle_task);
        scheduler.add_task(_compositor_task);
        scheduler.add_task(task_serial);
        // scheduler.add_task(task_a);
        scheduler.add_task(task_shell);
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
        crate::multitasker::yield_now();
    }
}

fn task_a() -> ! {
    loop {
        timer::sleep_ms(1000);
        println!(
            "Current Heap Usage: {} KB",
            crate::memory::get_heap_usage() / 1024
        );
    }
}

fn task_b() -> ! {
    // do an illegal instruction to test task killing
    timer::sleep_ms(2000);
    println!("Task B: About to execute illegal instruction...");
    unsafe {
        asm!("ud2");
    }
    loop {}
}

fn task_ls() -> ! {
    timer::sleep_ms(2000);
    loop {
        crate::println!("--- File System Root Directory ---");

        {
            let fs_lock = fs::FILESYSTEM.lock();

            if let Some(fs) = fs_lock.as_ref() {
                // 1. Unwrap the Result from read_dir to get the iterator
                if let Ok(dir_iter) = fs.read_dir("/") {
                    for entry_result in dir_iter {
                        // 2. Unwrap the Result from the iterator to get the DirEntry
                        if let Ok(entry) = entry_result {
                            let path = entry.path();
                            if entry.is_dir() {
                                crate::println!("[DIR]  {}", path);
                            } else if entry.is_file() {
                                // Note: use .file_size() based on the example
                                let size = entry.file_size();
                                crate::println!("[FILE] {} ({} bytes)", path, size);
                            }
                        }
                    }
                } else {
                    crate::println!("LS Task: Could not open root directory.");
                }
            } else {
                crate::println!("LS Task: Filesystem not initialized!");
            }
        }

        crate::println!("--- End of Directory ---");

        timer::sleep_ms(1000);
    }
}
