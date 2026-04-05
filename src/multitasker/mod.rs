/********************************************************************************************************************************************
 *                                                              DOCUMENTATION                                                               *
 *                                                   THIS MODULE IMPLEMENTS MULTITASKING,                                                   *
 *                     RIGHT NOW INIT MULTITASKING INITIALIZES A SCHEDULER AND CREATES THE MAIN TASK (THE KERNEL TASK).                     *
 * IT ALSO IMPLEMENTS A YIELD_NOW FUNCTION THAT TRIGGERS THE SCHEDULER INTERRUPT, AND AN IDLE TASK THAT RUNS WHEN NO OTHER TASKS ARE READY. *
 ********************************************************************************************************************************************/

pub mod scheduler;
pub mod task;

pub fn init_multitasking() {
    let mut new_scheduler = scheduler::Scheduler::new();

    // We save the Main Task, aka the kernel task
    let main_task = task::Task {
        id: 0,
        stack_pointer: 0, // Will be set during the first context switch
        wake_at: 0,
        status: task::TaskStatus::Ready,
        fpu_state: task::FpuState::default(),
        stack_base: 0,
        stack_size: 0,
        owned_program_image: None,
        owned_arg_bytes: None,
    };

    // We set the current task to the main task before enabling the scheduler
    new_scheduler.current_task = Some(main_task);
    *scheduler::SCHEDULER.lock() = Some(new_scheduler); // Publish the scheduler for use by the timer interrupt handler
}

pub fn yield_now() {
    unsafe {
        core::arch::asm!("int 0x20"); // Manually trigger the Timer/Scheduler IRQ
    }
}

pub fn idle_task() -> ! {
    loop {
        // Increment a global counter of "idle time"
        unsafe { crate::globals::IDLE_TICKS.fetch_add(1, core::sync::atomic::Ordering::Relaxed) };

        // Use 'hlt' to stop the CPU until the next interrupt
        // This is crucial: 'hlt' makes the loop pulse at the same rate as the timer
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
