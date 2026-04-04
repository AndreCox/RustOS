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

// Output framebuffer for doomgeneric's scaled blit path.
// Must match DG_Width x DG_Height to avoid overflow in I_FinishUpdate.
static mut FRAMEBUFFER: [u32; 640 * 400] = [0; 640 * 400];

const MALLOC_SIZE: usize = 16 * 1024 * 1024; // 16MB
static mut MALLOC_BUFFER: [u8; MALLOC_SIZE] = [0; MALLOC_SIZE];
static mut MALLOC_PTR: usize = 0;

#[repr(C)]
struct AllocHeader {
    size: usize,
}

const ALLOC_ALIGN: usize = 16;

#[inline]
const fn align_up(value: usize, align: usize) -> usize {
    (value + (align - 1)) & !(align - 1)
}

// =============================================================================
// SYSCALL WRAPPERS
// =============================================================================

const SYS_PRINT_CHAR: u64 = 1;
const SYS_FS_WRITE: u64 = 6;
const SYS_DRAW_BUFFER: u64 = 10;
const SYS_GET_UPTIME: u64 = 11;
const SYS_FS_OPEN: u64 = 12;
const SYS_FS_READ_HANDLE: u64 = 13;
const SYS_FS_SEEK_HANDLE: u64 = 14;
const SYS_FS_CLOSE: u64 = 15;
const SYS_ENTER_EXCLUSIVE_GRAPHICS: u64 = 16;
const SYS_EXIT_EXCLUSIVE_GRAPHICS: u64 = 17;
const SYS_FS_MKDIR: u64 = 18;
const SYS_FS_REMOVE: u64 = 19;
const SYS_FS_RENAME: u64 = 20;
const SYS_GET_SCANCODE: u64 = 7;
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
    if n == 0 {
        print_char(b'0');
        return;
    }
    if n < 0 {
        print_char(b'-');
        n = -n;
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while n > 0 {
        buf[i] = (n % 10) as u8 + b'0';
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        print_char(buf[i]);
    }
}

struct Dummy;
#[global_allocator]
static ALLOCATOR: Dummy = Dummy;
unsafe impl core::alloc::GlobalAlloc for Dummy {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

// =============================================================================
// LIBC STUBS
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    unsafe {
        let base = core::ptr::addr_of_mut!(MALLOC_BUFFER) as usize;
        let header_size = core::mem::size_of::<AllocHeader>();
        let payload_size = align_up(size.max(1), ALLOC_ALIGN);

        // Keep payload 16-byte aligned while storing size metadata right before it.
        let cursor = base + MALLOC_PTR;
        let payload_addr = align_up(cursor + header_size, ALLOC_ALIGN);
        let header_addr = payload_addr - header_size;
        let new_ptr = payload_addr
            .checked_add(payload_size)
            .and_then(|end| end.checked_sub(base))
            .unwrap_or(usize::MAX);

        if new_ptr > MALLOC_SIZE {
            print_str("[DOOM] malloc failed! size: ");
            print_num(size as i64);
            print_char(b'\n');
            return core::ptr::null_mut();
        }

        (header_addr as *mut AllocHeader).write(AllocHeader { size: payload_size });
        MALLOC_PTR = new_ptr;
        payload_addr as *mut c_void
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() {
        return unsafe { malloc(size) };
    }

    if size == 0 {
        return core::ptr::null_mut();
    }

    let base = unsafe { core::ptr::addr_of!(MALLOC_BUFFER) as usize };
    let header_size = core::mem::size_of::<AllocHeader>();
    let ptr_addr = ptr as usize;

    if ptr_addr < base + header_size || ptr_addr > base + unsafe { MALLOC_PTR } {
        return core::ptr::null_mut();
    }

    let old_header = (ptr as usize).saturating_sub(header_size) as *const AllocHeader;
    let old_size = unsafe { (*old_header).size };

    let new_ptr = unsafe { malloc(size) };
    if !new_ptr.is_null() {
        unsafe {
            core::ptr::copy_nonoverlapping(
                ptr as *const u8,
                new_ptr as *mut u8,
                core::cmp::min(old_size, size),
            );
        }
    }
    new_ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut c_void {
    let total = nmemb * size;
    let ptr = unsafe { malloc(total) };
    if !ptr.is_null() {
        unsafe {
            core::ptr::write_bytes(ptr, 0, total);
        }
    }
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(_ptr: *mut c_void) {}

#[unsafe(no_mangle)]
pub extern "C" fn fabs(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}

#[unsafe(no_mangle)]
pub extern "C" fn abs(n: i32) -> i32 {
    if n < 0 { -n } else { n }
}

#[unsafe(no_mangle)]
pub static mut stdout: *mut c_void = core::ptr::null_mut();
#[unsafe(no_mangle)]
pub static mut stderr: *mut c_void = core::ptr::null_mut();
#[unsafe(no_mangle)]
pub static mut errno: i32 = 0;

const FILE_MODE_READ: u32 = 0;
const FILE_MODE_WRITE: u32 = 1;

#[repr(C)]
struct KernelFile {
    mode: u32,
    handle: u64,
    path: *mut c_char,
    buffer: *mut u8,
    len: usize,
    cap: usize,
    pos: usize,
}

unsafe fn cstr_len(ptr: *const c_char) -> usize {
    let mut len = 0usize;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    len
}

unsafe fn cstr_dup(ptr: *const c_char) -> *mut c_char {
    let len = unsafe { cstr_len(ptr) };
    let out = unsafe { malloc(len + 1) as *mut c_char };
    if out.is_null() {
        return core::ptr::null_mut();
    }
    unsafe {
        core::ptr::copy_nonoverlapping(ptr, out, len + 1);
    }
    out
}

// Kernel FS paths are most reliable with an explicit leading '/'.
// Doom often uses relative-looking paths like ".savegame/...".
unsafe fn normalize_kernel_path(src: *const c_char, dst: &mut [u8; 512]) -> *const c_char {
    if src.is_null() {
        return src;
    }

    let first = unsafe { *src as u8 };
    if first == b'/' {
        return src;
    }

    let mut s = src;
    loop {
        let c0 = unsafe { *s as u8 };
        let c1 = unsafe { *s.add(1) as u8 };
        if c0 == b'.' && c1 == b'/' {
            s = unsafe { s.add(2) };
            continue;
        }
        break;
    }

    dst[0] = b'/';
    let mut i = 1usize;
    loop {
        if i >= dst.len() - 1 {
            break;
        }
        let b = unsafe { *s as u8 };
        dst[i] = b;
        if b == 0 {
            return dst.as_ptr() as *const c_char;
        }
        i += 1;
        s = unsafe { s.add(1) };
    }

    dst[dst.len() - 1] = 0;
    dst.as_ptr() as *const c_char
}

unsafe fn file_reserve(file: *mut KernelFile, needed: usize) -> bool {
    let file = unsafe { &mut *file };
    if needed <= file.cap {
        return true;
    }

    let mut new_cap = if file.cap == 0 { 256 } else { file.cap };
    while new_cap < needed {
        new_cap = new_cap.saturating_mul(2);
        if new_cap == 0 {
            return false;
        }
    }

    let new_buf = unsafe { realloc(file.buffer as *mut c_void, new_cap) as *mut u8 };
    if new_buf.is_null() {
        return false;
    }

    file.buffer = new_buf;
    file.cap = new_cap;
    true
}

unsafe fn flush_kernel_file(file: *mut KernelFile) -> i32 {
    let file = unsafe { &mut *file };
    if file.mode != FILE_MODE_WRITE {
        return 0;
    }

    let mut result: u64 = u64::MAX;
    let buf_ptr = if file.len == 0 {
        core::ptr::null()
    } else {
        file.buffer as *const u8
    };
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") SYS_FS_WRITE,
            in("rdi") file.path as u64,
            in("rsi") buf_ptr as u64,
            in("rdx") file.len as u64,
            lateout("rax") result
        );
    }
    if result == u64::MAX { -1 } else { 0 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fopen(path: *const c_char, _mode: *const c_char) -> *mut c_void {
    unsafe {
        let mut norm_path_buf = [0u8; 512];
        let kernel_path = normalize_kernel_path(path, &mut norm_path_buf);

        print_str("[DOOM] fopen: ");
        let mut i = 0;
        while *path.add(i) != 0 {
            print_char(*path.add(i) as u8);
            i += 1;
        }

        let mode = if _mode.is_null() { b'r' } else { *_mode as u8 };
        if mode == b'w' || mode == b'a' {
            let file_ptr = malloc(core::mem::size_of::<KernelFile>()) as *mut KernelFile;
            if file_ptr.is_null() {
                print_str(" -> FAILED\n");
                return core::ptr::null_mut();
            }

            let path_copy = cstr_dup(kernel_path);
            if path_copy.is_null() {
                print_str(" -> FAILED\n");
                return core::ptr::null_mut();
            }

            file_ptr.write(KernelFile {
                mode: FILE_MODE_WRITE,
                handle: 0,
                path: path_copy,
                buffer: core::ptr::null_mut(),
                len: 0,
                cap: 0,
                pos: 0,
            });

            print_str(" -> SUCCESS (write)\n");
            return file_ptr as *mut c_void;
        }

        let mut handle: u64 = u64::MAX;
        core::arch::asm!("int 0x80", in("rax") SYS_FS_OPEN, in("rdi") kernel_path as u64, lateout("rax") handle);
        if handle == u64::MAX {
            let mut fallback = [0u8; 128];
            let mut j = 0;
            let mut k = 0;
            while *kernel_path.add(j) != 0 && k < 127 {
                let c = *kernel_path.add(j) as u8;
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
        let file_ptr = malloc(core::mem::size_of::<KernelFile>()) as *mut KernelFile;
        if file_ptr.is_null() {
            print_str(" -> FAILED\n");
            return core::ptr::null_mut();
        }
        file_ptr.write(KernelFile {
            mode: FILE_MODE_READ,
            handle,
            path: core::ptr::null_mut(),
            buffer: core::ptr::null_mut(),
            len: 0,
            cap: 0,
            pos: 0,
        });

        print_str(" -> SUCCESS (");
        print_num((handle + 1) as i64);
        print_str(")\n");
        file_ptr as *mut c_void
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fileno(_stream: *mut c_void) -> i32 {
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn isatty(_fd: i32) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn getenv(_name: *const c_char) -> *mut c_char {
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn feof(_stream: *mut c_void) -> i32 {
    // Treat files as EOF-driven by explicit read/fscanf behavior.
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_Quit() {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DEH_LoadFile(_filename: *mut c_char) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DEH_LoadLump(_lumpnum: i32, _allow_long: i32, _allow_error: i32) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DEH_LoadLumpByName(
    _name: *mut c_char,
    _allow_long: i32,
    _allow_error: i32,
) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fread(
    ptr: *mut c_void,
    size: usize,
    nmemb: usize,
    fp: *mut c_void,
) -> usize {
    if fp.is_null() {
        return 0;
    }
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    if file.mode != FILE_MODE_READ {
        return 0;
    }
    let handle = file.handle;
    let mut read: u64 = 0;
    unsafe {
        core::arch::asm!("int 0x80", in("rax") SYS_FS_READ_HANDLE, in("rdi") handle, in("rsi") ptr as u64, in("rdx") (size * nmemb) as u64, lateout("rax") read);
    }
    if read == u64::MAX {
        return 0;
    }
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
            if i == 0 {
                return core::ptr::null_mut();
            }
            break;
        }
        unsafe {
            *s.add(i as usize) = c as i8;
        }
        i += 1;
        if c == b'\n' as i32 {
            break;
        }
    }
    unsafe {
        *s.add(i as usize) = 0;
    }
    s
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fseek(fp: *mut c_void, offset: i64, whence: i32) -> i32 {
    if fp.is_null() {
        return -1;
    }
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    if file.mode == FILE_MODE_READ {
        let handle = file.handle;
        let mut res: u64 = 0;
        unsafe {
            core::arch::asm!("int 0x80", in("rax") SYS_FS_SEEK_HANDLE, in("rdi") handle, in("rsi") offset as u64, in("rdx") whence as u64, lateout("rax") res);
        }
        if res == u64::MAX { -1 } else { 0 }
    } else {
        let base = match whence {
            0 => 0isize,
            1 => file.pos as isize,
            2 => file.len as isize,
            _ => return -1,
        };
        let new_pos = base.saturating_add(offset as isize);
        if new_pos < 0 {
            return -1;
        }
        let new_pos = new_pos as usize;
        if new_pos > file.len {
            if !unsafe { file_reserve(fp as *mut KernelFile, new_pos) } {
                return -1;
            }
            unsafe {
                core::ptr::write_bytes(file.buffer.add(file.len), 0, new_pos - file.len);
            }
            file.len = new_pos;
        }
        file.pos = new_pos;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ftell(fp: *mut c_void) -> i64 {
    if fp.is_null() {
        return -1;
    }
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    if file.mode == FILE_MODE_READ {
        let handle = file.handle;
        let mut res: u64 = 0;
        unsafe {
            core::arch::asm!("int 0x80", in("rax") SYS_FS_SEEK_HANDLE, in("rdi") handle, in("rsi") 0u64, in("rdx") 1u64, lateout("rax") res);
        }
        res as i64
    } else {
        file.pos as i64
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fclose(fp: *mut c_void) -> i32 {
    if fp.is_null() {
        return -1;
    }
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    if file.mode == FILE_MODE_READ {
        unsafe {
            core::arch::asm!("int 0x80", in("rax") SYS_FS_CLOSE, in("rdi") file.handle);
        }
        0
    } else {
        unsafe { flush_kernel_file(fp as *mut KernelFile) }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fwrite(
    ptr: *const c_void,
    size: usize,
    nmemb: usize,
    stream: *mut c_void,
) -> usize {
    if stream.is_null() || ptr.is_null() || size == 0 || nmemb == 0 {
        return 0;
    }

    let file = unsafe { &mut *(stream as *mut KernelFile) };
    if file.mode != FILE_MODE_WRITE {
        return 0;
    }

    let bytes = size.saturating_mul(nmemb);
    let end = file.pos.saturating_add(bytes);
    if !unsafe { file_reserve(stream as *mut KernelFile, end) } {
        return 0;
    }

    unsafe {
        core::ptr::copy_nonoverlapping(ptr as *const u8, file.buffer.add(file.pos), bytes);
    }
    file.pos = end;
    if end > file.len {
        file.len = end;
    }

    nmemb
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fflush(stream: *mut c_void) -> i32 {
    if stream.is_null() {
        return -1;
    }
    unsafe { flush_kernel_file(stream as *mut KernelFile) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn remove(path: *const i8) -> i32 {
    let mut norm_path_buf = [0u8; 512];
    let kernel_path = unsafe { normalize_kernel_path(path, &mut norm_path_buf) };
    let mut res: u64 = u64::MAX;
    unsafe {
        core::arch::asm!("int 0x80", in("rax") SYS_FS_REMOVE, in("rdi") kernel_path as u64, lateout("rax") res);
    }
    if res == u64::MAX { -1 } else { 0 }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rename(old: *const i8, new: *const i8) -> i32 {
    let mut old_buf = [0u8; 512];
    let mut new_buf = [0u8; 512];
    let kernel_old = unsafe { normalize_kernel_path(old, &mut old_buf) };
    let kernel_new = unsafe { normalize_kernel_path(new, &mut new_buf) };
    let mut res: u64 = u64::MAX;
    unsafe {
        core::arch::asm!("int 0x80", in("rax") SYS_FS_RENAME, in("rdi") kernel_old as u64, in("rsi") kernel_new as u64, lateout("rax") res);
    }
    if res == u64::MAX { -1 } else { 0 }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mkdir(path: *const i8, _mode: u32) -> i32 {
    let mut norm_path_buf = [0u8; 512];
    let kernel_path = unsafe { normalize_kernel_path(path, &mut norm_path_buf) };
    let mut res: u64 = u64::MAX;
    unsafe {
        core::arch::asm!("int 0x80", in("rax") SYS_FS_MKDIR, in("rdi") kernel_path as u64, lateout("rax") res);
    }
    if res == u64::MAX { -1 } else { 0 }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn access(path: *const c_char, _mode: i32) -> i32 {
    unsafe {
        let fp = fopen(path, core::ptr::null());
        if fp.is_null() {
            -1
        } else {
            fclose(fp);
            0
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn exit(_status: i32) -> ! {
    unsafe {
        print_str("[DOOM] Exit called with status: ");
        print_num(_status as i64);
        print_char(b'\n');
    }
    loop {
        unsafe {
            core::arch::asm!("int 0x80", in("rax") 2u64, in("rdi") _status as u64);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sscanf(mut str: *const i8, mut fmt: *const i8, mut ap: ...) -> i32 {
    unsafe {
        let mut count = 0;
        loop {
            let f = *fmt as u8;
            if f == 0 {
                break;
            }
            if f.is_ascii_whitespace() {
                while (*str as u8).is_ascii_whitespace() {
                    str = str.add(1);
                }
            } else if f == b'%' {
                fmt = fmt.add(1);
                match *fmt as u8 {
                    b's' => {
                        let dest = ap.arg::<*mut i8>();
                        while (*str as u8).is_ascii_whitespace() {
                            str = str.add(1);
                        }
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
                        while (*str as u8).is_ascii_whitespace() {
                            str = str.add(1);
                        }
                        let mut val = 0;
                        let mut sign = 1;
                        if *str == b'-' as i8 {
                            sign = -1;
                            str = str.add(1);
                        }
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
                if *str != *fmt {
                    break;
                }
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
        if fgets(buf.as_mut_ptr(), 512, fp).is_null() {
            return -1;
        }

        print_str("[DOOM] fscanf read line: ");
        let mut i = 0;
        while buf[i] != 0 {
            print_char(buf[i] as u8);
            i += 1;
        }

        let mut str = buf.as_ptr();
        let mut count = 0;
        loop {
            let f = *fmt as u8;
            if f == 0 {
                break;
            }
            if f.is_ascii_whitespace() {
                while (*str as u8).is_ascii_whitespace() {
                    str = str.add(1);
                }
            } else if f == b'%' {
                fmt = fmt.add(1);
                match *fmt as u8 {
                    b's' => {
                        let dest = ap.arg::<*mut i8>();
                        while (*str as u8).is_ascii_whitespace() {
                            str = str.add(1);
                        }
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
                        while (*str as u8).is_ascii_whitespace() {
                            str = str.add(1);
                        }
                        let mut val = 0;
                        let mut sign = 1;
                        if *str == b'-' as i8 {
                            sign = -1;
                            str = str.add(1);
                        }
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
                if *str != *fmt {
                    break;
                }
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
        if n_len == 0 {
            return haystack as *mut i8;
        }
        let mut h = haystack;
        while *h != 0 {
            if strncmp(h, needle, n_len) == 0 {
                return h as *mut i8;
            }
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
        while (*s.add(i) as u8).is_ascii_whitespace() {
            i += 1;
        }
        let sign = if *s.add(i) == b'-' as i8 {
            i += 1;
            -1
        } else {
            1
        };
        while *s.add(i) >= b'0' as i8 && *s.add(i) <= b'9' as i8 {
            res = res * 10 + (*s.add(i) - b'0' as i8) as i32;
            i += 1;
        }
        res * sign
    }
}

unsafe fn parse_format_spec(mut fptr: *const i8) -> (*const i8, i32, i32, bool) {
    let mut width: i32 = 0;
    let mut precision: i32 = -1;
    let mut zero_pad = false;

    if unsafe { *fptr } == b'0' as i8 {
        zero_pad = true;
        fptr = unsafe { fptr.add(1) };
    }

    while unsafe { *fptr } >= b'0' as i8 && unsafe { *fptr } <= b'9' as i8 {
        width = width * 10 + (unsafe { *fptr } - b'0' as i8) as i32;
        fptr = unsafe { fptr.add(1) };
    }

    if unsafe { *fptr } == b'.' as i8 {
        precision = 0;
        fptr = unsafe { fptr.add(1) };
        while unsafe { *fptr } >= b'0' as i8 && unsafe { *fptr } <= b'9' as i8 {
            precision = precision * 10 + (unsafe { *fptr } - b'0' as i8) as i32;
            fptr = unsafe { fptr.add(1) };
        }
    }

    // Ignore basic length modifiers used by Doom's C code.
    while unsafe { *fptr } == b'l' as i8
        || unsafe { *fptr } == b'h' as i8
        || unsafe { *fptr } == b'z' as i8
    {
        fptr = unsafe { fptr.add(1) };
    }

    (fptr, width, precision, zero_pad)
}

unsafe fn vsprintf_core(buf: *mut i8, n: usize, fmt: *const i8, mut ap: core::ffi::VaList) -> i32 {
    let mut fptr = fmt;
    let mut written = 0;

    macro_rules! write_char {
        ($c:expr) => {
            if (written as usize) < n.saturating_sub(1) {
                unsafe {
                    *buf.add(written as usize) = $c as i8;
                }
                written += 1;
            }
        };
    }

    loop {
        let c = unsafe { *fptr as u8 };
        if c == 0 {
            break;
        }
        if c == b'%' {
            fptr = unsafe { fptr.add(1) };
            if unsafe { *fptr } == b'%' as i8 {
                write_char!(b'%');
                fptr = unsafe { fptr.add(1) };
                continue;
            }

            let (spec_ptr, width, precision, zero_pad) = unsafe { parse_format_spec(fptr) };
            fptr = spec_ptr;

            match unsafe { *fptr as u8 } {
                b's' => {
                    let s = unsafe { ap.arg::<*const i8>() };
                    if !s.is_null() {
                        let mut j = 0;
                        while unsafe { *s.add(j) } != 0 {
                            write_char!(unsafe { *s.add(j) as u8 });
                            j += 1;
                        }
                    } else {
                        for b in b"(null)".iter() {
                            write_char!(*b);
                        }
                    }
                }
                b'd' | b'i' => {
                    let mut val = unsafe { ap.arg::<i32>() } as i64;
                    let negative = val < 0;
                    if val < 0 {
                        val = -val;
                    }
                    let mut tmp = [0u8; 20];
                    let mut ti = 0;
                    if val == 0 {
                        tmp[ti] = b'0';
                        ti += 1;
                    }
                    while val > 0 {
                        tmp[ti] = (val % 10) as u8 + b'0';
                        val /= 10;
                        ti += 1;
                    }

                    let zero_count = if precision >= 0 {
                        core::cmp::max(0, precision as isize - ti as isize) as usize
                    } else if zero_pad && width > (ti as i32 + if negative { 1 } else { 0 }) {
                        (width as usize).saturating_sub(ti + if negative { 1 } else { 0 })
                    } else {
                        0
                    };

                    let total_len = ti + zero_count + if negative { 1 } else { 0 };
                    let space_count = if width > total_len as i32 {
                        (width as usize).saturating_sub(total_len)
                    } else {
                        0
                    };

                    for _ in 0..space_count {
                        write_char!(b' ');
                    }
                    if negative {
                        write_char!(b'-');
                    }
                    for _ in 0..zero_count {
                        write_char!(b'0');
                    }
                    for j in (0..ti).rev() {
                        write_char!(tmp[j]);
                    }
                }
                b'u' => {
                    let mut val = unsafe { ap.arg::<u32>() as u64 };
                    let mut tmp = [0u8; 20];
                    let mut ti = 0;
                    if val == 0 {
                        tmp[ti] = b'0';
                        ti += 1;
                    }
                    while val > 0 {
                        tmp[ti] = (val % 10) as u8 + b'0';
                        val /= 10;
                        ti += 1;
                    }

                    let zero_count = if precision >= 0 {
                        core::cmp::max(0, precision as isize - ti as isize) as usize
                    } else if zero_pad && width > ti as i32 {
                        (width as usize).saturating_sub(ti)
                    } else {
                        0
                    };

                    let total_len = ti + zero_count;
                    let space_count = if width > total_len as i32 {
                        (width as usize).saturating_sub(total_len)
                    } else {
                        0
                    };

                    for _ in 0..space_count {
                        write_char!(b' ');
                    }
                    for _ in 0..zero_count {
                        write_char!(b'0');
                    }
                    for j in (0..ti).rev() {
                        write_char!(tmp[j]);
                    }
                }
                b'x' | b'p' => {
                    let val = if unsafe { *fptr == b'p' as i8 } {
                        unsafe { ap.arg::<u64>() }
                    } else {
                        unsafe { ap.arg::<u32>() as u64 }
                    };
                    let mut tmp = [0u8; 16];
                    let mut ti = 0;
                    let mut v = val;
                    if v == 0 {
                        tmp[ti] = b'0';
                        ti += 1;
                    }
                    while v > 0 {
                        let hex = (v & 0xF) as u8;
                        tmp[ti] = if hex < 10 {
                            b'0' + hex
                        } else {
                            b'a' + (hex - 10)
                        };
                        v >>= 4;
                        ti += 1;
                    }

                    let zero_count = if precision >= 0 {
                        core::cmp::max(0, precision as isize - ti as isize) as usize
                    } else if zero_pad && width > ti as i32 {
                        (width as usize).saturating_sub(ti)
                    } else {
                        0
                    };

                    let total_len = ti + zero_count;
                    let space_count = if width > total_len as i32 {
                        (width as usize).saturating_sub(total_len)
                    } else {
                        0
                    };

                    for _ in 0..space_count {
                        write_char!(b' ');
                    }
                    for _ in 0..zero_count {
                        write_char!(b'0');
                    }
                    for j in (0..ti).rev() {
                        write_char!(tmp[j]);
                    }
                }
                _ => {
                    write_char!(b'%');
                    write_char!(unsafe { *fptr as u8 });
                }
            }
        } else {
            write_char!(c);
        }
        fptr = unsafe { fptr.add(1) };
    }
    if n > 0 {
        unsafe {
            *buf.add(written as usize) = 0;
        }
    }
    written
}

unsafe fn vprintf_core(fmt: *const i8, mut ap: core::ffi::VaList) {
    let mut fptr = fmt;
    loop {
        let c = unsafe { *fptr as u8 };
        if c == 0 {
            break;
        }
        if c == b'%' {
            fptr = unsafe { fptr.add(1) };
            if unsafe { *fptr } == b'%' as i8 {
                print_char(b'%');
                fptr = unsafe { fptr.add(1) };
                continue;
            }

            let (spec_ptr, _width, _precision, _zero_pad) = unsafe { parse_format_spec(fptr) };
            fptr = spec_ptr;

            match unsafe { *fptr as u8 } {
                b's' => {
                    let s = unsafe { ap.arg::<*const i8>() };
                    if !s.is_null() {
                        let mut j = 0;
                        while unsafe { *s.add(j) } != 0 {
                            print_char(unsafe { *s.add(j) as u8 });
                            j += 1;
                        }
                    } else {
                        print_str("(null)");
                    }
                }
                b'd' | b'i' => {
                    print_num(unsafe { ap.arg::<i32>() } as i64);
                }
                b'u' => {
                    print_num(unsafe { ap.arg::<u32>() } as i64);
                }
                b'x' | b'p' => {
                    let n = if unsafe { *fptr == b'p' as i8 } {
                        unsafe { ap.arg::<u64>() }
                    } else {
                        unsafe { ap.arg::<u32>() as u64 }
                    };
                    let mut started = false;
                    for j in 0..16 {
                        let hex = (n >> (60 - j * 4)) & 0xF;
                        if hex != 0 || started || j == 15 {
                            started = true;
                            let b = if hex < 10 {
                                hex as u8 + b'0'
                            } else {
                                hex as u8 - 10 + b'a'
                            };
                            print_char(b);
                        }
                    }
                }
                _ => {
                    print_char(b'%');
                    print_char(unsafe { *fptr as u8 });
                }
            }
        } else {
            print_char(c);
        }
        fptr = unsafe { fptr.add(1) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn printf(fmt: *const i8, mut ap: ...) -> i32 {
    unsafe {
        vprintf_core(fmt, ap);
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf(_s: *mut c_void, fmt: *const i8, ap: core::ffi::VaList) -> i32 {
    unsafe {
        vprintf_core(fmt, ap);
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsprintf(s: *mut i8, fmt: *const i8, ap: core::ffi::VaList) -> i32 {
    unsafe { vsnprintf(s, 1024, fmt, ap) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fprintf(_s: *mut c_void, fmt: *const i8, mut ap: ...) -> i32 {
    unsafe {
        vprintf_core(fmt, ap);
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn snprintf(s: *mut i8, n: usize, fmt: *const i8, mut ap: ...) -> i32 {
    unsafe { vsnprintf(s, n, fmt, ap) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsnprintf(
    s: *mut i8,
    n: usize,
    fmt: *const i8,
    ap: core::ffi::VaList,
) -> i32 {
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
            if ch as i32 == c {
                return s.add(i) as *mut i8;
            }
            if ch == 0 {
                return core::ptr::null_mut();
            }
            i += 1;
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn puts(s: *const i8) -> i32 {
    unsafe {
        let mut i = 0;
        while *s.add(i) != 0 {
            print_char(*s.add(i) as u8);
            i += 1;
        }
        print_char(b'\n');
    }
    0
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn system(_c: *const i8) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn strlen(s: *const i8) -> usize {
    let mut len = 0;
    unsafe {
        while *s.add(len) != 0 {
            len += 1;
        }
    }
    len
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcmp(s1: *const i8, s2: *const i8) -> i32 {
    unsafe {
        let mut i = 0;
        loop {
            let c1 = *s1.add(i) as u8;
            let c2 = *s2.add(i) as u8;
            if c1 != c2 || c1 == 0 {
                return (c1 as i32) - (c2 as i32);
            }
            i += 1;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncpy(d: *mut i8, s: *const i8, n: usize) -> *mut i8 {
    unsafe {
        let mut i = 0;
        while i < n && *s.add(i) != 0 {
            *d.add(i) = *s.add(i);
            i += 1;
        }
        while i < n {
            *d.add(i) = 0;
            i += 1;
        }
        d
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strrchr(s: *const i8, c: i32) -> *mut i8 {
    unsafe {
        let mut last = core::ptr::null_mut();
        let mut i = 0;
        while *s.add(i) != 0 {
            if *s.add(i) == c as i8 {
                last = s.add(i) as *mut i8;
            }
            i += 1;
        }
        if c == 0 { s.add(i) as *mut i8 } else { last }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncmp(s1: *const i8, s2: *const i8, n: usize) -> i32 {
    unsafe {
        for i in 0..n {
            let c1 = *s1.add(i) as u8;
            let c2 = *s2.add(i) as u8;
            if c1 != c2 || c1 == 0 {
                return (c1 as i32) - (c2 as i32);
            }
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
            if c1 != c2 || c1 == 0 {
                return (c1 as i32) - (c2 as i32);
            }
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
            if c1 != c2 || c1 == 0 {
                return (c1 as i32) - (c2 as i32);
            }
        }
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn toupper(c: i32) -> i32 {
    (c as u8).to_ascii_uppercase() as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn tolower(c: i32) -> i32 {
    (c as u8).to_ascii_lowercase() as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn isspace(c: i32) -> i32 {
    if c == b' ' as i32 || c == b'\t' as i32 || c == b'\n' as i32 || c == b'\r' as i32 {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(d: *mut u8, s: *const u8, n: usize) -> *mut u8 {
    unsafe {
        core::ptr::copy(s, d, n);
        d
    }
}

// =============================================================================
// DOOMGENERIC INTERFACE
// =============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn DG_Init() {}

static mut HAVE_E0_PREFIX: bool = false;

fn scancode_to_doom_key(scancode: u8, extended: bool) -> Option<u8> {
    Some(match (extended, scancode) {
        (_, 0x01) => 27,
        (_, 0x02) => b'1',
        (_, 0x03) => b'2',
        (_, 0x04) => b'3',
        (_, 0x05) => b'4',
        (_, 0x06) => b'5',
        (_, 0x07) => b'6',
        (_, 0x08) => b'7',
        (_, 0x09) => b'8',
        (_, 0x0A) => b'9',
        (_, 0x0B) => b'0',
        (_, 0x0C) => b'-',
        (_, 0x0D) => b'=',
        (_, 0x0E) => 0x7f,
        (_, 0x0F) => b'\t',
        (_, 0x10) => b'q',
        (_, 0x11) => b'w',
        (_, 0x12) => b'e',
        (_, 0x13) => b'r',
        (_, 0x14) => b't',
        (_, 0x15) => b'y',
        (_, 0x16) => b'u',
        (_, 0x17) => b'i',
        (_, 0x18) => b'o',
        (_, 0x19) => b'p',
        (_, 0x1A) => b'[',
        (_, 0x1B) => b']',
        (_, 0x1C) => 13,
        (false, 0x1D) | (true, 0x1D) => 0xa3,
        (_, 0x1E) => b'a',
        (_, 0x1F) => b's',
        (_, 0x20) => b'd',
        (_, 0x21) => b'f',
        (_, 0x22) => b'g',
        (_, 0x23) => b'h',
        (_, 0x24) => b'j',
        (_, 0x25) => b'k',
        (_, 0x26) => b'l',
        (_, 0x27) => b';',
        (_, 0x28) => b'\'',
        (_, 0x29) => b'`',
        (_, 0x2A) | (_, 0x36) => 0xb6,
        (_, 0x2B) => b'\\',
        (_, 0x2C) => b'z',
        (_, 0x2D) => b'x',
        (_, 0x2E) => b'c',
        (_, 0x2F) => b'v',
        (_, 0x30) => b'b',
        (_, 0x31) => b'n',
        (_, 0x32) => b'm',
        (_, 0x33) => b',',
        (_, 0x34) => b'.',
        (_, 0x35) => b'/',
        (_, 0x37) => b'*',
        (_, 0x38) => 0xb8,
        (_, 0x39) => 0xa2,
        (_, 0x3A) => 0xba,
        (_, 0x3B) => 0xbb,
        (_, 0x3C) => 0xbc,
        (_, 0x3D) => 0xbd,
        (_, 0x3E) => 0xbe,
        (_, 0x3F) => 0xbf,
        (_, 0x40) => 0xc0,
        (_, 0x41) => 0xc1,
        (_, 0x42) => 0xc2,
        (_, 0x43) => 0xc3,
        (_, 0x44) => 0xc4,
        (_, 0x45) => 0xc5,
        (_, 0x47) => 0xc7,
        (_, 0x48) => 0xad,
        (_, 0x49) => 0xc9,
        (_, 0x4A) => b'-',
        (_, 0x4B) => 0xac,
        (_, 0x4D) => 0xae,
        (_, 0x4E) => b'+',
        (_, 0x4F) => 0xcf,
        (_, 0x50) => 0xaf,
        (_, 0x51) => 0xd1,
        (_, 0x52) => 0xd2,
        (_, 0x53) => 0xd3,
        (_, 0x57) => 0xd7,
        (_, 0x58) => 0xd8,
        _ => return None,
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DG_DrawFrame() {
    unsafe {
        if DG_ScreenBuffer.is_null() {
            return;
        }
        let arg2 = ((DG_Height as u64) << 32) | (DG_Width as u64);
        core::arch::asm!("int 0x80", in("rax") SYS_DRAW_BUFFER, in("rdi") DG_ScreenBuffer as u64, in("rsi") arg2);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn DG_SleepMs(ms: u32) {
    for _ in 0..(ms * 1000) {
        unsafe {
            core::arch::asm!("pause");
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn DG_GetTicksMs() -> u32 {
    let mut res: u64 = 0;
    unsafe {
        core::arch::asm!("int 0x80", in("rax") SYS_GET_UPTIME, lateout("rax") res);
    }
    res as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DG_GetKey(p: *mut i32, k: *mut u8) -> i32 {
    unsafe {
        loop {
            let mut v: u64 = 0;
            core::arch::asm!("int 0x80", in("rax") SYS_GET_SCANCODE, lateout("rax") v);
            let raw = v as u8;

            if raw == 0 {
                return 0;
            }

            if raw == 0xE0 {
                HAVE_E0_PREFIX = true;
                continue;
            }

            let released = (raw & 0x80) != 0;
            let scancode = raw & 0x7F;
            let extended = HAVE_E0_PREFIX;
            HAVE_E0_PREFIX = false;

            if let Some(doom_key) = scancode_to_doom_key(scancode, extended) {
                *p = if released { 0 } else { 1 };
                *k = doom_key;
                return 1;
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn DG_SetWindowTitle(_t: *const c_char) {}
#[unsafe(no_mangle)]
pub extern "C" fn DG_BeginFrame() {}
#[unsafe(no_mangle)]
pub extern "C" fn DG_EndFrame() {}

// =============================================================================
// ENTRY POINT
// =============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    unsafe {
        core::arch::asm!("int 0x80", in("rax") SYS_ENTER_EXCLUSIVE_GRAPHICS);
        DG_ScreenBuffer = core::ptr::addr_of_mut!(FRAMEBUFFER) as *mut u32;
        let argv = [
            "doom\0".as_ptr() as *const i8,
            "DOOM.WAD\0".as_ptr() as *const i8,
            core::ptr::null(),
        ];
        doomgeneric_Create(2, argv.as_ptr());
        loop {
            doomgeneric_Tick();
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        core::arch::asm!("int 0x80", in("rax") SYS_EXIT_EXCLUSIVE_GRAPHICS);
    }
    loop {}
}
