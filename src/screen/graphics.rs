use alloc::format;

use crate::screen::renderer::FramebufferWriter;
use crate::timer;

pub fn draw_ui(w: &mut FramebufferWriter) {
    // 1. Draw the Title Bar
    draw_title_bar(w);
}

fn draw_title_bar(w: &mut FramebufferWriter) {
    let bar_height = 28;
    let bg_color = 0x1E1E2E; // Modern "Catppuccin" dark theme
    let accent_color = 0x89B4FA; // Soft Blue

    // Draw background and bottom border
    w.draw_rect(0, 0, w.width, bar_height, bg_color);
    w.draw_rect(0, bar_height - 1, w.width, 1, accent_color);

    // Left side: System Info
    w.draw_string_at("Rust", 12, 6, 0xFF0000); // "Rust" in red
    let rust_width = ("Rust".len() as u64) * (w.font.header.width as u64);
    w.draw_string_at("OS", 12 + rust_width, 6, 0xFFFFFF); // " RET OS" in white

    // Right side: Live Stats
    let uptime = timer::get_uptime_ms() / 1000;
    let uptime_str = format!("UPTIME: {}s", uptime);
    let uptime_width = (uptime_str.len() as u64) * (w.font.header.width as u64);
    w.draw_string_at(
        &format!("UPTIME: {}s", uptime),
        w.width - uptime_width - 12,
        6,
        0xF38BA8,
    );

    // get heap usage
    let heap_usage = crate::memory::get_heap_usage();
    let heap_size = crate::memory::get_heap_size();

    let heap_str = format!("HEAP: {}/{} KB", heap_usage / 1024, heap_size / 1024);

    let heap_width = (heap_str.len() as u64) * (w.font.header.width as u64);
    w.draw_string_at(&heap_str, w.width - heap_width - 12, 18, 0xA6E3A1);
}
