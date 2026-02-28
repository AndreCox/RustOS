use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub mod font;
pub mod graphics;
pub mod renderer;
pub mod vfb;

static LAST_UPTIME_DRAW: AtomicU64 = AtomicU64::new(0);
pub static EXCLUSIVE_GRAPHICS: AtomicBool = AtomicBool::new(false);
use core::sync::atomic::AtomicU64 as AtomicCounter;
static FAILED_LOCKS: AtomicCounter = AtomicCounter::new(0);

pub fn compositor_task() -> ! {
    let mut was_exclusive = false;
    loop {
        let is_exclusive = EXCLUSIVE_GRAPHICS.load(Ordering::Relaxed);
        if let Some(mut guard) = crate::screen::renderer::WRITER.try_lock() {
            if let Some(writer) = guard.as_mut() {
                if !is_exclusive {
                    // 0. If we just exited exclusive mode, clear the screen
                    if was_exclusive {
                        writer.clear_screen();
                        was_exclusive = false;
                    }

                    // 1. Process text queue but limit to 2000 chars per frame to prevent stutter
                    for _ in 0..2000 {
                        if let Some(c) = crate::io::log_buffer::DISPLAY_QUEUE.pop_char() {
                            writer.put_char(c as char);
                        } else {
                            break;
                        }
                    }

                    // 2. Only draw UI if NOT in exclusive mode
                    let current_time = crate::timer::get_uptime_ms();
                    if current_time - LAST_UPTIME_DRAW.load(Ordering::Relaxed) >= 100 {
                        crate::screen::graphics::draw_ui(writer);
                        LAST_UPTIME_DRAW.store(current_time, Ordering::Relaxed);
                    }
                } else {
                    was_exclusive = true;
                    // Discard text buffer logs to prevent massive unrendered backlog 
                    while let Some(_) = crate::io::log_buffer::DISPLAY_QUEUE.pop_char() {}
                }

                // 3. Composite any virtual framebuffers into the hardware framebuffer.
                let vlist = vfb::snapshot_meta();
                for (ptr, width, height, owner, min_y, max_y) in vlist.iter() {
                    // Skip unowned buffers (owner==0)
                    if *owner == 0 { continue; }
                    // Sanity check sizes
                    if *width == 0 || *height == 0 || *width > 4096 || *height > 4096 {
                        continue;
                    }
                    if *min_y < *max_y {
                        unsafe {
                            let src_ptr = (*ptr) as *mut u32;
                            let fb_ptr = writer.buffer.as_mut_ptr() as *mut u32;
                            let stride = (writer.pitch / 4) as usize;

                            for y in 0..*height {
                                let src_row = src_ptr.add(y * *width);
                                let dst_row0 = fb_ptr.add((y * 2) * stride);
                                let dst_row1 = fb_ptr.add((y * 2 + 1) * stride);

                                for x in 0..*width {
                                    let px = src_row.add(x).read();
                                    dst_row0.add(x * 2).write(px);
                                    dst_row0.add(x * 2 + 1).write(px);
                                    dst_row1.add(x * 2).write(px);
                                    dst_row1.add(x * 2 + 1).write(px);
                                }
                            }
                        }
                        // Clear the vfb dirty region so we don't re-copy
                        vfb::clear_dirty(*ptr as *mut u32);
                        writer.mark_dirty(0, writer.height);
                    }
                }

                // 4. ALWAYS present so compositor-owned changes appear
                writer.present();
            }
        } else {
            let prev = FAILED_LOCKS.fetch_add(1, Ordering::Relaxed) + 1;
            if prev > 200 {
                crate::println!(
                    "[COMPOSITOR] diagnostic: WRITER locked >200 times, dumping scheduler state."
                );
                x86_64::instructions::interrupts::without_interrupts(|| {
                    let sched_guard = crate::multitasker::scheduler::SCHEDULER.lock();
                    if let Some(ref sched) = *sched_guard {
                        crate::println!("Scheduler current_task_id={}", sched.get_current_task_id());
                    }
                });
                unsafe {
                    let lock_ptr = core::ptr::addr_of!(crate::screen::renderer::WRITER) as *mut u64;
                    lock_ptr.write_volatile(0);
                }
                FAILED_LOCKS.store(0, Ordering::Relaxed);
            }
        }

        // 4. ALWAYS drain serial queue so you don't lose debug info
        while let Some(byte) = crate::io::log_buffer::SERIAL_QUEUE.pop_char() {
            crate::io::serial::serial_write_byte(byte);
        }
        crate::timer::sleep_ms(16);
    }
}

pub fn enter_exclusive_mode() {
    EXCLUSIVE_GRAPHICS.store(true, Ordering::SeqCst);
}

pub fn exit_exclusive_mode() {
    EXCLUSIVE_GRAPHICS.store(false, Ordering::SeqCst);
}

pub fn is_exclusive_mode() -> bool {
    EXCLUSIVE_GRAPHICS.load(Ordering::SeqCst)
}
