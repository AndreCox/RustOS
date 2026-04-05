#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn setjmp(env: *mut u64) -> i32 {
    core::arch::naked_asm!(
        "mov [rdi + 0], rbx",
        "mov [rdi + 8], rbp",
        "mov [rdi + 16], r12",
        "mov [rdi + 24], r13",
        "mov [rdi + 32], r14",
        "mov [rdi + 40], r15",
        "lea rdx, [rsp + 8]",
        "mov [rdi + 48], rdx",
        "mov rdx, [rsp]",
        "mov [rdi + 56], rdx",
        "xor eax, eax",
        "ret"
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn longjmp(env: *mut u64, val: i32) {
    core::arch::naked_asm!(
        "mov rbx, [rdi + 0]",
        "mov rbp, [rdi + 8]",
        "mov r12, [rdi + 16]",
        "mov r13, [rdi + 24]",
        "mov r14, [rdi + 32]",
        "mov r15, [rdi + 40]",
        "mov rsp, [rdi + 48]",
        "mov rdx, [rdi + 56]",
        "mov [rsp], rdx",
        "mov eax, esi",
        "test eax, eax",
        "jnz 1f",
        "inc eax",
        "1:",
        "ret"
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _setjmp(env: *mut u64) -> i32 {
    unsafe { setjmp(env) }
}
