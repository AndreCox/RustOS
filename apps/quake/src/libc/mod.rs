pub mod memory;
pub mod string;
pub mod stdio;
pub mod math;
pub mod jmp;

use core::ffi::c_void;

#[repr(C)]
pub struct VaListC {
    pub gp_offset: u32,
    pub fp_offset: u32,
    pub overflow_arg_area: *mut c_void,
    pub reg_save_area: *mut c_void,
}

pub unsafe fn next_gp<T: Copy>(ap: &mut VaListC) -> T {
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

pub unsafe fn next_fp<T: Copy>(ap: &mut VaListC) -> T {
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
