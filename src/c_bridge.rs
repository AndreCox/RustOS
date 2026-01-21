use crate::{println, timer};
use core::ffi::{CStr, VaList, c_char, c_int, c_void};

// =============================================================================
// GLOBAL STATE
// =============================================================================

#[unsafe(no_mangle)]
pub static mut net_client_connected: i32 = 0;

#[unsafe(no_mangle)]
pub static mut errno: c_int = 0;

#[unsafe(no_mangle)]
pub static mut drone: i32 = 0;

// =============================================================================
// CONSTANTS
// =============================================================================

const MAX_STRLEN_SEARCH: usize = 4096;
const FAKE_STDOUT_HANDLE: *mut c_void = 0x5678 as *mut c_void;

// =============================================================================
// POINTER VALIDATION
// =============================================================================

#[inline]
unsafe fn is_valid_readable_ptr(ptr: *const c_char) -> bool {
    let addr = ptr as usize;

    // Null check
    if addr == 0 {
        return false;
    }

    // Check for obviously bad addresses
    if addr < 0x1000 {
        return false;
    }

    // CRITICAL: Check for sign-extended 32-bit offsets being used as pointers
    // These look like 0xFFFFFFFFxxxxxxxx where xxxxxxxx is a small number
    if addr > 0xFFFFFFFF00000000 && addr < 0xFFFFFFFFFFFFFFFF {
        let lower_32 = addr as u32;
        // If the lower 32 bits form a "reasonable" offset (< 16MB),
        // this is likely a mcmodel=kernel issue
        if lower_32 < 0x01000000 {
            crate::println!(
                "MCMODEL BUG: Detected sign-extended offset {:#x} (lower: {:#x})",
                addr,
                lower_32
            );
            crate::println!("  Recompile DOOM with -mcmodel=large instead of -mcmodel=kernel");
            return false;
        }
    }

    // Check for non-canonical addresses on x86-64
    // Bits 48-63 must be copies of bit 47
    let bit_47 = (addr >> 47) & 1;
    let high_bits = addr >> 48;

    if bit_47 == 0 && high_bits != 0 {
        crate::println!("Non-canonical address detected: {:#x}", addr);
        return false;
    }
    if bit_47 == 1 && high_bits != 0xFFFF {
        crate::println!("Non-canonical address detected: {:#x}", addr);
        return false;
    }

    true
}

// =============================================================================
// STRING OPERATIONS
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    if !is_valid_readable_ptr(s) {
        crate::println!("strlen: invalid pointer {:#x}", s as usize);
        return 0;
    }

    (0..MAX_STRLEN_SEARCH)
        .take_while(|&i| *s.add(i) != 0)
        .count()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcpy(dest: *mut c_char, src: *const c_char) -> *mut c_char {
    let mut i = 0;
    loop {
        let byte = *src.add(i);
        *dest.add(i) = byte;
        if byte == 0 {
            break;
        }
        i += 1;
    }
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn toupper(c: c_int) -> c_int {
    if c >= b'a' as c_int && c <= b'z' as c_int {
        return c - 32;
    }
    c
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tolower(c: c_int) -> c_int {
    if c >= b'A' as c_int && c <= b'Z' as c_int {
        return c + 32;
    }
    c
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memchr(s: *const c_void, c: c_int, n: usize) -> *mut c_void {
    let p = s as *const u8;
    for i in 0..n {
        if *p.add(i) == c as u8 {
            return p.add(i) as *mut c_void;
        }
    }
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    // Rust's copy is actually a memmove (handles overlap)
    core::ptr::copy(src as *const u8, dest as *mut u8, n);
    dest
}

static mut DUMMY_FILE_BUFFER: i32 = 0;

#[unsafe(no_mangle)]
pub static mut stderr: *mut i32 = unsafe { core::ptr::addr_of_mut!(DUMMY_FILE_BUFFER) };

#[unsafe(no_mangle)]
pub static mut stdout: *mut i32 = unsafe { core::ptr::addr_of_mut!(DUMMY_FILE_BUFFER) };

#[unsafe(no_mangle)]
pub static mut stdin: *mut i32 = unsafe { core::ptr::addr_of_mut!(DUMMY_FILE_BUFFER) };

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncpy(dest: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
    let mut i = 0;
    while i < n {
        let byte = *src.add(i);
        *dest.add(i) = byte;
        if byte == 0 {
            // Fill remainder with nulls per C spec
            dest.add(i + 1).write_bytes(0, n - i - 1);
            break;
        }
        i += 1;
    }
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcmp(s1: *const c_char, s2: *const c_char) -> i32 {
    if !is_valid_readable_ptr(s1) {
        crate::println!("strcmp: invalid s1={:#x}", s1 as usize);
        return -1;
    }
    if !is_valid_readable_ptr(s2) {
        crate::println!("strcmp: invalid s2={:#x}", s2 as usize);
        return 1;
    }
    compare_strings(s1, s2, usize::MAX, |b| b)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> i32 {
    if !is_valid_readable_ptr(s1) || !is_valid_readable_ptr(s2) {
        return if s1 as usize > s2 as usize { 1 } else { -1 };
    }
    compare_strings(s1, s2, n, |b| b)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcasecmp(s1: *const c_char, s2: *const c_char) -> i32 {
    if !is_valid_readable_ptr(s1) || !is_valid_readable_ptr(s2) {
        return if s1 as usize > s2 as usize { 1 } else { -1 };
    }
    compare_strings(s1, s2, usize::MAX, |b| b.to_ascii_lowercase())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncasecmp(s1: *const c_char, s2: *const c_char, n: usize) -> i32 {
    if !is_valid_readable_ptr(s1) || !is_valid_readable_ptr(s2) {
        return if s1 as usize > s2 as usize { 1 } else { -1 };
    }
    compare_strings(s1, s2, n, |b| b.to_ascii_lowercase())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strdup(s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return core::ptr::null_mut();
    }

    let len = strlen(s);
    let ptr = crate::memory::c_mem_bridge::malloc(len + 1) as *mut c_char;

    if !ptr.is_null() {
        core::ptr::copy_nonoverlapping(s, ptr, len);
        *ptr.add(len) = 0;
    }
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strchr(mut s: *const c_char, c: c_int) -> *mut c_char {
    if !is_valid_readable_ptr(s) {
        crate::println!("strchr: invalid pointer {:#x}", s as usize);
        return core::ptr::null_mut();
    }

    let target = c as i8;
    while *s != 0 {
        if *s == target {
            return s as *mut c_char;
        }
        s = s.add(1);
    }
    if target == 0 {
        return s as *mut c_char;
    }
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strrchr(_s: *const c_char, _c: i32) -> *mut c_char {
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strstr(haystack: *const c_char, needle: *const c_char) -> *mut c_char {
    if !is_valid_readable_ptr(haystack) || !is_valid_readable_ptr(needle) {
        crate::println!(
            "strstr: invalid pointer(s) h={:#x} n={:#x}",
            haystack as usize,
            needle as usize
        );
        return core::ptr::null_mut();
    }

    if *needle == 0 {
        return haystack as *mut c_char;
    }

    let mut h = haystack;
    while *h != 0 {
        if strings_match_at(h, needle) {
            return h as *mut c_char;
        }
        h = h.add(1);
    }
    core::ptr::null_mut()
}

// =============================================================================
// MATH OPERATIONS
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn abs(n: i32) -> i32 {
    n.abs()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atoi(s: *const c_char) -> i32 {
    if s.is_null() {
        return 0;
    }

    CStr::from_ptr(s)
        .to_bytes()
        .iter()
        .take_while(|&&b| b.is_ascii_digit())
        .fold(0, |acc, &b| acc * 10 + (b - b'0') as i32)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atof(_s: *const c_char) -> f64 {
    0.0
}

// =============================================================================
// I/O OPERATIONS
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn printf(fmt: *const c_char, mut args: ...) -> i32 {
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
pub unsafe extern "C" fn sprintf(s: *mut c_char, fmt: *const c_char, mut args: ...) -> i32 {
    // Doom assumes the buffer is large enough (unsafe!), so we pass a huge size limit
    // to vsnprintf to emulate standard sprintf behavior.
    vsnprintf(s, usize::MAX, fmt, args)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsprintf(s: *mut c_char, fmt: *const c_char, args: VaList) -> i32 {
    vsnprintf(s, usize::MAX, fmt, args)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsnprintf(
    s: *mut c_char,
    n: usize,
    fmt: *const c_char,
    mut args: VaList,
) -> i32 {
    if n == 0 || s.is_null() || fmt.is_null() {
        return 0;
    }

    let fmt_bytes = CStr::from_ptr(fmt).to_bytes();
    let mut writer = BufferWriter::new(s, n);
    let mut i = 0;

    while i < fmt_bytes.len() && writer.has_space() {
        if fmt_bytes[i] == b'%' && i + 1 < fmt_bytes.len() {
            i += 1;
            i = handle_format_specifier(&fmt_bytes, i, &mut args, &mut writer);
        } else {
            unsafe {
                writer.write_byte(fmt_bytes[i]);
            }
        }
        i += 1;
    }

    unsafe {
        writer.finalize()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sscanf(_s: *const c_char, _fmt: *const c_char, ...) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fprintf(_fp: *mut c_void, fmt: *const c_char, mut args: ...) -> c_int {
    format_print(fmt, &mut args);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf(_fp: *mut c_void, fmt: *const c_char, mut args: VaList) -> c_int {
    format_print(fmt, &mut args);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn getenv(_name: *const c_char) -> *mut c_char {
    // Returning NULL is perfectly safe; Doom will just use default paths.
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn isspace(c: i32) -> i32 {
    (c == 32 || (c >= 9 && c <= 13)) as i32
}

// =============================================================================
// FILE OPERATIONS
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fwrite(
    _ptr: *const c_void,
    size: usize,
    nmemb: usize,
    fp: *mut c_void,
) -> usize {
    if fp == FAKE_STDOUT_HANDLE {
        size * nmemb
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fflush(_fp: *mut c_void) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fgets(s: *mut c_char, size: i32, fp: *mut c_void) -> *mut c_char {
    if fp.is_null() || size <= 0 {
        return core::ptr::null_mut();
    }
    // Return NULL to indicate EOF
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn feof(_fp: *mut c_void) -> c_int {
    1 // Always report EOF
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferror(_fp: *mut c_void) -> c_int {
    0 // No error
}

// =============================================================================
// SYSTEM OPERATIONS
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mkdir(_path: *const c_char, _mode: u32) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn remove(_pathname: *const c_char) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rename(_old: *const c_char, _new: *const c_char) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn system(_command: *const c_char) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn exit(status: c_int) -> ! {
    crate::println!("DOOM exited with status {}", status);
    loop {
        timer::sleep_ms(100000);
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

unsafe fn compare_strings<F>(
    s1: *const c_char,
    s2: *const c_char,
    max_len: usize,
    transform: F,
) -> i32
where
    F: Fn(u8) -> u8,
{
    for i in 0..max_len {
        let b1 = transform(*s1.add(i) as u8);
        let b2 = transform(*s2.add(i) as u8);
        if b1 != b2 {
            return (b1 as i32) - (b2 as i32);
        }
        if b1 == 0 {
            return 0;
        }
    }
    0
}

unsafe fn strings_match_at(haystack: *const c_char, needle: *const c_char) -> bool {
    let mut h = haystack;
    let mut n = needle;
    while *n != 0 {
        if *h != *n {
            return false;
        }
        h = h.add(1);
        n = n.add(1);
    }
    true
}

unsafe fn format_print(fmt: *const c_char, va_list: &mut VaList) {
    if fmt.is_null() {
        return;
    }

    // ALIGNMENT FIX: Validate pointer alignment before dereferencing
    if (fmt as usize) % core::mem::align_of::<c_char>() != 0 {
        crate::println!("WARNING: Misaligned format string pointer");
        return;
    }

    let fmt_bytes = CStr::from_ptr(fmt).to_bytes();
    let mut i = 0;

    while i < fmt_bytes.len() {
        if fmt_bytes[i] == b'%' && i + 1 < fmt_bytes.len() {
            i += 1;
            match fmt_bytes[i] {
                b's' => print_string_arg(va_list),
                b'd' | b'i' => crate::print!("{}", va_list.arg::<i32>()),
                b'u' => crate::print!("{}", va_list.arg::<u32>()),
                b'x' | b'p' => crate::print!("{:#x}", va_list.arg::<usize>()),
                b'c' => crate::print!("{}", va_list.arg::<i32>() as u8 as char),
                b'%' => crate::print!("%"),
                _ => crate::print!("%{}", fmt_bytes[i] as char),
            }
        } else {
            crate::print!("{}", fmt_bytes[i] as char);
        }
        i += 1;
    }
}

unsafe fn print_string_arg(va_list: &mut VaList) {
    let ptr = va_list.arg::<*const c_char>();

    // CRITICAL ALIGNMENT FIX: Validate pointer before dereferencing
    if ptr.is_null() {
        crate::print!("(null)");
        return;
    }

    // Check alignment
    if (ptr as usize) % core::mem::align_of::<c_char>() != 0 {
        crate::print!("(misaligned-ptr:{:#x})", ptr as usize);
        return;
    }

    // Validate pointer is in reasonable range (not corrupted)
    if !is_valid_readable_ptr(ptr) {
        let addr = ptr as usize;
        crate::print!("(invalid-ptr:{:#x})", addr);
        return;
    }

    let s = CStr::from_ptr(ptr).to_string_lossy();
    crate::print!("{}", s);
}

unsafe fn handle_format_specifier(
    fmt_bytes: &[u8],
    mut i: usize,
    args: &mut VaList,
    writer: &mut BufferWriter,
) -> usize {
    match fmt_bytes[i] {
        b's' => {
            let ptr = args.arg::<*const c_char>();
            if !ptr.is_null()
                && (ptr as usize) % core::mem::align_of::<c_char>() == 0
                && is_valid_readable_ptr(ptr)
            {
                writer.write_cstr(ptr);
            }
            i
        }
        b'.' | b'0'..=b'9' => {
            let (new_i, precision) = parse_precision(fmt_bytes, i);
            let check_i = new_i;

            if check_i < fmt_bytes.len() && (fmt_bytes[check_i] == b'd' || fmt_bytes[check_i] == b'i') {
                let val = args.arg::<i32>();
                writer.write_int_with_precision(val, precision);
                new_i
            } else {
                new_i - 1
            }
        }
        b'i' | b'd' => {
            let val = args.arg::<i32>();
            unsafe {
                writer.write_int(val);
            }
            i
        }
        _ => {
            unsafe {
                writer.write_byte(fmt_bytes[i]);
            }
            i
        }
    }
}

fn parse_precision(fmt_bytes: &[u8], mut i: usize) -> (usize, usize) {
    let mut precision = 0;

    if fmt_bytes[i] == b'.' {
        i += 1;
        while i < fmt_bytes.len() && fmt_bytes[i].is_ascii_digit() {
            precision = precision * 10 + (fmt_bytes[i] - b'0') as usize;
            i += 1;
        }
    } else if fmt_bytes[i] == b'0' {
        while i < fmt_bytes.len() && fmt_bytes[i].is_ascii_digit() {
            precision = precision * 10 + (fmt_bytes[i] - b'0') as usize;
            i += 1;
        }
    }

    (i, precision)
}

// =============================================================================
// BUFFER WRITER
// =============================================================================

struct BufferWriter {
    buffer: *mut c_char,
    capacity: usize,
    position: usize,
}

impl BufferWriter {
    unsafe fn new(buffer: *mut c_char, capacity: usize) -> Self {
        Self {
            buffer,
            capacity,
            position: 0,
        }
    }

    fn has_space(&self) -> bool {
        self.position < self.capacity - 1
    }

    unsafe fn write_byte(&mut self, byte: u8) {
        if self.has_space() {
            self.buffer.add(self.position).write(byte as c_char);
            self.position += 1;
        }
    }

    unsafe fn write_cstr(&mut self, s: *const c_char) {
        let bytes = CStr::from_ptr(s).to_bytes();
        for &b in bytes {
            if !self.has_space() {
                break;
            }
            self.write_byte(b);
        }
    }

    unsafe fn write_int(&mut self, val: i32) {
        let buf = int_to_bytes(val);
        for &b in &buf {
            if b == 0 {
                break;
            }
            self.write_byte(b);
        }
    }

    unsafe fn write_int_with_precision(&mut self, val: i32, precision: usize) {
        let buf = int_to_bytes_with_precision(val, precision);
        for &b in &buf {
            if b == 0 {
                break;
            }
            self.write_byte(b);
        }
    }

    unsafe fn finalize(self) -> i32 {
        self.buffer.add(self.position).write(0);
        self.position as i32
    }
}

fn int_to_bytes(val: i32) -> [u8; 12] {
    let mut buf = [0u8; 12];
    let mut curr: isize = 11;
    let is_neg = val < 0;
    let mut v = val.unsigned_abs();

    if v == 0 {
        buf[curr as usize] = b'0';
        curr -= 1;
    } else {
        while v > 0 && curr >= 0 {
            buf[curr as usize] = b'0' + (v % 10) as u8;
            v /= 10;
            curr -= 1;
        }
    }

    if is_neg && curr >= 0 {
        buf[curr as usize] = b'-';
        curr -= 1;
    }

    // Shift to beginning
    let mut result = [0u8; 12];
    let mut src = curr + 1;
    let mut dst = 0;
    while src < 12 && dst < 12 {
        result[dst] = buf[src as usize];
        dst += 1;
        src += 1;
    }
    result
}

fn int_to_bytes_with_precision(val: i32, precision: usize) -> [u8; 12] {
    let mut buf = [0u8; 12];
    let mut curr: isize = 11;
    let mut v = val.unsigned_abs();

    while (v > 0 || (11 - curr) < precision as isize) && curr >= 0 {
        buf[curr as usize] = b'0' + (v % 10) as u8;
        v /= 10;
        curr -= 1;
    }

    if val < 0 && curr >= 0 {
        buf[curr as usize] = b'-';
        curr -= 1;
    }

    let mut result = [0u8; 12];
    let mut src = curr + 1;
    let mut dst = 0;
    while src < 12 && dst < 12 {
        result[dst] = buf[src as usize];
        dst += 1;
        src += 1;
    }
    result
}
