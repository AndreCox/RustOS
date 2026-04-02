#![no_std]
#![no_main]

use core::ffi::{CStr, c_char, c_int, c_void};
use core::sync::atomic::Ordering;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// =============================================================================
// DOOM MEMORY & CONFIG
// =============================================================================

unsafe extern "C" {
    pub static mut DG_ScreenBuffer: *mut u32;
}

#[unsafe(no_mangle)]
#[used]
pub static mut DG_Width: i32 = 640;

#[unsafe(no_mangle)]
#[used]
pub static mut DG_Height: i32 = 400;

// The WAD data embedded in the app
static WAD_DATA: &[u8] = include_bytes!("../../assets/data/DOOM.WAD");
static mut WAD_CURSOR: usize = 0;

// A dummy buffer to act as the FILE struct.
static mut DUMMY_FILE_STRUCT: [u8; 128] = [0; 128];

// =============================================================================
// SYSCALL WRAPPERS (these will call into the kernel)
// =============================================================================

fn print_char(c: u8) {
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") 1u64, // Syscall 1: print_char
            in("rdi") c as u64,
            options(nostack, preserves_flags)
        );
    }
}

fn println_str(s: &str) {
    for &b in s.as_bytes() {
        print_char(b);
    }
    print_char(b'\n');
}

// =============================================================================
// INITIALIZATION
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DG_Init() {
    println_str("[DOOM] DG_Init - Starting...");

    // TODO: Allocate a virtual framebuffer
    // For now, this is a stub that will need syscalls to access graphics
}

// =============================================================================
// VIDEO & INPUT
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DG_DrawFrame() {
    if DG_ScreenBuffer.is_null() {
        return;
    }
    // TODO: Mark the virtual framebuffer dirty
}

#[unsafe(no_mangle)]
pub extern "C" fn DG_SleepMs(ms: u32) {
    // TODO: Implement sleep via syscall
}

#[unsafe(no_mangle)]
pub extern "C" fn DG_GetTicksMs() -> u32 {
    // TODO: Implement ticks via syscall
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DG_GetKey(pressed: *mut i32, key: *mut u8) -> i32 {
    if pressed.is_null() || key.is_null() {
        return 0;
    }
    // TODO: Read from keyboard queue via syscall
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn DG_SetWindowTitle(_title: *const c_char) {}

// =============================================================================
// FILE SYSTEM
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fopen(path: *const c_char, mode: *const c_char) -> *mut c_void {
    if path.is_null() {
        return core::ptr::null_mut();
    }

    let path_slice = CStr::from_ptr(path).to_bytes();
    let mode_slice = CStr::from_ptr(mode).to_bytes();

    // Detect WAD Open Request
    let mut is_wad = false;
    for window in path_slice.windows(3) {
        if window.eq_ignore_ascii_case(b"wad") {
            is_wad = true;
            break;
        }
    }

    if is_wad && mode_slice.contains(&b'r') {
        WAD_CURSOR = 0;
        return core::ptr::addr_of_mut!(DUMMY_FILE_STRUCT) as *mut c_void;
    }

    if mode_slice.contains(&b'w') || mode_slice.contains(&b'a') || mode_slice.contains(&b'+') {
        return core::ptr::null_mut();
    }

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
        0 => offset,
        1 => (WAD_CURSOR as i64).wrapping_add(offset),
        2 => (WAD_DATA.len() as i64).wrapping_add(offset),
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fclose(_fp: *mut c_void) -> i32 {
    0
}

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

// =============================================================================
// ENTRY POINT (minimal - just for structure)
// =============================================================================

unsafe extern "C" {
    fn doomgeneric_Tick();
    fn doomgeneric_Create(argc: i32, argv: *const *const i8) -> i32;
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.start")]
pub extern "C" fn _start() -> ! {
    // For now, this is a stub. The actual doom game should be launched
    // from the kernel's task system.
    loop {}
}
