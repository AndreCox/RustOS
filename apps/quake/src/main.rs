#![no_std]
#![no_main]
#![feature(c_variadic)]

mod libc;

use core::ffi::c_void;
use core::panic::PanicInfo;
use rustos_user::{
    SYS_DRAW_BUFFER, SYS_ENTER_EXCLUSIVE_GRAPHICS, SYS_EXIT_EXCLUSIVE_GRAPHICS,
    SYS_GET_KEY, SYS_GET_SCANCODE, SYS_GET_UPTIME, SYS_YIELD, exit as user_exit,
    syscall0, syscall2,
};

pub use crate::libc::stdio::vfprintf_internal;
pub use crate::libc::{VaListC, next_fp, next_gp};
pub use crate::libc::math::{abs, acos, atan2, ceil, cos, fabs, floor, modf, pow, rand, sin, sqrt, srand};
pub use crate::libc::string::strtod_rust;

#[unsafe(no_mangle)]
pub static mut stdout: *mut c_void = core::ptr::null_mut();

#[unsafe(no_mangle)]
pub static mut vid_menudrawfn: Option<unsafe extern "C" fn()> = None;

#[unsafe(no_mangle)]
pub static mut vid_menukeyfn: Option<unsafe extern "C" fn(i32)> = None;


#[inline]
pub fn print_char(c: u8) { rustos_user::print_char(c); }
#[inline]
pub fn print_str(s: &str) { rustos_user::print_str(s); }

unsafe extern "C" {
    fn main(argc: i32, argv: *const *const i8) -> i32;
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exit(_status: i32) -> ! {
    user_exit();
}

#[unsafe(no_mangle)] pub extern "C" fn quake_enter_graphics() { let _ = syscall0(SYS_ENTER_EXCLUSIVE_GRAPHICS); }
#[unsafe(no_mangle)] pub extern "C" fn quake_exit_graphics() { let _ = syscall0(SYS_EXIT_EXCLUSIVE_GRAPHICS); }
#[unsafe(no_mangle)] pub extern "C" fn quake_draw_buffer(p: *const u32, w: u32, h: u32) {
    let dims = (w as u64) | ((h as u64) << 32);
    let _ = syscall2(SYS_DRAW_BUFFER, p as u64, dims);
}
#[unsafe(no_mangle)] pub extern "C" fn quake_poll_key() -> i32 { syscall0(SYS_GET_KEY) as i32 }
#[unsafe(no_mangle)] pub extern "C" fn quake_poll_scancode() -> i32 { syscall0(SYS_GET_SCANCODE) as i32 }
#[unsafe(no_mangle)] pub extern "C" fn quake_yield() { let _ = syscall0(SYS_YIELD); }
#[unsafe(no_mangle)] pub extern "C" fn quake_uptime_ms() -> u64 { syscall0(SYS_GET_UPTIME) }

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start() -> ! {
    let argc = 1;
    let argv: [*const i8; 1] = [c"quake".as_ptr()];
    let _res = unsafe { main(argc, argv.as_ptr()) };
    user_exit();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    print_str("PANIC!\n");
    user_exit();
}
