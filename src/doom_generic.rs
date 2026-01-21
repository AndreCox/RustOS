use core::ffi::{CStr, c_char, c_int, c_void};
use core::sync::atomic::Ordering;

use crate::{io, println};

// =============================================================================
// DOOM MEMORY & CONFIG
// =============================================================================

unsafe extern "C" {
    pub static mut DG_ScreenBuffer: *mut u32;
    // We link these from C to avoid the "Duplicate Symbol" error
    // but we must initialize them in DG_Init or task_doom!
}

#[unsafe(no_mangle)]
#[used]
pub static mut DG_Width: i32 = 640;

#[unsafe(no_mangle)]
#[used]
pub static mut DG_Height: i32 = 400;

// The WAD data embedded in the kernel
static WAD_DATA: &[u8] = include_bytes!("../assets/data/DOOM.WAD");
static mut WAD_CURSOR: usize = 0;

// A dummy buffer to act as the FILE struct.
// This prevents segfaults if libc tries to read file flags/locks.
static mut DUMMY_FILE_STRUCT: [u8; 128] = [0; 128];

// =============================================================================
// INITIALIZATION
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DG_Init() {
    crate::println!("[DOOM] DG_Init - Starting...");

    // Switch to Exclusive Graphics Mode
    crate::screen::EXCLUSIVE_GRAPHICS.store(true, Ordering::SeqCst);

    // Ensure dimensions are correct (Now 640x400 scaled from 320x200 by C side)
    DG_Width = 640;
    DG_Height = 400;
}

// =============================================================================
// VIDEO & INPUT
// =============================================================================
use core::arch::x86_64::*;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DG_DrawFrame() {
    if DG_ScreenBuffer.is_null() {
        return;
    }

    let Some(mut guard) = crate::screen::renderer::WRITER.try_lock() else {
        return;
    };
    let Some(writer) = guard.as_mut() else {
        return;
    };

    let src_ptr = DG_ScreenBuffer;
    let fb_ptr = writer.buffer.as_mut_ptr() as *mut u32;
    let stride = (writer.pitch / 4) as usize;

    for y in 0..400 {
        let src_row = src_ptr.add(y * 640);
        let dst_row0 = fb_ptr.add((y * 2) * stride);
        let dst_row1 = fb_ptr.add((y * 2 + 1) * stride);

        // Process 4 source pixels at a time (outputting 8 destination pixels)
        // 640 / 4 = 160 iterations
        for x in (0..640).step_by(4) {
            // Load 4 pixels: [P3, P2, P1, P0]
            let src_pixels = _mm_loadu_si128(src_row.add(x) as *const __m128i);

            // Unpack Low: [P1, P1, P0, P0]
            let low = _mm_unpacklo_epi32(src_pixels, src_pixels);
            // Unpack High: [P3, P3, P2, P2]
            let high = _mm_unpackhi_epi32(src_pixels, src_pixels);

            // Store to first row
            _mm_storeu_si128(dst_row0.add(x * 2) as *mut __m128i, low);
            _mm_storeu_si128(dst_row0.add(x * 2 + 4) as *mut __m128i, high);

            // Store to second row (vertical doubling)
            _mm_storeu_si128(dst_row1.add(x * 2) as *mut __m128i, low);
            _mm_storeu_si128(dst_row1.add(x * 2 + 4) as *mut __m128i, high);
        }
    }
    writer.mark_dirty(0, 800);
}

#[unsafe(no_mangle)]
pub extern "C" fn DG_SleepMs(ms: u32) {
    crate::timer::sleep_ms(ms as u64);
}

#[unsafe(no_mangle)]
pub extern "C" fn DG_GetTicksMs() -> u32 {
    crate::timer::get_uptime_ms() as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DG_GetKey(pressed: *mut i32, key: *mut u8) -> i32 {
    if pressed.is_null() || key.is_null() {
        return 0;
    }

    if let Some(scancode) = crate::io::keyboard::SCANCODE_QUEUE.pop() {
        let is_released = (scancode & 0x80) != 0;
        let base_scancode = scancode & 0x7F;

        unsafe {
            *pressed = if is_released { 0 } else { 1 };
        }

        let doom_key = match base_scancode {
            0x01 => 0x1b, // Escape
            0x1c => 0x0d, // Enter
            0x39 => 0xa2, // Spacebar (Doom KEY_USE)

            // DoomGeneric often uses these specific constants:
            0x1d => 0xa3, // Left Control (Doom KEY_FIRE / KEY_RCTRL)
            0x38 => 0x92, // Left Alt (Doom KEY_STRAFE / KEY_RALT)

            // Arrows
            0x48 => 0xad, // Up
            0x50 => 0xaf, // Down
            0x4b => 0xac, // Left
            0x4d => 0xae, // Right

            _ => 0,
        };

        println!(
            "Key Event: Base={:#x}, Released={}, DoomKey={:#x}",
            base_scancode, is_released, doom_key
        );

        if doom_key != 0 {
            unsafe {
                *key = doom_key;
            }
            return 1;
        }

        // Fallback to ASCII
        if let Some(c) = crate::io::keyboard::scancode_to_char(base_scancode) {
            unsafe {
                *key = c as u8;
            }
            return 1;
        }
    }
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn DG_SetWindowTitle(_title: *const c_char) {}

// =============================================================================
// FILE SYSTEM (CRITICAL FIXES)
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fopen(path: *const c_char, mode: *const c_char) -> *mut c_void {
    if path.is_null() {
        return core::ptr::null_mut();
    }

    let path_slice = CStr::from_ptr(path).to_bytes();
    let mode_slice = CStr::from_ptr(mode).to_bytes();
    let path_str = core::str::from_utf8(path_slice).unwrap_or("?");

    // 1. Detect WAD Open Request
    let mut is_wad = false;
    for window in path_slice.windows(3) {
        if window.eq_ignore_ascii_case(b"wad") {
            is_wad = true;
            break;
        }
    }

    if is_wad && mode_slice.contains(&b'r') {
        crate::println!("[FS] fopen: {:?} -> OK (WAD)", path_str);
        WAD_CURSOR = 0;
        // RETURN VALID MEMORY, NOT RANDOM NUMBERS
        return core::ptr::addr_of_mut!(DUMMY_FILE_STRUCT) as *mut c_void;
    }

    // 2. DENY WRITES to prevent Crash
    // Doom tries to write default.cfg. If we return a fake pointer, fprintf crashes.
    // Returning NULL tells Doom "Write Failed", so it continues safely.
    if mode_slice.contains(&b'w') || mode_slice.contains(&b'a') || mode_slice.contains(&b'+') {
        crate::println!("[FS] fopen: {:?} (Write) -> DENIED", path_str);
        return core::ptr::null_mut();
    }

    crate::println!("[FS] fopen: {:?} -> Not Found", path_str);
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fread(
    ptr: *mut c_void,
    size: usize,
    nmemb: usize,
    _fp: *mut c_void,
) -> usize {
    if size == 0 || nmemb == 0 || ptr.is_null() {
        return 0;
    }

    let to_read = size * nmemb;
    let available = WAD_DATA.len().saturating_sub(WAD_CURSOR);
    let actual = if to_read > available {
        available
    } else {
        to_read
    };

    if actual > 0 {
        core::ptr::copy_nonoverlapping(WAD_DATA.as_ptr().add(WAD_CURSOR), ptr as *mut u8, actual);
        WAD_CURSOR += actual;
    }

    actual / size
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fseek(_fp: *mut c_void, offset: i64, whence: i32) -> i32 {
    let new_pos: i64 = match whence {
        0 => offset,                                       // SEEK_SET
        1 => (WAD_CURSOR as i64).wrapping_add(offset),     // SEEK_CUR
        2 => (WAD_DATA.len() as i64).wrapping_add(offset), // SEEK_END
        _ => return -1,
    };

    if new_pos < 0 || new_pos as usize > WAD_DATA.len() {
        return -1;
    }

    WAD_CURSOR = new_pos as usize;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ftell(_fp: *mut c_void) -> i64 {
    WAD_CURSOR as i64
}

// Override fclose locally to be safe
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fclose(_fp: *mut c_void) -> i32 {
    0
}

// Override access to claim WAD exists
#[unsafe(no_mangle)]
pub unsafe extern "C" fn access(path: *const c_char, _mode: i32) -> i32 {
    if path.is_null() {
        return -1;
    }
    let path_slice = CStr::from_ptr(path).to_bytes();
    for window in path_slice.windows(3) {
        if window.eq_ignore_ascii_case(b"wad") {
            return 0;
        }
    }
    -1
}
