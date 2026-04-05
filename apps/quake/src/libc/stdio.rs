use core::ffi::{c_char, c_void, VaList};
use crate::{VaListC, print_char};
use rustos_user::{
    SYS_FS_CLOSE, SYS_FS_OPEN, SYS_FS_READ_HANDLE, SYS_FS_SEEK_HANDLE,
    syscall1, syscall3,
};

#[repr(C)]
#[derive(Copy, Clone)]
struct KernelFile {
    used: bool,
    handle: u64,
}

const MAX_FILES: usize = 32;
static mut FILES: [KernelFile; MAX_FILES] = [KernelFile { used: false, handle: 0 }; MAX_FILES];

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fopen(path: *const c_char, _mode: *const c_char) -> *mut c_void {
    if path.is_null() { return core::ptr::null_mut(); }
    let handle = syscall1(SYS_FS_OPEN, path as u64);
    if handle == u64::MAX { return core::ptr::null_mut(); }

    unsafe {
        for i in 0..MAX_FILES {
            if !FILES[i].used {
                FILES[i].used = true;
                FILES[i].handle = handle;
                return &mut FILES[i] as *mut KernelFile as *mut c_void;
            }
        }
    }
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fclose(fp: *mut c_void) -> i32 {
    if fp.is_null() { return -1; }
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    let _ = syscall1(SYS_FS_CLOSE, file.handle);
    file.used = false;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fread(ptr: *mut c_void, size: usize, nmemb: usize, fp: *mut c_void) -> usize {
    if ptr.is_null() || fp.is_null() || size == 0 { return 0; }
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    let n = syscall3(SYS_FS_READ_HANDLE, file.handle, ptr as u64, (size * nmemb) as u64);
    if n == u64::MAX { 0 } else { (n as usize) / size }
}

#[unsafe(no_mangle)] pub unsafe extern "C" fn fwrite(_: *const c_void, _: usize, _: usize, _: *mut c_void) -> usize { 0 }

#[unsafe(no_mangle)] pub unsafe extern "C" fn fseek(fp: *mut c_void, off: i64, wh: i32) -> i32 {
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    let res = syscall3(SYS_FS_SEEK_HANDLE, file.handle, off as u64, wh as u64);
    if res == u64::MAX { -1 } else { 0 }
}

#[unsafe(no_mangle)] pub unsafe extern "C" fn ftell(fp: *mut c_void) -> i64 {
    let file = unsafe { &mut *(fp as *mut KernelFile) };
    let res = syscall3(SYS_FS_SEEK_HANDLE, file.handle, 0, 1);
    if res == u64::MAX { -1 } else { res as i64 }
}

#[unsafe(no_mangle)] pub unsafe extern "C" fn fgetc(s: *mut c_void) -> i32 {
    let mut c = 0u8;
    if unsafe { fread(&mut c as *mut u8 as *mut c_void, 1, 1, s) == 1 } { c as i32 } else { -1 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn getc(fp: *mut c_void) -> i32 {
    unsafe { fgetc(fp) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn unlink(_path: *const c_char) -> i32 {
    0
}

static mut ERRNO: i32 = 0;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __errno_location() -> *mut i32 {
    &raw mut ERRNO
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strerror(_err: i32) -> *const c_char {
    c"error".as_ptr()
}

// Formatting engine
unsafe fn write_unum_base(mut v: u64, base: u32, upper: bool, out: *mut i8) -> i32 {
    let mut buf = [0u8; 64];
    let mut i = 0;
    if v == 0 {
        unsafe { *out = b'0' as i8; }
        return 1;
    }
    let chars = if upper { b"0123456789ABCDEF" } else { b"0123456789abcdef" };
    while v > 0 {
        buf[i] = chars[(v % base as u64) as usize];
        v /= base as u64;
        i += 1;
    }
    for j in 0..i {
        unsafe { *out.add(j) = buf[i - 1 - j] as i8 };
    }
    i as i32
}

unsafe fn write_num(v: i64, out: *mut i8) -> i32 {
    if v < 0 {
        unsafe { *out = b'-' as i8 };
        1 + unsafe { write_unum_base(-(v as i128) as u64, 10, false, out.add(1)) }
    } else {
        unsafe { write_unum_base(v as u64, 10, false, out) }
    }
}

unsafe fn write_float(mut f: f64, out: *mut c_char, precision: i32) -> i32 {
    let mut total = 0;
    if f < 0.0 {
        unsafe { *out = b'-' as i8 };
        total += 1;
        f = -f;
    }
    let ipart = f as u64;
    let len = unsafe { write_unum_base(ipart, 10, false, out.add(total as usize)) };
    total += len;
    
    if precision > 0 {
        unsafe { *out.add(total as usize) = b'.' as i8 };
        total += 1;
        let mut frac = f - ipart as f64;
        for _ in 0..precision {
            frac *= 10.0;
            let d = frac as u8;
            unsafe { *out.add(total as usize) = (b'0' + d) as i8 };
            total += 1;
            frac -= d as f64;
        }
    }
    total
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf_internal(
    _fp: *mut c_void,
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
            } else {
                print_char(ch);
            }
            total += 1;
        }};
    }

    loop {
        let c = unsafe { *f as u8 };
        if c == 0 { break; }
        if c != b'%' {
            write_char!(c);
            f = unsafe { f.add(1) };
            continue;
        }

        f = unsafe { f.add(1) };
        let mut width = 0i32;
        let mut pad_char = b' ';
        if unsafe { *f as u8 } == b'0' {
            pad_char = b'0';
            f = unsafe { f.add(1) };
        }
        while unsafe { *f as u8 >= b'0' && *f as u8 <= b'9' } {
            width = width * 10 + (unsafe { *f as u8 } - b'0') as i32;
            f = unsafe { f.add(1) };
        }
        while unsafe { matches!(*f as u8, b'-' | b'+' | b' ' | b'#' | b'.' | b'1'..=b'9') } {
            f = unsafe { f.add(1) };
        }

        let mut long_mod = false;
        if unsafe { *f as u8 } == b'l' {
            long_mod = true;
            f = unsafe { f.add(1) };
            if unsafe { *f as u8 } == b'l' { f = unsafe { f.add(1) }; }
        }

        match unsafe { *f as u8 } {
            b'%' => write_char!(b'%'),
            b'c' => {
                let val = unsafe { crate::next_gp::<i32>(ap) } as u8;
                write_char!(val);
            }
            b's' => {
                let s = unsafe { crate::next_gp::<*const c_char>(ap) };
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
                let mut tmp = [0i8; 32];
                let n = if long_mod { unsafe { crate::next_gp::<i64>(ap) } } else { (unsafe { crate::next_gp::<i32>(ap) }) as i64 };
                let len = unsafe { write_num(n, tmp.as_mut_ptr()) };
                for _ in 0..(width - len) { write_char!(pad_char); }
                for i in 0..len { write_char!(tmp[i as usize] as u8); }
            }
            b'u' => {
                let mut tmp = [0i8; 32];
                let n = if long_mod { unsafe { crate::next_gp::<u64>(ap) } } else { (unsafe { crate::next_gp::<u32>(ap) }) as u64 };
                let len = unsafe { write_unum_base(n, 10, false, tmp.as_mut_ptr()) };
                for _ in 0..(width - len) { write_char!(pad_char); }
                for i in 0..len { write_char!(tmp[i as usize] as u8); }
            }
            b'x' | b'X' => {
                let upper = unsafe { *f as u8 } == b'X';
                let mut tmp = [0i8; 32];
                let n = if long_mod { unsafe { crate::next_gp::<u64>(ap) } } else { (unsafe { crate::next_gp::<u32>(ap) }) as u64 };
                let len = unsafe { write_unum_base(n, 16, upper, tmp.as_mut_ptr()) };
                for i in 0..len { write_char!(tmp[i as usize] as u8); }
            }
            b'p' => {
                let n = unsafe { crate::next_gp::<usize>(ap) } as u64;
                write_char!(b'0'); write_char!(b'x');
                let mut tmp = [0i8; 32];
                let len = unsafe { write_unum_base(n, 16, false, tmp.as_mut_ptr()) };
                for i in 0..len { write_char!(tmp[i as usize] as u8); }
            }
            b'f' => {
                let val = unsafe { crate::next_fp::<f64>(ap) };
                let mut tmp = [0i8; 64];
                let len = unsafe { write_float(val, tmp.as_mut_ptr(), 2) };
                for i in 0..len { write_char!(tmp[i as usize] as u8); }
            }
            _ => {
                write_char!(b'%');
                write_char!(unsafe { *f as u8 });
            }
        }
        f = unsafe { f.add(1) };
    }
    if !dst.is_null() { unsafe { *out = 0 }; }
    total
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn feof(_fp: *mut c_void) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fflush(_fp: *mut c_void) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fprintf(fp: *mut c_void, fmt: *const c_char, mut ap: ...) -> i32 {
    let ap_c = unsafe { core::mem::transmute::<&mut VaList, &mut VaListC>(&mut ap) };
    unsafe { vfprintf_internal(fp, core::ptr::null_mut(), fmt, ap_c) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __isoc99_fscanf(_fp: *mut c_void, _fmt: *const c_char, _: ...) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn open(_path: *const c_char, _flags: i32, _: ...) -> i32 {
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn write(_fd: i32, _buf: *const c_void, _count: usize) -> isize {
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn close(_fd: i32) -> i32 {
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn printf(fmt: *const c_char, mut ap: ...) -> i32 {
    let ap_c = unsafe { core::mem::transmute::<&mut VaList, &mut VaListC>(&mut ap) };
    unsafe { vfprintf_internal(core::ptr::null_mut(), core::ptr::null_mut(), fmt, ap_c) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vprintf(fmt: *const c_char, mut ap: VaList) -> i32 {
    let ap_c = unsafe { core::mem::transmute::<&mut VaList, &mut VaListC>(&mut ap) };
    unsafe { vfprintf_internal(core::ptr::null_mut(), core::ptr::null_mut(), fmt, ap_c) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sprintf(dst: *mut c_char, fmt: *const c_char, mut ap: ...) -> i32 {
    let ap_c = unsafe { core::mem::transmute::<&mut VaList, &mut VaListC>(&mut ap) };
    unsafe { vfprintf_internal(core::ptr::null_mut(), dst, fmt, ap_c) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsprintf(dst: *mut c_char, fmt: *const c_char, mut ap: VaList) -> i32 {
    let ap_c = unsafe { core::mem::transmute::<&mut VaList, &mut VaListC>(&mut ap) };
    unsafe { vfprintf_internal(core::ptr::null_mut(), dst, fmt, ap_c) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf(fp: *mut c_void, fmt: *const c_char, mut ap: VaList) -> i32 {
    let ap_c = unsafe { core::mem::transmute::<&mut VaList, &mut VaListC>(&mut ap) };
    unsafe { vfprintf_internal(fp, core::ptr::null_mut(), fmt, ap_c) }
}
