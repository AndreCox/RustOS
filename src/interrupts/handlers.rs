use crate::helpers::hcf;
use crate::serial_println;
use core::arch::asm;

#[repr(C)]
#[derive(Debug)]
pub struct InterruptStackFrame {
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

    pub interrupt_number: u64,
    pub error_code: u64,

    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[unsafe(no_mangle)]
pub extern "C" fn exception_handler(frame: &InterruptStackFrame) -> u64 {
    let num = frame.interrupt_number;
    let mut current_rsp = frame as *const _ as u64;

    if num < 32 {
        let error_string = "There was a CPU Exception!";
        for &byte in error_string.as_bytes() {
            crate::io::serial::serial_write_byte(byte);
        }
        crate::io::serial::serial_write_byte(b'\n');

        for &b in b"Error Code: " {
            crate::io::serial::serial_write_byte(b);
        }
        for i in (0..16).rev() {
            let nibble = ((frame.error_code >> (i * 4)) & 0xF) as u8;
            let ch = if nibble < 10 {
                b'0' + nibble
            } else {
                b'a' + (nibble - 10)
            };
            crate::io::serial::serial_write_byte(ch);
        }
        crate::io::serial::serial_write_byte(b'\n');

        let cr2: u64;
        unsafe {
            core::arch::asm!("mov {}, cr2", out(reg) cr2);
        }
        crate::println!(
            "\n[CPU EXCEPTION {}] at RIP: {:#x}. CR2: {:#x}. Error Code: {:#x}",
            num,
            frame.rip,
            cr2,
            frame.error_code
        );
        crate::println!(
            "RAX: {:#x} RBX: {:#x} RCX: {:#x} RDX: {:#x}",
            frame.rax,
            frame.rbx,
            frame.rcx,
            frame.rdx
        );
        crate::println!(
            "RSI: {:#x} RDI: {:#x} RBP: {:#x}",
            frame.rsi,
            frame.rdi,
            frame.rbp
        );
        crate::println!(
            "R8:  {:#x} R9:  {:#x} R10: {:#x} R11: {:#x}",
            frame.r8,
            frame.r9,
            frame.r10,
            frame.r11
        );
        crate::println!(
            "R12: {:#x} R13: {:#x} R14: {:#x} R15: {:#x}",
            frame.r12,
            frame.r13,
            frame.r14,
            frame.r15
        );
        crate::println!("RSP: {:#x} RFLAGS: {:#x}", frame.rsp, frame.rflags);
        crate::println!("CS:  {:#x} SS:     {:#x}", frame.cs, frame.ss);

        if let Some(mut guard) = crate::multitasker::scheduler::SCHEDULER.try_lock() {
            if let Some(sched) = guard.as_mut() {
                if let Some(task) = sched.current_task.as_mut() {
                    if crate::io::keyboard::task_has_focus(task.id) {
                        crate::io::keyboard::set_focus_and_clear(
                            crate::io::keyboard::SHELL_TASK_ID,
                        );
                        crate::screen::exit_exclusive_mode();
                        crate::screen::vfb::release_owner(task.id);
                    }

                    task.status = crate::multitasker::task::TaskStatus::Killed;

                    unsafe {
                        let lock_ptr =
                            core::ptr::addr_of!(crate::screen::renderer::WRITER) as *mut u64;
                        lock_ptr.write_volatile(0);
                    }

                    current_rsp = sched.schedule(current_rsp);
                    return current_rsp;
                }
            }
        }

        serial_println!("KERNEL PANIC: Exception outside task context or Scheduler locked.");
        hcf();
    }

    if num == 32 {
        super::on_timer_tick();
        crate::timer::tick();

        if let Some(mut guard) = crate::multitasker::scheduler::SCHEDULER.try_lock() {
            if let Some(sched) = guard.as_mut() {
                current_rsp = sched.schedule(current_rsp);
            }
        }
    } else if num == 33 {
        let scancode: u8;
        unsafe {
            asm!("in al, dx", out("al") scancode, in("dx") 0x60 as u16);
        }
        crate::io::keyboard::push_scancode(scancode);
    }

    if num >= 32 {
        super::idt::send_eoi(num);
    }

    current_rsp
}
