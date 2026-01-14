use alloc::format;

use crate::screen::renderer::FramebufferWriter;
use crate::timer;

pub fn draw_ui(w: &mut FramebufferWriter) {
    // 1. Draw the Title Bar
    draw_title_bar(w);
}

fn draw_title_bar(w: &mut FramebufferWriter) {
    let bar_height = 32;
    let bg_color = 0x1E1E2E; // Modern "Catppuccin" dark theme
    let accent_color = 0x89B4FA; // Soft Blue

    // Draw background and bottom border
    w.draw_rect(0, 0, w.width, bar_height, bg_color);
    w.draw_rect(0, bar_height - 1, w.width, 1, accent_color);

    // Layout params
    let left_pad = 12u64;
    let right_pad = 12u64;
    let v_spacing = 2u64;
    let font_h = w.font.header.height as u64;
    let font_w = w.font.header.width as u64;

    // Top-left: RustOS logo (single line)
    let logo_y = 6u64;
    w.draw_string_at("Rust", left_pad, logo_y, 0xFF0000); // "Rust" in red
    let rust_width = ("Rust".len() as u64) * font_w;
    w.draw_string_at("OS", left_pad + rust_width, logo_y, 0xFFFFFF); // "OS" in white

    // Under the logo: UPTIME
    let uptime = timer::get_uptime_ms() / 1000;
    let uptime_str = format!("UPTIME: {}s", uptime);
    let uptime_y = logo_y + font_h + v_spacing;
    w.draw_string_at(&uptime_str, left_pad, uptime_y, 0xF38BA8);

    // Stack CPU and Heap usage on the right hand side
    // CPU row
    let cpu_row_y = logo_y;
    let cpu_percent = crate::interrupts::get_cpu_usage() as u64;
    let cpu_str = format!("CPU: {}%", cpu_percent);

    // small bar to the left of the CPU text (so text is flush-right)
    let cpu_text_width = (cpu_str.len() as u64) * font_w;
    let bar_width = 80u64;
    let bar_h = 8u64;
    let gap = 6u64;

    // Position the bar and text anchored to the right
    let bar_x = w.width.saturating_sub(right_pad + bar_width);
    let bar_y = cpu_row_y + (font_h / 2).saturating_sub(bar_h / 2);
    let cpu_text_x = bar_x.saturating_sub(gap + cpu_text_width);

    // Draw CPU text and bar (right-aligned)
    w.draw_string_at(&cpu_str, cpu_text_x, cpu_row_y, 0xFFFFFF);
    let filled_w = (bar_width * cpu_percent) / 100;
    w.draw_rect(bar_x, bar_y, bar_width, bar_h, 0x2A2A37); // bar bg
    w.draw_rect(bar_x, bar_y, filled_w, bar_h, 0xA6E3A1); // bar filled

    // Heap row (below CPU), right-aligned
    let heap_row_y = cpu_row_y + font_h + v_spacing;
    let heap_usage = crate::memory::get_heap_usage();
    let heap_size = crate::memory::get_heap_size();
    let heap_str = format!("HEAP: {}/{} KB", heap_usage / 1024, heap_size / 1024);
    let heap_text_width = (heap_str.len() as u64) * font_w;
    let heap_text_x = w.width.saturating_sub(right_pad + heap_text_width);
    w.draw_string_at(&heap_str, heap_text_x, heap_row_y, 0xA6E3A1);
}
