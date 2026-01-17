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
pub unsafe extern "C" fn snprintf(
    s: *mut c_char,
    n: usize,
    fmt: *const c_char,
    mut args: ...
) -> i32 {
    vsnprintf(s, n, fmt, args)
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
    s: *mut c_char,
    n: usize,
    fmt: *const c_char,
    mut args: VaList,
) -> i32 {
    let fmt_bytes = CStr::from_ptr(fmt).to_bytes();
    let mut write_idx = 0;
    let mut i = 0;

    while i < fmt_bytes.len() && write_idx < n - 1 {
        if fmt_bytes[i] == b'%' && i + 1 < fmt_bytes.len() {
            i += 1;
            match fmt_bytes[i] {
                b's' => {
                    let arg_ptr = args.arg::<*const c_char>();
                    if !arg_ptr.is_null() {
                        let arg_str = CStr::from_ptr(arg_ptr).to_bytes();
                        for &b in arg_str {
                            if write_idx < n - 1 {
                                s.add(write_idx).write(b as c_char);
                                write_idx += 1;
                            }
                        }
                    }
                }
                b'.' | b'0'..=b'9' => {
                    let mut precision = 0;
                    // Simple parser for %.3d or %03d
                    if fmt_bytes[i] == b'.' {
                        i += 1;
                        while i < fmt_bytes.len() && fmt_bytes[i].is_ascii_digit() {
                            precision = precision * 10 + (fmt_bytes[i] - b'0') as usize;
                            i += 1;
                        }
                    } else if fmt_bytes[i] == b'0' {
                        // Handle %03d style
                        while i < fmt_bytes.len() && fmt_bytes[i].is_ascii_digit() {
                            precision = precision * 10 + (fmt_bytes[i] - b'0') as usize;
                            i += 1;
                        }
                    }

                    if i < fmt_bytes.len() && (fmt_bytes[i] == b'd' || fmt_bytes[i] == b'i') {
                        let val = args.arg::<i32>();
                        let mut buf = [0u8; 12];
                        let mut curr = 11;
                        let mut v = val.abs() as u32;

                        // Standard number conversion
                        if v == 0 && precision == 0 { /* output nothing? or '0'? DOOM usually wants '0' */
                        }

                        while v > 0 || (11 - curr) < precision {
                            buf[curr] = (b'0' + (v % 10) as u8);
                            v /= 10;
                            curr -= 1;
                            if curr == 0 {
                                break;
                            } // Safety
                        }

                        if val < 0 {
                            buf[curr] = b'-';
                            curr -= 1;
                        }

                        for b in &buf[curr + 1..12] {
                            if write_idx < n - 1 {
                                s.add(write_idx).write(*b as c_char);
                                write_idx += 1;
                            }
                        }
                    }
                }
                b'i' | b'd' => {
                    let val = args.arg::<i32>();
                    let mut buf = [0u8; 12];
                    let mut curr = 11;
                    let is_neg = val < 0;
                    let mut v = if is_neg { -val as u32 } else { val as u32 };

                    if v == 0 {
                        buf[curr] = b'0';
                        curr -= 1;
                    } else {
                        while v > 0 {
                            buf[curr] = (b'0' + (v % 10) as u8);
                            v /= 10;
                            curr -= 1;
                        }
                    }
                    if is_neg {
                        buf[curr] = b'-';
                        curr -= 1;
                    }
                    for b in &buf[curr + 1..12] {
                        if write_idx < n - 1 {
                            s.add(write_idx).write(*b as c_char);
                            write_idx += 1;
                        }
                    }
                }
                _ => {
                    s.add(write_idx).write(fmt_bytes[i] as c_char);
                    write_idx += 1;
                }
            }
        } else {
            s.add(write_idx).write(fmt_bytes[i] as c_char);
            write_idx += 1;
        }
        i += 1;
    }
    s.add(write_idx).write(0); // CRITICAL: Null terminator
    write_idx as i32
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
