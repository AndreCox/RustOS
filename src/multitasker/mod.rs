pub mod scheduler;
pub mod task;

pub fn init_multitasking() {
    let mut scheduler = scheduler::SCHEDULER.lock();

    // We save the Main Task, aka the kernel task
    let main_task = task::Task {
        id: 0,
        stack_pointer: 0, // Will be set during the first context switch
        wake_at: 0,
    };

    scheduler.current_task = Some(main_task);
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
