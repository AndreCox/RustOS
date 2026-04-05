use crate::io::keyboard::SCANCODE_QUEUE;
use crate::serial_println;

use super::fs_syscalls::{
    sys_fs_close, sys_fs_mkdir, sys_fs_open, sys_fs_read, sys_fs_read_handle, sys_fs_remove,
    sys_fs_rename, sys_fs_seek_handle, sys_fs_write,
};
use super::handlers::InterruptStackFrame;

#[unsafe(no_mangle)]
pub extern "C" fn syscall_handler(frame: &mut InterruptStackFrame) -> u64 {
    let syscall_nr = frame.rax;
    let arg1 = frame.rdi;
    let arg2 = frame.rsi;
    let arg3 = frame.rdx;

    match syscall_nr {
        1 => {
            let c = arg1 as u8;
            crate::io::log_buffer::SERIAL_QUEUE.push_char(c);
            crate::io::log_buffer::DISPLAY_QUEUE.push_char(c);
        }
        2 => {
            let mut guard = crate::multitasker::scheduler::SCHEDULER.lock();
            if let Some(sched) = guard.as_mut() {
                if let Some(task) = sched.current_task.as_mut() {
                    if crate::io::keyboard::task_has_focus(task.id) {
                        crate::io::keyboard::set_focus_and_clear(
                            crate::io::keyboard::SHELL_TASK_ID,
                        );
                        crate::screen::exit_exclusive_mode();
                        crate::screen::vfb::release_owner(task.id);
                    }
                    task.status = crate::multitasker::task::TaskStatus::Exited;
                }
                return sched.schedule(frame as *const _ as u64);
            }
        }
        3 => {
            let q = &crate::io::log_buffer::DISPLAY_QUEUE;
            q.push_char(0x1B);
            q.push_char(b'[');
            q.push_char(b'J');
        }
        4 => {
            let x = arg1 & 0xFFFF;
            let y = (arg1 >> 16) & 0xFFFF;
            let q = &crate::io::log_buffer::DISPLAY_QUEUE;

            q.push_char(0x1B);
            q.push_char(b'[');
            push_u64_digits(q, y + 1);
            q.push_char(b';');
            push_u64_digits(q, x + 1);
            q.push_char(b'H');
        }
        5 => {
            frame.rax = unsafe { sys_fs_read(arg1, arg2, arg3) };
        }
        6 => {
            frame.rax = unsafe { sys_fs_write(arg1, arg2, arg3) };
        }
        7 => {
            let focused = crate::io::keyboard::focused_task();
            let current_task = crate::multitasker::scheduler::SCHEDULER
                .try_lock()
                .and_then(|guard| {
                    guard
                        .as_ref()
                        .and_then(|sched| sched.current_task.as_ref().map(|task| task.id))
                });

            frame.rax = if current_task == Some(focused) {
                SCANCODE_QUEUE.pop().map(|s| s as u64).unwrap_or(0)
            } else {
                0
            };
        }
        8 => {
            let mut guard = crate::multitasker::scheduler::SCHEDULER.lock();
            if let Some(sched) = guard.as_mut() {
                return sched.schedule(frame as *const _ as u64);
            }
        }
        9 => {
            let focused = crate::io::keyboard::focused_task();
            let current_task = crate::multitasker::scheduler::SCHEDULER
                .try_lock()
                .and_then(|guard| {
                    guard
                        .as_ref()
                        .and_then(|sched| sched.current_task.as_ref().map(|task| task.id))
                });

            frame.rax = if current_task == Some(focused) {
                SCANCODE_QUEUE
                    .pop()
                    .and_then(|s| crate::io::keyboard::scancode_to_byte(s))
                    .map(|b| b as u64)
                    .unwrap_or(0)
            } else {
                0
            };
        }
        10 => {
            let ptr = arg1 as *const u32;
            let width = (arg2 & 0xFFFFFFFF) as u32;
            let height = ((arg2 >> 32) & 0xFFFFFFFF) as u32;
            if let Some(mut writer_guard) = crate::screen::renderer::WRITER.try_lock() {
                if let Some(writer) = writer_guard.as_mut() {
                    writer.blit_buffer(ptr, width, height);
                }
            }
        }
        11 => {
            frame.rax = crate::timer::get_uptime_ms();
        }
        12 => {
            frame.rax = unsafe { sys_fs_open(arg1) };
        }
        13 => {
            frame.rax = unsafe { sys_fs_read_handle(arg1, arg2, arg3) };
        }
        14 => {
            frame.rax = unsafe { sys_fs_seek_handle(arg1, arg2, arg3) };
        }
        15 => {
            unsafe { sys_fs_close(arg1) };
        }
        16 => {
            crate::screen::enter_exclusive_mode();
        }
        17 => {
            crate::screen::exit_exclusive_mode();
        }
        18 => {
            frame.rax = unsafe { sys_fs_mkdir(arg1) };
        }
        19 => {
            frame.rax = unsafe { sys_fs_remove(arg1) };
        }
        20 => {
            frame.rax = unsafe { sys_fs_rename(arg1, arg2) };
        }
        _ => {
            serial_println!("Unknown syscall: {}", syscall_nr);
        }
    }

    frame as *const InterruptStackFrame as u64
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
