use crate::alloc::alloc::{Layout, alloc};

pub struct Task {
    pub id: u64,
    pub stack_pointer: u64,
    pub wake_at: u64,
}

#[repr(C)]
struct TaskContext {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,

    interrupt_number: u64,
    error_code: u64,

    instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}

impl Task {
    pub fn new(id: u64, entry_point: u64) -> Self {
        let stack_size = 4096 * 4; // 16 KiB stack
        let layout = Layout::from_size_align(stack_size, 16).unwrap();

        let stack_base = unsafe { alloc(layout) } as u64;
        let stack_top = stack_base + stack_size as u64;

        let context_ptr =
            (stack_top - core::mem::size_of::<TaskContext>() as u64) as *mut TaskContext;

        unsafe {
            context_ptr.write(TaskContext {
                r15: 0,
                r14: 0,
                r13: 0,
                r12: 0,
                r11: 0,
                r10: 0,
                r9: 0,
                r8: 0,
                rbp: 0,
                rdi: 0,
                rsi: 0,
                rdx: 0,
                rcx: 0,
                rbx: 0,
                rax: 0,

                interrupt_number: 0,
                error_code: 0,

                instruction_pointer: entry_point as u64,
                code_segment: 0x28,       // Your Kernel Code Segment
                cpu_flags: 0x202,         // Interrupts enabled (IF bit)
                stack_pointer: stack_top, // Original top of stack
                stack_segment: 0x30,      // Your Kernel Data Segment
            });
        }

        Self {
            id,
            stack_pointer: context_ptr as u64,
            wake_at: 0,
        }
    }
}
