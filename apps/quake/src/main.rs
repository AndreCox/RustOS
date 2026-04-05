#![no_std]
#![no_main]
#![feature(c_variadic)]

use core::ffi::{VaList, c_char, c_void};
use core::panic::PanicInfo;
use rustos_user::{
    SYS_DRAW_BUFFER, SYS_ENTER_EXCLUSIVE_GRAPHICS, SYS_EXIT, SYS_EXIT_EXCLUSIVE_GRAPHICS,
    SYS_FS_CLOSE, SYS_FS_OPEN, SYS_FS_READ_HANDLE, SYS_FS_SEEK_HANDLE, SYS_GET_KEY,
    SYS_GET_SCANCODE, SYS_GET_UPTIME, SYS_YIELD,
    exit as user_exit,
    print_char, print_str, syscall0, syscall1, syscall2, syscall3,
};

unsafe extern "C" {
    fn main(argc: i32, argv: *const *const i8) -> i32;
}

#[repr(C)]
#[derive(Copy, Clone)]
struct KernelFile {
    used: bool,
    handle: u64,
}

const MAX_FILES: usize = 32;
const MAX_PATH_TMP: usize = 512;
static mut FILES: [KernelFile; MAX_FILES] = [KernelFile {
    used: false,
    handle: 0,
}; MAX_FILES];
static mut RAND_STATE: u32 = 0x1234_abcd;
static mut ERRNO: i32 = 0;

const MALLOC_SIZE: usize = 128 * 1024 * 1024;
static mut MALLOC_BUFFER: [u8; MALLOC_SIZE] = [0; MALLOC_SIZE];
static mut MALLOC_PTR: usize = 0;

#[unsafe(no_mangle)]
pub static mut stdout: *mut c_void = core::ptr::null_mut();

#[unsafe(no_mangle)]
pub static mut vid_menudrawfn: Option<unsafe extern "C" fn()> = None;

#[unsafe(no_mangle)]
pub static mut vid_menukeyfn: Option<unsafe extern "C" fn(i32)> = None;

#[inline]
unsafe fn c_strlen(s: *const c_char) -> usize {
    let mut n = 0usize;
    while unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

#[inline]
fn ascii_upper(b: u8) -> u8 {
    if b.is_ascii_lowercase() { b - 32 } else { b }
}

unsafe fn copy_path_normalized(src: *const c_char, dst: &mut [u8; MAX_PATH_TMP]) -> usize {
    let mut i = 0usize;
    while i + 1 < dst.len() {
        let c = unsafe { *src.add(i) as u8 };
        if c == 0 {
            break;
        }
        dst[i] = if c == b'\\' { b'/' } else { c };
        i += 1;
    }
    dst[i] = 0;
    i
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    unsafe { c_strlen(s) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    let mut i = 0usize;
    loop {
        let c = unsafe { *src.add(i) };
        unsafe { *dst.add(i) = c };
        if c == 0 {
            break;
        }
        i += 1;
    }
    dst
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
    let mut i = 0usize;
    while i < n {
        let c = unsafe { *src.add(i) };
        unsafe { *dst.add(i) = c };
        i += 1;
        if c == 0 {
            break;
        }
    }
    while i < n {
        unsafe { *dst.add(i) = 0 };
        i += 1;
    }
    dst
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcmp(a: *const c_char, b: *const c_char) -> i32 {
    let mut i = 0usize;
    loop {
        let ca = unsafe { *a.add(i) as u8 };
        let cb = unsafe { *b.add(i) as u8 };
        if ca != cb || ca == 0 {
            return ca as i32 - cb as i32;
        }
        i += 1;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncmp(a: *const c_char, b: *const c_char, n: usize) -> i32 {
    let mut i = 0usize;
    while i < n {
        let ca = unsafe { *a.add(i) as u8 };
        let cb = unsafe { *b.add(i) as u8 };
        if ca != cb || ca == 0 {
            return ca as i32 - cb as i32;
        }
        i += 1;
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcat(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    let mut dlen = 0usize;
    while unsafe { *dst.add(dlen) } != 0 {
        dlen += 1;
    }
    let mut i = 0usize;
    loop {
        let c = unsafe { *src.add(i) };
        unsafe { *dst.add(dlen + i) = c };
        if c == 0 {
            break;
        }
        i += 1;
    }
    dst
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fopen(path: *const c_char, _mode: *const c_char) -> *mut c_void {
    if path.is_null() {
        return core::ptr::null_mut();
    }

    let mut handle = syscall1(SYS_FS_OPEN, path as u64);

    // Retry with canonicalized paths to tolerate C-side relative paths and FAT case semantics.
    if handle == u64::MAX {
        let mut norm = [0u8; MAX_PATH_TMP];
        let len = unsafe { copy_path_normalized(path, &mut norm) };

        if len > 0 {
            let mut start = 0usize;
            while start + 1 < len && norm[start] == b'.' && norm[start + 1] == b'/' {
                start += 2;
            }

            // Variant A: relative path (no leading '/'), eg: id1/pak0.pak
            let mut rel = [0u8; MAX_PATH_TMP];
            let mut rw = 0usize;
            while start < len && rw + 1 < rel.len() {
                rel[rw] = norm[start];
                rw += 1;
                start += 1;
            }
            rel[rw] = 0;

            if rw > 0 {
                handle = syscall1(SYS_FS_OPEN, rel.as_ptr() as u64);

                if handle == u64::MAX {
                    for b in &mut rel[..rw] {
                        *b = ascii_upper(*b);
                    }
                    handle = syscall1(SYS_FS_OPEN, rel.as_ptr() as u64);
                }
            }

            // Variant B: absolute path (leading '/'), eg: /id1/pak0.pak
            let mut abs = [0u8; MAX_PATH_TMP];
            let mut w = 0usize;
            start = 0;
            while start + 1 < len && norm[start] == b'.' && norm[start + 1] == b'/' {
                start += 2;
            }
            if start < len && norm[start] != b'/' {
                abs[w] = b'/';
                w += 1;
            }
            while start < len && w + 1 < abs.len() {
                abs[w] = norm[start];
                w += 1;
                start += 1;
            }
            abs[w] = 0;

            if handle == u64::MAX {
                handle = syscall1(SYS_FS_OPEN, abs.as_ptr() as u64);
            }

            if handle == u64::MAX {
                for b in &mut abs[..w] {
                    *b = ascii_upper(*b);
                }
                handle = syscall1(SYS_FS_OPEN, abs.as_ptr() as u64);
            }
        }
    }

    if handle == u64::MAX {
        return core::ptr::null_mut();
    }

    let mut idx = None;
    for i in 0..MAX_FILES {
        // SAFETY: single-threaded userland tasks in this kernel model
        if unsafe { !FILES[i].used } {
            idx = Some(i);
            break;
        }
    }
    let Some(i) = idx else {
        return core::ptr::null_mut();
    };

    unsafe {
        FILES[i].used = true;
        FILES[i].handle = handle;
        &mut FILES[i] as *mut KernelFile as *mut c_void
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn quake_enter_graphics() {
    let _ = syscall0(SYS_ENTER_EXCLUSIVE_GRAPHICS);
}

#[unsafe(no_mangle)]
pub extern "C" fn quake_exit_graphics() {
    let _ = syscall0(SYS_EXIT_EXCLUSIVE_GRAPHICS);
}

#[unsafe(no_mangle)]
pub extern "C" fn quake_draw_buffer(pixels: *const u32, width: u32, height: u32) {
    if pixels.is_null() || width == 0 || height == 0 {
        return;
    }
    let packed_dims = (width as u64) | ((height as u64) << 32);
    let _ = syscall2(SYS_DRAW_BUFFER, pixels as u64, packed_dims);
}

#[unsafe(no_mangle)]
pub extern "C" fn quake_poll_key() -> i32 {
    syscall0(SYS_GET_KEY) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn quake_poll_scancode() -> i32 {
    syscall0(SYS_GET_SCANCODE) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn quake_yield() {
    let _ = syscall0(SYS_YIELD);
}

#[unsafe(no_mangle)]
pub extern "C" fn quake_uptime_ms() -> u64 {
    syscall0(SYS_GET_UPTIME)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fclose(fp: *mut c_void) -> i32 {
    if fp.is_null() {
        return -1;
    }
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    let _ = syscall1(SYS_FS_CLOSE, file.handle);
    file.used = false;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fread(
    ptr: *mut c_void,
    size: usize,
    nmemb: usize,
    fp: *mut c_void,
) -> usize {
    if ptr.is_null() || fp.is_null() || size == 0 || nmemb == 0 {
        return 0;
    }
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    let bytes = size.saturating_mul(nmemb);
    if bytes > 16 * 1024 * 1024 {
        print_str("[quake] fread size exceeds 16MB, returning 0 to prevent overflow\n");
        return 0;
    }
    let n = syscall3(SYS_FS_READ_HANDLE, file.handle, ptr as u64, bytes as u64);
    if n == u64::MAX {
        0
    } else {
        (n as usize) / size
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fwrite(
    _ptr: *const c_void,
    _size: usize,
    _nmemb: usize,
    _fp: *mut c_void,
) -> usize {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fseek(fp: *mut c_void, offset: i64, whence: i32) -> i32 {
    if fp.is_null() {
        return -1;
    }
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    let res = syscall3(
        SYS_FS_SEEK_HANDLE,
        file.handle,
        offset as u64,
        whence as u64,
    );
    if res == u64::MAX { -1 } else { 0 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ftell(fp: *mut c_void) -> i64 {
    if fp.is_null() {
        return -1;
    }
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    let res = syscall3(SYS_FS_SEEK_HANDLE, file.handle, 0, 1);
    if res == u64::MAX { -1 } else { res as i64 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn open(_path: *const c_char, _flags: i32, _mode: i32) -> i32 {
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn write(_fd: i32, _buf: *const c_void, _count: usize) -> isize {
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn close(_fd: i32) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn unlink(_path: *const c_char) -> i32 {
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fflush(_fp: *mut c_void) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn feof(_fp: *mut c_void) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ungetc(_c: i32, _stream: *mut c_void) -> i32 {
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn getc(stream: *mut c_void) -> i32 {
    fgetc(stream)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fgetc(stream: *mut c_void) -> i32 {
    let mut c: u8 = 0;
    let n = fread(&mut c as *mut u8 as *mut c_void, 1, 1, stream);
    if n == 0 {
        -1
    } else {
        c as i32
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __errno_location() -> *mut i32 {
    &raw mut ERRNO
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strerror(_err: i32) -> *const c_char {
    c"error".as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn abs(n: i32) -> i32 {
    if n < 0 { -n } else { n }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    let size = size.max(1);
    let aligned_ptr = (unsafe { MALLOC_PTR } + 15) & !15;
    if aligned_ptr.saturating_add(size) > MALLOC_SIZE {
        print_str("[quake] OOM in malloc\n");
        return core::ptr::null_mut();
    }
    let p = unsafe { (core::ptr::addr_of_mut!(MALLOC_BUFFER) as *mut u8).add(aligned_ptr) };
    unsafe {
        MALLOC_PTR = aligned_ptr + size;
    }
    p as *mut c_void
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut c_void {
    let total = nmemb.saturating_mul(size);
    let p = unsafe { malloc(total) };
    if !p.is_null() {
        unsafe { core::ptr::write_bytes(p, 0, total) };
    }
    p
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() {
        return unsafe { malloc(size) };
    }
    let new_ptr = unsafe { malloc(size) };
    if !new_ptr.is_null() {
        unsafe {
            core::ptr::copy_nonoverlapping(ptr as *const u8, new_ptr as *mut u8, size);
        }
    }
    new_ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(_ptr: *mut c_void) {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strchr(s: *const c_char, c: i32) -> *mut c_char {
    let target = c as u8;
    let mut i = 0usize;
    loop {
        let ch = unsafe { *s.add(i) as u8 };
        if ch == target {
            return unsafe { s.add(i) as *mut c_char };
        }
        if ch == 0 {
            return core::ptr::null_mut();
        }
        i += 1;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strstr(haystack: *const c_char, needle: *const c_char) -> *mut c_char {
    let nlen = unsafe { c_strlen(needle) };
    if nlen == 0 {
        return haystack as *mut c_char;
    }
    let mut i = 0usize;
    while unsafe { *haystack.add(i) } != 0 {
        if unsafe { strncmp(haystack.add(i), needle, nlen) } == 0 {
            return unsafe { haystack.add(i) as *mut c_char };
        }
        i += 1;
    }
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strtol(nptr: *const c_char, endptr: *mut *mut c_char, _base: i32) -> i64 {
    let mut s = nptr;
    while unsafe { *s == b' ' as i8 || *s == b'\t' as i8 || *s == b'\n' as i8 } {
        s = unsafe { s.add(1) };
    }
    let mut neg = false;
    if unsafe { *s } == b'-' as i8 {
        neg = true;
        s = unsafe { s.add(1) };
    }
    let mut out: i64 = 0;
    while unsafe { *s >= b'0' as i8 && *s <= b'9' as i8 } {
        out = out * 10 + (unsafe { *s } - b'0' as i8) as i64;
        s = unsafe { s.add(1) };
    }
    if !endptr.is_null() {
        unsafe { *endptr = s as *mut c_char };
    }
    if neg { -out } else { out }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strtod(nptr: *const c_char, endptr: *mut *mut c_char) -> f64 {
    let mut s = nptr;
    while unsafe { *s == b' ' as i8 || *s == b'\t' as i8 || *s == b'\n' as i8 || *s == b'\r' as i8 } {
        s = unsafe { s.add(1) };
    }
    let mut neg = false;
    if unsafe { *s } == b'-' as i8 {
        neg = true;
        s = unsafe { s.add(1) };
    } else if unsafe { *s } == b'+' as i8 {
        s = unsafe { s.add(1) };
    }
    let mut val: f64 = 0.0;
    while unsafe { *s >= b'0' as i8 && *s <= b'9' as i8 } {
        val = val * 10.0 + (unsafe { *s } - b'0' as i8) as f64;
        s = unsafe { s.add(1) };
    }
    if unsafe { *s } == b'.' as i8 {
        s = unsafe { s.add(1) };
        let mut frac = 0.1;
        while unsafe { *s >= b'0' as i8 && *s <= b'9' as i8 } {
            val += (unsafe { *s } - b'0' as i8) as f64 * frac;
            frac /= 10.0;
            s = unsafe { s.add(1) };
        }
    }
    if !endptr.is_null() {
        unsafe { *endptr = s as *mut c_char };
    }
    if neg { -val } else { val }
}

#[unsafe(no_mangle)]
pub extern "C" fn rand() -> i32 {
    unsafe {
        RAND_STATE = RAND_STATE.wrapping_mul(1103515245).wrapping_add(12345);
        ((RAND_STATE >> 16) & 0x7fff) as i32
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn setjmp(_env: *mut c_void) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn _setjmp(env: *mut c_void) -> i32 {
    setjmp(env)
}

#[unsafe(no_mangle)]
pub extern "C" fn longjmp(_env: *mut c_void, val: i32) -> ! {
    let _ = syscall1(SYS_EXIT, val as u64);
    loop {
        core::hint::spin_loop();
    }
}

#[repr(C)]
pub struct VaListC {
    pub gp_offset: u32,
    pub fp_offset: u32,
    pub overflow_arg_area: *mut c_void,
    pub reg_save_area: *mut c_void,
}

unsafe fn next_gp<T: Copy>(ap: &mut VaListC) -> T {
    let size = core::mem::size_of::<T>();
    if size <= 8 && ap.gp_offset < 48 {
        let p = (ap.reg_save_area as usize + ap.gp_offset as usize) as *const T;
        ap.gp_offset += 8;
        core::ptr::read_unaligned(p)
    } else {
        let p = ap.overflow_arg_area as *const T;
        ap.overflow_arg_area = (ap.overflow_arg_area as usize + 8) as *mut c_void;
        core::ptr::read_unaligned(p)
    }
}

unsafe fn next_fp<T: Copy>(ap: &mut VaListC) -> T {
    let size = core::mem::size_of::<T>();
    if size <= 16 && ap.fp_offset < 176 {
        let p = (ap.reg_save_area as usize + ap.fp_offset as usize) as *const T;
        ap.fp_offset += 16;
        core::ptr::read_unaligned(p)
    } else {
        let p = ap.overflow_arg_area as *const T;
        ap.overflow_arg_area = (ap.overflow_arg_area as usize + 8) as *mut c_void;
        core::ptr::read_unaligned(p)
    }
}

unsafe fn vprintf_core(fmt: *const c_char, ap: &mut VaListC) {
    let mut f = fmt;
    loop {
        let c = unsafe { *f as u8 };
        if c == 0 {
            break;
        }
        if c != b'%' {
            print_char(c);
            f = unsafe { f.add(1) };
            continue;
        }

        f = unsafe { f.add(1) };
        // Skip width/precision
        while unsafe { (*f as u8).is_ascii_digit() || *f as u8 == b'.' || *f as u8 == b'-' } {
            f = unsafe { f.add(1) };
        }

        match unsafe { *f as u8 } {
            b'%' => print_char(b'%'),
            b's' => {
                let s = unsafe { next_gp::<*const c_char>(ap) };
                if !s.is_null() {
                    let mut i = 0usize;
                    while unsafe { *s.add(i) } != 0 {
                        print_char(unsafe { *s.add(i) as u8 });
                        i += 1;
                    }
                }
            }
            b'd' | b'i' => {
                let mut tmp = [0i8; 24];
                let n = unsafe { next_gp::<i32>(ap) } as i64;
                let _ = unsafe { write_num(n, tmp.as_mut_ptr()) };
                let mut i = 0usize;
                while tmp[i] != 0 {
                    print_char(tmp[i] as u8);
                    i += 1;
                }
            }
            b'f' | b'g' => {
                let mut tmp = [0i8; 64];
                let val = unsafe { next_fp::<f64>(ap) };
                let _ = unsafe { write_float(val, tmp.as_mut_ptr(), 2) };
                let mut i = 0usize;
                while tmp[i] != 0 {
                    print_char(tmp[i] as u8);
                    i += 1;
                }
            }
            _ => print_char(b'?'),
        }
        f = unsafe { f.add(1) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vprintf(fmt: *const c_char, ap: *mut VaListC) -> i32 {
    unsafe { vprintf_core(fmt, &mut *ap) };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn printf(fmt: *const c_char, mut ap: ...) -> i32 {
    if fmt.is_null() {
        return -1;
    }
    let mut f = fmt;
    loop {
        let c = unsafe { *f as u8 };
        if c == 0 {
            break;
        }
        if c != b'%' {
            print_char(c);
            f = unsafe { f.add(1) };
            continue;
        }
        f = unsafe { f.add(1) };
        // Skip width/precision
        while unsafe { (*f as u8).is_ascii_digit() || *f as u8 == b'.' || *f as u8 == b'-' } {
            f = unsafe { f.add(1) };
        }
        match unsafe { *f as u8 } {
            b'%' => print_char(b'%'),
            b's' => {
                let s = unsafe { ap.arg::<*const c_char>() };
                if !s.is_null() {
                    let mut i = 0usize;
                    while unsafe { *s.add(i) } != 0 {
                        print_char(unsafe { *s.add(i) as u8 });
                        i += 1;
                    }
                }
            }
            b'd' | b'i' => {
                let mut tmp = [0i8; 24];
                let n = unsafe { ap.arg::<i32>() } as i64;
                let _ = unsafe { write_num(n, tmp.as_mut_ptr()) };
                let mut i = 0usize;
                while tmp[i] != 0 {
                    print_char(tmp[i] as u8);
                    i += 1;
                }
            }
            b'f' | b'g' => {
                let mut tmp = [0i8; 64];
                let val = unsafe { ap.arg::<f64>() };
                let _ = unsafe { write_float(val, tmp.as_mut_ptr(), 2) };
                let mut i = 0usize;
                while tmp[i] != 0 {
                    print_char(tmp[i] as u8);
                    i += 1;
                }
            }
            _ => print_char(b'?'),
        }
        f = unsafe { f.add(1) };
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf(_stream: *mut c_void, fmt: *const c_char, ap: *mut VaListC) -> i32 {
    unsafe { vprintf_core(fmt, &mut *ap) };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fprintf(_stream: *mut c_void, fmt: *const c_char, mut ap: ...) -> i32 {
    if fmt.is_null() {
        return -1;
    }
    let mut f = fmt;
    loop {
        let c = unsafe { *f as u8 };
        if c == 0 {
            break;
        }
        if c != b'%' {
            print_char(c);
            f = unsafe { f.add(1) };
            continue;
        }
        f = unsafe { f.add(1) };
        // Skip width/precision
        while unsafe { (*f as u8).is_ascii_digit() || *f as u8 == b'.' || *f as u8 == b'-' } {
            f = unsafe { f.add(1) };
        }
        match unsafe { *f as u8 } {
            b'%' => print_char(b'%'),
            b's' => {
                let s = unsafe { ap.arg::<*const c_char>() };
                if !s.is_null() {
                    let mut i = 0usize;
                    while unsafe { *s.add(i) } != 0 {
                        print_char(unsafe { *s.add(i) as u8 });
                        i += 1;
                    }
                }
            }
            b'd' | b'i' => {
                let mut tmp = [0i8; 24];
                let n = unsafe { ap.arg::<i32>() } as i64;
                let _ = unsafe { write_num(n, tmp.as_mut_ptr()) };
                let mut i = 0usize;
                while tmp[i] != 0 {
                    print_char(tmp[i] as u8);
                    i += 1;
                }
            }
            b'f' | b'g' => {
                let mut tmp = [0i8; 64];
                let val = unsafe { ap.arg::<f64>() };
                let _ = unsafe { write_float(val, tmp.as_mut_ptr(), 2) };
                let mut i = 0usize;
                while tmp[i] != 0 {
                    print_char(tmp[i] as u8);
                    i += 1;
                }
            }
            _ => print_char(b'?'),
        }
        f = unsafe { f.add(1) };
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fscanf(_fp: *mut c_void, _fmt: *const c_char, mut _ap: ...) -> i32 {
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __isoc99_fscanf(fp: *mut c_void, fmt: *const c_char, mut ap: ...) -> i32 {
    let _ = fp;
    let _ = fmt;
    let _ = &mut ap;
    -1
}

unsafe fn write_num(mut n: i64, out: *mut c_char) -> usize {
    if n == 0 {
        unsafe {
            *out = b'0' as i8;
            *out.add(1) = 0;
        }
        return 1;
    }
    let mut pos = 0usize;
    if n < 0 {
        unsafe {
            *out = b'-' as i8;
        }
        pos += 1;
        n = -n;
    }
    let mut tmp = [0u8; 20];
    let mut i = 0usize;
    while n > 0 {
        tmp[i] = (n % 10) as u8 + b'0';
        n /= 10;
        i += 1;
    }
    for j in 0..i {
        unsafe {
            *out.add(pos + j) = tmp[i - 1 - j] as i8;
        }
    }
    unsafe {
        *out.add(pos + i) = 0;
    }
    pos + i
}

unsafe fn write_float(mut f: f64, out: *mut c_char, precision: i32) -> usize {
    let mut pos = 0usize;
    if f < 0.0 {
        unsafe { *out = b'-' as i8; }
        pos += 1;
        f = -f;
    }
    let ipart = f as i64;
    let written = unsafe { write_num(ipart, out.add(pos)) };
    pos += written;
    if precision > 0 {
        unsafe { *out.add(pos) = b'.' as i8; }
        pos += 1;
        let mut fpart = f - ipart as f64;
        for _ in 0..precision {
            fpart *= 10.0;
            let digit = fpart as i32;
            unsafe { *out.add(pos) = (digit as u8 + b'0') as i8; }
            pos += 1;
            fpart -= digit as f64;
        }
    }
    unsafe { *out.add(pos) = 0; }
    pos
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsprintf(
    dst: *mut c_char,
    fmt: *const c_char,
    ap: *mut VaListC,
) -> i32 {
    let ap_ref = unsafe { &mut *ap };
    if dst.is_null() || fmt.is_null() {
        return -1;
    }
    let mut f = fmt;
    let mut out = dst;
    loop {
        let c = unsafe { *f as u8 };
        if c == 0 {
            break;
        }
        if c != b'%' {
            unsafe {
                *out = c as i8;
            }
            out = unsafe { out.add(1) };
            f = unsafe { f.add(1) };
            continue;
        }

        f = unsafe { f.add(1) };
        // Skip width/precision
        while unsafe { (*f as u8).is_ascii_digit() || *f as u8 == b'.' || *f as u8 == b'-' } {
            f = unsafe { f.add(1) };
        }
        match unsafe { *f as u8 } {
            b'%' => {
                unsafe { *out = b'%' as i8 };
                out = unsafe { out.add(1) };
            }
            b's' => {
                let s = unsafe { next_gp::<*const c_char>(ap_ref) };
                if !s.is_null() {
                    let mut i = 0usize;
                    while unsafe { *s.add(i) } != 0 {
                        unsafe {
                            *out = *s.add(i);
                        }
                        out = unsafe { out.add(1) };
                        i += 1;
                    }
                }
            }
            b'd' | b'i' => {
                let n = unsafe { next_gp::<i32>(ap_ref) } as i64;
                let wrote = unsafe { write_num(n, out) };
                out = unsafe { out.add(wrote) };
            }
            b'f' | b'g' => {
                let val = unsafe { next_fp::<f64>(ap_ref) };
                let wrote = unsafe { write_float(val, out, 2) };
                out = unsafe { out.add(wrote) };
            }
            b'c' => {
                let n = unsafe { next_gp::<i32>(ap_ref) } as i8;
                unsafe { *out = n; }
                out = unsafe { out.add(1) };
            }
            _ => {
                unsafe { *out = b'?' as i8 };
                out = unsafe { out.add(1) };
            }
        }
        f = unsafe { f.add(1) };
    }
    unsafe {
        *out = 0;
    }
    (out as usize - dst as usize) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sprintf(dst: *mut c_char, fmt: *const c_char, mut ap: ...) -> i32 {
    if dst.is_null() || fmt.is_null() {
        return -1;
    }
    let mut f = fmt;
    let mut out = dst;
    loop {
        let c = unsafe { *f as u8 };
        if c == 0 {
            break;
        }
        if c != b'%' {
            unsafe {
                *out = c as i8;
            }
            out = unsafe { out.add(1) };
            f = unsafe { f.add(1) };
            continue;
        }

        f = unsafe { f.add(1) };
        // Skip width/precision
        while unsafe { (*f as u8).is_ascii_digit() || *f as u8 == b'.' || *f as u8 == b'-' } {
            f = unsafe { f.add(1) };
        }

        match unsafe { *f as u8 } {
            b'%' => {
                unsafe { *out = b'%' as i8 };
                out = unsafe { out.add(1) };
            }
            b's' => {
                let s = unsafe { ap.arg::<*const c_char>() };
                if !s.is_null() {
                    let mut i = 0usize;
                    while unsafe { *s.add(i) } != 0 {
                        unsafe { *out = *s.add(i) };
                        out = unsafe { out.add(1) };
                        i += 1;
                    }
                }
            }
            b'd' | b'i' => {
                let n = unsafe { ap.arg::<i32>() } as i64;
                let wrote = unsafe { write_num(n, out) };
                out = unsafe { out.add(wrote) };
            }
            b'f' | b'g' => {
                let val = unsafe { ap.arg::<f64>() };
                let wrote = unsafe { write_float(val, out, 2) };
                out = unsafe { out.add(wrote) };
            }
            _ => {
                unsafe {
                    *out = b'?' as i8;
                }
                out = unsafe { out.add(1) };
            }
        }
        f = unsafe { f.add(1) };
    }
    unsafe {
        *out = 0;
    }
    (out as usize - dst as usize) as i32
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    print_str("[quake] panic\n");
    user_exit()
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    print_str("[quake] stub start\n");
    let argv0 = b"quake\0";
    let argv: [*const i8; 2] = [argv0.as_ptr() as *const i8, core::ptr::null()];
    let _ = unsafe { main(1, argv.as_ptr()) };
    user_exit()
}

#[unsafe(no_mangle)]
pub extern "C" fn exit(_code: i32) -> ! {
    let _ = syscall1(SYS_EXIT, _code as u64);
    loop {
        core::hint::spin_loop();
    }
}
