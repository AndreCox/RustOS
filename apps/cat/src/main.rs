#![no_std]
#![no_main]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

const SYS_PRINT_CHAR: u64 = 1;
const SYS_FS_READ: u64 = 5;

fn print_str(s: &str) {
    for b in s.as_bytes() {
        unsafe {
            core::arch::asm!(
                "int 0x80",
                in("rax") SYS_PRINT_CHAR,
                in("rdi") *b as u64,
                options(nostack, preserves_flags)
            );
        }
    }
}

fn read_file(path: &str, buf: &mut [u8]) -> Option<usize> {
    let mut bytes_read: u64 = 0;
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") SYS_FS_READ,
            in("rdi") path.as_ptr() as u64,
            in("rsi") buf.as_mut_ptr() as u64,
            in("rdx") buf.len() as u64,
            lateout("rax") bytes_read,
            options(nostack, preserves_flags)
        );
    }
    if bytes_read == u64::MAX {
        None
    } else {
        Some(bytes_read as usize)
    }
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.start")]
pub extern "C" fn _start(arg_ptr: *const u8) -> ! {
    let args = if arg_ptr.is_null() {
        ""
    } else {
        unsafe {
            let mut len = 0;
            while *arg_ptr.add(len) != 0 {
                len += 1;
            }
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(arg_ptr, len))
        }
    };

    if args.is_empty() {
        print_str("Usage: cat <filename>\n");
    } else {
        // Simple buffer for reading
        let mut buf = [0u8; 4096 * 4]; // 16KB buffer
        match read_file(args, &mut buf) {
            Some(len) => {
                let bytes = &buf[..len];
                for &b in bytes {
                    unsafe {
                        core::arch::asm!(
                            "int 0x80",
                            in("rax") SYS_PRINT_CHAR,
                            in("rdi") b as u64,
                            options(nostack, preserves_flags)
                        );
                    }
                }
            }
            None => {
                print_str("Error: Could not read file '");
                print_str(args);
                print_str("'\n");
            }
        }
    }

    // Call Syscall 2: exit
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") 2u64,
            options(noreturn, nostack)
        );
    }
}
