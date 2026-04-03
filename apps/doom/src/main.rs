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

const MALLOC_SIZE: usize = 16 * 1024 * 1024; // 16MB
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

fn print_str(s: &str) {
    for &b in s.as_bytes() {
        print_char(b);
    }
}

fn print_num(mut n: i64) {
    if n == 0 { print_char(b'0'); return; }
    if n < 0 { print_char(b'-'); n = -n; }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while n > 0 { buf[i] = (n % 10) as u8 + b'0'; n /= 10; i += 1; }
    while i > 0 { i -= 1; print_char(buf[i]); }
}

struct Dummy;
#[global_allocator]
static ALLOCATOR: Dummy = Dummy;
unsafe impl core::alloc::GlobalAlloc for Dummy {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 { core::ptr::null_mut() }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

// =============================================================================
// LIBC STUBS
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    unsafe {
        let base = core::ptr::addr_of_mut!(MALLOC_BUFFER) as usize;
        let aligned_size = (size + 15) & !15; // 16-byte alignment
        if MALLOC_PTR + aligned_size > MALLOC_SIZE {
            print_str("[DOOM] malloc failed! size: ");
            print_num(size as i64);
            print_char(b'\n');
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
    // This is a naive realloc for a bump allocator
    let new_ptr = unsafe { malloc(size) };
    if !new_ptr.is_null() {
        unsafe { core::ptr::copy_nonoverlapping(ptr as *const u8, new_ptr as *mut u8, 16); } // Dummy copy
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
    unsafe {
        print_str("[DOOM] fopen: ");
        let mut i = 0;
        while *path.add(i) != 0 { print_char(*path.add(i) as u8); i += 1; }
        
        let mut handle: u64 = u64::MAX;
        core::arch::asm!("int 0x80", in("rax") SYS_FS_OPEN, in("rdi") path as u64, lateout("rax") handle);
        if handle == u64::MAX {
            let mut fallback = [0u8; 128];
            let mut j = 0;
            let mut k = 0;
            while *path.add(j) != 0 && k < 127 {
                let c = *path.add(j) as u8;
                if c != b'.' && c != b'/' && c != b'\\' {
                    fallback[k] = c.to_ascii_uppercase();
                    k += 1;
                }
                j += 1;
            }
            fallback[k] = 0;
            core::arch::asm!("int 0x80", in("rax") SYS_FS_OPEN, in("rdi") fallback.as_ptr() as u64, lateout("rax") handle);
            
            if handle == u64::MAX {
                print_str(" -> FAILED\n");
                return core::ptr::null_mut();
            }
        }
        print_str(" -> SUCCESS (");
        print_num((handle + 1) as i64);
        print_str(")\n");
        (handle + 1) as *mut c_void
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fread(ptr: *mut c_void, size: usize, nmemb: usize, fp: *mut c_void) -> usize {
    if fp.is_null() { return 0; }
    let handle = (fp as u64) - 1;
    let mut read: u64 = 0;
    unsafe { core::arch::asm!("int 0x80", in("rax") SYS_FS_READ_HANDLE, in("rdi") handle, in("rsi") ptr as u64, in("rdx") (size * nmemb) as u64, lateout("rax") read); }
    if read == u64::MAX { return 0; }
    // Optional: log large reads if needed
    (read as usize) / size
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fgetc(fp: *mut c_void) -> i32 {
    let mut c: u8 = 0;
    if unsafe { fread(core::ptr::addr_of_mut!(c) as *mut c_void, 1, 1, fp) } == 1 {
        c as i32
    } else {
        -1 // EOF
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fgets(s: *mut i8, n: i32, fp: *mut c_void) -> *mut i8 {
    let mut i = 0;
    while i < n - 1 {
        let c = unsafe { fgetc(fp) };
        if c == -1 {
            if i == 0 { return core::ptr::null_mut(); }
            break;
        }
        unsafe { *s.add(i as usize) = c as i8; }
        i += 1;
        if c == b'\n' as i32 { break; }
    }
    unsafe { *s.add(i as usize) = 0; }
    s
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
pub unsafe extern "C" fn access(path: *const c_char, _mode: i32) -> i32 {
    unsafe {
        let fp = fopen(path, core::ptr::null());
        if fp.is_null() { -1 } else { fclose(fp); 0 }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn exit(_status: i32) -> ! {
    unsafe {
        print_str("[DOOM] Exit called with status: ");
        print_num(_status as i64);
        print_char(b'\n');
    }
    loop { unsafe { core::arch::asm!("int 0x80", in("rax") 2u64, in("rdi") _status as u64); } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sscanf(mut str: *const i8, mut fmt: *const i8, mut ap: ...) -> i32 {
    unsafe {
        let mut count = 0;
        loop {
            let f = *fmt as u8;
            if f == 0 { break; }
            if f.is_ascii_whitespace() {
                while (*str as u8).is_ascii_whitespace() { str = str.add(1); }
            } else if f == b'%' {
                fmt = fmt.add(1);
                match *fmt as u8 {
                    b's' => {
                        let dest = ap.arg::<*mut i8>();
                        while (*str as u8).is_ascii_whitespace() { str = str.add(1); }
                        let mut j = 0;
                        while *str != 0 && !(*str as u8).is_ascii_whitespace() {
                            *dest.add(j) = *str;
                            str = str.add(1);
                            j += 1;
                        }
                        *dest.add(j) = 0;
                        count += 1;
                    }
                    b'd' => {
                        let dest = ap.arg::<*mut i32>();
                        while (*str as u8).is_ascii_whitespace() { str = str.add(1); }
                        let mut val = 0;
                        let mut sign = 1;
                        if *str == b'-' as i8 { sign = -1; str = str.add(1); }
                        while *str >= b'0' as i8 && *str <= b'9' as i8 {
                            val = val * 10 + (*str - b'0' as i8) as i32;
                            str = str.add(1);
                        }
                        *dest = val * sign;
                        count += 1;
                    }
                    _ => break,
                }
            } else {
                if *str != *fmt { break; }
                str = str.add(1);
            }
            fmt = fmt.add(1);
        }
        count
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fscanf(fp: *mut c_void, mut fmt: *const i8, mut ap: ...) -> i32 {
    unsafe {
        let mut buf = [0i8; 512];
        if fgets(buf.as_mut_ptr(), 512, fp).is_null() { return -1; }
        
        print_str("[DOOM] fscanf read line: ");
        let mut i = 0;
        while buf[i] != 0 { print_char(buf[i] as u8); i += 1; }
        
        let mut str = buf.as_ptr();
        let mut count = 0;
        loop {
            let f = *fmt as u8;
            if f == 0 { break; }
            if f.is_ascii_whitespace() {
                while (*str as u8).is_ascii_whitespace() { str = str.add(1); }
            } else if f == b'%' {
                fmt = fmt.add(1);
                match *fmt as u8 {
                    b's' => {
                        let dest = ap.arg::<*mut i8>();
                        while (*str as u8).is_ascii_whitespace() { str = str.add(1); }
                        let mut j = 0;
                        while *str != 0 && !(*str as u8).is_ascii_whitespace() {
                            *dest.add(j) = *str;
                            str = str.add(1);
                            j += 1;
                        }
                        *dest.add(j) = 0;
                        count += 1;
                    }
                    b'd' => {
                        let dest = ap.arg::<*mut i32>();
                        while (*str as u8).is_ascii_whitespace() { str = str.add(1); }
                        let mut val = 0;
                        let mut sign = 1;
                        if *str == b'-' as i8 { sign = -1; str = str.add(1); }
                        while *str >= b'0' as i8 && *str <= b'9' as i8 {
                            val = val * 10 + (*str - b'0' as i8) as i32;
                            str = str.add(1);
                        }
                        *dest = val * sign;
                        count += 1;
                    }
                    _ => break,
                }
            } else {
                if *str != *fmt { break; }
                str = str.add(1);
            }
            fmt = fmt.add(1);
        }
        print_str(" -> matched: ");
        print_num(count as i64);
        print_char(b'\n');
        count
    }
}

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
pub unsafe extern "C" fn strstr(haystack: *const i8, needle: *const i8) -> *mut i8 {
    unsafe {
        let n_len = strlen(needle);
        if n_len == 0 { return haystack as *mut i8; }
        let mut h = haystack;
        while *h != 0 {
            if strncmp(h, needle, n_len) == 0 { return h as *mut i8; }
            h = h.add(1);
        }
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atoi(s: *const i8) -> i32 {
    unsafe {
        let mut res = 0;
        let mut i = 0;
        while (*s.add(i) as u8).is_ascii_whitespace() { i += 1; }
        let sign = if *s.add(i) == b'-' as i8 { i += 1; -1 } else { 1 };
        while *s.add(i) >= b'0' as i8 && *s.add(i) <= b'9' as i8 {
            res = res * 10 + (*s.add(i) - b'0' as i8) as i32;
            i += 1;
        }
        res * sign
    }
}

unsafe fn vsprintf_core(buf: *mut i8, n: usize, fmt: *const i8, mut ap: core::ffi::VaList) -> i32 {
    let mut fptr = fmt;
    let mut written = 0;
    
    macro_rules! write_char {
        ($c:expr) => {
            if (written as usize) < n.saturating_sub(1) {
                unsafe { *buf.add(written as usize) = $c as i8; }
                written += 1;
            }
        };
    }

    loop {
        let c = unsafe { *fptr as u8 };
        if c == 0 { break; }
        if c == b'%' {
            fptr = unsafe { fptr.add(1) };
            match unsafe { *fptr as u8 } {
                b's' => {
                    let s = unsafe { ap.arg::<*const i8>() };
                    if !s.is_null() {
                        let mut j = 0;
                        while unsafe { *s.add(j) } != 0 { write_char!(unsafe { *s.add(j) as u8 }); j += 1; }
                    } else {
                        for b in b"(null)".iter() { write_char!(*b); }
                    }
                }
                b'd' | b'i' => {
                    let mut val = unsafe { ap.arg::<i32>() } as i64;
                    if val < 0 { write_char!(b'-'); val = -val; }
                    let mut tmp = [0u8; 20]; let mut ti = 0;
                    if val == 0 { tmp[ti] = b'0'; ti += 1; }
                    while val > 0 { tmp[ti] = (val % 10) as u8 + b'0'; val /= 10; ti += 1; }
                    for j in (0..ti).rev() { write_char!(tmp[j]); }
                }
                b'u' => {
                    let mut val = unsafe { ap.arg::<u32>() as u64 };
                    let mut tmp = [0u8; 20]; let mut ti = 0;
                    if val == 0 { tmp[ti] = b'0'; ti += 1; }
                    while val > 0 { tmp[ti] = (val % 10) as u8 + b'0'; val /= 10; ti += 1; }
                    for j in (0..ti).rev() { write_char!(tmp[j]); }
                }
                b'x' | b'p' => {
                    let val = if unsafe { *fptr == b'p' as i8 } { unsafe { ap.arg::<u64>() } } else { unsafe { ap.arg::<u32>() as u64 } };
                    let mut started = false;
                    for j in 0..16 {
                        let hex = (val >> (60 - j * 4)) & 0xF;
                        if hex != 0 || started || j == 15 {
                            started = true;
                            let b = if hex < 10 { hex as u8 + b'0' } else { hex as u8 - 10 + b'a' };
                            write_char!(b);
                        }
                    }
                }
                _ => { write_char!(b'%'); write_char!(unsafe { *fptr as u8 }); }
            }
        } else {
            write_char!(c);
        }
        fptr = unsafe { fptr.add(1) };
    }
    if n > 0 { unsafe { *buf.add(written as usize) = 0; } }
    written
}

unsafe fn vprintf_core(fmt: *const i8, mut ap: core::ffi::VaList) {
    let mut fptr = fmt;
    loop {
        let c = unsafe { *fptr as u8 };
        if c == 0 { break; }
        if c == b'%' {
            fptr = unsafe { fptr.add(1) };
            match unsafe { *fptr as u8 } {
                b's' => {
                    let s = unsafe { ap.arg::<*const i8>() };
                    if !s.is_null() {
                        let mut j = 0;
                        while unsafe { *s.add(j) } != 0 { print_char(unsafe { *s.add(j) as u8 }); j += 1; }
                    } else {
                        print_str("(null)");
                    }
                }
                b'd' | b'i' => { print_num(unsafe { ap.arg::<i32>() } as i64); }
                b'u' => { print_num(unsafe { ap.arg::<u32>() } as i64); }
                b'x' | b'p' => {
                    let n = if unsafe { *fptr == b'p' as i8 } { unsafe { ap.arg::<u64>() } } else { unsafe { ap.arg::<u32>() as u64 } };
                    let mut started = false;
                    for j in 0..16 {
                        let hex = (n >> (60 - j * 4)) & 0xF;
                        if hex != 0 || started || j == 15 {
                            started = true;
                            let b = if hex < 10 { hex as u8 + b'0' } else { hex as u8 - 10 + b'a' };
                            print_char(b);
                        }
                    }
                }
                _ => { print_char(b'%'); print_char(unsafe { *fptr as u8 }); }
            }
        } else {
            print_char(c);
        }
        fptr = unsafe { fptr.add(1) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn printf(fmt: *const i8, mut ap: ...) -> i32 {
    unsafe { vprintf_core(fmt, ap); 0 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf(_s: *mut c_void, fmt: *const i8, ap: core::ffi::VaList) -> i32 {
    unsafe { vprintf_core(fmt, ap); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsprintf(s: *mut i8, fmt: *const i8, ap: core::ffi::VaList) -> i32 {
    unsafe { vsnprintf(s, 1024, fmt, ap) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fprintf(_s: *mut c_void, fmt: *const i8, mut ap: ...) -> i32 {
    unsafe { vprintf_core(fmt, ap); 0 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn snprintf(s: *mut i8, n: usize, fmt: *const i8, mut ap: ...) -> i32 {
    unsafe { vsnprintf(s, n, fmt, ap) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsnprintf(s: *mut i8, n: usize, fmt: *const i8, ap: core::ffi::VaList) -> i32 {
    unsafe { vsprintf_core(s, n, fmt, ap) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn putchar(c: i32) -> i32 {
    print_char(c as u8);
    c
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atof(s: *const i8) -> f64 {
    unsafe { atoi(s) as f64 }
}

#[unsafe(no_mangle)]
pub extern "C" fn isprint(c: i32) -> i32 {
    if c >= 32 && c <= 126 { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strchr(s: *const i8, c: i32) -> *mut i8 {
    let mut i = 0;
    unsafe {
        loop {
            let ch = *s.add(i);
            if ch as i32 == c { return s.add(i) as *mut i8; }
            if ch == 0 { return core::ptr::null_mut(); }
            i += 1;
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn puts(s: *const i8) -> i32 {
    unsafe {
        let mut i = 0;
        while *s.add(i) != 0 { print_char(*s.add(i) as u8); i += 1; }
        print_char(b'\n');
    }
    0
}
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
        let argv = [
            "doom\0".as_ptr() as *const i8,
            "DOOM.WAD\0".as_ptr() as *const i8,
            core::ptr::null()
        ];
        doomgeneric_Create(2, argv.as_ptr());
        loop { doomgeneric_Tick(); }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }
