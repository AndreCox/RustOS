use crate::screen::font::{FONT_DATA, Font};
use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

pub static WRITER: Mutex<Option<FramebufferWriter>> = Mutex::new(None);

pub const HEADER_HEIGHT: u64 = 32; // Sync this with your UI

pub fn init(writer: FramebufferWriter<'static>) {
    *WRITER.lock() = Some(writer);
}

pub struct FramebufferWriter<'a> {
    pub framebuffer: &'a mut [u8],
    pub backbuffer: &'static mut [u8],
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
        framebuffer: &'a mut [u8],
        backbuffer: &'static mut [u8],
        pitch: u64,
        width: u64,
        height: u64,
    ) -> Self {
        Self {
            framebuffer,
            backbuffer,
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
        for row in y..(y + height) {
            let phys_y = self.get_phys_y(row);
            let start = (phys_y * self.pitch as usize) + (x as usize * 4);
            let row_bytes = (width * 4) as usize;
            if start + row_bytes <= self.backbuffer.len() {
                unsafe {
                    let ptr = self.backbuffer.as_mut_ptr().add(start) as *mut u32;
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

        for row in 0..font_h {
            let phys_y = self.get_phys_y(y + row as u64);
            let row_offset = phys_y * self.pitch as usize;
            for col in 0..font_w {
                let byte_idx = row * bytes_per_row + (col / 8);
                let bit_idx = 7 - (col % 8);
                if (glyph[byte_idx] >> bit_idx) & 1 == 1 {
                    let pixel_offset = row_offset + ((x + col as u64) as usize * 4);
                    if pixel_offset + 3 < self.backbuffer.len() {
                        (self.backbuffer.as_mut_ptr().add(pixel_offset) as *mut u32).write(color);
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
        let log_zone_h = (self.height - HEADER_HEIGHT) as usize;
        self.render_offset_y = (self.render_offset_y + char_h) % log_zone_h;
        let bottom_y = self.height - char_h as u64;
        self.clear_rect(0, bottom_y, self.width, char_h as u64);
        self.cursor_y = self.height - char_h as u64;
    }

    pub fn swap_buffers(&mut self) {
        if !self.full_redraw.swap(false, Ordering::Relaxed) {
            return;
        }
        let pitch = self.pitch as usize;
        let header_bytes = (HEADER_HEIGHT * self.pitch) as usize;
        let log_bytes_total = self.backbuffer.len() - header_bytes;
        let split_point = self.render_offset_y * pitch;

        unsafe {
            let dst = self.framebuffer.as_mut_ptr();
            let src = self.backbuffer.as_ptr();
            Self::simd_memcpy(dst, src, header_bytes); // Header
            let part_a_len = log_bytes_total - split_point;
            Self::simd_memcpy(
                dst.add(header_bytes),
                src.add(header_bytes + split_point),
                part_a_len,
            );
            if split_point > 0 {
                Self::simd_memcpy(
                    dst.add(header_bytes + part_a_len),
                    src.add(header_bytes),
                    split_point,
                );
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
