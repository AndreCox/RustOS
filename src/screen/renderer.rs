// In your renderer.rs

use crate::screen::font::{FONT_DATA, Font};
use core::fmt;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use spin::Mutex;

pub static WRITER: Mutex<Option<FramebufferWriter>> = Mutex::new(None);

pub const HEADER_HEIGHT: u64 = 32;

pub fn init(writer: FramebufferWriter<'static>) {
    *WRITER.lock() = Some(writer);
}

pub struct FramebufferWriter<'a> {
    // Hardware framebuffer (what the display controller actually reads)
    pub hardware_fb: &'a mut [u8],

    // Our two software buffers for double buffering
    pub buffer_0: &'a mut [u8],
    pub buffer_1: &'a mut [u8],

    // Track which buffer we're currently drawing to
    current_draw_buffer: AtomicU8, // 0 or 1

    // Track which buffer should be displayed next
    current_display_buffer: AtomicU8, // 0 or 1

    // Flag to indicate we need to copy to hardware
    needs_hw_update: AtomicBool,

    pitch: u64,
    pub width: u64,
    pub height: u64,
    cursor_x: u64,
    cursor_y: u64,
    pub font: Font,
    pub render_offset_y: usize,
    full_redraw: AtomicBool,
}

impl<'a> FramebufferWriter<'a> {
    pub fn new(
        hardware_fb: &'a mut [u8],
        buffer_0: &'a mut [u8],
        buffer_1: &'a mut [u8],
        pitch: u64,
        width: u64,
        height: u64,
    ) -> Self {
        // Clear all buffers initially
        for byte in hardware_fb.iter_mut() {
            *byte = 0;
        }
        for byte in buffer_0.iter_mut() {
            *byte = 0;
        }
        for byte in buffer_1.iter_mut() {
            *byte = 0;
        }

        Self {
            hardware_fb,
            buffer_0,
            buffer_1,
            current_draw_buffer: AtomicU8::new(0),
            current_display_buffer: AtomicU8::new(0),
            needs_hw_update: AtomicBool::new(false),
            pitch,
            width,
            height,
            cursor_x: 0,
            cursor_y: HEADER_HEIGHT,
            font: Font::new(FONT_DATA),
            render_offset_y: 0,
            full_redraw: AtomicBool::new(true),
        }
    }

    #[inline(always)]
    fn get_phys_y(&self, y: u64) -> usize {
        if y < HEADER_HEIGHT {
            y as usize
        } else {
            let log_zone_h = self.height - HEADER_HEIGHT;
            let relative_y = y - HEADER_HEIGHT;
            let phys_rel_y = (relative_y as usize + self.render_offset_y) % log_zone_h as usize;
            (HEADER_HEIGHT as usize) + phys_rel_y
        }
    }
    pub fn draw_rect(&mut self, x: u64, y: u64, width: u64, height: u64, color: u32) {
        let draw_idx = self.current_draw_buffer.load(Ordering::Acquire);
        let pitch = self.pitch;

        // Get the draw buffer pointer
        let draw_buffer_ptr = if draw_idx == 0 {
            self.buffer_0.as_mut_ptr()
        } else {
            self.buffer_1.as_mut_ptr()
        };

        let draw_buffer_len = if draw_idx == 0 {
            self.buffer_0.len()
        } else {
            self.buffer_1.len()
        };

        for row in y..(y + height) {
            let phys_y = self.get_phys_y(row);
            let start = (phys_y * pitch as usize) + (x as usize * 4);
            let row_bytes = (width * 4) as usize;
            if start + row_bytes <= draw_buffer_len {
                unsafe {
                    let ptr = draw_buffer_ptr.add(start) as *mut u32;
                    for i in 0..width as usize {
                        ptr.add(i).write(color);
                    }
                }
            }
        }
        self.full_redraw.store(true, Ordering::Relaxed);
    }

    pub fn clear_rect(&mut self, x: u64, y: u64, width: u64, height: u64) {
        self.draw_rect(x, y, width, height, 0x00000000);
    }

    pub unsafe fn draw_char(&mut self, c: char, x: u64, y: u64, color: u32) {
        let font_w = self.font.header.width as usize;
        let font_h = self.font.header.height as usize;
        let bytes_per_row = (font_w + 7) / 8;
        let glyph = self.font.get_glyph(c);

        let draw_idx = self.current_draw_buffer.load(Ordering::Acquire);
        let pitch = self.pitch;

        // Get the draw buffer pointer
        let draw_buffer_ptr = if draw_idx == 0 {
            self.buffer_0.as_mut_ptr()
        } else {
            self.buffer_1.as_mut_ptr()
        };

        let draw_buffer_len = if draw_idx == 0 {
            self.buffer_0.len()
        } else {
            self.buffer_1.len()
        };

        for row in 0..font_h {
            let phys_y = self.get_phys_y(y + row as u64);
            let row_offset = phys_y * pitch as usize;
            for col in 0..font_w {
                let byte_idx = row * bytes_per_row + (col / 8);
                let bit_idx = 7 - (col % 8);
                if (glyph[byte_idx] >> bit_idx) & 1 == 1 {
                    let pixel_offset = row_offset + ((x + col as u64) as usize * 4);
                    if pixel_offset + 3 < draw_buffer_len {
                        (draw_buffer_ptr.add(pixel_offset) as *mut u32).write(color);
                    }
                }
            }
        }
        self.full_redraw.store(true, Ordering::Relaxed);
    }

    pub fn draw_string_at(&mut self, s: &str, x: u64, y: u64, color: u32) {
        let mut local_x = x;
        for c in s.chars() {
            unsafe {
                self.draw_char(c, local_x, y, color);
            }
            local_x += self.font.header.width as u64;
        }
    }

    pub fn put_char(&mut self, c: char) {
        let char_w = self.font.header.width as u64;
        let char_h = self.font.header.height as u64;
        if self.cursor_y + char_h > self.height {
            self.scroll();
        }
        match c {
            //backspace
            '\x08' => {
                if self.cursor_x >= char_w {
                    self.cursor_x -= char_w;
                    unsafe {
                        self.draw_rect(self.cursor_x, self.cursor_y, char_w, char_h, 0x00000000);
                    }
                }
            }
            '\n' => {
                self.cursor_x = 0;
                self.cursor_y += char_h;
            }
            '\r' => self.cursor_x = 0,
            _ => {
                unsafe {
                    self.draw_char(c, self.cursor_x, self.cursor_y, 0xFFFFFFFF);
                }
                self.cursor_x += char_w;
            }
        }
        if self.cursor_x + char_w > self.width {
            self.cursor_x = 0;
            self.cursor_y += char_h;
        }
    }

    fn scroll(&mut self) {
        let char_h: usize = self.font.header.height as usize;
        let log_zone_h = (self.height - HEADER_HEIGHT) as usize;
        self.render_offset_y = (self.render_offset_y + char_h) % log_zone_h;
        let bottom_y = self.height - char_h as u64;
        self.clear_rect(0, bottom_y, self.width, char_h as u64);
        self.cursor_y = self.height - char_h as u64;
    }

    // Fast page flip between our buffers
    pub fn swap_buffers(&mut self) {
        if !self.full_redraw.swap(false, Ordering::Relaxed) {
            return; // Nothing changed
        }

        // Flip which buffer is "active" for display
        let old_draw = self.current_draw_buffer.load(Ordering::Acquire);
        let old_display = self.current_display_buffer.load(Ordering::Acquire);

        // The buffer we were drawing to is now the display buffer
        self.current_display_buffer
            .store(old_draw, Ordering::Release);

        // The old display buffer becomes our new draw buffer
        self.current_draw_buffer
            .store(old_display, Ordering::Release);

        // Mark that we need to update hardware framebuffer
        self.needs_hw_update.store(true, Ordering::Release);

        // Copy the old display buffer to new draw buffer for continuity
        self.copy_display_to_draw();
    }

    // Copy display buffer to draw buffer for continuity
    fn copy_display_to_draw(&mut self) {
        let draw_idx = self.current_draw_buffer.load(Ordering::Acquire);
        let display_idx = self.current_display_buffer.load(Ordering::Acquire);

        if draw_idx == display_idx {
            return; // Nothing to copy
        }

        let len = self.buffer_0.len();

        unsafe {
            let src = if display_idx == 0 {
                self.buffer_0.as_ptr()
            } else {
                self.buffer_1.as_ptr()
            };

            let dst = if draw_idx == 0 {
                self.buffer_0.as_mut_ptr()
            } else {
                self.buffer_1.as_mut_ptr()
            };

            Self::simd_memcpy(dst, src, len);
        }
    }

    // Chunked present - update hardware framebuffer in smaller chunks
    // This allows interrupts to fire between chunks
    pub fn present_chunked(&mut self) {
        if !self.needs_hw_update.swap(false, Ordering::Acquire) {
            return;
        }

        let display_idx = self.current_display_buffer.load(Ordering::Acquire);
        let src = if display_idx == 0 {
            self.buffer_0.as_ptr()
        } else {
            self.buffer_1.as_ptr()
        };
        let dst = self.hardware_fb.as_mut_ptr();

        let pitch = self.pitch as usize;
        let header_bytes = HEADER_HEIGHT as usize * pitch;
        let log_zone_h = (self.height - HEADER_HEIGHT) as usize;
        let offset_bytes = self.render_offset_y * pitch;
        let total_log_bytes = log_zone_h * pitch;

        unsafe {
            // 1. Copy Header (Fixed)
            Self::simd_memcpy(dst, src, header_bytes);

            // 2. Copy the "Bottom" half of memory to the "Top" of the screen
            // This is the data from the offset to the end of the buffer
            let part_a_src = src.add(header_bytes + offset_bytes);
            let part_a_dst = dst.add(header_bytes);
            let part_a_len = total_log_bytes - offset_bytes;
            Self::simd_memcpy(part_a_dst, part_a_src, part_a_len);

            // 3. Copy the "Top" half of memory to the "Bottom" of the screen
            // This is the data from the start of the log zone up to the offset
            if offset_bytes > 0 {
                let part_b_src = src.add(header_bytes);
                let part_b_dst = dst.add(header_bytes + part_a_len);
                let part_b_len = offset_bytes;
                Self::simd_memcpy(part_b_dst, part_b_src, part_b_len);
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn simd_memcpy(dst: *mut u8, src: *const u8, len: usize) {
        use core::arch::x86_64::*;
        let mut offset = 0;

        #[cfg(target_feature = "avx2")]
        {
            while offset + 32 <= len {
                _mm256_storeu_si256(
                    dst.add(offset) as *mut __m256i,
                    _mm256_loadu_si256(src.add(offset) as *const __m256i),
                );
                offset += 32;
            }
        }

        while offset + 8 <= len {
            (dst.add(offset) as *mut u64)
                .write_unaligned((src.add(offset) as *const u64).read_unaligned());
            offset += 8;
        }

        while offset < len {
            dst.add(offset).write(src.add(offset).read());
            offset += 1;
        }
    }
}

impl<'a> fmt::Write for FramebufferWriter<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.put_char(c);
        }
        Ok(())
    }
}
