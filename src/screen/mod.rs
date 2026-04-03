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
    let mut last_blink = false;
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

                    // 1. Process text queue
                    for _ in 0..256 {
                        if let Some(c) = crate::io::log_buffer::DISPLAY_QUEUE.pop_char() {
                            writer.put_char(c as char);
                        } else {
                            break;
                        }
                    }

                    // 2. Only draw UI if NOT in exclusive mode
                    let current_time = crate::timer::get_uptime_ms();
                    if current_time - LAST_UPTIME_DRAW.load(Ordering::Relaxed) >= 100 {
                        crate::screen::graphics::draw_ui(writer.width);
                        LAST_UPTIME_DRAW.store(current_time, Ordering::Relaxed);
                    }

                    // 3. Composite virtual framebuffers
                    let vlist = vfb::snapshot_meta();
                    for (ptr, width, height, owner, min_y, max_y) in vlist.iter() {
                        if *owner == 0 {
                            continue;
                        }
                        if *width == 0 || *height == 0 || *width > 4096 || *height > 4096 {
                            continue;
                        }
                        if *min_y < *max_y {
                            unsafe {
                                let src_ptr = (*ptr) as *mut u32;
                                let fb_ptr = writer.buffer.as_mut_ptr() as *mut u32;
                                let stride = (writer.pitch / 4) as usize;
                                let hw_width = writer.width as usize;
                                if *width == hw_width {
                                    for y in 0..*height {
                                        core::ptr::copy_nonoverlapping(
                                            src_row(src_ptr, y, *width),
                                            dst_row(fb_ptr, y, stride),
                                            *width,
                                        );
                                    }
                                } else {
                                    // Scaling... (implementation details)
                                    for y in 0..*height {
                                        let src = src_ptr.add(y * *width);
                                        let dst0 = fb_ptr.add((y * 2) * stride);
                                        let dst1 = fb_ptr.add((y * 2 + 1) * stride);
                                        for x in 0..*width {
                                            let px = *src.add(x);
                                            *dst0.add(x * 2) = px;
                                            *dst0.add(x * 2 + 1) = px;
                                            *dst1.add(x * 2) = px;
                                            *dst1.add(x * 2 + 1) = px;
                                        }
                                    }
                                }
                            }
                            vfb::clear_dirty(*ptr as *mut u32);
                            writer.mark_dirty(0, writer.height);
                        }
                    }

                    // 4. Handle blinking cursor
                    let uptime = crate::timer::get_uptime_ms();
                    let blink = (uptime % 1000) < 500;
                    if blink != last_blink {
                        let char_h = writer.font.header.height as u64;
                        writer.mark_dirty(writer.cursor_y, char_h);
                        last_blink = blink;
                    }
                    writer.present_with_cursor(blink);
                } else {
                    was_exclusive = true;
                    // Discard text buffer logs to prevent massive unrendered backlog
                    while let Some(_) = crate::io::log_buffer::DISPLAY_QUEUE.pop_char() {}
                    writer.present(); // Empty present to clear needs_hw_update
                }
            }
        }
        crate::timer::sleep_ms(16);
    }
}

pub fn serial_task() -> ! {
    loop {
        // Drain serial queue
        for _ in 0..1024 {
            if let Some(byte) = crate::io::log_buffer::SERIAL_QUEUE.pop_char() {
                crate::io::serial::serial_write_byte(byte);
            } else {
                break;
            }
        }
        // Yield to other tasks
        crate::multitasker::yield_now();
    }
}

// Helper to avoid clutter
unsafe fn src_row(ptr: *const u32, y: usize, w: usize) -> *const u32 {
    ptr.add(y * w)
}
unsafe fn dst_row(ptr: *mut u32, y: usize, s: usize) -> *mut u32 {
    ptr.add(y * s)
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
