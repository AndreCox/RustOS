#![no_std]
#![no_main]
#![feature(c_variadic)]

use core::ffi::{VaList, c_char, c_void};
use core::panic::PanicInfo;
use rustos_user::{
    SYS_DRAW_BUFFER, SYS_ENTER_EXCLUSIVE_GRAPHICS, SYS_EXIT, SYS_EXIT_EXCLUSIVE_GRAPHICS,
    SYS_FS_CLOSE, SYS_FS_OPEN, SYS_FS_READ_HANDLE, SYS_FS_SEEK_HANDLE, SYS_GET_KEY,
    SYS_GET_SCANCODE, SYS_GET_UPTIME, SYS_YIELD, exit as user_exit, print_char, print_str,
    syscall0, syscall1, syscall2, syscall3,
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
static mut RAND_STATE: u64 = 0x1234_abcd;
static mut ERRNO: i32 = 0;

fn print_hex_u64(mut v: u64) {
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    let hex = b"0123456789abcdef";
    let mut i = 0usize;
    while i < 16 {
        let shift = (15 - i) * 4;
        let d = ((v >> shift) & 0xF) as usize;
        buf[2 + i] = hex[d];
        i += 1;
    }
    for b in buf {
        print_char(b);
    }
}

const MALLOC_SIZE: usize = 128 * 1024 * 1024;

#[repr(align(16))]
struct AlignedMallocBuffer([u8; MALLOC_SIZE]);

static mut MALLOC_BUFFER: AlignedMallocBuffer = AlignedMallocBuffer([0; MALLOC_SIZE]);
static mut MALLOC_PTR: usize = 0;
const MALLOC_HEADER_SIZE: usize = core::mem::size_of::<usize>();

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
pub unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    if n == 0 || core::ptr::eq(dst, src as *mut c_void) {
        return dst;
    }

    let d = dst as *mut u8;
    let s = src as *const u8;
    let mut i = 0usize;
    while i < n {
        unsafe {
            *d.add(i) = *s.add(i);
        }
        i += 1;
    }
    dst
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    if n == 0 || core::ptr::eq(dst, src as *mut c_void) {
        return dst;
    }

    let d = dst as *mut u8;
    let s = src as *const u8;

    if (d as usize) <= (s as usize) || (d as usize) >= (s as usize).saturating_add(n) {
        let mut i = 0usize;
        while i < n {
            unsafe {
                *d.add(i) = *s.add(i);
            }
            i += 1;
        }
    } else {
        let mut i = n;
        while i > 0 {
            let j = i - 1;
            unsafe {
                *d.add(j) = *s.add(j);
            }
            i -= 1;
        }
    }

    dst
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let d = dst as *mut u8;
    let b = c as u8;
    let mut i = 0usize;
    while i < n {
        unsafe {
            *d.add(i) = b;
        }
        i += 1;
    }
    dst
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> i32 {
    let pa = a as *const u8;
    let pb = b as *const u8;
    let mut i = 0usize;
    while i < n {
        let va = unsafe { *pa.add(i) };
        let vb = unsafe { *pb.add(i) };
        if va != vb {
            return va as i32 - vb as i32;
        }
        i += 1;
    }
    0
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

    if handle == u64::MAX {
        let mut norm = [0u8; MAX_PATH_TMP];
        let len = unsafe { copy_path_normalized(path, &mut norm) };

        if len > 0 {
            let mut start = 0usize;
            while start + 1 < len && norm[start] == b'.' && norm[start + 1] == b'/' {
                start += 2;
            }

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
    let mut total = 0usize;
    while total < bytes {
        let n = syscall3(
            SYS_FS_READ_HANDLE,
            file.handle,
            (ptr as *mut u8).wrapping_add(total) as u64,
            (bytes - total) as u64,
        );
        if n == u64::MAX || n == 0 {
            break;
        }
        total = total.saturating_add(n as usize);
    }
    total / size
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fwrite(
    ptr: *const c_void,
    size: usize,
    nmemb: usize,
    fp: *mut c_void,
) -> usize {
    if ptr.is_null() || fp.is_null() || size == 0 || nmemb == 0 {
        return 0;
    }
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
pub unsafe extern "C" fn fgetc(stream: *mut c_void) -> i32 {
    let mut c = 0u8;
    let n = unsafe { fread(&mut c as *mut u8 as *mut c_void, 1, 1, stream) };
    if n == 1 { c as i32 } else { -1 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn getc(stream: *mut c_void) -> i32 {
    unsafe { fgetc(stream) }
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
pub unsafe extern "C" fn rand() -> i32 {
    unsafe {
        RAND_STATE = RAND_STATE.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((RAND_STATE >> 33) & 0x7FFFFFFF) as i32
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn srand(seed: u32) {
    unsafe { RAND_STATE = seed as u64 };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __isoc99_fscanf(_stream: *mut c_void, _format: *const c_char, mut _ap: ...) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    let size = size.max(1);
    let ptr = unsafe { MALLOC_PTR };
    let aligned_user_ptr = (ptr.saturating_add(MALLOC_HEADER_SIZE) + 15) & !15;
    if aligned_user_ptr < MALLOC_HEADER_SIZE {
        return core::ptr::null_mut();
    }
    let header_ptr = aligned_user_ptr - MALLOC_HEADER_SIZE;
    if aligned_user_ptr.saturating_add(size) > MALLOC_SIZE {
        print_str("[quake] OOM in malloc\n");
        return core::ptr::null_mut();
    }
    unsafe {
        let base = core::ptr::addr_of_mut!(MALLOC_BUFFER.0) as *mut u8;
        *(base.add(header_ptr) as *mut usize) = size;
        MALLOC_PTR = aligned_user_ptr + size;
        base.add(aligned_user_ptr) as *mut c_void
    }
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
    if size == 0 {
        return core::ptr::null_mut();
    }
    
    let header_ptr = (ptr as usize) - MALLOC_HEADER_SIZE;
    let old_size = unsafe { *(header_ptr as *const usize) };
    
    if size <= old_size {
        return ptr;
    }
    
    let new_ptr = unsafe { malloc(size) };
    if !new_ptr.is_null() {
        unsafe { memcpy(new_ptr, ptr, old_size) };
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
pub unsafe extern "C" fn strtol(nptr: *const c_char, endptr: *mut *mut c_char, mut base: i32) -> i64 {
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

    if base == 0 || base == 16 {
        if unsafe { *s == b'0' as i8 && (*s.add(1) == b'x' as i8 || *s.add(1) == b'X' as i8) } {
            base = 16;
            s = unsafe { s.add(2) };
        } else if base == 0 {
            if unsafe { *s == b'0' as i8 } {
                base = 8;
                s = unsafe { s.add(1) };
            } else {
                base = 10;
            }
        }
    } else if base == 0 {
        base = 10;
    }

    let mut out: i64 = 0;
    loop {
        let c = unsafe { *s as u8 };
        let digit = match c {
            b'0'..=b'9' => (c - b'0') as i32,
            b'a'..=b'z' => (c - b'a') as i32 + 10,
            b'A'..=b'Z' => (c - b'A') as i32 + 10,
            _ => -1,
        };
        if digit < 0 || digit >= base {
            break;
        }
        out = out * base as i64 + digit as i64;
        s = unsafe { s.add(1) };
    }

    if !endptr.is_null() {
        unsafe { *endptr = s as *mut c_char };
    }
    if neg {
        -out
    } else {
        out
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strtod(nptr: *const c_char, endptr: *mut *mut c_char) -> f64 {
    let mut s = nptr;
    while unsafe { *s == b' ' as i8 || *s == b'\t' as i8 || *s == b'\n' as i8 || *s == b'\r' as i8 }
    {
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
#[unsafe(naked)]
pub unsafe extern "C" fn setjmp(env: *mut u64) -> i32 {
    core::arch::naked_asm!(
        "mov [rdi + 0], rbx",
        "mov [rdi + 8], rbp",
        "mov [rdi + 16], r12",
        "mov [rdi + 24], r13",
        "mov [rdi + 32], r14",
        "mov [rdi + 40], r15",
        "lea rdx, [rsp + 8]",
        "mov [rdi + 48], rdx",
        "mov rdx, [rsp]",
        "mov [rdi + 56], rdx",
        "xor eax, eax",
        "ret"
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _setjmp(env: *mut u64) -> i32 {
    core::arch::naked_asm!("jmp setjmp")
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn longjmp(env: *mut u64, val: i32) -> ! {
    core::arch::naked_asm!(
        "mov rbx, [rdi + 0]",
        "mov rbp, [rdi + 8]",
        "mov r12, [rdi + 16]",
        "mov r13, [rdi + 24]",
        "mov r14, [rdi + 32]",
        "mov r15, [rdi + 40]",
        "mov rsp, [rdi + 48]",
        "mov rdx, [rdi + 56]",
        "mov eax, esi",
        "test eax, eax",
        "jnz 1f",
        "inc eax",
        "1:",
        "jmp rdx"
    )
}

// ========================================================================
//  Math library — naked assembly to bypass LLVM soft-float ABI mismatch.
//  C callers pass f64 in xmm0 (SysV ABI), but LLVM compiles Rust extern "C"
//  fn(f64)->f64 with soft-float (f64 in rdi).  Naked asm sidesteps this.
// ========================================================================

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn floor(_x: f64) -> f64 {
    // xmm0 = input, return in xmm0
    // Truncate toward zero, then adjust for negatives
    core::arch::naked_asm!(
        "cvttsd2si rax, xmm0",   // rax = (i64)x  (truncation)
        "cvtsi2sd  xmm1, rax",   // xmm1 = (f64)rax
        "comisd    xmm1, xmm0",  // compare truncated vs original
        "jbe 2f",                 // if truncated <= x, result is truncated (floor correct)
        "movsd     xmm2, qword ptr [rip + .Lone_f]",
        "subsd     xmm1, xmm2",  // truncated - 1.0
        "2:",
        "movsd     xmm0, xmm1",
        "ret",
        ".Lone_f:",
        ".quad 0x3FF0000000000000",  // 1.0 in f64
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn ceil(_x: f64) -> f64 {
    core::arch::naked_asm!(
        "cvttsd2si rax, xmm0",
        "cvtsi2sd  xmm1, rax",
        "comisd    xmm0, xmm1",   // compare original vs truncated
        "jbe 2f",                  // if x <= truncated, result is truncated
        "movsd     xmm2, qword ptr [rip + .Lone_c]",
        "addsd     xmm1, xmm2",   // truncated + 1.0
        "2:",
        "movsd     xmm0, xmm1",
        "ret",
        ".Lone_c:",
        ".quad 0x3FF0000000000000",  // 1.0
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn sqrt(_x: f64) -> f64 {
    core::arch::naked_asm!(
        "sqrtsd xmm0, xmm0",
        "ret",
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn fabs(_x: f64) -> f64 {
    core::arch::naked_asm!(
        "movq   rax, xmm0",
        "btr    rax, 63",        // clear sign bit
        "movq   xmm0, rax",
        "ret",
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn sin(_x: f64) -> f64 {
    // Taylor: sin(x) ≈ x - x³/6 + x⁵/120 - x⁷/5040 + x⁹/362880
    // First reduce x to [-pi, pi] range using floor
    core::arch::naked_asm!(
        // Save original x
        "sub     rsp, 8",
        "movsd   qword ptr [rsp], xmm0",
        // Check if |x| > pi; if so, reduce
        "movq    rax, xmm0",
        "btr     rax, 63",
        "movq    xmm1, rax",            // xmm1 = |x|
        "movsd   xmm2, qword ptr [rip + .Lpi_s]",
        "comisd  xmm1, xmm2",
        "jbe     .Lsin_nored",
        // x = x - floor(x / (2*pi)) * (2*pi)
        "movsd   xmm0, qword ptr [rsp]",
        "movsd   xmm3, qword ptr [rip + .Ltwo_pi_s]",
        "divsd   xmm0, xmm3",
        "call    floor",
        "mulsd   xmm0, qword ptr [rip + .Ltwo_pi_s]",
        "movsd   xmm1, qword ptr [rsp]",
        "subsd   xmm1, xmm0",
        "movsd   qword ptr [rsp], xmm1",
        // If still > pi, subtract 2*pi
        "movsd   xmm2, qword ptr [rip + .Lpi_s]",
        "comisd  xmm1, xmm2",
        "jbe     .Lsin_nored",
        "subsd   xmm1, qword ptr [rip + .Ltwo_pi_s]",
        "movsd   qword ptr [rsp], xmm1",
        ".Lsin_nored:",
        "movsd   xmm0, qword ptr [rsp]",
        // x2 = x * x
        "movsd   xmm1, xmm0",
        "mulsd   xmm1, xmm1",       // xmm1 = x²
        // Horner form: x * (1 - x²/6 * (1 - x²/20 * (1 - x²/42 * (1 - x²/72))))
        "movsd   xmm2, xmm1",
        "divsd   xmm2, qword ptr [rip + .L72_s]",   // x²/72
        "movsd   xmm3, qword ptr [rip + .Lone_s]",
        "subsd   xmm3, xmm2",       // 1 - x²/72
        "mulsd   xmm3, xmm1",
        "divsd   xmm3, qword ptr [rip + .L42_s]",   // * x²/42
        "movsd   xmm2, qword ptr [rip + .Lone_s]",
        "subsd   xmm2, xmm3",       // 1 - ...
        "mulsd   xmm2, xmm1",
        "divsd   xmm2, qword ptr [rip + .L20_s]",
        "movsd   xmm3, qword ptr [rip + .Lone_s]",
        "subsd   xmm3, xmm2",
        "mulsd   xmm3, xmm1",
        "divsd   xmm3, qword ptr [rip + .L6_s]",
        "movsd   xmm2, qword ptr [rip + .Lone_s]",
        "subsd   xmm2, xmm3",
        "mulsd   xmm0, xmm2",       // x * result
        "add     rsp, 8",
        "ret",
        ".Lpi_s:",
        ".quad 0x400921FB54442D18",   // pi
        ".Ltwo_pi_s:",
        ".quad 0x401921FB54442D18",   // 2*pi
        ".Lone_s:",
        ".quad 0x3FF0000000000000",   // 1.0
        ".L6_s:",
        ".quad 0x4018000000000000",   // 6.0
        ".L20_s:",
        ".quad 0x4034000000000000",   // 20.0
        ".L42_s:",
        ".quad 0x4045000000000000",   // 42.0
        ".L72_s:",
        ".quad 0x4052000000000000",   // 72.0
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn cos(_x: f64) -> f64 {
    core::arch::naked_asm!(
        "addsd  xmm0, qword ptr [rip + .Lhalf_pi_c]",
        "jmp    sin",
        ".Lhalf_pi_c:",
        ".quad 0x3FF921FB54442D18",   // pi/2
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn acos(_x: f64) -> f64 {
    // acos(x) ≈ pi/2 - (x + x³/6 + 3x⁵/40)
    core::arch::naked_asm!(
        "movsd   xmm1, xmm0",       // xmm1 = x
        "mulsd   xmm1, xmm1",       // xmm1 = x²
        "movsd   xmm2, xmm0",       // xmm2 = x
        "mulsd   xmm2, xmm1",       // xmm2 = x³
        "movsd   xmm3, qword ptr [rip + .Lsixth_a]",
        "mulsd   xmm3, xmm2",       // x³/6
        "addsd   xmm0, xmm3",       // x + x³/6
        "mulsd   xmm2, xmm1",       // xmm2 = x⁵
        "movsd   xmm3, qword ptr [rip + .L3_40_a]",
        "mulsd   xmm3, xmm2",       // 3x⁵/40
        "addsd   xmm0, xmm3",       // x + x³/6 + 3x⁵/40
        "movsd   xmm1, qword ptr [rip + .Lhalf_pi_a]",
        "subsd   xmm1, xmm0",       // pi/2 - asin(x)
        "movsd   xmm0, xmm1",
        "ret",
        ".Lsixth_a:",
        ".quad 0x3FC5555555555555",   // 1/6
        ".L3_40_a:",
        ".quad 0x3FD3333333333333",   // 3/40 = 0.075
        ".Lhalf_pi_a:",
        ".quad 0x3FF921FB54442D18",   // pi/2
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn atan2(_y: f64, _x: f64) -> f64 {
    // Simple: if x>0 return atan(y/x), handle quadrants
    // xmm0 = y, xmm1 = x
    core::arch::naked_asm!(
        "xorpd   xmm2, xmm2",       // xmm2 = 0.0
        // if x == 0
        "comisd  xmm1, xmm2",
        "jne     .Lat2_nonzero",
        // x == 0: return sign(y) * pi/2
        "comisd  xmm0, xmm2",
        "ja      .Lat2_pos_half_pi",
        "jb      .Lat2_neg_half_pi",
        "xorpd   xmm0, xmm0",       // y==0, x==0 → 0
        "ret",
        ".Lat2_pos_half_pi:",
        "movsd   xmm0, qword ptr [rip + .Lhalf_pi_at]",
        "ret",
        ".Lat2_neg_half_pi:",
        "movsd   xmm0, qword ptr [rip + .Lneg_half_pi_at]",
        "ret",
        ".Lat2_nonzero:",
        // ratio = y / x
        "divsd   xmm0, xmm1",       // xmm0 = y/x
        // Simple atan approx for small values: a - a³/3 + a⁵/5
        "movsd   xmm3, xmm0",       // save ratio
        "movsd   xmm2, xmm0",
        "mulsd   xmm2, xmm2",       // a²
        "movsd   xmm4, xmm0",
        "mulsd   xmm4, xmm2",       // a³
        "movsd   xmm5, qword ptr [rip + .Lthird_at]",
        "mulsd   xmm5, xmm4",       // a³/3
        "subsd   xmm0, xmm5",       // a - a³/3
        // Adjust quadrant: if x < 0, add/sub pi
        "xorpd   xmm2, xmm2",
        "comisd  xmm1, xmm2",
        "ja      .Lat2_done",
        // x < 0
        "comisd  xmm3, xmm2",       // check original y/x sign
        "jb      .Lat2_sub_pi",
        "addsd   xmm0, qword ptr [rip + .Lpi_at]",
        "ret",
        ".Lat2_sub_pi:",
        "subsd   xmm0, qword ptr [rip + .Lpi_at]",
        ".Lat2_done:",
        "ret",
        ".Lhalf_pi_at:",
        ".quad 0x3FF921FB54442D18",
        ".Lneg_half_pi_at:",
        ".quad 0xBFF921FB54442D18",
        ".Lpi_at:",
        ".quad 0x400921FB54442D18",
        ".Lthird_at:",
        ".quad 0x3FD5555555555555",   // 1/3
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn pow(_base: f64, _exp: f64) -> f64 {
    // xmm0 = base, xmm1 = exp
    // Handle: exp==0 → 1, exp==1 → base, exp==2 → base*base
    core::arch::naked_asm!(
        "xorpd   xmm2, xmm2",
        "comisd  xmm1, xmm2",
        "jne 1f",
        "movsd   xmm0, qword ptr [rip + .Lone_p]",
        "ret",
        "1:",
        "movsd   xmm2, qword ptr [rip + .Lone_p]",
        "comisd  xmm1, xmm2",
        "jne 2f",
        "ret",                       // return base
        "2:",
        "addsd   xmm2, xmm2",       // xmm2 = 2.0
        "comisd  xmm1, xmm2",
        "jne 3f",
        "mulsd   xmm0, xmm0",       // base * base
        "ret",
        "3:",
        // fallback: return 1.0 for other exponents
        "movsd   xmm0, qword ptr [rip + .Lone_p]",
        "ret",
        ".Lone_p:",
        ".quad 0x3FF0000000000000",
    )
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
        unsafe { core::ptr::read_unaligned(p) }
    } else {
        let p = ap.overflow_arg_area as *const T;
        ap.overflow_arg_area = (ap.overflow_arg_area as usize + 8) as *mut c_void;
        unsafe { core::ptr::read_unaligned(p) }
    }
}

unsafe fn next_fp<T: Copy>(ap: &mut VaListC) -> T {
    let size = core::mem::size_of::<T>();
    if size <= 16 && ap.fp_offset < 176 {
        let p = (ap.reg_save_area as usize + ap.fp_offset as usize) as *const T;
        ap.fp_offset += 16;
        unsafe { core::ptr::read_unaligned(p) }
    } else {
        let p = ap.overflow_arg_area as *const T;
        ap.overflow_arg_area = (ap.overflow_arg_area as usize + 8) as *mut c_void;
        unsafe { core::ptr::read_unaligned(p) }
    }
}

unsafe fn vfprintf_internal(
    fp: *mut c_void,
    dst: *mut c_char,
    fmt: *const c_char,
    ap: &mut VaListC,
) -> i32 {
    let mut f = fmt;
    let mut out = dst;
    let mut total = 0i32;

    macro_rules! write_char {
        ($c:expr) => {{
            let ch = $c as u8;
            if !dst.is_null() {
                unsafe { *out = ch as i8 };
                out = unsafe { out.add(1) };
            } else if fp.is_null() {
                print_char(ch);
            } else {
                print_char(ch);
            }
            total += 1;
        }};
    }

    loop {
        let c = unsafe { *f as u8 };
        if c == 0 {
            break;
        }
        if c != b'%' {
            write_char!(c);
            f = unsafe { f.add(1) };
            continue;
        }

        f = unsafe { f.add(1) };
        // Skip flags/width/precision.
        while unsafe {
            matches!(
                *f as u8,
                b'-' | b'+' | b' ' | b'#' | b'0' | b'.' | b'1'..=b'9'
            )
        } {
            f = unsafe { f.add(1) };
        }

        let mut long_mod = false;
        if unsafe { *f as u8 } == b'l' {
            long_mod = true;
            f = unsafe { f.add(1) };
            if unsafe { *f as u8 } == b'l' {
                f = unsafe { f.add(1) };
            }
        }

        match unsafe { *f as u8 } {
            b'%' => write_char!(b'%'),
            b'c' => {
                let val = unsafe { next_gp::<i32>(ap) } as u8;
                write_char!(val);
            }
            b's' => {
                let s = unsafe { next_gp::<*const c_char>(ap) };
                if !s.is_null() {
                    let mut i = 0usize;
                    while unsafe { *s.add(i) } != 0 {
                        write_char!(unsafe { *s.add(i) as u8 });
                        i += 1;
                    }
                } else {
                    for &b in b"(null)" { write_char!(b); }
                }
            }
            b'd' | b'i' => {
                let mut tmp = [0i8; 24];
                let n = if long_mod {
                    unsafe { next_gp::<i64>(ap) }
                } else {
                    unsafe { next_gp::<i32>(ap) as i64 }
                };
                let len = unsafe { write_num(n, tmp.as_mut_ptr()) };
                for i in 0..len { write_char!(tmp[i] as u8); }
            }
            b'u' => {
                let mut tmp = [0i8; 32];
                let n = if long_mod {
                    unsafe { next_gp::<u64>(ap) }
                } else {
                    unsafe { next_gp::<u32>(ap) as u64 }
                };
                let len = unsafe { write_unum_base(n, 10, false, tmp.as_mut_ptr()) };
                for i in 0..len { write_char!(tmp[i] as u8); }
            }
            b'x' | b'X' => {
                let mut tmp = [0i8; 32];
                let n = if long_mod {
                    unsafe { next_gp::<u64>(ap) }
                } else {
                    unsafe { next_gp::<u32>(ap) as u64 }
                };
                let len = unsafe { write_unum_base(n, 16, (*f as u8) == b'X', tmp.as_mut_ptr()) };
                for i in 0..len { write_char!(tmp[i] as u8); }
            }
            b'p' => {
                let ptr = unsafe { next_gp::<usize>(ap) } as u64;
                write_char!(b'0'); write_char!(b'x');
                let mut tmp = [0i8; 32];
                let len = unsafe { write_unum_base(ptr, 16, false, tmp.as_mut_ptr()) };
                for i in 0..len { write_char!(tmp[i] as u8); }
            }
            b'f' | b'g' => {
                let mut tmp = [0i8; 64];
                let val = unsafe { next_fp::<f64>(ap) };
                let len = unsafe { write_float(val, tmp.as_mut_ptr(), 2) };
                for i in 0..len { write_char!(tmp[i] as u8); }
            }
            _ => {
                write_char!(b'%');
                write_char!(unsafe { *f as u8 });
            }
        }
        f = unsafe { f.add(1) };
    }

    if !dst.is_null() {
        unsafe { *out = 0 };
    }
    total
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vprintf(fmt: *const c_char, ap: *mut VaListC) -> i32 {
    unsafe { vfprintf_internal(core::ptr::null_mut(), core::ptr::null_mut(), fmt, &mut *ap) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf(fp: *mut c_void, fmt: *const c_char, ap: *mut VaListC) -> i32 {
    unsafe { vfprintf_internal(fp, core::ptr::null_mut(), fmt, &mut *ap) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsprintf(dst: *mut c_char, fmt: *const c_char, ap: *mut VaListC) -> i32 {
    unsafe { vfprintf_internal(core::ptr::null_mut(), dst, fmt, &mut *ap) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn printf(fmt: *const c_char, mut _ap: ...) -> i32 {
    let mut res = 0;
    unsafe {
        core::arch::asm!(
            "sub rsp, 208",
            "mov [rsp + 0], rdi",
            "mov [rsp + 8], rsi",
            "mov [rsp + 16], rdx",
            "mov [rsp + 24], rcx",
            "mov [rsp + 32], r8",
            "mov [rsp + 40], r9",
            "movaps [rsp + 48], xmm0",
            "movaps [rsp + 64], xmm1",
            "movaps [rsp + 80], xmm2",
            "movaps [rsp + 96], xmm3",
            "movaps [rsp + 112], xmm4",
            "movaps [rsp + 128], xmm5",
            "movaps [rsp + 144], xmm6",
            "movaps [rsp + 160], xmm7",
            "lea rax, [rsp + 208 + 8]",
            "mov [rsp + 200], rax",
            "lea rax, [rsp]",
            "mov [rsp + 208], rax",
            "mov dword ptr [rsp + 192], 8",
            "mov dword ptr [rsp + 196], 48",
            "lea rdx, [rsp + 192]",
            "mov rsi, {fmt}",
            "mov rdi, 0",
            "call {vfprintf_internal_wrapper_shim}",
            "mov {res:e}, eax",
            "add rsp, 208",
            fmt = in(reg) fmt,
            vfprintf_internal_wrapper_shim = sym vfprintf_internal_wrapper,
            res = out(reg) res,
        );
    }
    res
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fprintf(fp: *mut c_void, fmt: *const c_char, mut _ap: ...) -> i32 {
    let mut res = 0;
    unsafe {
        core::arch::asm!(
            "sub rsp, 208",
            "mov [rsp + 0], rdi",
            "mov [rsp + 8], rsi",
            "mov [rsp + 16], rdx",
            "mov [rsp + 24], rcx",
            "mov [rsp + 32], r8",
            "mov [rsp + 40], r9",
            "movaps [rsp + 48], xmm0",
            "movaps [rsp + 64], xmm1",
            "movaps [rsp + 80], xmm2",
            "movaps [rsp + 96], xmm3",
            "movaps [rsp + 112], xmm4",
            "movaps [rsp + 128], xmm5",
            "movaps [rsp + 144], xmm6",
            "movaps [rsp + 160], xmm7",
            "lea rax, [rsp + 208 + 8]",
            "mov [rsp + 200], rax",
            "lea rax, [rsp]",
            "mov [rsp + 208], rax",
            "mov dword ptr [rsp + 192], 16",
            "mov dword ptr [rsp + 196], 48",
            "lea rdx, [rsp + 192]",
            "mov rsi, {fmt}",
            "mov rdi, {fp}",
            "call {vfprintf_internal_wrapper_shim}",
            "mov {res:e}, eax",
            "add rsp, 208",
            fp = in(reg) fp,
            fmt = in(reg) fmt,
            vfprintf_internal_wrapper_shim = sym vfprintf_internal_wrapper,
            res = out(reg) res,
        );
    }
    res
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sprintf(dst: *mut c_char, fmt: *const c_char, mut _ap: ...) -> i32 {
    let mut res = 0;
    unsafe {
        core::arch::asm!(
            "sub rsp, 208",
            "mov [rsp + 0], rdi",
            "mov [rsp + 8], rsi",
            "mov [rsp + 16], rdx",
            "mov [rsp + 24], rcx",
            "mov [rsp + 32], r8",
            "mov [rsp + 40], r9",
            "movaps [rsp + 48], xmm0",
            "movaps [rsp + 64], xmm1",
            "movaps [rsp + 80], xmm2",
            "movaps [rsp + 96], xmm3",
            "movaps [rsp + 112], xmm4",
            "movaps [rsp + 128], xmm5",
            "movaps [rsp + 144], xmm6",
            "movaps [rsp + 160], xmm7",
            "lea rax, [rsp + 208 + 8]",
            "mov [rsp + 200], rax",
            "lea rax, [rsp]",
            "mov [rsp + 208], rax",
            "mov dword ptr [rsp + 192], 16",
            "mov dword ptr [rsp + 196], 48",
            "lea rdx, [rsp + 192]",
            "mov rsi, {fmt}",
            "mov rdi, {dst}",
            "call {vfprintf_internal_sprintf_shim}",
            "mov {res:e}, eax",
            "add rsp, 208",
            dst = in(reg) dst,
            fmt = in(reg) fmt,
            vfprintf_internal_sprintf_shim = sym vfprintf_internal_sprintf_shim,
            res = out(reg) res,
        );
    }
    res
}


// Shims to bridge assembly calls to Rust functions with proper ABI
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf_internal_wrapper(fp: *mut c_void, fmt: *const c_char, ap: *mut VaListC) -> i32 {
    unsafe { vfprintf_internal(fp, core::ptr::null_mut(), fmt, &mut *ap) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf_internal_sprintf_shim(dst: *mut c_char, fmt: *const c_char, ap: *mut VaListC) -> i32 {
    unsafe { vfprintf_internal(core::ptr::null_mut(), dst, fmt, &mut *ap) }
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

unsafe fn write_unum_base(mut n: u64, base: u64, uppercase: bool, out: *mut c_char) -> usize {
    if n == 0 {
        unsafe {
            *out = b'0' as i8;
            *out.add(1) = 0;
        }
        return 1;
    }

    let mut tmp = [0u8; 32];
    let mut i = 0usize;
    while n > 0 {
        let d = (n % base) as u8;
        tmp[i] = match d {
            0..=9 => b'0' + d,
            _ if uppercase => b'A' + (d - 10),
            _ => b'a' + (d - 10),
        };
        n /= base;
        i += 1;
    }

    for j in 0..i {
        unsafe {
            *out.add(j) = tmp[i - 1 - j] as i8;
        }
    }
    unsafe {
        *out.add(i) = 0;
    }
    i
}

unsafe fn write_float(mut f: f64, out: *mut c_char, precision: i32) -> usize {
    let mut pos = 0usize;
    if f < 0.0 {
        unsafe {
            *out = b'-' as i8;
        }
        pos += 1;
        f = -f;
    }
    let ipart = f as i64;
    let written = unsafe { write_num(ipart, out.add(pos)) };
    pos += written;
    if precision > 0 {
        unsafe {
            *out.add(pos) = b'.' as i8;
        }
        pos += 1;
        let mut fpart = f - ipart as f64;
        for _ in 0..precision {
            fpart *= 10.0;
            let digit = fpart as i32;
            unsafe {
                *out.add(pos) = (digit as u8 + b'0') as i8;
            }
            pos += 1;
            fpart -= digit as f64;
        }
    }
    unsafe {
        *out.add(pos) = 0;
    }
    pos
}


#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    print_str("[quake] panic\n");
    user_exit()
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    print_str("[quake] stub start\n");
    print_str("[quake] addr _start=");
    print_hex_u64(_start as *const () as usize as u64);
    print_str(" main=");
    print_hex_u64(main as *const () as usize as u64);
    print_str(" printf=");
    print_hex_u64(printf as *const () as usize as u64);
    print_str("\n");
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
