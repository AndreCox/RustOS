use crate::println;
use core::ffi::{CStr, c_char, c_int, c_void};

static WAD_DATA: &[u8] = include_bytes!("../assets/data/DOOM.WAD");
static mut WAD_CURSOR: usize = 0;

#[unsafe(no_mangle)]
pub extern "C" fn DG_Init() {
    println!("[DOOM] DG_Init");
}

#[unsafe(no_mangle)]
pub extern "C" fn DG_DrawFrame() {
    // We'll hook this to your compositor soon!
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
pub extern "C" fn DG_GetKey(_pressed: *mut i32, _key: *mut i32) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn DG_SetWindowTitle(_title: *const c_char) {}

// --- 4. Fake File System ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fopen(path: *const c_char, mode: *const c_char) -> *mut c_void {
    if path.is_null() {
        return core::ptr::null_mut();
    }

    let path_str = CStr::from_ptr(path).to_bytes();
    let mode_str = CStr::from_ptr(mode).to_bytes();

    // LOG EVERYTHING
    crate::println!(
        "[FS] fopen: {:?} mode {:?}",
        core::str::from_utf8(path_str).unwrap_or("???"),
        core::str::from_utf8(mode_str).unwrap_or("???")
    );

    // Check if it's a WAD (case-insensitive)
    let mut is_wad = false;
    for window in path_str.windows(3) {
        if window.eq_ignore_ascii_case(b"wad") {
            is_wad = true;
            break;
        }
    }

    if is_wad {
        if mode_str.contains(&b'r') {
            crate::println!("[FS] ✓ Providing WAD handle");
            WAD_CURSOR = 0;
            return 0x1234 as *mut c_void;
        }
    }

    // If DOOM is trying to WRITE (mode "w") a config or save
    if mode_str.contains(&b'w') {
        crate::println!("[FS] ✓ Providing write sink");
        return 0x5678 as *mut c_void;
    }

    // For reading config files, return NULL
    crate::println!("[FS] ✗ File not found, returning NULL");
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

    // Use saturating_sub to prevent underflow if cursor is somehow broken
    let available = WAD_DATA.len().saturating_sub(WAD_CURSOR);
    let actual = if to_read > available {
        available
    } else {
        to_read
    };

    if actual > 0 {
        // Only copy if we are within the bounds of WAD_DATA
        core::ptr::copy_nonoverlapping(WAD_DATA.as_ptr().add(WAD_CURSOR), ptr as *mut u8, actual);
        WAD_CURSOR += actual;
    }

    // Return number of elements successfully read
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

// Stub for access() if DOOM uses it to check file existence before opening
#[unsafe(no_mangle)]
pub unsafe extern "C" fn access(path: *const c_char, _mode: i32) -> i32 {
    if path.is_null() {
        return -1;
    }
    let path_str = CStr::from_ptr(path).to_bytes();

    // Only claim the WAD exists
    for window in path_str.windows(3) {
        if window.eq_ignore_ascii_case(b"wad") {
            return 0; // Success
        }
    }

    -1 // File not found
}
