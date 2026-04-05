/****************************************************************************************************************************************************************************************
 *                                                                                    DOCUMENTATION                                                                                     *
 *                                                  THIS MODULE CONTAINS THE ASSEMBLY STUBS FOR OUR INTERRUPT SERVICE ROUTINES (ISRS).                                                  *
 *                                           THESE STUBS ARE THE LOW-LEVEL ENTRY POINTS FOR HANDLING CPU EXCEPTIONS AND HARDWARE INTERRUPTS.                                            *
 * EACH STUB IS RESPONSIBLE FOR SAVING THE CPU STATE, CALLING THE APPROPRIATE RUST HANDLER FUNCTION, AND THEN RESTORING THE CPU STATE BEFORE RETURNING CONTROL TO THE INTERRUPTED CODE. *
 ****************************************************************************************************************************************************************************************/

use core::arch::global_asm;

unsafe extern "C" {
    pub(super) static isr_stub_table: [extern "C" fn(); 48];
    pub(super) fn isr_stub_128();
}

global_asm!(
    r#"
    .altmacro

    /* 1. The Macros */
    .macro isr_no_err_stub num
        .global isr_stub_\num
        isr_stub_\num:
            push 0
            push \num
            jmp isr_common_stub
    .endm

    .macro isr_err_stub num
        .global isr_stub_\num
        isr_stub_\num:
            push \num
            jmp isr_common_stub
    .endm

    /* 2. Generation Loop */
    .set i, 0
    .rept 48
        .if i == 8 || (i >= 10 && i <= 14) || i == 17 // Interrupts with error codes
            isr_err_stub %i
        .else
            isr_no_err_stub %i
        .endif
        .set i, i + 1
    .endr

    // Special entry for 0x80 we use this to handle syscalls, linux and stuff like that uses 0x80 so I chose that
    .global isr_stub_128
    isr_stub_128:
        push 0 // no error code
        push 128 // interrupt number
        fxsave [rip + INTERRUPT_FPU_SNAPSHOT]
        mov byte ptr [rip + INTERRUPT_FPU_SNAPSHOT_VALID], 1
        push r15; push r14; push r13; push r12
        push r11; push r10; push r9;  push r8
        push rbp; push rdi; push rsi; push rdx
        push rcx; push rbx; push rax

        mov rdi, rsp
        call syscall_handler

        mov rsp, rax
        pop rax; pop rbx; pop rcx; pop rdx
        pop rsi; pop rdi; pop rbp; pop r8
        pop r9;  pop r10; pop r11; pop r12
        pop r13; pop r14; pop r15

        cmp byte ptr [rip + INTERRUPT_FPU_SNAPSHOT_VALID], 0
        je 1f
        fxrstor [rip + INTERRUPT_FPU_SNAPSHOT]
        mov byte ptr [rip + INTERRUPT_FPU_SNAPSHOT_VALID], 0
    1:
        add rsp, 16
        iretq

    /* 3. The Common Handler */
    isr_common_stub:
        fxsave [rip + INTERRUPT_FPU_SNAPSHOT]
        mov byte ptr [rip + INTERRUPT_FPU_SNAPSHOT_VALID], 1
        push r15; push r14; push r13; push r12
        push r11; push r10; push r9;  push r8
        push rbp; push rdi; push rsi; push rdx
        push rcx; push rbx; push rax

        mov rdi, rsp
        call exception_handler

        // On return, rax contains the new rsp
        mov rsp, rax

        // Restore registers
        pop rax; pop rbx; pop rcx; pop rdx
        pop rsi; pop rdi; pop rbp; pop r8
        pop r9;  pop r10; pop r11; pop r12
        pop r13; pop r14; pop r15

        cmp byte ptr [rip + INTERRUPT_FPU_SNAPSHOT_VALID], 0
        je 2f
        fxrstor [rip + INTERRUPT_FPU_SNAPSHOT]
        mov byte ptr [rip + INTERRUPT_FPU_SNAPSHOT_VALID], 0
    2:

        add rsp, 16
        iretq

    /* 4. The Address Table */
    .section .data
    .align 8
    .global isr_stub_table

    .macro push_stub_addr n
        .quad isr_stub_\n
    .endm

    isr_stub_table:
        .set i, 0
        .rept 48
            push_stub_addr %i
            .set i, i + 1
        .endr

    .noaltmacro
    "#
);
