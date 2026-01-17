use crate::{println, timer};
use core::{
    ffi::{CStr, VaList, c_char, c_int, c_void},
    time,
};

// --- NETWORKING ---
#[unsafe(no_mangle)]
pub static mut net_client_connected: i32 = 0;

#[unsafe(no_mangle)]
pub static mut errno: c_int = 0;

#[unsafe(no_mangle)]
pub static mut drone: i32 = 0;

unsafe fn format_print(fmt: *const c_char, va_list: &mut VaList) {
    if fmt.is_null() {
        return;
    }
    let fmt_bytes = CStr::from_ptr(fmt).to_bytes();
    let mut i = 0;

    while i < fmt_bytes.len() {
        if fmt_bytes[i] == b'%' && i + 1 < fmt_bytes.len() {
            i += 1;
            match fmt_bytes[i] {
                b's' => {
                    let ptr = va_list.arg::<*const c_char>();
                    if !ptr.is_null() {
                        let s = CStr::from_ptr(ptr).to_string_lossy();
                        crate::print!("{}", s);
                    } else {
                        crate::print!("(null)");
                    }
                }
                b'd' | b'i' => {
                    crate::print!("{}", va_list.arg::<i32>());
                }
                b'u' => {
                    crate::print!("{}", va_list.arg::<u32>());
                }
                b'x' | b'p' => {
                    crate::print!("{:#x}", va_list.arg::<usize>());
                }
                b'c' => {
                    crate::print!("{}", va_list.arg::<i32>() as u8 as char);
                }
                b'%' => {
                    crate::print!("%");
                }
                _ => {
                    crate::print!("%{}", fmt_bytes[i] as char);
                }
            }
        } else {
            crate::print!("{}", fmt_bytes[i] as char);
        }
        i += 1;
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn printf(fmt: *const c_char, mut args: ...) -> i32 {
    // 2. Pass it by reference to our helper
    format_print(fmt, &mut args);
    0
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn putchar(c: i32) -> i32 {
    c
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn puts(_s: *const c_char) -> i32 {
    0
}

#[unsafe(no_mangle)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn snprintf(
    s: *mut c_char,
    n: usize, // DON'T IGNORE THIS!
    fmt: *const c_char,
    mut args: ...
) -> i32 {
    if s.is_null() || fmt.is_null() || n == 0 {
        return -1;
    }

    let mut va = args;
    let fmt_bytes = CStr::from_ptr(fmt).to_bytes();
    let mut write_ptr = s;
    let mut written = 0usize;

    let mut i = 0;
    while i < fmt_bytes.len() && written < n - 1 {
        // Leave room for null terminator
        if fmt_bytes[i] == b'%' && i + 1 < fmt_bytes.len() {
            i += 1;
            match fmt_bytes[i] {
                b's' => {
                    let src_ptr = va.arg::<*const c_char>();
                    if !src_ptr.is_null() {
                        let src = CStr::from_ptr(src_ptr).to_bytes();
                        for &b in src {
                            if written >= n - 1 {
                                break;
                            }
                            write_ptr.write(b as c_char);
                            write_ptr = write_ptr.add(1);
                            written += 1;
                        }
                    }
                }
                b'd' | b'i' => {
                    // Format integer to string
                    let val = va.arg::<i32>();
                    let mut buf = [0u8; 16];
                    let mut temp_written = 0;

                    // Simple integer to string conversion
                    let is_neg = val < 0;
                    let mut abs_val = if is_neg {
                        val.wrapping_neg() as u32
                    } else {
                        val as u32
                    };

                    if abs_val == 0 {
                        buf[temp_written] = b'0';
                        temp_written += 1;
                    } else {
                        let mut digits = [0u8; 16];
                        let mut digit_count = 0;
                        while abs_val > 0 {
                            digits[digit_count] = (abs_val % 10) as u8 + b'0';
                            abs_val /= 10;
                            digit_count += 1;
                        }
                        if is_neg {
                            buf[temp_written] = b'-';
                            temp_written += 1;
                        }
                        for j in (0..digit_count).rev() {
                            buf[temp_written] = digits[j];
                            temp_written += 1;
                        }
                    }

                    for j in 0..temp_written {
                        if written >= n - 1 {
                            break;
                        }
                        write_ptr.write(buf[j] as c_char);
                        write_ptr = write_ptr.add(1);
                        written += 1;
                    }
                }
                _ => {
                    if written < n - 1 {
                        write_ptr.write(fmt_bytes[i] as c_char);
                        write_ptr = write_ptr.add(1);
                        written += 1;
                    }
                }
            }
        } else {
            write_ptr.write(fmt_bytes[i] as c_char);
            write_ptr = write_ptr.add(1);
            written += 1;
        }
        i += 1;
    }

    write_ptr.write(0); // Null terminator
    written as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sscanf(_s: *const c_char, _fmt: *const c_char, ...) -> i32 {
    0
}

// --- STRING FUNCTIONS ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    if s.is_null() {
        return 0;
    }
    let mut len = 0;
    // Safety: limit the search to 4096 bytes so we don't hang if there's no null
    while len < 4096 && *s.add(len) != 0 {
        len += 1;
    }
    len
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcpy(dest: *mut c_char, src: *const c_char) -> *mut c_char {
    let mut i = 0;
    while unsafe { *src.add(i) } != 0 {
        unsafe { *dest.add(i) = *src.add(i) };
        i += 1;
    }
    unsafe { *dest.add(i) = 0 };
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncpy(dest: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
    let mut i = 0;
    while i < n {
        let b = *src.add(i);
        *dest.add(i) = b;
        if b == 0 {
            // Fill the rest with nulls per C spec
            for j in i..n {
                *dest.add(j) = 0;
            }
            break;
        }
        i += 1;
    }
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcmp(s1: *const c_char, s2: *const c_char) -> i32 {
    let mut i = 0;
    loop {
        let b1 = unsafe { *s1.add(i) } as u8;
        let b2 = unsafe { *s2.add(i) } as u8;
        if b1 != b2 {
            return (b1 as i32) - (b2 as i32);
        }
        if b1 == 0 {
            return 0;
        }
        i += 1;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strdup(s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return core::ptr::null_mut();
    }

    let len = strlen(s);
    let ptr = crate::memory::c_mem_bridge::malloc(len + 1) as *mut c_char;

    if !ptr.is_null() {
        // Use a simple loop to ensure no hidden compiler optimizations interfere
        for i in 0..len {
            *ptr.add(i) = *s.add(i);
        }
        *ptr.add(len) = 0; // Explicitly null terminate
    }
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> i32 {
    for i in 0..n {
        let b1 = *s1.add(i) as u8;
        let b2 = *s2.add(i) as u8;
        if b1 != b2 {
            return (b1 as i32) - (b2 as i32);
        }
        if b1 == 0 {
            return 0;
        }
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcasecmp(s1: *const c_char, s2: *const c_char) -> i32 {
    let mut i = 0;
    loop {
        let b1 = (*s1.add(i) as u8).to_ascii_lowercase();
        let b2 = (*s2.add(i) as u8).to_ascii_lowercase();
        if b1 != b2 {
            return (b1 as i32) - (b2 as i32);
        }
        if b1 == 0 {
            return 0;
        }
        i += 1;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncasecmp(s1: *const c_char, s2: *const c_char, n: usize) -> i32 {
    for i in 0..n {
        let b1 = (*s1.add(i) as u8).to_ascii_lowercase();
        let b2 = (*s2.add(i) as u8).to_ascii_lowercase();
        if b1 != b2 {
            return (b1 as i32) - (b2 as i32);
        }
        if b1 == 0 {
            return 0;
        }
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strrchr(_s: *const c_char, _c: i32) -> *mut c_char {
    core::ptr::null_mut()
}

// --- MATH ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn abs(n: i32) -> i32 {
    n.abs()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atoi(s: *const c_char) -> i32 {
    if s.is_null() {
        return 0;
    }
    let mut res = 0;
    let mut i = 0;
    let bytes = CStr::from_ptr(s).to_bytes();

    while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
        res = res * 10 + (bytes[i] - b'0') as i32;
        i += 1;
    }
    res
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atof(_s: *const c_char) -> f64 {
    0.0
}

// --- FILE I/O ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fclose(_fp: *mut c_void) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fwrite(
    _ptr: *const c_void,
    size: usize,
    nmemb: usize,
    fp: *mut c_void,
) -> usize {
    if fp == 0x5678 as *mut c_void {
        // Just pretend we wrote everything
        return size * nmemb;
    }
    0
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fflush(_fp: *mut c_void) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mkdir(_path: *const c_char, _mode: u32) -> c_int {
    0
}

// --- PRINTF / VARIADICS ---
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fprintf(_fp: *mut c_void, _fmt: *const c_char, mut _args: ...) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf(_fp: *mut c_void, _fmt: *const c_char, _args: VaList) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn system(_command: *const c_char) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strchr(mut s: *const c_char, c: c_int) -> *mut c_char {
    let c = c as i8;
    while *s != 0 {
        if *s == c {
            return s as *mut c_char;
        }
        s = s.add(1);
    }
    if c == 0 {
        return s as *mut c_char;
    }
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strstr(haystack: *const c_char, needle: *const c_char) -> *mut c_char {
    if *needle == 0 {
        return haystack as *mut c_char;
    }
    let mut h = haystack;
    while *h != 0 {
        let mut h_sub = h;
        let mut n = needle;
        while *h_sub != 0 && *n != 0 && *h_sub == *n {
            h_sub = h_sub.add(1);
            n = n.add(1);
        }
        if *n == 0 {
            return h as *mut c_char;
        }
        h = h.add(1);
    }
    core::ptr::null_mut()
}

// --- VARIADICS ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsnprintf(
    _s: *mut c_char,
    _n: usize,
    fmt: *const c_char,
    mut args: VaList, // <--- args is ALREADY a VaList here
) -> i32 {
    // 1. Pass it directly by reference
    format_print(fmt, &mut args);
    0
}
// --- SYSTEM & FILE MGMT ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn remove(_pathname: *const c_char) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rename(_old: *const c_char, _new: *const c_char) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn exit(status: c_int) -> ! {
    crate::println!("DOOM exited with status {}", status);
    loop {
        timer::sleep_ms(100000);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fgets(s: *mut c_char, size: i32, fp: *mut c_void) -> *mut c_char {
    // If fp is null or invalid, return null
    if fp.is_null() || size <= 0 {
        return core::ptr::null_mut();
    }

    // For our fake file handles, just return NULL (EOF)
    // This tells DOOM "no more lines to read"
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn feof(_fp: *mut c_void) -> c_int {
    1 // Always report EOF for config files we can't provide
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferror(_fp: *mut c_void) -> c_int {
    0 // No error
}
