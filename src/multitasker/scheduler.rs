/******************************************************************************************************************************************************************************************************************************************************
 *                                                                                                                   DOCUMENTATION                                                                                                                    *
 *                                                                              THIS IS THE SCHEDULER, IT MANAGES THE QUEUE OF TASKS AND DECIDES WHICH ONE TO RUN NEXT,                                                                               *
 *                                     WE STORE THE SCHEDULER IN A GLOBAL MUTEX<OPTION<SCHEDULER>> SO THAT IT CAN BE SAFELY ACCESSED FROM THE TIMER INTERRUPT HANDLER, WHICH IS WHERE THE CONTEXT SWITCH HAPPENS.                                     *
 * THE SCHEDULER ALSO SAVES AND RESTORES THE FPU/SSE STATE OF TASKS DURING CONTEXT SWITCHES, USING THE FXSAVE/FXRSTOR INSTRUCTIONS. THIS IS CRUCIAL FOR SUPPORTING FLOATING-POINT OPERATIONS IN USER TASKS WITHOUT CORRUPTING THE KERNEL'S FPU STATE. *
 *                                   WE DEALLOCATE TASKS WHEN THEY ARE KILLED OR EXITED, THIS IS DONE USING THE DROP IMPLEMENTATION OF THE TASK STRUCT, WHICH DEALLOCATES THE STACK MEMORY AND ANY OWNED RESOURCES.                                   *
 *                                                                                                       THIS CAN BE FOUND IN THE TASK.RS FILE.                                                                                                       *
 ******************************************************************************************************************************************************************************************************************************************************/

use super::task::Task;
use crate::alloc::collections::VecDeque;
use spin::Mutex;
use x86_64::instructions::interrupts;

pub struct Scheduler {
    pub tasks: VecDeque<Task>,
    pub current_task: Option<Task>,
}
impl Scheduler {
    pub fn new() -> Self {
        Scheduler {
            tasks: VecDeque::new(),
            current_task: None,
        }
    }

    pub fn add_task(&mut self, task: Task) {
        x86_64::instructions::interrupts::without_interrupts(|| {
            self.tasks.push_back(task);
        });
    }

    pub fn get_next_task(&mut self) -> Option<Task> {
        let now = crate::timer::get_uptime_ms();

        for _ in 0..self.tasks.len() {
            let task = self.tasks.pop_front()?;

            // If it's the Idle Task (ID 0) OR it's time to wake up
            if task.id == 0 || now >= task.wake_at {
                return Some(task);
            } else {
                // Not ready yet, put it back in the queue and try the next one
                self.tasks.push_back(task);
            }
        }
        None
    }

    pub fn get_current_task_id(&self) -> u64 {
        self.current_task.as_ref().map(|t| t.id).unwrap_or(0)
    }

    pub fn schedule(&mut self, stack_pointer: u64) -> u64 {
        let now = crate::timer::get_uptime_ms();

        // 1. Save the state of the task that just finished
        if let Some(mut task) = self.current_task.take() {
            if task.status == super::task::TaskStatus::Killed
                || task.status == super::task::TaskStatus::Exited
            {
                crate::serial_println!(
                    "Scheduler: Reaping task {} (status: {:?})",
                    task.id,
                    task.status
                );
            } else {
                task.stack_pointer = stack_pointer;
                task.status = super::task::TaskStatus::Ready;

                // SAVE SSE/FPU STATE
                unsafe {
                    core::arch::asm!("fxsave [{}]", in(reg) &mut task.fpu_state.data);
                }

                self.tasks.push_back(task);
            }
        }

        // 2. Look for the next READY task
        // We loop through the queue to find someone who is awake.
        for _ in 0..self.tasks.len() {
            if let Some(mut task) = self.tasks.pop_front() {
                if task.id == 0 || now >= task.wake_at {
                    // We found a task!
                    let next_sp = task.stack_pointer;
                    task.status = super::task::TaskStatus::Running;

                    // RESTORE SSE/FPU STATE
                    unsafe {
                        core::arch::asm!("fxrstor [{}]", in(reg) &task.fpu_state.data);
                    }

                    self.current_task = Some(task);
                    return next_sp;
                } else {
                    // Still sleeping, put it back at the end of the line
                    self.tasks.push_back(task);
                }
            }
        }

        // 3. Fallback: If NO ONE is ready (which shouldn't happen if Idle is in the queue)
        stack_pointer
    }
}

pub static SCHEDULER: Mutex<Option<Scheduler>> = Mutex::new(None);

pub fn with_scheduler<R>(f: impl FnOnce(&mut Option<Scheduler>) -> R) -> R {
    interrupts::without_interrupts(|| {
        let mut scheduler_lock = SCHEDULER.lock();
        f(&mut scheduler_lock)
    })
}
