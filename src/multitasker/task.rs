use crate::alloc::alloc::{Layout, alloc};

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum TaskStatus {
    Ready,
    Running,
    Killed, // The "Crashing" state
    Waiting,
}

#[repr(align(16))]
#[derive(Debug, Clone)]
pub struct FpuState {
    pub data: [u8; 512],
}

impl Default for FpuState {
    fn default() -> Self {
        let mut data = [0u8; 512];
        // Minimal initialization for FXSAVE format:
        // Set MXCSR to default value (0x1f80) to avoid floating point exceptions
        data[24..28].copy_from_slice(&0x1f80u32.to_le_bytes());
        FpuState { data }
    }
}

pub struct Task {
    pub id: u64,
    pub stack_pointer: u64,
    pub wake_at: u64,
    pub status: TaskStatus,
    pub fpu_state: FpuState,
}

#[repr(C)]
#[derive(Default)]
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
        let stack_size = 1024 * 4; // 16 KB
        let layout = Layout::from_size_align(stack_size, 16).unwrap();
        let stack_base = unsafe { alloc(layout) } as u64;
        let stack_top = stack_base + stack_size as u64;

        let aligned_top = stack_top & !0xF;

        let context_size = core::mem::size_of::<TaskContext>() as u64;

        let context_ptr = (aligned_top - context_size) as *mut TaskContext;

        unsafe {
            context_ptr.write(TaskContext {
                rbp: aligned_top,

                instruction_pointer: entry_point,
                code_segment: 0x28,
                cpu_flags: 0x202,

                // When iretq finishes, RSP will be set to this value.
                // It must be abi_compliant_top so the function starts with (RSP % 16) == 8.
                stack_pointer: aligned_top,
                stack_segment: 0x30,
                ..Default::default()
            });
        }

        Self {
            id,
            stack_pointer: context_ptr as u64,
            wake_at: 0,
            status: TaskStatus::Ready,
            fpu_state: FpuState::default(),
        }
    }
}
