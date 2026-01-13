use crate::serial_println;

pub mod font;
pub mod graphics;
pub mod renderer;

use super::screen::renderer::swap_buffers;

pub fn compositor_task() -> ! {
    serial_println!("Compositor task started.");

    loop {
        // 1. Draw the global UI elements to the back buffer
        crate::screen::graphics::draw_ui();

        // 2. Flush the back buffer to the screen
        swap_buffers();

        // 3. Sleep for a short duration to limit CPU usage
        crate::timer::sleep_ms(16); // Approx ~60 FPS
    }
}
