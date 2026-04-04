use crate::helpers::hcf;
use crate::io::keyboard::SCANCODE_QUEUE;
use crate::{globals, println, screen, serial_println};
use alloc::string::String;
use alloc::vec::Vec;
use core::arch::{asm, global_asm};
use core::sync::atomic::AtomicU64;

static BUSY_TICKS: AtomicU64 = AtomicU64::new(0);
static TOTAL_TICKS: AtomicU64 = AtomicU64::new(0);
const SYSCALL_ERR: u64 = u64::MAX;
const MAX_SYSCALL_PATH: usize = 512;
const MAX_SYSCALL_RW: usize = 1024 * 1024;

unsafe fn user_cstr_to_string(ptr: u64, max_len: usize) -> Option<String> {
    if ptr == 0 || max_len == 0 {
        return None;
    }

    let mut bytes = Vec::new();
    let base = ptr as *const u8;
    for i in 0..max_len {
        let b = unsafe { base.add(i).read() };
        if b == 0 {
            break;
        }
        bytes.push(b);
    }

    if bytes.is_empty() {
        return None;
    }

    let s = core::str::from_utf8(&bytes).ok()?;
    Some(s.into())
}

unsafe fn sys_fs_read(path_ptr: u64, buf_ptr: u64, len: u64) -> u64 {
    if buf_ptr == 0 || len == 0 {
        return 0;
    }

    let read_len = core::cmp::min(len as usize, MAX_SYSCALL_RW);
    let path = match unsafe { user_cstr_to_string(path_ptr, MAX_SYSCALL_PATH) } {
        Some(p) => p,
        None => return SYSCALL_ERR,
    };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    let mut file = match fs.get_ro_file(path.as_str()) {
        Ok(f) => f,
        Err(_) => return SYSCALL_ERR,
    };

    let out = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, read_len) };
    match embedded_io::Read::read(&mut file, out) {
        Ok(n) => n as u64,
        Err(_) => SYSCALL_ERR,
    }
}

unsafe fn sys_fs_open(path_ptr: u64) -> u64 {
    let path = match user_cstr_to_string(path_ptr, MAX_SYSCALL_PATH) {
        Some(p) => p,
        None => return SYSCALL_ERR,
    };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    let file = match fs.get_ro_file(path.as_str()) {
        Ok(f) => f,
        Err(_) => return SYSCALL_ERR,
    };

    let mut open_files = crate::fs::OPEN_FILES.lock();
    for (i, slot) in open_files.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(file.props.clone());
            return i as u64;
        }
    }
    open_files.push(Some(file.props.clone()));
    (open_files.len() - 1) as u64
}

unsafe fn sys_fs_read_handle(handle: u64, buf_ptr: u64, len: u64) -> u64 {
    let mut props = {
        let open_files = crate::fs::OPEN_FILES.lock();
        match open_files.get(handle as usize) {
            Some(Some(p)) => p.clone(),
            _ => return SYSCALL_ERR,
        }
    };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    let mut file = simple_fatfs::ROFile::from_props(props, fs);
    let out = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, len as usize);
    let bytes_read = match embedded_io::Read::read(&mut file, out) {
        Ok(n) => n as u64,
        Err(_) => return SYSCALL_ERR,
    };

    // Update props in the global list
    let mut open_files = crate::fs::OPEN_FILES.lock();
    if let Some(slot) = open_files.get_mut(handle as usize) {
        *slot = Some(file.props.clone());
    }

    bytes_read
}

unsafe fn sys_fs_seek_handle(handle: u64, offset: u64, whence: u64) -> u64 {
    let mut props = {
        let open_files = crate::fs::OPEN_FILES.lock();
        match open_files.get(handle as usize) {
            Some(Some(p)) => p.clone(),
            _ => return SYSCALL_ERR,
        }
    };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    let mut file = simple_fatfs::ROFile::from_props(props, fs);
    let seek_from = match whence {
        0 => embedded_io::SeekFrom::Start(offset),
        1 => embedded_io::SeekFrom::Current(offset as i64),
        2 => embedded_io::SeekFrom::End(offset as i64),
        _ => return SYSCALL_ERR,
    };

    let new_pos = match embedded_io::Seek::seek(&mut file, seek_from) {
        Ok(n) => n,
        Err(_) => return SYSCALL_ERR,
    };

    // Update props in the global list
    let mut open_files = crate::fs::OPEN_FILES.lock();
    if let Some(slot) = open_files.get_mut(handle as usize) {
        *slot = Some(file.props.clone());
    }

    new_pos
}

unsafe fn sys_fs_close(handle: u64) {
    let mut open_files = crate::fs::OPEN_FILES.lock();
    if let Some(slot) = open_files.get_mut(handle as usize) {
        *slot = None;
    }
}

unsafe fn sys_fs_write(path_ptr: u64, buf_ptr: u64, len: u64) -> u64 {
    if buf_ptr == 0 || len == 0 {
        return 0;
    }

    let write_len = core::cmp::min(len as usize, MAX_SYSCALL_RW);
    let path = match unsafe { user_cstr_to_string(path_ptr, MAX_SYSCALL_PATH) } {
        Some(p) => p,
        None => return SYSCALL_ERR,
    };

    let input = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, write_len) };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    // If the file already exists, open it for read-write, overwrite, and
    // truncate.  This avoids a u32 underflow bug in simple_fatfs that only
    // triggers when file_size == 0 (brand-new files).
    if fs.get_ro_file(path.as_str()).is_ok() {
        crate::serial_println!("sys_fs_write: File exists, opening rw...");
        let mut file = match fs.get_rw_file(path.as_str()) {
            Ok(f) => f,
            Err(e) => {
                crate::serial_println!("sys_fs_write: Failed to get_rw_file: {:?}", e);
                return SYSCALL_ERR;
            }
        };

        crate::serial_println!("sys_fs_write: Seeking to start...");
        if embedded_io::Seek::seek(&mut file, embedded_io::SeekFrom::Start(0)).is_err() {
            crate::serial_println!("sys_fs_write: Seek failed");
            return SYSCALL_ERR;
        }

        crate::serial_println!("sys_fs_write: Writing {} bytes...", input.len());
        let n = match embedded_io::Write::write(&mut file, input) {
            Ok(n) => n,
            Err(e) => {
                crate::serial_println!("sys_fs_write: Write failed: {:?}", e);
                return SYSCALL_ERR;
            }
        };

        crate::serial_println!("sys_fs_write: Truncating...");
        if file.truncate().is_err() {
             crate::serial_println!("sys_fs_write: Truncate failed");
             // don't fail, just continue
        }
        let _ = embedded_io::Write::flush(&mut file);

        drop(file);
        crate::serial_println!("sys_fs_write: Success, wrote {}", n);
        let _ = fs.unmount();
        return n as u64;
    }

    // File doesn't exist.  We must create it, but simple_fatfs has a u32
    // underflow bug when writing small data to a file with file_size == 0.
    //
    // Workaround: delete-if-exists, create, write the real data padded to
    // cluster_size + 1 bytes (so the seek arithmetic works), close the
    // handle, then re-open and overwrite+truncate using the existing-file
    // path above (which is safe because file_size > 0).
    {
        let mut file = match fs.create_file(path.as_str()) {
            Ok(f) => f,
            Err(_) => return SYSCALL_ERR,
        };

        // Initial write: cluster_size + 1 zeros to establish file_size > cluster_size.
        // This avoids the underflow because (cluster_size+1) > cluster_size makes
        // clusters_to_allocate = 1, which is valid.
        let init_buf = alloc::vec![0u8; 4097];
        if embedded_io::Write::write(&mut file, &init_buf).is_err() {
            return SYSCALL_ERR;
        }
        let _ = embedded_io::Write::flush(&mut file);
        // file is dropped here, syncing the directory entry with file_size = 4097
    }

    // Now re-open the file (file_size > 0) and overwrite with real data.
    let mut file = match fs.get_rw_file(path.as_str()) {
        Ok(f) => f,
        Err(_) => return SYSCALL_ERR,
    };

    if embedded_io::Seek::seek(&mut file, embedded_io::SeekFrom::Start(0)).is_err() {
        return SYSCALL_ERR;
    }

    let n = match embedded_io::Write::write(&mut file, input) {
        Ok(n) => n,
        Err(_) => return SYSCALL_ERR,
    };

    let _ = file.truncate();
    let _ = embedded_io::Write::flush(&mut file);

    drop(file);
    let _ = fs.unmount();
    n as u64
}

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
    fn isr_stub_128(); // 0x80 Syscall
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
        // Manual entry for 0x80 (Syscall)
        IDT[0x80].set_handler(isr_stub_128 as u64, cs);
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

        let mut ec_hex = frame.error_code;
        for &b in b"Error Code: " {
            crate::io::serial::serial_write_byte(b);
        }
        for i in (0..16).rev() {
            let nibble = ((ec_hex >> (i * 4)) & 0xF) as u8;
            let ch = if nibble < 10 {
                b'0' + nibble
            } else {
                b'a' + (nibble - 10)
            };
            crate::io::serial::serial_write_byte(ch);
        }
        crate::io::serial::serial_write_byte(b'\n');

        let mut cr2: u64;
        unsafe {
            core::arch::asm!("mov {}, cr2", out(reg) cr2);
        }
        println!(
            "\n[CPU EXCEPTION {}] at RIP: {:#x}. CR2: {:#x}. Error Code: {:#x}",
            num, frame.rip, cr2, frame.error_code
        );
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
        println!("RSP: {:#x} RFLAGS: {:#x}", frame.rsp, frame.rflags);
        println!("CS:  {:#x} SS:     {:#x}", frame.cs, frame.ss);

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

#[unsafe(no_mangle)]
pub extern "C" fn syscall_handler(frame: &mut InterruptStackFrame) -> u64 {
    let syscall_nr = frame.rax;
    let arg1 = frame.rdi;
    let arg2 = frame.rsi;
    let arg3 = frame.rdx;

    match syscall_nr {
        1 => {
            // sys_print_char(char)
            let c = arg1 as u8;
            crate::io::log_buffer::SERIAL_QUEUE.push_char(c);
            crate::io::log_buffer::DISPLAY_QUEUE.push_char(c);
        }
        2 => {
            // sys_exit()
            let mut guard = crate::multitasker::scheduler::SCHEDULER.lock();
            if let Some(sched) = guard.as_mut() {
                if let Some(task) = sched.current_task.as_mut() {
                    task.status = crate::multitasker::task::TaskStatus::Exited;
                }
                return sched.schedule(frame as *const _ as u64);
            }
        }
        3 => {
            // sys_clear()
            let q = &crate::io::log_buffer::DISPLAY_QUEUE;
            q.push_char(0x1B);
            q.push_char(b'[');
            q.push_char(b'J');
        }
        4 => {
            // sys_set_cursor(x, y) - converts 0-indexed (x, y) to 1-indexed ANSI ESC[y+1;x+1H
            let x = (arg1 & 0xFFFF);
            let y = ((arg1 >> 16) & 0xFFFF);
            let q = &crate::io::log_buffer::DISPLAY_QUEUE;
            
            q.push_char(0x1B);
            q.push_char(b'[');
            
            // Push y+1 (line) as digits
            push_u64_digits(q, y + 1);
            q.push_char(b';');
            // Push x+1 (col) as digits
            push_u64_digits(q, x + 1);
            
            q.push_char(b'H');
        }
        5 => {
            // sys_fs_read(path_ptr, out_buf_ptr, len) -> bytes_read | u64::MAX on error
            frame.rax = unsafe { sys_fs_read(arg1, arg2, arg3) };
        }
        6 => {
            // sys_fs_write(path_ptr, in_buf_ptr, len) -> bytes_written | u64::MAX on error
            frame.rax = unsafe { sys_fs_write(arg1, arg2, arg3) };
        }
        7 => {
            // sys_get_scancode() -> returns next raw scancode byte, or 0 if none
            frame.rax = SCANCODE_QUEUE.pop().map(|s| s as u64).unwrap_or(0);
        }
        9 => {
            // sys_get_key() -> returns next translated key byte, or 0 if none
            frame.rax = SCANCODE_QUEUE
                .pop()
                .and_then(|s| crate::io::keyboard::scancode_to_byte(s))
                .map(|b| b as u64)
                .unwrap_or(0);
        }
        8 => {
            // sys_yield() -> cooperate with the scheduler
            let mut guard = crate::multitasker::scheduler::SCHEDULER.lock();
            if let Some(sched) = guard.as_mut() {
                return sched.schedule(frame as *const _ as u64);
            }
        }
        10 => {
            // sys_draw_buffer(ptr, width, height)
            let ptr = arg1 as *const u32;
            let width = (arg2 & 0xFFFFFFFF) as u32;
            let height = ((arg2 >> 32) & 0xFFFFFFFF) as u32;
            // This syscall runs in interrupt context. If another task was
            // preempted while holding WRITER, blocking on the spinlock here
            // deadlocks the whole system because the lock owner cannot run.
            // It is better to drop one frame and let the next tick retry.
            if let Some(mut writer_guard) = crate::screen::renderer::WRITER.try_lock() {
                if let Some(writer) = writer_guard.as_mut() {
                    writer.blit_buffer(ptr, width, height);
                }
            }
        }
        11 => {
            // sys_get_uptime() -> returns uptime in ms
            frame.rax = crate::timer::get_uptime_ms();
        }
        12 => {
            // sys_fs_open(path_ptr) -> handle
            frame.rax = unsafe { sys_fs_open(arg1) };
        }
        13 => {
            // sys_fs_read_handle(handle, buf_ptr, len) -> bytes_read
            frame.rax = unsafe { sys_fs_read_handle(arg1, arg2, arg3) };
        }
        14 => {
            // sys_fs_seek_handle(handle, offset, whence) -> new_pos
            frame.rax = unsafe { sys_fs_seek_handle(arg1, arg2, arg3) };
        }
        15 => {
            // sys_fs_close(handle)
            unsafe { sys_fs_close(arg1) };
        }
        _ => {
            serial_println!("Unknown syscall: {}", syscall_nr);
        }
    }

    // Return the current stack pointer so the CPU can resume
    (frame as *const InterruptStackFrame as u64)
}

fn push_u64_digits(q: &crate::io::log_buffer::LogQueue, mut n: u64) {
    if n == 0 {
        q.push_char(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while n > 0 {
        buf[i] = (n % 10) as u8 + b'0';
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        q.push_char(buf[i]);
    }
}
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

    // Special entry for 0x80 we use this to handle syscalls, linux and stuff like that uses 0x80 so I chose that
    .global isr_stub_128
    isr_stub_128:
        push 0 // no error code
        push 128 // interrupt number
        push r15; push r14; push r13; push r12
        push r11; push r10; push r9;  push r8
        push rbp; push rdi; push rsi; push rdx
        push rcx; push rbx; push rax

        mov rdi, rsp
        call syscall_handler

        mov rsp, rax
        pop rax; pop rbx; pop rcx; pop rdx
        pop rsi; pop rdi; pop rbp; pop r8
        pop r9;  pop r10; pop r11; pop r12
        pop r13; pop r14; pop r15
        add rsp, 16
        iretq

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
