use crate::{println, serial_println};

pub mod font;
pub mod graphics;
pub mod renderer;

pub fn compositor_task() -> ! {
    println!("Compositor task started.");

    loop {
        if let Some(mut guard) = crate::screen::renderer::WRITER.try_lock() {
            if let Some(writer) = guard.as_mut() {
                let mut count = 0;
                while let Some(c) = crate::io::log_buffer::DISPLAY_QUEUE.pop_char() {
                    writer.put_char(c as char);
                    count += 1;
                    // Break after 500 chars to ensure we actually swap_buffers and yield
                    if count > 500 {
                        break;
                    }
                }
                crate::screen::graphics::draw_ui(writer);
                writer.swap_buffers();
            }
        }

        // Drain the Serial Queue independently
        while let Some(byte) = crate::io::log_buffer::SERIAL_QUEUE.pop_char() {
            crate::io::serial::serial_write_byte(byte);
        }

        // 3. Sleep for a short duration to limit CPU usage
        crate::timer::sleep_ms(16); // Approx ~60 FPS
    }
}
