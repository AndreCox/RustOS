use crate::helpers::hcf;
use crate::io::keyboard::SCANCODE_QUEUE;
use crate::{globals, println, screen, serial_println};
use core::arch::{asm, global_asm};
use core::sync::atomic::AtomicU64;

static BUSY_TICKS: AtomicU64 = AtomicU64::new(0);
static TOTAL_TICKS: AtomicU64 = AtomicU64::new(0);

// Define the Interrupt Descriptor Table (IDT) structures
#[repr(C, packed)]
#[derive(Copy, Clone)]

pub struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

// Implement methods for IdtEntry
impl IdtEntry {
    // Create a missing (zeroed) IDT entry
    pub const fn missing() -> Self {
        IdtEntry {
            offset_low: 0,
            selector: 0,
            ist: 0,
            attributes: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }
    // Set the handler address and code segment for the IDT entry
    pub fn set_handler(&mut self, handler_addr: u64, code_segment: u16) {
        self.offset_low = handler_addr as u16;
        self.offset_mid = (handler_addr >> 16) as u16;
        self.offset_high = (handler_addr >> 32) as u32;
        self.selector = code_segment; // Limine's Kernel Code Segment
        self.attributes = 0x8E; // 0x8E = Present + Ring 0 + Interrupt Gate
        self.ist = 0;
        self.reserved = 0;
    }
}

// Define the IDT pointer structure
#[repr(C, packed)]
pub struct IdtPointer {
    limit: u16,
    base: u64,
}

// Define the IDT as a static mutable array
static mut IDT: [IdtEntry; 256] = [IdtEntry::missing(); 256];

// Array of ISR stubs for the first 48 interrupts
unsafe extern "C" {
    static isr_stub_table: [extern "C" fn(); 48];
}

// Struct to represent the interrupt stack frame
#[repr(C)]
#[derive(Debug)]
pub struct InterruptStackFrame {
    // Pushed by isr_common_stub (in reverse order of push)
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,

    // Pushed by the macro
    pub interrupt_number: u64,
    pub error_code: u64,

    // Pushed by the CPU automatically
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

// Function to initialize and load the IDT
pub unsafe fn init_idt() {
    // Get the current code segment
    let cs: u16;
    unsafe {
        asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
    }

    globals::KERNEL_CODE_SEGMENT = cs;

    unsafe {
        asm!("mov {0:x}, ss", out(reg) globals::KERNEL_DATA_SEGMENT, options(nomem, nostack, preserves_flags));
    }

    println!("Current CS is: {:#x}", cs);

    // Install handlers for the first 48 ISRs
    unsafe {
        for i in 0..48 {
            IDT[i].set_handler(isr_stub_table[i] as u64, cs);
        }
    }

    // Create the pointer on the stack
    let idt_ptr = IdtPointer {
        limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };

    // Load it
    unsafe {
        asm!("lidt [{}]", in(reg) &idt_ptr, options(readonly, nostack, preserves_flags));
    }
}

// Function to initialize the PICs
pub unsafe fn init_pic() {
    const PIC1_COMMAND: u16 = 0x20;
    const PIC1_DATA: u16 = 0x21;
    const PIC2_COMMAND: u16 = 0xA0;
    const PIC2_DATA: u16 = 0xA1;

    // ICW1: Start initialization
    outb(PIC1_COMMAND, 0x11);
    outb(PIC2_COMMAND, 0x11);

    // ICW2: Set vector offsets (0..31 are exceptions, so we start at 32)
    outb(PIC1_DATA, 0x20); // Master: IRQs 0-7  -> Vectors 32-39
    outb(PIC2_DATA, 0x28); // Slave:  IRQs 8-15 -> Vectors 40-47

    // ICW3: Wiring
    outb(PIC1_DATA, 4); // Slave at IRQ2
    outb(PIC2_DATA, 2); // Slave's identity

    // ICW4: 8086 mode
    outb(PIC1_DATA, 0x01);
    outb(PIC2_DATA, 0x01);

    // Mask all interrupts except Keyboard (IRQ 1) and Timer (IRQ 0)
    outb(PIC1_DATA, 0xFC);
    outb(PIC2_DATA, 0xFF);
}

fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
    }
}

// The common exception handler
#[unsafe(no_mangle)]
pub extern "C" fn exception_handler(frame: &InterruptStackFrame) -> u64 {
    let num = frame.interrupt_number;
    let mut current_rsp = frame as *const _ as u64;

    // --- 1. HANDLE CPU EXCEPTIONS (0-31) ---
    if num < 32 {
        // do a manual write to serial port just in case we can't catch the error
        let error_string = "There was a CPU Exception!";
        for &byte in error_string.as_bytes() {
            crate::io::serial::serial_write_byte(byte);
        }
        crate::io::serial::serial_write_byte(b'\n');

        // Print the error code in hex (64-bit -> 16 hex digits)
        let ec = frame.interrupt_number;
        for &b in b"Interrupt Number: " {
            crate::io::serial::serial_write_byte(b);
        }
        for i in (0..16).rev() {
            let nibble = ((ec >> (i * 4)) & 0xF) as u8;
            let ch = if nibble < 10 {
                b'0' + nibble
            } else {
                b'a' + (nibble - 10)
            };
            crate::io::serial::serial_write_byte(ch);
        }
        crate::io::serial::serial_write_byte(b'\n');

        println!("\n[CPU EXCEPTION {}] at RIP: {:#x}", num, frame.rip);
        // print debug info
        println!(
            "RAX: {:#x} RBX: {:#x} RCX: {:#x} RDX: {:#x}",
            frame.rax, frame.rbx, frame.rcx, frame.rdx
        );
        println!(
            "RSI: {:#x} RDI: {:#x} RBP: {:#x}",
            frame.rsi, frame.rdi, frame.rbp
        );
        println!(
            "R8:  {:#x} R9:  {:#x} R10: {:#x} R11: {:#x}",
            frame.r8, frame.r9, frame.r10, frame.r11
        );
        println!(
            "R12: {:#x} R13: {:#x} R14: {:#x} R15: {:#x}",
            frame.r12, frame.r13, frame.r14, frame.r15
        );
        serial_println!("RSP: {:#x} RFLAGS: {:#x}", frame.rsp, frame.rflags);
        serial_println!("CS:  {:#x} SS:     {:#x}", frame.cs, frame.ss);

        // Step 1: Try to get the MutexGuard
        if let Some(mut guard) = crate::multitasker::scheduler::SCHEDULER.try_lock() {
            if let Some(sched) = guard.as_mut() {
                if let Some(task) = sched.current_task.as_mut() {
                    // 1. Mark dead
                    task.status = crate::multitasker::task::TaskStatus::Killed;

                    // 2. Set the flag (Safe, no locks)
                    //

                    unsafe {
                        // We access the raw Mutex data to reset it to "0" (Unlocked)
                        // This is safe because we've already marked the owner task as Killed.
                        let lock_ptr =
                            core::ptr::addr_of!(crate::screen::renderer::WRITER) as *mut u64;
                        lock_ptr.write_volatile(0);
                    }

                    // screen::exit_exclusive_mode();

                    current_rsp = sched.schedule(current_rsp);
                    return current_rsp;
                }
            }
        }

        serial_println!("KERNEL PANIC: Exception outside task context or Scheduler locked.");

        hcf(); // If we can't lock or no task exists, halt.
    }

    // --- 2. HANDLE IRQs (32+) ---
    if num == 32 {
        TOTAL_TICKS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        crate::timer::tick();

        // Use the same double-unwrap pattern here
        if let Some(mut guard) = crate::multitasker::scheduler::SCHEDULER.try_lock() {
            if let Some(sched) = guard.as_mut() {
                current_rsp = sched.schedule(current_rsp);
            }
        }
    } else if num == 33 {
        // IRQ 1: Keyboard
        let scancode: u8;
        unsafe {
            asm!("in al, dx", out("al") scancode, in("dx") 0x60 as u16);
        }
        if scancode != 0xE0 {
            let _ = SCANCODE_QUEUE.push(scancode);
        }
    }

    // --- 3. SEND EOI (End of Interrupt) ---
    // Exceptions (0-31) DO NOT need an EOI. Hardware IRQs (32+) DO.
    if num >= 32 {
        outb(0x20, 0x20); // Master PIC
        if num >= 40 {
            outb(0xA0, 0x20); // Slave PIC
        }
    }

    current_rsp
}

// Assembly code for ISR stubs and common handler
global_asm!(
    r#"
    .altmacro

    /* 1. The Macros */
    .macro isr_no_err_stub num
        .global isr_stub_\num
        isr_stub_\num:
            push 0
            push \num
            jmp isr_common_stub
    .endm

    .macro isr_err_stub num
        .global isr_stub_\num
        isr_stub_\num:
            push \num
            jmp isr_common_stub
    .endm

    /* 2. Generation Loop */
    .set i, 0
    .rept 48
        .if i == 8 || (i >= 10 && i <= 14) || i == 17 // Interrupts with error codes
            isr_err_stub %i
        .else
            isr_no_err_stub %i
        .endif
        .set i, i + 1
    .endr

    /* 3. The Common Handler */
    isr_common_stub:
        push r15; push r14; push r13; push r12
        push r11; push r10; push r9;  push r8
        push rbp; push rdi; push rsi; push rdx
        push rcx; push rbx; push rax

        mov rdi, rsp
        call exception_handler

        // On return, rax contains the new rsp
        mov rsp, rax

        // Restore registers
        pop rax; pop rbx; pop rcx; pop rdx
        pop rsi; pop rdi; pop rbp; pop r8
        pop r9;  pop r10; pop r11; pop r12
        pop r13; pop r14; pop r15

        add rsp, 16
        iretq

    /* 4. The Address Table */
    .section .data
    .align 8
    .global isr_stub_table

    .macro push_stub_addr n
        .quad isr_stub_\n
    .endm

    isr_stub_table:
        .set i, 0
        .rept 48
            push_stub_addr %i
            .set i, i + 1
        .endr
    
    .noaltmacro
    "#
);

pub fn get_cpu_usage() -> u32 {
    let total = TOTAL_TICKS.swap(0, core::sync::atomic::Ordering::Relaxed);
    let busy = BUSY_TICKS.swap(0, core::sync::atomic::Ordering::Relaxed);

    if total == 0 {
        return 0;
    }

    // Math: (Busy / Total) * 100
    ((busy as u64 * 100) / total as u64) as u32
}
