use core::fmt::Write;
use core::{arch::x86_64, fmt};
use spin::Mutex;

pub static WRITER: Mutex<Option<FramebufferWriter>> = Mutex::new(None);

pub fn init(writer: FramebufferWriter<'static>) {
    *WRITER.lock() = Some(writer);
}

#[repr(C)]
struct PsfHeader {
    magic: u32, // 0x864ab572 for PSF2
    version: u32,
    header_size: u32,
    flags: u32,
    length: u32,    // Number of glyphs (usually 256)
    char_size: u32, // Bytes per glyph (e.g., 16)
    height: u32,    // Height in pixels
    width: u32,     // Width in pixels
}

pub struct Font {
    data: &'static [u8],
    header: &'static PsfHeader,
}
#[repr(align(16))]
struct AlignedData<T: ?Sized>(T);

static FONT_DATA: &AlignedData<[u8]> =
    &AlignedData(*include_bytes!("../assets/fonts/sanserif.psf"));

impl Font {
    pub fn new(data: &'static AlignedData<[u8]>) -> Self {
        unsafe {
            // 2. Access .0 here (inside the function)
            let header = &*(data.0.as_ptr() as *const PsfHeader);
            Self {
                data: &data.0,
                header,
            }
        }
    }

    pub fn get_glyph(&self, c: char) -> &[u8] {
        let codepoint = if (c as u32) < self.header.length {
            c as u32
        } else {
            0
        };
        // Calculate where the bits for this character start
        let offset = self.header.header_size as usize
            + (codepoint as usize * self.header.char_size as usize);

        &self.data[offset..offset + self.header.char_size as usize]
    }
}

// Use u32 for color (0xFFFFFFFF = White)
pub unsafe fn write_pixel(x: u64, y: u64, color: u32, framebuffer: &mut [u8], pitch: u64) {
    // calculate the exact byte position
    let byte_offset = y * pitch + x * 4;

    // bound check
    if byte_offset + 3 < framebuffer.len() as u64 {
        unsafe {
            let pixel_ptr = framebuffer.as_mut_ptr().add(byte_offset as usize) as *mut u32;
            pixel_ptr.write_volatile(color);
        }
    }
}

pub unsafe fn draw_char(c: char, x: u64, y: u64, color: u32, fb: &mut [u8], pitch: u64) {
    let font = Font::new(FONT_DATA);
    let glyph = font.get_glyph(c);

    let bytes_per_row = (font.header.width as usize + 7) / 8;

    // Use usize for the loops to make indexing 'glyph' easy
    for row in 0..font.header.height as usize {
        for col in 0..font.header.width as usize {
            let byte_offset = row * bytes_per_row + (col / 8);
            let bit_offset = 7 - (col % 8);

            let bit_is_set = (glyph[byte_offset] >> bit_offset) & 1 == 1;

            if bit_is_set {
                // Cast row and col to u64 to add them to x and y
                unsafe {
                    write_pixel(x + col as u64, y + row as u64, color, fb, pitch);
                }
            }
        }
    }
}

pub struct FramebufferWriter<'a> {
    framebuffer: &'a mut [u8],
    pitch: u64,
    width: u64,
    height: u64,
    cursor_x: u64,
    cursor_y: u64,
}

impl<'a> FramebufferWriter<'a> {
    pub fn new(framebuffer: &'a mut [u8], pitch: u64, width: u64, height: u64) -> Self {
        Self {
            framebuffer,
            pitch,
            width,
            height,
            cursor_x: 0,
            cursor_y: 0,
        }
    }

    pub fn backspace(&mut self) {
        let font = Font::new(FONT_DATA);
        let char_width = font.header.width as u64;
        let char_height = font.header.height as u64;

        // don't backspace if at the start of the line
        if self.cursor_x >= char_width {
            self.cursor_x -= char_width;

            for y in 0..char_height {
                for x in 0..char_width {
                    unsafe {
                        write_pixel(
                            self.cursor_x + x,
                            self.cursor_y + y,
                            0x00000000,
                            self.framebuffer,
                            self.pitch,
                        );
                    }
                }
            }
        } else if self.cursor_y >= char_height {
            // Move to end of previous line
            self.cursor_y -= char_height;
            self.cursor_x = self.width - char_width;

            for y in 0..char_height {
                for x in 0..char_width {
                    unsafe {
                        write_pixel(
                            self.cursor_x + x,
                            self.cursor_y + y,
                            0x00000000,
                            self.framebuffer,
                            self.pitch,
                        );
                    }
                }
            }
        }
    }

    pub fn put_char(&mut self, c: char) {
        let font = Font::new(FONT_DATA);
        let char_width = font.header.width as u64;
        let char_height = font.header.height as u64;

        if c == '\n' {
            self.cursor_x = 0;
            self.cursor_y += char_height;
            return;
        }

        if c == '\r' {
            self.cursor_x = 0;
            return;
        }

        unsafe {
            draw_char(
                c,
                self.cursor_x,
                self.cursor_y,
                0xFFFFFFFF,
                self.framebuffer,
                self.pitch,
            );
        }

        self.cursor_x += char_width;

        // Wrap text to next line if we hit the edge
        if self.cursor_x + char_width > self.width {
            self.cursor_x = 0;
            self.cursor_y += char_height;
        }
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

    unsafe {
        // 1. Save RFLAGS and disable interrupts
        let rflags: u64;
        core::arch::asm!(
            "pushfq",
            "pop {}",
            out(reg) rflags,
            options(nomem, preserves_flags) // Fixed the 's' here!
        );
        core::arch::asm!("cli", options(nomem, nostack));

        // 2. Perform the locked write
        if let Some(ref mut writer) = *WRITER.lock() {
            let _ = writer.write_fmt(args);
        }

        // 3. Restore interrupts ONLY if they were enabled (Bit 9)
        if (rflags & (1 << 9)) != 0 {
            core::arch::asm!("sti", options(nomem, nostack));
        }
    }
}

#[macro_export]
macro_rules! screen_print {
    ($($arg:tt)*) => ($crate::renderer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! screen_println {
    () => ($crate::screen_print!("\n"));
    ($($arg:tt)*) => ($crate::screen_print!("{}\n", format_args!($($arg)*)));
}
