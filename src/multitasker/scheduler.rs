use super::task::Task;
use crate::alloc::collections::VecDeque;
use lazy_static::lazy_static;
use spin::Mutex;

pub struct Scheduler {
    tasks: VecDeque<Task>,
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
            let mut task = self.tasks.pop_front()?;

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
            task.stack_pointer = stack_pointer;
            self.tasks.push_back(task);
        }

        // 2. Look for the next READY task
        // We loop through the queue to find someone who is awake.
        for _ in 0..self.tasks.len() {
            if let Some(mut task) = self.tasks.pop_front() {
                if task.id == 0 || now >= task.wake_at {
                    // We found a task!
                    let next_sp = task.stack_pointer;
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

// this lazy_static delays the initialization of the SCHEDULER until it's first accessed
// since we use the heap we need to wait until after the allocator is set up
lazy_static! {
    pub static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
}
