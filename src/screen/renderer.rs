// In your renderer.rs

use crate::screen::font::{FONT_DATA, Font};
use core::fmt;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

pub static WRITER: Mutex<Option<FramebufferWriter>> = Mutex::new(None);

pub const HEADER_HEIGHT: u64 = 32;

pub fn init(writer: FramebufferWriter<'static>) {
    *WRITER.lock() = Some(writer);
}

pub struct FramebufferWriter<'a> {
    // Hardware framebuffer (VRAM)
    pub hardware_fb: &'a mut [u8],

    // Software Shadow Buffer (RAM)
    // We only use buffer_0 as our "Shadow". buffer_1 is unused in this optimized method,
    // but kept to match your initialization signature.
    pub buffer: &'a mut [u8],

    // Dirty tracking (Y-coordinates only are usually sufficient for terminal optimization)
    dirty_min_y: usize,
    dirty_max_y: usize,

    // Flag to indicate we need to copy to hardware
    needs_hw_update: AtomicBool,

    pub pitch: u64,
    pub width: u64,
    pub height: u64,
    cursor_x: u64,
    cursor_y: u64,
    pub font: Font,
}

impl<'a> FramebufferWriter<'a> {
    pub fn new(
        hardware_fb: &'a mut [u8],
        buffer_0: &'a mut [u8],
        _buffer_1: &'a mut [u8], // Unused in Shadow Buffer mode
        pitch: u64,
        width: u64,
        height: u64,
    ) -> Self {
        // Clear buffers
        hardware_fb.fill(0);
        buffer_0.fill(0);

        Self {
            hardware_fb,
            buffer: buffer_0,
            dirty_min_y: 0,
            dirty_max_y: height as usize, // Initial full redraw
            needs_hw_update: AtomicBool::new(true),
            pitch,
            width,
            height,
            cursor_x: 0,
            cursor_y: HEADER_HEIGHT,
            font: Font::new(FONT_DATA),
        }
    }

    /// Mark a vertical range as dirty so it gets copied to GPU next frame
    #[inline]
    pub fn mark_dirty(&mut self, y: u64, height: u64) {
        let y_start = y as usize;
        let y_end = (y + height) as usize;

        if y_start < self.dirty_min_y {
            self.dirty_min_y = y_start;
        }
        if y_end > self.dirty_max_y {
            self.dirty_max_y = y_end.min(self.height as usize);
        }
        self.needs_hw_update.store(true, Ordering::Relaxed);
    }

    pub fn draw_rect(&mut self, x: u64, y: u64, width: u64, height: u64, color: u32) {
        let pitch = self.pitch as usize;
        let width = width as usize;

        // Cast the byte buffer to u32 once for easier indexing
        // Note: This assumes self.buffer is aligned to 4 bytes!
        let (_, pixels, _) = unsafe { self.buffer.align_to_mut::<u32>() };
        let pixels_per_row = pitch / 4;

        for row in y as usize..(y + height) as usize {
            let start = row * pixels_per_row + x as usize;
            let end = start + width;

            if end <= pixels.len() {
                // This is significantly faster than a manual 'for' loop
                pixels[start..end].fill(color);
            }
        }
        self.mark_dirty(y, height);
    }

    pub fn clear_rect(&mut self, x: u64, y: u64, width: u64, height: u64) {
        self.draw_rect(x, y, width, height, 0x00000000);
    }

    pub unsafe fn draw_char(&mut self, c: char, x: u64, y: u64, color: u32) {
        let font_w = self.font.header.width as usize;
        let font_h = self.font.header.height as usize;
        let bytes_per_row = (font_w + 7) / 8;
        let glyph = self.font.get_glyph(c);
        let pitch = self.pitch as usize;

        // Get the base pointer to the start of the character on the screen
        let start_offset = (y as usize * pitch) + (x as usize * 4);
        let buffer_ptr = self.buffer.as_mut_ptr().add(start_offset) as *mut u32;

        for row in 0..font_h {
            // Pointer to the start of this specific row on the screen
            let row_ptr = (buffer_ptr as *mut u8).add(row * pitch) as *mut u32;

            for byte_col in 0..bytes_per_row {
                let font_byte = glyph[row * bytes_per_row + byte_col];

                // Process 8 pixels at a time from one font byte
                for bit in 0..8 {
                    let col = byte_col * 8 + bit;
                    if col >= font_w {
                        break;
                    } // Handle fonts not divisible by 8

                    // Check bit from most significant to least significant
                    if (font_byte << bit) & 0x80 != 0 {
                        row_ptr.add(col).write(color);
                    }
                }
            }
        }
        self.mark_dirty(y, font_h as u64);
    }

    pub fn clear_screen(&mut self) {
        self.buffer.fill(0);
        self.mark_dirty(0, self.height);
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
            '\x08' => {
                // Backspace
                if self.cursor_x >= char_w {
                    self.cursor_x -= char_w;
                    self.clear_rect(self.cursor_x, self.cursor_y, char_w, char_h);
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
        let char_h = self.font.header.height as usize;
        let pitch = self.pitch as usize;
        let buffer_len = self.buffer.len();

        // Define the scrollable area (excluding header)
        let header_bytes = HEADER_HEIGHT as usize * pitch;
        let scroll_zone_bytes = buffer_len - header_bytes;
        let bytes_to_shift = scroll_zone_bytes - (char_h * pitch);

        unsafe {
            let ptr = self.buffer.as_mut_ptr();

            // Move memory UP: dst = header_end, src = header_end + 1 line
            ptr::copy(
                ptr.add(header_bytes + char_h * pitch), // src
                ptr.add(header_bytes),                  // dst
                bytes_to_shift,                         // count
            );

            // Clear the new bottom line
            let bottom_start = header_bytes + bytes_to_shift;
            ptr::write_bytes(ptr.add(bottom_start), 0, char_h * pitch);
        }

        self.cursor_y -= char_h as u64;

        // Scrolling invalidates the whole scrollable area
        self.mark_dirty(HEADER_HEIGHT, self.height - HEADER_HEIGHT);
    }

    /// Optimized present function
    /// Only copies rows that have changed (dirty) to the hardware framebuffer
    pub fn present(&mut self) {
        if !self.needs_hw_update.swap(false, Ordering::Acquire) {
            return;
        }

        // Capture current dirty bounds
        let y_start = self.dirty_min_y;
        let y_end = self.dirty_max_y;

        // Reset dirty bounds for next frame (inverted logic to expand on next write)
        self.dirty_min_y = self.height as usize;
        self.dirty_max_y = 0;

        // Sanity check
        if y_start >= y_end {
            return;
        }

        let pitch = self.pitch as usize;
        let start_offset = y_start * pitch;
        let end_offset = y_end * pitch;
        let len = end_offset - start_offset;

        // Safety bounds check
        if end_offset > self.hardware_fb.len() || end_offset > self.buffer.len() {
            return;
        }

        unsafe {
            let src = self.buffer.as_ptr().add(start_offset);
            let dst = self.hardware_fb.as_mut_ptr().add(start_offset);

            Self::simd_memcpy(dst, src, len);
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn simd_memcpy(dst: *mut u8, src: *const u8, len: usize) {
        use core::arch::x86_64::*;
        let mut offset = 0;

        // Use AVX2 if available (32 bytes at a time)
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

        // Fallback to SSE/64-bit copy
        while offset + 8 <= len {
            (dst.add(offset) as *mut u64)
                .write_unaligned((src.add(offset) as *const u64).read_unaligned());
            offset += 8;
        }

        // Handle remaining bytes
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
