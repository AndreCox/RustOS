pub mod scheduler;
pub mod task;

pub fn init_multitasking() {
    let mut scheduler = scheduler::SCHEDULER.lock();

    // We save the Main Task, aka the kernel task
    let main_task = task::Task {
        id: 0,
        stack_pointer: 0, // Will be set during the first context switch
    };

    scheduler.current_task = Some(main_task);
}
