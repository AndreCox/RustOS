use crate::println;
use core::sync::atomic::{AtomicU64, Ordering};

pub mod font;
pub mod graphics;
pub mod renderer;

static LAST_UPTIME_DRAW: AtomicU64 = AtomicU64::new(0);

pub fn compositor_task() -> ! {
    println!("Compositor task started with Shadow Buffering.");

    loop {
        // Single lock scope: We draw to RAM and flush to VRAM in one go.
        // The new `present()` is smart enough to be fast (only copies dirty rows).
        if let Some(mut guard) = crate::screen::renderer::WRITER.try_lock() {
            if let Some(writer) = guard.as_mut() {
                let mut needs_present = false;
                let mut count = 0;

                // 1. Drain the text queue (Writes to Shadow Buffer)
                while let Some(c) = crate::io::log_buffer::DISPLAY_QUEUE.pop_char() {
                    writer.put_char(c as char);
                    count += 1;
                    needs_present = true;

                    // Limit chars per frame to prevent freezing if queue is flooded
                    if count > 500 {
                        break;
                    }
                }

                // 2. Draw UI elements (Writes to Shadow Buffer)
                let current_time = crate::timer::get_uptime_ms();
                let last_draw = LAST_UPTIME_DRAW.load(Ordering::Relaxed);

                if current_time - last_draw >= 100 {
                    // Assuming draw_ui uses the standard draw functions,
                    // it will automatically trigger the dirty flags in the writer.
                    crate::screen::graphics::draw_ui(writer);
                    LAST_UPTIME_DRAW.store(current_time, Ordering::Relaxed);
                    needs_present = true;
                }

                // 3. Flush to Hardware
                // If nothing changed, this returns immediately.
                // If text was typed, it copies ~16 rows (tiny).
                // If screen scrolled, it copies the update.
                if needs_present {
                    writer.present();
                }
            }
        }

        // Drain serial queue (totally independent of screen)
        while let Some(byte) = crate::io::log_buffer::SERIAL_QUEUE.pop_char() {
            crate::io::serial::serial_write_byte(byte);
        }

        // Cap at ~60 FPS
        crate::timer::sleep_ms(16);
    }
}
