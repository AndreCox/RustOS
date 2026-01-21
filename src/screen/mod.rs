use crate::println;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub mod font;
pub mod graphics;
pub mod renderer;

static LAST_UPTIME_DRAW: AtomicU64 = AtomicU64::new(0);
pub static EXCLUSIVE_GRAPHICS: AtomicBool = AtomicBool::new(false);

pub fn compositor_task() -> ! {
    loop {
        let is_exclusive = EXCLUSIVE_GRAPHICS.load(Ordering::Relaxed);

        if let Some(mut guard) = crate::screen::renderer::WRITER.try_lock() {
            if let Some(writer) = guard.as_mut() {
                if !is_exclusive {
                    // 1. Only process text queue if NOT in exclusive mode
                    while let Some(c) = crate::io::log_buffer::DISPLAY_QUEUE.pop_char() {
                        writer.put_char(c as char);
                    }

                    // 2. Only draw UI if NOT in exclusive mode
                    let current_time = crate::timer::get_uptime_ms();
                    if current_time - LAST_UPTIME_DRAW.load(Ordering::Relaxed) >= 100 {
                        crate::screen::graphics::draw_ui(writer);
                        LAST_UPTIME_DRAW.store(current_time, Ordering::Relaxed);
                    }
                }

                // 3. ALWAYS present (This allows DOOM's mark_dirty to actually show up)
                writer.present();
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
