#![no_std]
#![no_main]
#![feature(c_variadic)]

use core::ffi::{c_char, c_void};

// =============================================================================
// DOOM MEMORY & CONFIG
// =============================================================================

unsafe extern "C" {
    pub static mut DG_ScreenBuffer: *mut u32;
    fn doomgeneric_Create(argc: i32, argv: *const *const i8);
    fn doomgeneric_Tick();
}

#[unsafe(no_mangle)]
pub static mut DG_Width: i32 = 640;

#[unsafe(no_mangle)]
pub static mut DG_Height: i32 = 400;

static mut FRAMEBUFFER: [u32; 320 * 200] = [0; 320 * 200];

const MALLOC_SIZE: usize = 1024 * 1024; // 1MB for generic allocations
static mut MALLOC_BUFFER: [u8; MALLOC_SIZE] = [0; MALLOC_SIZE];
static mut MALLOC_PTR: usize = 0;

// =============================================================================
// SYSCALL WRAPPERS
// =============================================================================

const SYS_PRINT_CHAR: u64 = 1;
const SYS_DRAW_BUFFER: u64 = 10;
const SYS_GET_UPTIME: u64 = 11;
const SYS_FS_OPEN: u64 = 12;
const SYS_FS_READ_HANDLE: u64 = 13;
const SYS_FS_SEEK_HANDLE: u64 = 14;
const SYS_FS_CLOSE: u64 = 15;
const SYS_GET_KEY: u64 = 9;
const SYS_EXIT: u64 = 2;

fn print_char(c: u8) {
    unsafe {
        core::arch::asm!("int 0x80", in("rax") SYS_PRINT_CHAR, in("rdi") c as u64);
    }
}

// =============================================================================
// LIBC STUBS
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    unsafe {
        let base = core::ptr::addr_of_mut!(MALLOC_BUFFER) as usize;
        let aligned_size = (size + 7) & !7;
        if MALLOC_PTR + aligned_size > MALLOC_SIZE {
            return core::ptr::null_mut();
        }
        let current = base + MALLOC_PTR;
        MALLOC_PTR += aligned_size;
        current as *mut c_void
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() { return unsafe { malloc(size) }; }
    let new_ptr = unsafe { malloc(size) };
    if !new_ptr.is_null() {
        unsafe { core::ptr::copy_nonoverlapping(ptr as *const u8, new_ptr as *mut u8, 8); }
    }
    new_ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut c_void {
    let total = nmemb * size;
    let ptr = unsafe { malloc(total) };
    if !ptr.is_null() { unsafe { core::ptr::write_bytes(ptr, 0, total); } }
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(_ptr: *mut c_void) {}

#[unsafe(no_mangle)]
pub extern "C" fn fabs(x: f64) -> f64 { if x < 0.0 { -x } else { x } }

#[unsafe(no_mangle)]
pub extern "C" fn abs(n: i32) -> i32 { if n < 0 { -n } else { n } }

#[unsafe(no_mangle)]
pub static mut stdout: *mut c_void = core::ptr::null_mut();
#[unsafe(no_mangle)]
pub static mut stderr: *mut c_void = core::ptr::null_mut();
#[unsafe(no_mangle)]
pub static mut errno: i32 = 0;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fopen(path: *const c_char, _mode: *const c_char) -> *mut c_void {
    let mut handle: u64 = u64::MAX;
    unsafe { core::arch::asm!("int 0x80", in("rax") SYS_FS_OPEN, in("rdi") path as u64, lateout("rax") handle); }
    if handle == u64::MAX { core::ptr::null_mut() } else { (handle + 1) as *mut c_void }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fread(ptr: *mut c_void, size: usize, nmemb: usize, fp: *mut c_void) -> usize {
    if fp.is_null() { return 0; }
    let handle = (fp as u64) - 1;
    let mut read: u64 = 0;
    unsafe { core::arch::asm!("int 0x80", in("rax") SYS_FS_READ_HANDLE, in("rdi") handle, in("rsi") ptr as u64, in("rdx") (size * nmemb) as u64, lateout("rax") read); }
    if read == u64::MAX { 0 } else { (read as usize) / size }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fseek(fp: *mut c_void, offset: i64, whence: i32) -> i32 {
    if fp.is_null() { return -1; }
    let handle = (fp as u64) - 1;
    let mut res: u64 = 0;
    unsafe { core::arch::asm!("int 0x80", in("rax") SYS_FS_SEEK_HANDLE, in("rdi") handle, in("rsi") offset as u64, in("rdx") whence as u64, lateout("rax") res); }
    if res == u64::MAX { -1 } else { 0 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ftell(fp: *mut c_void) -> i64 {
    if fp.is_null() { return -1; }
    let handle = (fp as u64) - 1;
    let mut res: u64 = 0;
    unsafe { core::arch::asm!("int 0x80", in("rax") SYS_FS_SEEK_HANDLE, in("rdi") handle, in("rsi") 0u64, in("rdx") 1u64, lateout("rax") res); }
    res as i64
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fclose(fp: *mut c_void) -> i32 {
    if fp.is_null() { return -1; }
    let handle = (fp as u64) - 1;
    unsafe { core::arch::asm!("int 0x80", in("rax") SYS_FS_CLOSE, in("rdi") handle); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fwrite(_ptr: *const c_void, _size: usize, _nmemb: usize, _stream: *mut c_void) -> usize { 0 }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fflush(_stream: *mut c_void) -> i32 { 0 }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn remove(_path: *const i8) -> i32 { 0 }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rename(_old: *const i8, _new: *const i8) -> i32 { 0 }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mkdir(_path: *const i8, _mode: u32) -> i32 { 0 }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn access(path: *const c_char, mode: i32) -> i32 {
    unsafe {
        let fp = fopen(path, core::ptr::null());
        if fp.is_null() { -1 } else { fclose(fp); 0 }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn exit(_status: i32) -> ! {
    loop { unsafe { core::arch::asm!("int 0x80", in("rax") SYS_EXIT, in("rdi") _status as u64); } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sscanf(_str: *const i8, _format: *const i8, _: ...) -> i32 { 0 }

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strdup(s: *const i8) -> *mut i8 {
    unsafe {
        let len = strlen(s);
        let ptr = malloc(len + 1) as *mut i8;
        if !ptr.is_null() {
            core::ptr::copy_nonoverlapping(s as *const u8, ptr as *mut u8, len + 1);
        }
        ptr
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strstr(_haystack: *const i8, _needle: *const i8) -> *mut i8 {
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn putchar(c: i32) -> i32 {
    print_char(c as u8);
    c
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atoi(s: *const i8) -> i32 {
    unsafe {
        let mut res = 0;
        let mut i = 0;
        while *s.add(i) == b' ' as i8 { i += 1; }
        let sign = if *s.add(i) == b'-' as i8 { i += 1; -1 } else { 1 };
        while *s.add(i) >= b'0' as i8 && *s.add(i) <= b'9' as i8 {
            res = res * 10 + (*s.add(i) - b'0' as i8) as i32;
            i += 1;
        }
        res * sign
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn printf(_fmt: *const i8, _: ...) -> i32 { 0 }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fprintf(_s: *mut c_void, _fmt: *const i8, _: ...) -> i32 { 0 }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn snprintf(s: *mut i8, n: usize, fmt: *const i8, _: ...) -> i32 {
    unsafe {
        let mut i = 0;
        while i < n.saturating_sub(1) && *fmt.add(i) != 0 { *s.add(i) = *fmt.add(i); i += 1; }
        if n > 0 { *s.add(i) = 0; }
        i as i32
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsnprintf(s: *mut i8, n: usize, fmt: *const i8, _ap: *mut c_void) -> i32 {
    unsafe {
        let mut i = 0;
        while i < n.saturating_sub(1) && *fmt.add(i) != 0 { *s.add(i) = *fmt.add(i); i += 1; }
        if n > 0 { *s.add(i) = 0; }
        i as i32
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf(_s: *mut c_void, _f: *const i8, _a: *mut c_void) -> i32 { 0 }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn puts(_s: *const i8) -> i32 { 0 }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn system(_c: *const i8) -> i32 { 0 }

#[unsafe(no_mangle)]
pub extern "C" fn strlen(s: *const i8) -> usize {
    let mut len = 0;
    unsafe { while *s.add(len) != 0 { len += 1; } }
    len
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcmp(s1: *const i8, s2: *const i8) -> i32 {
    unsafe {
        let mut i = 0;
        loop {
            let c1 = *s1.add(i) as u8; let c2 = *s2.add(i) as u8;
            if c1 != c2 || c1 == 0 { return (c1 as i32) - (c2 as i32); }
            i += 1;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncpy(d: *mut i8, s: *const i8, n: usize) -> *mut i8 {
    unsafe {
        let mut i = 0;
        while i < n && *s.add(i) != 0 { *d.add(i) = *s.add(i); i += 1; }
        while i < n { *d.add(i) = 0; i += 1; }
        d
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strchr(s: *const i8, c: i32) -> *mut i8 {
    unsafe {
        let mut i = 0;
        while *s.add(i) != 0 { if *s.add(i) == c as i8 { return s.add(i) as *mut i8; } i += 1; }
        if c == 0 { s.add(i) as *mut i8 } else { core::ptr::null_mut() }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strrchr(s: *const i8, c: i32) -> *mut i8 {
    unsafe {
        let mut last = core::ptr::null_mut();
        let mut i = 0;
        while *s.add(i) != 0 { if *s.add(i) == c as i8 { last = s.add(i) as *mut i8; } i += 1; }
        if c == 0 { s.add(i) as *mut i8 } else { last }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncmp(s1: *const i8, s2: *const i8, n: usize) -> i32 {
    unsafe {
        for i in 0..n {
            let c1 = *s1.add(i) as u8; let c2 = *s2.add(i) as u8;
            if c1 != c2 || c1 == 0 { return (c1 as i32) - (c2 as i32); }
        }
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcasecmp(s1: *const i8, s2: *const i8) -> i32 {
    unsafe {
        let mut i = 0;
        loop {
            let c1 = (*s1.add(i) as u8).to_ascii_lowercase();
            let c2 = (*s2.add(i) as u8).to_ascii_lowercase();
            if c1 != c2 || c1 == 0 { return (c1 as i32) - (c2 as i32); }
            i += 1;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncasecmp(s1: *const i8, s2: *const i8, n: usize) -> i32 {
    unsafe {
        for i in 0..n {
            let c1 = (*s1.add(i) as u8).to_ascii_lowercase();
            let c2 = (*s2.add(i) as u8).to_ascii_lowercase();
            if c1 != c2 || c1 == 0 { return (c1 as i32) - (c2 as i32); }
        }
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn toupper(c: i32) -> i32 { (c as u8).to_ascii_uppercase() as i32 }
#[unsafe(no_mangle)]
pub extern "C" fn tolower(c: i32) -> i32 { (c as u8).to_ascii_lowercase() as i32 }
#[unsafe(no_mangle)]
pub extern "C" fn isspace(c: i32) -> i32 {
    if c == b' ' as i32 || c == b'\t' as i32 || c == b'\n' as i32 || c == b'\r' as i32 { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(d: *mut u8, s: *const u8, n: usize) -> *mut u8 { unsafe { core::ptr::copy(s, d, n); d } }

// =============================================================================
// DOOMGENERIC INTERFACE
// =============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn DG_Init() {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DG_DrawFrame() {
    unsafe {
        if DG_ScreenBuffer.is_null() { return; }
        let arg2 = (200u64 << 32) | 320u64;
        core::arch::asm!("int 0x80", in("rax") SYS_DRAW_BUFFER, in("rdi") DG_ScreenBuffer as u64, in("rsi") arg2);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn DG_SleepMs(ms: u32) { for _ in 0..(ms * 1000) { unsafe { core::arch::asm!("pause"); } } }

#[unsafe(no_mangle)]
pub extern "C" fn DG_GetTicksMs() -> u32 {
    let mut res: u64 = 0;
    unsafe { core::arch::asm!("int 0x80", in("rax") SYS_GET_UPTIME, lateout("rax") res); }
    res as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DG_GetKey(p: *mut i32, k: *mut u8) -> i32 {
    unsafe {
        let mut v: u64 = 0;
        core::arch::asm!("int 0x80", in("rax") SYS_GET_KEY, lateout("rax") v);
        if v == 0 { 0 } else { *p = 1; *k = v as u8; 1 }
    }
}

#[unsafe(no_mangle)] pub extern "C" fn DG_SetWindowTitle(_t: *const c_char) {}
#[unsafe(no_mangle)] pub extern "C" fn DG_BeginFrame() {}
#[unsafe(no_mangle)] pub extern "C" fn DG_EndFrame() {}

// =============================================================================
// ENTRY POINT
// =============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    unsafe {
        DG_ScreenBuffer = core::ptr::addr_of_mut!(FRAMEBUFFER) as *mut u32;
        let argv = ["doom\0".as_ptr() as *const i8, core::ptr::null()];
        doomgeneric_Create(1, argv.as_ptr());
        loop { doomgeneric_Tick(); }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }
