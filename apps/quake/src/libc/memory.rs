use core::ffi::c_void;
use crate::print_str;

const MALLOC_SIZE: usize = 128 * 1024 * 1024;

#[repr(align(16))]
struct AlignedMallocBuffer([u8; MALLOC_SIZE]);

static mut MALLOC_BUFFER: AlignedMallocBuffer = AlignedMallocBuffer([0; MALLOC_SIZE]);
static mut MALLOC_PTR: usize = 0;
const MALLOC_HEADER_SIZE: usize = core::mem::size_of::<usize>();

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
pub unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    if n == 0 || core::ptr::eq(dst, src as *mut c_void) { return dst; }
    let d = dst as *mut u8;
    let s = src as *const u8;
    let mut i = 0usize;
    while i < n {
        unsafe { *d.add(i) = *s.add(i); }
        i += 1;
    }
    dst
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    if n == 0 || core::ptr::eq(dst, src as *mut c_void) { return dst; }
    let d = dst as *mut u8;
    let s = src as *const u8;
    if (d as usize) <= (s as usize) || (d as usize) >= (s as usize).saturating_add(n) {
        let mut i = 0usize;
        while i < n {
            unsafe { *d.add(i) = *s.add(i); }
            i += 1;
        }
    } else {
        let mut i = n;
        while i > 0 {
            let j = i - 1;
            unsafe { *d.add(j) = *s.add(j); }
            i -= 1;
        }
    }
    dst
}
