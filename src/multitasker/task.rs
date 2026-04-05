/*********************************************************************************************************************************************************************************************************************************************************
 *                                                                                                                     DOCUMENTATION                                                                                                                     *
 *                                                                 THE TASK CODE IS RESPONSIBLE FOR DEFINING THE TASK STRUCT, THIS REPRESENTS A SINGLE TASK IN OUR MULTITASKING SYSTEM.                                                                  *
 *                                                                                             WE HAVE A DEFAULT FPU STATE THAT WE INITIALIZE FOR EACH TASK.                                                                                             *
 *                      WE THEN CREATE A TASK WHICH IS THE HIGH LEVEL ABSTRACTION OF A TASK, CONTAINING ITS ID, STACK POINTER, WAKE TIME, STATUS, FPU STATE, AND SOME BOOKKEEPING FIELDS FOR MEMORY MANAGEMENT AND OWNED RESOURCES.                      *
 * THE TASKCONTEXT STRUCT IS THE LOW LEVEL CPU CONTEXT THAT WE SAVE AND RESTORE DURING CONTEXT SWITCHES, IT CONTAINS ALL THE GENERAL PURPOSE REGISTERS, AS WELL AS THE INSTRUCTION POINTER, CODE SEGMENT, FLAGS, AND STACK INFORMATION NEEDED FOR IRETQ. *
 *                                              WE ALSO IMPLEMENT A DROP TRAIT FOR TASK TO ENSURE THAT WHEN A TASK IS DROPPED, ITS ALLOCATED STACK MEMORY IS PROPERLY DEALLOCATED TO PREVENT MEMORY LEAKS.                                               *
 *********************************************************************************************************************************************************************************************************************************************************/
use crate::{
    alloc::alloc::{Layout, alloc, dealloc},
    alloc::boxed::Box,
    alloc::vec::Vec,
};

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum TaskStatus {
    Ready,
    Running,
    Killed, // The "Crashing" state
    Exited, // Normal termination
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
#[repr(C, align(16))]

// This Task is a high level abstraction of the Task
pub struct Task {
    pub fpu_state: FpuState,
    pub id: u64,
    pub stack_pointer: u64, // This is the pointer to the TaskContext on the task's stack.
    pub wake_at: u64,
    pub status: TaskStatus,
    pub stack_base: u64, // The base of the allocated stack, used for deallocation.
    pub stack_size: usize, // The size of the allocated stack, used for deallocation.
    pub owned_program_image: Option<Vec<u8>>, // Keep original ELF allocation to preserve alignment.
    pub owned_arg_bytes: Option<Box<[u8]>>, // NUL-terminated argument bytes kept alive for task lifetime.
}

// This is the low level CPU context, we save and restore it during context switches.
#[repr(C)]
#[derive(Default)]
struct TaskContext {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,

    interrupt_number: u64,
    error_code: u64,

    instruction_pointer: u64,
    code_segment: u64, // This will be 0x28 for kernel tasks and 0x23 for user tasks, to ensure correct privilege level after iretq.
    cpu_flags: u64,    // holds the RFLAGS value to be loaded during iretq
    stack_pointer: u64, // This is the value that will be loaded into RSP when iretq finishes.
    stack_segment: u64, // This is the value that will be loaded into SS when iretq finishes.
}

impl Task {
    pub fn new(id: u64, entry_point: u64, arg: u64, user_stack_top: Option<u64>) -> Self {
        let stack_size = 1024 * 1024 * 2; // 2 MB
        let layout = Layout::from_size_align(stack_size, 16).unwrap();
        let stack_base = unsafe { alloc(layout) } as u64;
        let stack_top = stack_base + stack_size as u64;

        let aligned_top = stack_top & !0xF;
        let abi_compliant_top = aligned_top - 8;

        let context_size = core::mem::size_of::<TaskContext>() as u64;

        let context_ptr = (abi_compliant_top - context_size) as *mut TaskContext;
        let task_stack_pointer = user_stack_top.unwrap_or(abi_compliant_top);

        unsafe {
            context_ptr.write(TaskContext {
                rbp: aligned_top,
                rdi: arg,

                instruction_pointer: entry_point,
                code_segment: 0x28,
                cpu_flags: 0x202,

                // For kernel tasks, use the ABI-compliant top of this stack.
                // For user tasks, use the caller-provided user stack pointer.
                stack_pointer: task_stack_pointer,
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
            stack_base: stack_base,
            stack_size: stack_size,
            owned_program_image: None,
            owned_arg_bytes: None,
        }
    }

    // This function allows us to attach owned memory (like the program image and argument bytes) to the task, ensuring that they will be kept alive for the lifetime of the task and automatically deallocated when the task is dropped. This is crucial for preventing memory leaks when tasks exit or are killed.
    pub fn with_owned_memory(
        mut self,
        owned_program_image: Option<Vec<u8>>,
        owned_arg_bytes: Option<Box<[u8]>>,
    ) -> Self {
        self.owned_program_image = owned_program_image;
        self.owned_arg_bytes = owned_arg_bytes;
        self
    }
}

// Drop is a built-in Rust trait that allows us to specify custom behavior when a value goes out of scope. By implementing Drop for Task, we can ensure that when a Task is dropped (for example, when it is removed from the scheduler and no longer needed), we automatically deallocate its stack memory to prevent memory leaks. This is especially important in an OS kernel where we may be creating and destroying many tasks over time.
impl Drop for Task {
    fn drop(&mut self) {
        if self.stack_base != 0 {
            let layout = Layout::from_size_align(self.stack_size, 16).unwrap();
            unsafe {
                dealloc(self.stack_base as *mut u8, layout);
            }
        }
    }
}
