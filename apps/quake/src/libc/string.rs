use core::ffi::c_char;

#[inline]
pub unsafe fn c_strlen(s: *const c_char) -> usize {
    let mut n = 0usize;
    while unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
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
pub unsafe extern "C" fn strtol(
    nptr: *const c_char,
    endptr: *mut *mut c_char,
    mut base: i32,
) -> i64 {
    let mut s = nptr;
    while unsafe { *s != 0 && (*s as u8).is_ascii_whitespace() } {
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
    if neg { -out } else { out }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strtod_rust(s_ptr: *const c_char, endptr: *mut *mut c_char) -> f64 {
    if s_ptr.is_null() {
        if !endptr.is_null() {
            unsafe {
                *endptr = core::ptr::null_mut();
            }
        }
        return 0.0;
    }
    let mut s = s_ptr;
    while unsafe { *s != 0 && (*s as u8).is_ascii_whitespace() } {
        s = unsafe { s.add(1) };
    }

    let mut neg = false;
    if unsafe { *s == b'-' as i8 } {
        neg = true;
        s = unsafe { s.add(1) };
    } else if unsafe { *s == b'+' as i8 } {
        s = unsafe { s.add(1) };
    }

    let mut val: f64 = 0.0;
    let mut has_digit = false;

    while unsafe { *s >= b'0' as i8 && *s <= b'9' as i8 } {
        has_digit = true;
        val = val * 10.0 + (unsafe { *s } - b'0' as i8) as f64;
        s = unsafe { s.add(1) };
    }

    if unsafe { *s == b'.' as i8 } {
        s = unsafe { s.add(1) };
        let mut frac = 0.1;
        while unsafe { *s >= b'0' as i8 && *s <= b'9' as i8 } {
            has_digit = true;
            val += (unsafe { *s } - b'0' as i8) as f64 * frac;
            frac /= 10.0;
            s = unsafe { s.add(1) };
        }
    }

    if has_digit && unsafe { *s == b'e' as i8 || *s == b'E' as i8 } {
        let mut exp_s = unsafe { s.add(1) };
        let mut exp_neg = false;
        if unsafe { *exp_s == b'-' as i8 } {
            exp_neg = true;
            exp_s = unsafe { exp_s.add(1) };
        } else if unsafe { *exp_s == b'+' as i8 } {
            exp_s = unsafe { exp_s.add(1) };
        }

        let mut exp_val = 0;
        let mut exp_has_digit = false;
        while unsafe { *exp_s >= b'0' as i8 && *exp_s <= b'9' as i8 } {
            exp_has_digit = true;
            exp_val = exp_val * 10 + (unsafe { *exp_s } - b'0' as i8) as i32;
            exp_s = unsafe { exp_s.add(1) };
        }

        if exp_has_digit {
            s = exp_s;
            let mut multiplier = 1.0;
            for _ in 0..exp_val {
                multiplier *= 10.0;
            }
            if exp_neg {
                val /= multiplier;
            } else {
                val *= multiplier;
            }
        }
    }

    if !endptr.is_null() {
        unsafe {
            *endptr = if has_digit {
                s as *mut c_char
            } else {
                s_ptr as *mut c_char
            }
        };
    }

    if has_digit && neg { -val } else { val }
}

pub unsafe fn strtod(nptr: *const c_char, endptr: *mut *mut c_char) -> f64 {
    unsafe { strtod_rust(nptr, endptr) }
}

pub unsafe fn atof(nptr: *const c_char) -> f64 {
    unsafe { strtod_rust(nptr, core::ptr::null_mut()) }
}
