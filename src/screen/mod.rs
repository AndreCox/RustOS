use crate::{println, serial_println};
use core::sync::atomic::{AtomicU64, Ordering};

pub mod font;
pub mod graphics;
pub mod renderer;

static LAST_UPTIME_DRAW: AtomicU64 = AtomicU64::new(0);

pub fn compositor_task() -> ! {
    println!("Compositor task started with double buffering.");

    loop {
        let mut did_work = false;
        let mut needs_present = false;

        // Phase 1: Draw to backbuffer
        if let Some(mut guard) = crate::screen::renderer::WRITER.try_lock() {
            if let Some(writer) = guard.as_mut() {
                let mut count = 0;
                let mut drew_chars = false;

                while let Some(c) = crate::io::log_buffer::DISPLAY_QUEUE.pop_char() {
                    writer.put_char(c as char);
                    count += 1;
                    drew_chars = true;
                    did_work = true;
                    if count > 500 {
                        break;
                    }
                }

                let current_time = crate::timer::get_uptime_ms();
                let last_draw = LAST_UPTIME_DRAW.load(Ordering::Relaxed);
                if current_time - last_draw >= 100 {
                    crate::screen::graphics::draw_ui(writer);
                    LAST_UPTIME_DRAW.store(current_time, Ordering::Relaxed);
                    drew_chars = true;
                    did_work = true;
                }

                // Fast page flip between our buffers
                if drew_chars {
                    writer.swap_buffers();
                    needs_present = true;
                }
            }
        }

        // Phase 2: Copy to hardware framebuffer (if needed)
        // Do this in a separate lock to reduce contention
        if needs_present {
            if let Some(mut guard) = crate::screen::renderer::WRITER.try_lock() {
                if let Some(writer) = guard.as_mut() {
                    // Use chunked present to allow interrupts
                    writer.present_chunked();
                }
            }
        }

        // Drain serial queue
        while let Some(byte) = crate::io::log_buffer::SERIAL_QUEUE.pop_char() {
            crate::io::serial::serial_write_byte(byte);
            did_work = true;
        }

        if !did_work {
            crate::timer::sleep_ms(16); // ~60 FPS max
        } else {
            crate::multitasker::yield_now();
        }
    }
}
