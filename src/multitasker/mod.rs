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
    };

    new_scheduler.current_task = Some(main_task);
    *scheduler::SCHEDULER.lock() = Some(new_scheduler);
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
