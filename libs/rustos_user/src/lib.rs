#![no_std]

use core::arch::asm;

pub const SYS_PRINT_CHAR: u64 = 1;
pub const SYS_EXIT: u64 = 2;
pub const SYS_CLEAR: u64 = 3;
pub const SYS_SET_CURSOR: u64 = 4;
pub const SYS_FS_READ: u64 = 5;
pub const SYS_FS_WRITE: u64 = 6;
pub const SYS_GET_SCANCODE: u64 = 7;
pub const SYS_YIELD: u64 = 8;
pub const SYS_GET_KEY: u64 = 9;
pub const SYS_DRAW_BUFFER: u64 = 10;
pub const SYS_GET_UPTIME: u64 = 11;
pub const SYS_FS_OPEN: u64 = 12;
pub const SYS_FS_READ_HANDLE: u64 = 13;
pub const SYS_FS_SEEK_HANDLE: u64 = 14;
pub const SYS_FS_CLOSE: u64 = 15;
pub const SYS_ENTER_EXCLUSIVE_GRAPHICS: u64 = 16;
pub const SYS_EXIT_EXCLUSIVE_GRAPHICS: u64 = 17;
pub const SYS_FS_MKDIR: u64 = 18;
pub const SYS_FS_REMOVE: u64 = 19;
pub const SYS_FS_RENAME: u64 = 20;

#[inline]
pub fn syscall0(nr: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") nr,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline]
pub fn syscall1(nr: u64, arg1: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") nr,
            in("rdi") arg1,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline]
pub fn syscall2(nr: u64, arg1: u64, arg2: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") nr,
            in("rdi") arg1,
            in("rsi") arg2,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline]
pub fn syscall3(nr: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") nr,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline]
pub fn print_char(c: u8) {
    let _ = syscall1(SYS_PRINT_CHAR, c as u64);
}

#[inline]
pub fn print_str(s: &str) {
    for &b in s.as_bytes() {
        print_char(b);
    }
}

#[inline]
pub fn get_key() -> Option<u8> {
    match syscall0(SYS_GET_KEY) as u8 {
        0 => None,
        b => Some(b),
    }
}

#[inline]
pub fn get_scancode() -> Option<u8> {
    match syscall0(SYS_GET_SCANCODE) as u8 {
        0 => None,
        b => Some(b),
    }
}

#[inline]
pub fn yield_now() {
    let _ = syscall0(SYS_YIELD);
}

#[inline]
pub fn uptime_ms() -> u64 {
    syscall0(SYS_GET_UPTIME)
}

#[inline]
pub fn exit() -> ! {
    let _ = syscall0(SYS_EXIT);
    loop {
        core::hint::spin_loop();
    }
}
