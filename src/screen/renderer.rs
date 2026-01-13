use crate::screen::font;

use super::font::{FONT_DATA, Font};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use spin::Mutex;

pub static WRITER: Mutex<Option<FramebufferWriter>> = Mutex::new(None);

pub fn init(writer: FramebufferWriter<'static>) {
    *WRITER.lock() = Some(writer);
}

#[derive(Clone, Copy)]
struct DirtyRect {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

pub struct FramebufferWriter<'a> {
    pub framebuffer: &'a mut [u8],
    pub backbuffer: Vec<u8>,
    pitch: u64,
    pub width: u64,
    pub height: u64,
    cursor_x: u64,
    cursor_y: u64,
    pub font: Font,

    dirty_rects: Vec<DirtyRect>,
    full_redraw: bool,
}

impl<'a> FramebufferWriter<'a> {
    pub fn new(framebuffer: &'a mut [u8], pitch: u64, width: u64, height: u64) -> Self {
        let mut backbuffer = vec![0; framebuffer.len()];
        backbuffer.copy_from_slice(framebuffer);

        Self {
            framebuffer,
            backbuffer: backbuffer,
            pitch,
            width,
            height,
            cursor_x: 0,
            cursor_y: 32,
            font: Font::new(FONT_DATA),

            dirty_rects: Vec::with_capacity(64),
            full_redraw: false,
        }
    }

    fn mark_dirty(&mut self, x: u64, y: u64, width: u64, height: u64) {
        // merge with existing dirty rects if overlapping
        for rect in &mut self.dirty_rects {
            if Self::rectangles_overlap(*rect, x, y, width, height) {
                *rect = Self::merge_rectangles(*rect, x, y, width, height);
                return;
            }
        }

        if self.dirty_rects.len() < 64 {
            self.dirty_rects.push(DirtyRect {
                x,
                y,
                width,
                height,
            });
        } else {
            // rerender full screen if too many dirty rects
            self.full_redraw = true;
        }
    }

    fn merge_rectangles(rect: DirtyRect, x: u64, y: u64, w: u64, h: u64) -> DirtyRect {
        let min_x = rect.x.min(x);
        let min_y = rect.y.min(y);
        let max_x = (rect.x + rect.width).max(x + w);
        let max_y = (rect.y + rect.height).max(y + h);

        DirtyRect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }

    fn rectangles_overlap(rect: DirtyRect, x: u64, y: u64, w: u64, h: u64) -> bool {
        !(rect.x + rect.width < x || x + w < rect.x || rect.y + rect.height < y || y + h < rect.y)
    }

    /// Writes a pixel at (x, y) with the specified color
    pub fn write_pixel(&mut self, x: u64, y: u64, color: u32) {
        let byte_offset = (y * self.pitch + x * 4) as usize;

        if byte_offset + 3 < self.backbuffer.len() {
            unsafe {
                let pixel_ptr = self.backbuffer.as_mut_ptr().add(byte_offset) as *mut u32;
                pixel_ptr.write(color);
            }

            self.mark_dirty(x, y, 1, 1);
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn simd_memcpy(dst: *mut u8, src: *const u8, len: usize) {
        use core::arch::x86_64::*;

        let mut offset = 0usize;

        // If compiled with avx2 support, use 256-bit copies
        #[cfg(target_feature = "avx2")]
        {
            while offset + 32 <= len {
                let data = _mm256_loadu_si256(src.add(offset) as *const __m256i);
                _mm256_storeu_si256(dst.add(offset) as *mut __m256i, data);
                offset += 32;
            }
        }

        // If avx2 is not available but sse2 is compiled in, use 128-bit copies
        #[cfg(all(not(target_feature = "avx2"), target_feature = "sse2"))]
        {
            while offset + 16 <= len {
                let data = _mm_loadu_si128(src.add(offset) as *const __m128i);
                _mm_storeu_si128(dst.add(offset) as *mut __m128i, data);
                offset += 16;
            }
        }

        // Copy remaining bytes with 8-byte chunks
        while offset + 8 <= len {
            let data = (src.add(offset) as *const u64).read_unaligned();
            (dst.add(offset) as *mut u64).write_unaligned(data);
            offset += 8;
        }

        // Copy remaining bytes one at a time
        while offset < len {
            dst.add(offset).write(src.add(offset).read());
            offset += 1;
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn simd_memset(dst: *mut u8, value: u8, len: usize) {
        use core::arch::x86_64::*;

        let mut offset = 0;

        #[cfg(target_feature = "avx2")]
        {
            // Create a 256-bit value filled with the byte
            let pattern = _mm256_set1_epi8(value as i8);

            // Set 32 bytes at a time
            while offset + 32 <= len {
                _mm256_storeu_si256(dst.add(offset) as *mut __m256i, pattern);
                offset += 32;
            }
        }
        #[cfg(all(not(target_feature = "avx2"), target_feature = "sse2"))]
        {
            // Create a 128-bit value filled with the byte
            let pattern = _mm_set1_epi8(value as i8);

            // Set 16 bytes at a time
            while offset + 16 <= len {
                _mm_storeu_si128(dst.add(offset) as *mut __m128i, pattern);
                offset += 16;
            }
        }

        // Set remaining bytes with 8-byte chunks
        let val64 = (value as u64) * 0x0101010101010101u64;
        while offset + 8 <= len {
            (dst.add(offset) as *mut u64).write_unaligned(val64);
            offset += 8;
        }

        // Set remaining bytes one at a time
        while offset < len {
            dst.add(offset).write(value);
            offset += 1;
        }
    }

    pub unsafe fn draw_char(&mut self, c: char, x: u64, y: u64, color: u32) {
        // 1. Get the font data first
        let font_width = self.font.header.width as usize;
        let font_height = self.font.header.height as usize;
        let bytes_per_row = (font_width + 7) / 8;

        self.mark_dirty(x, y, font_width as u64, font_height as u64);

        let glyph = self.font.get_glyph(c);

        // 2. Access the hardware fields directly to avoid calling a &mut self method
        let pitch = self.pitch;
        let fb_len = self.backbuffer.len();
        let fb_ptr = self.backbuffer.as_mut_ptr();

        for row in 0..font_height {
            for col in 0..font_width {
                let byte_offset = row * bytes_per_row + (col / 8);
                let bit_offset = 7 - (col % 8);

                if (glyph[byte_offset] >> bit_offset) & 1 == 1 {
                    let pixel_x = x + col as u64;
                    let pixel_y = y + row as u64;

                    // 3. Manual pixel write logic (Inlined write_pixel)
                    let offset = (pixel_y * pitch + pixel_x * 4) as usize;
                    if offset + 3 < fb_len {
                        unsafe {
                            let pixel_ptr = fb_ptr.add(offset) as *mut u32;
                            pixel_ptr.write(color);
                        }
                    }
                }
            }
        }
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

    pub fn draw_rect(&mut self, x: u64, y: u64, width: u64, height: u64, color: u32) {
        self.mark_dirty(x, y, width, height);

        let fb_ptr = self.backbuffer.as_mut_ptr();

        unsafe {
            for row in y..(y + height) {
                let row_offset = (row * self.pitch + x * 4) as usize;

                // Write entire row at once
                if row_offset + (width * 4) as usize <= self.backbuffer.len() {
                    let pixels_ptr = fb_ptr.add(row_offset) as *mut u32;
                    for i in 0..width as usize {
                        pixels_ptr.add(i).write(color);
                    }
                }
            }
        }
    }

    pub fn clear_rect(&mut self, x: u64, y: u64, width: u64, height: u64) {
        self.mark_dirty(x, y, width, height);

        for row in y..(y + height) {
            let row_start = (row * self.pitch + x * 4) as usize;
            let row_len = (width * 4) as usize;
            let row_end = (row_start + row_len).min(self.backbuffer.len());

            if row_start < self.backbuffer.len() && row_end <= self.backbuffer.len() {
                self.backbuffer[row_start..row_end].fill(0);
            }
        }
    }

    pub fn backspace(&mut self) {
        let char_width = self.font.header.width as u64;
        let char_height = self.font.header.height as u64;

        if self.cursor_x >= char_width {
            self.cursor_x -= char_width;
        } else if self.cursor_y >= char_height {
            self.cursor_y -= char_height;
            self.cursor_x = (self.width / char_width) * char_width - char_width;
        } else {
            return;
        }

        self.clear_rect(self.cursor_x, self.cursor_y, char_width, char_height);
    }

    pub fn put_char(&mut self, c: char) {
        let char_width = self.font.header.width as u64;
        let char_height = self.font.header.height as u64;

        if self.cursor_y + char_height > self.height {
            self.scroll();
        }

        match c {
            '\n' => {
                self.cursor_x = 0;
                self.cursor_y += char_height;
            }
            '\r' => {
                let row_start = (self.cursor_y * self.pitch) as usize;
                let row_end = ((self.cursor_y + char_height) * self.pitch) as usize;

                if row_end <= self.backbuffer.len() {
                    self.backbuffer[row_start..row_end].fill(0);
                }

                self.cursor_x = 0;
            }
            _ => {
                unsafe {
                    self.draw_char(c, self.cursor_x, self.cursor_y, 0xFFFFFFFF);
                }
                self.cursor_x += char_width;
            }
        }

        if self.cursor_x + char_width > self.width {
            self.cursor_x = 0;
            self.cursor_y += char_height;
        }
    }

    fn scroll(&mut self) {
        let char_height = self.font.header.height as usize;
        let pitch = self.pitch as usize;
        let bytes_per_text_row = char_height * pitch;

        unsafe {
            Self::simd_memcpy(
                self.backbuffer.as_mut_ptr(),
                self.backbuffer.as_ptr().add(bytes_per_text_row),
                self.backbuffer.len() - bytes_per_text_row,
            );
        }

        // Clear the bottom row with SIMD
        let last_row_start = (self.height as usize - char_height) * pitch;
        if last_row_start < self.backbuffer.len() {
            unsafe {
                Self::simd_memset(
                    self.backbuffer.as_mut_ptr().add(last_row_start),
                    0,
                    self.backbuffer.len() - last_row_start,
                );
            }
        }

        self.cursor_y = self.height - char_height as u64;

        // Full redraw needed after scroll
        self.full_redraw = true;
    }
}

impl<'a> fmt::Write for FramebufferWriter<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            if c == '\x08' {
                self.backspace();
                continue;
            }
            self.put_char(c);
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    // We disable interrupts to prevent deadlocks when an interrupt handler
    // also tries to print while WRITER is locked.
    x86_64::instructions::interrupts::without_interrupts(|| {
        if let Some(ref mut writer) = *WRITER.lock() {
            let _ = writer.write_fmt(args);
        }
    });
}

#[macro_export]
macro_rules! screen_print {
    ($($arg:tt)*) => ($crate::screen::renderer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! screen_println {
    () => ($crate::screen_print!("\n"));
    ($($arg:tt)*) => ($crate::screen_print!("{}\n", format_args!($($arg)*)));
}

pub fn swap_buffers() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        if let Some(ref mut w) = *WRITER.lock() {
            if w.full_redraw {
                // Full copy with SIMD acceleration
                unsafe {
                    FramebufferWriter::simd_memcpy(
                        w.framebuffer.as_mut_ptr(),
                        w.backbuffer.as_ptr(),
                        w.backbuffer.len(),
                    );
                }
                w.full_redraw = false;
            } else {
                // Copy dirty regions with SIMD
                for rect in &w.dirty_rects {
                    let start_y = rect.y as usize;
                    let end_y = (rect.y + rect.height).min(w.height) as usize;

                    for y in start_y..end_y {
                        let row_start = (y as u64 * w.pitch + rect.x * 4) as usize;
                        let row_len = (rect.width * 4) as usize;
                        let row_end = row_start + row_len;

                        if row_end <= w.framebuffer.len() {
                            unsafe {
                                FramebufferWriter::simd_memcpy(
                                    w.framebuffer.as_mut_ptr().add(row_start),
                                    w.backbuffer.as_ptr().add(row_start),
                                    row_len,
                                );
                            }
                        }
                    }
                }
            }
            w.dirty_rects.clear();
        }
    });
}
